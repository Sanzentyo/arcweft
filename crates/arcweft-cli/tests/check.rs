use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn jit_check_json_compares_cranelift_and_vm() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("7")
        .output()
        .expect("arcw jit check runs");

    assert!(
        output.status.success(),
        "jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"ok\"")
            && stdout.contains("\"jit_backend\": \"jit\"")
            && stdout.contains("\"aot_backend\": \"aot\"")
            && stdout.contains("\"matches_vm\": true")
            && stdout.contains("\"aot_compile_elapsed_ns\"")
            && stdout.contains("\"compile_elapsed_ns\"")
            && stdout.contains("\"aot_elapsed_ns\"")
            && stdout.contains("\"jit_elapsed_ns\"")
            && stdout.contains("\"vm_elapsed_ns\""),
        "jit check JSON should include conformance and timing data: {stdout}"
    );
    assert!(
        stdout.contains("\"speedup_x\"")
            && stdout.contains("\"dynamic_inputs\": true")
            && stdout.contains("\"input_seed\": 7")
            && stdout.contains("\"input_bindings\"")
            && stdout.contains("\"jit_per_iteration_ns\"")
            && stdout.contains("\"aot_per_iteration_ns\"")
            && stdout.contains("\"vm_per_iteration_ns\"")
            && stdout.contains("\"aot_samples\"")
            && stdout.contains("\"jit_samples\"")
            && stdout.contains("\"vm_samples\""),
        "jit check JSON should include conformance and timing data: {stdout}"
    );
}

