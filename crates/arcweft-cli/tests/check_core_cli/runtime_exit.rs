use super::{CommandOutput, temp_arcw};
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

const EXECUTORS: [&str; 3] = ["bytecode-vm", "aot", "awbc-product"];

fn arithmetic_fixture(label: &str) -> PathBuf {
    temp_arcw(
        label,
        r#"
flow work(divisor: i64) -> i64 {
    log.info("first")
    log.info("second")
    return 7i64 / divisor
}
"#,
    )
}

fn run(path: &Path, executor: &str, divisor: &str, flags: &[&str]) -> CommandOutput {
    let mut arguments = vec![OsStr::new("run"), path.as_os_str()];
    arguments.extend(
        ["--flow", "work", "--executor", executor, "--value", divisor]
            .into_iter()
            .map(OsStr::new),
    );
    arguments.extend(flags.iter().copied().map(OsStr::new));
    CommandOutput::run(arguments).expect("execute the catalog-built CLI")
}

#[test]
fn headless_runtime_failure_returns_nonzero_after_publishing_its_report() {
    let path = arithmetic_fixture("runtime-failure-exit");
    for executor in EXECUTORS {
        for json in [false, true] {
            let mut flags = vec!["--mode", "drain", "--steps", "2", "--max-ops", "64"];
            if json {
                flags.push("--json");
            }
            let output = run(&path, executor, "divisor=0", &flags);
            output.assert_failure();
            assert_eq!(output.status().code(), Some(1), "{}", output.stderr());
            if json {
                let report: serde_json::Value = serde_json::from_str(&output.stdout())
                    .expect("failed execution still publishes the complete run report");
                let status = report["final_status"].as_str().expect("execution status");
                assert!(status.starts_with("failed "), "{report}");
                assert!(status.contains("division by zero"), "{report}");
                assert!(
                    !report["steps"]
                        .as_array()
                        .expect("runtime steps")
                        .is_empty()
                );
                assert!(
                    report.get("executor").is_some(),
                    "execution, not compilation, failed"
                );
            } else {
                let stdout = output.stdout();
                assert!(
                    stdout.lines().any(|line| line.starts_with("failed: ")),
                    "{stdout}"
                );
                assert!(
                    !stdout.lines().any(|line| line.starts_with("ok: ")),
                    "{stdout}"
                );
                assert!(
                    stdout.contains("final_status=failed division by zero"),
                    "{stdout}"
                );
            }
        }
    }
    fs::remove_file(path).expect("remove arithmetic fixture");
}

#[test]
fn headless_completed_execution_keeps_a_successful_exit() {
    let path = arithmetic_fixture("runtime-completed-exit");
    for executor in EXECUTORS {
        let output = run(
            &path,
            executor,
            "divisor=1",
            &[
                "--mode",
                "drain",
                "--steps",
                "2",
                "--max-ops",
                "64",
                "--json",
            ],
        );
        output.assert_success();
        let report: serde_json::Value = serde_json::from_str(&output.stdout()).expect("run report");
        assert_eq!(report["final_status"], "done Return(\"7\")");
        assert!(
            !report["steps"]
                .as_array()
                .expect("runtime steps")
                .is_empty()
        );
    }
    fs::remove_file(path).expect("remove arithmetic fixture");
}

#[test]
fn headless_budget_stop_remains_a_successful_bounded_observation() {
    let path = arithmetic_fixture("runtime-budget-stop-exit");
    for executor in EXECUTORS {
        let output = run(
            &path,
            executor,
            "divisor=1",
            &[
                "--mode",
                "one-op",
                "--steps",
                "1",
                "--max-ops",
                "1",
                "--json",
            ],
        );
        output.assert_success();
        let report: serde_json::Value =
            serde_json::from_str(&output.stdout()).expect("bounded run report");
        assert_eq!(report["final_status"], "running");
        assert_eq!(report["steps"].as_array().expect("runtime steps").len(), 1);
    }
    fs::remove_file(path).expect("remove arithmetic fixture");
}
