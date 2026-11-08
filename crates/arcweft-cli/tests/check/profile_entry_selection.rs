fn profiled_cli_source() -> &'static str {
    r"
extern capability cli {
    fn args() -> Vec<String>
}

entry cli @entry.cli.main { goto @flow.main }

flow main() -> Vec<String> effects {} {
    return cli.args()
}
"
}

fn profiled_test_source() -> &'static str {
    r#"
struct TestState {
    passed: bool
}

enum TestEvent {
    Start
}

fn initial_test_state() -> TestState
effects {}
{
    TestState { passed = true }
}

fn reduce_test(state: &TestState, event: TestEvent)
    -> Result<Reduction<TestState>, ReducerError>
effects {}
{
    Ok(Reduction.unchanged(state))
}

entry test @entry.validation.opening {
    state = TestState
    initializer = initial_test_state
    event = TestEvent
    reducer = reduce_test
    goto @flow.opening
}

test @test.opening scenario {
    goto @flow.opening
    expect.no_assertion_failures()
}

flow opening(state: TestState) -> String effects {} {
    return "done"
}
"#
}

fn profiled_bench_source() -> &'static str {
    r#"
entry bench @entry.measurement.opening { goto @flow.bench_profile }

flow bench_profile() -> String effects {} {
    return "done"
}

bench @bench.opening {
    measure iterations = 1 { goto @flow.bench_profile }
}
"#
}

fn cli_test_bench_profile_manifest() -> &'static str {
    r#"
schema = 1

[package]
id = "org.arcweft.test.profile-cli-test-bench"
version = "0.1.0"

[profiles."cli.main"]
kind = "cli"
source = "src/tool.arcw"
entry = "@entry.cli.main"
adapter = "native-cli"

[profiles."test.opening"]
kind = "test"
entry = "@entry.validation.opening"
source = "src/opening_test.arcw"

[profiles."bench.opening"]
kind = "bench"
entry = "@entry.measurement.opening"
source = "src/opening_bench.arcw"
"#
}

#[test]
fn cli_test_and_bench_profiles_use_profile_sources() {
    let dir = temp_dir("profile-cli-test-bench");
    let cli_source = dir.join("src/tool.arcw");
    let test_source = dir.join("src/opening_test.arcw");
    let bench_source = dir.join("src/opening_bench.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(&cli_source, profiled_cli_source()).expect("write cli source");
    fs::write(&test_source, profiled_test_source()).expect("write test source");
    fs::write(&bench_source, profiled_bench_source()).expect("write bench source");
    fs::write(&manifest, cli_test_bench_profile_manifest()).expect("write launch manifest");

    let cli = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("cli")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("cli.main")
        .arg("--json")
        .arg("--steps")
        .arg("2")
        .arg("--")
        .arg("alice")
        .output()
        .expect("arcw cli --profile runs");
    assert!(
        cli.status.success(),
        "cli profile should run, stderr: {}",
        String::from_utf8_lossy(&cli.stderr)
    );
    let cli_json: serde_json::Value =
        serde_json::from_slice(&cli.stdout).expect("cli profile output is structured JSON");
    assert!(
        cli_json["final_status"]
            .as_str()
            .is_some_and(|status| status.contains("seq/values/1")),
        "cli profile should return the supplied args sequence with one item: {cli_json}"
    );
    assert_eq!(
        cli_json["native_io"]["completed_tasks"],
        serde_json::json!(1),
        "cli.args should complete through the standard host adapter: {cli_json}"
    );

    let test = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("test")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("test.opening")
        .arg("--json")
        .output()
        .expect("arcw test --profile runs");
    assert!(
        test.status.success(),
        "test profile should run, stderr: {}",
        String::from_utf8_lossy(&test.stderr)
    );

    let test_json: serde_json::Value =
        serde_json::from_slice(&test.stdout).expect("test profile output is structured JSON");
    let tests = test_json["tests"]
        .as_array()
        .expect("profile scenario inventory");
    assert_eq!(tests.len(), 1);
    assert_eq!(tests[0]["id"], "test.opening");
    assert_eq!(tests[0]["kind"], "scenario");
    assert_eq!(tests[0]["status"], "passed");
    assert_eq!(tests[0]["final_status"], "done Return(\"done\")");
    assert!(
        tests[0]["steps_run"]
            .as_u64()
            .is_some_and(|steps| steps > 0)
    );
    assert_eq!(tests[0]["diagnostics"], serde_json::json!([]));

    let bench = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("bench.opening")
        .arg("--json")
        .output()
        .expect("arcw bench --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(
        bench.status.success(),
        "bench profile should run, stderr: {}",
        String::from_utf8_lossy(&bench.stderr)
    );
    let bench_json: serde_json::Value =
        serde_json::from_slice(&bench.stdout).expect("bench profile output is structured JSON");
    let benches = bench_json["benches"]
        .as_array()
        .expect("profile bench inventory");
    assert_eq!(benches.len(), 1);
    assert_eq!(benches[0]["id"], "bench.opening");
    assert_eq!(benches[0]["status"], "measured");
    assert_eq!(benches[0]["sections"][0]["status"], "measured");
    assert!(
        benches[0]["sections"][0]["measurement"]["deterministic"]["executed_ops_median"]
            .as_u64()
            .is_some_and(|ops| ops > 0),
        "the selected bench source must execute its measured flow: {bench_json}",
    );
    assert_eq!(benches[0]["diagnostics"], serde_json::json!([]));
}
