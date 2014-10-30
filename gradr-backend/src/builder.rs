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
use std::io::process::{Command, Process, ExitStatus, ExitSignal};
use std::collections::HashMap;

/// Peforms builds and testing via abstract requests.
trait Builder<A : Tester> {
    /// Gets everything in order for testing to be performed.
    /// After calling this, it is assumed that we are ready
    /// to call make
    fn setup_env(&self);

    /// Makes a tester.  This should be the only way
    /// to make a tester.  Avoids situations where we
    /// tried to call `make test` without a successful
    /// `make build`
    fn make_tester(&self) -> A;

    fn build_timeout(&self) -> u64;

    fn test_timeout(&self) -> u64;

    /// runs make build
    fn do_build(&self) -> IoResult<A> {
        Command::new("make").arg("build")
            .spawn().and_then(|mut process| {
                process.set_timeout(Some(self.build_timeout()));
                process.wait().and_then(|res| match res {
                    ExitStatus(0) => Ok(self.make_tester()),
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
                })
            })
    }
} // trait Builder

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

    fn do_testing(&self) -> IoResult<HashMap<String, TestResult>> {
        match ProcessReader::new(&*Command::new("make").arg("test")) {
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

/// Encapsulates a request to perform a build and begin test execution.
/// In the future, this may include information for spinning up Docker
/// and pulling from GitHub, though for the moment this is just a directory
/// to go to.
struct BuildRequest {
    dir: String, // directory where the build is to be performed
    makefile_loc: String // where the makefile is located
}

