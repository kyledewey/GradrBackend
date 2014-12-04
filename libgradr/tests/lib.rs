extern crate libgradr;
extern crate hyper;
extern crate github;
extern crate url;

use std::comm::sync_channel;
use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use libgradr::util::MessagingUnwrapper;
use libgradr::builder::{WholeBuildable, ToWholeBuildable};
use libgradr::database::{Database, DatabaseEntry, Build};
use libgradr::database::testing::TestDatabase;
use libgradr::database::postgres_db::PostgresDatabase;

use libgradr::notification_listener::{NotificationSource, GitHubServer,
                                           RunningServer, AsDatabaseInput};
use libgradr::notification_listener::testing::TestNotificationSource;
use libgradr::worker::worker_loop_step;

use self::github::server::testing::{send_to_server, SendPush};
use self::github::notification::PushNotification;

use self::url::Url;
use self::hyper::{IpAddr, Ipv4Addr, Port};

#[cfg(test)]
static ADDR: IpAddr = Ipv4Addr(127, 0, 0, 1);

#[cfg(test)]
fn end_to_end<A : WholeBuildable, DBIn : ToWholeBuildable<A>, NotIn : AsDatabaseInput<DBIn>, DBOut : DatabaseEntry<DBIn>, DB : Database<DBIn, DBOut>, Not : NotificationSource<DBIn, NotIn>>(
    db: DB,
    not_src: Not,
    to_send: Vec<NotIn>,
    sender: |NotIn| -> (),
    stop_not: fn (&Not) -> (),
    stop_clo: || -> (),
    checker: |&DB| -> bool) { // returns true if it expects more results

    let len = to_send.len();

    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(db);
    let db2 = db1.clone();
    let db3 = db1.clone();
    
    spawn(proc() {
        for _ in range(0, len) {
            let res = not_src.notification_event_loop_step(&*db1);
            assert!(res);
        }
        stop_not(&not_src);
    });

    spawn(proc() {
        while !*done1.read() {
            worker_loop_step(&*db2);
        }
    });

    for e in to_send.into_iter() {
        sender(e);
    }

    stop_clo();

    let mut success = false;

    for _ in range(0, 600u) {
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

#[cfg(test)]
fn end_to_end_test_not_source<A : Database<Path, Path>>(db: A) {
    let (notification_sender, notification_recv) = sync_channel(10);
    let not_src = TestNotificationSource::new(notification_recv);
    let to_send = vec!(Path::new("test/end_to_end"));
    let sender = |path: Path| {
        notification_sender.send(Some(path));
    };
    
    fn stop_not(_: &TestNotificationSource) {}

    let stop_clo = || {
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

    end_to_end(db, not_src, to_send, sender, stop_not, stop_clo, checker);
}

#[cfg(test)]
fn end_to_end_github_not_source(db: PostgresDatabase, port: Port) {
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

    fn stop_not(n: &RunningServer) {
        n.send_finish();
    }

    let stop_clo = || { };


    let checker = |db: &PostgresDatabase| {
        // HACK
        let key = Build {
            id: 1,
            status: 0,
            clone_url: "".to_string(),
            branch: "".to_string(),
            results: "".to_string()
        };

        match db.results_for_entry(&key) {
            Some(ref s) => {
                assert!(s.contains("test1: Pass"));
                assert!(s.contains("test2: Fail"));
                false
            },
            None => true
        }
    };

    end_to_end(db, not_src, to_send, sender, stop_not, stop_clo, checker);
}


#[test]
fn end_to_end_test_not_source_in_memory() {
    end_to_end_test_not_source(TestDatabase::<Path>::new());
}

/*
#[test]
fn end_to_end_github_not_source_in_memory() {
    end_to_end_github_not_source(TestDatabase::<Path>::new(), 12346);
}
*/

#[test]
fn end_to_end_github_not_source_postgres() {
    end_to_end_github_not_source(
        PostgresDatabase::new_testing().unwrap(),
        12347);
}
