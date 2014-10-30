// Portion that builds and runs tests.  Assumes that this can be
// done via a call to `make build` and `make test`.  Compiler failure
// is assumed to be communicated by return value.  Tests are assumed
// to have the following output format:
//
// some test name: <PASS|FAIL>
//
// ...where each test result is on its own line.  If multiple tests
// have the same name, then only the last test is recorded.

use std::io::{BufferedReader, RefReader, IoResult, IoError, OtherIoError};
use std::io::pipe::PipeStream;
use std::io::process::{Command, Process, ExitStatus, ExitSignal,
                       ProcessExit};
use std::collections::HashMap;

struct BuilderState<A>(A);

struct InitialState;
struct EnvDone;
struct BuildDone;
struct TestDone;

fn spawn_with_timeout(c: &Command, timeout: Option<u64>) -> IoResult<Process> {
    match c.spawn() {
        Ok(mut p) => {
            p.set_timeout(timeout);
            Ok(p)
        },
        Err(e) => Err(e)
    }
}

/// Runs the given command with the given timeout, ignoring the output.
/// If it returns non-zero, then it's a failure, as with a signal.
/// Takes what it should return on success.
fn run_command<A>(c: &Command, timeout: Option<u64>, on_success: A) -> IoResult<A> {
    // would like to use `and_then` here, but we get problems with capturing
    // on_success, seemingly because the compiler cannot enforce that
    // the closure passed to `and_then` only calls it once
    match spawn_with_timeout(c, timeout) {
        Ok(mut p) => {
            match p.wait() {
                Ok(mut res) => res.if_ok(on_success),
                Err(e) => Err(e)
            }
        },
        Err(e) => Err(e)
    }
}

trait ErrorSimplifier {
    fn if_ok<A>(&self, ret_this: A) -> IoResult<A>;
}

impl ErrorSimplifier for ProcessExit {
    fn if_ok<A>(&self, ret_this: A) -> IoResult<A> {
        match *self {
            ExitStatus(0) => Ok(ret_this),
            ExitStatus(x) => Err(
                IoError {
                    kind: OtherIoError,
                    desc: "Non-zero exit code",
                    detail: Some(x.to_string())
                }),
            ExitSignal(x) => Err(
                IoError {
                    kind: OtherIoError,
                    desc: "Exit signal",
                    detail: Some(x.to_string())
                })
        }
    }
}

trait EnvSetup {
    fn timeout(&self) -> Option<u64>;

    fn command(&self) -> Command;

    /// Gets everything in order for testing to be performed.
    /// After calling this, it is assumed that we are ready
    /// to call make
    fn setup_env(&self) -> IoResult<BuilderState<EnvDone>> {
        run_command(&self.command(), self.timeout(),
                    BuilderState(EnvDone))
    }
}

trait BuildSetup {
    fn timeout(&self) -> Option<u64>;

    fn command(&self) -> Command;
    
    fn do_build(&self) -> IoResult<BuilderState<BuildDone>> {
        run_command(&self.command(), self.timeout(),
                    BuilderState(BuildDone))
    }
}

enum TestResult {
    Pass,
    Fail
}

struct ProcessReader {
    p: Process
}

impl ProcessReader {
    fn new(cmd: &Command) -> IoResult<ProcessReader> {
        cmd.spawn().map(|p| ProcessReader { p: p })
    }

    fn output_reader<'a>(&'a mut self) -> BufferedReader<RefReader<'a, PipeStream>> {
        // unwrap should be ok - the documentation says `Some` is the default,
        // and we are not messing with any of the defaults
        BufferedReader::new(
            Reader::by_ref(
                self.p.stdout.as_mut().unwrap()))
    }
}

trait Tester {
    fn parse_test_result(line: &str) -> IoResult<TestResult> {
        match line {
            "PASS" => Ok(Pass),
            "FAIL" => Ok(Fail),
            _ => Err(
                IoError {
                    kind: OtherIoError,
                    desc: "Malformed test result",
                    detail: Some(line.to_string())
                })
        }
    }

    fn parse_line(line: &str) -> IoResult<(String, TestResult)> {
        let results: Vec<&str> = line.split_str(":").collect();
        if results.len() == 2 {
            Tester::parse_test_result(results[1]).map(|res| {
                (results[0].to_string(), res)
            })
        } else {
            Err(
                IoError {
                    kind: OtherIoError,
                    desc: "Malformed test string",
                    detail: Some(line.to_string())
                })
        }
    }

    fn test_command(&self) -> Command;

    fn do_testing(&self) -> IoResult<HashMap<String, TestResult>> {
        // TODO: without do syntax this becomes nightmarish in
        // functional style, and I had issues with closures capturing
        // too much
        match ProcessReader::new(&self.test_command()) {
            Ok(mut reader) => {
                let mut map = HashMap::new();
                for op_line in reader.output_reader().lines() {
                    match op_line {
                        Ok(line) => {
                            match Tester::parse_line(line.as_slice()) {
                                Ok((k, v)) => {
                                    map.insert(k, v);
                                }
                                Err(s) => { return Err(s); }
                            }
                        }
                        Err(s) => { return Err(s); }
                    }
                }
                Ok(map)
                
            }
            Err(s) => { return Err(s); }
        }
    }
}

struct TestingRequest {
    dir: String, // directory where the build is to be performed
    makefile_loc: String // where the makefile is located
}