#[test]
fn jit_check_json_uses_source_pure_helper() {
    let path = temp_arcw(
        "jit-pure-helper",
        r"
#[pure]
fn score(base: i64, bonus: i64, scale: i64) -> i64 {
    let boosted = bonus + 2
    let weighted = base * boosted
    return if base >= 3 { weighted + scale } else { scale }
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg(&path)
        .arg("--helper")
        .arg("score")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("3")
        .output()
        .expect("arcw jit check source helper runs");
    fs::remove_file(&path).expect("remove temp pure helper");

    assert!(
        output.status.success(),
        "jit source check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"helper\": \"score\"")
            && stdout.contains("\"helper_source\": \"source\"")
            && stdout.contains("\"matches_vm\": true")
            && stdout.contains("\"scale\"")
            && stdout.contains("\"input_seed\": 3"),
        "jit check JSON should describe the source helper: {stdout}"
    );
}

#[test]
fn check_accepts_valid_arcw_file() {
    let path = temp_arcw(
        "valid",
        r"
pub surface character @character.alice Alice as alice {
}

flow @flow.opening opening {
    @<character.alice>.say[待って。[mark .release][p]]
    with:
        init:
            'line.flag <- true
        at(0.25s): 'line.flag |> drop_optional
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    assert!(
        output.status.success(),
        "expected success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("1 line task group"),
        "stdout should include runtime-plan count"
    );
}

#[test]
fn check_json_reports_compiler_pipeline_summary() {
    let path = temp_arcw(
        "valid-json",
        r#"
flow @flow.opening opening {
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw check runs");

    assert!(
        output.status.success(),
        "expected JSON check success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"ok\"")
            && stdout.contains("\"flows\": 1")
            && stdout.contains("\"line_task_groups\"")
            && stdout.contains("\"verifier_obligations\""),
        "check JSON should include pipeline summary: {stdout}"
    );
}

#[test]
fn check_accepts_state_write_effect_contract() {
    let path = temp_arcw(
        "state-write-effect",
        r"
flow @flow.registry registry
effects { state.write('flow) }
{
    'flow.flags.seen <- true
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    assert!(
        output.status.success(),
        "expected state.write effect to pass, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_rejects_unlowered_line_plan_item() {
    let path = temp_arcw(
        "unsupported-line-plan",
        r"
pub surface character @character.alice Alice as alice {
}

flow @flow.unsupported unsupported {
    @<character.alice>.say[待って。[p]]
    with:
        @bad raw item
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    assert!(
        !output.status.success(),
        "unsupported line plan item must fail"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("raw expression"),
        "stderr should explain the unsupported line-plan item"
    );
}

#[test]
fn check_rejects_invalid_arcw_file() {
    let path = temp_arcw(
        "invalid",
        r"
flow @flow.bad bad {
    alice[unclosed
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    assert!(!output.status.success(), "invalid source must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("error:"),
        "stderr should contain diagnostics"
    );
}

#[test]
fn verify_json_reports_missing_promotion_proof() {
    let path = temp_arcw(
        "verify-missing-proof",
        r"
flow @flow.verify verify {
    let summary = promote('flow)
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        !output.status.success(),
        "missing promotion proof should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("lifetime_promotion"),
        "JSON report should include the promotion obligation: {stdout}"
    );
}

#[test]
fn verify_json_records_required_solver_checks() {
    let path = temp_arcw(
        "verify-solver-check",
        r"
flow @flow.verify verify {
    let summary = promote('flow)
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--backend")
        .arg("oxiz")
        .arg("--json")
        .output()
        .expect("arcw verify with oxiz runs");

    assert!(
        !output.status.success(),
        "required unknown solver check should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"solver_checks\"")
            && stdout.contains("\"backend\": \"oxiz\"")
            && stdout.contains("\"outcome\": \"unknown\"")
            && stdout.contains("\"required\": true"),
        "JSON report should include the required solver check: {stdout}"
    );
}

#[test]
fn verify_json_reports_semantic_thread_join_conflict() {
    let path = temp_arcw(
        "verify-thread-join",
        r#"
pub surface character @character.alice Alice as alice {
}

flow @flow.thread_join thread_join {
    alice[待って。[p]]
    with:
        thread worker:
            out 1i32
            out "bad"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        !output.status.success(),
        "semantic thread join conflict should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("thread join result branches must produce one compatible type"),
        "JSON report should include the semantic thread-join obligation: {stdout}"
    );
}

#[test]
fn verify_json_reports_effect_capability_obligation() {
    let path = temp_arcw(
        "verify-effect-capability",
        r"
signal @signal:.current_flow: Watch<Ref<Flow>>

flow @flow.effects effects {
    signal.set(@signal.current_flow, @flow.effects)
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        !output.status.success(),
        "missing effect capability should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("effect_capability") && stdout.contains("signal.write"),
        "JSON report should include the effect capability obligation: {stdout}"
    );
}

#[test]
fn verify_json_accepts_effect_capability_from_flow_contract() {
    let path = temp_arcw(
        "verify-effect-contract",
        r"
signal @signal:.current_flow: Watch<Ref<Flow>>

flow @flow.effects effects
effects { signal.write }
{
    signal.set(@signal.current_flow, @flow.effects)
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        output.status.success(),
        "flow effects clause should discharge signal.write, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn verify_json_reports_invalid_proof_body() {
    let path = temp_arcw(
        "verify-proof-body",
        r"
proof @proof.requires_only {
    requires summary.lifetime >= 'flow
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        !output.status.success(),
        "invalid proof body should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("proof_body") && stdout.contains("proof.requires_only"),
        "JSON report should include the proof body obligation: {stdout}"
    );
}

#[test]
fn verify_json_reports_unknown_proof_axiom() {
    let path = temp_arcw(
        "verify-proof-axiom",
        r"
proof @proof.missing_axiom {
    use @axiom.missing
    check no_lifetime_below(LineSummary, 'flow)
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        !output.status.success(),
        "unknown proof axiom should fail test-mode verification"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("proof_body") && stdout.contains("axiom.missing"),
        "JSON report should include the unknown axiom obligation: {stdout}"
    );
}

#[test]
fn verify_json_respects_semantic_defer_cancel_discharge() {
    let path = temp_arcw(
        "verify-cancel-defer",
        r"
pub surface character @character.alice Alice as alice {
}

flow @flow.cancel_cleanup cancel_cleanup {
    @<character.alice>.say[待って。[p]]
    with:
        init:
            'line.focus <- true
        defer on completed:
            'line.focus |> drop_optional
        defer on cancelled:
            'line.focus |> drop_optional
        cancel on input(.SkipLine) { out .Skipped }
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify")
        .arg(&path)
        .arg("--mode")
        .arg("test")
        .arg("--json")
        .output()
        .expect("arcw verify runs");

    assert!(
        output.status.success(),
        "completed and cancelled defers should discharge line focus, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unsafe_json_lists_audit_regions() {
    let path = temp_arcw(
        "unsafe-audit",
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache reason = "owned clone" {
        /// SAFETY: value is owned before promotion
        let summary = promote_unchecked('flow)
    }
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("unsafe")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw unsafe runs");

    assert!(
        output.status.success(),
        "unsafe listing should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("cache"),
        "unsafe JSON should include audit id: {stdout}"
    );
}

#[test]
fn plan_json_lists_runtime_task_graph() {
    let path = temp_arcw(
        "runtime-plan",
        r#"
pub surface character @character.alice Alice as alice {
}

flow @flow.plan plan {
    @<character.alice>.say[待って。[mark .release][p]]
    with:
        thread motion:
            wait(0.1s)
        on mark(.release):
            log.info("release")
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("plan")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw plan runs");

    assert!(
        output.status.success(),
        "plan listing should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"lines\"")
            && stdout.contains("\"child_tasks\": 2")
            && stdout.contains("mark .release"),
        "plan JSON should include runtime graph metadata: {stdout}"
    );
}

#[test]
fn run_json_steps_runtime_plan() {
    let path = temp_arcw(
        "runtime-run",
        r"
pub surface character @character.alice Alice as alice {
}

flow @flow.run run {
    @<character.alice>.say[待って。[p]]
    with:
        out .Done
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--steps")
        .arg("2")
        .arg("--json")
        .output()
        .expect("arcw run runs");

    assert!(
        output.status.success(),
        "runtime dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"flow_events\"")
            && stdout.contains("dialogue")
            && stdout.contains("done"),
        "run JSON should include step events and final status: {stdout}"
    );
}

#[test]
fn run_json_modes_and_budget_drive_engine_step_boundary() {
    let path = temp_arcw(
        "runtime-step-modes",
        r#"
flow @flow.run run {
    log.info("first")
    log.info("second")
    return "done"
}
"#,
    );

    let one_op = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("one-op")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw run runs");
    assert!(
        one_op.status.success(),
        "one-op run should succeed, stderr: {}",
        String::from_utf8_lossy(&one_op.stderr)
    );
    let one_op_stdout = String::from_utf8_lossy(&one_op.stdout);
    assert!(
        one_op_stdout.contains("\"stop_reason\": \"OneOp\"")
            && one_op_stdout.contains("\"executed_ops\": 1")
            && one_op_stdout.contains("\"final_status\": \"running\""),
        "one-op should return after one VM op: {one_op_stdout}"
    );

    let drain = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run runs");
    assert!(
        drain.status.success(),
        "drain run should succeed, stderr: {}",
        String::from_utf8_lossy(&drain.stderr)
    );
    let drain_stdout = String::from_utf8_lossy(&drain.stdout);
    assert!(
        drain_stdout.contains("\"stop_reason\": \"Done\"")
            && drain_stdout.contains("\"final_status\": \"done"),
        "drain should finish within one host step: {drain_stdout}"
    );

    let budget = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw run runs");
    assert!(
        budget.status.success(),
        "budgeted run should succeed, stderr: {}",
        String::from_utf8_lossy(&budget.stderr)
    );
    let budget_stdout = String::from_utf8_lossy(&budget.stdout);
    assert!(
        budget_stdout.contains("\"stop_reason\": \"BudgetExhausted\""),
        "drain max-ops should stop with budget exhaustion: {budget_stdout}"
    );
}

#[test]
fn profile_json_reports_phase_timings_and_runtime_stats_without_absolute_source() {
    let path = temp_arcw(
        "profile-json",
        r#"
flow @flow.profile profile {
    log.info("profile")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("profile")
        .arg(&path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw profile runs");

    assert!(
        output.status.success(),
        "profile should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"name\": \"parse\"")
            && stdout.contains("\"name\": \"typecheck\"")
            && stdout.contains("\"name\": \"runtime_type_validate\"")
            && stdout.contains("\"name\": \"bytecode_lower\"")
            && stdout.contains("\"name\": \"run\"")
            && stdout.contains("\"compiler\"")
            && stdout.contains("\"typecheck\"")
            && stdout.contains("\"borrow_check\"")
            && stdout.contains("\"runtime_type_validation\"")
            && stdout.contains("\"bytecode\"")
            && stdout.contains("\"instructions\"")
            && stdout.contains("\"expressions\"")
            && stdout.contains("\"judgments\"")
            && stdout.contains("\"judgment_rules\"")
            && stdout.contains("\"judgment_samples\"")
            && stdout.contains("\"boundary_checks\"")
            && stdout.contains("\"executed_ops\": 2")
            && stdout.contains("\"source\": \"arcweft-cli-profile-json-"),
        "profile json should include phase timings, compiler stats, borrow stats, and VM stats: {stdout}"
    );
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "profile json must not record absolute temp paths: {stdout}"
    );
}

#[test]
fn cli_json_selects_cli_entry_and_binds_args() {
    let path = temp_arcw(
        "cli-entry",
        r"
entry cli @entry.main { run(@flow.main) }

flow @flow.main main(argc: i32) {
    return argc
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("cli")
        .arg(&path)
        .arg("--json")
        .arg("--")
        .arg("one")
        .arg("two")
        .output()
        .expect("arcw cli runs");
    assert!(
        output.status.success(),
        "cli run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"final_status\": \"done") && stdout.contains("return 2"),
        "cli entry should bind argc from trailing args: {stdout}"
    );
}

#[test]
fn run_json_reports_headless_observations() {
    let path = temp_arcw(
        "runtime-observations",
        r#"
signal @signal:.current_flow: Watch<Ref<Flow>>
metric gauge @metric.frame_count: i32

flow @flow.observed observed
effects { signal.write, metric.write }
{
    log.info("enter observed")
    signal.set(@signal.current_flow, @flow.observed)
    metric.set(@metric.frame_count, 1i32)
    event.emit("GameEvent::Entered")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--steps")
        .arg("5")
        .arg("--json")
        .output()
        .expect("arcw run runs");

    assert!(
        output.status.success(),
        "runtime dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"observations\"")
            && stdout.contains("signal.current_flow")
            && stdout.contains("metric.frame_count")
            && stdout.contains("enter observed")
            && stdout.contains("GameEvent::Entered"),
        "run JSON should include cumulative headless observations: {stdout}"
    );
}

#[test]
fn plan_json_lists_generation_plans() {
    let path = temp_arcw(
        "generation-plan",
        r#"
stream fn passthrough(frames: Stream<IteratorItem, CaptureError>) -> Stream<IteratorItem, CaptureError> {
    for frame in frames {
        yield frame
    }
}

pub source @source.fixture_frames: Source<IteratorItem, CaptureError> {
    from "fixture"
    backpressure = latest
    replay = hash_only
    privacy = transient

    on item frame => yield frame
}

flow @flow.generation generation {
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("plan")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw plan runs");

    assert!(
        output.status.success(),
        "generation plan listing should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"streams\"")
            && stdout.contains("passthrough")
            && stdout.contains("\"sources\"")
            && stdout.contains("source.fixture_frames")
            && stdout.contains("HashOnly"),
        "plan JSON should include stream/source metadata: {stdout}"
    );
}

#[test]
fn run_json_lists_source_and_stream_runtime_state() {
    let path = temp_arcw(
        "generation-run",
        r#"
stream fn passthrough(frames: Stream<IteratorItem, CaptureError>) -> Stream<IteratorItem, CaptureError> {
    for frame in frames {
        yield frame
    }
}

pub source @source.fixture_frames: Source<IteratorItem, CaptureError> {
    from "fixture"
    backpressure = latest
    replay = hash_only
    privacy = transient

    on item frame => yield frame
}

flow @flow.generation generation {
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw run runs");

    assert!(
        output.status.success(),
        "generation runtime dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"source_states\"")
            && stdout.contains("source.fixture_frames")
            && stdout.contains("\"stream_states\"")
            && stdout.contains("passthrough"),
        "run JSON should include source/stream runtime state: {stdout}"
    );
}

#[test]
fn run_json_executes_scope_and_loop_value_bindings() {
    let path = temp_arcw(
        "runtime-value-bindings",
        r#"
flow @flow.value_bindings value_bindings {
    let local_target = scope target_scope {
        let candidate = @flow.done
        candidate
    }

    let next = 'pick: loop {
        break 'pick local_target
    }

    goto next
}

flow @flow.done done {
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--steps")
        .arg("12")
        .arg("--json")
        .output()
        .expect("arcw run runs");

    assert!(
        output.status.success(),
        "runtime dry-run should execute scope/loop value bindings, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("goto flow.done") && stdout.contains("done Return"),
        "run JSON should include goto produced by loop break value: {stdout}"
    );
}

#[test]
fn fmt_preserves_sugar_by_default() {
    let source = "flow @flow.opening opening {\n    alice: hi[p]\n}\n";
    let path = temp_arcw("fmt-preserve", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg(&path)
        .output()
        .expect("arcw fmt runs");

    assert!(
        output.status.success(),
        "fmt should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("alice: hi[p]"),
        "default fmt should preserve authoring sugar"
    );
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn fmt_expand_sugar_accepts_flags_before_path_and_writes() {
    let path = temp_arcw(
        "fmt-expand",
        "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi[p]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg("--expand-sugar")
        .arg("--write")
        .arg(&path)
        .output()
        .expect("arcw fmt runs");

    assert!(
        output.status.success(),
        "fmt --expand-sugar should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rewritten = fs::read_to_string(&path).expect("rewritten source");
    assert!(rewritten.contains("alice.say()[hi[p]]"));
    assert!(rewritten.contains("with {"));
    assert!(rewritten.contains("goto super::next"));
}

#[test]
fn ids_materialize_accepts_flags_before_path_without_write() {
    let source = "flow @flow.opening opening {\n    scope rain {\n        alice(id=@.comment, text_key=@.comment_text):\n            Hi[p]\n    }\n    alice:\n        Omitted[p]\n=== line 地の文 ===\nFlat[p]\n=== with ===\nwait(mark(.done))\n=== /with ===\n=== /line ===\n    choice @.first {\n        @.listen \"Listen\" -> @flow.next\n    }\n}\n";
    let path = temp_arcw("ids-materialize", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("ids")
        .arg("materialize")
        .arg("--json")
        .arg(&path)
        .output()
        .expect("arcw ids materialize runs");

    assert!(
        output.status.success(),
        "ids materialize should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(
        "alice(id=@say.opening.alice.rain.comment, text_key=@text.opening.alice.rain.comment_text):"
    ));
    assert!(stdout.contains("alice(id=@say.opening.alice.001, text_key=@text.opening.alice.001):"));
    assert!(stdout.contains(
        "=== line 地の文(id=@say.opening.narrator.001, text_key=@text.opening.narrator.001) ==="
    ));
    assert!(stdout.contains("choice @choice.opening.first"));
    assert!(stdout.contains("@choice.opening.first.listen"));
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn test_json_lists_script_tests() {
    let path = temp_arcw(
        "script-test",
        r"
test @test.opening scenario {
    start(@flow.opening)
    expect.no_assertion_failures()
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("test")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw test runs");

    assert!(
        output.status.success(),
        "test listing should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test.opening") && stdout.contains("scenario"),
        "test JSON should include script test metadata: {stdout}"
    );
}

#[test]
fn test_json_executes_headless_scenario_expectations() {
    let path = temp_arcw(
        "script-test-headless",
        r#"
signal @signal:.current_flow: Watch<Ref<Flow>>

flow @flow.observed observed
effects { signal.write }
{
    log.info("enter observed")
    signal.set(@signal.current_flow, @flow.observed)
    return "done"
}

test @test.observed scenario {
    start(@flow.observed)
    expect.log(.info, contains="enter observed")
    expect.signal(@signal.current_flow, @flow.observed)
    expect.no_assertion_failures()
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("test")
        .arg(&path)
        .arg("--steps")
        .arg("5")
        .arg("--json")
        .output()
        .expect("arcw test runs");

    assert!(
        output.status.success(),
        "headless scenario test should pass, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("test.observed")
            && stdout.contains("\"status\": \"passed\"")
            && stdout.contains("\"steps_run\""),
        "test JSON should include headless run result: {stdout}"
    );
}

#[test]
fn bench_json_validates_headless_script_benches() {
    let path = temp_arcw(
        "script-bench",
        r#"
metric gauge @metric:.memo_hit_rate: f32

bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 10 { opening_choices() }
    assert(metric.value(@metric.memo_hit_rate) >= 0.95)
    report { cpu_time }
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw bench runs");

    assert!(
        output.status.success(),
        "bench validation should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bench.opening")
            && stdout.contains("\"status\": \"validated\"")
            && stdout.contains("measure")
            && stdout.contains("report"),
        "bench JSON should include headless validation metadata: {stdout}"
    );
}

#[test]
fn bench_json_measures_headless_runtime_sections() {
    let path = temp_arcw(
        "script-bench-measured",
        r#"
bench @bench.runtime {
    measure iterations = 2 { start(@flow.bench) }
}

flow @flow.bench bench {
    log.info("bench")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("2")
        .arg("--warmup")
        .arg("1")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw bench runs");

    assert!(
        output.status.success(),
        "measured bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"measured\"")
            && stdout.contains("\"iterations\": 2")
            && stdout.contains("\"warmup\": 1")
            && stdout.contains("\"executed_ops_median\": 2"),
        "bench JSON should include headless measurement: {stdout}"
    );
}

#[test]
fn bench_json_skips_adapter_only_script_benches() {
    let path = temp_arcw(
        "script-bench-adapter",
        r"
bench @bench.audio {
    setup { audio.play(@bgm.alice_theme) }
    measure iterations = 3 { render_audio_offline() }
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw bench runs");

    assert!(
        output.status.success(),
        "adapter-only bench should be skipped, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bench.audio")
            && stdout.contains("\"status\": \"skipped\"")
            && stdout.contains("adapter-only"),
        "bench JSON should make unsupported headless work explicit: {stdout}"
    );
}

#[test]
fn check_rejects_non_arcw_file_extension() {
    let path = temp_file(
        "non-arcw",
        "arwt",
        r#"
flow @flow.main main {
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    fs::remove_file(&path).expect("remove temp non-arcw fixture");
    assert!(
        !output.status.success(),
        ".arwt files must not be accepted as Arcweft source"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not an .arcw source file"),
        "stderr should explain extension policy: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn check_rejects_direct_non_arcw_edge_extensions() {
    for extension in ["txt", ""] {
        let path = temp_file(
            "direct-extension-edge",
            extension,
            r#"
flow @flow.main main {
    return "done"
}
"#,
        );

        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("check")
            .arg(&path)
            .output()
            .expect("arcw check runs");

        fs::remove_file(&path).expect("remove temp non-arcw fixture");
        assert!(
            !output.status.success(),
            "direct non-arcw path with extension `{extension}` must fail"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not an .arcw source file"),
            "stderr should explain extension policy: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tooling_commands_reject_direct_non_arcw_paths() {
    for args in [&["fmt"][..], &["ids", "materialize"][..]] {
        let path = temp_file(
            "tooling-non-arcw",
            "arwt",
            r#"
flow @flow.main main {
    return "done"
}
"#,
        );

        let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
        command.args(args).arg(&path);
        let output = command.output().expect("arcw tooling command runs");

        fs::remove_file(&path).expect("remove temp non-arcw fixture");
        assert!(
            !output.status.success(),
            "{args:?} must reject direct non-arcw path"
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("not an .arcw source file"),
            "stderr should explain extension policy: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn tooling_directory_scan_ignores_non_arcw_files() {
    let dir = temp_dir("tooling-directory-scan");
    let arcw = dir.join("valid.arcw");
    let arwt = dir.join("invalid.arwt");
    fs::write(
        &arcw,
        r#"
flow @flow.main main {
    return "done"
}
"#,
    )
    .expect("write valid arcw fixture");
    fs::write(&arwt, "this is intentionally not arcw {").expect("write ignored arwt fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg(&dir)
        .output()
        .expect("arcw fmt runs");

    fs::remove_dir_all(&dir).expect("remove temp fixture dir");
    assert!(
        output.status.success(),
        "tooling directory scan should ignore non-arcw files, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn spec_valid_run_edge_fixture_now_executes() {
    let relative_path =
        "tests/fixtures/arcw/spec_should_pass/run/011_dialogue_line_value_and_handle_discard.arcw";
    let path = workspace_root().join(relative_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--json")
        .arg("--steps")
        .arg("5")
        .output()
        .expect("arcw run runs");

    assert!(
        output.status.success(),
        "{} should now execute, stdout: {}, stderr: {}",
        path.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn rejected_await_question_with_fixture_fails_with_guidance() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_fail/011_await_question_with_rejected.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&path)
        .output()
        .expect("arcw check runs");

    assert!(!output.status.success(), "ambiguous await form must fail");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("await expr? with") && stderr.contains("try await"),
        "diagnostic should point to try-await replacement: {stderr}"
    );
}

#[test]
fn spec_rejected_edge_fixtures_fail_with_diagnostics() {
    for (relative_path, expected) in [
        (
            "tests/fixtures/arcw/spec_should_fail/012_name_at_pattern_removed_rejected.arcw",
            "unresolved entity reference",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/013_continue_expr_position_rejected.arcw",
            "unknown symbol `continue`",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/014_let_else_non_diverging_rejected.arcw",
            "let-else else block must leave the current continuation",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/015_break_value_in_while_rejected.arcw",
            "break expr",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/016_yield_in_flow_rejected.arcw",
            "yield",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/017_out_in_flow_rejected.arcw",
            "`out` can only be used",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/018_private_full_replay_rejected.arcw",
            "privacy = private",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/019_unsafe_lifetime_missing_reason_rejected.arcw",
            "unsafe lifetime block requires a reason",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/020_unsafe_block_missing_safety_doc_rejected.arcw",
            "unsafe lifetime block requires a SAFETY doc comment",
        ),
    ] {
        let path = workspace_root().join(relative_path);
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("check")
            .arg(&path)
            .output()
            .expect("arcw check runs");

        assert!(
            !output.status.success(),
            "{} must be rejected by arcw check",
            path.display()
        );
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "stderr for {} should contain `{expected}`:\n{}",
            path.display(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn serve_json_lists_server_routes() {
    let path = temp_arcw(
        "serve-routes",
        r#"
entry server @entry.http {
    route GET "/health" -> @flow.health
    route POST "/save" -> @flow.save
}

flow @flow.health health {
    return "ok"
}

flow @flow.save save {
    return "saved"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("serve")
        .arg(&path)
        .arg("--entry")
        .arg("@entry.http")
        .arg("--adapter")
        .arg("native-http")
        .arg("--json")
        .output()
        .expect("arcw serve runs");

    assert!(
        output.status.success(),
        "expected serve route plan success, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"planned\"")
            && stdout.contains("\"entry\": \"entry.http\"")
            && stdout.contains("\"method\": \"GET\"")
            && stdout.contains("\"path\": \"/health\"")
            && stdout.contains("\"target\": \"flow.save\""),
        "serve JSON should list lowered server routes: {stdout}"
    );
}

#[test]
fn serve_json_typechecks_explicit_route_parameters() {
    let path = temp_arcw(
        "serve-route-params-explicit",
        r#"
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) {
    return name
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("serve")
        .arg(&path)
        .arg("--entry")
        .arg("http")
        .arg("--adapter")
        .arg("native-http")
        .arg("--json")
        .output()
        .expect("arcw serve runs");

    assert!(
        output.status.success(),
        "expected explicit route parameters to typecheck in server entry context, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn serve_json_treats_server_run_entry_as_default_route() {
    let path = temp_arcw(
        "serve-run",
        r#"
entry server @entry.server {
    run(@flow.main)
}

flow @flow.main main {
    return "server"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("serve")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw serve runs");

    assert!(
        output.status.success(),
        "expected server run entry success, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"method\": \"*\"")
            && stdout.contains("\"path\": \"*\"")
            && stdout.contains("\"target\": \"flow.main\""),
        "server run entry should become a default route: {stdout}"
    );
}

#[test]
fn profile_check_accepts_explicit_route_parameters() {
    let dir = temp_dir("profile-check-explicit-routes");
    let source = dir.join("server.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello(name = :name)
}

flow @flow.hello hello(name: String) {
    return name
}
"#,
    )
    .expect("write server profile source");
    fs::write(
        &manifest,
        r#"
[profiles."server.dev"]
kind = "server"
source = "server.arcw"
entry = "http"
adapter = "native-http"
"#,
    )
    .expect("write launch manifest");

    let profiled = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("server.dev")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(
        profiled.status.success(),
        "profiled check should accept explicit route parameters, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&profiled.stdout),
        String::from_utf8_lossy(&profiled.stderr)
    );
}

#[test]
fn profile_check_rejects_ambient_route_params() {
    let dir = temp_dir("profile-check-route-params-rejected");
    let source = dir.join("server.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
entry server @entry.http {
    route GET "/hello/:name" -> @flow.hello
}

flow @flow.hello hello {
    return route_params.name
}
"#,
    )
    .expect("write server profile source");
    fs::write(
        &manifest,
        r#"
[profiles."server.dev"]
kind = "server"
source = "server.arcw"
entry = "http"
adapter = "native-http"
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("server.dev")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(
        !output.status.success(),
        "ambient route_params must not be accepted by profile context"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown symbol `route_params`"),
        "stderr should reject route_params: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn serve_profile_alias_lists_server_routes() {
    let dir = temp_dir("serve-profile-routes");
    let source = dir.join("server.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
entry server @entry.http {
    route GET "/health" -> @flow.health
}

flow @flow.health health {
    return "ok"
}
"#,
    )
    .expect("write server profile source");
    fs::write(
        &manifest,
        r#"
[profiles."server.plan"]
kind = "server"
source = "server.arcw"
entry = "http"
adapter = "native-http"
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("serve")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("server.plan")
        .arg("--json")
        .output()
        .expect("arcw serve --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(
        output.status.success(),
        "serve profile should succeed, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"entry\": \"entry.http\"")
            && stdout.contains("\"adapter\": \"native-http\"")
            && stdout.contains("\"target\": \"flow.health\""),
        "serve profile JSON should list routes: {stdout}"
    );
}

#[test]
fn profile_source_and_path_are_mutually_exclusive() {
    let dir = temp_dir("profile-mutual-exclusion");
    let source = dir.join("main.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
flow @flow.main main {
    return "done"
}
"#,
    )
    .expect("write profile source");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "main.arcw"
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&source)
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .output()
        .expect("arcw check runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(
        !output.status.success(),
        "path plus --profile must be rejected"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("source path and --profile"),
        "stderr should explain mutually exclusive source selection"
    );
}

#[test]
fn profile_rejects_unknown_adapter() {
    let dir = temp_dir("profile-unknown-adapter");
    let source = dir.join("server.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
entry server @entry.http { run(@flow.main) }

flow @flow.main main {
    return "ok"
}
"#,
    )
    .expect("write server profile source");
    fs::write(
        &manifest,
        r#"
[profiles.bad]
kind = "server"
source = "server.arcw"
entry = "http"
adapter = "custom-http"
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("bad")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");
    assert!(!output.status.success(), "unknown adapter must fail");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unknown adapter `custom-http`"),
        "stderr should explain unknown adapter: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn cli_test_and_bench_profiles_use_profile_sources() {
    let dir = temp_dir("profile-cli-test-bench");
    let cli_source = dir.join("tool.arcw");
    let test_source = dir.join("opening_test.arcw");
    let bench_source = dir.join("opening_bench.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &cli_source,
        r"
entry cli @entry.main { run(@flow.main) }

flow @flow.main main(argc: i32) {
    return argc
}
",
    )
    .expect("write cli source");
    fs::write(
        &test_source,
        r#"
test @test.opening scenario {
    start(@flow.opening)
    expect.no_assertion_failures()
}

flow @flow.opening opening {
    return "done"
}
"#,
    )
    .expect("write test source");
    fs::write(
        &bench_source,
        r#"
bench @bench.opening {
    setup { let state = fixture<GameState>("opening.json") }
    measure iterations = 1 { opening_choices() }
}
"#,
    )
    .expect("write bench source");
    fs::write(
        &manifest,
        r#"
[profiles."cli.main"]
kind = "cli"
source = "tool.arcw"
entry = "main"

[profiles."test.opening"]
kind = "test"
source = "opening_test.arcw"

[profiles."bench.opening"]
kind = "bench"
source = "opening_bench.arcw"
"#,
    )
    .expect("write launch manifest");

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

fn temp_arcw(name: &str, source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("arcweft-cli-{name}-{}.arcw", std::process::id()));
    fs::write(&path, source).expect("write temp arcw fixture");
    path
}

fn temp_file(name: &str, extension: &str, source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    let suffix = if extension.is_empty() {
        String::new()
    } else {
        format!(".{extension}")
    };
    path.push(format!(
        "arcweft-cli-{name}-{}{}",
        std::process::id(),
        suffix
    ));
    fs::write(&path, source).expect("write temp fixture");
    path
}

fn temp_dir(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("arcweft-cli-{name}-{}", std::process::id()));
    if path.exists() {
        fs::remove_dir_all(&path).expect("remove stale temp fixture dir");
    }
    fs::create_dir_all(&path).expect("create temp fixture dir");
    path
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
