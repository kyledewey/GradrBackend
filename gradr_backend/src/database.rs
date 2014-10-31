// Abstraction of the database.
// For the purposes of the backend, the only necessary
// actions are the following:
//
// 1. Put a pending build into the database.
// 2. Grab a pending build from the database, which
//    will immediately undergo being built.
// 3. Put test results into a database, given what
//    build was pending

/// Type A is the entry, type B is for test results
pub trait Database<A, B> {
    fn add_pending(&mut self, entry: A);

    fn get_pending(&mut self) -> A;

    fn add_test_results(&mut self, entry: A, results: B);
}
