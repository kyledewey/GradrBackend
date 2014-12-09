// A worker thread.  Workers get items from the database,
// and process them.

use builder::{WholeBuildable, ToWholeBuildable};
use database::Database;

/// A = key type
/// B = WholeBuildable type
pub fn worker_loop_step<B : WholeBuildable, A : ToWholeBuildable<B>, D : Database>(db: &D) {
    match db.get_pending() {
        Some(ref a) => {
            // cannot do this as a one-liner, because we transfer ownership
            // with the first parameter to `add_test_results`, and the compiler
            // won't allow the `a.to_whole_buildable...` after that
            let res = a.to_whole_buildable().whole_build();
            db.add_test_results(a, res);
        },
        None => ()
    }
}
