extern crate hyper;
extern crate gradr_backend;

use std::sync::Arc;

use hyper::Ipv4Addr;

use gradr_backend::database::postgres_db::PostgresDatabase;
use gradr_backend::notification_listener::{GitHubServer, NotificationSource};
use gradr_backend::worker::worker_loop_step;

#[cfg(not(test))]
fn main() {
    let server = GitHubServer::new(Ipv4Addr(0, 0, 0, 0), 1337);
    let running_server = server.event_loop().unwrap();

    let db1 = 
        Arc::new(
            PostgresDatabase::new(
                "postgres://jroesch@localhost/gradr-production").unwrap());
    let db2 = db1.clone();
    
    // thread for the notification listener
    spawn(proc() {
        loop {
            running_server.notification_event_loop_step(&*db1);
        }
    });

    // thread for the builder
    spawn(proc() {
        loop {
            worker_loop_step(&*db2);
        }
    });

    println!("THREADS STARTED");
}
