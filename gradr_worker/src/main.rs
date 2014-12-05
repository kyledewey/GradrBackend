extern crate libgradr;

use libgradr::database::postgres_db::PostgresDatabase;
use libgradr::worker::worker_loop_step;

#[cfg(not(test))]
fn main() {
    let db = PostgresDatabase::new(
        "postgres://jroesch@localhost/gradr-production").unwrap();
    
    loop {
        worker_loop_step(&db);
    }
}
