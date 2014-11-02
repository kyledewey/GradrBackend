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
pub trait Database<A> : Sync + Send {
    fn add_pending(&self, entry: A);

    /// Optionally gets a pending build from the database.
    /// If `Some` is returned, it will not be returned again.
    /// If `None` is returned, it is expected that the caller will sleep.
    fn get_pending(&self) -> Option<A>;

    fn add_test_results(&self, entry: A, results: BuildResult);
}

pub mod testing {
    use std::collections::HashMap;
    use std::sync::mpmc_bounded_queue::Queue;
    use std::sync::RWLock;

    use builder::BuildResult;
    use super::Database;

    /// Simply a directory to a status.
    pub struct TestDatabase {
        pending: Queue<String>,
        pub results: RWLock<HashMap<String, BuildResult>>, // pub is HACK
    }

    impl TestDatabase {
        pub fn new() -> TestDatabase {
            TestDatabase {
                pending: Queue::with_capacity(10),
                results: RWLock::new(HashMap::new())
            }
        }
    }

    impl Database<String> for TestDatabase {
        fn add_pending(&self, entry: String) {
            self.pending.push(entry);
        }

        fn get_pending(&self) -> Option<String> {
            self.pending.pop()
        }
        
        fn add_test_results(&self, entry: String, results: BuildResult) {
            let mut val = self.results.write();
            val.insert(entry, results);
            val.downgrade();
        }
    }
}

pub mod sqlite {
    extern crate sqlite3;

    use std::sync::Mutex;

    use builder::BuildResult;
    
    use self::sqlite3::types::SqliteResult;
    use self::sqlite3::types::{SQLITE_ROW, SQLITE_DONE};
    use self::sqlite3::database::Database as SqliteDatabaseInternal;

    use super::Database;

    pub struct SqliteDatabase {
        db: Mutex<SqliteDatabaseInternal>
    }

    enum EntryStatus {
        Pending,
        InProgress,
        Done
    }

    fn status_to_int(status: &EntryStatus) -> int {
        match *status {
            Pending => 0,
            InProgress => 1,
            Done => 2
        }
    }

    impl SqliteDatabase {
        pub fn new() -> SqliteResult<SqliteDatabase> {
            let mut db = try!(sqlite3::open(":memory:"));
            try!(db.exec(
                "CREATE TABLE tbl(entry TEXT PRIMARY KEY, status INTEGER NOT NULL, results Text)"));
            Ok(SqliteDatabase { db: Mutex::new(db) })
        }

        fn get_candidate_entry(&self) -> Option<String> {
            self.read_one_text(
                format!("SELECT entry FROM tbl WHERE status = {} LIMIT 1",
                        status_to_int(&Pending)).as_slice())
        }

        fn try_lock_entry(&self, entry: &String) -> bool {
            self.db.lock().exec(
                format!("UPDATE tbl SET status = {} WHERE entry = \"{}\" AND status = {}",
                        status_to_int(&InProgress),
                        entry,
                        status_to_int(&Pending)).as_slice()).ok().expect("ONE");
            match self.db.lock().get_changes() {
                0 => false,
                1 => true,
                _ => { assert!(false); false }
            }
        }

        fn read_one_text(&self, query: &str) -> Option<String> {
            let lock = self.db.lock();
            let mut cursor = lock.prepare(query, &None).ok().expect("TWO");
            if cursor.step() == SQLITE_ROW {
                let entry = cursor.get_text(0).unwrap_or("").to_string();
                let step_res = cursor.step();
                assert_eq!(step_res, SQLITE_DONE);
                Some(entry.to_string())
            } else {
                None
            }
        }

        pub fn results_for_entry(&self, entry: &str) -> Option<String> {
            self.read_one_text(
                format!("SELECT results FROM tbl WHERE entry = \"{}\"", entry).as_slice())
        }
    }

    impl Database<String> for SqliteDatabase {
        fn add_pending(&self, entry: String) {
            let query = format!("INSERT INTO tbl VALUES(\"{}\", {}, NULL)",
                        entry, status_to_int(&Pending));
            self.db.lock().exec(query.as_slice()).ok().expect(query.as_slice());
        }

        fn get_pending(&self) -> Option<String> {
            loop {
                match self.get_candidate_entry() {
                    Some(entry) => {
                        if self.try_lock_entry(&entry) {
                            return Some(entry);
                        }
                    }
                    None => { return None; }
                }
            }
        }

        fn add_test_results(&self, entry: String, results: BuildResult) {
            self.db.lock().exec(
                format!("UPDATE tbl SET status = {}, results = \"{}\" WHERE entry = \"{}\"",
                        status_to_int(&Done),
                        results.to_string(),
                        entry).as_slice()).ok().expect("FOUR");
        }
    }
}
