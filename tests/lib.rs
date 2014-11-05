extern crate gradr_backend;
extern crate hyper;
extern crate github;
extern crate url;

use std::comm::sync_channel;
use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use gradr_backend::database::Database;
use gradr_backend::database::sqlite::SqliteDatabase;
use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::{NotificationSource, GitHubServer,
                                           Convertable};
use gradr_backend::notification_listener::testing::TestNotificationSource;
use gradr_backend::worker::worker_loop_step;

use self::github::server::testing::send_to_server;
use self::github::notification::PushNotification;

use self::url::Url;
use self::hyper::{IpAddr, Ipv4Addr, Port};

static ADDR: IpAddr = Ipv4Addr(127, 0, 0, 1);
static END_TO_END_KEY: &'static str = "test/end_to_end";

/*
// STOPPED HERE
// Code compiles right now, but it wouldn't work because it
// implicitly assumes the database is returning a path to build.
// We need to implement a new kind of testing request which knows
// how to build a github repo, and `end_to_end_github` needs to be
// constrained (in the types) so that we need to make this new kind
// of request.
fn end_to_end_github<N : Convertable<String>, D : Database<String>>(db: D, port: Port) {
    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(db);
    let db2 = db1.clone();
    let db3 = db1.clone();

    let source = GitHubServer::new(ADDR, port);
    let running_source = source.event_loop().unwrap();

    spawn(proc() {
        while running_source.notification_event_loop_step(&*db1) {}
    });

    spawn(proc() {
        while !*done1.read() {
            worker_loop_step(&*db2);
        }
    });

    send_to_server(
        PushNotification {
            clone_url: Url::parse("https://github.com/baxterthehacker/public-repo.git").unwrap(),
            branch: "master".to_string()
        }.to_string().as_slice(), ADDR, 1235u16);
    
    let mut success = false;

    for _ in range(0, 300u) {
        timer::sleep(Duration::milliseconds(10));
        match db3.results_for_entry(END_TO_END_KEY.to_string()) {
            Some(ref s) => {
                assert!(s.contains("test1: Pass"));
                assert!(s.contains("test2: Fail"));
                success = true;
                break;
            },
            None => ()
        }
    }

    let mut val = done2.write();
    *val = true;
    val.downgrade();

    assert!(success);
}
*/
    
fn end_to_end<A : Database<String>>(db: A) {
    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(db);
    let db2 = db1.clone();
    let db3 = db1.clone();

    let (notification_sender, notification_recv) = sync_channel(10);

    spawn(proc() {
        let source = TestNotificationSource::new(notification_recv);
        while source.notification_event_loop_step(&*db1) {}
    });

    spawn(proc() {
        while !*done1.read() {
            worker_loop_step(&*db2);
        }
    });

    notification_sender.send(Some(Path::new(END_TO_END_KEY)));
    notification_sender.send(None);

    let mut success = false;

    for _ in range(0, 300u) {
        timer::sleep(Duration::milliseconds(10));
        match db3.results_for_entry(END_TO_END_KEY.to_string()) {
            Some(ref s) => {
                assert!(s.contains("test1: Pass"));
                assert!(s.contains("test2: Fail"));
                success = true;
                break;
            },
            None => ()
        }
    }

    let mut val = done2.write();
    *val = true;
    val.downgrade();

    assert!(success);
}
    
#[test]
fn end_to_end_in_memory() {
    end_to_end(TestDatabase::new());
}

#[test]
fn end_to_end_sqlite() {
    end_to_end(SqliteDatabase::new().unwrap());
}
