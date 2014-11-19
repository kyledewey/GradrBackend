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

use self::EntryStatus::{Pending, InProgress, Done};

pub trait DatabaseEntry<A> : Send {
    fn get_base(&self) -> &A;
}

impl<A : Send> DatabaseEntry<A> for A {
    fn get_base(&self) -> &A { self }
}

/// Type A is some key
pub trait Database<A, B : DatabaseEntry<A>> : Sync + Send {
    fn add_pending(&self, entry: A);

    /// Optionally gets a pending build from the database.
    /// If `Some` is returned, it will not be returned again.
    /// If `None` is returned, it is expected that the caller will sleep.
    fn get_pending(&self) -> Option<B>;

    fn add_test_results(&self, entry: B, results: BuildResult);

    fn results_for_entry(&self, entry: &B) -> Option<String>;
}

pub enum EntryStatus {
    Pending,
    InProgress,
    Done
}

impl EntryStatus {
    fn to_int(&self) -> int {
        match *self {
            Pending => 0,
            InProgress => 1,
            Done => 2
        }
    }
}

/*
pub mod postgres {
    extern crate postgres;

    use std::sync::Mutex;

    use self::postgres::{Connection, SslMode};

    use super::Database;

    pub struct PostgresDatabase {
        db: Mutex<Connection>
    }

    impl PostgresDatabase {
        pub fn new(loc: &str) -> Option<PostgresDatabase> {
            Connection::connect(loc, &SslMode::None).map(|db| {
                PostgresDatabase {
                    db: Mutex::new(db)
                }
            })
        }
    }
}
*/

/// For integration tests.
pub mod testing {
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::sync::mpmc_bounded_queue::Queue;
    use std::sync::RWLock;

    use builder::BuildResult;
    use super::{Database, DatabaseEntry};

    /// Simply a directory to a status.
    pub struct TestDatabase<A> {
        pending: Queue<A>,
        results: RWLock<HashMap<A, BuildResult>>,
    }

    impl<A : Eq + Send + Hash> TestDatabase<A> {
        pub fn new<A : Eq + Send + Hash>() -> TestDatabase<A> {
            TestDatabase {
                pending: Queue::with_capacity(10),
                results: RWLock::new(HashMap::new())
            }
        }
    }

    impl<A : Eq + Send + Hash> Database<A, A> for TestDatabase<A> {
        fn add_pending(&self, entry: A) {
            self.pending.push(entry);
        }

        fn get_pending(&self) -> Option<A> {
            self.pending.pop()
        }
        
        fn add_test_results(&self, entry: A, results: BuildResult) {
            let mut val = self.results.write();
            val.insert(entry, results);
            val.downgrade();
        }

        fn results_for_entry(&self, entry: &A) -> Option<String> {
            self.results.read().get(entry).map(|res| res.to_string())
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::Database;
    use super::testing::TestDatabase;

    use builder::BuildResult::TestSuccess;

    use std::collections::HashMap;

    static KEY : &'static str = "foobar";
            
    fn add_get_pending<D : Database<Path, Path>>(db: &D) {
        db.add_pending(Path::new(KEY));
        let actual = db.get_pending().and_then(|pending| {
            pending.as_str().map(|s| s.to_string())
        });
        let expected = Path::new(KEY).as_str().map(|s| s.to_string());
        assert_eq!(actual, expected);
    }

    fn add_test_results<D : Database<Path, Path>>(db: &D) {
        add_get_pending(db);
        db.add_test_results(Path::new(KEY),
                            TestSuccess(HashMap::new()));
    }
        
    #[test]
    fn memory_add_get_pending() {
        add_get_pending(&TestDatabase::<Path>::new());
    }

    #[test]
    fn memory_add_test_results() {
        add_test_results(&TestDatabase::<Path>::new());
    }
}
