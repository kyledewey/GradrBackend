// Abstraction of the database.
// For the purposes of the backend, the only necessary
// actions are the following:
//
// 1. Put a pending build into the database.
// 2. Grab a pending build from the database, which
//    will immediately undergo being built.
// 3. Put test results into a database, given what
//    build was pending

use builder::BuildResult;

/// Type A is some key
pub trait Database<A> {
    fn add_pending(&mut self, entry: A);

    /// Optionally gets a pending build from the database.
    /// If `Some` is returned, it will not be returned again.
    /// If `None` is returned, it is expected that the caller will sleep.
    fn get_pending(&mut self) -> Option<A>;

    fn add_test_results(&mut self, entry: A, results: BuildResult);
}

pub mod testing {
    use std::collections::HashMap;
    use std::sync::mpsc_queue::Queue;

    use builder::BuildResult;
    use super::Database;

    /// Simply a directory to a status.
    pub struct TestDatabase {
        pending: Queue<String>,
        complete: HashMap<String, BuildResult>
    }

    impl TestDatabase {
        pub fn new() -> TestDatabase {
            TestDatabase {
                pending: Queue::new(),
                complete: HashMap::new()
            }
        }
    }

    impl Database<String> for TestDatabase {
        fn add_pending(&mut self, entry: String) {
            self.pending.push(entry);
        }

        fn get_pending(&mut self) -> Option<String> {
            self.pending.casual_pop()
        }
        
        fn add_test_results(&mut self, entry: String, results: BuildResult) {
            self.complete.insert(entry, results);
        }
    }
}
