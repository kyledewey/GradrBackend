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
