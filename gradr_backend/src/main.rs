extern crate gradr_backend;

use std::comm::sync_channel;
use std::io::timer;
use std::sync::Arc;
use std::time::Duration;

use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::NotificationSource;
use gradr_backend::notification_listener::testing::TestNotificationSource;
use gradr_backend::worker::worker_loop_step;

#[cfg(not(test))]
fn main() {
    let (notification_sender, notification_recv) = sync_channel(10);

    let db = Arc::new(TestDatabase::new());
    let c1 = db.clone();
    let c2 = db.clone();

    spawn(proc() {
        let source = TestNotificationSource::new(notification_recv);
        loop {
            source.notification_event_loop_step(&*c1);
        }
    });

    spawn(proc() {
        loop {
            worker_loop_step(&*c2);
        }
    });

    notification_sender.send("test/testing_parsing_nonempty_success".to_string());

    loop {
        timer::sleep(Duration::seconds(1));
        println!("{}", db.results.read().find_equiv(
            &"test/testing_parsing_nonempty_success".to_string()));
    }
}
