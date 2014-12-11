extern crate hyper;
extern crate libgradr;
extern crate github;
extern crate serialize;
extern crate postgres;

use self::hyper::{IpAddr, Ipv4Addr, Port};

use self::github::server::testing::{Sendable, send_to_server};
use self::github::server::testing::Sendable::SendPush;
use self::github::notification::PushNotification;
use self::github::clone_url::CloneUrl;

use self::postgres::GenericConnection;

use libgradr::database::Database;
use libgradr::database::postgres_db::{PostgresDatabase, Build, BuildSearch,
                                      Commit, CommitSearch};
use libgradr::database::EntryStatus::Done;
use libgradr::notification_listener::{NotificationSource, GitHubServer};
use libgradr::util::MessagingUnwrapper;
use libgradr::worker::worker_loop_step;

use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use serialize::json::{ToJson, from_str};

static ADDR: IpAddr = Ipv4Addr(127, 0, 0, 1);

fn end_to_end<D: Database>(db: D,
                           port: Port,
                           send: Vec<Sendable>,
                           is_done: |&D| -> bool) { 
    let server = GitHubServer::new(ADDR, port);
    let running_server = server.event_loop().unwrap_msg(line!());

    let len = send.len();

    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(db);
    let db2 = db1.clone();
    let db3 = db1.clone();

    // notification sender
    spawn(proc() {
        for _ in range(0, len) {
            let res = running_server.notification_event_loop_step(&*db1);
            assert!(res);
        }
        running_server.send_finish();
    });

    // worker
    spawn(proc() {
        while !*done1.read() {
            worker_loop_step(&*db2);
        }
    });

    for e in send.iter() {
        send_to_server(e.to_string().as_slice(), ADDR, port)
    }

    let mut success = false;

    for _ in range(0, 600u) {
        timer::sleep(Duration::milliseconds(10));

        if is_done(&*db3) {
            success = true;
            break;
        }
    }

    let mut val = done2.write();
    *val = true;
    assert!(success);
}

#[test]
fn full_end_to_end() {
    let clone_url =
        "https://github.com/scalableinternetservices/GradrBackend.git";
    let branch = "testing";

    let get_commit: |&GenericConnection| -> Option<Commit> = |conn| {
        CommitSearch::new()
            .where_clone_url(clone_url.to_string())
            .where_branch_name(branch.to_string())
            .search(conn, Some(1))
            .pop()
    };

    let get_build: |&GenericConnection, &Commit| -> Option<Build> = |conn, commit| {
        BuildSearch::new()
            .where_commit_id(commit.get_id())
            .where_status((&Done).to_int())
            .search(conn, Some(1))
            .pop()
    };

    let is_done: |&PostgresDatabase| -> bool = |db: &PostgresDatabase| {
        db.with_connection(|conn| {
            get_commit(conn).and_then(|commit| {
                get_build(conn, &commit).map(|build| {
                    let json = from_str(build.results.as_slice());
                    assert!(json.is_ok());

                    let json = json.unwrap();
                    let obj = json.as_object();
                    assert!(obj.is_some());

                    let obj = obj.unwrap();
                    let res = obj.get(&"success".to_string());
                    assert!(res.is_some());

                    let res = res.unwrap().as_object();
                    assert!(res.is_some());

                    let res = res.unwrap();
                    assert_eq!(res.get(&"test1".to_string()),
                               Some(&true.to_json()));
                    assert_eq!(res.get(&"test2".to_string()),
                               Some(&false.to_json()));
                    true
                })
            }).unwrap_or(false)
        })
    }; // is_done

    end_to_end(PostgresDatabase::new_testing().unwrap(),
               12347,
               vec!(
                   SendPush(PushNotification {
                       clone_url: CloneUrl::new_from_str(
                           "https://github.com/scalableinternetservices/GradrBackend.git").unwrap(),
                       branch: "testing".to_string()
                   })),
               is_done);
}
