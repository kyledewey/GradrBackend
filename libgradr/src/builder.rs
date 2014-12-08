// Portion that builds and runs tests.  Assumes that this can be
// done via a call to `make build` and `make test`.  Compiler failure
// is assumed to be communicated by return value.  Tests are assumed
// to have the following output format:
//
// some test name: <PASS|FAIL>
//
// ...where each test result is on its own line.  If multiple tests
// have the same name, then only the last test is recorded.

use std::collections::HashMap;
use std::io::{BufferedReader, ByRefReader, RefReader, IoResult,
              IoError, OtherIoError};
use std::io::pipe::PipeStream;
use std::io::process::{Command, Process, ExitStatus, ExitSignal,
                       ProcessExit};

use util::MessagingUnwrapper;

use self::BuildResult::{SetupEnvFailure, BuildFailure, TestFailure, TestSuccess};
use self::TestResult::{Pass, Fail};

fn spawn_with_timeout(c: &Command, timeout: Option<u64>) -> IoResult<Process> {
    let mut p = try!(c.spawn());
    p.set_timeout(timeout);
    Ok(p)
}

/// Runs the given command with the given timeout, ignoring the output.
/// If it returns non-zero, then it's a failure, as with a signal.
/// Takes what it should return on success.
pub fn run_command<A>(c: &Command, timeout: Option<u64>, on_success: A) -> IoResult<A> {
    (try!(
        (try!(
            spawn_with_timeout(c, timeout)))
            .wait())).if_ok(on_success)
}

