#[test]
fn run_json_uses_jit_for_map_closure_pure_batch() {
    let path = temp_arcw(
        "runtime-map-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.map_pure map_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    for item in scores {
        log.info(item)
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
        .arg("32")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw run executes map-closure pure batch");

    assert!(
        output.status.success(),
        "runtime map pure JIT run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(json["final_status"], "done Return(\"done\")");
    assert_eq!(sum_step_pure_counter(&json, "pure_calls"), 4);
    assert_eq!(sum_step_pure_counter(&json, "batch_calls"), 1);
    assert_eq!(sum_step_pure_counter(&json, "batch_items"), 4);
    assert_eq!(sum_step_pure_counter(&json, "jit_calls"), 4);
    assert_eq!(sum_step_pure_counter(&json, "arg_stack_packs"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_vec_allocations"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_copied"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_borrowed"), 64);
    assert_eq!(sum_step_pure_counter(&json, "result_bytes_copied"), 32);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
}

#[test]
fn run_json_fuses_jit_map_closure_pure_sum() {
    let path = temp_arcw(
        "runtime-map-pure-jit-sum",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.map_pure_sum map_pure_sum {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let total: i64 = values.map(|item| score(item, 2i64)).sum()
    log.info(total)
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
        .arg("32")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw run executes fused map-closure pure sum");

    assert!(
        output.status.success(),
        "runtime map pure JIT sum run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(json["final_status"], "done Return(\"done\")");
    assert_eq!(sum_step_pure_counter(&json, "pure_calls"), 4);
    assert_eq!(sum_step_pure_counter(&json, "batch_calls"), 1);
    assert_eq!(sum_step_pure_counter(&json, "batch_items"), 4);
    assert_eq!(sum_step_pure_counter(&json, "jit_calls"), 4);
    assert_eq!(sum_step_pure_counter(&json, "arg_stack_packs"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_vec_allocations"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_copied"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_borrowed"), 64);
    assert_eq!(sum_step_pure_counter(&json, "result_bytes_copied"), 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
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
fn check_rejects_decl_binding_mismatch_lint_error() {
    let path = temp_arcw(
        "decl-binding-mismatch",
        r"
flow @flow.opening start {
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
        "decl binding mismatch should fail check, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[AWF0102 identity::decl_binding_mismatch]"));
}

#[test]
fn check_allows_redundant_decl_identity_warning() {
    let path = temp_arcw(
        "redundant-decl-identity-warning",
        r"
flow @flow.opening opening {
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
        "redundant declaration identity is a warning, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("warning[AWF0101 style::redundant_decl_identity]")
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
    let path = runtime_plan_fixture_path();

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
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("plan output is structured JSON");
    let line_display = json["line_display_catalog"]
        .as_array()
        .and_then(|catalog| catalog.first())
        .expect("plan JSON includes line display catalog");
    let contributions = line_display["style_contributions"]
        .as_array()
        .expect("line display includes style contributions");
    assert_plan_style_contributions(&stdout, contributions);
}

#[test]
fn plan_json_uses_profile_selected_dialogue_defaults() {
    let dir = temp_dir("plan-profile-dialogue-defaults");
    let source_path = dir.join("src/main.arcw");
    fs::create_dir_all(source_path.parent().expect("source parent")).expect("create source dir");
    fs::write(
        &source_path,
        r"
pub dialogue defaults @dialogue.defaults {
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub dialogue defaults @dialogue.defaults.mobile {
    rich_text {
        ruby {
            size = 10px
        }
    }
}

pub surface character alice {
}

flow opening {
    alice: |[夢](ゆめ)[p]
}
",
    )
    .expect("write profile source");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &manifest,
        r#"
[profiles.dev]
kind = "game"
source = "src/main.arcw"
adapter = "sans-io"
dialogue_defaults = "dialogue.defaults.mobile"
"#,
    )
    .expect("write profile manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("plan")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("dev")
        .arg("--json")
        .output()
        .expect("arcw plan --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(
        output.status.success(),
        "profile plan should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("profile plan output is structured JSON");
    let contributions = json["line_display_catalog"]
        .as_array()
        .and_then(|catalog| catalog.first())
        .and_then(|display| display["style_contributions"].as_array())
        .expect("profile plan includes line display style contributions");

    assert_plan_style_contribution(
        &stdout,
        contributions,
        PlanStyleContribution {
            layer: "dialogue_defaults",
            path: "rich_text.ruby.size",
            value: "10px",
            active: Some(true),
            requires_range: true,
            context: "profile-selected dialogue defaults",
        },
    );
    assert!(
        !contributions.iter().any(|contribution| {
            contribution["layer"] == "dialogue_defaults"
                && contribution["path"] == "rich_text.ruby.size"
                && contribution["value"] == "14px"
        }),
        "profile plan should not use implicit defaults when dialogue_defaults selects mobile: {stdout}"
    );
}

#[test]
fn rich_text_profiled_sample_checks_profiles_and_plan_defaults() {
    let manifest = workspace_root().join("samples/rich-text-profiled/arcw.toml");

    for profile in ["desktop", "mobile"] {
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("check")
            .arg("--manifest")
            .arg(&manifest)
            .arg("--profile")
            .arg(profile)
            .output()
            .expect("arcw check --profile runs for rich-text-profiled sample");
        assert!(
            output.status.success(),
            "rich-text-profiled {profile} check should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("plan")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("mobile")
        .arg("--json")
        .output()
        .expect("arcw plan --profile runs for rich-text-profiled sample");

    assert!(
        output.status.success(),
        "rich-text-profiled mobile plan should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("sample profile plan output is structured JSON");
    let display = json["line_display_catalog"]
        .as_array()
        .and_then(|catalog| catalog.first())
        .expect("sample profile plan includes a line display");
    assert_eq!(
        display["window"], "textbox.mobile",
        "mobile profile should select the mobile textbox: {stdout}"
    );
    let contributions = display["style_contributions"]
        .as_array()
        .expect("sample profile line display includes style contributions");

    assert_plan_style_contribution(
        &stdout,
        contributions,
        PlanStyleContribution {
            layer: "dialogue_defaults",
            path: "window",
            value: "@textbox.mobile",
            active: Some(true),
            requires_range: true,
            context: "sample mobile textbox defaults",
        },
    );
    assert_plan_style_contribution(
        &stdout,
        contributions,
        PlanStyleContribution {
            layer: "dialogue_defaults",
            path: "rich_text.ruby.gap",
            value: "1px",
            active: Some(true),
            requires_range: true,
            context: "sample mobile ruby gap defaults",
        },
    );
    assert_plan_style_contribution(
        &stdout,
        contributions,
        PlanStyleContribution {
            layer: "inline_span",
            path: "rich_text.effect",
            value: "shake",
            active: Some(true),
            requires_range: true,
            context: "sample inferred inline effect",
        },
    );
    assert!(
        contributions.iter().all(|contribution| {
            !(contribution["layer"] == "dialogue_defaults"
                && contribution["source"]["item_id"] == "dialogue.defaults"
                && contribution["active"] == true)
        }),
        "mobile profile should not activate desktop dialogue defaults: {stdout}"
    );
}

fn runtime_plan_fixture_path() -> PathBuf {
    temp_arcw(
        "runtime-plan",
        r##"
pub dialogue defaults @dialogue.defaults {
    window = @textbox.plan
    rich_text {
        ruby {
            size = 14px
        }
    }
}

pub textbox plan {
    rich_text {
        text {
            color = "#303132"
        }
    }
}

pub surface character alice {
    dialogue_style {
        rich_text {
            text {
                color = "#202122"
            }
        }
    }
}

flow plan {
    let alice_preset = alice(text_color="#404142")
    alice_preset(text_color="#505152")[|[待](ま)って。[mark .release][p]]
    with:
        thread motion:
            wait(0.1s)
        on mark(.release):
            log.info("release")
}
"##,
    )
}

fn assert_plan_style_contributions(stdout: &str, contributions: &[serde_json::Value]) {
    assert_plan_style_contribution(
        stdout,
        contributions,
        PlanStyleContribution {
            layer: "dialogue_defaults",
            path: "rich_text.ruby.size",
            value: "14px",
            active: None,
            requires_range: true,
            context: "dialogue defaults",
        },
    );
    assert_plan_style_contribution(
        stdout,
        contributions,
        PlanStyleContribution {
            layer: "character_dialogue_style",
            path: "rich_text.text.color",
            value: "\"#202122\"",
            active: None,
            requires_range: true,
            context: "character dialogue_style",
        },
    );
    assert_plan_style_contribution(
        stdout,
        contributions,
        PlanStyleContribution {
            layer: "text_box_theme",
            path: "rich_text.text.color",
            value: "\"#303132\"",
            active: Some(false),
            requires_range: true,
            context: "textbox theme",
        },
    );
    assert_plan_style_contribution(
        stdout,
        contributions,
        PlanStyleContribution {
            layer: "speaker_preset",
            path: "text_color",
            value: "\"#404142\"",
            active: Some(false),
            requires_range: true,
            context: "speaker preset",
        },
    );
    assert_plan_style_contribution(
        stdout,
        contributions,
        PlanStyleContribution {
            layer: "line_options",
            path: "text_color",
            value: "\"#505152\"",
            active: Some(true),
            requires_range: false,
            context: "line option",
        },
    );
}

#[derive(Clone, Copy)]
struct PlanStyleContribution<'a> {
    layer: &'a str,
    path: &'a str,
    value: &'a str,
    active: Option<bool>,
    requires_range: bool,
    context: &'a str,
}

fn assert_plan_style_contribution(
    stdout: &str,
    contributions: &[serde_json::Value],
    expected: PlanStyleContribution<'_>,
) {
    assert!(
        contributions.iter().any(|contribution| {
            contribution["layer"] == expected.layer
                && contribution["path"] == expected.path
                && contribution["value"] == expected.value
                && contribution["source"]["kind"] == "source_file"
                && expected
                    .active
                    .is_none_or(|active| contribution["active"] == active)
                && (!expected.requires_range
                    || (contribution["source"]["range"]["start"].is_number()
                        && contribution["source"]["range"]["end"].is_number()))
        }),
        "plan JSON should include {} provenance: {stdout}",
        expected.context
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
fn profile_json_reports_runtime_math_backend_selection() {
    let path = temp_arcw(
        "profile-math-backend",
        r#"
flow @flow.profile profile {
    log.info("profile math")
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("profile")
        .arg(&path)
        .arg("--math-backend")
        .arg("ndarray")
        .arg("--math-wgpu-min-elements")
        .arg("4096")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw profile runs with explicit math backend");

    assert!(
        output.status.success(),
        "math backend profile should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("profile output is structured JSON");
    let pure_config = &json["runtime"]["executor_stats"]["pure_config"];
    assert_eq!(pure_config["math_backend"], "ndarray");
    assert_eq!(pure_config["math_wgpu_min_elements"], 4096);
    assert!(
        !String::from_utf8_lossy(&output.stdout)
            .contains(&std::env::temp_dir().display().to_string()),
        "profile JSON must not record absolute temp paths: {json}"
    );
}

#[test]
fn run_json_executes_math_intrinsic_with_cli_matrix_bindings() {
    let path = temp_arcw(
        "runtime-math-bindings",
        r"
flow @flow.math math(lhs: MatrixF32, rhs: MatrixF32) -> MatrixF32 {
    let out = math.matmul_f32(lhs, rhs)
    return out
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--flow")
        .arg("flow.math")
        .arg("--math-backend")
        .arg("glam")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--value")
        .arg("lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1")
        .arg("--value")
        .arg("rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2")
        .arg("--json")
        .output()
        .expect("arcw run executes math intrinsic with CLI matrix bindings");

    assert!(
        output.status.success(),
        "runtime math run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(
        json["executor_stats"]["pure_config"]["math_backend"],
        "glam"
    );
    let math = &json["executor_stats"]["math"];
    assert_eq!(math["glam_calls"], 1);
    assert_eq!(math["bytes_borrowed"], 128);
    assert_eq!(math["bytes_copied"], 0);
    assert_eq!(math["last_backend"], "glam");
    let pure = &json["steps"][0]["stats"]["pure"];
    assert_eq!(pure["math_calls"], 1);
    assert_eq!(pure["math_accelerated_calls"], 1);
    assert_eq!(json["final_status"], "done Return(\"matrix/f32/4x4\")");
    assert!(
        !String::from_utf8_lossy(&output.stdout)
            .contains(&std::env::temp_dir().display().to_string()),
        "runtime math JSON must not record absolute temp paths: {json}"
    );
}

#[test]
fn run_json_executes_f64_math_intrinsic_with_cli_matrix_bindings() {
    let path = temp_arcw(
        "runtime-math-f64-bindings",
        r"
flow @flow.math_f64 math_f64(lhs: MatrixF64, rhs: MatrixF64) -> MatrixF64 {
    let out = math.matmul_f64(lhs, rhs)
    return out
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .arg("--flow")
        .arg("flow.math_f64")
        .arg("--math-backend")
        .arg("ndarray")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--value")
        .arg("lhs=matrix/f64/2x2:1.5,2,3.25,4.5")
        .arg("--value")
        .arg("rhs=matrix/f64/2x2:5,6.5,7,8.25")
        .arg("--json")
        .output()
        .expect("arcw run executes f64 math intrinsic with CLI matrix bindings");

    assert!(
        output.status.success(),
        "runtime f64 math run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(
        json["executor_stats"]["pure_config"]["math_backend"],
        "ndarray"
    );
    let math = &json["executor_stats"]["math"];
    assert_eq!(math["ndarray_calls"], 1);
    assert_eq!(math["bytes_borrowed"], 64);
    assert_eq!(math["bytes_copied"], 0);
    assert_eq!(math["last_backend"], "ndarray");
    let pure = &json["steps"][0]["stats"]["pure"];
    assert_eq!(pure["math_calls"], 1);
    assert_eq!(pure["math_accelerated_calls"], 1);
    assert_eq!(pure["arg_bytes_borrowed"], 64);
    assert_eq!(pure["result_bytes_copied"], 32);
    assert_eq!(json["final_status"], "done Return(\"matrix/f64/2x2\")");
    assert!(
        !String::from_utf8_lossy(&output.stdout)
            .contains(&std::env::temp_dir().display().to_string()),
        "runtime f64 math JSON must not record absolute temp paths: {json}"
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
        stdout.contains("\"name\": \"executor_prepare\""),
        "verify-types JSON should split executor construction from run timing: {stdout}"
    );
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
            && stdout.contains("\"name\": \"executor_prepare\"")
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
entry cli @entry.main { goto @flow.main }

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
entry cli @entry.main { goto @flow.main }
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
fn bundle_json_packages_save_files_and_run_bundle_executes_native_file_tasks() {
    let fixture = bundle_native_file_fixture();
    let bundle_stdout = run_bundle_package_command(&fixture);
    assert_bundle_package_output(&fixture, &bundle_stdout);

    let run_stdout = run_bundle_fixture_command(&fixture);
    assert_run_bundle_output(&fixture, &run_stdout);

    let build_stdout = run_build_bundle_command(&fixture);
    assert_build_bundle_output(&fixture, &build_stdout);
}

#[test]
fn run_bundle_rejects_image_asset_metadata_mismatch_before_execution() {
    let fixture = bundle_native_file_fixture();
    let _bundle_stdout = run_bundle_package_command(&fixture);
    let mut bundle_json: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&fixture.bundle_path).expect("bundle JSON is written"),
    )
    .expect("bundle artifact is JSON");
    let image_assets = bundle_json["image_assets"]
        .as_array_mut()
        .expect("image assets are an array");
    let logo = image_assets
        .iter_mut()
        .find(|asset| asset["id"] == "asset.view.logo")
        .expect("static webp image asset is present");
    logo["animation"] = serde_json::Value::String("animated".to_owned());
    fs::write(
        &fixture.bundle_path,
        serde_json::to_vec_pretty(&bundle_json).expect("mutated bundle serializes"),
    )
    .expect("mutated bundle is written");

    let run_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run-bundle")
        .arg(&fixture.bundle_path)
        .arg("--steps")
        .arg("1")
        .output()
        .expect("arcw run-bundle rejects invalid image metadata");

    assert!(
        !run_output.status.success(),
        "run-bundle should reject contradictory image metadata"
    );
    let stderr = String::from_utf8_lossy(&run_output.stderr);
    assert!(
        stderr.contains("metadata mismatch for animation") && stderr.contains("asset.view.logo"),
        "run-bundle error should report the image metadata mismatch: {stderr}"
    );
}

#[test]
fn bundle_json_packages_image_animation_sample_assets_and_run_bundle_validates_them() {
    let dir = temp_dir("bundle-image-animation-sample");
    let source_path = workspace_root().join("samples/image-animation.arcw");
    let bundle_path = dir.join("image-animation.awfb");

    let bundle_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bundle")
        .arg(&source_path)
        .arg("--output")
        .arg(&bundle_path)
        .arg("--json")
        .output()
        .expect("arcw bundle packages image animation sample");

    assert!(
        bundle_output.status.success(),
        "image animation sample bundle should succeed, stderr: {}",
        String::from_utf8_lossy(&bundle_output.stderr)
    );
    let bundle_stdout = String::from_utf8_lossy(&bundle_output.stdout);
    assert!(
        bundle_stdout.contains("\"image_assets\": 5"),
        "bundle JSON summary should count all static and animated sample images: {bundle_stdout}"
    );
    assert!(
        !bundle_stdout.contains(&workspace_root().display().to_string()),
        "bundle JSON summary must not leak workspace paths: {bundle_stdout}"
    );

    let bundle = arcweft_bundle::ArcweftBundle::from_format_slice(
        arcweft_bundle::BundleFormat::Awfb,
        &fs::read(&bundle_path).expect("image animation AWFB is written"),
    )
    .expect("image animation AWFB decodes");
    let bundle_json = serde_json::to_value(bundle).expect("image animation bundle serializes");
    assert_image_animation_bundle_assets(&bundle_json);
    let bundle_text =
        serde_json::to_string(&bundle_json).expect("image animation bundle JSON reserializes");
    assert!(
        !bundle_text.contains(&workspace_root().display().to_string()),
        "bundle artifact must not leak workspace paths: {bundle_text}"
    );

    let run_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run-bundle")
        .arg(&bundle_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("2")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw run-bundle validates image animation sample assets");

    assert!(
        run_output.status.success(),
        "run-bundle should validate and execute image animation sample, stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    let run_stdout = String::from_utf8_lossy(&run_output.stdout);
    assert_image_animation_run_bundle_output(&run_stdout);
}

fn assert_image_animation_bundle_assets(bundle_json: &serde_json::Value) {
    let image_assets = bundle_json["image_assets"]
        .as_array()
        .expect("image animation bundle records image_assets");
    assert_eq!(image_assets.len(), 5);
    assert_bundle_image_asset(
        image_assets,
        "asset.bg.room",
        "bg/room.png",
        "png",
        "static",
    );
    assert_bundle_image_asset(
        image_assets,
        "asset.bg.garden",
        "bg/garden.jpg",
        "jpeg",
        "static",
    );
    assert_bundle_image_asset(
        image_assets,
        "asset.bg.poster",
        "bg/poster.webp",
        "webp",
        "static",
    );
    assert_bundle_image_asset(
        image_assets,
        "asset.bg.pulse",
        "bg/pulse.gif",
        "gif",
        "animated",
    );
    assert_bundle_image_asset(
        image_assets,
        "asset.bg.loop",
        "bg/loop.webp",
        "webp",
        "animated",
    );
}

fn assert_image_animation_run_bundle_output(run_stdout: &str) {
    let run_json: serde_json::Value =
        serde_json::from_str(run_stdout).expect("run-bundle output is structured JSON");
    assert!(
        run_json["phases"]
            .as_array()
            .expect("run-bundle reports phases")
            .iter()
            .any(|phase| phase["name"] == "validate_image_assets"),
        "run-bundle should explicitly validate bundled image assets: {run_stdout}"
    );
    assert_eq!(run_json["executor"], "awbc_product");
    assert_eq!(run_json["final_status"], "dialogue image.static_png");
    assert!(
        !run_stdout.contains(&workspace_root().display().to_string()),
        "run-bundle JSON must not leak workspace paths: {run_stdout}"
    );
    assert!(
        !run_json["phases"]
            .as_array()
            .expect("phases are present")
            .iter()
            .any(|phase| matches!(
                phase["name"].as_str(),
                Some("parse" | "typecheck" | "runtime_plan_lower")
            )),
        "run-bundle should execute decoded bytecode without source recompilation: {run_stdout}"
    );
}

fn assert_bundle_image_asset(
    image_assets: &[serde_json::Value],
    id: &str,
    path: &str,
    format: &str,
    animation: &str,
) {
    let asset = image_assets
        .iter()
        .find(|asset| asset["id"] == id)
        .unwrap_or_else(|| panic!("bundle image asset {id} should be present"));
    assert_eq!(asset["file"]["space"], "asset");
    assert_eq!(asset["file"]["path"], path);
    assert_eq!(asset["format"], format);
    assert_eq!(asset["animation"], animation);
    assert_eq!(asset["dimensions"]["width"], 2);
    assert_eq!(asset["dimensions"]["height"], 1);
}

struct BundleNativeFileFixture {
    dir: PathBuf,
    source_path: PathBuf,
    bundle_path: PathBuf,
}

fn bundle_native_file_fixture() -> BundleNativeFileFixture {
    let dir = temp_dir("bundle-native-file-task");
    let source_path = dir.join("main.arcw");
    let asset_dir = dir.join("assets");
    let save_dir = dir.join(".arcweft").join("save");
    let bundle_path = dir.join("game.awfb");
    fs::create_dir_all(asset_dir.join("bg")).expect("create virtual asset bg root");
    fs::create_dir_all(asset_dir.join("view")).expect("create virtual asset view root");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::copy(
        sample_image_asset_path(Path::new("bg").join("room.png")),
        asset_dir.join("bg").join("room.png"),
    )
    .expect("seed png asset");
    fs::copy(
        sample_image_asset_path(Path::new("bg").join("poster.webp")),
        asset_dir.join("view").join("logo.webp"),
    )
    .expect("seed static webp asset");
    fs::write(save_dir.join("input.txt"), "bundle-ok").expect("seed virtual input");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
    fn write_text(path: VirtualPath, body: String) -> Need<Unit, FsError> effects { fs.write }
}
extern capability path { fn save(path: String) -> VirtualPath }
entry cli @entry.main { goto @flow.main }
flow @flow.main main effects { fs.read(save), fs.write(save) } {
    let text = try await fs.read_text(path.save("input.txt")) with { error e => return "read_failed" }
    try await fs.write_text(path.save("output.txt"), text) with { error e => return "write_failed" }
    return text
}
"#,
    )
    .expect("write bundle fixture");
    BundleNativeFileFixture {
        dir,
        source_path,
        bundle_path,
    }
}

fn run_bundle_package_command(fixture: &BundleNativeFileFixture) -> String {
    let bundle_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bundle")
        .arg(&fixture.source_path)
        .arg("--output")
        .arg(&fixture.bundle_path)
        .arg("--include-save")
        .arg("--json")
        .output()
        .expect("arcw bundle packages native file task");

    assert!(
        bundle_output.status.success(),
        "bundle should succeed, stderr: {}",
        String::from_utf8_lossy(&bundle_output.stderr)
    );
    String::from_utf8_lossy(&bundle_output.stdout).into_owned()
}

fn assert_bundle_package_output(fixture: &BundleNativeFileFixture, bundle_stdout: &str) {
    assert!(
        bundle_stdout.contains("\"source\": \"main.arcw\"")
            && bundle_stdout.contains("\"required_host_calls\"")
            && bundle_stdout.contains("fs.read_text")
            && bundle_stdout.contains("fs.write_text")
            && bundle_stdout.contains("\"adapter_manifests\": 2")
            && bundle_stdout.contains("\"bytecode_instructions\"")
            && bundle_stdout.contains("\"name\": \"bytecode_lower\"")
            && bundle_stdout.contains("\"name\": \"encode_bundle\"")
            && bundle_stdout.contains("\"virtual_files\": 3")
            && bundle_stdout.contains("\"image_assets\": 2"),
        "bundle JSON should describe native requirements and packaged save input: {bundle_stdout}"
    );
    assert!(
        !bundle_stdout.contains(&fixture.dir.display().to_string()),
        "bundle JSON must not record absolute temp paths: {bundle_stdout}"
    );
    let bundle = arcweft_bundle::ArcweftBundle::from_format_slice(
        arcweft_bundle::BundleFormat::Awfb,
        &fs::read(&fixture.bundle_path).expect("bundle AWFB is written"),
    )
    .expect("bundle AWFB decodes");
    let bundle_json =
        serde_json::to_string_pretty(&bundle).expect("decoded bundle serializes for assertions");
    assert!(
        bundle_json.contains("\"adapter_manifest_ids\"")
            && bundle_json.contains("\"adapter_manifests\"")
            && bundle_json.contains("\"bytecode\"")
            && bundle_json.contains("\"program\"")
            && bundle_json.contains("native-file")
            && bundle_json.contains("save")
            && bundle_json.contains("input.txt")
            && bundle_json.contains("\"image_assets\"")
            && bundle_json.contains("\"id\": \"asset.bg.room\"")
            && bundle_json.contains("\"format\": \"png\"")
            && bundle_json.contains("\"id\": \"asset.view.logo\"")
            && bundle_json.contains("\"format\": \"webp\"")
            && bundle_json.contains("\"animation\": \"static\"")
            && bundle_json.contains("\"dimensions\""),
        "bundle artifact should include executable bytecode, native-file adapter metadata, relative save file, typed image assets, and decoded image metadata: {bundle_json}"
    );
    assert!(
        !bundle_json.contains(&fixture.dir.display().to_string()),
        "bundle artifact must not record absolute temp paths: {bundle_json}"
    );
}

fn sample_image_asset_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("samples")
        .join("assets")
        .join(relative)
}

fn run_bundle_fixture_command(fixture: &BundleNativeFileFixture) -> String {
    let run_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run-bundle")
        .arg(&fixture.bundle_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw run-bundle executes packaged source");

    assert!(
        run_output.status.success(),
        "run-bundle should succeed, stderr: {}",
        String::from_utf8_lossy(&run_output.stderr)
    );
    String::from_utf8_lossy(&run_output.stdout).into_owned()
}

fn assert_run_bundle_output(fixture: &BundleNativeFileFixture, run_stdout: &str) {
    let run_json: serde_json::Value =
        serde_json::from_str(run_stdout).expect("run-bundle output is structured JSON");
    assert!(
        run_stdout.contains("\"source\": \"main.arcw\"")
            && run_stdout.contains("\"executor\": \"bytecode_vm\"")
            && run_stdout.contains("\"bytecode_instructions\"")
            && run_stdout.contains("\"adapter_manifests\": 2")
            && run_stdout.contains("\"name\": \"decode_bundle\"")
            && run_stdout.contains("\"name\": \"bytecode_decode\"")
            && run_stdout.contains("\"name\": \"run\"")
            && run_stdout.contains("\"completed_tasks\": 2")
            && run_stdout.contains("\"failed_tasks\": 0")
            && run_stdout.contains("\"read_ops\": 1")
            && run_stdout.contains("\"write_ops\": 1")
            && run_stdout.contains("return bundle-ok"),
        "run-bundle JSON should show packaged native I/O completion: {run_stdout}"
    );
    assert!(
        !run_stdout.contains(&fixture.dir.display().to_string()),
        "run-bundle JSON must not record absolute temp paths: {run_stdout}"
    );
    assert!(
        !run_json["phases"]
            .as_array()
            .expect("phases are present")
            .iter()
            .any(|phase| matches!(
                phase["name"].as_str(),
                Some("parse" | "typecheck" | "runtime_plan_lower")
            )),
        "run-bundle should execute decoded bytecode without source recompilation: {run_stdout}"
    );
}

fn run_build_bundle_command(fixture: &BundleNativeFileFixture) -> String {
    let build_bundle_path = fixture.dir.join("game-build.awfb");
    let build_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("build")
        .arg("bundle")
        .arg(&fixture.source_path)
        .arg("--output")
        .arg(&build_bundle_path)
        .arg("--include-save")
        .arg("--json")
        .output()
        .expect("arcw build bundle packages native file task");
    assert!(
        build_output.status.success(),
        "build bundle should succeed, stderr: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    String::from_utf8_lossy(&build_output.stdout).into_owned()
}

fn assert_build_bundle_output(fixture: &BundleNativeFileFixture, build_stdout: &str) {
    assert!(
        build_stdout.contains("\"bundle\": \"game-build.awfb\"")
            && build_stdout.contains("\"bytecode_instructions\"")
            && build_stdout.contains("\"adapter_manifests\": 2"),
        "build bundle JSON should use the same executable bundle pipeline: {build_stdout}"
    );
    assert!(
        !build_stdout.contains(&fixture.dir.display().to_string()),
        "build bundle JSON must not record absolute temp paths: {build_stdout}"
    );
}

#[test]
fn run_bundle_uses_embedding_registered_custom_adapter() {
    let fixture = custom_adapter_bundle_fixture();
    CUSTOM_BUNDLE_ADAPTER_CALLS.store(0, Ordering::SeqCst);
    *CUSTOM_BUNDLE_ADAPTER_OUTPUT
        .lock()
        .expect("custom adapter output lock") = Some(fixture.marker_path.clone());

    let build_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("build")
        .arg("bundle")
        .arg("--manifest")
        .arg(&fixture.manifest_path)
        .arg("--profile")
        .arg("game")
        .arg("--output")
        .arg(&fixture.bundle_path)
        .arg("--json")
        .output()
        .expect("arcw build bundle packages custom adapter surface");

    assert!(
        build_output.status.success(),
        "custom adapter bundle build should succeed, stderr: {}",
        String::from_utf8_lossy(&build_output.stderr)
    );
    let bundle_json = fs::read_to_string(&fixture.bundle_path).expect("bundle is written");
    assert!(
        bundle_json.contains("\"adapter_manifests\"")
            && bundle_json.contains("custom-file")
            && bundle_json.contains("custom.read"),
        "bundle should carry custom adapter manifest body and host call: {bundle_json}"
    );
    assert!(
        !bundle_json.contains(&fixture.dir.display().to_string()),
        "custom bundle artifact must not record absolute temp paths: {bundle_json}"
    );

    let runner_options = BundleRunnerOptions::default();
    let registrars: [NativeAdapterRegistrar; 1] =
        [|_, builder| builder.register(CustomBundleAdapter::new())];
    let report =
        run_bundle_file_with_native_adapters(&fixture.bundle_path, &runner_options, &registrars)
            .expect("embedding runner executes custom adapter bundle");

    assert_eq!(report.native_io.completed_tasks, 1);
    assert!(
        report
            .phases
            .iter()
            .any(|phase| phase.name == "read_bundle")
            && report
                .phases
                .iter()
                .any(|phase| phase.name == "decode_bundle")
            && report.phases.iter().any(|phase| phase.name == "run"),
        "embedding runner report should include load and run phases: {report:?}"
    );
    assert_eq!(CUSTOM_BUNDLE_ADAPTER_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(
        fs::read_to_string(&fixture.marker_path).expect("custom adapter marker is written"),
        "custom-ok"
    );
}

struct CustomAdapterBundleFixture {
    dir: PathBuf,
    manifest_path: PathBuf,
    bundle_path: PathBuf,
    marker_path: PathBuf,
}

fn custom_adapter_bundle_fixture() -> CustomAdapterBundleFixture {
    let dir = temp_dir("bundle-custom-adapter");
    let source_path = dir.join("game.arcw");
    let manifest_path = dir.join("arcw.toml");
    let adapter_manifest_path = dir.join("custom-adapter.toml");
    let bundle_path = dir.join("custom.awfb");
    let marker_path = dir.join("custom-marker.txt");
    fs::write(
        &source_path,
        r#"
extern capability custom {
    type CustomError
    fn read(path: String) -> Need<String, CustomError> effects { custom.read }
}

flow @flow.opening opening effects { custom.read } {
    let body = try await custom.read(path = "opening.txt") with { error e => return "failed" }
    return body
}
"#,
    )
    .expect("write custom adapter source");
    fs::write(
        &adapter_manifest_path,
        r#"
schema_version = 1
id = "custom-file"
display_name = "Custom File"
effects = ["custom.read"]

[[host_calls]]
id = "custom.read"
effects = ["custom.read"]
"#,
    )
    .expect("write custom adapter manifest");
    fs::write(
        &manifest_path,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
adapter = "custom-file"
adapter_manifests = ["custom-adapter.toml"]
"#,
    )
    .expect("write launch manifest");
    CustomAdapterBundleFixture {
        dir,
        manifest_path,
        bundle_path,
        marker_path,
    }
}

#[derive(Debug)]
struct CustomBundleAdapter {
    manifest: AdapterManifest,
}

impl CustomBundleAdapter {
    fn new() -> Self {
        Self {
            manifest: AdapterManifest::new("custom-file", "Custom File")
                .with_effect(AdapterEffectCapability::new("custom.read"))
                .with_host_call(AdapterHostCall::new(
                    "custom.read",
                    [AdapterEffectCapability::new("custom.read")],
                )),
        }
    }
}

impl HostAdapter for CustomBundleAdapter {
    fn manifest(&self) -> &AdapterManifest {
        &self.manifest
    }

    fn complete(&self, task: &TaskSpec) -> Option<HostTaskOutcome> {
        let HostTaskRequest::Custom {
            capability,
            operation,
            args,
            ..
        } = &task.request
        else {
            return None;
        };
        if capability.0 != "custom" || operation != "read" {
            return None;
        }
        CUSTOM_BUNDLE_ADAPTER_CALLS.fetch_add(1, Ordering::SeqCst);
        let result = RuntimePayload::from(format!("custom-ok:{}", args.len()));
        if let Some(path) = CUSTOM_BUNDLE_ADAPTER_OUTPUT
            .lock()
            .expect("custom adapter output lock")
            .as_ref()
        {
            fs::write(path, "custom-ok").expect("write custom adapter marker");
        }
        Some(HostTaskOutcome {
            result: Ok(result),
            metrics: HostTaskMetrics::default(),
        })
    }

    fn can_complete_in_parallel(&self, _request: &HostTaskRequest) -> bool {
        false
    }
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
entry cli @entry.main { goto @flow.main }
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
            && stdout.contains("\"parallel_io_tasks\": 2")
            && stdout.contains("\"parallel_system_info_tasks\": 0")
            && stdout.contains("\"parallel_marker_tasks\": 0")
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
entry cli @entry.main { goto @flow.main }
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
            && stdout.contains("\"host_system\"")
            && stdout.contains("\"completed_tasks\": 3")
            && stdout.contains("\"system_info_ops\": 3")
            && stdout.contains("\"scheduler\"")
            && stdout.contains("\"submitted\": 3")
            && stdout.contains("\"in_flight\": 0"),
        "run JSON should show runtime system info task completion: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("system info run output is structured JSON");
    assert!(json["host_system"]["physical_cores"].as_u64().unwrap_or(0) > 0);
    assert!(json["host_system"]["logical_threads"].as_u64().unwrap_or(0) > 0);
    assert!(
        json["host_system"]["available_parallelism"]
            .as_u64()
            .unwrap_or(0)
            > 0
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
fn fmt_accepts_awfagent_path_and_preserves_agent_source_json() {
    let source = "#[agent(version = 1)]\nagent @agent.cli.format_smoke format_smoke()\neffects { agent.resource.read, debug.record }\n{\n    // Keep comments and Agent calls stable.\n    let resource = try read_resource(\"arcweft://session/cli/observation/latest.json\")\n    attach(resource)\n    return resource.uri\n}\n";
    let path = temp_file("fmt-agent-preserve", "awfagent", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg(&path)
        .arg("--json")
        .output()
        .expect("arcw fmt runs on .awfagent");

    assert!(
        output.status.success(),
        "fmt .awfagent should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("fmt JSON report");
    let file = &report["files"][0];
    assert_eq!(file["changed"], false);
    assert_eq!(file["edits"], 0);
    assert_eq!(file["output"], source);
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn fmt_rejects_game_sugar_rewrites_for_awfagent_path() {
    let source = "#[agent(version = 1)]\nagent @agent.cli.format_smoke format_smoke()\n{\n    return \"ok\"\n}\n";
    let path = temp_file("fmt-agent-reject-expand", "awfagent", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg("--expand-sugar")
        .arg(&path)
        .output()
        .expect("arcw fmt runs on .awfagent");

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not supported for Agent"),
        "Agent formatter should reject game sugar rewrites, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn fmt_expand_sugar_accepts_flags_before_path_and_writes() {
    let path = temp_arcw(
        "fmt-expand",
        "pub surface character @character.alice Alice as alice {}\nflow @flow.opening opening {\n    alice: hi $(name)[.shake]there[/][page]\n    with:\n        log.info(\"x\")\n    goto parent::next\n}\n",
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
    assert!(rewritten.contains("alice.say()[hi #[name][effect .shake]there[/effect][p]]"));
    assert!(rewritten.contains("with {"));
    assert!(rewritten.contains("goto super::next"));
}

#[test]
fn fmt_expand_sugar_respects_decl_identity_attributes_when_writing() {
    let source = "#[generated]\nflow @flow.generated generated {\n}\n#[allow(style::redundant_decl_identity)]\nsource @source.http_requests http_requests: Source<HttpRequest, HttpError> {\n}\nflow @flow.opening opening {\n}\nflow @flow.opening start {\n}\n";
    let path = temp_arcw("fmt-expand-decl-identity-attrs", source);

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
    assert!(rewritten.contains("flow @flow.generated generated {"));
    assert!(rewritten.contains("source @source.http_requests http_requests"));
    assert!(rewritten.contains("flow opening {"));
    assert!(rewritten.contains("flow @flow.opening start {"));
}

#[test]
fn fmt_expand_sugar_respects_source_generated_attribute_when_writing() {
    let source = "#![generated(tool)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let path = temp_arcw("fmt-expand-source-generated", source);

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
    assert!(rewritten.contains("flow @flow.generated generated {"));
    assert!(rewritten.contains("alice.say()[hi[p]]"));
}

#[test]
fn fmt_expand_sugar_respects_source_allow_attribute_when_writing() {
    let source = "#![allow(style::redundant_decl_identity)]\nflow @flow.generated generated {\n    alice: hi[p]\n}\n";
    let path = temp_arcw("fmt-expand-source-allow", source);

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
    assert!(rewritten.contains("flow @flow.generated generated {"));
    assert!(rewritten.contains("alice.say()[hi[p]]"));
}

#[test]
fn fmt_expand_sugar_nests_dotted_dialogue_defaults_when_writing() {
    let source = "pub dialogue defaults @dialogue.defaults {\n    rich_text.ruby.size = 14px\n    rich_text.ruby.gap += 1px\n}\n";
    let path = temp_arcw("fmt-expand-dialogue-defaults", source);

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
    assert!(
        rewritten.contains(
            "rich_text {\n        ruby {\n            size = 14px\n            gap += 1px"
        )
    );
    assert!(!rewritten.contains("rich_text.ruby.size"));
    assert!(!rewritten.contains("rich_text.ruby.gap"));
}

#[test]
fn fmt_canonical_rich_text_rewrites_inferred_tags_without_other_sugar() {
    let source = "flow @flow.opening opening {\n    alice: hi $(name)[.keyword]word[/][.sparkle amp=2px]there[/][page]\n    let handles = alice.say()[[.vertical_rl]縦[/][p]] with: out handles\n}\n";
    let path = temp_arcw("fmt-canonical-rich-text", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg("--canonical-rich-text")
        .arg(&path)
        .output()
        .expect("arcw fmt runs");

    assert!(
        output.status.success(),
        "fmt --canonical-rich-text should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("$(name)"));
    assert!(stdout.contains("[mark .keyword]word[effect .sparkle amp=2px]there[/effect]"));
    assert!(stdout.contains("[effect .sparkle amp=2px]there[/effect]"));
    assert!(stdout.contains("[layout .vertical_rl]縦[/layout][p]"));
    assert!(stdout.contains("[page]"));
    assert!(!stdout.contains("[/]"));
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn fmt_canonical_rich_text_rewrites_text_proxy_inference() {
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=choice]proxy[/][.KeywordHit]typed[/][p]\n}\n";
    let path = temp_arcw("fmt-canonical-rich-text-text-proxy", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg("--canonical-rich-text")
        .arg(&path)
        .output()
        .expect("arcw fmt runs");

    assert!(
        output.status.success(),
        "fmt --canonical-rich-text should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[object .hotspot type=KeywordHit channel=choice]proxy[/object]"));
    assert!(stdout.contains("[object .KeywordHit type=KeywordHit]typed[/object]"));
    assert!(!stdout.contains("[effect .hotspot"));
    assert!(!stdout.contains("[effect .KeywordHit"));
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
}

#[test]
fn fmt_canonical_rich_text_preserves_nested_proxy_params() {
    let source = "#[text_proxy(kind=\"keyword\", default_hit=true)]\npub struct KeywordHit {\n    channel: String\n}\n\n#[text_proxy(kind=\"hover\", default_hit=false)]\npub struct HoverHit {\n    layer: String\n}\n\nflow @flow.opening opening {\n    alice: [.hotspot type=KeywordHit channel=inventory][.HoverHit tone=alert]multi[/][/][p]\n}\n";
    let path = temp_arcw("fmt-canonical-rich-text-nested-text-proxy", source);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("fmt")
        .arg("--canonical-rich-text")
        .arg(&path)
        .output()
        .expect("arcw fmt runs");

    assert!(
        output.status.success(),
        "fmt --canonical-rich-text should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(
        "[object .hotspot type=KeywordHit channel=inventory][object .HoverHit type=HoverHit tone=alert]multi[/object][/object]"
    ));
    assert!(!stdout.contains("[effect .hotspot"));
    assert!(!stdout.contains("[effect .HoverHit"));
    assert!(!stdout.contains("[/]"));
    assert_eq!(fs::read_to_string(&path).expect("source remains"), source);
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
    goto @flow.opening
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
signal current_flow: Watch<Ref<Flow>>

flow observed
effects { signal.write }
{
    log.info("enter observed")
    signal.set(@signal.current_flow, @flow.observed)
    return "done"
}

test @test.observed scenario {
    goto @flow.observed
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
    measure iterations = 2 { goto @flow.bench }
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
            && stdout.contains("\"host_system\"")
            && stdout.contains("\"executed_ops_median\": 2")
            && stdout.contains("\"compiler\"")
            && stdout.contains("\"phases\"")
            && stdout.contains("\"name\": \"typecheck\"")
            && stdout.contains("\"borrow_check\"")
            && stdout.contains("\"boundary_checks\""),
        "bench JSON should include headless measurement and compiler profiling: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let host = &json["benches"][0]["sections"][0]["measurement"]["host_system"];
    assert!(host["physical_cores"].as_u64().unwrap_or(0) > 0);
    assert!(host["logical_threads"].as_u64().unwrap_or(0) > 0);
    assert!(host["available_parallelism"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn bench_json_can_measure_aot_executor_sections() {
    let path = temp_arcw(
        "script-bench-aot",
        r#"
bench @bench.runtime_aot {
    measure iterations = 1 { goto @flow.bench }
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
    measure iterations = 2 { goto @flow.for_pure }
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
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
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
fn bench_json_batches_bracket_sequence_pure_jit_calls() {
    let path = temp_arcw(
        "script-bench-bracket-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.bracket_pure {
    measure iterations = 2 { goto @flow.bracket_pure }
}

flow @flow.bracket_pure bracket_pure {
    let scores: Vec<i64> = [score(1i64, 2i64), score(2i64, 2i64), score(3i64, 2i64), score(4i64, 2i64)]
    for item in scores {
        log.info(item)
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
        .expect("arcw bench measures bracket-sequence pure JIT batch");

    assert!(
        output.status.success(),
        "bracket pure JIT bench should succeed, stderr: {}",
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
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        4
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_flatten_materializations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_flatten_bytes_copied_median"],
        0
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        32
    );
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_fuses_bracket_sequence_pure_sum_jit_calls() {
    let path = temp_arcw(
        "script-bench-bracket-pure-sum-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.bracket_pure_sum {
    measure iterations = 2 { goto @flow.bracket_pure_sum }
}

flow @flow.bracket_pure_sum bracket_pure_sum {
    let total: i64 = [score(1i64, 2i64), score(2i64, 2i64), score(3i64, 2i64), score(4i64, 2i64)].sum()
    log.info(total)
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
        .expect("arcw bench measures fused bracket-sequence pure sum JIT batch");

    assert!(
        output.status.success(),
        "bracket pure sum JIT bench should succeed, stderr: {}",
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
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_batches_bracket_sequence_pure_aot_on_worker_pool() {
    let path = temp_arcw(
        "script-bench-bracket-pure-aot",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.bracket_pure {
    measure iterations = 2 { goto @flow.bracket_pure }
}

flow @flow.bracket_pure bracket_pure {
    let scores: Vec<i64> = [score(1i64, 2i64), score(2i64, 2i64), score(3i64, 2i64), score(4i64, 2i64)]
    for item in scores {
        log.info(item)
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
        .arg("aot")
        .arg("--pure-workers")
        .arg("2")
        .arg("--pure-batch-min-len")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw bench measures bracket-sequence pure AOT batch");

    assert!(
        output.status.success(),
        "bracket pure AOT bench should succeed, stderr: {}",
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
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert!(
        measurement["deterministic"]["pure_thread_pool_jobs_median"]
            .as_u64()
            .is_some_and(|jobs| jobs >= 2),
        "AOT bracket batch should use the configured worker pool: {measurement}"
    );
    assert_aot_parallel_policy(measurement, "AOT bracket batch");
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["executor_stats"]["pure_config"]["backend"],
        "aot"
    );
    assert_eq!(
        measurement["executor_stats"]["pure_config"]["workers"]["fixed"],
        2
    );
    assert_eq!(
        measurement["executor_stats"]["pure_config"]["worker_pool_active"],
        true
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["aot_successes"],
        1
    );
}

#[test]
fn bench_json_batches_map_closure_pure_jit_calls() {
    let path = temp_arcw(
        "script-bench-map-pure-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.map_pure {
    measure iterations = 2 { goto @flow.map_pure }
}

flow @flow.map_pure map_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    for item in scores {
        log.info(item)
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
        .expect("arcw bench measures map-closure pure JIT batch");

    assert!(
        output.status.success(),
        "map pure JIT bench should succeed, stderr: {}",
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
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_fuses_map_closure_pure_sum_jit_calls() {
    let path = temp_arcw(
        "script-bench-map-pure-sum-jit",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.map_pure_sum {
    measure iterations = 2 { goto @flow.map_pure_sum }
}

flow @flow.map_pure_sum map_pure_sum {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let total: i64 = values.map(|item| score(item, 2i64)).sum()
    log.info(total)
    return "done"
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--json")
        .arg("--samples")
        .arg("2")
        .arg("--steps")
        .arg("32")
        .arg("--pure-backend")
        .arg("jit")
        .output()
        .expect("arcw bench measures fused map-closure pure sum JIT batch");

    assert!(
        output.status.success(),
        "map pure sum JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("bench output is utf8");
    assert!(
        !stdout.contains(path.to_string_lossy().as_ref()),
        "map pure sum bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        1
    );
}

#[test]
fn bench_json_batches_map_closure_pure_aot_on_worker_pool() {
    let path = temp_arcw(
        "script-bench-map-pure-aot",
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

bench @bench.map_pure {
    measure iterations = 2 { goto @flow.map_pure }
}

flow @flow.map_pure map_pure {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    for item in scores {
        log.info(item)
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
        .arg("aot")
        .arg("--pure-workers")
        .arg("2")
        .arg("--pure-batch-min-len")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw bench measures map-closure pure AOT batch");

    assert!(
        output.status.success(),
        "map pure AOT bench should succeed, stderr: {}",
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
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 4);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 4);
    assert!(
        measurement["deterministic"]["pure_thread_pool_jobs_median"]
            .as_u64()
            .is_some_and(|jobs| jobs >= 2),
        "AOT map batch should use the configured worker pool: {measurement}"
    );
    assert_aot_parallel_policy(measurement, "AOT map batch");
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["aot_successes"],
        1
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
    measure iterations = 2 { goto @flow.branch_for_pure }
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
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        192
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
    measure iterations = 2 { goto @flow.match_pure }
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
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
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
fn bench_json_measures_thread_scheduling_characteristics() {
    let path = temp_arcw(
        "script-bench-thread-scheduling",
        r#"
bench @bench.threads {
    measure iterations = 1 { goto @flow.threads }
}

flow @flow.threads threads {
    thread first {
        log.info("first")
    }
    thread second {
        log.info("second")
    }
    thread third {
        log.info("third")
    }
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
        .arg("16")
        .arg("--max-ops")
        .arg("1")
        .arg("--mode")
        .arg("drain")
        .arg("--json")
        .output()
        .expect("arcw bench measures flow thread scheduling");

    assert!(
        output.status.success(),
        "thread scheduling bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "thread scheduling bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["task_requests_median"], 3);
    assert_eq!(measurement["native_io"]["scheduler"]["submitted"], 3);
    assert_eq!(measurement["native_io"]["scheduler"]["dispatch_sorts"], 0);
    assert_scheduler_completion_counts(measurement, 3);
    assert_native_bridge_phase_timings(measurement);
    assert_eq!(measurement["native_io"]["completed_tasks"], 3);
    assert!(
        measurement["deterministic"]["child_fiber_ticks_median"]
            .as_u64()
            .is_some_and(|ticks| ticks >= 3),
        "bench JSON should expose child-fiber activity ticks: {measurement}"
    );
    assert_eq!(
        measurement["deterministic"]["max_child_fibers_median"], 3,
        "bench JSON should expose peak flow child-fiber fanout: {measurement}"
    );
}

fn assert_scheduler_completion_counts(measurement: &serde_json::Value, expected_events: u64) {
    let scheduler = &measurement["native_io"]["scheduler"];
    assert_eq!(scheduler["dispatch_sort_items"], 0);
    assert_eq!(scheduler["completion_sorts"], 0);
    assert_eq!(scheduler["completion_sort_items"], 0);
    assert_eq!(scheduler["completion_events_in"], expected_events);
    assert_eq!(scheduler["completion_events_out"], expected_events);
    assert!(
        scheduler["completion_normalization_passes"]
            .as_u64()
            .is_some_and(|passes| passes >= 1),
        "bench JSON should expose completion normalization passes: {measurement}"
    );
    assert!(
        scheduler["completion_normalization_checks"]
            .as_u64()
            .is_some(),
        "bench JSON should expose completion normalization checks: {measurement}"
    );
    assert!(
        scheduler["completion_sort_skipped_items"]
            .as_u64()
            .is_some(),
        "bench JSON should expose skipped completion sort items: {measurement}"
    );
    assert_eq!(scheduler["joined_completion_events_emitted"], 0);
}

fn assert_native_bridge_phase_timings(measurement: &serde_json::Value) {
    let native_io = &measurement["native_io"];
    for field in [
        "scheduler_submit_elapsed_ns",
        "scheduler_dispatch_elapsed_ns",
        "host_complete_elapsed_ns",
        "event_build_elapsed_ns",
        "scheduler_complete_elapsed_ns",
    ] {
        assert!(
            native_io[field].as_u64().is_some(),
            "bench JSON should expose native bridge timing field `{field}`: {measurement}"
        );
    }
}

fn assert_aot_parallel_policy(measurement: &serde_json::Value, label: &str) {
    assert_eq!(
        measurement["deterministic"]["pure_parallel_policy_checks_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_parallel_batches_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_parallel_skipped_backend_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_parallel_skipped_small_median"],
        0
    );
    assert!(
        measurement["deterministic"]["pure_parallel_work_units_median"]
            .as_u64()
            .is_some_and(|work| work > 4),
        "{label} should report weighted parallel work: {measurement}"
    );
}

#[test]
fn bench_json_measures_checked_in_thread_scheduling_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/001_thread_scheduling.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("16")
        .arg("--max-ops")
        .arg("16")
        .arg("--mode")
        .arg("drain")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in thread scheduling fixture");

    assert!(
        output.status.success(),
        "checked-in thread scheduling bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "thread scheduling bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["native_io"]["scheduler"]["submitted"], 3);
    assert_eq!(measurement["native_io"]["scheduler"]["max_in_flight"], 3);
    assert_eq!(measurement["native_io"]["parallel_batches"], 0);
    assert_eq!(measurement["native_io"]["parallel_marker_tasks"], 0);
    assert_eq!(measurement["native_io"]["scheduler"]["dispatch_sorts"], 0);
    assert_eq!(measurement["native_io"]["scheduler"]["completion_sorts"], 0);
    assert_native_bridge_phase_timings(measurement);
    assert_eq!(
        measurement["native_io"]["scheduler"]["completion_events_in"],
        3
    );
    assert_eq!(
        measurement["native_io"]["scheduler"]["completion_events_out"],
        3
    );
    assert_eq!(
        measurement["native_io"]["scheduler"]["completion_sort_skipped_items"],
        3
    );
}

#[test]
fn bench_json_measures_checked_in_system_info_thread_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/004_system_info_threads.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("24")
        .arg("--max-ops")
        .arg("24")
        .arg("--mode")
        .arg("drain")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in system info thread fixture");

    assert!(
        output.status.success(),
        "checked-in system info thread bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "system info thread bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["task_requests_median"], 6);
    assert_eq!(measurement["deterministic"]["task_events_in_median"], 6);
    assert_eq!(measurement["deterministic"]["max_child_fibers_median"], 3);
    assert_eq!(measurement["native_io"]["system_info_ops"], 3);
    assert_eq!(measurement["native_io"]["completed_tasks"], 6);
    assert_eq!(measurement["native_io"]["parallel_batches"], 1);
    assert_eq!(measurement["native_io"]["parallel_system_info_tasks"], 3);
    assert_eq!(measurement["native_io"]["parallel_marker_tasks"], 3);
    assert!(
        measurement["native_io"]["parallel_workers"]
            .as_u64()
            .is_some_and(|workers| workers >= 1)
    );
    assert_eq!(measurement["native_io"]["scheduler"]["submitted"], 6);
    assert_eq!(measurement["native_io"]["scheduler"]["max_in_flight"], 6);
    assert_eq!(
        measurement["native_io"]["scheduler"]["submitted_by_class"]["cpu"],
        6
    );
    assert_eq!(
        measurement["native_io"]["scheduler"]["dispatched_by_class"]["cpu"],
        6
    );
    assert_eq!(
        measurement["native_io"]["scheduler"]["completed_by_class"]["cpu"],
        6
    );
    assert_eq!(measurement["native_io"]["scheduler"]["dispatch_sorts"], 0);
    assert_native_bridge_phase_timings(measurement);
}

#[test]
fn bench_json_measures_checked_in_inferred_pure_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/005_inferred_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in inferred pure fixture");

    assert!(
        output.status.success(),
        "checked-in inferred pure bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "inferred pure bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(
        measurement["executor_stats"]["pure_acceleration"]["annotated"],
        0
    );
    assert_eq!(
        measurement["executor_stats"]["pure_acceleration"]["inferred"],
        1
    );
    assert_eq!(measurement["executor_stats"]["pure_acceleration"]["jit"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 4);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
}

#[test]
fn bench_json_measures_checked_in_branching_iter_pure_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/007_branching_iter_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("6")
        .arg("--warmup")
        .arg("2")
        .arg("--steps")
        .arg("192")
        .arg("--max-ops")
        .arg("192")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in branching iter pure fixture");

    assert!(
        output.status.success(),
        "checked-in branching iter pure bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "branching iter pure bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 16);
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 32);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 32);
    assert_eq!(measurement["deterministic"]["line_effects_median"], 17);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        512
    );
}

#[test]
fn bench_json_measures_checked_in_scalar_for_pure_jit_perf_guard() {
    let path =
        workspace_root().join("tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in scalar for-loop pure JIT fixture");

    assert!(
        output.status.success(),
        "checked-in scalar for pure JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "scalar for pure JIT bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert!(
        json["source"].as_str().is_some_and(|source| {
            source.ends_with("003_for_pure_jit.arcw") && !source.contains(':')
        }),
        "bench source should stay path-free and identify the fixture: {json}"
    );
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(json["compiler"]["runtime_plan"]["pure_call_exprs"], 1);

    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_observational_bench_timings(measurement);
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 16);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 16);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        256
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );

    let compile = &measurement["executor_stats"]["pure_compile"];
    assert_eq!(compile["jit_attempts"], 1);
    assert_eq!(compile["jit_successes"], 1);
    assert_eq!(compile["jit_failures"], 0);
    assert_eq!(compile["cache_misses"], 0);
    assert!(
        compile["cache_hits"]
            .as_u64()
            .is_some_and(|hits| hits >= 16),
        "scalar JIT calls should stay cached after compile: {compile}"
    );
    assert!(
        compile["compile_elapsed_ns"].as_u64().is_some(),
        "JIT compile timing should remain observable but threshold-free: {compile}"
    );
}

#[test]
fn bench_json_measures_checked_in_mixed_for_iter_pure_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("2")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("1")
        .arg("--steps")
        .arg("128")
        .arg("--max-ops")
        .arg("128")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in mixed for/iter pure fixture");

    assert!(
        output.status.success(),
        "checked-in mixed for/iter pure bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "mixed for/iter pure bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        3
    );
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 40);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 3);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 24);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        3
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        24
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        192
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 40);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        320
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        96
    );
}

#[test]
fn bench_json_measures_checked_in_mixed_width_for_iter_pure_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("2")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("1")
        .arg("--steps")
        .arg("128")
        .arg("--max-ops")
        .arg("128")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in mixed width for/iter pure fixture");

    assert!(
        output.status.success(),
        "checked-in mixed width for/iter pure bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "mixed width for/iter pure bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 5);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "bytecode_vm");
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        5
    );
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 80);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 5);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 40);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        5
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        40
    );
    let row_width = std::mem::size_of::<i16>()
        + std::mem::size_of::<u16>()
        + std::mem::size_of::<isize>()
        + std::mem::size_of::<usize>()
        + std::mem::size_of::<f64>();
    let flat_input_bytes = 8 * 2 * row_width;
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        serde_json::json!(flat_input_bytes)
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 80);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_stack_packs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_copied_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        serde_json::json!(flat_input_bytes * 2)
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        serde_json::json!(8 * row_width)
    );
}

#[test]
fn bench_json_measures_checked_in_wide_for_pure_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/038_wide_for_pure_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in wide scalar pure JIT fixture");

    assert!(
        output.status.success(),
        "checked-in wide scalar pure JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "wide scalar pure JIT bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 2);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 16);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 16);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        512
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        2
    );
}

#[test]
fn bench_json_measures_checked_in_hot_for_pure_auto_jit_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/039_hot_for_pure_auto_jit.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("512")
        .arg("--max-ops")
        .arg("512")
        .arg("--pure-backend")
        .arg("auto")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in hot scalar Auto JIT fixture");

    assert!(
        output.status.success(),
        "checked-in hot scalar Auto JIT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "hot scalar Auto JIT bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 2);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_calls_median"], 256);
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 256);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_fallbacks_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        5120
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["auto_jit_deferred"],
        2
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["auto_jit_promotions"],
        2
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["jit_successes"],
        2
    );
}

#[test]
fn bench_json_measures_checked_in_large_map_pure_batch_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/008_large_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("auto")
        .arg("--pure-workers")
        .arg("4")
        .arg("--pure-batch-min-len")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in large map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in large map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "large map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(
        json["compiler"]["runtime_type_validation"]["expressions"],
        6
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(
        measurement["deterministic"]["pure_batch_items_median"],
        4096
    );
    assert_eq!(
        measurement["deterministic"]["pure_jit_calls_median"], 4096,
        "{stdout}"
    );
    assert_eq!(
        measurement["deterministic"]["pure_aot_calls_median"], 0,
        "{stdout}"
    );
    assert_eq!(
        measurement["executor_stats"]["pure_compile"]["auto_jit_promotions"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_thread_pool_jobs_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        16
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_nonuniform_map_pure_batch_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--pure-workers")
        .arg("4")
        .arg("--pure-batch-min-len")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in nonuniform map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in nonuniform map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "nonuniform map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(
        json["compiler"]["runtime_type_validation"]["expressions"],
        5
    );
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["pure_rewrite_expr_visits"],
        0
    );
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    assert_eq!(json["compiler"]["runtime_plan"]["pure_call_exprs"], 1);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        2048
    );
    assert_eq!(
        measurement["deterministic"]["pure_flatten_materializations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_flatten_bytes_copied_median"],
        0
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        2048
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_dense_i32_map_pure_batch_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/016_dense_i32_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense i32 map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense i32 map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense i32 map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        1024
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        1024
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_small_dense_integer_map_pure_batch_fixtures() {
    for (fixture, input_bytes) in [
        ("029_dense_i8_map_pure_batch.arcw", 32),
        ("030_dense_i16_map_pure_batch.arcw", 64),
        ("031_dense_u8_map_pure_batch.arcw", 32),
        ("032_dense_u16_map_pure_batch.arcw", 64),
    ] {
        let path = workspace_root()
            .join("tests/fixtures/arcw/spec_should_pass/bench")
            .join(fixture);
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("bench")
            .arg(&path)
            .arg("--iterations")
            .arg("3")
            .arg("--warmup")
            .arg("1")
            .arg("--samples")
            .arg("3")
            .arg("--steps")
            .arg("64")
            .arg("--max-ops")
            .arg("64")
            .arg("--pure-backend")
            .arg("jit")
            .arg("--json")
            .output()
            .unwrap_or_else(|error| panic!("arcw bench measures {fixture}: {error}"));

        assert!(
            output.status.success(),
            "{fixture} bench should succeed, stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains(&workspace_root().display().to_string()),
            "{fixture} bench JSON must not record the workspace path: {stdout}"
        );
        let json: serde_json::Value =
            serde_json::from_str(&stdout).expect("bench output is structured JSON");
        assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
        assert_eq!(
            json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
            1
        );
        let measurement = &json["benches"][0]["sections"][0]["measurement"];
        assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
        assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 16);
        assert_eq!(
            measurement["deterministic"]["pure_flat_batch_calls_median"],
            1
        );
        assert_eq!(
            measurement["deterministic"]["pure_flat_batch_items_median"],
            16
        );
        assert_eq!(
            measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
            input_bytes
        );
        assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 16);
        assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
        assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
        assert_eq!(
            measurement["deterministic"]["pure_arg_vec_allocations_median"],
            0
        );
        assert_eq!(
            measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
            input_bytes
        );
        assert_eq!(
            measurement["deterministic"]["pure_result_bytes_copied_median"],
            0
        );
    }
}

#[test]
fn bench_json_measures_checked_in_dense_f32_map_pure_batch_fixture_with_auto_jit() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/022_dense_f32_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense f32 map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense f32 map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense f32 map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        1024
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        1024
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        512
    );
}

#[test]
fn bench_json_measures_checked_in_dense_f64_map_pure_batch_fixture_with_auto_jit() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/023_dense_f64_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense f64 map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense f64 map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense f64 map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        2048
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        2048
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        1024
    );
}

#[test]
fn bench_json_measures_checked_in_matrix_f64_math_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--math-backend")
        .arg("ndarray")
        .arg("--value")
        .arg("lhs=matrix/f64/2x2:1.5,2,3.25,4.5")
        .arg("--value")
        .arg("rhs=matrix/f64/2x2:5,6.5,7,8.25")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in f64 matrix math fixture");

    assert!(
        output.status.success(),
        "checked-in f64 matrix math bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "f64 matrix math bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["math_calls_median"], 1);
    assert_eq!(
        measurement["deterministic"]["math_accelerated_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        32
    );
    assert_eq!(measurement["executor_stats"]["math"]["ndarray_calls"], 1);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_borrowed"], 64);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_copied"], 0);
    assert_eq!(
        measurement["executor_stats"]["math"]["last_backend"],
        "ndarray"
    );
}

#[test]
fn bench_json_measures_checked_in_tensor_f64_math_fixture() {
    let path =
        workspace_root().join("tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--math-backend")
        .arg("ndarray")
        .arg("--value")
        .arg("lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5")
        .arg("--value")
        .arg("rhs=tensor/f64/2x2:5,6.25,7.5,8.75")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in f64 tensor math fixture");

    assert!(
        output.status.success(),
        "checked-in f64 tensor math bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "f64 tensor math bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["math_calls_median"], 1);
    assert_eq!(
        measurement["deterministic"]["math_accelerated_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        32
    );
    assert_eq!(measurement["executor_stats"]["math"]["ndarray_calls"], 1);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_borrowed"], 64);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_copied"], 0);
    assert_eq!(
        measurement["executor_stats"]["math"]["last_backend"],
        "ndarray"
    );
}

#[test]
fn bench_json_measures_checked_in_matrix_add_f64_math_fixture() {
    let path =
        workspace_root().join("tests/fixtures/arcw/spec_should_pass/bench/035_matrix_add_f64.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--math-backend")
        .arg("ndarray")
        .arg("--value")
        .arg("lhs=matrix/f64/2x2:1.5,2.25,3.75,4.5")
        .arg("--value")
        .arg("rhs=matrix/f64/2x2:5,6.25,7.5,8.75")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in f64 matrix-add fixture");

    assert!(
        output.status.success(),
        "checked-in f64 matrix-add bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "f64 matrix-add bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["math_calls_median"], 1);
    assert_eq!(
        measurement["deterministic"]["math_accelerated_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        64
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        32
    );
    assert_eq!(measurement["executor_stats"]["math"]["ndarray_calls"], 1);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_borrowed"], 64);
    assert_eq!(measurement["executor_stats"]["math"]["bytes_copied"], 0);
    assert_eq!(
        measurement["executor_stats"]["math"]["last_backend"],
        "ndarray"
    );
}

#[test]
fn bench_json_measures_profile_inference_matmul_bias_adapter_fixture() {
    let scalar = profile_inference_matmul_bias_adapter_measurement("scalar");
    assert_eq!(scalar["deterministic"]["math_calls_median"], 1);
    assert_eq!(scalar["deterministic"]["math_accelerated_calls_median"], 0);
    assert_eq!(
        scalar["deterministic"]["pure_arg_bytes_borrowed_median"],
        40
    );
    assert_eq!(
        scalar["deterministic"]["pure_result_bytes_copied_median"],
        16
    );
    assert_eq!(
        scalar["executor_stats"]["math"]["fused_matmul_bias_add_calls"],
        1
    );
    assert_eq!(scalar["executor_stats"]["math"]["scalar_calls"], 1);
    assert_eq!(scalar["executor_stats"]["math"]["last_backend"], "scalar");

    let ndarray = profile_inference_matmul_bias_adapter_measurement("ndarray");
    assert_eq!(ndarray["deterministic"]["math_calls_median"], 1);
    assert_eq!(ndarray["deterministic"]["math_accelerated_calls_median"], 1);
    assert_eq!(
        ndarray["deterministic"]["pure_arg_bytes_borrowed_median"],
        40
    );
    assert_eq!(
        ndarray["deterministic"]["pure_result_bytes_copied_median"],
        16
    );
    assert_eq!(
        ndarray["executor_stats"]["math"]["fused_matmul_bias_add_calls"],
        1
    );
    assert_eq!(ndarray["executor_stats"]["math"]["ndarray_calls"], 1);
    assert_eq!(ndarray["executor_stats"]["math"]["bytes_borrowed"], 40);
    assert_eq!(ndarray["executor_stats"]["math"]["bytes_copied"], 0);
    assert_eq!(ndarray["executor_stats"]["math"]["last_backend"], "ndarray");
}

fn profile_inference_matmul_bias_adapter_measurement(math_backend: &str) -> serde_json::Value {
    let dir = temp_dir(&format!(
        "bench-profile-inference-matmul-bias-{math_backend}"
    ));
    let source = dir.join("infer_bench.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r"
bench @bench.infer_matmul_bias_add_f32 {
    measure iterations = 3 { goto @flow.infer_matmul_bias_add_f32 }
}

flow @flow.infer_matmul_bias_add_f32 infer_matmul_bias_add_f32(lhs: TensorF32, rhs: TensorF32, bias: TensorF32) -> TensorF32 {
    let out = infer.matmul_bias_add_f32(lhs, rhs, bias)
    return out
}
",
    )
    .expect("write inference adapter bench source");
    fs::write(
        &manifest,
        r#"
[profiles."bench.infer"]
kind = "bench"
source = "infer_bench.arcw"
adapter = "inference-tensor"
"#,
    )
    .expect("write inference adapter launch profile");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("bench.infer")
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("1")
        .arg("--max-ops")
        .arg("8")
        .arg("--math-backend")
        .arg(math_backend)
        .arg("--value")
        .arg("lhs=tensor/f32/2x2:1,2,3,4")
        .arg("--value")
        .arg("rhs=tensor/f32/2x2:5,6,7,8")
        .arg("--value")
        .arg("bias=tensor/f32/2:0.5,1.5")
        .arg("--json")
        .output()
        .expect("arcw bench measures profile-selected inference adapter call");
    fs::remove_dir_all(&dir).expect("remove temp inference bench project");

    assert!(
        output.status.success(),
        "profile inference matmul-bias bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "inference adapter bench JSON must not record the workspace path: {stdout}"
    );
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "inference adapter bench JSON must not record temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    json["benches"][0]["sections"][0]["measurement"].clone()
}

#[test]
fn bench_json_measures_checked_in_dense_u32_map_pure_batch_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/017_dense_u32_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense u32 map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense u32 map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense u32 map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        1024
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        1024
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_dense_u64_map_pure_batch_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/018_dense_u64_map_pure_batch.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense u64 map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense u64 map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense u64 map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        2048
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        2048
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_dense_i128_map_pure_batch_fixture() {
    assert_wide_integer_map_pure_batch_fixture(
        "tests/fixtures/arcw/spec_should_pass/bench/019_dense_i128_map_pure_batch.arcw",
    );
}

#[test]
fn bench_json_measures_checked_in_dense_u128_map_pure_batch_fixture() {
    assert_wide_integer_map_pure_batch_fixture(
        "tests/fixtures/arcw/spec_should_pass/bench/020_dense_u128_map_pure_batch.arcw",
    );
}

#[test]
fn bench_json_measures_checked_in_dense_target_size_integer_map_pure_batch_fixtures() {
    for fixture in [
        "036_dense_isize_map_pure_batch.arcw",
        "037_dense_usize_map_pure_batch.arcw",
    ] {
        assert_target_size_integer_map_pure_batch_fixture(
            &format!("tests/fixtures/arcw/spec_should_pass/bench/{fixture}"),
            fixture,
        );
    }
}

fn assert_wide_integer_map_pure_batch_fixture(relative_path: &str) {
    let path = workspace_root().join(relative_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense wide integer map pure batch fixture");

    assert!(
        output.status.success(),
        "checked-in dense wide integer map pure batch bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense wide integer map pure batch bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        4096
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        4096
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

fn assert_target_size_integer_map_pure_batch_fixture(relative_path: &str, label: &str) {
    let path = workspace_root().join(relative_path);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--pure-backend")
        .arg("jit")
        .arg("--json")
        .output()
        .unwrap_or_else(|error| panic!("arcw bench measures {label}: {error}"));

    assert!(
        output.status.success(),
        "{label} bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "{label} bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["runtime_plan"]["pure_helpers"], 1);
    assert_eq!(
        json["compiler"]["runtime_plan"]["sequence_map_sum_fusions"],
        1
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["pure_batch_calls_median"], 1);
    assert_eq!(measurement["deterministic"]["pure_batch_items_median"], 128);
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_calls_median"],
        1
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_items_median"],
        128
    );
    assert_eq!(
        measurement["deterministic"]["pure_flat_batch_bytes_borrowed_median"],
        2048
    );
    assert_eq!(measurement["deterministic"]["pure_jit_calls_median"], 128);
    assert_eq!(measurement["deterministic"]["pure_aot_calls_median"], 0);
    assert_eq!(measurement["deterministic"]["pure_vm_calls_median"], 0);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_bytes_borrowed_median"],
        2048
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_dense_integer_widths_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/012_dense_integer_widths_sum.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense integer widths fixture");

    assert!(
        output.status.success(),
        "checked-in dense integer widths bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense integer widths bench JSON must not record the workspace path: {stdout}"
    );
    for expected in ["Vec(I8)", "Vec(I16)", "Vec(I32)", "Vec(U8)"] {
        assert!(
            stdout.contains(expected),
            "dense integer widths bench should validate {expected}: {stdout}"
        );
    }
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 10);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
}

#[test]
fn bench_json_measures_checked_in_dense_scalar_len_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/013_dense_scalar_len.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense scalar len fixture");

    assert!(
        output.status.success(),
        "checked-in dense scalar len bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense scalar len bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert!(
        json["source"]
            .as_str()
            .is_some_and(|source| source.ends_with("013_dense_scalar_len.arcw")),
        "dense scalar len bench should report the fixture source without an absolute path: {stdout}"
    );
    assert_eq!(json["compiler"]["typecheck"]["warnings"], 0);
    assert_eq!(
        json["compiler"]["typecheck"]["judgment_rules"]["let_binding"],
        6
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 8);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
    assert!(
        measurement["elapsed_ns"]["median"]
            .as_u64()
            .is_some_and(|value| value > 0),
        "dense scalar len bench should record elapsed time: {stdout}"
    );
}

#[test]
fn bench_json_measures_checked_in_dense_textual_scalar_len_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/014_dense_textual_scalar_len.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense textual scalar len fixture");

    assert!(
        output.status.success(),
        "checked-in dense textual scalar len bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense textual scalar len bench JSON must not record the workspace path: {stdout}"
    );
    assert!(
        stdout.contains("Vec(String)"),
        "dense textual scalar len bench should validate String sequence typing: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert!(
        json["source"]
            .as_str()
            .is_some_and(|source| source.ends_with("014_dense_textual_scalar_len.arcw")),
        "dense textual scalar len bench should report the fixture source without an absolute path: {stdout}"
    );
    assert_eq!(json["compiler"]["typecheck"]["warnings"], 0);
    assert_eq!(
        json["compiler"]["typecheck"]["judgment_rules"]["let_binding"],
        5
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 7);
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
    assert!(
        measurement["elapsed_ns"]["median"]
            .as_u64()
            .is_some_and(|value| value > 0),
        "dense textual scalar len bench should record elapsed time: {stdout}"
    );
}

#[test]
fn bench_json_measures_checked_in_dense_wide_numeric_len_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/015_dense_wide_numeric_len.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--iterations")
        .arg("3")
        .arg("--warmup")
        .arg("1")
        .arg("--samples")
        .arg("3")
        .arg("--steps")
        .arg("64")
        .arg("--max-ops")
        .arg("64")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in dense wide numeric len fixture");

    assert!(
        output.status.success(),
        "checked-in dense wide numeric len bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "dense wide numeric len bench JSON must not record the workspace path: {stdout}"
    );
    for expected in ["Vec(I128)", "Vec(U128)", "Vec(ISize)", "Vec(USize)"] {
        assert!(
            stdout.contains(expected),
            "dense wide numeric len bench should validate {expected}: {stdout}"
        );
    }
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert!(
        json["source"]
            .as_str()
            .is_some_and(|source| source.ends_with("015_dense_wide_numeric_len.arcw")),
        "dense wide numeric len bench should report the fixture source without an absolute path: {stdout}"
    );
    assert_eq!(json["compiler"]["typecheck"]["warnings"], 0);
    assert_eq!(
        json["compiler"]["typecheck"]["judgment_rules"]["let_binding"],
        5
    );
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 7);
    assert_eq!(
        measurement["deterministic"]["pure_flatten_materializations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_arg_vec_allocations_median"],
        0
    );
    assert_eq!(
        measurement["deterministic"]["pure_result_bytes_copied_median"],
        0
    );
    assert!(
        measurement["elapsed_ns"]["median"]
            .as_u64()
            .is_some_and(|value| value > 0),
        "dense wide numeric len bench should record elapsed time: {stdout}"
    );
}

#[test]
fn bench_json_measures_checked_in_linear_aot_fixture() {
    let path =
        workspace_root().join("tests/fixtures/arcw/spec_should_pass/bench/006_linear_aot.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--executor")
        .arg("aot")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--steps")
        .arg("8")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in linear AOT fixture");

    assert!(
        output.status.success(),
        "checked-in linear AOT bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "linear AOT bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "aot");
    assert_eq!(measurement["executor_stats"]["aot_fast_path_ops"], 3);
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 3);
    assert_eq!(measurement["deterministic"]["line_effects_median"], 1);
}

#[test]
fn bench_json_measures_checked_in_mixed_aot_prefix_fixture() {
    let path = workspace_root()
        .join("tests/fixtures/arcw/spec_should_pass/bench/034_mixed_aot_prefix.arcw");
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&path)
        .arg("--executor")
        .arg("aot")
        .arg("--iterations")
        .arg("4")
        .arg("--warmup")
        .arg("1")
        .arg("--steps")
        .arg("8")
        .arg("--max-ops")
        .arg("8")
        .arg("--json")
        .output()
        .expect("arcw bench measures checked-in mixed AOT prefix fixture");

    assert!(
        output.status.success(),
        "checked-in mixed AOT prefix bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&workspace_root().display().to_string()),
        "mixed AOT prefix bench JSON must not record the workspace path: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    assert_eq!(json["compiler"]["aot"]["mixed_dispatch_flows"], 1);
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["executor"], "aot");
    assert_eq!(measurement["executor_stats"]["aot_fast_path_ops"], 2);
    assert_eq!(measurement["deterministic"]["executed_ops_median"], 5);
    assert_eq!(measurement["deterministic"]["line_effects_median"], 1);
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
        .arg("1")
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
    assert_eq!(runtime_batch["config"]["batch_min_len"], 1);
    assert_eq!(runtime_batch["compile"]["aot_successes"], 1);
    assert_eq!(runtime_batch["stats"]["batch_calls"], 2);
    assert_eq!(runtime_batch["stats"]["batch_items"], 8);
    assert_eq!(runtime_batch["stats"]["pure_calls"], 8);
    assert_eq!(runtime_batch["stats"]["aot_calls"], 8);
    assert_eq!(runtime_batch["stats"]["arg_stack_packs"], 0);
    assert_eq!(runtime_batch["stats"]["arg_vec_allocations"], 0);
    assert_eq!(runtime_batch["stats"]["arg_bytes_copied"], 0);
    assert!(
        runtime_batch["stats"]["arg_bytes_borrowed"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0),
        "runtime pure batch should report borrowed flat input bytes: {runtime_batch}"
    );
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
    measure iterations = 1 { goto @flow.bench }
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
    measure iterations = 1 { goto @flow.bench }
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
    measure iterations = 1 { goto @flow.bench_io }
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
    measure iterations = 1 { goto @flow.parallel_io }
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
            && stdout.contains("\"parallel_io_tasks\": 2")
            && stdout.contains("\"parallel_marker_tasks\": 0")
            && stdout.contains("\"write_ops\": 1"),
        "bench JSON should expose bounded traverse fanout counters: {stdout}"
    );
    assert_eq!(
        fs::read_to_string(save_dir.join("output.txt")).expect("read virtual output"),
        "done"
    );
}

#[test]
fn bench_json_measures_threaded_native_read_scheduling() {
    let dir = temp_dir("bench-threaded-native-read");
    let source_path = dir.join("bench.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
    fs::write(save_dir.join("a.txt"), "alpha").expect("seed input a");
    fs::write(save_dir.join("b.txt"), "beta").expect("seed input b");
    fs::write(
        &source_path,
        r#"
extern capability fs {
    type FsError
    fn read_text(path: VirtualPath) -> Need<String, FsError> effects { fs.read }
}
extern capability path { fn save(path: String) -> VirtualPath }

bench @bench.threaded_reads {
    measure iterations = 1 { goto @flow.threaded_reads }
}

flow @flow.threaded_reads threaded_reads effects { fs.read(save) } {
    thread left {
        let text = try await fs.read_text(path.save("a.txt")) with { error e => return "left_failed" }
        log.info(text)
    }
    thread right {
        let text = try await fs.read_text(path.save("b.txt")) with { error e => return "right_failed" }
        log.info(text)
    }
    return "done"
}
"#,
    )
    .expect("write threaded native read bench fixture");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("bench")
        .arg(&source_path)
        .arg("--iterations")
        .arg("1")
        .arg("--warmup")
        .arg("0")
        .arg("--steps")
        .arg("16")
        .arg("--max-ops")
        .arg("16")
        .arg("--mode")
        .arg("drain")
        .arg("--json")
        .output()
        .expect("arcw bench runs threaded native reads");

    assert!(
        output.status.success(),
        "threaded native read bench should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "threaded native read bench JSON must not record absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("bench output is structured JSON");
    let measurement = &json["benches"][0]["sections"][0]["measurement"];
    assert_eq!(measurement["deterministic"]["task_requests_median"], 4);
    assert_eq!(measurement["deterministic"]["task_events_in_median"], 4);
    assert_eq!(measurement["deterministic"]["max_child_fibers_median"], 2);
    assert_eq!(measurement["native_io"]["completed_tasks"], 4);
    assert_eq!(measurement["native_io"]["read_ops"], 2);
    assert_eq!(measurement["native_io"]["scheduler"]["submitted"], 4);
    assert_eq!(measurement["native_io"]["scheduler"]["dispatched"], 4);
    assert_eq!(measurement["native_io"]["scheduler"]["max_in_flight"], 4);
    assert_eq!(
        measurement["native_io"]["scheduler"]["completion_events_in"],
        4
    );
    assert_eq!(
        measurement["native_io"]["scheduler"]["completion_events_out"],
        4
    );
    assert_native_bridge_phase_timings(measurement);
    assert!(
        measurement["native_io"]["scheduler"]["completion_normalization_checks"]
            .as_u64()
            .is_some_and(|checks| checks >= 1),
        "threaded native reads should expose completion normalization checks: {measurement}"
    );
    assert_eq!(measurement["native_io"]["parallel_io_tasks"], 2);
    assert_eq!(measurement["native_io"]["parallel_system_info_tasks"], 0);
    assert_eq!(measurement["native_io"]["parallel_marker_tasks"], 2);
    assert!(
        measurement["native_io"]["parallel_batches"]
            .as_u64()
            .is_some_and(|batches| batches >= 1),
        "threaded native reads should expose parallel scheduler completion: {measurement}"
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
    measure iterations = 1 { goto @flow.write_actual }
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
            "Generate unsafe lifetime audit metadata",
        ),
        (
            "tests/fixtures/arcw/spec_should_fail/020_unsafe_block_missing_safety_doc_rejected.arcw",
            "Generate unsafe lifetime audit metadata",
        ),
    ] {
        let path = workspace_root().join(relative_path);
        let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
            .arg("verify")
            .arg(&path)
            .output()
            .expect("arcw verify runs");

        assert!(
            !output.status.success(),
            "{} must be rejected by arcw verify",
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
entry server @entry.server { goto @flow.main }

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
fn profile_check_loads_project_adapter_manifest() {
    let dir = temp_dir("profile-check-custom-adapter-manifest");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    let adapter_manifest = dir.join("custom-adapter.toml");
    fs::write(
        &source,
        r#"
flow @flow.opening opening {
    let body = custom.read(path = "opening.txt")
    return body
}
"#,
    )
    .expect("write source using custom adapter manifest");
    fs::write(
        &adapter_manifest,
        r#"
schema_version = 1
id = "custom-file"
display_name = "Custom File"
effects = ["custom.read"]

[[functions]]
name = "custom.read"
return_type = "String"
effects = ["custom.read"]
params = [{ name = "path", ty = "String" }]

[[host_calls]]
id = "custom.read"
effects = ["custom.read"]
"#,
    )
    .expect("write adapter manifest");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
adapter = "custom-file"
adapter_manifests = ["custom-adapter.toml"]
"#,
    )
    .expect("write launch manifest");

    let direct = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&source)
        .output()
        .expect("arcw direct check runs");
    let profiled = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(
        !direct.status.success(),
        "direct check should not see project adapter manifest, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );
    assert!(
        profiled.status.success(),
        "profiled check should load project adapter manifest, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&profiled.stdout),
        String::from_utf8_lossy(&profiled.stderr)
    );
    let stdout = String::from_utf8_lossy(&profiled.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "profile JSON must not record absolute temp paths: {stdout}"
    );
}

#[test]
fn profile_check_reports_adapter_manifest_read_error() {
    let dir = temp_dir("profile-check-adapter-manifest-read-error");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(&source, "flow @flow.opening opening { return \"ok\" }\n").expect("write source");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
adapter = "missing"
adapter_manifests = ["missing-adapter.toml"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load adapter manifest"));
    assert!(stderr.contains("failed to read adapter manifest"));
    assert!(stderr.contains("missing-adapter.toml"));
}

#[test]
fn profile_check_reports_adapter_manifest_parse_error() {
    let dir = temp_dir("profile-check-adapter-manifest-parse-error");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    let adapter_manifest = dir.join("bad-adapter.toml");
    fs::write(&source, "flow @flow.opening opening { return \"ok\" }\n").expect("write source");
    fs::write(&adapter_manifest, "schema_version = ").expect("write bad adapter manifest");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
adapter = "missing"
adapter_manifests = ["bad-adapter.toml"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load adapter manifest"));
    assert!(stderr.contains("failed to parse adapter manifest"));
    assert!(stderr.contains("bad-adapter.toml"));
}

#[test]
fn profile_check_loads_rust_metadata_for_extern_module() {
    let dir = temp_dir("profile-check-rust-metadata");
    let metadata_dir = dir.join("target").join("arcweft");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    let metadata = metadata_dir.join("truck_game.json");
    fs::create_dir_all(&metadata_dir).expect("create metadata dir");
    fs::write(
        &source,
        r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub type Rank
    pub fn score_to_rank(score: i32) -> Rank
}

flow @flow.opening opening {
    let rank = mini_games.truck.score_to_rank(score = 42i32)
    return "ok"
}
"#,
    )
    .expect("write source using rust metadata");
    fs::write(
        &metadata,
        truck_game_rust_manifest()
            .to_json_pretty()
            .expect("metadata encodes"),
    )
    .expect("write rust metadata");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
rust_metadata = ["target/arcweft/truck_game.json"]
"#,
    )
    .expect("write launch manifest");

    let direct = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg(&source)
        .output()
        .expect("arcw direct check runs");
    let profiled = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(
        !direct.status.success(),
        "direct check should not load profile metadata, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&direct.stdout),
        String::from_utf8_lossy(&direct.stderr)
    );
    assert!(
        profiled.status.success(),
        "profiled check should load rust metadata, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&profiled.stdout),
        String::from_utf8_lossy(&profiled.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&profiled.stdout).expect("profiled check output is JSON");
    assert!(
        json["phases"]
            .as_array()
            .expect("phases are reported")
            .iter()
            .any(|phase| phase["name"] == "rust_metadata"),
        "profiled check should report rust_metadata phase: {json}"
    );
}

#[test]
fn profile_check_reports_rust_metadata_read_error() {
    let dir = temp_dir("profile-check-rust-metadata-read-error");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    fs::write(
        &source,
        r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub fn score_to_rank(score: i32) -> i64
}

flow @flow.opening opening {
    let rank = mini_games.truck.score_to_rank(score = 42i32)
    return "ok"
}
"#,
    )
    .expect("write source using rust metadata");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
rust_metadata = ["target/arcweft/missing.json"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load Rust ABI metadata"));
    assert!(stderr.contains("failed to read Rust ABI metadata"));
    assert!(stderr.contains("missing.json"));
}

#[test]
fn profile_check_reports_rust_metadata_parse_error() {
    let dir = temp_dir("profile-check-rust-metadata-parse-error");
    let metadata_dir = dir.join("target").join("arcweft");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    let metadata = metadata_dir.join("bad.json");
    fs::create_dir_all(&metadata_dir).expect("create metadata dir");
    fs::write(
        &source,
        r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub fn score_to_rank(score: i32) -> i64
}

flow @flow.opening opening {
    let rank = mini_games.truck.score_to_rank(score = 42i32)
    return "ok"
}
"#,
    )
    .expect("write source using rust metadata");
    fs::write(&metadata, "{ not json").expect("write bad rust metadata");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
rust_metadata = ["target/arcweft/bad.json"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("check")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--json")
        .output()
        .expect("arcw check --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("failed to load Rust ABI metadata"));
    assert!(stderr.contains("failed to parse Rust ABI metadata"));
    assert!(stderr.contains("bad.json"));
}

#[test]
fn profile_json_loads_rust_metadata_for_extern_module() {
    let dir = temp_dir("profile-rust-metadata");
    let metadata_dir = dir.join("target").join("arcweft");
    let source = dir.join("game.arcw");
    let manifest = dir.join("arcw.toml");
    let metadata = metadata_dir.join("truck_game.json");
    fs::create_dir_all(&metadata_dir).expect("create metadata dir");
    fs::write(
        &source,
        r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub type Rank
    pub fn score_to_rank(score: i32) -> Rank
}

flow @flow.opening opening {
    return "ok"
}
"#,
    )
    .expect("write profile source using rust metadata");
    fs::write(
        &metadata,
        truck_game_rust_manifest()
            .to_json_pretty()
            .expect("metadata encodes"),
    )
    .expect("write rust metadata");
    fs::write(
        &manifest,
        r#"
[profiles.game]
kind = "game"
source = "game.arcw"
rust_metadata = ["target/arcweft/truck_game.json"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("profile")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("game")
        .arg("--steps")
        .arg("1")
        .arg("--json")
        .output()
        .expect("arcw profile --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(
        output.status.success(),
        "profile should load rust metadata, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("profile output is JSON");
    assert!(
        json["phases"]
            .as_array()
            .expect("phases are reported")
            .iter()
            .any(|phase| phase["name"] == "rust_metadata"),
        "profile should report rust_metadata phase: {json}"
    );
    assert!(
        !json
            .to_string()
            .contains(&std::env::temp_dir().display().to_string()),
        "profile JSON must not record absolute temp paths: {json}"
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
fn serve_profile_preserves_rust_metadata_when_adapter_comes_from_profile() {
    let dir = temp_dir("serve-profile-rust-metadata");
    let metadata_dir = dir.join("target").join("arcweft");
    let source = dir.join("server.arcw");
    let manifest = dir.join("arcw.toml");
    let metadata = metadata_dir.join("truck_game.json");
    fs::create_dir_all(&metadata_dir).expect("create metadata dir");
    fs::write(
        &source,
        r#"
extern rust mod mini_games::truck from crate "truck_game" {
    pub type Rank
    pub fn score_to_rank(score: i32) -> Rank
}

entry server @entry.http {
    route GET "/rank" -> @flow.rank
}

flow @flow.rank rank {
    let rank = mini_games.truck.score_to_rank(score = 42i32)
    return "ok"
}
"#,
    )
    .expect("write server profile source using rust metadata");
    fs::write(
        &metadata,
        truck_game_rust_manifest()
            .to_json_pretty()
            .expect("metadata encodes"),
    )
    .expect("write rust metadata");
    fs::write(
        &manifest,
        r#"
[profiles."server.rust"]
kind = "server"
source = "server.arcw"
entry = "http"
adapter = "native-http"
rust_metadata = ["target/arcweft/truck_game.json"]
"#,
    )
    .expect("write launch manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("serve")
        .arg("--manifest")
        .arg(&manifest)
        .arg("--profile")
        .arg("server.rust")
        .arg("--json")
        .output()
        .expect("arcw serve --profile runs");
    fs::remove_dir_all(&dir).expect("remove temp profile project");

    assert!(
        output.status.success(),
        "serve should load profile rust metadata when adapter is not a CLI override, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\"entry\": \"entry.http\"")
            && stdout.contains("\"adapter\": \"native-http\"")
            && stdout.contains("\"target\": \"flow.rank\""),
        "serve profile JSON should list rust metadata-backed route: {stdout}"
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
entry server @entry.http { goto @flow.main }

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
entry cli @entry.main { goto @flow.main }

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
    goto @flow.opening
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
