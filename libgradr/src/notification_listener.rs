extern crate github;
extern crate hyper;
extern crate url;

use self::hyper::HttpResult;
use std::comm::{Receiver, SyncSender};
use database::{Database, DatabaseEntry, BuildInsert, EntryStatus};

use self::github::server::{NotificationReceiver, NotificationListener,
                           ConnectionCloser};
use self::github::notification::PushNotification;

use self::hyper::{IpAddr, Port};
use self::url::Url;

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait AsDatabaseInput<A> {
    fn as_database_input(self) -> A;
}

impl<A> AsDatabaseInput<A> for A {
    fn as_database_input(self) -> A { self }
}

pub trait NotificationSource<B, A : AsDatabaseInput<B>> : Send {
    fn get_notification(&self) -> Option<A>;

    /// Returns true if processing should continue, else false
    fn notification_event_loop_step<DBOut : DatabaseEntry<B>, DB : Database<B, DBOut>>(&self, db: &DB) -> bool {
        match self.get_notification() {
            Some(not) => {
                db.add_pending(not.as_database_input());
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

impl AsDatabaseInput<BuildInsert> for PushNotification {
    fn as_database_input(self) -> BuildInsert {
        BuildInsert {
            status: (&EntryStatus::Pending).to_int(),
            clone_url: self.clone_url.to_string(),
            branch: self.branch,
            results: "".to_string()
        }
    }
}

impl NotificationSource<BuildInsert, PushNotification> for RunningServer {
    fn get_notification(&self) -> Option<PushNotification> {
        self.recv.recv()
    }
}

pub struct RunningServer {
    closer: ConnectionCloser,
    recv: Receiver<Option<PushNotification>>,
    send_kill_to: SyncSender<Option<PushNotification>>
}

impl RunningServer {
    pub fn send_finish(&self) {
        self.send_kill_to.send(None);
    }
}

impl Drop for RunningServer {
    fn drop(&mut self) {
        self.closer.close();
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
            closer: close,
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

    impl NotificationSource<Path, Path> for TestNotificationSource {
        fn get_notification(&self) -> Option<Path> {
            self.source.recv()
        }
    }
}
