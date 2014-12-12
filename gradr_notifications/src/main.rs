extern crate hyper;
extern crate libgradr;

use hyper::Ipv4Addr;

use libgradr::database::postgres_db::PostgresDatabase;
use libgradr::notification_listener::{GitHubServer, NotificationSource};

static PORT: u16 = 1337;

#[cfg(not(test))]
fn main() {
    let db = PostgresDatabase::new_development().unwrap();

    let server = GitHubServer::new(Ipv4Addr(0, 0, 0, 0), PORT);
    let running_server = server.event_loop().unwrap();

    loop {
        running_server.notification_event_loop_step(&db);
    }
}
