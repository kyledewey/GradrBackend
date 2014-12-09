extern crate github;
extern crate hyper;
extern crate url;

use self::hyper::HttpResult;
use std::comm::{Receiver, SyncSender};
use std::sync::Mutex;
use database::Database;

use self::github::server::{NotificationReceiver, NotificationListener,
                           ConnectionCloser};
use self::github::notification::PushNotification;

use self::hyper::{IpAddr, Port};

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource : Send {
    fn get_notification(&self) -> Option<PushNotification>;

    /// Returns true if processing should continue, else false
    fn notification_event_loop_step<D : Database>(&self, db: &D) -> bool {
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
    wrapped: Mutex<SyncSender<Option<PushNotification>>>
}

impl NotificationReceiver for SenderWrapper {
    fn receive_push_notification(&self, not: PushNotification) {
        self.wrapped.lock().send(Some(not));
    }
}

pub struct GitHubServer<'a> {
    conn: NotificationListener<'a, SenderWrapper>,
    recv: Receiver<Option<PushNotification>>,
    send_kill_to: SyncSender<Option<PushNotification>>
}

impl NotificationSource for RunningServer {
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
                SenderWrapper { wrapped: Mutex::new(tx.clone()) }),
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

