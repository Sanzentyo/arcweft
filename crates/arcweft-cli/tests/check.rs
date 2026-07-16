use std::collections::BTreeMap;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use arcweft_adapter_context::manifest::{
    AdapterEffectCapability, AdapterHostCall, AdapterManifest,
};
use arcweft_agent_protocol::ids::{PublicId, SessionId, StableHash};
use arcweft_core::task::{HostTaskRequest, TaskSpec};
use arcweft_core::value::RuntimePayload;
use arcweft_debug_model::chunk::{ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass};
use arcweft_debug_model::diagnostic::DebugDiagnostic;
use arcweft_debug_model::embedding::{EmbeddingModelDescriptor, StoredEmbedding};
use arcweft_debug_model::event::{DebugEvent, DebugEventKind};
use arcweft_debug_model::graph::{DebugGraphEdge, DebugGraphSymbol};
use arcweft_debug_model::history::DebugHistoryEntry;
use arcweft_debug_model::repl::DebugReplCell;
use arcweft_debug_model::script::DebugScriptRunOutcome;
use arcweft_debug_model::session::{DebugSession, DebugSessionStatus};
use arcweft_debug_model::sink::DebugEventSink;
use arcweft_debug_model::test_result::DebugTestResult;
use arcweft_debug_sqlite::store::DebugStore;
use arcweft_host_adapter::{HostAdapter, HostTaskMetrics, HostTaskOutcome};
use arcweft_runtime_host::{
    BundleRunnerOptions, NativeAdapterRegistrar, run_bundle_file_with_native_adapters,
};
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeRef,
    ArcweftRustVariant,
};

static CUSTOM_BUNDLE_ADAPTER_CALLS: AtomicUsize = AtomicUsize::new(0);
static CUSTOM_BUNDLE_ADAPTER_OUTPUT: Mutex<Option<PathBuf>> = Mutex::new(None);
static AGENT_MCP_STDIO_LOCK: Mutex<()> = Mutex::new(());

fn workspace_path(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(path)
}

fn run_agent_mcp_stdio(requests: &[serde_json::Value]) -> std::process::Output {
    let _guard = AGENT_MCP_STDIO_LOCK
        .lock()
        .expect("agent MCP stdio tests serialize native subprocesses");
    let mut child = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn arcw agent mcp");
    {
        let stdin = child.stdin.as_mut().expect("MCP stdin is piped");
        for request in requests {
            writeln!(
                stdin,
                "{}",
                serde_json::to_string(&request).expect("request serializes")
            )
            .expect("write MCP request");
        }
    }
    child.wait_with_output().expect("wait for MCP server")
}

fn agent_mcp_responses(stdout: &[u8]) -> Vec<serde_json::Value> {
    String::from_utf8_lossy(stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("MCP line is JSON"))
        .collect()
}

include!("check/agent_script_debug.rs");

fn current_unix_millis_for_test() -> i64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after unix epoch")
        .as_millis();
    i64::try_from(millis).unwrap_or(i64::MAX)
}

fn stable_hash(value: &str) -> StableHash {
    StableHash::new(value).expect("stable hash")
}

fn agent_script_cli_run_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-run-smoke.awfagent")
}

fn agent_script_cli_composite_wait_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-composite-wait-smoke.awfagent")
}

fn agent_script_cli_state_wait_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-state-wait-smoke.awfagent")
}

fn agent_script_cli_capture_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-capture-smoke.awfagent")
}

fn agent_script_cli_advance_text_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-advance-text-smoke.awfagent")
}

fn agent_script_cli_pointer_click_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-pointer-click-smoke.awfagent")
}

fn agent_script_cli_read_resource_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-read-resource-smoke.awfagent")
}

fn agent_script_cli_read_resource_metadata_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-read-resource-metadata-smoke.awfagent")
}

fn agent_script_cli_attach_resource_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-attach-resource-smoke.awfagent")
}

fn agent_script_cli_attach_capture_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-attach-capture-smoke.awfagent")
}

fn agent_script_cli_read_resource_value_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/cli-read-resource-value-smoke.awfagent")
}

fn agent_script_native_advance_text_game_path() -> PathBuf {
    workspace_path("samples/agent-script/native-advance-text-game.awfagent")
}

fn agent_script_native_flow_wait_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/native-flow-wait-smoke.awfagent")
}

fn agent_script_native_choice_dispatch_path() -> PathBuf {
    workspace_path("samples/agent-script/native-choice-dispatch.awfagent")
}

fn agent_script_native_choice_dispatch_source_path() -> PathBuf {
    workspace_path("samples/agent-script/native-choice-dispatch.arcw")
}

