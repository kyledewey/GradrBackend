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

static TABLE_NAME : &'static str = "tbl";

pub trait StringInterconvertable {
    fn convert_to_string(&self) -> String;
    fn convert_from_string(s: String) -> Self;
}

impl StringInterconvertable for Path {
    fn convert_to_string(&self) -> String {
        self.as_str().unwrap().to_string()
    }

    fn convert_from_string(s: String) -> Path {
        Path::new(s)
    }
}

/// Type A is some key
pub trait Database<A> : Sync + Send {
    fn add_pending(&self, entry: A);

    /// Optionally gets a pending build from the database.
    /// If `Some` is returned, it will not be returned again.
    /// If `None` is returned, it is expected that the caller will sleep.
    fn get_pending(&self) -> Option<A>;

    fn add_test_results(&self, entry: A, results: BuildResult);

    fn results_for_entry(&self, entry: &A) -> Option<String>;
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

// TODO: we are abstracting over SQLite3 bindings, which do not
// provide proper parameter passing and are thus susceptible to
// injection attacks.  We should make it so we only care about
// Postgres.
pub trait SqlDatabaseInterface {
    /// Executes the given query, and returns the number of rows modified
    /// as a result.
    fn execute_query(&self, query: &str) -> uint;

    /// Reads a textual column from a query which is expected to
    /// return one text column, and at most one row.  Returns None if
    /// there were no rows returned.
    fn read_one_string(&self, query: &str) -> Option<String>;
}

mod sql_db_helpers {
    use super::{SqlDatabaseInterface, TABLE_NAME, Pending, InProgress};

    pub fn get_candidate_entry<T : SqlDatabaseInterface>(t: &T) -> Option<String> {
        t.read_one_string(
            format!("SELECT entry FROM {} WHERE status = {} LIMIT 1",
                    TABLE_NAME, Pending.to_int()).as_slice())
    }

    pub fn try_lock_entry<T : SqlDatabaseInterface>(t: &T, entry: &String) -> bool {
        let num_changed = 
            t.execute_query(
                format!("UPDATE {} SET status = {} WHERE entry = \"{}\" AND status = {}",
                        TABLE_NAME,
                        InProgress.to_int(),
                        entry,
                        Pending.to_int()).as_slice());
        match num_changed {
            0 => false,
            1 => true,
            _ => { assert!(false); false }
        }
    }
}

impl<T : SqlDatabaseInterface + Send + Sync, E : StringInterconvertable>
    Database<E> for T {
    fn add_pending(&self, entry: E) {
        self.execute_query(
            format!("INSERT INTO {} VALUES (\"{}\", {}, NULL)",
                    TABLE_NAME,
                    entry.convert_to_string(),
                    Pending.to_int()).as_slice());
    }

    fn get_pending(&self) -> Option<E> {
        loop {
            match sql_db_helpers::get_candidate_entry(self) {
                Some(entry) => {
                    if sql_db_helpers::try_lock_entry(self, &entry) {
                        return Some(StringInterconvertable::convert_from_string(entry));
                    }
                }
                None => { return None; }
            }
        }
    }

    fn add_test_results(&self, entry: E, results: BuildResult) {
        let num_changed = 
            self.execute_query(
                format!("UPDATE {} SET status = {}, results = \"{}\" WHERE entry = \"{}\"",
                        TABLE_NAME,
                        Done.to_int(),
                        results.to_string(),
                        entry.convert_to_string()).as_slice());
        assert_eq!(num_changed, 1);
    }

    fn results_for_entry(&self, entry: &E) -> Option<String> {
        let query = 
            format!("SELECT results FROM {} WHERE entry = \"{}\"",
                    TABLE_NAME, entry.convert_to_string());
        self.read_one_string(query.as_slice())
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

    impl<A : Eq + Send + Hash> TestDatabase<A> {
        pub fn new<A : Eq + Send + Hash>() -> TestDatabase<A> {
            TestDatabase {
                pending: Queue::with_capacity(10),
                results: RWLock::new(HashMap::new())
            }
        }
    }

    impl<A : Eq + Send + Hash> Database<A> for TestDatabase<A> {
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
            self.results.read().find(entry).map(|res| res.to_string())
        }
    }
}

pub mod sqlite {
    extern crate sqlite3;

    use std::sync::Mutex;

