use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/arcw")
}

fn arcw_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "arcw"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn run_arcw(args: &[&str], path: &Path) -> std::process::Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arcw"));
    for arg in args {
        cmd.arg(arg);
    }
    cmd.arg(path);
    cmd.output().expect("arcw command runs")
}

#[test]
fn current_check_fixtures_pass() {
    for path in arcw_files(&fixture_root().join("current_pass/check")) {
        let output = run_arcw(&["check"], &path);
        assert!(
            output.status.success(),
            "arcw check should pass for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn current_run_fixtures_pass() {
    for path in arcw_files(&fixture_root().join("current_pass/run")) {
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("run")
            .arg(&path)
            .arg("--steps")
            .arg("16")
            .output()
            .expect("arcw run runs");
        assert!(
            output.status.success(),
            "arcw run should pass for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn spec_should_pass_check_fixtures_pass_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/check")) {
        let output = run_arcw(&["check"], &path);
        assert!(output.status.success(), "{} should check", path.display());
    }
}

#[test]
fn spec_should_pass_run_fixtures_pass_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/run")) {
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("run")
            .arg(&path)
            .arg("--mode")
            .arg("drain")
            .arg("--steps")
            .arg("16")
            .output()
            .expect("arcw run runs");
        assert!(output.status.success(), "{} should run", path.display());
    }
}

#[test]
fn spec_should_fail_fixtures_fail() {
    for path in arcw_files(&fixture_root().join("spec_should_fail")) {
        let output = run_arcw(&["check"], &path);
        assert!(!output.status.success(), "{} should fail", path.display());
    }
}
