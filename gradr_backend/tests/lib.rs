extern crate gradr_backend;

use gradr_backend::builder::{EnvSetup, BuildSetup, Tester, run_command,
                             Pass, Fail};

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
        c.arg("-s").arg(arg).cwd(&self.dir);
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
    /// Automatically deletes the copied-over makefile on test end,
    /// along with any applicable executables (namely `a.out`)
    #[allow(unused_must_use)]
    fn drop(&mut self) {
        let dir = self.dir.as_str().unwrap();
        run_command(
            &*Command::new("rm")
                .arg(format!("{}/makefile", dir))
                .arg(format!("{}/a.out", dir)),
            None,
            ());
    }
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
    println!("{}", res);
    assert!(res.is_ok());
    assert_eq!(res.unwrap().len(), 0);
}

#[test]
fn testing_parsing_nonempty_success() {
    let r = req("testing_parsing_nonempty_success");
    assert!(r.setup_env().is_ok());
    assert!(r.do_build().is_ok());
    let res = r.do_testing();
    println!("{}", res);
    assert!(res.is_ok());
    let u = res.unwrap();
    assert_eq!(u.len(), 2);

    let t1 = u.find_equiv(&"test1".to_string());
    assert!(t1.is_some());
    assert_eq!(t1.unwrap(), &Pass);

    let t2 = u.find_equiv(&"test2".to_string());
    assert!(t2.is_some());
    assert_eq!(t2.unwrap(), &Fail);
}
