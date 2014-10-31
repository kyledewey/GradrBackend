// A worker thread.  Workers get items from the database,
// and process them.

use std::io::timer;
use std::time::Duration;

use builder::ToWholeBuildable;
use database::Database;

pub fn worker_loop<A : ToWholeBuildable, B : Database<A>>(db: &mut B) {
    loop {
        match db.get_pending() {
            Some(ref a) => db.add_test_results(*a, a.to_whole_buildable().whole_build()),
            None => timer::sleep(Duration::seconds(1))
        }
    }
}
