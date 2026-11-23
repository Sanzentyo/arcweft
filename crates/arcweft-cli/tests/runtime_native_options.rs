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
fn session_io_requires_an_explicit_native_or_headless_runner() {
    let fixture = fixture();
    for runner in ["auto", "web"] {
        for (option, path) in session_options() {
            assert_run_usage_error(
                &[
                    "--runner",
                    runner,
                    fixture.to_str().expect("fixture path is UTF-8"),
                    option,
                    path,
                ],
                "--session-load and --session-save-out require --runner native or headless",
            );
        }
    }
}

#[test]
fn headless_native_session_preserves_restart_point_and_rejects_a_different_entry() {
    let directory = tempfile::tempdir().unwrap();
    let source = directory.path().join("checkpoint.arcw");
    let saved = directory.path().join("checkpoint.awfs");
    let completed = directory.path().join("completed.awfs");
    std::fs::write(
        &source,
        "flow main() -> i64 {
             let first = 6i64
             let second = first + 1i64
             return second
         }
         entry cli @entry.main { goto @flow.main }
         entry cli @entry.alias { goto @flow.main }",
    )
    .unwrap();
    let save = saved.to_str().unwrap();
    let finished_save = completed.to_str().unwrap();
    let capture = run_headless_entry(
        &source,
        "entry.main",
        &["--json", "--steps", "1", "--session-save-out", save],
    );
    assert!(capture.status.success(), "{capture:?}");
    let initial: serde_json::Value = serde_json::from_slice(&capture.stdout).unwrap();
    assert_eq!(initial["steps"][0]["index"], 0);
    assert_eq!(initial["final_status"], "running");
    let original = std::fs::read(&saved).unwrap();

    let wrong_entry = run_headless_entry(
        &source,
        "entry.alias",
        &["--session-load", save, "--session-save-out", save],
    );
    assert!(!wrong_entry.status.success(), "{wrong_entry:?}");
    assert!(String::from_utf8_lossy(&wrong_entry.stderr).contains("original invocation or entry"));
    assert_eq!(std::fs::read(&saved).unwrap(), original);

    std::fs::write(&completed, b"previous checkpoint").unwrap();
    let resumed = run_headless_entry(
        &source,
        "entry.main",
        &[
            "--executor",
            "aot",
            "--json",
            "--steps",
            "32",
            "--mode",
            "drain",
            "--max-ops",
            "64",
            "--session-load",
            save,
            "--session-save-out",
            finished_save,
        ],
    );
    assert!(resumed.status.success(), "{resumed:?}");
    let report: serde_json::Value = serde_json::from_slice(&resumed.stdout).unwrap();
    assert_eq!(report["steps"][0]["index"], 1);
    assert_eq!(report["final_status"], "done Return(\"7\")");
    let finished = std::fs::read(&completed).unwrap();
    assert!(finished.starts_with(b"AWFS"));
    assert_ne!(finished, original);

    let observed = run_headless_entry(
        &source,
        "entry.main",
        &["--json", "--steps", "0", "--session-load", finished_save],
    );
    assert!(observed.status.success(), "{observed:?}");
    let observation: serde_json::Value = serde_json::from_slice(&observed.stdout).unwrap();
    assert_eq!(observation["steps"], serde_json::json!([]));
    assert_eq!(observation["final_status"], report["final_status"]);
    assert_eq!(std::fs::read(&completed).unwrap(), finished);

    let mut corrupt = finished;
    *corrupt.last_mut().unwrap() ^= 1;
    std::fs::write(&completed, &corrupt).unwrap();
    let corrupted = run_headless_entry(
        &source,
        "entry.main",
        &["--session-load", finished_save, "--session-save-out", save],
    );
    assert!(!corrupted.status.success(), "{corrupted:?}");
    assert!(String::from_utf8_lossy(&corrupted.stderr).contains("source session save codec error"));
    assert_eq!(std::fs::read(&saved).unwrap(), original);
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

fn run_headless_entry(source: &Path, entry: &str, options: &[&str]) -> Output {
    let mut arguments = vec![
        "--runner",
        "headless",
        source.to_str().unwrap(),
        "--entry",
        entry,
    ];
    arguments.extend_from_slice(options);
    run(&arguments)
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
