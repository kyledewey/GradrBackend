// A worker thread.  Workers get items from the database,
// and process them.

use std::io::timer;
use std::time::Duration;

use builder::ToWholeBuildable;
use database::Database;

pub fn worker_loop<A : ToWholeBuildable, B : Database<A>>(db: &mut B) {
    loop {
        match db.get_pending() {
            Some(a) => {
                // cannot do this as a one-liner, because we transfer ownership
                // with the first parameter to `add_test_results`, and the compiler
                // won't allow the `a.to_whole_buildable...` after that
                let res = a.to_whole_buildable().whole_build();
                db.add_test_results(a, res);
            },
            None => timer::sleep(Duration::seconds(1))
        }
    }
}
