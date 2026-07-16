use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_RUN_FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

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

fn run_fixture_from_temp(
    path: &Path,
    configure: impl FnOnce(&mut Command),
) -> std::process::Output {
    let temp_path = temp_fixture_copy(path);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arcw"));
    configure(&mut cmd);
    cmd.arg(&temp_path);
    let output = cmd.output().expect("arcw run runs");
    if let Some(parent) = temp_path.parent() {
        let _ = fs::remove_dir_all(parent);
    }
    output
}

fn temp_fixture_copy(path: &Path) -> PathBuf {
    let index = TEMP_RUN_FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("fixture");
    let dir = std::env::temp_dir().join(format!(
        "arcweft-fixture-run-{}-{index}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).expect("temp fixture dir is created");
    let temp_path = dir.join(format!("{stem}.arcw"));
    fs::copy(path, &temp_path).unwrap_or_else(|error| {
        panic!(
            "failed to copy fixture {} to {}: {error}",
            path.display(),
            temp_path.display()
        )
    });
    temp_path
}

#[test]
fn current_check_fixtures_pass() {
    for path in arcw_files(&fixture_root().join("current_pass/check")) {
        let output = run_arcw(&["compile", "--emit", "check"], &path);
        assert!(
            output.status.success(),
            "arcw compile --emit check should pass for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn current_run_fixtures_pass() {
    for path in arcw_files(&fixture_root().join("current_pass/run")) {
        let output = run_fixture_from_temp(&path, |cmd| {
            cmd.arg("run")
                .arg("--entry")
                .arg("entry.main")
                .arg("--steps")
                .arg("16");
        });
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
        let output = run_arcw(&["compile", "--emit", "check"], &path);
        assert!(output.status.success(), "{} should check", path.display());
    }
}

#[test]
fn spec_should_pass_run_fixtures_pass_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/run")) {
        let output = run_fixture_from_temp(&path, |cmd| {
            cmd.arg("run")
                .arg("--entry")
                .arg("entry.main")
                .arg("--mode")
                .arg("drain")
                .arg("--steps")
                .arg("16");
        });
        assert!(output.status.success(), "{} should run", path.display());
    }
}

#[test]
fn spec_should_fail_fixtures_fail() {
    for path in arcw_files(&fixture_root().join("spec_should_fail")) {
        let output = run_arcw(&["compile", "--emit", "check"], &path);
        assert!(!output.status.success(), "{} should fail", path.display());
    }
}
