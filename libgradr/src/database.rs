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
use self::github::clone_url::CloneUrl;
use builder::BuildResult;

use self::EntryStatus::{Pending, InProgress, Done};

pub struct PendingBuild {
    pub clone_url: CloneUrl,
    pub branch: String,
    build_id: i32
}

/// Type A is some key
pub trait Database : Send {
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
    extern crate openssl;

    use super::PendingBuild;

    use self::github::notification::PushNotification;
    use self::github::clone_url::CloneUrl;

    use self::time::{now, Timespec};
    use self::pg_typeprovider::util::Joinable;

    use self::openssl::ssl::{SslContext, SslMethod};

    use super::postgres::{Connection, GenericConnection, SslMode, ToSql};

    use builder::BuildResult;

    use super::EntryStatus::{Pending, InProgress, Done};
    use super::Database;

    // TODO: these must all be string literals, so we can't
    // simply define a constant.  Defining a macro is also not
    // enough, since pg_table! will not expand passed macros (yet),
    // so it sees only a macro invocation.
    pg_table!(builds, "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test")
    pg_table!(commits, "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test")
    pg_table!(users, "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test")
    pg_table!(assignments, "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test")
    pg_table!(submissions, "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test")

    pub struct PostgresDatabase {
        db: Connection
    }

    impl PostgresDatabase {
        fn new(loc: &str) -> Option<PostgresDatabase> {
            Connection::connect(
                loc,
                &SslMode::Require(
                    SslContext::new(SslMethod::Sslv23).unwrap())).ok().map(|db| {
                    PostgresDatabase {
                        db: db
                    }
                })
        }

        pub fn new_development() -> Option<PostgresDatabase> {
            PostgresDatabase::new(
                "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-dev")
        }

        pub fn new_testing() -> Option<PostgresDatabase> {
            let retval = PostgresDatabase::new(
                "postgres://jroesch:password@railsdb.cblx9rk1d5gu.us-east-1.rds.amazonaws.com/gradr-test");
            match retval {
                Some(ref db) => {
                    db.with_connection(|conn| {
                        let delete_table = |s: &str| {
                            conn.execute(
                                format!("DELETE FROM {}", s).as_slice(),
                                &[]).unwrap();
                        };
                        delete_table("builds");
                        delete_table("commits");
                        delete_table("submissions");
                    })
                },
                None => ()
            };
            retval
        }

        pub fn with_connection<A>(&self, f: |&Connection| -> A) -> A {
            f(&self.db)
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
                branch: commit.branch_name,
                build_id: self.id
            }
        }
    }

    fn get_user_by_github_username(conn: &GenericConnection,
                                   name: &str) -> Option<User> {
        UserSearch::new()
            .where_github_username(name.to_string())
            .search(conn, Some(1))
            .pop()
    }

    fn get_assignment_by_git_project_name(conn: &GenericConnection,
                                          project_name: &str) -> Option<Assignment> {
        AssignmentSearch::new()
            .where_git_project_name(project_name.to_string())
            .search(conn, Some(1))
            .pop()
    }

    fn insert_submission(conn: &GenericConnection,
                         user: &User,
                         assignment: &Assignment) -> Submission {
        let current_time = now().to_timespec();
        SubmissionInsert {
            user_id: user.id,
            assignment_id: assignment.id,
            created_at: current_time.clone(),
            updated_at: current_time.clone()
        }.insert(conn);
        SubmissionSearch::new()
            .where_created_at(current_time)
            .search(conn, Some(1))
            .pop()
            .unwrap()
    }

    fn insert_commit(conn: &GenericConnection,
                     user: &User,
                     assignment: &Assignment,
                     submission: &Submission,
                     pn: &PushNotification) -> Commit {
        let current_time = now().to_timespec();
        CommitInsert {
            assignment_id: assignment.id,
            user_id: user.id,
            created_at: current_time.clone(),
            updated_at: current_time.clone(),
            submission_id: submission.id,
            branch_name: pn.branch.clone(),
            clone_url: pn.clone_url.url.to_string()
        }.insert(conn);
        CommitSearch::new()
            .where_created_at(current_time)
            .search(conn, Some(1))
            .pop()
            .unwrap()
    }

    fn insert_build(conn: &GenericConnection,
                    user: &User,
                    assignment: &Assignment,
                    commit: &Commit) {
        let current_time = now().to_timespec();
        BuildInsert {
            commit_id: commit.id,
            user_id: user.id,
            assignment_id: assignment.id,
            course_id: assignment.course_id,
            created_at: current_time.clone(),
            updated_at: current_time,
            status: (&Pending).to_int(),
            results: "".to_string()
        }.insert(conn);
    }
                    
    impl Database for PostgresDatabase {
        fn add_pending(&self, entry: PushNotification) {
            self.with_connection(|conn| {
                let trans = conn.transaction().unwrap();
                let op_user = get_user_by_github_username(
                    &trans, entry.clone_url.username());
                let op_assignment = get_assignment_by_git_project_name(
                    &trans, entry.clone_url.project_name());
                match (op_user, op_assignment) {
                    (Some(user), Some(assignment)) => {
                        let submission = insert_submission(&trans,
                                                           &user,
                                                           &assignment);
                        let commit = insert_commit(&trans,
                                                   &user,
                                                   &assignment,
                                                   &submission,
                                                   &entry);
                        insert_build(&trans, &user, &assignment, &commit);
                        trans.commit().unwrap();
                    },
                    _ => {
                        trans.set_rollback();
                        trans.finish().unwrap();
                    }
                }
            });
        }

        fn get_pending(&self) -> Option<PendingBuild> {
            self.with_connection(|conn| {
                loop {
                    match get_one_build(conn) {
                        Some(b) => {
                            if try_lock_build(conn, &b) {
                                return Some(b.to_pending_build(conn));
                            }
                        },
                        None => { return None; }
                    }
                }
                            
                // Code below uses a transaction to accomplish the same
                // thing.  In small tests there doesn't seem to be any
                // difference in timing, so I'm defaulting to the simpler,
                // lock-free approach above
                // loop {
                //     let start_time = current_time_millis();
                //     let trans = conn.transaction().unwrap();
                //     match get_one_build(&trans) {
                //         Some(b) => {
                //             let res = try_lock_build(&trans, &b);
                //             assert!(res);
                //             if trans.commit().is_ok() {
                //                 let end_time = current_time_millis();
                //                 println!("TIME TAKEN: {}ms", end_time - start_time);
                //                 return Some(b.to_pending_build(conn))
                //             }
                //         },
                //         None => { return None; }
                //     }
                // }
            })
        }

        fn add_test_results(&self, entry: &PendingBuild, results: BuildResult) {
            let num_updated =
                BuildUpdate::new()
                .status_to((&Done).to_int())
                .results_to(results.consume_to_json().to_string())
                .where_id(entry.build_id)
                // not using with_connection to avoid copying results
                .update(&self.db);
            assert_eq!(num_updated, 1);
        }
    }
}
