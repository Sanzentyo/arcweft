fn profiled_cli_source() -> &'static str {
    r"
mod crate

entry cli @entry.cli.main { goto @flow.main }

flow main(argc: i32) {
    return argc
}
"
}

fn profiled_test_source() -> &'static str {
    r#"
mod crate

struct TestState {
    passed: bool
}

enum TestEvent {
    Start
}

fn initial_test_state() -> TestState
effects {}
{
    initial_test_state()
}

fn reduce_test(state: &TestState, event: TestEvent)
    -> Result<Reduction<TestState>, ReducerError>
effects {}
{
    reduce_test(state, event)
}

entry test @entry.test.opening {
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

flow opening(state: TestState) {
    return "done"
}
"#
}

fn profiled_bench_source() -> &'static str {
    r#"
mod crate

entry bench @entry.bench.opening { goto @flow.bench_profile }

flow bench_profile {
    return "done"
}

bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 1 { opening_choices() }
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

[profiles."test.opening"]
kind = "test"
entry = "@entry.test.opening"
source = "src/opening_test.arcw"

[profiles."bench.opening"]
kind = "bench"
entry = "@entry.bench.opening"
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
        .arg("--")
        .arg("alice")
        .output()
        .expect("arcw cli --profile runs");
    assert!(
        cli.status.success(),
        "cli profile should run, stderr: {}",
        String::from_utf8_lossy(&cli.stderr)
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
}
