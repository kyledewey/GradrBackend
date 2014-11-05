extern crate github;
extern crate hyper;
extern crate url;

use self::hyper::HttpResult;
use std::comm::{Receiver, SyncSender};
use database::{Database, StringInterconvertable};

use self::github::server::{NotificationReceiver, NotificationListener,
                           ConnectionCloser};
use self::github::notification::PushNotification;

use self::hyper::{IpAddr, Port};
use self::url::Url;

impl StringInterconvertable for PushNotification {
    fn convert_to_string(&self) -> String {
        format!("{}\t{}", self.clone_url, self.branch)
    }

    fn convert_from_string(s: String) -> PushNotification {
        let v: Vec<&str> = s.as_slice().split('\t').collect();
        assert_eq!(v.len(), 2);
        PushNotification {
            clone_url: Url::parse(v[0]).unwrap(),
            branch: v[1].to_string()
        }
    }
}

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<A> : Send {
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

pub struct GitHubServer<'a> {
    conn: NotificationListener<'a, SenderWrapper>,
    recv: Receiver<Option<PushNotification>>,
    send_kill_to: SyncSender<Option<PushNotification>>
}

impl NotificationSource<PushNotification> for RunningServer {
    fn get_notification(&self) -> Option<PushNotification> {
        self.recv.recv()
    }
}

pub struct RunningServer {
    close: ConnectionCloser,
    recv: Receiver<Option<PushNotification>>,
    send_kill_to: SyncSender<Option<PushNotification>>
}

impl RunningServer {
    pub fn send_finish(&self) {
        self.send_kill_to.send(None)
    }
}

impl<'a> GitHubServer<'a> {
    pub fn new<'a>(addr: IpAddr, port: Port) -> GitHubServer<'a> {
        let (tx, rx) = sync_channel(100);
        GitHubServer {
            conn: NotificationListener::new(
                addr, port, 
                SenderWrapper { wrapped: tx.clone() }),
            recv: rx,
            send_kill_to: tx
        }
    }

    pub fn event_loop(self) ->HttpResult<RunningServer> {
        let recv = self.recv;
        let send_kill = self.send_kill_to;
        let close = try!(self.conn.event_loop());
        Ok(RunningServer {
            close: close,
            recv: recv,
            send_kill_to: send_kill
        })
    }
}

pub mod testing {
    use std::comm::Receiver;
    use super::NotificationSource;

    pub struct TestNotificationSource {
        source: Receiver<Option<Path>>
    }

    impl TestNotificationSource {
        pub fn new(source: Receiver<Option<Path>>) -> TestNotificationSource {
            TestNotificationSource {
                source: source
            }
        }
    }

    impl NotificationSource<Path> for TestNotificationSource {
        fn get_notification(&self) -> Option<Path> {
            self.source.recv()
        }
    }
}
