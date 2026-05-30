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
    assert_jit_check_json(&stdout, "score", "builtin", &["base", "bonus"], 7);
}

#[test]
fn toolchain_profile_json_plans_path_free_workspace_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("toolchain-profile")
        .arg("--command")
        .arg("fmt")
        .arg("--command")
        .arg("check")
        .arg("--command")
        .arg("clippy")
        .arg("--command")
        .arg("test")
        .arg("--dry-run")
        .arg("--json")
        .output()
        .expect("arcw toolchain-profile dry-run runs");

    assert!(
        output.status.success(),
        "toolchain profile dry-run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::current_dir().unwrap().display().to_string()),
        "toolchain profile JSON must not record absolute workspace paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("toolchain profile output is structured JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["commands"].as_array().unwrap().len(), 4);
    assert_eq!(json["commands"][0]["status"], "planned");
    assert_eq!(
        json["commands"][0]["argv"],
        serde_json::json!(["cargo", "fmt", "--all", "--check"])
    );
    assert_eq!(
        json["commands"][2]["argv"],
        serde_json::json!([
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features"
        ])
    );
}

#[test]
fn jit_check_json_can_compare_julia_baseline_without_absolute_source() {
    if !julia_is_available() {
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--json")
        .arg("--julia")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("7")
        .output()
        .expect("arcw jit check runs with Julia baseline");

    assert!(
        output.status.success(),
        "jit check with Julia baseline should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(&stdout, "score", "builtin", &["base", "bonus"], 7);
    assert_julia_baseline_json(&stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "jit check Julia JSON must not record absolute temp paths: {stdout}"
    );
}

#[test]
fn jit_check_json_measures_branch_mix_case_with_julia_baseline() {
    if !julia_is_available() {
        return;
    }
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--case")
        .arg("branch-mix")
        .arg("--json")
        .arg("--julia")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("11")
        .output()
        .expect("arcw jit check branch-mix runs with Julia baseline");

    assert!(
        output.status.success(),
        "branch-mix jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(
        &stdout,
        "branch_mix",
        "builtin",
        &["base", "bonus", "scale", "offset"],
        11,
    );
    assert_julia_baseline_json(&stdout);
}

#[test]
fn jit_check_json_measures_four_input_mix_case() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--case")
        .arg("four-input-mix")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("13")
        .output()
        .expect("arcw jit check four-input-mix runs");

    assert!(
        output.status.success(),
        "four-input-mix jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(
        &stdout,
        "four_input_mix",
        "builtin",
        &["a", "b", "c", "d"],
        13,
    );
}

#[test]
fn jit_check_json_measures_accumulation_mix_case() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--case")
        .arg("accumulation-mix")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("19")
        .output()
        .expect("arcw jit check accumulation-mix runs");

    assert!(
        output.status.success(),
        "accumulation-mix jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(
        &stdout,
        "accumulation_mix",
        "builtin",
        &["a", "b", "c", "d"],
        19,
    );
}

#[test]
fn jit_check_json_measures_let_chain_case() {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("jit")
        .arg("check")
        .arg("--case")
        .arg("let-chain")
        .arg("--json")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("17")
        .output()
        .expect("arcw jit check let-chain runs");

    assert!(
        output.status.success(),
        "let-chain jit check should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_jit_check_json(&stdout, "let_chain", "builtin", &["a", "b", "c"], 17);
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
    let adjusted = -weighted / scale
    return if base >= 3 { adjusted + scale } else { scale }
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
    assert_jit_check_json(&stdout, "score", "source", &["base", "bonus", "scale"], 3);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "jit check JSON must not record absolute temp paths: {stdout}"
    );
}

#[test]
fn run_json_uses_jit_for_runtime_pure_calls_without_arg_vec_allocation() {
    let path = temp_arcw(
        "runtime-pure-jit",
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2)
}

