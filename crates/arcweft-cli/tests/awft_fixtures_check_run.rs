use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/awft")
}

fn awft_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
        .map(|entry| entry.expect("fixture dir entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "awft"))
        .collect::<Vec<_>>();
    files.sort();
    files
}

fn file_name(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .expect("fixture file name is utf-8")
}

fn is_current_check_gap(path: &Path) -> bool {
    matches!(
        file_name(path),
        // These fixtures intentionally preserve direction-package syntax that
        // is not implemented yet. Keep the sources intact and promote them by
        // removing entries here as the parser/runtime catches up.
        "008_let_else_diverge.awft"
            | "011_dialogue_with_plan.awft"
            | "013_task_fn_await_shape.awft"
            | "014_struct_enum_type_alias.awft"
            | "015_state_defaults.awft"
            | "016_source_decl.awft"
            | "017_stream_fn_shape.awft"
    )
}

fn is_current_run_gap(path: &Path) -> bool {
    matches!(
        file_name(path),
        "004_signal_metric_event.awft" | "010_line_task_effects.awft"
    )
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
    for path in awft_files(&fixture_root().join("current_pass/check"))
        .into_iter()
        .filter(|path| !is_current_check_gap(path))
    {
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
    for path in awft_files(&fixture_root().join("current_pass/run"))
        .into_iter()
        .filter(|path| !is_current_run_gap(path))
    {
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("run")
            .arg(&path)
            .arg("--frames")
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
#[ignore = "enable after entry/capability/runtime-step refactor lands"]
fn spec_should_pass_check_fixtures_pass_after_refactor() {
    for path in awft_files(&fixture_root().join("spec_should_pass/check")) {
        let output = run_arcw(&["check"], &path);
        assert!(output.status.success(), "{} should check", path.display());
    }
}

#[test]
#[ignore = "enable after entry/capability/runtime-step refactor lands"]
fn spec_should_pass_run_fixtures_pass_after_refactor() {
    for path in awft_files(&fixture_root().join("spec_should_pass/run")) {
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
#[ignore = "enable after capability policy rejects invalid forms"]
fn spec_should_fail_fixtures_fail() {
    for path in awft_files(&fixture_root().join("spec_should_fail")) {
        let output = run_arcw(&["check"], &path);
        assert!(!output.status.success(), "{} should fail", path.display());
    }
}
