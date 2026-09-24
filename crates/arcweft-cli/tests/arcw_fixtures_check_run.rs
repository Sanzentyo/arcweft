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

fn has_successful_terminal_status(output: &std::process::Output) -> bool {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.split_once("final_status=").map(|(_, status)| status))
        .and_then(|status| status.split_whitespace().next())
        .is_some_and(|status| matches!(status, "done" | "return"))
}

fn run_fixture_from_temp(
    path: &Path,
    configure: impl FnOnce(&mut Command),
) -> std::process::Output {
    let temp_path = temp_fixture_copy(path);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_arcw"));
    configure(&mut cmd);
    let profile = path.with_extension("toml");
    if profile.is_file() {
        let manifest = temp_path
            .parent()
            .expect("fixture parent")
            .join("arcw.toml");
        fs::copy(profile, &manifest).expect("copy fixture launch profile");
        cmd.arg("--manifest-path")
            .arg(manifest)
            .arg("--profile")
            .arg("fixture");
    } else {
        cmd.arg(&temp_path);
    }
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
            cmd.arg("run").arg("--steps").arg("16");
            if !path.with_extension("toml").is_file() {
                cmd.arg("--entry").arg("entry.main");
            }
        });
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && has_successful_terminal_status(&output),
            "arcw run should complete successfully for {}\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            stdout,
            String::from_utf8_lossy(&output.stderr),
        );
    }
}

#[test]
fn capability_fs_spec_fixture_checks_with_selected_adapter() {
    let path = fixture_root().join("spec_should_pass/check/010_capability_fs_read.arcw");
    let output = run_fixture_from_temp(&path, |command| {
        command.arg("check");
    });
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn spec_should_pass_check_fixtures_pass_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/check")) {
        let output = if path.with_extension("toml").is_file() {
            run_fixture_from_temp(&path, |cmd| {
                cmd.arg("check");
            })
        } else {
            run_arcw(&["compile", "--emit", "check"], &path)
        };
        assert!(
            output.status.success(),
            "{} should check\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn spec_should_pass_run_fixtures_pass_after_refactor() {
    for path in arcw_files(&fixture_root().join("spec_should_pass/run")) {
        let output = run_fixture_from_temp(&path, |cmd| {
            cmd.arg("run")
                .arg("--mode")
                .arg("drain")
                .arg("--steps")
                .arg("16");
            if !path.with_extension("toml").is_file() {
                cmd.arg("--entry").arg("entry.main");
            }
        });
        let stdout = String::from_utf8_lossy(&output.stdout);
        let completed_flow = has_successful_terminal_status(&output);
        let successful_cli_exit = path
            .file_name()
            .is_some_and(|name| name == "001_cli_stdout_entry.arcw")
            && output.stdout == b"hello";
        assert!(
            output.status.success() && (completed_flow || successful_cli_exit),
            "{} should finish successfully\nstdout:\n{}\nstderr:\n{}",
            path.display(),
            stdout,
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn cli_stdout_spec_fixture_runs_with_selected_adapter() {
    let path = fixture_root().join("spec_should_pass/run/001_cli_stdout_entry.arcw");
    let output = run_fixture_from_temp(&path, |command| {
        command.args(["run", "--mode", "drain", "--steps", "16"]);
    });
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"hello");
}

#[test]
fn spec_should_fail_fixtures_fail() {
    for path in arcw_files(&fixture_root().join("spec_should_fail")) {
        let output = run_arcw(&["compile", "--emit", "check"], &path);
        assert!(!output.status.success(), "{} should fail", path.display());
    }
}