// Runs the given chain of commands.  Returns the first error.
pub fn run_commands<A>(commands: &Vec<Command>, timeout: Option<u64>, on_success: A) -> IoResult<A> {
    for cmd in commands.iter() {
        try!(run_command(cmd, timeout.clone(), ()));
    }
    Ok(on_success)
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

#[deriving(Show, PartialEq)]
pub enum TestResult {
    Pass,
    Fail
}

struct ProcessReader {
    p: Process
}

impl ProcessReader {
    fn new(cmd: &Command, timeout: Option<u64>) -> IoResult<ProcessReader> {
        spawn_with_timeout(cmd, timeout).map(|p| ProcessReader { p: p })
    }

    fn output_reader<'a>(&'a mut self) -> BufferedReader<RefReader<'a, PipeStream>> {
        // unwrap should be ok - the documentation says `Some` is the default,
        // and we are not messing with any of the defaults
        BufferedReader::new(
            self.p.stdout.as_mut().unwrap_msg(line!()).by_ref())
    }
}

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
        parse_test_result(results[1]).map(|res| {
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

#[deriving(Show)]
pub enum BuildResult {
    SetupEnvFailure(IoError),
    BuildFailure(IoError),
    TestFailure(IoError),
    TestSuccess(HashMap<String, TestResult>)
}

pub trait WholeBuildable {
    // BEGIN FUNCTIONS TO IMPLEMENT
    fn env_timeout(&self) -> Option<u64>;
    fn env_commands(&self) -> Vec<Command>;

    fn build_timeout(&self) -> Option<u64>;
    fn build_commands(&self) -> Vec<Command>;

    fn test_timeout(&self) -> Option<u64>;
    fn test_command(&self) -> Command;
    // END FUNCTIONS TO IMPLEMENT

    /// Gets everything in order for testing to be performed.
    /// After calling this, it is assumed that we are ready
    /// to call make
    fn setup_env(&self) -> IoResult<()> {
        run_commands(&self.env_commands(), self.env_timeout(), ())
    }
    
    fn do_build(&self) -> IoResult<()> { 
        run_commands(&self.build_commands(), self.build_timeout(), ())
    }

    fn do_testing(&self) -> IoResult<HashMap<String, TestResult>> {
        let mut reader = 
            try!(ProcessReader::new(&self.test_command(), self.test_timeout()));
        let mut map = HashMap::new();
        for op_line in reader.output_reader().lines() {
            let (k, v) = try!(
                parse_line(
                    try!(op_line).as_slice().trim()));
            map.insert(k, v);
        }

        Ok(map)
    }

    fn whole_build(&self) -> BuildResult {
        // Because we have different results for different kinds
        // of failures, we cannot use `try!`
        match self.setup_env() {
            Ok(_) => {
                match self.do_build() {
                    Ok(_) => {
                        match self.do_testing() {
                            Ok(res) => TestSuccess(res),
                            Err(e) => TestFailure(e)
                        }
                    },
                    Err(e) => BuildFailure(e)
                }
            },
            Err(e) => SetupEnvFailure(e)
        }
    }
}

pub trait ToWholeBuildable<A : WholeBuildable> {
    fn to_whole_buildable(&self) -> A;
}

pub mod github {
    extern crate url;
    extern crate github;

    use std::io::Command;

    use self::github::notification::PushNotification;
    use self::url::Url;

    use super::{WholeBuildable, ToWholeBuildable, run_command};
    use super::testing::TestingRequest;

    use database::postgres_db::BuildInsert;
    use util::MessagingUnwrapper;

    pub struct GitHubRequest {
        build_root: Path,
        branch: String,
        clone_url: Url,
        testing_req: TestingRequest,
    }

    impl GitHubRequest {
        pub fn new(pn: &PushNotification, build_root: Path, makefile_loc: Path) -> GitHubRequest {
            let mut dir = build_root.clone();

            // TODO: this is very hacky and likely doesn't work in general
            let trim: &[_] = &['.', 'g', 'i', 't'];
            let clone_url_string = pn.clone_url.serialize();
            let splits: Vec<&str> = clone_url_string.split('/').collect();
            dir.push(splits.last().unwrap_msg(line!()).trim_right_chars(trim));
            
            GitHubRequest {
                build_root: build_root,
                branch: pn.branch.clone(),
                clone_url: pn.clone_url.clone(),
                testing_req: TestingRequest::new(dir, makefile_loc)
            }
        }
    }
    
    impl WholeBuildable for GitHubRequest {
        fn env_timeout(&self) -> Option<u64> { None }
        fn build_timeout(&self) -> Option<u64> { None }
        fn test_timeout(&self) -> Option<u64> { None }
        
        fn env_commands(&self) -> Vec<Command> {
            let mut clone = Command::new("git");
            clone.arg("clone").arg("-b").arg(self.branch.as_slice());
            clone.arg(self.clone_url.serialize());
            clone.cwd(&self.build_root);

            let mut retval = vec!(clone);
            retval.push_all(self.testing_req.env_commands().as_slice());
            retval
        }

        fn build_commands(&self) -> Vec<Command> {
            self.testing_req.build_commands()
        }

        fn test_command(&self) -> Command {
            self.testing_req.test_command()
        }
    }

    impl ToWholeBuildable<GitHubRequest> for PushNotification {
        fn to_whole_buildable(&self) -> GitHubRequest {
            GitHubRequest::new(
                self,
                Path::new("build_test"),
                Path::new("test/makefile"))
        }
    }

    impl ToWholeBuildable<GitHubRequest> for BuildInsert {
        fn to_whole_buildable(&self) -> GitHubRequest {
            PushNotification {
                clone_url: Url::parse(self.clone_url.as_slice()).unwrap(),
                branch: self.branch.clone()
            }.to_whole_buildable()
        }
    }
    
    impl Drop for GitHubRequest {
        #[allow(unused_must_use)]
        fn drop(&mut self) {
            let mut c = Command::new("rm");
            c.arg("-rf").arg(self.testing_req.dir.as_str().unwrap_msg(line!()));
            run_command(&c, None, ());
        }
    }
}

#[cfg(test)]
mod process_tests {
    use std::io::process::Command;
    use super::{run_command, ProcessReader};

    use util::MessagingUnwrapper;

    #[test]
    fn echo_ok() {
        assert!(run_command(&*Command::new("echo").arg("foobar"), None, ()).is_ok());
    }
    
    #[test]
    fn false_ok() {
        assert!(run_command(&Command::new("false"), None, ()).is_err());
    }

    fn output_from_command(cmd: &Command) -> Vec<String> {
        let pr = ProcessReader::new(cmd, None);
        assert!(pr.is_ok());
        pr.unwrap_msg(line!()).output_reader().lines()
            .map(|line| {
                assert!(line.is_ok());
                line.unwrap_msg(line!()).as_slice().trim().to_string()
            }).collect()
    }
            
    #[test]
    fn output_single_line_read() {
        let lines = output_from_command(
            &*Command::new("sh").arg("-c").arg("echo foobar"));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].as_slice(), "foobar");
    }

    #[test]
    fn output_multi_line_read() {
        let lines = output_from_command(
            &*Command::new("sh").arg("-c").arg("echo foo; echo bar"));
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].as_slice(), "foo");
        assert_eq!(lines[1].as_slice(), "bar");
    }
}

#[cfg(test)]
mod parse_tests {
    use super::TestResult::{Pass, Fail};
    use super::{parse_test_result, parse_line};

    use util::MessagingUnwrapper;

    #[test]
    fn parse_test_pass() {
        let res = parse_test_result("PASS");
        assert!(res.is_ok());
        assert_eq!(res.unwrap_msg(line!()), Pass);
    }

    #[test]
    fn parse_test_fail() {
        let res = parse_test_result("FAIL");
        assert!(res.is_ok());
        assert_eq!(res.unwrap_msg(line!()), Fail);
    }

    #[test]
    fn parse_test_bad_test() {
        let res = parse_test_result("foobar");
        assert!(res.is_err());
    }

