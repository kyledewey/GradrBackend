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
    fn get_pending<'a, A: 'a>(&mut self) -> Option<A>;

    fn add_test_results<'a>(&mut self, entry: &'a A, results: BuildResult);
}
