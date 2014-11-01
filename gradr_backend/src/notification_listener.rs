use database::Database;

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<A> {
    fn get_notification(&self) -> Option<A>;

    /// Returns true if processing should continue, else false
    fn notification_event_loop_step<D : Database<A>>(&self, db: &D) -> bool {
        match self.get_notification() {
            Some(not) => {
                db.add_pending(not);
                true
            },
            None => false
        }
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
        fn get_notification(&self) -> Option<String> {
            let retval = self.source.recv();
            if retval.as_slice() == "TERMINATE" {
                None
            } else {
                Some(retval)
            }
        }
    }
}