    #[test]
    fn parse_valid_test_line() {
        let res = parse_line("my test:PASS");
        assert!(res.is_ok());
        let (key, result) = res.unwrap_msg(line!());
        assert_eq!(key.as_slice(), "my test");
        assert_eq!(result, Pass);
    }

    #[test]
    fn parse_invalid_test_line() {
        assert!(parse_line("this:is:PASS").is_err());
    }
}

pub mod testing {
    use std::io::process::Command;

    use super::{run_command, WholeBuildable};

    use util::MessagingUnwrapper;

    pub struct TestingRequest {
        pub dir: Path, // directory where the build is to be performed
        pub makefile_loc: Path // where the makefile is located
    }

    impl TestingRequest {
        pub fn new(dir: Path, makefile_loc: Path) -> TestingRequest {
            TestingRequest {
                dir: dir,
                makefile_loc: makefile_loc
            }
        }

        fn make_with_arg<A : ToCStr>(&self, arg: A) -> Command {
            let mut c = Command::new("make");
            c.arg("-s").arg(arg).cwd(&self.dir);
            c
        }
    }

    impl Drop for TestingRequest {
        /// Automatically deletes the copied-over makefile on test end,
        /// along with any applicable executables (namely `a.out`)
        #[allow(unused_must_use)]
        fn drop(&mut self) {
            let dir = self.dir.as_str().unwrap_msg(line!());
            run_command(
                &*Command::new("rm")
                    .arg(format!("{}/makefile", dir))
                    .arg(format!("{}/a.out", dir)),
                None,
                ());
        }
    }

    impl WholeBuildable for TestingRequest {
        fn env_timeout(&self) -> Option<u64> { None }

        fn env_commands(&self) -> Vec<Command> {
            let mut c = Command::new("cp");
            c.arg(self.makefile_loc.as_str().unwrap_msg(line!()));
            c.arg(self.dir.as_str().unwrap_msg(line!()));
            vec!(c)
        }

        fn build_timeout(&self) -> Option<u64> { None }

        fn build_commands(&self) -> Vec<Command> {
            vec!(self.make_with_arg("build"))
        }

        fn test_timeout(&self) -> Option<u64> { None }

        fn test_command(&self) -> Command {
            self.make_with_arg("test")
        }
    }
}

#[cfg(test)]
mod build_tests {
    use super::BuildResult::TestSuccess;
    use super::TestResult::{Pass, Fail};

    use super::WholeBuildable;
    use super::testing::TestingRequest;

    use util::MessagingUnwrapper;

    fn req(name: &str) -> TestingRequest {
        TestingRequest::new(
            Path::new(format!("test/{}", name).as_slice()),
            Path::new("test/makefile"))
    }

    #[test]
    fn makefile_copy_ok() {
        assert!(req("compile_error").setup_env().is_ok());
    }

    #[test]
    fn expected_compile_failure() {
        let r = req("compile_error");
        assert!(r.setup_env().is_ok());
        assert!(r.do_build().is_err());
    }

    #[test]
    fn expected_compile_success() {
        let r = req("compile_success");
        assert!(r.setup_env().is_ok());
        assert!(r.do_build().is_ok());
    }

    #[test]
    fn testing_parsing_empty_success() {
        let r = req("testing_parsing_empty_success");
        assert!(r.setup_env().is_ok());
        assert!(r.do_build().is_ok());
        let res = r.do_testing();
        assert!(res.is_ok());
        assert_eq!(res.unwrap_msg(line!()).len(), 0);
    }

    #[test]
    fn testing_parsing_nonempty_success() {
        let r = req("testing_parsing_nonempty_success");
        assert!(r.setup_env().is_ok());
        assert!(r.do_build().is_ok());
        let res = r.do_testing();

        assert!(res.is_ok());
        let u = res.unwrap_msg(line!());
        assert_eq!(u.len(), 2);

        let t1 = u.find_equiv(&"test1".to_string());
        assert!(t1.is_some());
        assert_eq!(t1.unwrap_msg(line!()), &Pass);

        let t2 = u.find_equiv(&"test2".to_string());
        assert!(t2.is_some());
        assert_eq!(t2.unwrap_msg(line!()), &Fail);
    }

    #[test]
    fn test_whole_build() {
        match req("test_whole_build").whole_build() {
            TestSuccess(u) => {
                let t1 = u.find_equiv(&"test1".to_string());
                assert!(t1.is_some());
                assert_eq!(t1.unwrap_msg(line!()), &Pass);
                
                let t2 = u.find_equiv(&"test2".to_string());
                assert!(t2.is_some());
                assert_eq!(t2.unwrap_msg(line!()), &Fail);
            },
            _ => { assert!(false); }
        };
    }
}
