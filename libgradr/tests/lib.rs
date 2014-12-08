extern crate libgradr;
extern crate hyper;
extern crate github;
extern crate url;

use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use libgradr::util::MessagingUnwrapper;
use libgradr::builder::{WholeBuildable, ToWholeBuildable};
use libgradr::database::{Database, DatabaseEntry};
use libgradr::database::EntryStatus::Done;
use libgradr::database::postgres_db::{PostgresDatabase, BuildSearch};

use libgradr::notification_listener::{NotificationSource, GitHubServer,
                                      RunningServer, AsDatabaseInput};
use libgradr::worker::worker_loop_step;

use self::github::server::testing::send_to_server;
use self::github::server::testing::Sendable::SendPush;
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
fn end_to_end_github_not_source(db: PostgresDatabase, port: Port) {
    let server = GitHubServer::new(ADDR, port);
    let not_src = server.event_loop().unwrap_msg(line!());
    let clone_url =
        "https://github.com/scalableinternetservices/GradrBackend.git";
    let branch = "testing";

    let not1 = 
        PushNotification {
            clone_url: Url::parse(clone_url).unwrap_msg(line!()),
            branch: branch.to_string()
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
        db.with_connection(|conn| {
            BuildSearch::new()
                .where_clone_url(clone_url.to_string())
                .where_branch(branch.to_string())
                .where_status((&Done).to_int())
                .search(conn, Some(1))
                .pop()
                .map(|build| {
                    let results = &build.results;
                    assert!(results.contains("test1: Pass"));
                    assert!(results.contains("test2: Fail"));
                    false
                }).unwrap_or(true)
        })
    };

    end_to_end(db, not_src, to_send, sender, stop_not, stop_clo, checker);
}


#[test]
fn end_to_end_github_not_source_postgres() {
    end_to_end_github_not_source(
        PostgresDatabase::new_testing().unwrap(),
        12347);
}
