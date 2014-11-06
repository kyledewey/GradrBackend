extern crate gradr_backend;
extern crate hyper;
extern crate github;
extern crate url;

use std::comm::sync_channel;
use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use gradr_backend::util::MessagingUnwrapper;
use gradr_backend::builder::{WholeBuildable, ToWholeBuildable};
use gradr_backend::database::Database;
use gradr_backend::database::sqlite::SqliteDatabase;
use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::{NotificationSource, GitHubServer};
use gradr_backend::notification_listener::testing::TestNotificationSource;
use gradr_backend::worker::worker_loop_step;

use self::github::server::testing::{send_to_server, SendPush};
use self::github::notification::PushNotification;

use self::url::Url;
use self::hyper::{IpAddr, Ipv4Addr, Port};

static ADDR: IpAddr = Ipv4Addr(127, 0, 0, 1);

fn end_to_end<B : WholeBuildable, E : ToWholeBuildable<B>, D : Database<E>, N : NotificationSource<E>>(
    db: D,
    not_src: N,
    to_send: Vec<E>,
    sender: |E| -> (),
    stop_all: || -> (),
    checker: |&D| -> bool) { // returns true if it expects more results

    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(db);
    let db2 = db1.clone();
    let db3 = db1.clone();
    
    spawn(proc() {
        while not_src.notification_event_loop_step(&*db1) {}
    });

    spawn(proc() {
        while !*done1.read() {
            worker_loop_step(&*db2);
        }
    });

    for e in to_send.into_iter() {
        sender(e);
    }

    stop_all();

    let mut success = false;

    for _ in range(0, 3000u) {
        timer::sleep(Duration::milliseconds(10));
        if !checker(&*db3) {
            success = true;
            break;
        }
    }

    let mut val = done2.write();
    *val = true;
    val.downgrade();

    assert!(success);
}

fn end_to_end_test_not_source<A : Database<Path>>(db: A) {
    let (notification_sender, notification_recv) = sync_channel(10);
    let not_src = TestNotificationSource::new(notification_recv);
    let to_send = vec!(Path::new("test/end_to_end"));
    let sender = |path: Path| {
        notification_sender.send(Some(path));
    };
    let stop_all = || {
        notification_sender.send(None);
    };
    let checker = |db: &A| {
        match db.results_for_entry(&Path::new("test/end_to_end")) {
            Some(ref s) => {
                assert!(s.contains("test1: Pass"));
                assert!(s.contains("test2: Fail"));
                false
            },
            None => true
        }
    };

    end_to_end(db, not_src, to_send, sender, stop_all, checker);
}

fn end_to_end_github_not_source<A : Database<PushNotification>>(db: A, port: Port) {
    let server = GitHubServer::new(ADDR, port);
    let not_src = server.event_loop().unwrap_msg(line!());
    let not1 = 
        PushNotification {
            clone_url: Url::parse("https://github.com/scalableinternetservices/GradrBackend.git").unwrap_msg(line!()),
            branch: "testing".to_string()
        };

    let to_send = vec!(not1);
    let sender = |not: PushNotification| {
        send_to_server(SendPush(not).to_string().as_slice(), ADDR, port)
    };
    let stop_all = || {
        //not_src1.wrapped.send_finish()
    };
    let checker = |db: &A| {
        let not2 = 
            PushNotification {
                clone_url: Url::parse("https://github.com/scalableinternetservices/GradrBackend.git").unwrap_msg(line!()),
                branch: "testing".to_string()
            };

        match db.results_for_entry(&not2) {
            Some(ref s) => {
                assert!(s.contains("test1: Pass"));
                assert!(s.contains("test2: Fail"));
                false
            },
            None => true
        }
    };

    end_to_end(db, not_src, to_send, sender, stop_all, checker);
}

//#[test]
fn end_to_end_test_not_source_in_memory() {
    end_to_end_test_not_source(TestDatabase::<Path>::new());
}

//#[test]
fn end_to_end_test_not_source_sqlite() {
    end_to_end_test_not_source(SqliteDatabase::new().unwrap_msg(line!()));
}

// #[test]
// fn end_to_end_github_not_source_in_memory() {
//     end_to_end_github_not_source(TestDatabase::<PushNotification>::new(), 12345);
// }

#[test]
fn end_to_end_github_not_source_sqlite() {
    end_to_end_github_not_source(SqliteDatabase::new().unwrap_msg(line!()), 12346);
}
