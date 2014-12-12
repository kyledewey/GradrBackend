extern crate libgradr;
extern crate postgres;

use libgradr::database::EntryStatus::Done;
use libgradr::database::postgres_db::PostgresDatabase;
use postgres::{Result, Error, Connection};

use std::os;
use std::io::timer;
use std::time::Duration;

static QUERY: &'static str = "SELECT COUNT(*) FROM builds WHERE status != $1";

fn queue_depth(conn: &Connection) -> Result<u64> {
    // Considering all builds which are not done to be queued up, even
    // if they are actively being processed at the moment
    let stmt = try!(conn.prepare(QUERY));
    for row in try!(stmt.query(&[&(&Done).to_int()])) {
        let res: i64 = row.get(0);
        return Ok(res.to_u64().unwrap());
    }

    Err(Error::BadData)
}

fn every_k_seconds(k: uint, do_this: |u64| -> ()) {
    let casted = k as u64;
    let dur = Duration::seconds(k as i64);
    let mut time_point: u64 = 0;

    loop {
        do_this(time_point);
        timer::sleep(dur);
        time_point += casted;
    }
}

#[cfg(not(test))]
fn main() {
    let args = os::args();

    if args.len() != 2 {
        println!("Needs a amount k to wait by");
    } else {
        match from_str::<uint>(args[1].as_slice()) {
            Some(wait_by) => {
                let db = PostgresDatabase::new_development().unwrap();
                every_k_seconds(
                    wait_by,
                    |time_point| {
                        db.with_connection(|conn| {
                            println!("{}: {}",
                                     time_point,
                                     queue_depth(conn).unwrap());
                        });
                    });
            },
            None => {
                println!("k must be an unsigned integer");
            }
        }
    }
}

                    

    