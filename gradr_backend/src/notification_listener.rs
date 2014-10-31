use database::Database;

// Listens for notifications from some external source.
// Upon receiving a notification, information gets put into
// a database which is polled upon later.

pub trait NotificationSource<A> {
    fn get_notification(&mut self) -> A;
}

fn process_notification_event<A, B, C : NotificationSource<A>, D : Database<A, B>>(
    source: &mut C, db: &mut D) {
    db.add_pending(source.get_notification());
}

fn notification_event_loop<A, B, C : NotificationSource<A>, D : Database<A, B>>(
    source: &mut C, db: &mut D) {
    loop {
        process_notification_event(source, db);
    }
}
