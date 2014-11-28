extern crate hyper;
extern crate gradr_backend;

use hyper::Ipv4Addr;
use gradr_backend::notification_listener::GitHubServer;

#[cfg(not(test))]
fn main() {
    let server = GitHubServer::new(Ipv4Addr(0, 0, 0, 0), 1337);
    let running_server = server.event_loop().unwrap();
    loop {
        println!("{}", running_server.get_notification());
    }
}
