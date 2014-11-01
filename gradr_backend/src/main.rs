extern crate gradr_backend;

// TODO: there must be a better way to not import these for testing
#[cfg(not(test))]
use std::comm::sync_channel;
#[cfg(not(test))]
use std::io::timer;
#[cfg(not(test))]
use std::sync::Arc;
#[cfg(not(test))]
use std::time::Duration;

#[cfg(not(test))]
use gradr_backend::database::testing::TestDatabase;
#[cfg(not(test))]
use gradr_backend::notification_listener::NotificationSource;
#[cfg(not(test))]
use gradr_backend::notification_listener::testing::TestNotificationSource;
#[cfg(not(test))]
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
