use std::io::{Command, BufferedReader};

fn command_output<A : ToCStr>(a: A) -> Vec<String> {
    match Command::new(a).spawn() {
        Ok(ref mut p) => {
            BufferedReader::new(
                Reader::by_ref(
                    p.stdout.as_mut().unwrap()))
                .lines()
                .map(|x| x.unwrap())
                .collect()
        }
        Err(e) => fail!("Failed to execute process: {}", e)
    }
}

fn main() {
    for s in command_output("ls").iter() {
        print!("{}", s);
    }
}

