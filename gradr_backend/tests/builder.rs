extern crate gradr_backend;

use gradr_backend::builder::{EnvSetup, BuildSetup, Tester, run_command};

use std::io::process::{Command};
use std::path::posix::Path;

struct TestingRequest {
    dir: Path, // directory where the build is to be performed
    makefile_loc: Path // where the makefile is located
}

fn req(name: &str) -> TestingRequest {
    TestingRequest {
        dir: Path::new(format!("test/{}", name)),
        makefile_loc: Path::new("test/makefile")
    }
}

impl TestingRequest {
    fn make_with_arg<A : ToCStr>(&self, arg: A) -> Command {
        let mut c = Command::new("make");
        c.arg(arg).cwd(&self.dir);
        c
    }
}

impl EnvSetup for TestingRequest {
    fn env_timeout(&self) -> Option<u64> { None }

    fn env_command(&self) -> Command {
        let mut c = Command::new("cp");
        c.arg(self.makefile_loc.as_str().unwrap());
        c.arg(self.dir.as_str().unwrap());
        c
    }
}

impl BuildSetup for TestingRequest {
    fn build_timeout(&self) -> Option<u64> { None }

    fn build_command(&self) -> Command {
        self.make_with_arg("build")
    }
}

impl Tester for TestingRequest {
    fn test_timeout(&self) -> Option<u64> { None }

    fn test_command(&self) -> Command {
        self.make_with_arg("test")
    }
}


impl Drop for TestingRequest {
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        run_command(
            &*Command::new("rm").arg(
                format!("{}/makefile", self.dir.as_str().unwrap())),
            None,
            ());
    }
}

#[test]
fn expected_compilation_failure() {
    assert!(req("compile_error").setup_env().is_err());
}

