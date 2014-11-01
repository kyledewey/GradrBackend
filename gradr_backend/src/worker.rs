// A worker thread.  Workers get items from the database,
// and process them.

use std::io::timer;
use std::sync::Arc;
use std::time::Duration;

use builder::{WholeBuildable, ToWholeBuildable};
use database::Database;

/// A = key type
/// B = WholeBuildable type
pub fn worker_loop<A : ToWholeBuildable<B>, B : WholeBuildable, C : Database<A>>(db: Arc<C>) {
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

pub mod testing {
    use builder::ToWholeBuildable;
    use builder::testing::TestingRequest;

    impl ToWholeBuildable<TestingRequest> for String {
        fn to_whole_buildable(&self) -> TestingRequest { 
            TestingRequest::new(self.as_slice(), "test/makefile")
        }
    }
}