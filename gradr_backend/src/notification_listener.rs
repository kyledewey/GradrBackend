use database::Database;

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<A> {
    fn get_notification(&self) -> A;

    fn notification_event_loop_step<D : Database<A>>(&self, db: &D) {
        db.add_pending(self.get_notification());
    }
}

pub mod testing {
    use std::comm::Receiver;
    use super::NotificationSource;

    pub struct TestNotificationSource {
        source: Receiver<String>
    }

    impl TestNotificationSource {
        pub fn new(source: Receiver<String>) -> TestNotificationSource {
            TestNotificationSource {
                source: source
            }
        }
    }

    impl NotificationSource<String> for TestNotificationSource {
        fn get_notification(&self) -> String {
            self.source.recv()
        }
    }
}
