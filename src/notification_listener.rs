extern crate github;
extern crate hyper;

use self::hyper::HttpResult;
use std::comm::{Receiver, SyncSender};
use std::sync::RWLock;
use database::Database;

use self::github::server::{NotificationReceiver, NotificationListener,
                           ConnectionCloser};
use self::github::notification::PushNotification;

use self::hyper::{IpAddr, Port};

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

struct SenderWrapper {
    wrapped: SyncSender<Option<PushNotification>>
}

impl NotificationReceiver for SenderWrapper {
    fn receive_push_notification(&self, not: PushNotification) {
        self.wrapped.send(Some(not));
    }
}

impl NotificationSource<PushNotification> for Receiver<Option<PushNotification>> {
    fn get_notification(&self) -> Option<PushNotification> {
        self.recv()
    }
}

struct GitHubServer<'a> {
    conn: NotificationListener<'a, SenderWrapper>,
    recv: Receiver<Option<PushNotification>>,
    send_kill_to: SyncSender<Option<PushNotification>>
}

impl<'a> NotificationSource<PushNotification> for GitHubServer<'a> {
    fn get_notification(&self) -> Option<PushNotification> {
        self.recv.get_notification()
    }
}

impl<'a> GitHubServer<'a> {
    fn new<'a>(addr: IpAddr, port: Port) -> GitHubServer<'a> {
        let (tx, rx) = sync_channel(100);
        GitHubServer {
            conn: NotificationListener::new(
                addr, port, 
                SenderWrapper { wrapped: tx.clone() }),
            recv: rx,
            send_kill_to: tx
        }
    }

    fn event_loop(self) ->HttpResult<ConnectionCloser> {
        self.conn.event_loop()
    }

    fn send_finish(&self) {
        self.send_kill_to.send(None)
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
