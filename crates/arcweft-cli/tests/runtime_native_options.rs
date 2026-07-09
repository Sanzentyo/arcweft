use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[test]
fn text_input_trace_rejects_headless_runner() {
    let fixture = fixture();
    assert_run_usage_error(
        &[
            "--runner",
            "headless",
            fixture.to_str().expect("fixture path is UTF-8"),
            "--text-input-trace-out",
            "target/unused-text-input-trace.json",
        ],
        "--text-input-trace-out requires --runner native",
    );
}

#[test]
fn session_io_rejects_headless_runner() {
    let fixture = fixture();
    for (option, path) in session_options() {
        assert_run_usage_error(
            &[
                "--runner",
                "headless",
                fixture.to_str().expect("fixture path is UTF-8"),
                option,
                path,
            ],
            "--session-load and --session-save-out require --runner native",
        );
    }
}

#[test]
fn session_io_rejects_watch_mode() {
    let fixture = fixture();
    for (option, path) in session_options() {
        assert_run_usage_error(
            &[
                "--runner",
                "native",
                fixture.to_str().expect("fixture path is UTF-8"),
                "--watch",
                option,
                path,
            ],
            "--session-load and --session-save-out cannot be combined with --watch",
        );
    }
}

fn session_options() -> [(&'static str, &'static str); 2] {
    [
        ("--session-load", "target/missing-session-load.json"),
        ("--session-save-out", "target/unused-session-save.json"),
    ]
}

fn assert_run_usage_error(arguments: &[&str], expected: &str) {
    let output = run(arguments);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains(expected),
        "stderr did not contain {expected:?}: {stderr}"
    );
}

fn run(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .args(arguments)
        .output()
        .expect("arcw run starts")
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/arcw/spec_should_pass/run/001_cli_stdout_entry.arcw")
}