fn agent_script_native_invoke_action_path() -> PathBuf {
    workspace_path("samples/agent-script/native-invoke-action.awfagent")
}

fn agent_repl_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/repl-smoke.txt")
}

fn agent_repl_live_binding_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/repl-live-binding-smoke.txt")
}

fn agent_repl_inspection_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/repl-inspection-smoke.txt")
}

fn agent_repl_trace_readonly_smoke_path() -> PathBuf {
    workspace_path("samples/agent-script/repl-trace-readonly-smoke.txt")
}

fn agent_script_native_project_index_path() -> PathBuf {
    workspace_path("samples/agent-script/native-project-index.arcw")
}

fn rich_text_showcase_path() -> PathBuf {
    workspace_path("samples/rich-text-showcase.arcw")
}

fn image_animation_sample_path() -> PathBuf {
    workspace_path("samples/image-animation.arcw")
}

include!("check/toolchain_jit.rs");

#[path = "check/agent_observe_native.rs"]
mod agent_observe_native;

include!("check/cli_runtime_bench.rs");
include!("check/profile_entry_selection.rs");

fn temp_arcw(name: &str, source: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!("arcweft-cli-{name}-{}.arcw", std::process::id()));
    fs::write(&path, source).expect("write temp arcw fixture");
    path
}

fn temp_arcw_project(name: &str, source: &str) -> PathBuf {
    let root = temp_dir(name);
    fs::write(
        root.join("arcw.toml"),
        format!("[package]\nname = \"{name}\"\n"),
    )
    .expect("write temp project manifest");
    let source_dir = root.join("src");
    fs::create_dir_all(&source_dir).expect("create temp project source directory");
    let path = source_dir.join("main.arcw");
    fs::write(&path, source).expect("write temp project source");
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
    fs::create_dir(path.join("src")).expect("create temp project source dir");
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
    assert!(json["host_system"]["physical_cores"].as_u64().unwrap_or(0) > 0);
    assert!(json["host_system"]["logical_threads"].as_u64().unwrap_or(0) > 0);
    assert!(
        json["host_system"]["available_parallelism"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
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
    assert_runtime_plan_metrics(&compiler["runtime_plan"]);
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

fn assert_runtime_plan_metrics(runtime_plan: &serde_json::Value) {
    assert!(
        runtime_plan["optimized_flows"]
            .as_u64()
            .is_some_and(|value| value > 0)
            && runtime_plan["optimized_op_slices"]
                .as_u64()
                .is_some_and(|value| value > 0)
            && runtime_plan["local_use_tail_scans"].as_u64().is_some()
            && runtime_plan["pure_candidate_functions_seen"]
                .as_u64()
                .is_some()
            && runtime_plan["pure_candidate_lower_attempts"]
                .as_u64()
                .is_some()
            && runtime_plan["pure_expr_lowered_nodes"].as_u64().is_some()
            && runtime_plan["pure_expr_cloned_nodes"].as_u64().is_some()
            && runtime_plan["pure_rewrite_expr_visits"]
                .as_u64()
                .is_some_and(|value| value == 0),
        "runtime-plan lowering counters should be populated: {runtime_plan}"
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
        "state_delta_entries",
        "state_full_clones",
        "state_merge_keys",
        "boundary_checks",
        "escape_checks",
        "active_borrow_removes",
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

fn assert_observational_bench_timings(measurement: &serde_json::Value) {
    assert!(
        measurement["per_executed_op_ns"].as_u64().is_some(),
        "bench JSON should expose observational per-op timing: {measurement}"
    );
    let elapsed = &measurement["elapsed_ns"];
    assert!(
        elapsed["min"].as_u64().is_some()
            && elapsed["median"].as_u64().is_some()
            && elapsed["max"].as_u64().is_some(),
        "bench JSON should expose observational elapsed timing: {measurement}"
    );
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

fn truck_game_rust_manifest() -> ArcweftRustManifest {
    ArcweftRustManifest::builder(ArcweftRustPackage {
        name: "truck_game".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        name: "Rank".to_owned(),
        rust_path: "truck_game::Rank".to_owned(),
        kind: ArcweftRustTypeKind::Enum {
            variants: vec![ArcweftRustVariant {
                name: "Gold".to_owned(),
                fields: Vec::new(),
            }],
        },
    })
    .with_function(ArcweftRustFunction {
        name: "score_to_rank".to_owned(),
        rust_path: "truck_game::score_to_rank".to_owned(),
        params: vec![ArcweftRustParam {
            name: "score".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::Named {
            name: "Rank".to_owned(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
    .build()
}