    use self::sqlite3::types::SqliteResult;
    use self::sqlite3::types::{SQLITE_ROW, SQLITE_DONE};
    use self::sqlite3::database::Database as SqliteDatabaseInternal;

    use super::{SqlDatabaseInterface, TABLE_NAME};

    pub struct SqliteDatabase {
        db: Mutex<SqliteDatabaseInternal>
    }

    impl SqliteDatabase {
        pub fn new() -> SqliteResult<SqliteDatabase> {
            let mut db = try!(sqlite3::open("data.db"));

            try!(
                db.exec(
                    format!(
                        "CREATE TABLE {}(entry TEXT PRIMARY KEY, status INTEGER NOT NULL, results Text)",
                        TABLE_NAME).as_slice()));
            Ok(SqliteDatabase { db: Mutex::new(db) })
        }
    }

    impl SqlDatabaseInterface for SqliteDatabase {
        fn execute_query(&self, query: &str) -> uint {
            let mut lock = self.db.lock();
            lock.exec(query).ok().expect(query);
            lock.get_changes().to_uint().unwrap()
        }

        fn read_one_string(&self, query: &str) -> Option<String> {
            let lock = self.db.lock();
            let mut cursor = lock.prepare(query, &None).ok().expect("TWO");
            let step_one = cursor.step();
            if step_one == SQLITE_ROW {
                let op_text = cursor.get_text(0).map(|s| s.to_string());
                let step_two = cursor.step();
                assert_eq!(step_two, SQLITE_DONE);
                op_text
            } else {
                None
            }
        }
    }
}

pub mod postgres {
    extern crate postgres;

    use std::sync::Mutex;

    use self::postgres::{Connection, NoSsl};

    use super::{SqlDatabaseInterface, TABLE_NAME};

    pub struct PostgresDatabase {
        db: Mutex<Connection>
    }
    
    impl PostgresDatabase {
        pub fn new(loc: &str) -> Option<PostgresDatabase> {
            match Connection::connect(loc, &NoSsl).ok() {
                Some(db) => {
                    db.execute(
                        format!(
                            "CREATE TABLE {}(entry TEXT PRIMARY KEY, status INTEGER NOT NULL, results Text)",
                            TABLE_NAME).as_slice(), []).unwrap();
                    Some(PostgresDatabase { db: Mutex::new(db) })
                },
                None => None
            }
        }
    }

    impl SqlDatabaseInterface for PostgresDatabase {
        fn execute_query(&self, query: &str) -> uint {
            self.db.lock().execute(query, []).unwrap()
        }

        fn read_one_string(&self, query: &str) -> Option<String> {
            let lock = self.db.lock();
            let stmt = lock.prepare(query).unwrap();
            let mut rows = stmt.query([]).unwrap();
            let op_row_one = rows.next();

            match op_row_one {
                Some(ref row) => {
                    let op_row_two = rows.next();
                    assert!(op_row_two.is_none());
                    Some(row.get(0))
                },
                None => None
            }
        }
    }
}
        
#[cfg(test)]
pub mod tests {
    use super::Database;
    use super::sqlite::SqliteDatabase;
    use super::testing::TestDatabase;

    use builder::TestSuccess;

    use std::collections::HashMap;

    static KEY : &'static str = "foobar";
            
    fn add_get_pending<D : Database<Path>>(db: &D) {
        db.add_pending(Path::new(KEY));
        let actual = db.get_pending().and_then(|pending| {
            pending.as_str().map(|s| s.to_string())
        });
        let expected = Path::new(KEY).as_str().map(|s| s.to_string());
        assert_eq!(actual, expected);
    }

    fn add_test_results<D : Database<Path>>(db: &D) {
        add_get_pending(db);
        db.add_test_results(Path::new(KEY),
                            TestSuccess(HashMap::new()));
    }
        
    //#[test]
    fn memory_add_get_pending() {
        add_get_pending(&TestDatabase::<Path>::new());
    }

    //#[test]
    fn memory_add_test_results() {
        add_test_results(&TestDatabase::<Path>::new());
    }

    //#[test]
    fn sqlite_add_get_pending() {
        add_get_pending(&SqliteDatabase::new().unwrap());
    }

    //#[test]
    fn sqlite_add_test_results() {
        add_test_results(&SqliteDatabase::new().unwrap());
    }
}
