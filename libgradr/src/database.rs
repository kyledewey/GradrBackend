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
extern crate github;

use self::github::notification::PushNotification;
use clone_url::CloneUrl;
use builder::BuildResult;

use self::EntryStatus::{Pending, InProgress, Done};

pub struct PendingBuild {
    pub clone_url: CloneUrl,
    pub branch: String
}

/// Type A is some key
pub trait Database : Sync + Send {
    fn add_pending(&self, entry: PushNotification);

    /// Optionally gets a pending build from the database.
    /// If `Some` is returned, it will not be returned again.
    /// If `None` is returned, it is expected that the caller will sleep.
    fn get_pending(&self) -> Option<PendingBuild>;

    fn add_test_results(&self, entry: &PendingBuild, results: BuildResult);
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
    extern crate time;
    extern crate pg_typeprovider;
    extern crate github;

    use super::PendingBuild;

    use self::github::notification::PushNotification;

    use self::time::Timespec;
    use self::pg_typeprovider::util::Joinable;

    use std::sync::Mutex;

    use super::postgres::{Connection, GenericConnection, SslMode, ToSql};

    use builder::BuildResult;
    use clone_url::CloneUrl;
    use super::EntryStatus::{Pending, InProgress, Done};
    use super::Database;

    pg_table!(builds)
    pg_table!(commits)

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
                "postgres://jroesch@localhost/gradr-test");
            match retval {
                Some(ref db) => {
                    db.with_connection(|conn| {
                        conn.execute(
                            "DELETE FROM users", &[]).unwrap();
                        conn.execute(
                            "DELETE FROM builds", &[]).unwrap();
                    })
                },
                None => ()
            };
            retval
        }

        pub fn with_connection<A>(&self, f: |&Connection| -> A) -> A {
            f(&*self.db.lock())
        }
    }

    fn get_one_build(conn: &GenericConnection) -> Option<Build> {
        BuildSearch::new()
            .where_status((&Pending).to_int())
            .search(conn, Some(1)).pop()
    }

    // returns true if it was able to lock it, else false
    fn try_lock_build(conn: &GenericConnection, b: &Build) -> bool {
        BuildUpdate::new()
            .status_to((&InProgress).to_int())
            .where_id(b.id)
            .where_status((&Pending).to_int())
            .update(conn) == 1
    }

    trait ToPendingBuild {
        fn to_pending_build(&self, conn: &GenericConnection) -> PendingBuild;
    }

    impl ToPendingBuild for Build {
        fn to_pending_build(&self, conn: &GenericConnection) -> PendingBuild {
            let commit = CommitSearch::new()
                .where_id(self.commit_id)
                .search(conn, Some(1))
                .pop()
                .unwrap(); // should be in there
            PendingBuild {
                clone_url: CloneUrl::new_from_str(
                    commit.clone_url.as_slice()).unwrap(),
                branch: commit.branch_name
            }
        }
    }
    
    impl Database for PostgresDatabase {
        fn add_pending(&self, entry: PushNotification) {
            // self.with_connection(|conn| {
            //     let trans = conn.transaction().unwrap();
                
            // self.with_connection(|conn| entry.insert(conn));
        }

        fn get_pending(&self) -> Option<PendingBuild> {
            self.with_connection(|conn| {
                loop {
                    let trans = conn.transaction().unwrap();
                    match get_one_build(&trans) {
                        Some(b) => {
                            let res = try_lock_build(&trans, &b);
                            assert!(res);
                            if trans.commit().is_ok() {
                                return Some(b.to_pending_build(conn))
                            }
                        },
                        None => { return None; }
                    }
                }
            })
        }

        fn add_test_results(&self, entry: &PendingBuild, results: BuildResult) {
            // TODO: we do a copy here because we cannot move into a closure,
            // even though this is what we want.  I spent around 2 hours trying
            // to get around this, but to no avail.

            // let res = results.consume_to_json().to_string();
            // let s = res.as_slice();
            // self.with_connection(|conn| {
            //     let num_updated = 
            //         BuildUpdate::new()
            //         .status_to((&Done).to_int())
            //         .results_to(s.to_string())
            //         .where_id(entry.id)
            //         .update(conn);
            //     assert_eq!(num_updated, 1);
            // });
        }
    }
}
