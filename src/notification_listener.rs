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

pub trait Convertable<A> {
    fn convert(self) -> A;
}

impl<A> Convertable<A> for A {
    fn convert(self) -> A { self }
}

impl Convertable<String> for PushNotification {
    fn convert(self) -> String {
        format!("{}\t{}", self.clone_url, self.branch)
    }
}

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<Rep, Not : Convertable<Rep>> {
    fn get_notification(&self) -> Option<Not>;

    /// Returns true if processing should continue, else false
    fn notification_event_loop_step<D : Database<Rep>>(&self, db: &D) -> bool {
        match self.get_notification() {
            Some(not) => {
                db.add_pending(not.convert());
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

impl NotificationSource<String, PushNotification> for RunningServer {
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
    use super::{NotificationSource, Convertable};

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

    impl Convertable<String> for Path {
        fn convert(self) -> String {
            self.as_str().unwrap().to_string()
        }
    }

    impl NotificationSource<String, Path> for TestNotificationSource {
        fn get_notification(&self) -> Option<Path> {
            self.source.recv()
        }
    }
}
