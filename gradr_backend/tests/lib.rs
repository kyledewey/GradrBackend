extern crate gradr_backend;

use std::collections::HashMap;
use std::comm::sync_channel;
use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use gradr_backend::builder::{Pass, Fail, TestSuccess, TestResult};
use gradr_backend::database::Database;
use gradr_backend::database::sqlite::SqliteDatabase;
use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::NotificationSource;
use gradr_backend::notification_listener::testing::TestNotificationSource;
use gradr_backend::worker::worker_loop_step;


static END_TO_END_KEY: &'static str = "test/end_to_end";

/// Basic idea: `get_res` returns true if we got a result, else false.
/// `get_res` also checks if the result was expected.  For Sqlite, we
/// can just do string comparison.
fn end_to_end<A : Database<String>>(db: A, get_res: |&A| -> bool) {
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

    notification_sender.send(END_TO_END_KEY.to_string());
    notification_sender.send("TERMINATE".to_string());

    let mut success = false;

    for _ in range(0, 300u) {
        timer::sleep(Duration::milliseconds(10));
        if get_res(&*db3) {
            success = true;
            break;
        }
    }

    let mut val = done2.write();
    *val = true;
    val.downgrade();

    assert!(success);
}
    
#[test]
fn end_to_end_in_memory() {
    fn results_ok(map: &HashMap<String, TestResult>) {
        let t1 = &"test1".to_string();
        let p = &Pass;
        let expect1 = Some(p);
        let t2 = &"test2".to_string();
        let f = &Fail;
        let expect2 = Some(f);

        assert_eq!(map.find_equiv(t1), expect1);
        assert_eq!(map.find_equiv(t2), expect2);
    }

    end_to_end(
        TestDatabase::new(),
        |db: &TestDatabase| {
            match db.results.read().find_equiv(&END_TO_END_KEY.to_string()) {
                Some(&TestSuccess(ref map)) => {
                    results_ok(map);
                    true
                },
                Some(_) => { assert!(false); false },
                None => false
            }
        });
}

#[test]
fn end_to_end_sqlite() {
    end_to_end(
        SqliteDatabase::new().unwrap(),
        |db: &SqliteDatabase| {
            match db.results_for_entry(END_TO_END_KEY) {
                Some(ref s) => {
                    assert!(s.contains("test1: Pass"));
                    assert!(s.contains("test2: Fail"));
                    true
                }
                None => false
            }
        });
}
