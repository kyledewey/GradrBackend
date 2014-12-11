extern crate time;

pub trait MessagingUnwrapper<A> {
    fn unwrap_msg(self, orig_line: uint) -> A;
}

impl<A, B : ToString> MessagingUnwrapper<A> for Result<A, B> {
    fn unwrap_msg(self, orig_line: uint) -> A {
        match self {
            Ok(a) => a,
            Err(ref b) => {
                let s = b.to_string();
                panic!(
                    format!(
                        "PANIC FROM {}: {}", orig_line, s.as_slice()));
            }
        }
    }
}

impl<A> MessagingUnwrapper<A> for Option<A> {
    fn unwrap_msg(self, orig_line: uint) -> A {
        match self {
            Some(a) => a,
            None => {
                panic!(
                    format!(
                        "PANIC FROM {} ON OPTION", orig_line))
            }
        }
    }
}

pub fn current_time_millis() -> i64 {
    let timespec = time::get_time();
    timespec.sec + timespec.nsec as i64 / 1000 / 1000
}

macro_rules! do_timing(
    ($what: stmt) => (
        let start_time = current_time_millis();
        $what;
        let end_time = current_time_millis();
        println!("TIME TAKEN: {}ms", end_time - start_time);
        ))
