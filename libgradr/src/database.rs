// Abstraction of the database.
// For the purposes of the backend, the only necessary
// actions are the following:
//
// 1. Put a pending build into the database.
// 2. Grab a pending build from the database, which
//    will immediately undergo being built.
// 3. Put test results into a database, given what
//    build was pending

extern crate postgres;
#[phase(plugin)]
extern crate pg_typeprovider;

use self::postgres::{Connection, ToSql};

use builder::BuildResult;

use self::EntryStatus::{Pending, InProgress, Done};

pub trait DatabaseEntry<A> : Send {
    fn get_base(&self) -> A;
}

impl<A : Send + Clone> DatabaseEntry<A> for A {
    fn get_base(&self) -> A { self.clone() }
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
    pub fn to_int(&self) -> i32 {
        match *self {
            Pending => 0,
            InProgress => 1,
            Done => 2
        }
    }
}

pub mod postgres_db {
    extern crate pg_typeprovider;

    use self::pg_typeprovider::util::Joinable;

    use std::sync::Mutex;

    use super::postgres::{Connection, SslMode, ToSql};

    use builder::BuildResult;
    use super::EntryStatus::{Pending, InProgress, Done};
    use super::{Database, DatabaseEntry};

    pg_table!(builds)

    pub struct PostgresDatabase {
        db: Mutex<Connection>
    }

    impl PostgresDatabase {
        pub fn new(loc: &str) -> Option<PostgresDatabase> {
            Connection::connect(loc, &SslMode::None).ok().map(|db| {
                PostgresDatabase {
                    db: Mutex::new(db)
                }
            })
        }

        pub fn new_testing() -> Option<PostgresDatabase> {
            let retval = PostgresDatabase::new(
                "postgres://jroesch@localhost/gradr-testing");
            match retval {
                Some(ref db) => {
                    let lock = db.db.lock();
                    lock.execute(
                        "DROP TABLE IF EXISTS users", &[]);
                    lock.execute(
                        "DROP TABLE IF EXISTS builds", &[]);
                    lock.execute(
                        "CREATE TABLE users (
                            id SERIAL,
                            email varchar(500),
                            first_name varchar(500),
                            last_name varchar(500),
                            access_token varchar(500),
                            created_at timestamp without time zone,
                            updated_at timestamp without time zone,
                            github_username varchar(500),
                            password_digest varchar(500)
                            )", &[]);
                    lock.execute(
                        "CREATE TABLE builds (
                            id SERIAL,
                            status int,
                            clone_url text,
                            branch text,
                            results text
                            )", &[]);
                    },
                None => ()
            };
            retval
        }
    }

    impl DatabaseEntry<BuildInsert> for Build {
        fn get_base(&self) -> BuildInsert {
            BuildInsert {
                status: self.status,
                clone_url: self.clone_url.clone(),
                branch: self.branch.clone(),
                results: self.results.clone()
            }
        }
    }

    fn get_one_build(conn: &Connection) -> Option<Build> {
        let stmt = conn.prepare(
            "SELECT id, status, clone_url, branch, results FROM builds WHERE status=$1 LIMIT 1").unwrap();
        for row in stmt.query(&[&(&Pending).to_int()]).unwrap() {
            return Some(Build {
                id: row.get(0),
                status: row.get(1),
                clone_url: row.get(2),
                branch: row.get(3),
                results: row.get(4)
            })
        }
        
        None
    }

    // returns true if it was able to lock it, else false
    fn try_lock_build(conn: &Connection, b: &Build) -> bool {
        conn.execute(
            "UPDATE builds SET status=$1 WHERE id=$2 AND status=$3",
            &[&(&InProgress).to_int(), &b.id, &(&Pending).to_int()])
            .unwrap() == 1
    }

    impl Database<BuildInsert, Build> for PostgresDatabase {
        fn add_pending(&self, entry: BuildInsert) {
            entry.insert(&*self.db.lock());
        }

        fn get_pending(&self) -> Option<Build> {
            let conn: &Connection = &*self.db.lock();
            loop {
                match get_one_build(conn) {
                    Some(b) => {
                        if try_lock_build(conn, &b) {
                            return Some(b);
                        }
                    },
                    None => { return None; }
                }
            }
        }

        fn add_test_results(&self, entry: Build, results: BuildResult) {
            self.db.lock().execute(
                "UPDATE builds SET status=$1, results=$2 WHERE id=$3",
                &[&(&Done).to_int(), &results.to_string(), &entry.id]);
        }

        fn results_for_entry(&self, entry: &Build) -> Option<String> {
            let lock = self.db.lock();
            let stmt = lock.prepare(
                "SELECT results FROM builds WHERE id=$1 AND status=$2").unwrap();
            for row in stmt.query(&[&entry.id, &(&Done).to_int()]).unwrap() {
                return Some(row.get(0));
            }
            None
        }
    }
}

/// For integration tests.
pub mod testing {
    use std::collections::HashMap;
    use std::hash::Hash;
    use std::sync::mpmc_bounded_queue::Queue;
    use std::sync::RWLock;

    use builder::BuildResult;
    use super::Database;

    /// Simply a directory to a status.
    pub struct TestDatabase<A> {
        pending: Queue<A>,
        results: RWLock<HashMap<A, BuildResult>>,
    }

    impl<A : Clone + Eq + Send + Hash> TestDatabase<A> {
        pub fn new<A : Eq + Send + Hash>() -> TestDatabase<A> {
            TestDatabase {
                pending: Queue::with_capacity(10),
                results: RWLock::new(HashMap::new())
            }
        }
    }

    impl<A : Clone + Eq + Send + Hash> Database<A, A> for TestDatabase<A> {
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
