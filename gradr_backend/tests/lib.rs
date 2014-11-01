extern crate gradr_backend;

use std::collections::HashMap;
use std::comm::sync_channel;
use std::io::timer;
use std::sync::{Arc, RWLock};
use std::time::Duration;

use gradr_backend::builder::{Pass, Fail, TestSuccess, TestResult};
use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::NotificationSource;
use gradr_backend::notification_listener::testing::TestNotificationSource;
use gradr_backend::worker::worker_loop_step;

#[test]
fn end_to_end() {
    let done1 = Arc::new(RWLock::new(false));
    let done2 = done1.clone();

    let db1 = Arc::new(TestDatabase::new());
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

    notification_sender.send("test/end_to_end".to_string());
    notification_sender.send("TERMINATE".to_string());

    let mut success = false;
    let key = &"test/end_to_end".to_string();

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

    for _ in range(0, 300u) {
        timer::sleep(Duration::milliseconds(10));
        match db3.results.read().find_equiv(key) {
            Some(&TestSuccess(ref map)) => {
                results_ok(map);
                success = true;
                break;
            },
            _ => ()
        }
    }

    let mut val = done2.write();
    *val = true;
    val.downgrade();

    assert!(success);
}