extern crate gradr_backend;

use std::comm::sync_channel;

use gradr_backend::database::testing::TestDatabase;
use gradr_backend::notification_listener::testing::TestNotificationSource;
    

#[cfg(not(test))]
fn main() {
/*    let mut db = TestDatabase::new();
    let (sender, recv) = sync_channel(10);

    spawn(proc() {
        TestNotificationSource::new(recv).notification_event_loop(db)
    });

    spawn(proc()
*/
}
