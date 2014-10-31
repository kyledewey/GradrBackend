use database::Database;

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<A> {
    fn get_notification(&mut self) -> A;
}

pub fn notification_event_loop<A, B : NotificationSource<A>, C : Database<A>>(
    source: &mut B, db: &mut C) {
    loop {
        db.add_pending(source.get_notification());
    }
}
