mod bench_completion {
    use super::*;

    const FINITE: &str = r#"
entry cli @entry.run { goto @flow.work }
entry bench @entry.measure { goto @flow.work }
bench @bench.completion { measure iterations = 2 { goto @flow.work } }
flow work() -> String {
    log.info("first")
    log.info("second")
    return "done"
}
"#;

    fn bench(path: &Path, steps: usize, args: &[&str]) -> std::process::Output {
        Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("bench")
            .arg(path)
            .args([
                "--iterations",
                "2",
                "--samples",
                "1",
                "--pure-backend",
                "vm",
                "--json",
                "--steps",
            ])
            .arg(steps.to_string())
            .args(args)
            .output()
            .unwrap()
    }

    fn failed(output: &std::process::Output) -> serde_json::Value {
        assert!(
            !output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stdout)
        );
        let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
            panic!("{error}; stderr: {}", String::from_utf8_lossy(&output.stderr))
        });
        let bench = &json["benches"][0];
        assert_eq!(bench["status"], "failed", "{json}");
        let section = &bench["sections"][0];
        assert_eq!(section["status"], "failed", "{json}");
        assert!(
            section["measurement"].is_null(),
            "partial timing must not be published: {json}"
        );
        assert!(section["pure_helper"].is_null(), "{json}");
        json
    }

    #[test]
    fn incomplete_warmup_never_publishes_a_measurement() {
        let path = temp_arcw("bench-incomplete-warmup", FINITE);
        for steps in [0, 1] {
        let output = bench(
            &path,
            steps,
            &["--warmup", "1", "--mode", "one-op", "--max-ops", "1"],
        );
        let json = failed(&output);
        let diagnostics = json["benches"][0]["diagnostics"].to_string();
        assert!(diagnostics.contains("warmup iteration 1"), "{diagnostics}");
        assert!(diagnostics.contains("step limit"), "{diagnostics}");
        assert!(
            !diagnostics.contains("adapter registration"),
            "{diagnostics}"
        );
        }
    }

    #[test]
    fn failed_runtime_never_publishes_a_measurement() {
        let path = temp_arcw(
            "bench-runtime-failed",
            r#"
entry bench @entry.measure { goto @flow.work }
bench @bench.completion { measure iterations = 2 { goto @flow.work } }
flow work(divisor: i64) -> String {
    log.info(7i64 / divisor)
    return "done"
}
"#,
        );
        let output = bench(
            &path,
            8,
            &[
                "--warmup",
                "0",
                "--mode",
                "drain",
                "--max-ops",
                "64",
                "--value",
                "divisor=0",
            ],
        );
        let json = failed(&output);
        let diagnostics = json["benches"][0]["diagnostics"].to_string();
        assert!(
            diagnostics.contains("measured iteration 1"),
            "{diagnostics}"
        );
        assert!(diagnostics.contains("division"), "{diagnostics}");
        assert!(
            !diagnostics.contains("adapter registration"),
            "{diagnostics}"
        );
    }

    #[test]
    fn zero_work_is_rejected_before_execution() {
        let path = temp_arcw("bench-zero-work", FINITE);
        for option in ["--iterations", "--samples"] {
            let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("bench")
                .arg(&path)
                .args([option, "0", "--json"])
                .output()
                .unwrap();
            assert!(!output.status.success());
            let diagnostic = String::from_utf8_lossy(&output.stderr);
            assert!(diagnostic.contains(option), "{diagnostic}");
            assert!(diagnostic.contains("invalid value '0'"), "{diagnostic}");
            assert!(output.stdout.is_empty());
        }
    }

    #[test]
    fn later_failure_discards_earlier_successful_samples() {
        let dir = temp_dir("bench-late-failure");
        let path = dir.join("bench.arcw");
        let save = dir.join(".arcweft").join("save");
        fs::create_dir_all(&save).unwrap();
        fs::write(save.join("state.txt"), "ready").unwrap();
        fs::write(
            &path,
            r#"
entry bench @entry.measure { goto @flow.work }
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<Result<String, FsError>> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Result<Unit, FsError>> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }
bench @bench.completion { measure iterations = 2 { goto @flow.work } }
flow work() -> String effects { fs.read(save), fs.write(save), log.write } {
    let text = match (await fs.read_text(path.save("state.txt"))) {
        .Ok(text) => text, .Err(_) => "read_failed"
    }
    let divisor = match (await fs.write_text(path.save("state.txt"), "spent")) {
        .Ok(_) => if text == "ready" { 1i64 } else { 0i64 }, .Err(_) => 0i64
    }
    log.info(7i64 / divisor)
    return "done"
}
"#,
        )
        .unwrap();
        let output = bench(&path, 64, &["--warmup", "0", "--mode", "drain", "--max-ops", "64"]);
        let json = failed(&output);
        let diagnostics = json["benches"][0]["diagnostics"].to_string();
        assert!(diagnostics.contains("measured iteration 2"), "{diagnostics}");
        assert!(diagnostics.contains("division"), "{diagnostics}");
        assert!(!String::from_utf8_lossy(&output.stdout).contains(&dir.display().to_string()));
        assert_eq!(fs::read_to_string(save.join("state.txt")).unwrap(), "spent");
    }

    #[test]
    fn last_allowed_step_completes_but_one_step_less_cannot_measure() {
        let path = temp_arcw("bench-completion-boundary", FINITE);
        for executor in ["bytecode-vm", "aot"] {
            let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.run",
                    "--executor",
                    executor,
                    "--mode",
                    "one-op",
                    "--max-ops",
                    "1",
                    "--steps",
                    "64",
                    "--pure-backend",
                    "vm",
                    "--json",
                ])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["final_status"], "done Return(\"done\")", "{json}");
            let steps = json["steps"].as_array().unwrap().len();
            assert!(steps > 1);
            let args = [
                "--warmup",
                "0",
                "--executor",
                executor,
                "--mode",
                "one-op",
                "--max-ops",
                "1",
            ];
            let complete = bench(&path, steps, &args);
            assert!(
                complete.status.success(),
                "{}",
                String::from_utf8_lossy(&complete.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&complete.stdout).unwrap();
            assert_eq!(json["benches"][0]["status"], "measured", "{json}");
            assert_eq!(
                json["benches"][0]["sections"][0]["measurement"]["steps"],
                steps
            );
            let incomplete = bench(&path, steps - 1, &args);
            failed(&incomplete);
        }
    }
}
