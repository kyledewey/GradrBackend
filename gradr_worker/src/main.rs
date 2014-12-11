extern crate libgradr;

use libgradr::database::postgres_db::PostgresDatabase;
use libgradr::worker::worker_loop_step;

#[cfg(not(test))]
fn main() {
    let db = PostgresDatabase::new_development().unwrap();
    
    loop {
        worker_loop_step(&db);
    }
}