flow @flow.main main {
    return score(3, 4)
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("5")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--pure-workers")
        .arg("1")
        .arg("--pure-batch-min-len")
        .arg("2")
        .output()
        .expect("arcw run executes runtime pure JIT source");
    fs::remove_file(&path).expect("remove temp runtime pure helper");

    assert!(
        output.status.success(),
        "runtime pure JIT run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(json["final_status"], "done Return(\"18\")");
    let pure = &json["steps"][0]["stats"]["pure"];
    assert_eq!(pure["pure_calls"], 1);
    assert_eq!(pure["jit_calls"], 1);
    assert_eq!(pure["arg_stack_packs"], 1);
    assert_eq!(pure["arg_vec_allocations"], 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(json["executor_stats"]["pure_config"]["workers"]["fixed"], 1);
    assert_eq!(json["executor_stats"]["pure_config"]["resolved_workers"], 1);
    assert_eq!(
        json["executor_stats"]["pure_config"]["worker_pool_active"],
        false
    );
    assert_eq!(json["executor_stats"]["pure_config"]["batch_min_len"], 2);
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
}

#[test]
fn run_json_uses_jit_for_for_loop_pure_calls_without_arg_vec_allocation() {
    let path = temp_arcw(
        "runtime-for-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.for_pure for_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let scored = score(item, 2i64)
        log.info(scored)
    }
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("32")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw run executes for-loop pure calls");

    assert!(
        output.status.success(),
        "runtime for-loop pure JIT run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(json["final_status"], "done Return(\"done\")");
    let pure_calls = sum_step_pure_counter(&json, "pure_calls");
    assert_eq!(pure_calls, 4);
    assert_eq!(sum_step_pure_counter(&json, "jit_calls"), 4);
    assert_eq!(sum_step_pure_counter(&json, "arg_vec_allocations"), 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(
        json["executor_stats"]["pure_config"]["worker_pool_active"],
        false
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
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
            && stdout.contains("\"typecheck\"")
            && stdout.contains("\"borrow_check\"")
            && stdout.contains("\"phases\"")
            && stdout.contains("\"name\": \"parse\"")
            && stdout.contains("\"name\": \"typecheck\"")
            && stdout.contains("\"name\": \"line_task_lower\"")
            && stdout.contains("\"name\": \"verify\"")
            && stdout.contains("\"elapsed_ns\"")
            && stdout.contains("\"judgments\"")
            && stdout.contains("\"boundary_checks\"")
            && stdout.contains("\"verifier_obligations\""),
        "check JSON should include compiler timing and counter summary: {stdout}"
    );
    assert_check_json_pipeline_summary(&stdout);
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
            && stdout.contains("done")
            && stdout.contains("\"executor\": \"bytecode_vm\""),
        "run JSON should include bytecode executor, step events, and final status: {stdout}"
    );
}

#[test]
fn run_json_can_select_aot_executor() {
    let path = temp_arcw(
        "runtime-run-aot",
        r#"
flow @flow.run run {
    log.info("aot")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--executor")
        .arg("aot")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run runs with AOT executor");

    assert!(
        output.status.success(),
        "AOT run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"executor\": \"aot\"")
            && stdout.contains("\"executor_stats\"")
            && stdout.contains("\"aot_fast_path_ops\": 2")
            && stdout.contains("\"final_status\": \"done")
            && stdout.contains("return done"),
        "run JSON should report AOT executor, fast-path ops, and preserve VM-equivalent semantics: {stdout}"
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
fn profile_json_can_select_aot_executor_without_absolute_source() {
    let path = temp_arcw(
        "profile-aot",
        r#"
flow @flow.profile profile {
    log.info("profile aot")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("profile")
        .arg(&path)
        .arg("--executor")
        .arg("aot")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw profile runs with AOT executor");

    assert!(
        output.status.success(),
        "AOT profile should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"executor\": \"aot\"")
            && stdout.contains("\"aot_fast_path_ops\": 2")
            && stdout.contains("\"name\": \"run\"")
            && stdout.contains("\"source\": \"arcweft-cli-profile-aot-"),
        "profile JSON should include AOT executor, fast-path stats, and relative source label: {stdout}"
    );
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "profile JSON must not record absolute temp paths: {stdout}"
    );
}

#[test]
fn verify_types_json_reports_type_and_runtime_validation_without_absolute_source() {
    let path = temp_arcw(
        "verify-types",
        r#"
flow @flow.verify_types verify_types {
    log.info("verify types")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("verify-types")
        .arg(&path)
        .arg("--run")
        .arg("--executor")
        .arg("aot")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw verify-types runs");

    assert!(
        output.status.success(),
        "verify-types should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_verify_types_json_summary(&stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "verify-types JSON must not record absolute temp paths: {stdout}"
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
            && stdout.contains("\"name\": \"aot_lower\"")
            && stdout.contains("\"name\": \"bytecode_lower\"")
            && stdout.contains("\"name\": \"run\"")
            && stdout.contains("\"compiler\"")
            && stdout.contains("\"typecheck\"")
            && stdout.contains("\"borrow_check\"")
            && stdout.contains("\"runtime_type_validation\"")
            && stdout.contains("\"bytecode\"")
            && stdout.contains("\"aot\"")
            && stdout.contains("\"instructions\"")
            && stdout.contains("\"linear_dispatch_flows\"")
            && stdout.contains("\"mixed_dispatch_flows\"")
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
    assert_profile_json_summary(&stdout);
}

#[test]
fn profile_json_runs_native_file_tasks_without_absolute_source() {
    let dir = temp_dir("profile-native-file-task");
    let source_path = dir.join("profile.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("input.txt"), "profile-native-ok").expect("seed virtual input");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }
flow @flow.profile_io profile_io effects { fs.read(save), fs.write(save) } {
    let text = try await fs.read_text(path.save("input.txt")) with { error e => return "read_failed" }
    try await fs.write_text(path.save("output.txt"), text) with { error e => return "write_failed" }
    return text
}
"#,
    )
    .expect("write native profile fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("profile")
        .arg(&source_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("5")
        .arg("--json")
        .output()
        .expect("arcw profile runs native file task");

    assert!(
        output.status.success(),
        "native file task profile should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"task_events_in\": 1")
            && stdout.contains("\"source\": \"profile.arcw\"")
            && stdout.contains("\"native_io\"")
            && stdout.contains("\"completed_tasks\": 2")
            && stdout.contains("\"failed_tasks\": 0")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 2")
            && stdout.contains("\"dispatched\": 2")
            && stdout.contains("\"max_in_flight\": 1")
            && stdout.contains("\"read_ops\": 1")
            && stdout.contains("\"write_ops\": 1")
            && stdout.contains("\"bytes_read\": 17")
            && stdout.contains("\"bytes_written\": 17"),
        "profile JSON should include native task event and I/O counters without absolute source: {stdout}"
    );
    assert!(
        !stdout.contains(&dir.display().to_string()),
        "profile JSON must not record absolute temp paths: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(save_dir.join("output.txt")).expect("read virtual output"),
        "profile-native-ok"
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
        stdout.contains("\"executor\": \"bytecode_vm\"")
            && stdout.contains("\"final_status\": \"done")
            && stdout.contains("return 2"),
        "cli entry should run through bytecode VM and bind argc from trailing args: {stdout}"
    );
}

#[test]
fn run_json_executes_native_file_tasks_through_bytecode_vm() {
    let dir = temp_dir("native-file-task");
    let source_path = dir.join("main.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("input.txt"), "native-ok").expect("seed virtual input");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { run(@flow.main) }
flow @flow.main main effects { fs.read(save), fs.write(save) } {
    let text = try await fs.read_text(path.save("input.txt")) with { error e => return "read_failed" }
    try await fs.write_text(path.save("output.txt"), text) with { error e => return "write_failed" }
    return text
}
"#,
    )
    .expect("write native file task fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&source_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run executes native file tasks");

    assert!(
        output.status.success(),
        "native file task run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"executor\": \"bytecode_vm\"")
            && stdout.contains("file.read_text save:input.txt")
            && stdout.contains("file.write_text save:output.txt")
            && stdout.contains("task_events_in\": 1")
            && stdout.contains("\"native_io\"")
            && stdout.contains("\"completed_tasks\": 2")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 2")
            && stdout.contains("\"dispatched\": 2")
            && stdout.contains("\"read_ops\": 1")
            && stdout.contains("\"write_ops\": 1")
            && stdout.contains("\"bytes_read\": 9")
            && stdout.contains("\"bytes_written\": 9")
            && stdout.contains("return native-ok"),
        "run JSON should show native file task completion and I/O counters through bytecode VM: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(save_dir.join("output.txt")).expect("read virtual output"),
        "native-ok"
    );
}

#[test]
fn run_json_executes_traverse_parallel_file_tasks() {
    let dir = temp_dir("native-traverse-parallel");
    let source_path = dir.join("main.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("a.txt"), "A").expect("seed input a");
    fs::write(save_dir.join("b.txt"), "B").expect("seed input b");
    fs::write(save_dir.join("c.txt"), "C").expect("seed input c");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
}
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { run(@flow.main) }
flow @flow.main main effects { fs.read(save) } {
    let paths = [path.save("a.txt"), path.save("b.txt"), path.save("c.txt")]
    let values = try await paths.traverse(fs.read_text).parallel(limit = 2) with { error e => return "read_failed" }
    log.info("parallel done")
    return "done"
}
"#,
    )
    .expect("write traverse parallel task fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&source_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("12")
        .arg("--json")
        .output()
        .expect("arcw run executes traverse parallel file tasks");

    assert!(
        output.status.success(),
        "traverse parallel run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("file.read_text save:a.txt")
            && stdout.contains("file.read_text save:b.txt")
            && stdout.contains("file.read_text save:c.txt")
            && stdout.contains("\"completed_tasks\": 3")
            && stdout.contains("\"submitted\": 3")
            && stdout.contains("\"dispatched\": 3")
            && stdout.contains("\"max_in_flight\": 2")
            && stdout.contains("\"read_ops\": 3")
            && stdout.contains("\"parallel_batches\": 1")
            && stdout.contains("\"parallel_tasks\": 2")
            && stdout.contains("return done"),
        "run JSON should show bounded traverse fanout and native reads: {stdout}"
    );
}

#[test]
fn run_json_reports_runtime_system_info_tasks() {
    let path = temp_arcw(
        "system-info-task",
        r#"
extern capability system {
    type SystemError
    fn core_count() -> Need<String, SystemError> effects { system.read }
    fn thread_count() -> Need<String, SystemError> effects { system.read }
    fn available_parallelism() -> Need<String, SystemError> effects { system.read }
}
entry cli @entry.main { run(@flow.main) }
flow @flow.main main effects { system.read } {
    let cores = try await system.core_count() with { error e => return "core_failed" }
    let threads = try await system.thread_count() with { error e => return "thread_failed" }
    let available = try await system.available_parallelism() with { error e => return "available_failed" }
    log.info(threads)
    log.info(available)
    return cores
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run executes system info tasks");

    assert!(
        output.status.success(),
        "system info task run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("system.core_count")
            && stdout.contains("system.thread_count")
            && stdout.contains("system.available_parallelism")
            && stdout.contains("\"completed_tasks\": 3")
            && stdout.contains("\"system_info_ops\": 3")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 3")
            && stdout.contains("\"in_flight\": 0"),
        "run JSON should show runtime system info task completion: {stdout}"
    );
}

#[test]
fn run_json_observes_line_child_tasks_through_scheduler() {
    let path = temp_arcw(
        "line-child-scheduler",
        r#"
pub surface character @character.alice Alice as alice {
}

flow @flow.main main {
    alice[待って。[p]]
    with:
        thread motion:
            log.info("motion")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("one-op")
        .arg("--steps")
        .arg("3")
        .arg("--json")
        .output()
        .expect("arcw run observes line child task");

    assert!(
        output.status.success(),
        "line child task run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("line_task.run_child")
            && stdout.contains("\"native_io\"")
            && stdout.contains("\"completed_tasks\": 1")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 1")
            && stdout.contains("\"dispatched\": 1")
            && stdout.contains("\"in_flight\": 0"),
        "run JSON should observe line child task dispatch and completion: {stdout}"
    );
}

#[test]
fn run_json_executes_source_thread_through_scheduler_marker() {
    let path = temp_arcw(
        "source-thread-scheduler",
        r#"
flow @flow.main main {
    thread worker {
        log.info("worker")
    }
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--mode")
        .arg("one-op")
        .arg("--steps")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run executes source thread marker");

    assert!(
        output.status.success(),
        "source thread run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("flow_thread.run_child")
            && stdout.contains("\"message\": \"worker\"")
            && stdout.contains("return done")
            && stdout.contains("\"child_fibers\": 1")
            && stdout.contains("\"completed_tasks\": 1")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 1")
            && stdout.contains("\"in_flight\": 0"),
        "run JSON should execute source thread body and observe scheduler marker: {stdout}"
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
            && stdout.contains("\"executed_ops_median\": 2")
            && stdout.contains("\"compiler\"")
            && stdout.contains("\"phases\"")
            && stdout.contains("\"name\": \"typecheck\"")
            && stdout.contains("\"borrow_check\"")
            && stdout.contains("\"boundary_checks\""),
        "bench JSON should include headless measurement and compiler profiling: {stdout}"
    );
}

#[test]
fn bench_json_can_measure_aot_executor_sections() {
    let path = temp_arcw(
        "script-bench-aot",
        r#"
bench @bench.runtime_aot {
    measure iterations = 1 { start(@flow.bench) }
}

flow @flow.bench bench {
    log.info("bench aot")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--executor")
        .arg("aot")
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw bench runs with AOT executor");

    assert!(
        output.status.success(),
        "AOT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"measured\"")
            && stdout.contains("\"executor\": \"aot\"")
            && stdout.contains("\"aot_fast_path_ops\": 2")
            && stdout.contains("\"executed_ops_median\": 2"),
        "bench JSON should include AOT executor measurement and fast-path stats: {stdout}"
    );
}

#[test]
fn bench_json_measures_for_loop_pure_jit_without_arg_vec_allocation() {
    let path = temp_arcw(
        "script-bench-for-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.for_pure {
    measure iterations = 2 { start(@flow.for_pure) }
}

flow @flow.for_pure for_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let scored = score(item, 2i64)
        log.info(scored)
    }
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
        .arg("32")
        .arg("--max-ops")
        .arg("8")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures for-loop pure JIT calls");

    assert!(
        output.status.success(),
        "for-loop pure JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let section = &json["benches"][0]["sections"][0];
    assert_eq!(section["status"], "measured");

    let measurement = &section["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert_eq!(measurement["warmup"], 1);
    assert_eq!(measurement["iterations"], 2);
    assert_eq!(measurement["steps"], 32);
    assert!(
        measurement["per_executed_op_ns"].as_u64().is_some(),
        "bench JSON should expose median per-op cost: {measurement}"
    );
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["executor_stats"]["pure_config"]["backend"],
        "jit"
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
    assert!(
        measurement["executor_stats"]["pure_config"]["resolved_workers"]
            .as_u64()
            .is_some_and(|value| value >= 1),
        "bench JSON should expose resolved pure worker count: {measurement}"
    );
}

#[test]
fn bench_json_measures_branching_for_loop_pure_jit_characteristics() {
    let path = temp_arcw(
        "script-bench-branching-for-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64, scale: i64) -> i64 {
    let weighted = base * (bonus + 2i64)
    return if base >= 3i64 { weighted + scale } else { scale - weighted }
}

bench @bench.branch_for_pure {
    measure iterations = 2 { start(@flow.branch_for_pure) }
}

flow @flow.branch_for_pure branch_for_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let first = score(item, 2i64, 5i64)
        let second = score(first, 1i64, 7i64)
        log.info(second)
    }
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
        .arg("48")
        .arg("--max-ops")
        .arg("8")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures branching for-loop pure JIT calls");

    assert!(
        output.status.success(),
        "branching for-loop pure JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert!(
        measurement["per_executed_op_ns"].as_u64().is_some(),
        "bench JSON should expose median per-op cost: {measurement}"
    );
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 8);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_measures_runtime_match_to_pure_jit_characteristics() {
    let path = temp_arcw(
        "script-bench-match-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.match_pure {
    measure iterations = 2 { start(@flow.match_pure) }
}

flow @flow.match_pure match_pure {
    let next = @flow.scored
    match next {
        @flow.scored => goto @flow.scored
        _ => goto @flow.fallback
    }
}

flow @flow.scored scored {
    let scored = score(3i64, 2i64)
    log.info(scored)
    return "done"
}

flow @flow.fallback fallback {
    let scored = score(1i64, 1i64)
    log.info(scored)
    return "fallback"
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
        .arg("24")
        .arg("--max-ops")
        .arg("8")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures runtime match to pure JIT calls");

    assert!(
        output.status.success(),
        "runtime match pure JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert!(
        measurement["per_executed_op_ns"].as_u64().is_some(),
        "bench JSON should expose median per-op cost: {measurement}"
    );
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 7);
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 1);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_measures_pure_helper_with_vm_aot_and_jit() {
    let path = temp_arcw(
        "script-bench-pure-helper",
        r"
#[pure]
fn score(base: i64, bonus: i64, scale: i64) -> i64 {
    let boosted = bonus + 2
    let weighted = base * boosted
    return if base >= 3 { weighted + scale } else { scale }
}

bench @bench.pure_score {
    measure iterations = 4 { pure(score) }
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("2")
        .arg("--input-seed")
        .arg("5")
        .arg("--pure-backend")
        .arg("aot")
        .arg("--pure-workers")
        .arg("2")
        .arg("--pure-batch-min-len")
        .arg("2")
        .arg("--json")
        .output()
        .expect("arcw bench runs pure helper measurement");

    assert!(
        output.status.success(),
        "pure helper bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "pure helper bench JSON must not record absolute temp paths: {stdout}"
    );
    assert!(
        stdout.contains("bench.pure_score")
            && stdout.contains("\"pure_helper\"")
            && stdout.contains("\"helper\": \"score\"")
            && stdout.contains("\"matches_vm\": true")
            && stdout.contains("\"jit_elapsed_ns\"")
            && stdout.contains("\"vm_elapsed_ns\"")
            && stdout.contains("\"speedup_x\"")
            && stdout.contains("\"jit_batch\"")
            && stdout.contains("\"runtime_batch\"")
            && stdout.contains("\"jit_accumulator\""),
        "bench JSON should include VM/AOT/JIT pure helper timing: {stdout}"
    );
    assert_pure_helper_bench_json(&stdout);
}

fn assert_pure_helper_bench_json(stdout: &str) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("bench output is structured JSON");
    let pure_helper = &json["benches"][0]["sections"][0]["pure_helper"];
    assert_eq!(pure_helper["helper"], "score");
    assert_eq!(pure_helper["matches_vm"], true);
    assert_eq!(pure_helper["warmup"], 1);
    assert_eq!(pure_helper["iterations"], 4);
    assert_eq!(pure_helper["samples"], 2);
    assert_eq!(
        pure_helper["deterministic"]["vm_accumulator"],
        pure_helper["deterministic"]["aot_accumulator"]
    );
    assert_eq!(
        pure_helper["deterministic"]["vm_accumulator"],
        pure_helper["deterministic"]["jit_accumulator"]
    );
    assert_eq!(
        pure_helper["deterministic"]["vm_accumulator"],
        pure_helper["deterministic"]["jit_batch_accumulator"]
    );

    let timings = &pure_helper["timings"];
    assert_eq!(
        timings["vm_elapsed_ns"], timings["vm_samples"]["median"],
        "VM median should be exposed both as the top-level elapsed value and in the sample summary"
    );
    assert_eq!(timings["aot_elapsed_ns"], timings["aot_samples"]["median"]);
    assert_eq!(timings["jit_elapsed_ns"], timings["jit_samples"]["median"]);
    assert_sample_summary_is_ordered(&timings["vm_samples"]);
    assert_sample_summary_is_ordered(&timings["aot_samples"]);
    assert_sample_summary_is_ordered(&timings["jit_samples"]);
    assert!(
        timings["vm_per_iteration_ns"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && timings["aot_per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && timings["jit_per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0),
        "per-iteration timings should be positive: {timings}"
    );
    assert!(
        timings["speedup_x"]
            .as_str()
            .and_then(|value| value.parse::<f64>().ok())
            .is_some(),
        "JIT speedup should be numeric: {timings}"
    );
    assert!(
        timings["aot_speedup_x"]
            .as_str()
            .and_then(|value| value.parse::<f64>().ok())
            .is_some(),
        "AOT speedup should be numeric: {timings}"
    );
    let jit_batch = &pure_helper["jit_batch"];
    assert_eq!(
        jit_batch["elapsed_ns"], jit_batch["samples"]["median"],
        "JIT batch median should be exposed both as elapsed and in the sample summary"
    );
    assert_sample_summary_is_ordered(&jit_batch["samples"]);
    assert!(
        jit_batch["compile_elapsed_ns"].as_u64().is_some()
            && jit_batch["per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && jit_batch["speedup_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some()
            && jit_batch["jit_call_speedup_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some(),
        "pure helper bench should expose batch loop counters: {jit_batch}"
    );
    assert_runtime_pure_batch_json(pure_helper);
    assert_eq!(
        pure_helper["vm_stats"]["evaluated_binary_ops"],
        pure_helper["aot_stats"]["evaluated_binary_ops"]
    );
    let vm_binary_ops = pure_helper["vm_stats"]["evaluated_binary_ops"]
        .as_u64()
        .expect("VM binary op count is numeric");
    let jit_binary_ops = pure_helper["jit_stats"]["evaluated_binary_ops"]
        .as_u64()
        .expect("JIT binary op count is numeric");
    assert!(
        jit_binary_ops >= vm_binary_ops && vm_binary_ops > 0,
        "JIT compile stats cover the full expression while VM/AOT runtime stats cover the exercised branch: {pure_helper}"
    );
    assert!(
        pure_helper["vm_stats"]["evaluated_exprs"]
            .as_u64()
            .is_some_and(|value| value > 0)
    );
}

fn assert_runtime_pure_batch_json(pure_helper: &serde_json::Value) {
    let runtime_batch = &pure_helper["runtime_batch"];
    assert_eq!(runtime_batch["matches_vm"], true);
    assert_eq!(
        runtime_batch["accumulator"],
        pure_helper["deterministic"]["vm_accumulator"]
    );
    assert_eq!(runtime_batch["config"]["backend"], "aot");
    assert_eq!(runtime_batch["config"]["workers"]["fixed"], 2);
    assert_eq!(runtime_batch["config"]["resolved_workers"], 2);
    assert_eq!(runtime_batch["config"]["worker_pool_active"], true);
    assert_eq!(runtime_batch["config"]["batch_min_len"], 2);
    assert_eq!(runtime_batch["compile"]["aot_successes"], 1);
    assert_eq!(runtime_batch["stats"]["batch_calls"], 2);
    assert_eq!(runtime_batch["stats"]["batch_items"], 8);
    assert_eq!(runtime_batch["stats"]["pure_calls"], 8);
    assert_eq!(runtime_batch["stats"]["aot_calls"], 8);
    assert_eq!(runtime_batch["stats"]["arg_vec_allocations"], 0);
    assert!(
        runtime_batch["stats"]["thread_pool_jobs"]
            .as_u64()
            .is_some_and(|jobs| jobs >= 2),
        "runtime pure batch should use the configured worker pool: {runtime_batch}"
    );
}

#[test]
fn bench_json_checks_runtime_assert_sections() {
    let path = temp_arcw(
        "script-bench-assert",
        r#"
signal @signal:.bench_done: Watch<Bool>

bench @bench.runtime_assert {
    measure iterations = 1 { start(@flow.bench) }
    assert { expect.log(.info, contains="bench observed") }
    assert { expect.signal(@signal.bench_done, true) }
    assert { expect.no_assertion_failures() }
}

flow @flow.bench bench effects { signal.write } {
    log.info("bench observed")
    signal.set(@signal.bench_done, true)
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("4")
        .arg("--json")
        .output()
        .expect("arcw bench runs runtime assertions");

    assert!(
        output.status.success(),
        "bench assertions should pass, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("bench.runtime_assert") && stdout.contains("\"status\": \"measured\""),
        "bench JSON should keep measured status when assertions pass: {stdout}"
    );
}

#[test]
fn bench_json_fails_runtime_assert_sections() {
    let path = temp_arcw(
        "script-bench-assert-fail",
        r#"
signal @signal:.bench_done: Watch<Bool>

bench @bench.runtime_assert_fail {
    measure iterations = 1 { start(@flow.bench) }
    assert { expect.signal(@signal.bench_done, false) }
}

flow @flow.bench bench effects { signal.write } {
    signal.set(@signal.bench_done, true)
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("4")
        .arg("--json")
        .output()
        .expect("arcw bench runs failing runtime assertions");

    assert!(
        !output.status.success(),
        "failing bench assertions should fail the command"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"failed\"")
            && stdout.contains("bench assert failed")
            && stdout.contains("expected signal @signal.bench_done == false"),
        "bench JSON should report assertion diagnostics: {stdout}"
    );
}

#[test]
fn bench_json_measures_native_file_tasks() {
    let dir = temp_dir("bench-native-file-task");
    let source_path = dir.join("bench.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("input.txt"), "bench-native-ok").expect("seed virtual input");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }

bench @bench.native_io {
    measure iterations = 1 { start(@flow.bench_io) }
    assert { expect.file(path.save("output.txt"), equals="bench-native-ok") }
}

flow @flow.bench_io bench_io effects { fs.read(save), fs.write(save) } {
    let text = try await fs.read_text(path.save("input.txt")) with { error e => return "read_failed" }
    try await fs.write_text(path.save("output.txt"), text) with { error e => return "write_failed" }
    return text
}
"#,
    )
    .expect("write native bench fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&source_path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("5")
        .arg("--json")
        .output()
        .expect("arcw bench runs native file task");

    assert!(
        output.status.success(),
        "native file task bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"measured\"")
            && stdout.contains("\"task_requests_median\": 2")
            && stdout.contains("\"task_events_in_median\": 2")
            && stdout.contains("\"native_io\"")
            && stdout.contains("\"completed_tasks\": 2")
            && stdout.contains("\"failed_tasks\": 0")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 2")
            && stdout.contains("\"dispatched\": 2")
            && stdout.contains("\"read_ops\": 1")
            && stdout.contains("\"write_ops\": 1")
            && stdout.contains("\"bytes_read\": 15")
            && stdout.contains("\"bytes_written\": 15"),
        "bench JSON should include native task request/event and I/O counters: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(save_dir.join("output.txt")).expect("read virtual output"),
        "bench-native-ok"
    );
}

#[test]
fn bench_json_measures_traverse_parallel_file_tasks() {
    let dir = temp_dir("bench-traverse-parallel");
    let source_path = dir.join("bench.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("a.txt"), "A").expect("seed input a");
    fs::write(save_dir.join("b.txt"), "B").expect("seed input b");
    fs::write(save_dir.join("c.txt"), "C").expect("seed input c");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }

bench @bench.parallel_io {
    measure iterations = 1 { start(@flow.parallel_io) }
    assert { expect.file(path.save("output.txt"), equals="done") }
}

flow @flow.parallel_io parallel_io effects { fs.read(save), fs.write(save) } {
    let paths = [path.save("a.txt"), path.save("b.txt"), path.save("c.txt")]
    let values = try await paths.traverse(fs.read_text).parallel(limit = 2) with { error e => return "read_failed" }
    try await fs.write_text(path.save("output.txt"), "done") with { error e => return "write_failed" }
    return "done"
}
"#,
    )
    .expect("write traverse parallel bench fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&source_path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("12")
        .arg("--json")
        .output()
        .expect("arcw bench runs traverse parallel file tasks");

    assert!(
        output.status.success(),
        "traverse parallel bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"measured\"")
            && stdout.contains("\"task_requests_median\": 4")
            && stdout.contains("\"task_events_in_median\": 4")
            && stdout.contains("\"completed_tasks\": 4")
            && stdout.contains("\"submitted\": 4")
            && stdout.contains("\"dispatched\": 4")
            && stdout.contains("\"max_in_flight\": 2")
            && stdout.contains("\"read_ops\": 3")
            && stdout.contains("\"parallel_batches\": 1")
            && stdout.contains("\"parallel_tasks\": 2")
            && stdout.contains("\"write_ops\": 1"),
        "bench JSON should expose bounded traverse fanout counters: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(save_dir.join("output.txt")).expect("read virtual output"),
        "done"
    );
}

#[test]
fn bench_json_fails_native_file_assertions() {
    let dir = temp_dir("bench-native-file-assert-fail");
    let source_path = dir.join("bench.arcw");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }

bench @bench.native_file_assert {
    measure iterations = 1 { start(@flow.write_actual) }
    assert { expect.file(path.save("output.txt"), equals="expected") }
}

flow @flow.write_actual write_actual effects { fs.write(save) } {
    try await fs.write_text(path.save("output.txt"), "actual") with { error e => return "write_failed" }
    return "done"
}
"#,
    )
    .expect("write failing native file assertion fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&source_path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("4")
        .arg("--json")
        .output()
        .expect("arcw bench runs failing native file assertion");

    assert!(
        !output.status.success(),
        "native file assertion mismatch should fail the command"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"status\": \"failed\"")
            && stdout.contains("bench assert failed")
            && stdout.contains("expected file save:output.txt == `expected`, found `actual`"),
        "bench JSON should report native file assertion mismatch without host paths: {stdout}"
    );
    assert!(
        !stdout.contains(&dir.display().to_string()),
        "bench JSON must not record absolute temp paths: {stdout}"
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

fn assert_sample_summary_is_ordered(samples: &serde_json::Value) {
    let min = samples["min"].as_u64().expect("sample min is an integer");
    let median = samples["median"]
        .as_u64()
        .expect("sample median is an integer");
    let max = samples["max"].as_u64().expect("sample max is an integer");
    assert!(
        min <= median && median <= max,
        "sample summary should satisfy min <= median <= max: {samples}"
    );
}

fn assert_check_json_pipeline_summary(stdout: &str) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("check output is structured JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["flows"], 1);
    assert!(
        json["line_task_groups"].as_u64().is_some(),
        "line task group count should be numeric: {json}"
    );
    assert_phase_timings_include(
        &json["phases"],
        &[
            "read_source",
            "parse",
            "lint",
            "lower_hir",
            "resolve",
            "readiness",
            "typecheck",
            "line_task_lower",
            "verify",
        ],
    );

    assert_typecheck_metrics(&json["typecheck"]);
    assert_borrow_check_metrics(&json["borrow_check"]);
}

fn assert_jit_check_json(
    stdout: &str,
    expected_helper: &str,
    expected_helper_source: &str,
    expected_input_bindings: &[&str],
    expected_seed: u64,
) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("jit check output is structured JSON");
    assert_eq!(json["status"], "ok");
    assert_eq!(json["helper"], expected_helper);
    assert_eq!(json["helper_source"], expected_helper_source);
    assert_eq!(json["workload"]["case"], expected_helper);
    assert_eq!(json["workload"]["loop_kind"], "deterministic_input_series");
    assert_eq!(
        json["workload"]["inputs_per_iteration"],
        expected_input_bindings.len()
    );
    assert_eq!(json["vm_backend"], "vm");
    assert_eq!(json["aot_backend"], "aot");
    assert_eq!(json["jit_backend"], "jit");
    assert_eq!(json["matches_vm"], true);
    assert_eq!(json["dynamic_inputs"], true);
    assert_eq!(json["input_seed"], expected_seed);
    assert_eq!(json["warmup"], 1);
    assert_eq!(json["iterations"], 4);
    assert_eq!(json["samples"], 2);
    assert_eq!(
        json["input_bindings"]
            .as_array()
            .expect("input bindings should be an array")
            .iter()
            .map(|binding| binding.as_str().expect("input binding should be text"))
            .collect::<Vec<_>>(),
        expected_input_bindings
    );
    assert_eq!(json["vm_value"], json["aot_value"]);
    assert_eq!(json["vm_value"], json["jit_value"]);
    assert_eq!(
        json["deterministic"]["vm_accumulator"],
        json["deterministic"]["aot_accumulator"]
    );
    assert_eq!(
        json["deterministic"]["vm_accumulator"],
        json["deterministic"]["jit_accumulator"]
    );
    assert_eq!(
        json["deterministic"]["vm_accumulator"],
        json["deterministic"]["jit_batch_accumulator"]
    );
    assert_jit_timing_summary(&json["timings"]);
    assert_jit_batch_json(&json["jit_batch"]);
    assert_eq!(
        json["vm_stats"]["evaluated_binary_ops"],
        json["aot_stats"]["evaluated_binary_ops"]
    );
    assert!(
        json["vm_stats"]["evaluated_exprs"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && json["jit_stats"]["evaluated_exprs"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && json["jit_stats"]["evaluated_binary_ops"].as_u64()
                >= json["vm_stats"]["evaluated_binary_ops"].as_u64(),
        "VM/AOT/JIT eval counters should be populated: {json}"
    );

    if expected_helper_source == "source" {
        let source_compiler = &json["source_compiler"];
        assert_typecheck_metrics(&source_compiler["typecheck"]);
        assert_borrow_check_metrics(&source_compiler["borrow_check"]);
        assert_phase_timings_include(
            &source_compiler["phases"],
            &[
                "read_source",
                "parse",
                "lint",
                "lower_hir",
                "resolve",
                "readiness",
                "typecheck",
                "line_task_lower",
            ],
        );
    } else {
        assert!(
            json.get("source_compiler").is_none(),
            "builtin helper should not report source compiler metrics: {json}"
        );
    }
}

fn sum_step_pure_counter(json: &serde_json::Value, key: &str) -> u64 {
    json["steps"]
        .as_array()
        .expect("run JSON steps should be an array")
        .iter()
        .map(|step| step["stats"]["pure"][key].as_u64().unwrap_or(0))
        .sum()
}

fn assert_jit_timing_summary(timings: &serde_json::Value) {
    assert_eq!(timings["aot_elapsed_ns"], timings["aot_samples"]["median"]);
    assert_eq!(timings["jit_elapsed_ns"], timings["jit_samples"]["median"]);
    assert_eq!(timings["vm_elapsed_ns"], timings["vm_samples"]["median"]);
    assert_sample_summary_is_ordered(&timings["aot_samples"]);
    assert_sample_summary_is_ordered(&timings["jit_samples"]);
    assert_sample_summary_is_ordered(&timings["vm_samples"]);
    assert!(
        timings["aot_compile_elapsed_ns"].as_u64().is_some()
            && timings["compile_elapsed_ns"].as_u64().is_some()
            && timings["aot_per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && timings["jit_per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && timings["vm_per_iteration_ns"]
                .as_u64()
                .is_some_and(|value| value > 0),
        "JIT timing counters should be populated: {timings}"
    );
    assert!(
        timings["aot_speedup_x"]
            .as_str()
            .and_then(|value| value.parse::<f64>().ok())
            .is_some()
            && timings["speedup_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some(),
        "JIT speedups should be numeric strings: {timings}"
    );
}

fn assert_jit_batch_json(batch: &serde_json::Value) {
    assert_eq!(batch["backend"], "jit_batch");
    assert_eq!(batch["matches_vm"], true);
    assert_sample_summary_is_ordered(&batch["samples"]);
    assert!(
        batch["compile_elapsed_ns"].as_u64().is_some()
            && batch["elapsed_ns"].as_u64().is_some()
            && batch["per_iteration_ns"].as_u64().is_some()
            && batch["speedup_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some()
            && batch["jit_call_speedup_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some(),
        "JIT batch should expose compile, elapsed, and speedup counters: {batch}"
    );
}

fn assert_julia_baseline_json(stdout: &str) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("jit check output is structured JSON");
    let julia = &json["julia"];
    assert_eq!(julia["backend"], "julia");
    assert_eq!(julia["matches_vm"], true);
    assert_eq!(
        julia["accumulator"],
        json["deterministic"]["vm_accumulator"]
    );
    assert!(
        julia["version"]
            .as_str()
            .is_some_and(|version| !version.is_empty())
            && julia["elapsed_ns"].as_u64().is_some()
            && julia["per_iteration_ns"].as_u64().is_some()
            && julia["jit_vs_julia_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some()
            && julia["julia_vs_jit_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some()
            && julia["jit_batch_vs_julia_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some()
            && julia["julia_vs_jit_batch_x"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some(),
        "Julia baseline should report version, timings, and speed ratios: {julia}"
    );
    assert_sample_summary_is_ordered(&julia["samples"]);
}

fn assert_verify_types_json_summary(stdout: &str) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("verify-types output is structured JSON");
    assert_eq!(json["status"], "ok");
    assert!(
        json["source"]
            .as_str()
            .is_some_and(|source| source.starts_with("arcweft-cli-verify-types-")),
        "verify-types should report only a source label, not an absolute path: {json}"
    );
    assert!(
        json["line_task_groups"].as_u64().is_some(),
        "line task group count should be numeric: {json}"
    );
    assert_phase_timings_include(
        &json["phases"],
        &[
            "read_source",
            "parse",
            "lint",
            "lower_hir",
            "resolve",
            "readiness",
            "typecheck",
            "line_task_lower",
            "runtime_plan_lower",
            "runtime_type_validate",
            "verify",
            "run",
        ],
    );
    assert_typecheck_metrics(&json["typecheck"]);
    assert_borrow_check_metrics(&json["borrow_check"]);
    assert!(
        json["runtime_type_validation"]["diagnostics"]
            .as_u64()
            .is_some()
            && json["runtime_type_validation"]["errors"].as_u64().is_some()
            && json["runtime_type_validation"]["stats"]["flows"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && json["runtime_type_validation"]["stats"]["ops"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && json["runtime_type_validation"]["stats"]["type_judgments"]
                .as_u64()
                .is_some(),
        "runtime type validation counters should be populated: {json}"
    );
    assert!(
        json["verifier"]["diagnostics"].as_u64().is_some()
            && json["verifier"]["obligations"].as_u64().is_some()
            && json["verifier"]["unsafe_audits"].as_u64().is_some(),
        "verifier counters should be populated: {json}"
    );

    let runtime = &json["runtime"];
    assert_eq!(runtime["executor"], "aot");
    assert_eq!(runtime["failed"], false);
    assert_eq!(runtime["executor_stats"]["aot_fast_path_ops"], 2);
    assert!(
        runtime["steps_run"]
            .as_u64()
            .is_some_and(|value| value == 1)
            && runtime["steps"][0]["stats"]["executed_ops"]
                .as_u64()
                .is_some_and(|value| value == 2)
            && runtime["native_io"]["completed_tasks"].as_u64().is_some(),
        "verify-types runtime counters should be populated: {runtime}"
    );
}

fn assert_profile_json_summary(stdout: &str) {
    let json: serde_json::Value =
        serde_json::from_str(stdout).expect("profile output is structured JSON");
    assert!(
        json["source"]
            .as_str()
            .is_some_and(|source| source.starts_with("arcweft-cli-profile-json-")),
        "profile should report only a source label, not an absolute path: {json}"
    );
    assert_phase_timings_include(
        &json["phases"],
        &[
            "read_source",
            "parse",
            "lint",
            "lower_hir",
            "resolve",
            "readiness",
            "typecheck",
            "line_task_lower",
            "runtime_plan_lower",
            "runtime_type_validate",
            "aot_lower",
            "bytecode_lower",
            "run",
        ],
    );

    let compiler = &json["compiler"];
    assert_typecheck_metrics(&compiler["typecheck"]);
    assert_borrow_check_metrics(&compiler["borrow_check"]);
    assert!(
        compiler["runtime_type_validation"]["flows"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && compiler["runtime_type_validation"]["ops"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && compiler["runtime_type_validation"]["type_judgments"]
                .as_u64()
                .is_some(),
        "runtime type validation counters should be populated: {compiler}"
    );
    assert!(
        compiler["bytecode"]["flows"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && compiler["bytecode"]["instructions"]
                .as_u64()
                .is_some_and(|value| value > 0),
        "bytecode lowering counters should be populated: {compiler}"
    );
    assert!(
        compiler["aot"]["flows"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && compiler["aot"]["ops"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && compiler["aot"]["linear_dispatch_flows"].as_u64().is_some()
            && compiler["aot"]["mixed_dispatch_flows"].as_u64().is_some(),
        "AOT lowering counters should be populated: {compiler}"
    );

    let runtime = &json["runtime"];
    assert_eq!(runtime["executor"], "bytecode_vm");
    assert!(
        runtime["final_status"]
            .as_str()
            .is_some_and(|status| status.contains("Return")),
        "runtime should finish by returning from the profiled flow: {runtime}"
    );
    assert!(
        runtime["steps"]
            .as_array()
            .is_some_and(|steps| steps.len() == 1),
        "profile runtime should record one requested step: {runtime}"
    );
    assert!(
        runtime["steps"][0]["stats"]["executed_ops"]
            .as_u64()
            .is_some_and(|value| value == 2)
            && runtime["executor_stats"]["aot_fast_path_ops"]
                .as_u64()
                .is_some()
            && runtime["native_io"]["completed_tasks"].as_u64().is_some(),
        "runtime execution counters should be populated: {runtime}"
    );
}

fn assert_typecheck_metrics(typecheck: &serde_json::Value) {
    assert!(
        typecheck["expressions"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && typecheck["judgments"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && typecheck["judgment_rules"]["expr"]
                .as_u64()
                .is_some_and(|value| value > 0),
        "typecheck performance counters should be populated: {typecheck}"
    );
    assert!(
        typecheck["judgment_samples"]
            .as_array()
            .is_some_and(|samples| !samples.is_empty()),
        "typecheck should expose bounded judgment samples: {typecheck}"
    );
}

fn assert_borrow_check_metrics(borrow_check: &serde_json::Value) {
    for key in [
        "binding_groups",
        "bindings",
        "state_snapshots",
        "state_restores",
        "state_merges",
        "state_cloned_bindings",
        "boundary_checks",
        "escape_checks",
        "max_active_borrows",
    ] {
        assert!(
            borrow_check[key].as_u64().is_some(),
            "borrow-check counter `{key}` should be numeric: {borrow_check}"
        );
    }
}

fn assert_phase_timings_include(phases: &serde_json::Value, expected_names: &[&str]) {
    let phases = phases.as_array().expect("phases should be an array");
    for phase in phases {
        assert!(
            phase["name"].as_str().is_some_and(|name| !name.is_empty())
                && phase["elapsed_ns"].as_u64().is_some(),
            "each phase should include a name and elapsed_ns: {phase}"
        );
    }
    for expected in expected_names {
        assert!(
            phases
                .iter()
                .any(|phase| phase["name"].as_str() == Some(expected)),
            "missing compiler phase `{expected}` in {phases:?}"
        );
    }
}

fn julia_is_available() -> bool {
    Command::new("julia")
        .arg("--version")
        .output()
        .is_ok_and(|output| output.status.success())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
