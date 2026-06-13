use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcweft_adapter_context::manifest::{
    AdapterEffectCapability, AdapterHostCall, AdapterManifest,
};
use arcweft_core::task::{HostTaskRequest, TaskSpec};
use arcweft_core::value::RuntimePayload;
use arcweft_host_adapter::{HostAdapter, HostTaskMetrics, HostTaskOutcome};
use arcweft_runtime_host::{
    BundleRunnerOptions, NativeAdapterRegistrar, run_bundle_file_with_native_adapters,
};
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeRef,
    ArcweftRustVariant,
};
use base64::{Engine as _, engine::general_purpose};

static CUSTOM_BUNDLE_ADAPTER_CALLS: AtomicUsize = AtomicUsize::new(0);
static CUSTOM_BUNDLE_ADAPTER_OUTPUT: Mutex<Option<PathBuf>> = Mutex::new(None);
static AGENT_MCP_STDIO_LOCK: Mutex<()> = Mutex::new(());

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
    let output = toolchain_profile_dry_run_output();

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
    assert!(json["host_system"]["physical_cores"].as_u64().unwrap_or(0) > 0);
    assert!(json["host_system"]["logical_threads"].as_u64().unwrap_or(0) > 0);
    assert!(
        json["host_system"]["available_parallelism"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert_eq!(json["commands"].as_array().unwrap().len(), 43);
    assert_eq!(json["commands"][0]["status"], "planned");
    assert_eq!(json["commands"][0]["repeat"], 2);
    assert_eq!(json["commands"][0]["warmup"], 1);
    assert_eq!(json["commands"][0]["elapsed_ns"], 0);
    assert_eq!(json["commands"][0]["timing"]["median"], 0);
    assert_eq!(
        json["commands"][0]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["commands"][0]["warmup_samples"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(json["commands"][0]["samples"].as_array().unwrap().len(), 2);
    assert_toolchain_profile_workspace_commands(&json);
    assert_toolchain_profile_bench_commands(&json);
    assert_toolchain_profile_math_commands(&json);
    assert_toolchain_profile_object_commands(&json);
    assert_toolchain_profile_width_commands(&json);
    assert_toolchain_profile_flow_math_commands(&json);
    assert_eq!(
        json["commands"][6]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(
        json["commands"][6]["samples"][0]["arcweft_bench"],
        serde_json::Value::Null
    );
    assert_eq!(json["commands"][8]["math_bench"], serde_json::Value::Null);
    assert_eq!(
        json["commands"][8]["samples"][0]["math_bench"],
        serde_json::Value::Null
    );
}

fn toolchain_profile_dry_run_output() -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
    command.arg("toolchain-profile");
    for profile_command in TOOLCHAIN_PROFILE_DRY_RUN_COMMANDS {
        command.arg("--command").arg(profile_command);
    }
    command
        .args(["--repeat", "2", "--warmup", "1", "--dry-run", "--json"])
        .output()
        .expect("arcw toolchain-profile dry-run runs")
}

const TOOLCHAIN_PROFILE_DRY_RUN_COMMANDS: &[&str] = &[
    "fmt",
    "check",
    "check-full",
    "clippy",
    "test-build",
    "test",
    "bench-003",
    "bench-009",
    "math-matmul-bias",
    "math-matrix-add",
    "math-tensor-add",
    "math-matmul-f64",
    "math-matrix-add-f64",
    "math-tensor-add-f64",
    "math-matmul-bias-wgpu-reuse",
    "math-matrix-add-wgpu-reuse",
    "math-tensor-add-wgpu-reuse",
    "math-matmul-auto-wgpu",
    "math-matmul-bias-auto-wgpu-reuse",
    "math-matrix-add-auto-wgpu-reuse",
    "math-tensor-add-auto-wgpu-reuse",
    "bench-009-aot-object",
    "bench-033-width-jit",
    "bench-033-width-aot",
    "bench-033-width-vm",
    "bench-040-width-jit",
    "bench-040-width-aot",
    "bench-040-width-vm",
    "bench-033-width-jit-release",
    "bench-033-width-aot-release",
    "bench-033-width-vm-release",
    "bench-040-width-jit-release",
    "bench-040-width-aot-release",
    "bench-040-width-vm-release",
    "bench-033-width-aot-object",
    "bench-040-width-aot-object",
    "flow-math-matmul-glam",
    "flow-math-matrix-add-ndarray",
    "flow-math-tensor-add-ndarray",
    "flow-math-matmul-f64-ndarray",
    "flow-math-matrix-add-f64-ndarray",
    "flow-math-tensor-add-f64-ndarray",
    "flow-math-matmul-auto-wgpu",
];

fn assert_toolchain_profile_workspace_commands(json: &serde_json::Value) {
    assert_eq!(
        json["commands"][0]["argv"],
        serde_json::json!(["cargo", "fmt", "--all", "--check"])
    );
    assert_eq!(
        json["commands"][2]["argv"],
        serde_json::json!([
            "cargo",
            "check",
            "--workspace",
            "--all-targets",
            "--all-features"
        ])
    );
    assert_eq!(
        json["commands"][3]["argv"],
        serde_json::json!([
            "cargo",
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features"
        ])
    );
    assert_eq!(
        json["commands"][4]["argv"],
        serde_json::json!(["cargo", "test", "--workspace", "--no-run"])
    );
}

fn assert_toolchain_profile_bench_commands(json: &serde_json::Value) {
    assert_eq!(json["commands"][6]["label"], "arcw_bench_003_for_pure_jit");
    assert_eq!(
        json["commands"][6]["argv"],
        toolchain_profile_bench_003_argv()
    );
    assert_eq!(
        json["commands"][7]["label"],
        "arcw_bench_009_nonuniform_map_pure_batch"
    );
    assert_eq!(
        json["commands"][7]["argv"],
        toolchain_profile_bench_009_argv()
    );
}

fn assert_toolchain_profile_math_commands(json: &serde_json::Value) {
    assert_eq!(json["commands"][8]["label"], "math_bench_matmul_bias_add");
    assert_eq!(
        json["commands"][8]["argv"],
        toolchain_profile_math_matmul_bias_argv()
    );
    assert_eq!(json["commands"][9]["label"], "math_bench_matrix_add");
    assert_eq!(
        json["commands"][9]["argv"],
        toolchain_profile_math_matrix_add_argv()
    );
    assert_eq!(json["commands"][10]["label"], "math_bench_tensor_add");
    assert_eq!(
        json["commands"][10]["argv"],
        toolchain_profile_math_tensor_add_argv()
    );
    assert_eq!(json["commands"][11]["label"], "math_bench_matmul_f64");
    assert_eq!(
        json["commands"][11]["argv"],
        toolchain_profile_math_matmul_f64_argv()
    );
    assert_eq!(json["commands"][12]["label"], "math_bench_matrix_add_f64");
    assert_eq!(
        json["commands"][12]["argv"],
        toolchain_profile_math_matrix_add_f64_argv()
    );
    assert_eq!(json["commands"][13]["label"], "math_bench_tensor_add_f64");
    assert_eq!(
        json["commands"][13]["argv"],
        toolchain_profile_math_tensor_add_f64_argv()
    );
    assert_eq!(
        json["commands"][14]["label"],
        "math_bench_matmul_bias_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][14]["argv"],
        toolchain_profile_math_matmul_bias_wgpu_reuse_argv()
    );
    assert_eq!(
        json["commands"][15]["label"],
        "math_bench_matrix_add_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][15]["argv"],
        toolchain_profile_math_matrix_add_wgpu_reuse_argv()
    );
    assert_eq!(
        json["commands"][16]["label"],
        "math_bench_tensor_add_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][16]["argv"],
        toolchain_profile_math_tensor_add_wgpu_reuse_argv()
    );
    assert_eq!(json["commands"][17]["label"], "math_bench_matmul_auto_wgpu");
    assert_eq!(
        json["commands"][17]["argv"],
        toolchain_profile_math_matmul_auto_wgpu_argv()
    );
    assert_eq!(
        json["commands"][18]["label"],
        "math_bench_matmul_bias_auto_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][18]["argv"],
        toolchain_profile_math_matmul_bias_auto_wgpu_reuse_argv()
    );
    assert_eq!(
        json["commands"][19]["label"],
        "math_bench_matrix_add_auto_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][19]["argv"],
        toolchain_profile_math_matrix_add_auto_wgpu_reuse_argv()
    );
    assert_eq!(
        json["commands"][20]["label"],
        "math_bench_tensor_add_auto_wgpu_reuse"
    );
    assert_eq!(
        json["commands"][20]["argv"],
        toolchain_profile_math_tensor_add_auto_wgpu_reuse_argv()
    );
}

fn assert_toolchain_profile_object_commands(json: &serde_json::Value) {
    assert_eq!(
        json["commands"][21]["label"],
        "arcw_bench_009_aot_object_artifacts"
    );
    assert_eq!(
        json["commands"][21]["argv"],
        toolchain_profile_bench_009_aot_object_argv()
    );
    assert_eq!(
        json["commands"][21]["arcweft_bench"],
        serde_json::Value::Null
    );
}

fn assert_toolchain_profile_width_commands(json: &serde_json::Value) {
    assert_toolchain_profile_width_debug_commands(json);
    assert_toolchain_profile_width_release_commands(json);
    assert_toolchain_profile_width_object_commands(json);
}

fn assert_toolchain_profile_width_debug_commands(json: &serde_json::Value) {
    assert_eq!(
        json["commands"][22]["label"],
        "arcw_bench_033_mixed_width_jit"
    );
    assert_eq!(
        json["commands"][22]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "jit"
        )
    );
    assert_eq!(
        json["commands"][23]["label"],
        "arcw_bench_033_mixed_width_aot"
    );
    assert_eq!(
        json["commands"][23]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "aot"
        )
    );
    assert_eq!(
        json["commands"][24]["label"],
        "arcw_bench_033_mixed_width_vm"
    );
    assert_eq!(
        json["commands"][24]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "vm"
        )
    );
    assert_eq!(
        json["commands"][25]["label"],
        "arcw_bench_040_mixed_width_jit"
    );
    assert_eq!(
        json["commands"][25]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "jit"
        )
    );
    assert_eq!(
        json["commands"][26]["label"],
        "arcw_bench_040_mixed_width_aot"
    );
    assert_eq!(
        json["commands"][26]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "aot"
        )
    );
    assert_eq!(
        json["commands"][27]["label"],
        "arcw_bench_040_mixed_width_vm"
    );
    assert_eq!(
        json["commands"][27]["argv"],
        toolchain_profile_width_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "vm"
        )
    );
}

fn assert_toolchain_profile_width_release_commands(json: &serde_json::Value) {
    assert_eq!(
        json["commands"][28]["label"],
        "arcw_bench_033_mixed_width_jit_release"
    );
    assert_eq!(
        json["commands"][28]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "jit"
        )
    );
    assert_eq!(
        json["commands"][29]["label"],
        "arcw_bench_033_mixed_width_aot_release"
    );
    assert_eq!(
        json["commands"][29]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "aot"
        )
    );
    assert_eq!(
        json["commands"][30]["label"],
        "arcw_bench_033_mixed_width_vm_release"
    );
    assert_eq!(
        json["commands"][30]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
            "vm"
        )
    );
    assert_eq!(
        json["commands"][31]["label"],
        "arcw_bench_040_mixed_width_jit_release"
    );
    assert_eq!(
        json["commands"][31]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "jit"
        )
    );
    assert_eq!(
        json["commands"][32]["label"],
        "arcw_bench_040_mixed_width_aot_release"
    );
    assert_eq!(
        json["commands"][32]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "aot"
        )
    );
    assert_eq!(
        json["commands"][33]["label"],
        "arcw_bench_040_mixed_width_vm_release"
    );
    assert_eq!(
        json["commands"][33]["argv"],
        toolchain_profile_width_release_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
            "vm"
        )
    );
}

fn assert_toolchain_profile_width_object_commands(json: &serde_json::Value) {
    assert_eq!(
        json["commands"][34]["label"],
        "arcw_bench_033_mixed_width_aot_object"
    );
    assert_eq!(
        json["commands"][34]["argv"],
        toolchain_profile_width_object_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw"
        )
    );
    assert_eq!(
        json["commands"][35]["label"],
        "arcw_bench_040_mixed_width_aot_object"
    );
    assert_eq!(
        json["commands"][35]["argv"],
        toolchain_profile_width_object_argv(
            "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw"
        )
    );
}

fn assert_toolchain_profile_flow_math_commands(json: &serde_json::Value) {
    assert_eq!(json["commands"][36]["label"], "arcw_flow_math_matmul_glam");
    assert_eq!(
        json["commands"][36]["argv"],
        toolchain_profile_flow_math_matmul_glam_argv()
    );
    assert_eq!(
        json["commands"][37]["label"],
        "arcw_flow_math_matrix_add_ndarray"
    );
    assert_eq!(
        json["commands"][37]["argv"],
        toolchain_profile_flow_math_matrix_add_ndarray_argv()
    );
    assert_eq!(
        json["commands"][38]["label"],
        "arcw_flow_math_tensor_add_ndarray"
    );
    assert_eq!(
        json["commands"][38]["argv"],
        toolchain_profile_flow_math_tensor_add_ndarray_argv()
    );
    assert_eq!(
        json["commands"][39]["label"],
        "arcw_flow_math_matmul_f64_ndarray"
    );
    assert_eq!(
        json["commands"][39]["argv"],
        toolchain_profile_flow_math_matmul_f64_ndarray_argv()
    );
    assert_eq!(
        json["commands"][40]["label"],
        "arcw_flow_math_matrix_add_f64_ndarray"
    );
    assert_eq!(
        json["commands"][40]["argv"],
        toolchain_profile_flow_math_matrix_add_f64_ndarray_argv()
    );
    assert_eq!(
        json["commands"][41]["label"],
        "arcw_flow_math_tensor_add_f64_ndarray"
    );
    assert_eq!(
        json["commands"][41]["argv"],
        toolchain_profile_flow_math_tensor_add_f64_ndarray_argv()
    );
    assert_eq!(
        json["commands"][42]["label"],
        "arcw_flow_math_matmul_auto_wgpu"
    );
    assert_eq!(
        json["commands"][42]["argv"],
        toolchain_profile_flow_math_matmul_auto_wgpu_argv()
    );
}

fn toolchain_profile_bench_003_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw",
        "--json",
        "--iterations",
        "15",
        "--warmup",
        "3",
        "--samples",
        "9",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--pure-backend",
        "jit"
    ])
}

fn toolchain_profile_bench_009_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw",
        "--json",
        "--iterations",
        "15",
        "--warmup",
        "3",
        "--samples",
        "9",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--pure-backend",
        "jit",
        "--pure-workers",
        "4",
        "--pure-batch-min-len",
        "64"
    ])
}

fn toolchain_profile_bench_009_aot_object_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/009_nonuniform_map_pure_batch.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--pure-backend",
        "aot",
        "--pure-workers",
        "4",
        "--pure-batch-min-len",
        "64",
        "--pure-object-artifacts"
    ])
}

fn toolchain_profile_width_argv(fixture: &str, backend: &str) -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        fixture,
        "--json",
        "--iterations",
        "2",
        "--warmup",
        "1",
        "--samples",
        "1",
        "--steps",
        "128",
        "--max-ops",
        "128",
        "--pure-backend",
        backend
    ])
}

fn toolchain_profile_width_release_argv(fixture: &str, backend: &str) -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        fixture,
        "--json",
        "--iterations",
        "2",
        "--warmup",
        "1",
        "--samples",
        "1",
        "--steps",
        "128",
        "--max-ops",
        "128",
        "--pure-backend",
        backend
    ])
}

fn toolchain_profile_width_object_argv(fixture: &str) -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        fixture,
        "--json",
        "--iterations",
        "2",
        "--warmup",
        "1",
        "--samples",
        "1",
        "--steps",
        "128",
        "--max-ops",
        "128",
        "--pure-backend",
        "aot",
        "--pure-object-artifacts"
    ])
}

fn toolchain_profile_flow_math_matmul_glam_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "glam",
        "--value",
        "lhs=matrix/f32/4x4:1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1",
        "--value",
        "rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2"
    ])
}

fn toolchain_profile_flow_math_matrix_add_ndarray_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/025_matrix_add_f32.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "ndarray",
        "--value",
        "lhs=matrix/f32/4x4:1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16",
        "--value",
        "rhs=matrix/f32/4x4:16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1"
    ])
}

fn toolchain_profile_flow_math_tensor_add_ndarray_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/026_tensor_add_f32.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "ndarray",
        "--value",
        "lhs=tensor/f32/2x2x2:1,2,3,4,5,6,7,8",
        "--value",
        "rhs=tensor/f32/2x2x2:8,7,6,5,4,3,2,1"
    ])
}

fn toolchain_profile_flow_math_matmul_f64_ndarray_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/027_matrix_matmul_f64.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "ndarray",
        "--value",
        "lhs=matrix/f64/2x2:1.5,2,3.25,4.5",
        "--value",
        "rhs=matrix/f64/2x2:5,6.5,7,8.25"
    ])
}

fn toolchain_profile_flow_math_matrix_add_f64_ndarray_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/035_matrix_add_f64.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "ndarray",
        "--value",
        "lhs=matrix/f64/2x2:1.5,2.25,3.75,4.5",
        "--value",
        "rhs=matrix/f64/2x2:5,6.25,7.5,8.75"
    ])
}

fn toolchain_profile_flow_math_tensor_add_f64_ndarray_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/028_tensor_add_f64.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "ndarray",
        "--value",
        "lhs=tensor/f64/2x2:1.5,2.25,3.75,4.5",
        "--value",
        "rhs=tensor/f64/2x2:5,6.25,7.5,8.75"
    ])
}

fn toolchain_profile_flow_math_matmul_auto_wgpu_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-cli",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/024_matrix_matmul_f32.arcw",
        "--json",
        "--iterations",
        "5",
        "--warmup",
        "2",
        "--samples",
        "5",
        "--steps",
        "64",
        "--max-ops",
        "64",
        "--math-backend",
        "auto",
        "--math-wgpu-min-elements",
        "1",
        "--value",
        "lhs=matrix/f32/8x8:1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1",
        "--value",
        "rhs=matrix/f32/8x8:2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2"
    ])
}

fn toolchain_profile_math_matmul_bias_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "matmul-bias-add",
        "--size",
        "64",
        "--iterations",
        "10",
        "--warmup",
        "2"
    ])
}

fn toolchain_profile_math_matrix_add_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "matrix-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1"
    ])
}

fn toolchain_profile_math_tensor_add_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "tensor-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1"
    ])
}

fn toolchain_profile_math_matmul_f64_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "matmul-f64",
        "--size",
        "64",
        "--iterations",
        "10",
        "--warmup",
        "2"
    ])
}

fn toolchain_profile_math_matrix_add_f64_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "matrix-add-f64",
        "--size",
        "1024",
        "--iterations",
        "5",
        "--warmup",
        "1"
    ])
}

fn toolchain_profile_math_tensor_add_f64_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--quiet",
        "--",
        "--backend",
        "all",
        "--op",
        "tensor-add-f64",
        "--size",
        "1024",
        "--iterations",
        "5",
        "--warmup",
        "1"
    ])
}

fn toolchain_profile_math_matmul_bias_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "wgpu",
        "--op",
        "matmul-bias-add",
        "--size",
        "128",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse"
    ])
}

fn toolchain_profile_math_matrix_add_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "wgpu",
        "--op",
        "matrix-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse"
    ])
}

fn toolchain_profile_math_tensor_add_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "wgpu",
        "--op",
        "tensor-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse"
    ])
}

fn toolchain_profile_math_matmul_auto_wgpu_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "auto",
        "--op",
        "matmul",
        "--size",
        "512",
        "--iterations",
        "3",
        "--warmup",
        "1"
    ])
}

fn toolchain_profile_math_matmul_bias_auto_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "auto",
        "--op",
        "matmul-bias-add",
        "--size",
        "128",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse",
        "--wgpu-min-elements",
        "1"
    ])
}

fn toolchain_profile_math_matrix_add_auto_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "auto",
        "--op",
        "matrix-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse",
        "--wgpu-min-elements",
        "1"
    ])
}

fn toolchain_profile_math_tensor_add_auto_wgpu_reuse_argv() -> serde_json::Value {
    serde_json::json!([
        "cargo",
        "run",
        "--release",
        "-p",
        "arcweft-runtime-accelerator",
        "--example",
        "math_bench",
        "--features",
        "math-wgpu",
        "--quiet",
        "--",
        "--backend",
        "auto",
        "--op",
        "tensor-add",
        "--size",
        "4096",
        "--iterations",
        "5",
        "--warmup",
        "1",
        "--reuse",
        "--wgpu-min-elements",
        "1"
    ])
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
    assert_eq!(pure["arg_stack_packs"], 0);
    assert_eq!(pure["arg_vec_allocations"], 0);
    assert_eq!(pure["arg_bytes_copied"], 0);
    assert_eq!(pure["arg_bytes_borrowed"], 16);
    assert_eq!(pure["result_bytes_copied"], 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(json["executor_stats"]["pure_config"]["workers"]["fixed"], 1);
    assert_eq!(json["executor_stats"]["pure_config"]["resolved_workers"], 1);
    assert_eq!(
        json["executor_stats"]["pure_config"]["worker_pool_active"],
        false
    );
    assert_eq!(json["executor_stats"]["pure_config"]["batch_min_len"], 2);
    assert_eq!(
        json["executor_stats"]["pure_config"]["emit_object_artifacts"],
        false
    );
    assert_eq!(
        json["executor_stats"]["pure_config"]["math_backend"],
        "auto"
    );
    assert_eq!(
        json["executor_stats"]["pure_config"]["math_wgpu_min_elements"],
        67_108_864
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
}

#[test]
fn run_json_can_record_aot_object_artifact_stats_when_requested() {
    let path = temp_arcw(
        "runtime-aot-object-artifacts",
        r"
#[pure]
fn score(base: i32, bonus: i32) -> i32 {
    return base * (bonus + 2i32)
}

flow @flow.main main {
    return score(3i32, 4i32)
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
        .arg("aot")
        .arg("--pure-object-artifacts")
        .output()
        .expect("arcw run executes runtime pure AOT source");
    fs::remove_file(&path).expect("remove temp runtime pure helper");

    assert!(
        output.status.success(),
        "runtime pure AOT object artifact run should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("run output is structured JSON");
    assert_eq!(json["final_status"], "done Return(\"18\")");
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "aot");
    assert_eq!(
        json["executor_stats"]["pure_config"]["emit_object_artifacts"],
        true
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["aot_successes"], 1);
    assert_eq!(json["executor_stats"]["pure_compile"]["object_attempts"], 1);
    assert_eq!(
        json["executor_stats"]["pure_compile"]["object_successes"],
        1
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["object_failures"], 0);
    assert!(
        json["executor_stats"]["pure_compile"]["object_bytes"]
            .as_u64()
            .is_some_and(|bytes| bytes > 0)
    );
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
        .arg("one-op")
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
    assert_eq!(sum_step_pure_counter(&json, "arg_stack_packs"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_vec_allocations"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_copied"), 0);
    assert_eq!(sum_step_pure_counter(&json, "arg_bytes_borrowed"), 64);
    assert_eq!(sum_step_pure_counter(&json, "result_bytes_copied"), 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(
        json["executor_stats"]["pure_config"]["worker_pool_active"],
        false
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
}

fn assert_agent_observe_object_capture_refs(object: &serde_json::Value) {
    assert_eq!(object["capture_refs"]["object_id_color"]["alpha"], 255);
    let object_id = object["id"].as_str().expect("object id is present");
    let captures = object["capture_refs"]["captures"]
        .as_array()
        .expect("object capture refs are listed");
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "color"
            && capture["mime_type"] == "image/png"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.png")))
    }));
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "object_id"
            && capture["mime_type"] == "image/png"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.object-id.png")))
    }));
    assert!(captures.iter().any(|capture| {
        capture["kind"] == "mask"
            && capture["mime_type"] == "application/octet-stream"
            && capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with(&format!("/object.{object_id}.mask.rgba")))
    }));
}

fn assert_agent_observe_rich_text_display_map(object: &serde_json::Value) {
    let text_runs = object["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .unwrap();
    assert!(text_runs.iter().any(|run| run["source"] == "interpolation"
        && run["range"]["start"] == 6
        && run["range"]["end"] == 9));
    assert!(text_runs.iter().any(|run| run["source"] == "ruby_base"
        && run["range"]["start"] == 10
        && run["range"]["end"] == 13));
    assert!(
        text_runs
            .iter()
            .any(|run| run["source"] == "control_hard_break"
                && run["range"]["start"] == 13
                && run["range"]["end"] == 14)
    );
    assert!(
        object["rich_text"]["display_map"]["ruby_annotations"]
            .as_array()
            .unwrap()
            .iter()
            .any(|ruby| ruby["ruby"] == "ゆめ"
                && ruby["base_range"]["start"] == 10
                && ruby["base_range"]["end"] == 13)
    );
}

fn assert_agent_observe_rich_text_display_report(json: &serde_json::Value) {
    assert_eq!(json["status"], "ok");
    assert_eq!(json["viewport"]["width"], 1280);
    assert_eq!(json["images"][0]["kind"], "overlay_svg");
    assert!(
        json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.starts_with("arcweft://session/cli/frame/"))
    );
    assert!(
        json["overlay_svg"]
            .as_str()
            .is_some_and(|svg| svg.contains("Hello Aoi"))
    );
    let object = &json["objects"][0];
    assert_eq!(object["role"], "textbox");
    assert_eq!(object["bbox"]["space"], "viewport");
    assert_eq!(object["text"], "Hello Aoi 夢\n");
    assert_agent_observe_object_capture_refs(object);
    assert_eq!(
        object["rich_text"]["base_styles"].as_array().unwrap().len(),
        4
    );
    assert!(
        object["rich_text"]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["kind"] == "ruby")
    );
    assert_agent_observe_rich_text_display_map(object);
    assert!(
        object["rich_text"]["host_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["kind"] == "voice")
    );
    assert_eq!(json["actions"][0]["action"], "advance_text");
    let objects = json["objects"].as_array().expect("objects are listed");
    assert_agent_observe_rich_text_child_objects(objects);
}

fn assert_agent_observe_rich_text_child_objects(objects: &[serde_json::Value]) {
    let run_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("interpolation run is observable as an element");
    assert_eq!(run_object["role"], "rich_text_run");
    assert_eq!(run_object["layer"], "dialogue.rich_text");
    assert_eq!(run_object["text"], "Aoi");
    assert_eq!(run_object["rich_text_ref"]["kind"], "text_run");
    assert_eq!(run_object["rich_text_ref"]["index"], 1);
    assert_eq!(run_object["rich_text_ref"]["source"], "interpolation");
    assert_eq!(run_object["rich_text_ref"]["range"]["start"], 6);
    assert_eq!(run_object["rich_text_ref"]["range"]["end"], 9);

    let ruby_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.ruby.0")
        .expect("ruby annotation is observable as an element");
    assert_eq!(ruby_object["role"], "rich_text_ruby");
    assert_eq!(ruby_object["text"], "夢 (ゆめ)");
    assert_eq!(ruby_object["rich_text_ref"]["kind"], "ruby");
    assert_eq!(ruby_object["rich_text_ref"]["index"], 0);
    assert_eq!(ruby_object["rich_text_ref"]["ruby"], "ゆめ");
    assert_eq!(ruby_object["rich_text_ref"]["range"]["start"], 10);
    assert_eq!(ruby_object["rich_text_ref"]["range"]["end"], 13);
    assert_agent_observe_object_capture_refs(ruby_object);
}

#[test]
fn agent_observe_json_reports_rich_text_display_objects() {
    let path = temp_arcw(
        "agent-observe-rich-text",
        r##"
pub dialogue defaults @dialogue.defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("?")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
    }
}

flow @flow.main main {
    let player = "Aoi"
    alice(color=rgb("#303132")): Hello #[player] |[夢](ゆめ)[r][voice auto][p]
}
"##,
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("overlay")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs rich text source");
    fs::remove_file(&path).expect("remove temp agent observe source");

    assert!(
        output.status.success(),
        "agent observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains(&std::env::temp_dir().display().to_string()),
        "agent observe JSON should not leak absolute temp paths: {stdout}"
    );
    let json: serde_json::Value =
        serde_json::from_str(&stdout).expect("agent observe output is structured JSON");
    assert_agent_observe_rich_text_display_report(&json);
}

#[test]
fn agent_observe_json_reports_rich_text_reset_controls_and_host_markers() {
    let path = temp_arcw(
        "agent-observe-rich-text-controls",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [color red]Hot[reset]Cool[w 500ms][mark .sync][clear][voice auto][p]
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe runs rich text controls source");
    fs::remove_file(&path).expect("remove temp agent observe controls source");

    assert!(
        output.status.success(),
        "agent observe controls should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("agent observe controls output is JSON");
    let object = &json["objects"][0];
    assert_eq!(object["text"], "HotCool");
    let runs = object["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are listed");
    let hot = runs
        .iter()
        .find(|run| run["range"]["start"] == 0 && run["range"]["end"] == 3)
        .expect("styled Hot run is reported");
    assert!(
        hot["styles"]
            .as_array()
            .unwrap()
            .iter()
            .any(|style| style["kind"] == "color")
    );
    let cool = runs
        .iter()
        .find(|run| run["range"]["start"] == 3 && run["range"]["end"] == 7)
        .expect("post-reset Cool run is reported");
    assert!(
        cool["styles"].as_array().unwrap().is_empty(),
        "reset should clear active inline styles for following display runs"
    );
    let controls = object["rich_text"]["display_map"]["controls"]
        .as_array()
        .expect("control markers are listed");
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "reset")
    );
    assert!(controls.iter().any(|control| {
        control["control"]["kind"] == "timed_wait" && control["control"]["value"] == "time=500ms"
    }));
    assert!(controls.iter().any(|control| {
        control["control"]["kind"] == "mark" && control["control"]["name"] == ".sync"
    }));
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "clear")
    );
    assert!(
        controls
            .iter()
            .any(|control| control["control"]["kind"] == "page")
    );
    assert!(
        object["rich_text"]["display_map"]["host_events"]
            .as_array()
            .unwrap()
            .iter()
            .any(|event| event["event"]["kind"] == "voice")
    );
}

#[test]
#[ignore = "tier 2 Agent observe resource matrix: slow multi-subprocess image/resource coverage"]
#[allow(clippy::too_many_lines)]
fn agent_observe_writes_layer_png_and_object_raw_images() {
    let path = temp_arcw(
        "agent-observe-image-capture",
        r##"
pub dialogue defaults @dialogue.defaults {
    font = serif
    text_color = rgb("#101112")
    inline_error = InlineFailure.fallback("?")
}

character @character.alice Alice as alice {
    dialogue_style {
        text_color = rgb("#202122")
    }
}

flow @flow.main main {
    let player = "Aoi"
    alice(color=rgb("#303132")): Hello #[player] |[夢](ゆめ)[r][voice auto][p]
}
"##,
    );
    let dir = temp_dir("agent-observe-image-capture");
    let png_path = dir.join("dialogue.png");
    let object_id_path = dir.join("dialogue-object-id.png");
    let raw_path = dir.join("object.rgba");
    let mask_path = dir.join("object-mask.rgba");

    let png_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes layer PNG");

    assert!(
        png_output.status.success(),
        "agent observe PNG capture should succeed, stderr: {}",
        String::from_utf8_lossy(&png_output.stderr)
    );
    let png_bytes = fs::read(&png_path).expect("read captured PNG");
    assert_eq!(&png_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let png_json: serde_json::Value =
        serde_json::from_slice(&png_output.stdout).expect("PNG capture report is JSON");
    assert_eq!(png_json["images"][0]["kind"], "color");
    assert_eq!(png_json["images"][0]["renderer"], "native");
    assert_eq!(png_json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(png_json["images"][0]["scope"]["id"], "dialogue");
    assert_eq!(png_json["images"][0]["composition"], "framebuffer_crop");
    assert_eq!(png_json["images"][0]["mime_type"], "image/png");
    assert_eq!(png_json["images"][0]["width"], 1088);
    assert_eq!(png_json["images"][0]["height"], 124);
    assert_eq!(png_json["images"][0]["crop_origin"]["space"], "viewport");
    assert_eq!(png_json["images"][0]["crop_origin"]["x"], 96);
    assert_eq!(png_json["images"][0]["crop_origin"]["y"], 548);
    assert!(png_json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        png_json["images"][0]["content_bbox"]["width"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(png_json["images"][0]["written"], "dialogue.png");
    assert!(
        png_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.png"))
    );
    let layers = png_json["layers"]
        .as_array()
        .expect("observation reports layers");
    let dialogue_layer = layers
        .iter()
        .find(|layer| layer["id"] == "dialogue")
        .expect("dialogue layer is observed");
    assert_eq!(dialogue_layer["bbox"]["space"], "viewport");
    assert_eq!(dialogue_layer["bbox"]["x"], 96);
    assert_eq!(dialogue_layer["bbox"]["y"], 548);
    assert!(
        dialogue_layer["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.mask.rgba"))
                && capture["mime_type"] == "application/octet-stream")
    );

    let object_id_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--capture")
        .arg("object-id")
        .arg("--layer")
        .arg("dialogue")
        .arg("--out")
        .arg(&object_id_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes object-id PNG");

    assert!(
        object_id_output.status.success(),
        "agent observe object-id capture should succeed, stderr: {}",
        String::from_utf8_lossy(&object_id_output.stderr)
    );
    let object_id_bytes = fs::read(&object_id_path).expect("read captured object-id PNG");
    assert_eq!(&object_id_bytes[..8], b"\x89PNG\r\n\x1a\n");
    let object_id_json: serde_json::Value =
        serde_json::from_slice(&object_id_output.stdout).expect("object-id report is JSON");
    assert_eq!(object_id_json["images"][0]["kind"], "object_id");
    assert_eq!(object_id_json["images"][0]["mime_type"], "image/png");
    assert!(
        object_id_json["images"][0]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        object_id_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.object-id.png"))
    );

    let image_resource_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns image resource");

    assert!(
        image_resource_output.status.success(),
        "agent observe image resource should succeed, stderr: {}",
        String::from_utf8_lossy(&image_resource_output.stderr)
    );
    let image_resource: serde_json::Value = serde_json::from_slice(&image_resource_output.stdout)
        .expect("image resource output is JSON");
    assert_eq!(image_resource["kind"], "image");
    assert_eq!(image_resource["mime_type"], "image/png");
    assert_eq!(image_resource["body"]["body_kind"], "bytes_base64");
    assert_eq!(image_resource["body"]["body"]["encoding"], "base64");
    assert!(
        image_resource["body"]["body"]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let mcp_image_resource_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mcp")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP image resource");

    assert!(
        mcp_image_resource_output.status.success(),
        "agent observe MCP image resource should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_image_resource_output.stderr)
    );
    let mcp_image_resource: serde_json::Value =
        serde_json::from_slice(&mcp_image_resource_output.stdout)
            .expect("MCP image resource output is JSON");
    assert_eq!(mcp_image_resource["contents"][0]["mimeType"], "image/png");
    assert!(
        mcp_image_resource["contents"][0]["blob"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );

    let mcp_resource_list_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--capture")
        .arg("object-id")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("all")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("list")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP resource list");

    assert!(
        mcp_resource_list_output.status.success(),
        "agent observe MCP resource list should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_resource_list_output.stderr)
    );
    let mcp_resource_list: serde_json::Value =
        serde_json::from_slice(&mcp_resource_list_output.stdout)
            .expect("MCP resource list output is JSON");
    let resources = mcp_resource_list["resources"]
        .as_array()
        .expect("MCP resource list contains resources");
    assert!(resources.iter().any(|resource| {
        resource["name"] == "latest.json" && resource["mimeType"] == "application/json"
    }));
    assert!(resources.iter().any(|resource| {
        resource["name"] == "layer.dialogue.object-id.png" && resource["mimeType"] == "image/png"
    }));
    assert!(resources.iter().any(|resource| {
        resource["name"] == "layer.dialogue.mask.rgba"
            && resource["mimeType"] == "application/octet-stream"
    }));
    assert!(resources.iter().any(|resource| {
        resource["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.ruby.0.png"))
            && resource["mimeType"] == "image/png"
    }));

    let mcp_tool_image_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--resource")
        .arg("image")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe returns MCP image tool result");

    assert!(
        mcp_tool_image_output.status.success(),
        "agent observe MCP image tool result should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_tool_image_output.stderr)
    );
    let mcp_tool_image: serde_json::Value = serde_json::from_slice(&mcp_tool_image_output.stdout)
        .expect("MCP image tool result output is JSON");
    assert_eq!(mcp_tool_image["isError"], false);
    assert_eq!(mcp_tool_image["content"][0]["type"], "text");
    let mcp_tool_image_metadata: serde_json::Value =
        serde_json::from_str(mcp_tool_image["content"][0]["text"].as_str().unwrap())
            .expect("image metadata content is JSON");
    assert_eq!(mcp_tool_image_metadata["image"]["width"], 1088);
    assert_eq!(mcp_tool_image_metadata["image"]["height"], 124);
    assert_eq!(mcp_tool_image_metadata["image"]["renderer"], "native");
    assert_eq!(mcp_tool_image_metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(mcp_tool_image_metadata["image"]["scope"]["id"], "dialogue");
    assert_eq!(
        mcp_tool_image_metadata["image"]["composition"],
        "framebuffer_crop"
    );
    assert_eq!(
        mcp_tool_image_metadata["image"]["crop_origin"]["space"],
        "viewport"
    );
    assert_eq!(mcp_tool_image_metadata["image"]["crop_origin"]["x"], 96);
    assert_eq!(mcp_tool_image_metadata["image"]["crop_origin"]["y"], 548);
    assert!(
        mcp_tool_image_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        mcp_tool_image_metadata["image"]["content_bbox"]["width"]
            .as_u64()
            .is_some_and(|width| width > 0)
    );
    assert!(
        mcp_tool_image_metadata["image"]["content_bbox"]["height"]
            .as_u64()
            .is_some_and(|height| height > 0)
    );
    assert_eq!(mcp_tool_image["content"][1]["type"], "image");
    assert_eq!(mcp_tool_image["content"][1]["mimeType"], "image/png");
    assert!(
        mcp_tool_image["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let read_mask_uri_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/object.object.dialogue.0.0.mask.rgba")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads object mask resource URI");

    assert!(
        read_mask_uri_output.status.success(),
        "agent observe read-uri mask should succeed, stderr: {}",
        String::from_utf8_lossy(&read_mask_uri_output.stderr)
    );
    let read_mask_resource: serde_json::Value =
        serde_json::from_slice(&read_mask_uri_output.stdout)
            .expect("read-uri mask resource output is JSON");
    assert_eq!(read_mask_resource["kind"], "image");
    assert_eq!(
        read_mask_resource["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.mask.rgba"
    );
    assert_eq!(read_mask_resource["mime_type"], "application/octet-stream");
    assert_eq!(read_mask_resource["body"]["body"]["encoding"], "base64");
    assert!(
        read_mask_resource["body"]["body"]["data"]
            .as_str()
            .is_some_and(|data| !data.is_empty())
    );

    let mcp_read_object_image_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/object.object.dialogue.0.0.png")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads object PNG as MCP tool result");

    assert!(
        mcp_read_object_image_output.status.success(),
        "agent observe read-uri MCP image should succeed, stderr: {}",
        String::from_utf8_lossy(&mcp_read_object_image_output.stderr)
    );
    let mcp_read_object_image: serde_json::Value =
        serde_json::from_slice(&mcp_read_object_image_output.stdout)
            .expect("read-uri MCP image output is JSON");
    assert_eq!(mcp_read_object_image["content"][0]["type"], "text");
    let mcp_read_object_metadata: serde_json::Value = serde_json::from_str(
        mcp_read_object_image["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("read-uri image metadata content is JSON");
    assert_eq!(mcp_read_object_metadata["image"]["width"], 1088);
    assert_eq!(mcp_read_object_metadata["image"]["height"], 124);
    assert!(
        mcp_read_object_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(mcp_read_object_image["content"][1]["type"], "image");
    assert_eq!(mcp_read_object_image["content"][1]["mimeType"], "image/png");
    assert!(
        mcp_read_object_image["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    let raw_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes object raw RGBA");

    assert!(
        raw_output.status.success(),
        "agent observe raw capture should succeed, stderr: {}",
        String::from_utf8_lossy(&raw_output.stderr)
    );
    let raw_bytes = fs::read(&raw_path).expect("read captured raw RGBA");
    let raw_json: serde_json::Value =
        serde_json::from_slice(&raw_output.stdout).expect("raw capture report is JSON");
    let width = raw_json["images"][0]["width"]
        .as_u64()
        .expect("raw capture width is integer");
    let height = raw_json["images"][0]["height"]
        .as_u64()
        .expect("raw capture height is integer");
    assert_eq!(raw_json["images"][0]["kind"], "color");
    assert_eq!(
        raw_json["images"][0]["mime_type"],
        "application/octet-stream"
    );
    assert_eq!(raw_json["images"][0]["written"], "object.rgba");
    assert!(
        raw_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.rgba"))
    );
    assert_eq!(
        raw_bytes.len(),
        usize::try_from(width * height * 4).expect("raw capture byte count fits usize")
    );
    assert!(
        raw_bytes.chunks_exact(4).any(|pixel| {
            pixel == [170, 190, 220, 255]
                || (pixel[0] >= 70
                    && pixel[1] >= 90
                    && pixel[2] > pixel[0]
                    && pixel[2] >= 120
                    && pixel[3] == 255)
        }),
        "raw rich-text capture should include ruby annotation-colored pixels"
    );

    let mask_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&mask_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes object mask raw RGBA");

    assert!(
        mask_output.status.success(),
        "agent observe mask capture should succeed, stderr: {}",
        String::from_utf8_lossy(&mask_output.stderr)
    );
    let mask_bytes = fs::read(&mask_path).expect("read captured mask RGBA");
    let mask_json: serde_json::Value =
        serde_json::from_slice(&mask_output.stdout).expect("mask capture report is JSON");
    assert_eq!(mask_json["images"][0]["kind"], "mask");
    assert_eq!(
        mask_json["images"][0]["mime_type"],
        "application/octet-stream"
    );
    assert!(
        mask_json["images"][0]["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.mask.rgba"))
    );
    assert!(
        mask_bytes
            .chunks_exact(4)
            .any(|pixel| pixel == [255, 255, 255, 255]),
        "object mask crop should include selected native geometry"
    );
    assert!(
        mask_bytes.chunks_exact(4).any(|pixel| pixel[3] == 0),
        "native object mask should preserve transparent non-glyph pixels"
    );

    let ruby_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png")
        .arg("--mcp")
        .arg("--mcp-format")
        .arg("tool-result")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads rich text ruby child PNG");
    assert!(
        ruby_output.status.success(),
        "agent observe read-uri ruby image should succeed, stderr: {}",
        String::from_utf8_lossy(&ruby_output.stderr)
    );
    let ruby_json: serde_json::Value =
        serde_json::from_slice(&ruby_output.stdout).expect("ruby image tool result is JSON");
    assert_eq!(ruby_json["content"][0]["type"], "text");
    let ruby_metadata: serde_json::Value =
        serde_json::from_str(ruby_json["content"][0]["text"].as_str().unwrap())
            .expect("ruby image metadata is JSON");
    assert_eq!(ruby_metadata["image"]["kind"], "color");
    assert!(ruby_metadata["image"]["width"].as_u64().unwrap() > 0);
    assert!(ruby_metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(ruby_json["content"][1]["type"], "image");
    assert_eq!(ruby_json["content"][1]["mimeType"], "image/png");
    assert!(
        ruby_json["content"][1]["data"]
            .as_str()
            .is_some_and(|data| data.starts_with("iVBORw0KGgo"))
    );

    fs::remove_file(&path).expect("remove temp agent observe source");
    fs::remove_dir_all(&dir).expect("remove temp capture dir");
}

#[test]
fn agent_observe_native_renderer_writes_framebuffer_png() {
    let path = temp_arcw(
        "agent-observe-native-renderer",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-renderer");
    let png_path = dir.join("native.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native PNG");

    assert!(
        output.status.success(),
        "native renderer capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native capture report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["composition"], "framebuffer");
    assert_eq!(json["images"][0]["mime_type"], "image/png");
    assert_eq!(json["images"][0]["width"], 1280);
    assert_eq!(json["images"][0]["height"], 720);
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    let content_bbox = &json["images"][0]["content_bbox"];
    let content_x = content_bbox["x"].as_u64().unwrap();
    let content_y = content_bbox["y"].as_u64().unwrap();
    let content_bottom = content_y + content_bbox["height"].as_u64().unwrap();
    assert!(
        content_x >= 96,
        "native Agent capture should align text with the observed textbox bbox"
    );
    assert!(
        (536..=672).contains(&content_y) && content_bottom <= 672,
        "native Agent capture should include ruby above the base text without leaving the dialogue bbox"
    );
    assert_eq!(json["images"][0]["written"], "native.png");

    fs::remove_file(&path).expect("remove temp native renderer source");
    fs::remove_dir_all(&dir).expect("remove temp native renderer dir");
}

#[test]
fn agent_observe_native_vertical_capture_matches_imq_reference() {
    if !imq_is_available() {
        eprintln!("skipping native vertical capture imq comparison: imq is not available");
        return;
    }

    let path = temp_arcw(
        "agent-observe-native-vertical-imq",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]吾輩は猫である。ABC 123 2026。[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-vertical-imq");
    let reference_path = dir.join("vertical-reference.png");
    let candidate_path = dir.join("vertical-candidate.png");

    let reference_json = capture_native_png_report(&path, &reference_path);
    let candidate_json = capture_native_png_report(&path, &candidate_path);
    assert_native_capture_has_content(&reference_json, "vertical-reference.png");
    assert_native_capture_has_content(&candidate_json, "vertical-candidate.png");

    let imq_output = Command::new("imq")
        .arg("image")
        .arg(&reference_path)
        .arg(&candidate_path)
        .arg("--format")
        .arg("json")
        .output()
        .expect("imq compares native vertical captures");
    assert!(
        imq_output.status.success(),
        "imq comparison should succeed, stderr: {}",
        String::from_utf8_lossy(&imq_output.stderr)
    );
    let imq_json: serde_json::Value =
        serde_json::from_slice(&imq_output.stdout).expect("imq output is JSON");
    assert_eq!(imq_json["dimensions"]["width"], 1280);
    assert_eq!(imq_json["dimensions"]["height"], 720);
    assert_metric_close(metric_score(&imq_json, "mse"), 0.0, 0.0, "mse");
    assert_metric_close(metric_score(&imq_json, "mae"), 0.0, 0.0, "mae");
    assert_metric_close(metric_score(&imq_json, "maxae"), 0.0, 0.0, "maxae");
    assert!(
        metric_score(&imq_json, "ssim") >= 0.999_999,
        "ssim should report identical native captures: {imq_json}"
    );
    assert_metric_close(
        metric_detail(&imq_json, "psnr", "mse"),
        0.0,
        0.0,
        "psnr.mse",
    );

    fs::remove_file(&path).expect("remove temp native vertical source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical dir");
}

#[test]
fn native_checked_in_visual_golden_fixtures_are_well_formed() {
    assert_native_golden_fixture_source(
        include_str!("../../../tests/fixtures/native_capture/vertical_tutr_golden.arcw"),
        "[.vertical_rl]",
        "vertical Tu/Tr golden source should exercise vertical_rl rich text",
    );
    assert_native_golden_fixture_source(
        include_str!(
            "../../../tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.arcw"
        ),
        "jlreq=loose",
        "loose JLREQ golden source should pin the loose preset",
    );
    assert_native_golden_fixture_source(
        include_str!(
            "../../../tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.arcw"
        ),
        "jlreq=normal",
        "normal JLREQ golden source should pin the normal preset",
    );
    let vertical_lr_ruby_text_combine_source = include_str!(
        "../../../tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.arcw"
    );
    assert_native_golden_fixture_source(
        vertical_lr_ruby_text_combine_source,
        "[.vertical_lr]",
        "vertical_lr ruby/text-combine golden source should exercise vertical_lr rich text",
    );
    assert!(
        vertical_lr_ruby_text_combine_source.contains("|[夢](ゆめ)[r]"),
        "vertical_lr ruby/text-combine golden source should exercise ruby annotation"
    );
    assert!(
        vertical_lr_ruby_text_combine_source.contains("2026"),
        "vertical_lr ruby/text-combine golden source should exercise text-combine digits"
    );

    let tutr = include_bytes!("../../../tests/fixtures/native_capture/vertical_tutr_golden.png");
    let loose = include_bytes!(
        "../../../tests/fixtures/native_capture/vertical_jlreq_preset_loose_golden.png"
    );
    let normal = include_bytes!(
        "../../../tests/fixtures/native_capture/vertical_jlreq_preset_normal_golden.png"
    );
    let vertical_lr_ruby_text_combine = include_bytes!(
        "../../../tests/fixtures/native_capture/vertical_lr_ruby_text_combine_golden.png"
    );
    for (label, golden) in [
        ("vertical Tu/Tr", tutr.as_slice()),
        ("loose JLREQ preset", loose.as_slice()),
        ("normal JLREQ preset", normal.as_slice()),
        (
            "vertical_lr ruby/text-combine",
            vertical_lr_ruby_text_combine.as_slice(),
        ),
    ] {
        assert_checked_in_native_png_golden(label, golden);
    }
    assert_ne!(
        loose.as_slice(),
        normal.as_slice(),
        "loose and normal JLREQ preset visual goldens should capture different column plans"
    );
}

fn assert_native_golden_fixture_source(source: &str, required_fragment: &str, context: &str) {
    assert!(
        source.contains(required_fragment),
        "{context}: missing `{required_fragment}`"
    );
    assert!(
        source.contains("MS Mincho"),
        "{context}: source should pin the Windows fixture font"
    );
    assert!(
        source.contains("[.vertical_rl") || source.contains("[.vertical_lr"),
        "{context}: source should exercise vertical Japanese text"
    );
}

fn assert_checked_in_native_png_golden(label: &str, golden: &[u8]) {
    assert_eq!(&golden[..8], b"\x89PNG\r\n\x1a\n", "{label}");
    assert_eq!(
        png_dimensions(golden),
        Some((1280, 720)),
        "checked-in {label} golden should stay at the Agent capture size"
    );
    assert!(
        golden.len() > 1024,
        "checked-in {label} golden should contain image data"
    );
}

#[test]
#[ignore = "tier 2 visual regression: exact PNG/imq golden is environment-sensitive"]
fn agent_observe_native_renderer_matches_checked_in_imq_golden_fixtures() {
    if !cfg!(windows) {
        eprintln!("skipping checked-in native visual golden comparisons: Windows font fixtures");
        return;
    }
    if !imq_is_available() {
        eprintln!("skipping checked-in native visual golden comparisons: imq is not available");
        return;
    }

    assert_checked_in_native_imq_golden(
        "vertical Tu/Tr",
        "vertical_tutr_golden.arcw",
        "vertical_tutr_golden.png",
        "vertical-tutr-candidate.png",
    );
    assert_checked_in_native_imq_golden(
        "loose JLREQ preset",
        "vertical_jlreq_preset_loose_golden.arcw",
        "vertical_jlreq_preset_loose_golden.png",
        "vertical-jlreq-preset-loose-candidate.png",
    );
    assert_checked_in_native_imq_golden(
        "normal JLREQ preset",
        "vertical_jlreq_preset_normal_golden.arcw",
        "vertical_jlreq_preset_normal_golden.png",
        "vertical-jlreq-preset-normal-candidate.png",
    );
    assert_checked_in_native_imq_golden(
        "vertical_lr ruby/text-combine",
        "vertical_lr_ruby_text_combine_golden.arcw",
        "vertical_lr_ruby_text_combine_golden.png",
        "vertical-lr-ruby-text-combine-candidate.png",
    );
}

fn assert_checked_in_native_imq_golden(
    label: &str,
    source_filename: &str,
    golden_filename: &str,
    candidate_filename: &str,
) {
    let fixture_dir = workspace_root().join("tests/fixtures/native_capture");
    let source_path = fixture_dir.join(source_filename);
    let golden_path = fixture_dir.join(golden_filename);
    let golden_bytes = fs::read(&golden_path).expect("read checked-in native visual golden");
    assert_checked_in_native_png_golden(label, &golden_bytes);

    let dir = temp_dir(&format!(
        "agent-observe-native-{}-golden",
        filesystem_safe_test_label(label)
    ));
    let candidate_path = dir.join(candidate_filename);
    let candidate_json = capture_native_png_report(&source_path, &candidate_path);
    assert_native_capture_has_content(&candidate_json, candidate_filename);

    let imq_output = Command::new("imq")
        .arg("image")
        .arg(&golden_path)
        .arg(&candidate_path)
        .arg("--metrics")
        .arg("psnr,ssim,mse,mae,maxae")
        .arg("--format")
        .arg("json")
        .output()
        .expect("imq compares checked-in native visual golden");
    assert!(
        imq_output.status.success(),
        "{label} imq checked-in golden comparison should succeed, stderr: {}",
        String::from_utf8_lossy(&imq_output.stderr)
    );
    let imq_json: serde_json::Value =
        serde_json::from_slice(&imq_output.stdout).expect("imq output is JSON");
    assert_eq!(imq_json["dimensions"]["width"], 1280);
    assert_eq!(imq_json["dimensions"]["height"], 720);
    assert!(
        metric_score(&imq_json, "mse") <= 0.002,
        "{label} visual golden mse drift should stay bounded: {imq_json}"
    );
    assert!(
        metric_score(&imq_json, "mae") <= 0.003,
        "{label} visual golden mae drift should stay bounded: {imq_json}"
    );
    assert_metric_close(
        metric_detail(&imq_json, "psnr", "mse"),
        metric_score(&imq_json, "mse"),
        0.0,
        "psnr.mse",
    );

    fs::remove_dir_all(&dir).expect("remove temp native visual golden dir");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_ruby_text_combine_geometry() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-ruby-combine",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]縦 |[夢](ゆめ)[r] 2026 ABC。[/][p]
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reports native vertical_lr rich-text geometry");

    fs::remove_file(&path).expect("remove temp native vertical_lr source");
    assert!(
        output.status.success(),
        "native vertical_lr rich-text observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native vertical_lr report is JSON");
    assert_native_vertical_lr_ruby_text_combine_report(&json);
}

fn assert_native_vertical_lr_ruby_text_combine_report(json: &serde_json::Value) {
    let image = &json["images"][0];
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "layer");
    assert_eq!(image["scope"]["id"], "dialogue.rich_text");
    assert_eq!(image["composition"], "isolated_regions");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);

    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox")
        .expect("textbox object is observed");
    let text_runs = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are reported");
    assert!(
        text_runs.iter().all(|run| {
            run["presentation"]["layout"]["writing_mode"]
                .as_str()
                .is_some_and(|mode| mode == "vertical_lr")
        }),
        "all display-map runs in the sample should preserve vertical_lr presentation"
    );
    assert!(
        text_runs.iter().any(|run| {
            run["range"]["start"].as_u64() == Some(8) && run["range"]["end"].as_u64() == Some(20)
        }),
        "the run containing 2026 should remain observable for text-combine geometry"
    );

    let objects = json["objects"].as_array().unwrap();
    let digit_run = objects
        .iter()
        .find(|object| object["role"] == "rich_text_run" && object["text"] == " 2026 ABC。")
        .expect("vertical text-combine run object is observed");
    assert_eq!(digit_run["rich_text_ref"]["source"], "text");
    assert!(
        digit_run["bbox"]["height"].as_u64().unwrap()
            > digit_run["bbox"]["width"].as_u64().unwrap(),
        "vertical_lr text-combine run geometry should be column-oriented"
    );
    let text_combine = find_rich_text_cluster_object(json, "2026", 9, 13);
    assert_eq!(text_combine["rich_text_ref"]["kind"], "glyph_cluster");
    let next_latin = find_rich_text_cluster_object(json, "A", 14, 15);
    assert!(
        text_combine["bbox"]["width"].as_u64().unwrap()
            <= next_latin["bbox"]["width"].as_u64().unwrap()
            && text_combine["bbox"]["height"].as_u64().unwrap()
                <= next_latin["bbox"]["height"].as_u64().unwrap(),
        "4-digit text-combine cluster should occupy one vertical cell: {text_combine}"
    );
    assert!(
        next_latin["bbox"]["x"].as_u64().unwrap() > text_combine["bbox"]["x"].as_u64().unwrap(),
        "vertical_lr text after a text-combine cluster should advance to the next column"
    );
    assert!(
        text_combine["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "text-combine cluster should expose native mask capture refs"
    );

    let ruby = objects
        .iter()
        .find(|object| object["role"] == "rich_text_ruby")
        .expect("vertical ruby child object is observed");
    assert_eq!(ruby["rich_text_ref"]["kind"], "ruby");
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert!(ruby["bbox"]["width"].as_u64().unwrap() > 0);
    assert!(ruby["bbox"]["height"].as_u64().unwrap() > 0);
}

#[test]
fn agent_observe_native_renderer_reports_vertical_column_progression_direction() {
    assert_native_vertical_column_progression_direction("vertical_rl", false);
    assert_native_vertical_column_progression_direction("vertical_lr", true);
}

fn assert_native_vertical_column_progression_direction(
    writing_mode: &str,
    next_column_moves_right: bool,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-column-progression"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬月火水木金土[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native vertical progression source");
    assert_native_rich_text_layer_image_has_content(&json);
    assert_eq!(
        first_text_run_presentation_layout(&json)["writing_mode"],
        writing_mode
    );

    let first_column_start = find_rich_text_cluster_object(&json, "天", 0, 3);
    let next_column_start = find_rich_text_cluster_object(&json, "夏", 9, 12);
    assert!(
        agent_json_bbox_y(&first_column_start["bbox"])
            .abs_diff(agent_json_bbox_y(&next_column_start["bbox"]))
            <= 1,
        "{writing_mode} next column should restart near the top inline origin"
    );
    if next_column_moves_right {
        assert!(
            agent_json_bbox_x(&next_column_start["bbox"])
                > agent_json_bbox_x(&first_column_start["bbox"]),
            "{writing_mode} next column should advance rightward: {first_column_start} / {next_column_start}"
        );
    } else {
        assert!(
            agent_json_bbox_x(&next_column_start["bbox"])
                < agent_json_bbox_x(&first_column_start["bbox"]),
            "{writing_mode} next column should advance leftward: {first_column_start} / {next_column_start}"
        );
    }
}

#[test]
fn agent_observe_native_renderer_reports_vertical_cluster_orientation_metadata() {
    let path = temp_arcw(
        "agent-observe-native-vertical-cluster-metadata",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]A。ー12[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native vertical cluster metadata source");
    assert_native_rich_text_layer_image_has_content(&json);

    assert_rich_text_cluster_metadata(&json, "A", 0, 1, "sideways_cw", "none");
    assert_rich_text_cluster_metadata(&json, "。", 1, 4, "upright", "upright_alternate");
    assert_rich_text_cluster_metadata(&json, "ー", 4, 7, "sideways_cw", "rotated_alternate");
    assert_rich_text_cluster_metadata(&json, "12", 7, 9, "text_combine_upright", "none");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_ruby_collision_geometry() {
    assert_native_vertical_ruby_collision_geometry("vertical_rl", true);
    assert_native_vertical_ruby_collision_geometry("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_reports_long_vertical_ruby_expansion_geometry() {
    assert_native_long_vertical_ruby_expansion_geometry("vertical_rl", true);
    assert_native_long_vertical_ruby_expansion_geometry("vertical_lr", false);
}

fn assert_native_long_vertical_ruby_expansion_geometry(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-expansion"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native long vertical ruby source");
    assert_native_rich_text_layer_image_has_content(&json);

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_rich_text_object_has_mask_capture(ruby, "long vertical ruby object");

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long vertical ruby should expand base allocation along inline progression: {ruby}"
    );
    assert!(
        agent_json_bbox_height(annotation) >= agent_json_bbox_height(base),
        "long vertical ruby annotation should share the expanded inline extent: {ruby}"
    );
    assert!(
        agent_json_bbox_y(base) < agent_json_bbox_y(&base_cluster["bbox"]),
        "expanded ruby base should be observable beyond the base glyph cell: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
            "vertical_rl long ruby annotation should be on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
            "vertical_lr long ruby annotation should be on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(base)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(annotation)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(base)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(base),
        "ruby object bbox should cover expanded base and annotation geometry: {ruby}"
    );
}

#[test]
fn agent_observe_native_renderer_reports_short_vertical_rl_ruby_at_edge() {
    let path = temp_arcw(
        "agent-observe-native-short-vertical-rl-ruby-edge",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春夏秋冬|[夢](ゆめ)[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native short vertical_rl ruby edge source");
    assert_native_rich_text_layer_image_has_content(&json);

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    assert_rich_text_object_has_mask_capture(ruby, "short vertical_rl ruby object");

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
        "short vertical_rl ruby annotation should stay on the right side of the base: {ruby}"
    );
    assert!(
        agent_json_bbox_right(annotation)
            <= json["viewport"]["width"].as_u64().expect("viewport width"),
        "short vertical_rl ruby annotation should remain inside the viewport: {ruby}"
    );
}

fn assert_native_vertical_ruby_collision_geometry(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-{writing_mode}-ruby-collision"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいよみ)|[星](ながいよみ)[/][p]
}}
"
        ),
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp native ruby collision source");
    assert_native_rich_text_layer_image_has_content(&json);

    let first = find_rich_text_ruby_object(&json, 0);
    let second = find_rich_text_ruby_object(&json, 1);
    let first_annotation = &first["rich_text_ref"]["ruby_annotation_bbox"];
    let second_annotation = &second["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        !agent_json_bboxes_intersect(first_annotation, second_annotation),
        "{writing_mode} adjacent ruby annotation bboxes should be separated: {first} / {second}"
    );

    for ruby in [first, second] {
        let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
        let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
        if ruby_on_right {
            assert!(
                agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
                "vertical_rl ruby annotation should be on the right side of the base: {ruby}"
            );
        } else {
            assert!(
                agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
                "vertical_lr ruby annotation should be on the left side of the base: {ruby}"
            );
        }
    }
}

#[test]
fn agent_observe_native_renderer_reports_expanded_jlreq_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-expanded-jlreq-pairs",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬月火…人[/][p]
}
",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reports expanded JLREQ pair geometry");

    fs::remove_file(&path).expect("remove temp expanded JLREQ source");
    assert!(
        output.status.success(),
        "native expanded JLREQ observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("expanded JLREQ report is JSON");
    assert_native_rich_text_layer_image_has_content(&json);

    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox")
        .expect("textbox object is observed");
    let run = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "normal");

    let fire = find_rich_text_cluster_object(&json, "火", 21, 24);
    let leader = find_rich_text_cluster_object(&json, "…", 24, 27);
    let person = find_rich_text_cluster_object(&json, "人", 27, 30);
    assert_eq!(
        fire["bbox"]["x"], leader["bbox"]["x"],
        "leader should stay in the same native-layout column as the previous cluster"
    );
    assert_eq!(
        leader["bbox"]["x"], person["bbox"]["x"],
        "following text should remain in the same observed column after the leader"
    );
    assert!(
        leader["bbox"]["y"].as_u64().unwrap() > fire["bbox"]["y"].as_u64().unwrap(),
        "leader should advance after the previous cluster within the column"
    );
    assert!(
        person["bbox"]["y"].as_u64().unwrap() > leader["bbox"]["y"].as_u64().unwrap(),
        "text after the leader should advance after the leader within the column"
    );
    assert!(
        leader["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "expanded JLREQ cluster should expose native mask capture refs"
    );
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_leader_mark_raw_crops() {
    assert_native_jlreq_leader_mark_raw_crop("mask");
    assert_native_jlreq_leader_mark_raw_crop("object-id");
}

fn assert_native_jlreq_leader_mark_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-leader-mark-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬月火…人[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-jlreq-leader-mark-{capture_kind}"
    ));
    let raw_path = dir.join(format!("native-jlreq-leader-mark-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.8.24.27")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ leader-mark raw crop");

    assert!(
        output.status.success(),
        "native JLREQ leader-mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ leader-mark report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let leader = assert_native_jlreq_leader_mark_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], leader["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], leader["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], leader["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], leader["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(leader),
            content_pixels,
            "JLREQ leader-mark object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ leader-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ leader-mark source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ leader-mark dir");
}

fn assert_native_jlreq_leader_mark_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let fire = find_rich_text_cluster_object(json, "火", 21, 24);
    let leader = find_rich_text_cluster_object(json, "…", 24, 27);
    let person = find_rich_text_cluster_object(json, "人", 27, 30);
    assert_eq!(leader["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(leader["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        fire,
        leader,
        "leader mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        leader,
        person,
        "text after leader mark should continue in the same column",
    );
    leader
}

#[test]
fn agent_observe_native_renderer_reports_expanded_jlreq_normal_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-expanded-jlreq-normal-pairs",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp expanded normal JLREQ source");
    assert_native_rich_text_layer_image_has_content(&json);

    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox")
        .expect("textbox object is observed");
    let run = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "normal");

    let mountain = find_rich_text_cluster_object(&json, "山", 18, 21);
    let iteration = find_rich_text_cluster_object(&json, "々", 21, 24);
    let person = find_rich_text_cluster_object(&json, "人", 24, 27);
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        iteration,
        person,
        "text after iteration mark should continue in the same column",
    );

    let open = find_rich_text_cluster_object(&json, "「", 27, 30);
    let close = find_rich_text_cluster_object(&json, "」", 30, 33);
    let river = find_rich_text_cluster_object(&json, "川", 33, 36);
    assert_vertical_cluster_after(open, close, "compact bracket pair should stay together");
    assert_vertical_cluster_after(
        close,
        river,
        "text after compact bracket pair should stay in the same column",
    );

    let large_kana = find_rich_text_cluster_object(&json, "あ", 36, 39);
    let small_kana = find_rich_text_cluster_object(&json, "っ", 39, 42);
    let next_kana = find_rich_text_cluster_object(&json, "い", 42, 45);
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana should stay out of a column head",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana should continue in the same column",
    );

    let vowel = find_rich_text_cluster_object(&json, "お", 45, 48);
    let prolonged_sound = find_rich_text_cluster_object(&json, "ー", 48, 51);
    let after_dash = find_rich_text_cluster_object(&json, "え", 51, 54);
    assert_vertical_cluster_after(
        vowel,
        prolonged_sound,
        "prolonged sound mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        prolonged_sound,
        after_dash,
        "text after prolonged sound mark should continue in the same column",
    );
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_prolonged_sound_raw_crops() {
    assert_native_jlreq_prolonged_sound_raw_crop("mask");
    assert_native_jlreq_prolonged_sound_raw_crop("object-id");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_small_kana_raw_crops() {
    assert_native_jlreq_small_kana_raw_crop("mask");
    assert_native_jlreq_small_kana_raw_crop("object-id");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_iteration_mark_raw_crops() {
    assert_native_jlreq_iteration_mark_raw_crop("mask");
    assert_native_jlreq_iteration_mark_raw_crop("object-id");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compact_bracket_raw_crops() {
    assert_native_jlreq_compact_bracket_raw_crop("mask");
    assert_native_jlreq_compact_bracket_raw_crop("object-id");
}

fn assert_native_jlreq_prolonged_sound_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-prolonged-sound-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-jlreq-prolonged-sound-{capture_kind}"
    ));
    let raw_path = dir.join(format!("native-jlreq-prolonged-sound-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.16.48.51")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ prolonged-sound raw crop");

    assert!(
        output.status.success(),
        "native JLREQ prolonged-sound {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ prolonged-sound report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let prolonged_sound = assert_native_jlreq_prolonged_sound_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        prolonged_sound["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        prolonged_sound["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], prolonged_sound["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        prolonged_sound["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(prolonged_sound),
            content_pixels,
            "JLREQ prolonged-sound object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ prolonged-sound mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ prolonged-sound source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ prolonged-sound dir");
}

fn assert_native_jlreq_small_kana_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-small-kana-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-jlreq-small-kana-{capture_kind}"
    ));
    let raw_path = dir.join(format!("native-jlreq-small-kana-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.13.39.42")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ small-kana raw crop");

    assert!(
        output.status.success(),
        "native JLREQ small-kana {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ small-kana report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let small_kana = assert_native_jlreq_small_kana_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        small_kana["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        small_kana["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], small_kana["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], small_kana["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(small_kana),
            content_pixels,
            "JLREQ small-kana object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ small-kana mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ small-kana source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ small-kana dir");
}

fn assert_native_jlreq_iteration_mark_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-iteration-mark-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-jlreq-iteration-mark-{capture_kind}"
    ));
    let raw_path = dir.join(format!("native-jlreq-iteration-mark-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.7.21.24")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ iteration-mark raw crop");

    assert!(
        output.status.success(),
        "native JLREQ iteration-mark {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native JLREQ iteration-mark report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let iteration = assert_native_jlreq_iteration_mark_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        iteration["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        iteration["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], iteration["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], iteration["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(iteration),
            content_pixels,
            "JLREQ iteration-mark object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ iteration-mark mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ iteration-mark source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ iteration-mark dir");
}

fn assert_native_jlreq_compact_bracket_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-compact-bracket-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬山々人「」川あっいおーえ[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-jlreq-compact-bracket-{capture_kind}"
    ));
    let raw_path = dir.join(format!("native-jlreq-compact-bracket-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.10.30.33")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ compact-bracket raw crop");

    assert!(
        output.status.success(),
        "native JLREQ compact-bracket {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ compact-bracket report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let close = assert_native_jlreq_compact_bracket_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], close["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], close["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], close["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], close["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(close),
            content_pixels,
            "JLREQ compact-bracket object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ compact-bracket mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp JLREQ compact-bracket source");
    fs::remove_dir_all(&dir).expect("remove temp JLREQ compact-bracket dir");
}

fn assert_native_jlreq_prolonged_sound_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let vowel = find_rich_text_cluster_object(json, "お", 45, 48);
    let prolonged_sound = find_rich_text_cluster_object(json, "ー", 48, 51);
    let after_dash = find_rich_text_cluster_object(json, "え", 51, 54);
    assert_eq!(
        prolonged_sound["rich_text_ref"]["orientation"],
        "sideways_cw"
    );
    assert_eq!(
        prolonged_sound["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_vertical_cluster_after(
        vowel,
        prolonged_sound,
        "prolonged sound mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        prolonged_sound,
        after_dash,
        "text after prolonged sound mark should continue in the same column",
    );
    prolonged_sound
}

fn assert_native_jlreq_small_kana_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let large_kana = find_rich_text_cluster_object(json, "あ", 36, 39);
    let small_kana = find_rich_text_cluster_object(json, "っ", 39, 42);
    let next_kana = find_rich_text_cluster_object(json, "い", 42, 45);
    assert_eq!(small_kana["rich_text_ref"]["orientation"], "upright");
    assert_eq!(
        small_kana["rich_text_ref"]["vertical_form"],
        "upright_alternate"
    );
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana should stay out of a column head",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana should continue in the same column",
    );
    small_kana
}

fn assert_native_jlreq_iteration_mark_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let mountain = find_rich_text_cluster_object(json, "山", 18, 21);
    let iteration = find_rich_text_cluster_object(json, "々", 21, 24);
    let person = find_rich_text_cluster_object(json, "人", 24, 27);
    assert_eq!(iteration["rich_text_ref"]["orientation"], "upright");
    assert_eq!(iteration["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark should stay with the previous cluster",
    );
    assert_vertical_cluster_after(
        iteration,
        person,
        "text after iteration mark should continue in the same column",
    );
    iteration
}

fn assert_native_jlreq_compact_bracket_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let open = find_rich_text_cluster_object(json, "「", 27, 30);
    let close = find_rich_text_cluster_object(json, "」", 30, 33);
    let river = find_rich_text_cluster_object(json, "川", 33, 36);
    assert_eq!(close["rich_text_ref"]["orientation"], "sideways_cw");
    assert_eq!(close["rich_text_ref"]["vertical_form"], "rotated_alternate");
    assert_vertical_cluster_after(open, close, "compact bracket pair should stay together");
    assert_vertical_cluster_after(
        close,
        river,
        "text after compact bracket pair should stay in the same column",
    );
    close
}

#[test]
fn agent_observe_native_renderer_reports_strict_jlreq_middle_dot_pair_geometry() {
    let path = temp_arcw(
        "agent-observe-native-strict-jlreq-middle-dot-pair",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=strict]天地春夏秋冬月火中・外[/][p]
}
",
    );

    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp strict JLREQ source");
    assert_native_rich_text_layer_image_has_content(&json);

    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox")
        .expect("textbox object is observed");
    let run = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .first()
        .expect("text run is observed");
    assert_eq!(run["presentation"]["layout"]["jlreq_strictness"], "strict");

    let inside = find_rich_text_cluster_object(&json, "中", 24, 27);
    let middle_dot = find_rich_text_cluster_object(&json, "・", 27, 30);
    let outside = find_rich_text_cluster_object(&json, "外", 30, 33);
    assert_vertical_cluster_after(
        inside,
        middle_dot,
        "strict middle-dot pair should stay in the same native-layout column",
    );
    assert_vertical_cluster_after(
        middle_dot,
        outside,
        "text after strict middle dot should remain in the same observed column",
    );
}

#[test]
fn agent_observe_native_renderer_writes_strict_jlreq_middle_dot_raw_crops() {
    assert_native_strict_jlreq_middle_dot_raw_crop("mask");
    assert_native_strict_jlreq_middle_dot_raw_crop("object-id");
}

fn assert_native_strict_jlreq_middle_dot_raw_crop(capture_kind: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-strict-jlreq-middle-dot-{capture_kind}"),
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=strict]天地春夏秋冬月火中・外[/][p]
}
",
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-strict-jlreq-middle-dot-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-strict-jlreq-middle-dot-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.9.27.30")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native strict JLREQ middle-dot raw crop");

    assert!(
        output.status.success(),
        "native strict JLREQ middle-dot {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native strict JLREQ middle-dot report is JSON");
    assert_eq!(json["images"][0]["kind"], capture_kind.replace('-', "_"));
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let middle_dot = assert_native_strict_jlreq_middle_dot_geometry(&json);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        middle_dot["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        middle_dot["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], middle_dot["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], middle_dot["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(middle_dot),
            content_pixels,
            "strict JLREQ middle-dot object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native strict JLREQ middle-dot mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp strict JLREQ middle-dot source");
    fs::remove_dir_all(&dir).expect("remove temp strict JLREQ middle-dot dir");
}

fn assert_native_strict_jlreq_middle_dot_geometry(json: &serde_json::Value) -> &serde_json::Value {
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "strict"
    );
    let inside = find_rich_text_cluster_object(json, "中", 24, 27);
    let middle_dot = find_rich_text_cluster_object(json, "・", 27, 30);
    let outside = find_rich_text_cluster_object(json, "外", 30, 33);
    assert_eq!(middle_dot["rich_text_ref"]["orientation"], "upright");
    assert_eq!(middle_dot["rich_text_ref"]["vertical_form"], "none");
    assert_vertical_cluster_after(
        inside,
        middle_dot,
        "strict middle-dot pair should stay in the same native-layout column",
    );
    assert_vertical_cluster_after(
        middle_dot,
        outside,
        "text after strict middle dot should remain in the same observed column",
    );
    middle_dot
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_punctuation_compression_and_hanging() {
    let hanging_path = temp_arcw(
        "agent-observe-native-jlreq-hanging-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地、人人[/][p]
}
",
    );
    let hanging = observe_native_rich_text_layer_report(&hanging_path);
    fs::remove_file(&hanging_path).expect("remove temp hanging punctuation source");
    assert_native_rich_text_layer_image_has_content(&hanging);

    let earth = find_rich_text_cluster_object(&hanging, "地", 3, 6);
    let comma = find_rich_text_cluster_object(&hanging, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(&hanging, "人", 9, 12);
    assert_eq!(
        earth["bbox"]["x"], comma["bbox"]["x"],
        "hanging punctuation should remain in the previous column"
    );
    assert!(
        agent_json_bbox_y(&comma["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "hanging punctuation should sit after the previous cluster"
    );
    assert!(
        agent_json_bbox_x(&next_person["bbox"]) < agent_json_bbox_x(&comma["bbox"])
            && agent_json_bbox_y(&next_person["bbox"]) < agent_json_bbox_y(&comma["bbox"]),
        "text after hanging punctuation should start the next vertical_rl column"
    );

    let compression_path = temp_arcw(
        "agent-observe-native-jlreq-punctuation-compression",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天、。人[/][p]
}
",
    );
    let compression = observe_native_rich_text_layer_report(&compression_path);
    fs::remove_file(&compression_path).expect("remove temp punctuation compression source");
    assert_native_rich_text_layer_image_has_content(&compression);

    let first = find_rich_text_cluster_object(&compression, "天", 0, 3);
    let comma = find_rich_text_cluster_object(&compression, "、", 3, 6);
    let period = find_rich_text_cluster_object(&compression, "。", 6, 9);
    let person = find_rich_text_cluster_object(&compression, "人", 9, 12);
    assert_eq!(first["bbox"]["x"], comma["bbox"]["x"]);
    assert_eq!(comma["bbox"]["x"], period["bbox"]["x"]);
    assert_eq!(period["bbox"]["x"], person["bbox"]["x"]);
    let body_advance = agent_json_bbox_y(&comma["bbox"]) - agent_json_bbox_y(&first["bbox"]);
    let compressed_advance = agent_json_bbox_y(&period["bbox"]) - agent_json_bbox_y(&comma["bbox"]);
    assert_eq!(
        compressed_advance * 2,
        body_advance,
        "compressed punctuation should advance by half a body cell"
    );
    assert_eq!(
        agent_json_bbox_y(&person["bbox"]) - agent_json_bbox_y(&period["bbox"]),
        compressed_advance,
        "following text should consume the space left by punctuation compression"
    );
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_line_end_prohibited_opening_punctuation() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-line-end-opening-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春「人外[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp JLREQ opening punctuation source");
    assert_native_rich_text_layer_image_has_content(&json);

    let spring = find_rich_text_cluster_object(&json, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(&json, "「", 9, 12);
    let person = find_rich_text_cluster_object(&json, "人", 12, 15);
    assert!(
        agent_json_bbox_x(&opening_bracket["bbox"]) < agent_json_bbox_x(&spring["bbox"]),
        "line-end-prohibited opening punctuation should move to the next vertical_rl column"
    );
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after opening punctuation should continue in the same moved column",
    );
    assert_eq!(
        opening_bracket["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    assert_rich_text_object_has_mask_capture(opening_bracket, "opening punctuation cluster");
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_jlreq_edge_geometry() {
    let opening_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-opening-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地春「人外[/][p]
}
",
    );
    let opening = observe_native_rich_text_layer_report(&opening_path);
    fs::remove_file(&opening_path).expect("remove temp vertical_lr JLREQ opening source");
    assert_native_rich_text_layer_image_has_content(&opening);

    let spring = find_rich_text_cluster_object(&opening, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(&opening, "「", 9, 12);
    let person = find_rich_text_cluster_object(&opening, "人", 12, 15);
    assert!(
        agent_json_bbox_x(&opening_bracket["bbox"]) > agent_json_bbox_x(&spring["bbox"]),
        "line-end-prohibited opening punctuation should move to the next vertical_lr column"
    );
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a vertical_lr column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after vertical_lr opening punctuation should continue in the same moved column",
    );
    assert_rich_text_object_has_mask_capture(opening_bracket, "vertical_lr opening punctuation");

    let hanging_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-hanging-punctuation",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地、人人[/][p]
}
",
    );
    let hanging = observe_native_rich_text_layer_report(&hanging_path);
    fs::remove_file(&hanging_path).expect("remove temp vertical_lr JLREQ hanging source");
    assert_native_rich_text_layer_image_has_content(&hanging);

    let earth = find_rich_text_cluster_object(&hanging, "地", 3, 6);
    let comma = find_rich_text_cluster_object(&hanging, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(&hanging, "人", 9, 12);
    assert_eq!(
        earth["bbox"]["x"], comma["bbox"]["x"],
        "vertical_lr hanging punctuation should remain in the previous column"
    );
    assert!(
        agent_json_bbox_y(&comma["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "vertical_lr hanging punctuation should sit after the previous cluster"
    );
    assert!(
        agent_json_bbox_x(&next_person["bbox"]) > agent_json_bbox_x(&comma["bbox"])
            && agent_json_bbox_y(&next_person["bbox"]) < agent_json_bbox_y(&comma["bbox"]),
        "text after vertical_lr hanging punctuation should start the next column"
    );

    let leader_path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-leader-chain",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr jlreq=normal]天地………終[/][p]
}
",
    );
    let leader = observe_native_rich_text_layer_report(&leader_path);
    fs::remove_file(&leader_path).expect("remove temp vertical_lr JLREQ leader source");
    assert_native_rich_text_layer_image_has_content(&leader);

    assert_eq!(
        first_text_run_presentation_layout(&leader)["jlreq_strictness"],
        "normal"
    );
    let first_leader = find_rich_text_cluster_object(&leader, "…", 6, 9);
    let second_leader = find_rich_text_cluster_object(&leader, "…", 9, 12);
    let ending = find_rich_text_cluster_object(&leader, "終", 15, 18);
    assert_vertical_cluster_after(
        first_leader,
        second_leader,
        "vertical_lr repeated leaders stay together in one trailing suffix",
    );
    assert!(
        agent_json_bbox_x(&ending["bbox"]) > agent_json_bbox_x(&second_leader["bbox"]),
        "vertical_lr text after a partially clipped overhanging leader chain should continue in the next column"
    );
    assert!(
        agent_json_bbox_y(&ending["bbox"]) < agent_json_bbox_y(&second_leader["bbox"]),
        "vertical_lr text after a leader chain should restart near the column top"
    );
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_preset_specific_column_geometry() {
    let loose = observe_native_jlreq_preset_fixture("loose", "preset-loose");
    let normal = observe_native_jlreq_preset_fixture("normal", "preset-normal");
    assert_native_rich_text_layer_image_has_content(&loose);
    assert_native_rich_text_layer_image_has_content(&normal);

    assert_eq!(
        first_text_run_presentation_layout(&loose)["jlreq_strictness"],
        "loose"
    );
    assert_eq!(
        first_text_run_presentation_layout(&normal)["jlreq_strictness"],
        "normal"
    );

    let loose_fire = find_rich_text_cluster_object(&loose, "火", 21, 24);
    let loose_first_leader = find_rich_text_cluster_object(&loose, "…", 24, 27);
    let loose_second_leader = find_rich_text_cluster_object(&loose, "…", 27, 30);
    let loose_person = find_rich_text_cluster_object(&loose, "人", 30, 33);
    let normal_fire = find_rich_text_cluster_object(&normal, "火", 21, 24);
    let normal_first_leader = find_rich_text_cluster_object(&normal, "…", 24, 27);
    let normal_second_leader = find_rich_text_cluster_object(&normal, "…", 27, 30);
    let normal_person = find_rich_text_cluster_object(&normal, "人", 30, 33);

    assert_eq!(
        loose_first_leader["bbox"]["x"], loose_second_leader["bbox"]["x"],
        "loose still keeps repeated leaders in one observed column"
    );
    assert_eq!(
        normal_first_leader["bbox"]["x"], normal_second_leader["bbox"]["x"],
        "normal keeps repeated leaders in one observed column"
    );
    assert!(
        agent_json_bbox_x(&normal_fire["bbox"]) > agent_json_bbox_x(&loose_fire["bbox"]),
        "normal preset should move the leader group to an earlier vertical_rl column than loose"
    );
    assert!(
        agent_json_bbox_x(&normal_first_leader["bbox"])
            > agent_json_bbox_x(&loose_first_leader["bbox"]),
        "normal preset should expose a different column for leader punctuation"
    );
    assert!(
        agent_json_bbox_x(&normal_person["bbox"]) > agent_json_bbox_x(&loose_person["bbox"]),
        "following text should inherit the preset-specific column plan"
    );
}

#[test]
fn agent_observe_native_renderer_reports_jlreq_paragraph_column_geometry() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-paragraph-column-geometry",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl jlreq=normal]天地春夏秋冬月火、山々人「川」あっいおーえ中・外………終[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp JLREQ paragraph source");
    assert_native_jlreq_paragraph_overview(&json);
    assert_native_jlreq_paragraph_compression_and_iteration(&json, false);
    assert_native_jlreq_paragraph_grouping_and_leaders(&json, false);
}

#[test]
fn agent_observe_native_renderer_reports_vertical_lr_jlreq_paragraph_column_geometry() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-jlreq-paragraph-column-geometry",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr jlreq=normal]天地春夏秋冬月火、山々人「川」あっいおーえ中・外………終[/][p]
}
",
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp vertical_lr JLREQ paragraph source");
    assert_native_jlreq_paragraph_overview(&json);
    assert_native_jlreq_paragraph_compression_and_iteration(&json, true);
    assert_native_jlreq_paragraph_grouping_and_leaders(&json, true);
}

fn assert_native_jlreq_paragraph_overview(json: &serde_json::Value) {
    assert_native_rich_text_layer_image_has_content(json);
    assert_eq!(
        first_text_run_presentation_layout(json)["jlreq_strictness"],
        "normal"
    );
    let run = find_rich_text_run_object(
        json,
        "天地春夏秋冬月火、山々人「川」あっいおーえ中・外………終",
    );
    assert!(
        run["bbox"]["width"].as_u64().unwrap() >= 300
            && run["bbox"]["height"].as_u64().unwrap() >= 120,
        "published-style JLREQ paragraph fixture should span multiple vertical columns: {run}"
    );
    assert!(
        rich_text_cluster_column_count(json) >= 6,
        "JLREQ paragraph fixture should expose a multi-column native plan: {json}"
    );
}

fn assert_native_jlreq_paragraph_compression_and_iteration(
    json: &serde_json::Value,
    next_column_moves_right: bool,
) {
    let fire = find_rich_text_cluster_object(json, "火", 21, 24);
    let comma = find_rich_text_cluster_object(json, "、", 24, 27);
    let mountain = find_rich_text_cluster_object(json, "山", 27, 30);
    assert_vertical_cluster_after(fire, comma, "paragraph comma follows body text");
    assert_eq!(
        comma["bbox"]["x"], mountain["bbox"]["x"],
        "text after a compressed comma should remain in the same planned column"
    );
    assert_eq!(
        (agent_json_bbox_y(&mountain["bbox"]) - agent_json_bbox_y(&comma["bbox"])) * 2,
        agent_json_bbox_y(&comma["bbox"]) - agent_json_bbox_y(&fire["bbox"]),
        "paragraph comma compression should be visible in native cluster geometry"
    );

    let iteration = find_rich_text_cluster_object(json, "々", 30, 33);
    let person = find_rich_text_cluster_object(json, "人", 33, 36);
    assert_vertical_cluster_after(
        mountain,
        iteration,
        "iteration mark stays attached in paragraph context",
    );
    assert_next_paragraph_column(
        iteration,
        person,
        next_column_moves_right,
        "text after an overhanging iteration mark should continue in the next paragraph column",
    );
}

fn assert_native_jlreq_paragraph_grouping_and_leaders(
    json: &serde_json::Value,
    next_column_moves_right: bool,
) {
    let open = find_rich_text_cluster_object(json, "「", 36, 39);
    let river = find_rich_text_cluster_object(json, "川", 39, 42);
    let close = find_rich_text_cluster_object(json, "」", 42, 45);
    assert_vertical_cluster_after(
        open,
        river,
        "paragraph bracket base follows opening bracket",
    );
    assert_vertical_cluster_after(
        river,
        close,
        "paragraph closing bracket stays with its base",
    );

    let large_kana = find_rich_text_cluster_object(json, "あ", 45, 48);
    let small_kana = find_rich_text_cluster_object(json, "っ", 48, 51);
    let next_kana = find_rich_text_cluster_object(json, "い", 51, 54);
    assert_vertical_cluster_after(
        large_kana,
        small_kana,
        "small kana stays out of a column head in paragraph context",
    );
    assert_vertical_cluster_after(
        small_kana,
        next_kana,
        "text after small kana continues in the same paragraph column",
    );

    let middle_dot = find_rich_text_cluster_object(json, "・", 66, 69);
    let outside = find_rich_text_cluster_object(json, "外", 69, 72);
    assert_eq!(
        middle_dot["bbox"]["x"], outside["bbox"]["x"],
        "text after a middle dot should remain in the same paragraph column"
    );
    assert!(
        agent_json_bbox_y(&outside["bbox"]) > agent_json_bbox_y(&middle_dot["bbox"]),
        "middle-dot compression should still advance paragraph text downward"
    );

    let first_leader = find_rich_text_cluster_object(json, "…", 72, 75);
    let second_leader = find_rich_text_cluster_object(json, "…", 75, 78);
    let ending = find_rich_text_cluster_object(json, "終", 81, 84);
    assert_vertical_cluster_after(
        first_leader,
        second_leader,
        "repeated leaders stay together in paragraph context",
    );
    assert_next_paragraph_column(
        second_leader,
        ending,
        next_column_moves_right,
        "paragraph text after a partially clipped overhanging leader chain should continue in the next column",
    );
    assert_rich_text_object_has_mask_capture(first_leader, "paragraph leader cluster");
}

fn assert_next_paragraph_column(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    next_column_moves_right: bool,
    context: &str,
) {
    if next_column_moves_right {
        assert!(
            agent_json_bbox_x(&next["bbox"]) > agent_json_bbox_x(&previous["bbox"]),
            "{context}: next column should advance rightward"
        );
    } else {
        assert!(
            agent_json_bbox_x(&next["bbox"]) < agent_json_bbox_x(&previous["bbox"]),
            "{context}: next column should advance leftward"
        );
    }
    assert!(
        agent_json_bbox_y(&next["bbox"]) < agent_json_bbox_y(&previous["bbox"]),
        "{context}: next column should restart near the column top"
    );
}

fn observe_native_jlreq_preset_fixture(strictness: &str, label: &str) -> serde_json::Value {
    let path = temp_arcw(
        &format!("agent-observe-native-jlreq-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.vertical_rl jlreq={strictness}]天地春夏秋冬月火……人[/][p]
}}
"
        ),
    );
    let json = observe_native_rich_text_layer_report(&path);
    fs::remove_file(&path).expect("remove temp preset JLREQ source");
    json
}

#[test]
fn agent_observe_native_renderer_reports_windows_fonts_sample_vertical_rl_geometry() {
    let source_path = workspace_root().join("samples/rich-text-windows-fonts.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    let run = find_rich_text_run_object(
        &json,
        "縦書きの見本。吾輩は猫である。ABC 123 2026。春夏秋冬、朝昼夕夜、天地左右。",
    );
    assert_eq!(run["entity"], "sen.say");
    assert_eq!(run["rich_text"]["line"], "say.windows_fonts.001");
    assert_eq!(run["rich_text_ref"]["range"]["start"], 0);
    assert_eq!(run["rich_text_ref"]["range"]["end"], 105);
    assert!(
        run["bbox"]["height"].as_u64().unwrap() >= 120,
        "sample vertical_rl run should occupy multiple vertical cells: {run}"
    );
    assert!(
        run["bbox"]["width"].as_u64().unwrap() <= 400,
        "sample vertical_rl run should be column-shaped rather than one long horizontal line: {run}"
    );
    assert!(
        run["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba")))
    );
}

#[test]
fn agent_observe_native_renderer_reports_full_grammar_sample_vertical_inference_geometry() {
    let source_path = workspace_root().join("samples/rich-text-full-grammar.arcw");
    let json = observe_native_rich_text_layer_report(&source_path);

    assert_native_rich_text_layer_image_has_content(&json);
    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox" && object["rich_text"]["line"] == "say.full.005")
        .expect("target textbox object is observed");
    let vertical_rl_display_run = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .unwrap()
        .iter()
        .find(|run| {
            run["range"]["start"].as_u64() == Some(27) && run["range"]["end"].as_u64() == Some(63)
        })
        .expect("vertical_rl display-map run is observed");
    assert_eq!(
        vertical_rl_display_run["presentation"]["layout"]["jlreq_strictness"],
        "strict"
    );
    let vertical_rl = find_rich_text_run_object(&json, "吾輩は猫である。ABC 123 2026");
    assert_eq!(vertical_rl["entity"], "bob.say");
    assert_eq!(vertical_rl["rich_text"]["line"], "say.full.005");
    assert_eq!(vertical_rl["rich_text_ref"]["range"]["start"], 27);
    assert_eq!(vertical_rl["rich_text_ref"]["range"]["end"], 63);
    assert!(
        vertical_rl["bbox"]["height"].as_u64().unwrap() >= 120,
        "full grammar vertical_rl run should preserve column geometry: {vertical_rl}"
    );
    assert!(
        vertical_rl["bbox"]["width"].as_u64().unwrap() <= 260,
        "full grammar vertical_rl run should not flatten into a horizontal line: {vertical_rl}"
    );
    let first_vertical_cluster = find_rich_text_cluster_object(&json, "吾", 27, 30);
    assert_eq!(
        first_vertical_cluster["rich_text_ref"]["kind"],
        "glyph_cluster"
    );
    assert_eq!(first_vertical_cluster["rich_text_ref"]["source"], "text");
    assert_eq!(first_vertical_cluster["rich_text"]["line"], "say.full.005");
    assert_eq!(first_vertical_cluster["bbox"]["width"], 42);
    assert_eq!(first_vertical_cluster["bbox"]["height"], 42);
    assert!(
        first_vertical_cluster["capture_refs"]["captures"]
            .as_array()
            .unwrap()
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba")))
    );
    let first_vertical_cluster_mask_uri =
        rich_text_object_capture_uri(first_vertical_cluster, "mask", "application/octet-stream");
    assert_agent_read_uri_object_image_has_content(
        &source_path,
        first_vertical_cluster_mask_uri,
        first_vertical_cluster["id"].as_str().unwrap(),
        42,
        42,
    );

    let vertical_lr = find_rich_text_run_object(&json, "縦LR");
    assert_eq!(vertical_lr["rich_text"]["line"], "say.full.005");
    assert_eq!(vertical_lr["rich_text_ref"]["range"]["start"], 66);
    assert_eq!(vertical_lr["rich_text_ref"]["range"]["end"], 71);
    assert!(
        vertical_lr["bbox"]["height"].as_u64().unwrap()
            > vertical_lr["bbox"]["width"].as_u64().unwrap(),
        "short vertical_lr sample run should be visibly vertical: {vertical_lr}"
    );
    let first_vertical_lr_cluster = find_rich_text_cluster_object(&json, "縦", 66, 69);
    assert_eq!(
        first_vertical_lr_cluster["rich_text_ref"]["kind"],
        "glyph_cluster"
    );
    assert_eq!(first_vertical_lr_cluster["rich_text_ref"]["source"], "text");
    assert_eq!(first_vertical_lr_cluster["bbox"]["width"], 42);
    assert_eq!(first_vertical_lr_cluster["bbox"]["height"], 42);
}

#[test]
fn agent_observe_native_renderer_writes_sample_full_frame_png_vertical_captures() {
    let cases = [
        (
            "windows-fonts",
            workspace_root().join("samples/rich-text-windows-fonts.arcw"),
            "縦書きの見本。吾輩は猫である。ABC 123 2026。春夏秋冬、朝昼夕夜、天地左右。",
            120,
            400,
        ),
        (
            "full-grammar",
            workspace_root().join("samples/rich-text-full-grammar.arcw"),
            "吾輩は猫である。ABC 123 2026",
            120,
            260,
        ),
    ];
    let dir = temp_dir("agent-observe-native-sample-full-frame-png");
    for (label, source_path, run_text, min_height, max_width) in cases {
        let png_path = dir.join(format!("{label}-full-frame.png"));
        let json = capture_native_png_report(&source_path, &png_path);
        assert_native_capture_has_content(&json, &format!("{label}-full-frame.png"));
        let run = find_rich_text_run_object(&json, run_text);
        assert!(
            run["bbox"]["height"].as_u64().unwrap() >= min_height,
            "{label} full-frame PNG report should preserve vertical run height: {run}"
        );
        assert!(
            run["bbox"]["width"].as_u64().unwrap() <= max_width,
            "{label} full-frame PNG report should preserve column-shaped width: {run}"
        );
        assert!(
            json["images"][0]["content_bbox"]["height"]
                .as_u64()
                .is_some_and(|height| height > 0),
            "{label} full-frame PNG should contain rendered native pixels: {json}"
        );
    }
    fs::remove_dir_all(&dir).expect("remove temp native sample full-frame dir");
}

#[test]
fn agent_observe_native_renderer_writes_dialogue_layer_framebuffer_crop() {
    let path = temp_arcw(
        "agent-observe-native-dialogue-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Hello native layer[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-dialogue-layer");
    let png_path = dir.join("native-dialogue-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native dialogue layer PNG");

    assert!(
        output.status.success(),
        "native dialogue layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native dialogue layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native dialogue layer report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue");
    assert_eq!(json["images"][0]["composition"], "framebuffer_crop");
    assert_eq!(json["images"][0]["width"], 1088);
    assert_eq!(json["images"][0]["height"], 124);
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert_eq!(json["images"][0]["crop_origin"]["x"], 96);
    assert_eq!(json["images"][0]["crop_origin"]["y"], 548);
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(json["images"][0]["written"], "native-dialogue-layer.png");

    fs::remove_file(&path).expect("remove temp native dialogue layer source");
    fs::remove_dir_all(&dir).expect("remove temp native dialogue layer dir");
}

#[test]
fn agent_observe_native_renderer_writes_object_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-object-crop",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-object-crop");
    let raw_path = dir.join("native-object.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native object raw crop");

    assert!(
        output.status.success(),
        "native object crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native object crop report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert_eq!(json["images"][0]["width"], 1088);
    assert_eq!(json["images"][0]["height"], 124);
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert_eq!(json["images"][0]["crop_origin"]["x"], 96);
    assert_eq!(json["images"][0]["crop_origin"]["y"], 548);
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < 1088 * 124);
    let bytes = fs::read(&raw_path).expect("read native object raw crop");
    let width = usize::try_from(json["images"][0]["width"].as_u64().unwrap()).unwrap();
    let height = usize::try_from(json["images"][0]["height"].as_u64().unwrap()).unwrap();
    assert_eq!(bytes.len(), width.saturating_mul(height).saturating_mul(4));
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] == 0));

    fs::remove_file(&path).expect("remove temp native object source");
    fs::remove_dir_all(&dir).expect("remove temp native object dir");
}

#[test]
fn agent_observe_native_renderer_writes_rich_text_layer_png_crop() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-layer",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-rich-text-layer");
    let png_path = dir.join("native-rich-text-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native rich-text layer PNG");

    assert!(
        output.status.success(),
        "native rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native rich-text layer crop report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert_eq!(json["images"][0]["mime_type"], "image/png");
    assert!(
        json["images"][0]["width"].as_u64().unwrap() < 1088,
        "rich-text layer crop should be narrower than the textbox"
    );
    assert!(
        json["images"][0]["height"].as_u64().unwrap() < 124,
        "rich-text layer crop should be shorter than the textbox"
    );
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert!(
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "rich-text layer crop origin should map to viewport coordinates"
    );
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(json["images"][0]["written"], "native-rich-text-layer.png");

    fs::remove_file(&path).expect("remove temp native rich-text layer source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text layer dir");
}

#[test]
fn agent_observe_native_renderer_handles_clear_in_rich_text_layer_capture() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-clear-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-clear-layer");
    let png_path = dir.join("native-rich-text-clear-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native clear rich-text layer PNG");

    assert!(
        output.status.success(),
        "native clear rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native clear rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native clear rich-text layer crop report is JSON");
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    let textbox = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["role"] == "textbox")
        .expect("textbox object is observed");
    assert_eq!(textbox["text"], "BeforeAfter");
    assert!(
        textbox["rich_text"]["display_map"]["controls"]
            .as_array()
            .unwrap()
            .iter()
            .any(|control| control["control"]["kind"] == "clear")
    );

    fs::remove_file(&path).expect("remove temp native clear rich-text layer source");
    fs::remove_dir_all(&dir).expect("remove temp native clear rich-text layer dir");
}

#[test]
fn agent_observe_native_renderer_captures_clear_after_page_layer() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-page-layer",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-page-layer");
    let png_path = dir.join("native-rich-text-page-layer.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--page")
        .arg("1")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes page-selected native rich-text layer PNG");

    assert!(
        output.status.success(),
        "native page-selected rich-text layer crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native page-selected rich-text layer PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native page-selected rich-text layer report is JSON");
    assert_page_selected_native_rich_text_layer_report(&json);

    fs::remove_file(&path).expect("remove temp native rich-text page layer source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text page layer dir");
}

fn assert_page_selected_native_rich_text_layer_report(json: &serde_json::Value) {
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["page"], 1);
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    let run_object = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("page-selected run object is observed");
    assert_eq!(run_object["rich_text_ref"]["page"], 1);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"], run_object["bbox"]["x"],
        "page-selected layer bbox should use the visible page child x bound"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"], run_object["bbox"]["y"],
        "page-selected layer bbox should use the visible page child y bound"
    );
    assert_eq!(
        json["images"][0]["width"], run_object["bbox"]["width"],
        "page-selected layer crop width should match the visible page child"
    );
    assert_eq!(
        json["images"][0]["height"], run_object["bbox"]["height"],
        "page-selected layer crop height should match the visible page child"
    );
}

#[test]
fn agent_observe_native_renderer_captures_clear_after_page_object() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-page-object",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let dir = temp_dir("agent-observe-native-rich-text-page-object");
    let png_path = dir.join("native-rich-text-page-object.png");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--object")
        .arg("object.dialogue.0.0.run.1")
        .arg("--page")
        .arg("1")
        .arg("--out")
        .arg(&png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes page-selected native rich-text object PNG");

    assert!(
        output.status.success(),
        "native page-selected rich-text object crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(&png_path).expect("read native page-selected rich-text object PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native page-selected rich-text object report is JSON");
    let page_capture_uri = assert_page_selected_native_rich_text_object_report(&json);

    assert_agent_read_uri_page_capture_ref(&path, &page_capture_uri);

    fs::remove_file(&path).expect("remove temp native rich-text page object source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text page object dir");
}

fn assert_page_selected_native_rich_text_object_report(json: &serde_json::Value) -> String {
    assert_eq!(json["images"][0]["kind"], "color");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["page"], 1);
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(
        json["images"][0]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(json["images"][0]["composition"], "isolated_regions");
    assert!(json["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        json["images"][0]["width"].as_u64().unwrap() < 1088,
        "page-selected run crop should be narrower than the textbox"
    );
    assert!(
        json["objects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|object| object["id"] == "object.dialogue.0.0.run.1")
    );
    let run_object = json["objects"]
        .as_array()
        .unwrap()
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.run.1")
        .expect("page-selected run object is observed");
    assert_eq!(run_object["rich_text_ref"]["page"], 1);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"], run_object["bbox"]["x"],
        "page-selected child bbox should use the same native x bound as the capture"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"], run_object["bbox"]["y"],
        "page-selected child bbox should use the same native y bound as the capture"
    );
    assert_eq!(
        json["images"][0]["width"], run_object["bbox"]["width"],
        "page-selected child bbox width should match the native crop width"
    );
    assert_eq!(
        json["images"][0]["height"], run_object["bbox"]["height"],
        "page-selected child bbox height should match the native crop height"
    );
    let page_capture_uri = run_object["capture_refs"]["captures"]
        .as_array()
        .unwrap()
        .iter()
        .find(|capture| capture["kind"] == "color" && capture["mime_type"] == "image/png")
        .expect("page-selected run object has a color PNG capture ref");
    assert_eq!(page_capture_uri["page"], 1);
    let page_capture_uri = page_capture_uri["uri"]
        .as_str()
        .expect("page-selected run object color PNG capture ref has a URI")
        .to_owned();
    assert!(
        page_capture_uri.ends_with("/object.object.dialogue.0.0.run.1.png?page=1"),
        "page-selected rich-text child capture ref should encode page query: {page_capture_uri}"
    );
    page_capture_uri
}

fn assert_agent_read_uri_page_capture_ref(path: &Path, page_capture_uri: &str) {
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--read-uri")
        .arg(page_capture_uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads page-selected rich-text capture ref");
    assert!(
        read_output.status.success(),
        "page-selected rich-text capture ref read should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    let resource: serde_json::Value = serde_json::from_slice(&read_output.stdout)
        .expect("page-selected rich-text capture ref read is JSON");
    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], page_capture_uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["page"], 1);
    assert_eq!(resource["image"]["scope"]["kind"], "object");
    assert_eq!(
        resource["image"]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert!(resource["image"]["content_pixels"].as_u64().unwrap() > 0);
}

fn assert_agent_read_uri_object_image_has_content(
    path: &Path,
    uri: &str,
    object_id: &str,
    width: u64,
    height: u64,
) {
    let read_output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(path)
        .arg("--json")
        .arg("--read-uri")
        .arg(uri)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reads object image capture ref");
    assert!(
        read_output.status.success(),
        "object image capture ref read should succeed, stderr: {}",
        String::from_utf8_lossy(&read_output.stderr)
    );
    let resource: serde_json::Value =
        serde_json::from_slice(&read_output.stdout).expect("object image capture ref read is JSON");
    assert_eq!(resource["kind"], "image");
    assert_eq!(resource["uri"], uri);
    assert_eq!(resource["image"]["renderer"], "native");
    assert_eq!(resource["image"]["scope"]["kind"], "object");
    assert_eq!(resource["image"]["scope"]["id"], object_id);
    assert_eq!(resource["image"]["width"], width);
    assert_eq!(resource["image"]["height"], height);
    assert!(resource["image"]["content_pixels"].as_u64().unwrap() > 0);
}

#[test]
fn agent_observe_read_uri_returns_latest_native_layer_image() {
    let path = temp_arcw(
        "agent-observe-native-layer-read-uri",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/layer.dialogue.rich_text.png")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads latest native layer image");

    fs::remove_file(&path).expect("remove temp native layer read-uri source");
    assert!(
        output.status.success(),
        "native layer read-uri should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native layer read-uri resource is JSON");
    assert_eq!(
        json["uri"],
        "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
    );
    assert_eq!(json["image"]["kind"], "color");
    assert_eq!(json["image"]["renderer"], "native");
    assert_eq!(json["image"]["scope"]["kind"], "layer");
    assert_eq!(json["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["image"]["composition"], "isolated_regions");
    assert!(json["image"]["width"].as_u64().unwrap() < 1088);
    assert!(json["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(json["image"]["crop_origin"]["space"], "viewport");
    assert_eq!(json["body"]["body_kind"], "bytes_base64");
    assert_eq!(json["body"]["body"]["encoding"], "base64");
    assert!(
        json["body"]["body"]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
fn agent_observe_read_uri_uses_native_renderer_without_selected_image() {
    let path = temp_arcw(
        "agent-observe-native-read-uri-renderer",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--read-uri")
        .arg("arcweft://session/cli/frame/0/layer.dialogue.rich_text.png")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe reads native layer image by URI");

    fs::remove_file(&path).expect("remove temp native read-uri renderer source");
    assert!(
        output.status.success(),
        "native read-uri renderer should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native read-uri renderer resource is JSON");
    assert_eq!(
        json["uri"],
        "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
    );
    assert_eq!(json["image"]["renderer"], "native");
    assert_eq!(json["image"]["scope"]["kind"], "layer");
    assert_eq!(json["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["image"]["composition"], "isolated_regions");
    assert!(json["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        json["body"]["body"]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
fn agent_observe_native_renderer_writes_ruby_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-ruby-mask",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-ruby-mask");
    let raw_path = dir.join("native-ruby-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native ruby mask raw crop");

    assert!(
        output.status.success(),
        "native ruby mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native ruby mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert!(json["images"][0]["width"].as_u64().unwrap() < 180);
    assert!(json["images"][0]["height"].as_u64().unwrap() < 120);
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    let objects = json["objects"].as_array().expect("objects are listed");
    let ruby_object = objects
        .iter()
        .find(|object| object["id"] == "object.dialogue.0.0.ruby.0")
        .expect("ruby object is observed");
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        ruby_object["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        ruby_object["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], ruby_object["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby_object["bbox"]["height"]);
    assert!(
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "native ruby crop origin should be in textbox viewport bounds"
    );
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    let content_bbox = &json["images"][0]["content_bbox"];
    let content_viewport_bbox = &json["images"][0]["content_viewport_bbox"];
    assert_eq!(
        content_viewport_bbox["x"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + content_bbox["x"].as_u64().unwrap()
    );
    assert_eq!(
        content_viewport_bbox["y"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + content_bbox["y"].as_u64().unwrap()
    );
    assert!(
        content_viewport_bbox["x"].as_u64().unwrap() >= ruby_object["bbox"]["x"].as_u64().unwrap()
    );
    assert!(
        content_viewport_bbox["y"].as_u64().unwrap() >= ruby_object["bbox"]["y"].as_u64().unwrap()
    );
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native ruby mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert!(opaque > 0);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native ruby mask source");
    fs::remove_dir_all(&dir).expect("remove temp native ruby mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_ruby_mask_raw_crop() {
    assert_native_vertical_lr_ruby_raw_crop("mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_ruby_object_id_raw_crop() {
    assert_native_vertical_lr_ruby_raw_crop("object-id");
}

fn assert_native_vertical_lr_ruby_raw_crop(capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-vertical-lr-ruby-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地|[夢](ゆめ)星[/][p]
}
",
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!("native-vertical-lr-ruby-{capture_kind}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr ruby raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr ruby {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native vertical_lr ruby report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported vertical_lr ruby capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let ruby = assert_native_vertical_lr_ruby_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            "vertical_lr ruby object-id crop",
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native vertical_lr ruby mask raw crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native vertical_lr ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr ruby dir");
}

fn assert_native_vertical_lr_ruby_geometry(json: &serde_json::Value) -> &serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ゆめ");
    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
        "vertical_lr ruby annotation should render on the left side of the base: {ruby}"
    );
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(annotation)
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(base),
        "vertical_lr ruby mask content should stay within the ruby base/annotation union: {ruby}"
    );
    ruby
}

#[test]
fn agent_observe_native_renderer_writes_long_vertical_ruby_mask_raw_crop() {
    assert_native_long_vertical_ruby_mask_raw_crop("vertical_rl", true);
    assert_native_long_vertical_ruby_mask_raw_crop("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_writes_long_vertical_ruby_object_id_raw_crop() {
    assert_native_long_vertical_ruby_object_id_raw_crop("vertical_rl", true);
    assert_native_long_vertical_ruby_object_id_raw_crop("vertical_lr", false);
}

#[test]
fn agent_observe_native_renderer_writes_overheight_vertical_ruby_raw_crops() {
    assert_native_overheight_vertical_ruby_raw_crop("vertical_rl", true, "mask");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_lr", false, "mask");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_rl", true, "object-id");
    assert_native_overheight_vertical_ruby_raw_crop("vertical_lr", false, "object-id");
}

fn assert_native_long_vertical_ruby_mask_raw_crop(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-mask"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-long-{writing_mode}-ruby-mask"
    ));
    let raw_path = dir.join(format!("native-long-{writing_mode}-ruby-mask.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native long vertical ruby mask raw crop");

    assert!(
        output.status.success(),
        "native long {writing_mode} ruby mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native long vertical ruby mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long {writing_mode} ruby mask should observe expanded base geometry: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
            "vertical_rl long ruby annotation should be on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
            "vertical_lr long ruby annotation should be on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "long {writing_mode} ruby mask content should stay inside the expanded ruby object bbox: {ruby}"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native long vertical ruby mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native long vertical ruby mask source");
    fs::remove_dir_all(&dir).expect("remove temp native long vertical ruby mask dir");
}

fn assert_native_long_vertical_ruby_object_id_raw_crop(writing_mode: &str, ruby_on_right: bool) {
    let path = temp_arcw(
        &format!("agent-observe-native-long-{writing_mode}-ruby-object-id"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春夏秋冬|[夢](ながいながいよみ)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-long-{writing_mode}-ruby-object-id"
    ));
    let raw_path = dir.join(format!("native-long-{writing_mode}-ruby-object-id.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native long vertical ruby object-id raw crop");

    assert!(
        output.status.success(),
        "native long {writing_mode} ruby object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native long vertical ruby object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");

    let ruby = find_rich_text_ruby_object(&json, 0);
    assert_eq!(ruby["rich_text_ref"]["ruby"], "ながいながいよみ");
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    let base_cluster = find_rich_text_cluster_object(&json, "夢", 18, 21);
    assert!(
        agent_json_bbox_height(base) > agent_json_bbox_height(&base_cluster["bbox"]) * 2,
        "long {writing_mode} ruby object-id should observe expanded base geometry: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
            "vertical_rl long ruby annotation should be on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
            "vertical_lr long ruby annotation should be on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "long {writing_mode} ruby object-id content should stay inside the expanded ruby object bbox: {ruby}"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);

    let expected = agent_object_id_color_from_json(ruby);
    assert_raw_object_id_tint(
        &raw_path,
        expected,
        content_pixels,
        &format!("{writing_mode} long ruby object-id crop"),
    );

    fs::remove_file(&path).expect("remove temp native long vertical ruby object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native long vertical ruby object-id dir");
}

fn assert_native_overheight_vertical_ruby_raw_crop(
    writing_mode: &str,
    ruby_on_right: bool,
    capture_kind: &str,
) {
    let path = temp_arcw(
        &format!("agent-observe-native-overheight-{writing_mode}-ruby-{capture_kind}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地|[夢](あいうえおかきくけこさしすせそたちつてと)人外[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!(
        "agent-observe-native-overheight-{writing_mode}-ruby-{capture_kind}"
    ));
    let raw_path = dir.join(format!(
        "native-overheight-{writing_mode}-ruby-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.ruby.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native over-height vertical ruby raw crop");

    assert!(
        output.status.success(),
        "native over-height {writing_mode} ruby {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native over-height vertical ruby report is JSON");
    let expected_kind = capture_kind.replace('-', "_");
    assert_eq!(json["images"][0]["kind"], expected_kind);
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(
        json["images"][0]["composition"],
        if capture_kind == "object-id" {
            "object_id_attachment"
        } else {
            "mask_attachment"
        }
    );

    let ruby = assert_native_overheight_vertical_ruby_geometry(&json, writing_mode, ruby_on_right);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(ruby),
            content_pixels,
            &format!("{writing_mode} over-height ruby object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native over-height vertical ruby mask crop");
        let opaque = opaque_pixel_count(&bytes);
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native over-height vertical ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native over-height vertical ruby dir");
}

fn assert_native_overheight_vertical_ruby_geometry<'a>(
    json: &'a serde_json::Value,
    writing_mode: &str,
    ruby_on_right: bool,
) -> &'a serde_json::Value {
    let ruby = find_rich_text_ruby_object(json, 0);
    assert_eq!(
        ruby["rich_text_ref"]["ruby"],
        "あいうえおかきくけこさしすせそたちつてと"
    );
    assert_eq!(json["images"][0]["crop_origin"]["x"], ruby["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], ruby["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], ruby["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], ruby["bbox"]["height"]);

    let base = &ruby["rich_text_ref"]["ruby_base_bbox"];
    let annotation = &ruby["rich_text_ref"]["ruby_annotation_bbox"];
    assert!(
        agent_json_bbox_width(annotation) > 24,
        "over-height {writing_mode} ruby annotation should union split tracks: {ruby}"
    );
    if ruby_on_right {
        assert!(
            agent_json_bbox_x(annotation) >= agent_json_bbox_right(base),
            "vertical_rl over-height ruby annotation should stay on the right side of the base: {ruby}"
        );
    } else {
        assert!(
            agent_json_bbox_right(annotation) <= agent_json_bbox_x(base),
            "vertical_lr over-height ruby annotation should stay on the left side of the base: {ruby}"
        );
    }
    assert!(
        agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(base)
            && agent_json_bbox_x(&ruby["bbox"]) <= agent_json_bbox_x(annotation)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(base)
            && agent_json_bbox_right(&ruby["bbox"]) >= agent_json_bbox_right(annotation)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(base)
            && agent_json_bbox_y(&ruby["bbox"]) <= agent_json_bbox_y(annotation)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(base)
            && agent_json_bbox_bottom(&ruby["bbox"]) >= agent_json_bbox_bottom(annotation),
        "over-height {writing_mode} ruby object bbox should union base and split annotation geometry: {ruby}"
    );
    assert!(
        agent_json_bbox_x(&json["images"][0]["content_viewport_bbox"])
            >= agent_json_bbox_x(&ruby["bbox"])
            && agent_json_bbox_right(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_right(&ruby["bbox"])
            && agent_json_bbox_y(&json["images"][0]["content_viewport_bbox"])
                >= agent_json_bbox_y(&ruby["bbox"])
            && agent_json_bbox_bottom(&json["images"][0]["content_viewport_bbox"])
                <= agent_json_bbox_bottom(&ruby["bbox"]),
        "over-height {writing_mode} ruby content should stay inside the authored ruby object bbox: {ruby}"
    );
    ruby
}

fn agent_object_id_color_from_json(object: &serde_json::Value) -> [u8; 4] {
    let color = &object["capture_refs"]["object_id_color"];
    [
        u8::try_from(color["red"].as_u64().expect("object-id red"))
            .expect("object-id red fits in u8"),
        u8::try_from(color["green"].as_u64().expect("object-id green"))
            .expect("object-id green fits in u8"),
        u8::try_from(color["blue"].as_u64().expect("object-id blue"))
            .expect("object-id blue fits in u8"),
        u8::try_from(color["alpha"].as_u64().expect("object-id alpha"))
            .expect("object-id alpha fits in u8"),
    ]
}

fn assert_raw_object_id_tint(
    raw_path: &Path,
    expected: [u8; 4],
    content_pixels: u64,
    context: &str,
) {
    let bytes = fs::read(raw_path).expect("read native long vertical ruby object-id raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let tinted_color = bytes
        .chunks_exact(4)
        .filter(|pixel| {
            pixel[3] >= 128
                && pixel[0].abs_diff(expected[0]) <= 24
                && pixel[1].abs_diff(expected[1]) <= 24
                && pixel[2].abs_diff(expected[2]) <= 24
        })
        .count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(
        tinted_color > 0,
        "{context} should contain the observed object color tint"
    );
}

#[test]
fn agent_observe_native_renderer_writes_text_combine_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-text-combine-mask",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]A 2026 B[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-text-combine-mask");
    let raw_path = dir.join("native-text-combine-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native text-combine mask raw crop");

    assert!(
        output.status.success(),
        "native text-combine mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native text-combine mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["y"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["y"].as_u64().unwrap()
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native text-combine mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native text-combine mask source");
    fs::remove_dir_all(&dir).expect("remove temp native text-combine mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_text_combine_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-vertical-lr-text-combine-mask",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]A 2026 B[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-vertical-lr-text-combine-mask");
    let raw_path = dir.join("native-vertical-lr-text-combine-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr text-combine mask raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr text-combine mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical_lr text-combine mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["x"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["x"].as_u64().unwrap()
    );
    assert_eq!(
        json["images"][0]["content_viewport_bbox"]["y"]
            .as_u64()
            .unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + json["images"][0]["content_bbox"]["y"].as_u64().unwrap()
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native vertical_lr text-combine mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native vertical_lr text-combine mask source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr text-combine mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_text_combine_object_id_raw_crop() {
    assert_native_text_combine_object_id_raw_crop("vertical_rl", "text-combine-object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_text_combine_object_id_raw_crop() {
    assert_native_text_combine_object_id_raw_crop(
        "vertical_lr",
        "vertical-lr-text-combine-object-id",
    );
}

fn assert_native_text_combine_object_id_raw_crop(writing_mode: &str, label: &str) {
    let path = temp_arcw(
        &format!("agent-observe-native-{label}"),
        &format!(
            r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]A 2026 B[/][p]
}}
"
        ),
    );
    let dir = temp_dir(&format!("agent-observe-native-{label}"));
    let raw_path = dir.join(format!("native-{label}.rgba"));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.2.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native text-combine object-id raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} text-combine object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native text-combine object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");

    let text_combine = find_rich_text_cluster_object(&json, "2026", 2, 6);
    assert_eq!(
        text_combine["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        text_combine["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        text_combine["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], text_combine["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], text_combine["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);

    let color = &text_combine["capture_refs"]["object_id_color"];
    let expected = [
        u8::try_from(color["red"].as_u64().expect("object-id red"))
            .expect("object-id red fits in u8"),
        u8::try_from(color["green"].as_u64().expect("object-id green"))
            .expect("object-id green fits in u8"),
        u8::try_from(color["blue"].as_u64().expect("object-id blue"))
            .expect("object-id blue fits in u8"),
        u8::try_from(color["alpha"].as_u64().expect("object-id alpha"))
            .expect("object-id alpha fits in u8"),
    ];
    let bytes = fs::read(&raw_path).expect("read native text-combine object-id raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let exact_color = bytes
        .chunks_exact(4)
        .filter(|pixel| *pixel == expected)
        .count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(
        exact_color > 0,
        "{writing_mode} text-combine object-id crop should contain the observed object color"
    );

    fs::remove_file(&path).expect("remove temp native text-combine object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native text-combine object-id dir");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compressed_punctuation_mask_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-compressed-punctuation-mask",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天、。人[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-jlreq-compressed-punctuation-mask");
    let raw_path = dir.join("native-jlreq-compressed-punctuation-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.1.3.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ punctuation mask raw crop");

    assert!(
        output.status.success(),
        "native JLREQ punctuation mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ punctuation mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");

    let comma = find_rich_text_cluster_object(&json, "、", 3, 6);
    let period = find_rich_text_cluster_object(&json, "。", 6, 9);
    let person = find_rich_text_cluster_object(&json, "人", 9, 12);
    assert_eq!(comma["rich_text_ref"]["orientation"], "upright");
    assert_eq!(comma["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_eq!(json["images"][0]["crop_origin"]["x"], comma["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], comma["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], comma["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], comma["bbox"]["height"]);
    assert_eq!(
        agent_json_bbox_y(&period["bbox"]) - agent_json_bbox_y(&comma["bbox"]),
        21,
        "compressed comma should advance by half a body cell"
    );
    assert_eq!(
        agent_json_bbox_y(&person["bbox"]) - agent_json_bbox_y(&period["bbox"]),
        21,
        "following text should consume the space left by punctuation compression"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native JLREQ punctuation mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert_eq!(opaque as u64, content_pixels);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native JLREQ punctuation mask source");
    fs::remove_dir_all(&dir).expect("remove temp native JLREQ punctuation mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_compressed_punctuation_object_id_raw_crop() {
    let path = temp_arcw(
        "agent-observe-native-jlreq-compressed-punctuation-object-id",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天、。人[/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-jlreq-compressed-punctuation-object-id");
    let raw_path = dir.join("native-jlreq-compressed-punctuation-object-id.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("object-id")
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.1.3.6")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ punctuation object-id raw crop");

    assert!(
        output.status.success(),
        "native JLREQ punctuation object-id crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ punctuation object-id report is JSON");
    assert_eq!(json["images"][0]["kind"], "object_id");
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    assert_eq!(json["images"][0]["composition"], "object_id_attachment");

    let comma = find_rich_text_cluster_object(&json, "、", 3, 6);
    let period = find_rich_text_cluster_object(&json, "。", 6, 9);
    let person = find_rich_text_cluster_object(&json, "人", 9, 12);
    assert_eq!(comma["rich_text_ref"]["orientation"], "upright");
    assert_eq!(comma["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_eq!(json["images"][0]["crop_origin"]["x"], comma["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], comma["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], comma["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], comma["bbox"]["height"]);
    assert_eq!(
        agent_json_bbox_y(&period["bbox"]) - agent_json_bbox_y(&comma["bbox"]),
        21,
        "compressed comma should advance by half a body cell"
    );
    assert_eq!(
        agent_json_bbox_y(&person["bbox"]) - agent_json_bbox_y(&period["bbox"]),
        21,
        "following text should consume the space left by punctuation compression"
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    assert_raw_object_id_tint(
        &raw_path,
        agent_object_id_color_from_json(comma),
        content_pixels,
        "JLREQ compressed punctuation object-id crop",
    );

    fs::remove_file(&path).expect("remove temp native JLREQ punctuation object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native JLREQ punctuation object-id dir");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_opening_punctuation_mask_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_rl", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_jlreq_opening_punctuation_object_id_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_rl", "object-id");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_opening_punctuation_mask_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_lr", "mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_opening_punctuation_object_id_raw_crop() {
    assert_native_jlreq_opening_punctuation_raw_crop("vertical_lr", "object-id");
}

fn assert_native_jlreq_opening_punctuation_raw_crop(writing_mode: &str, capture_kind: &str) {
    let fixture_name =
        format!("agent-observe-native-{writing_mode}-jlreq-opening-punctuation-{capture_kind}");
    let source = format!(
        r"
character @character.alice Alice as alice {{}}

flow @flow.main main {{
    alice: [.{writing_mode}]天地春「人外[/][p]
}}
"
    );
    let path = temp_arcw(&fixture_name, &source);
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-{writing_mode}-jlreq-opening-punctuation-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.3.9.12")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native JLREQ opening punctuation raw crop");

    assert!(
        output.status.success(),
        "native {writing_mode} JLREQ opening {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native JLREQ opening punctuation report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported JLREQ opening punctuation capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let opening_bracket = assert_native_jlreq_opening_punctuation_geometry(&json, writing_mode);
    assert_eq!(
        json["images"][0]["crop_origin"]["x"],
        opening_bracket["bbox"]["x"]
    );
    assert_eq!(
        json["images"][0]["crop_origin"]["y"],
        opening_bracket["bbox"]["y"]
    );
    assert_eq!(json["images"][0]["width"], opening_bracket["bbox"]["width"]);
    assert_eq!(
        json["images"][0]["height"],
        opening_bracket["bbox"]["height"]
    );

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(opening_bracket),
            content_pixels,
            &format!("{writing_mode} JLREQ opening punctuation object-id crop"),
        );
    } else {
        let bytes = fs::read(&raw_path).expect("read native JLREQ opening punctuation mask crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native JLREQ opening punctuation source");
    fs::remove_dir_all(&dir).expect("remove temp native JLREQ opening punctuation dir");
}

fn assert_native_jlreq_opening_punctuation_geometry<'a>(
    json: &'a serde_json::Value,
    writing_mode: &str,
) -> &'a serde_json::Value {
    let spring = find_rich_text_cluster_object(json, "春", 6, 9);
    let opening_bracket = find_rich_text_cluster_object(json, "「", 9, 12);
    let person = find_rich_text_cluster_object(json, "人", 12, 15);
    assert_eq!(
        opening_bracket["rich_text_ref"]["orientation"],
        "sideways_cw"
    );
    assert_eq!(
        opening_bracket["rich_text_ref"]["vertical_form"],
        "rotated_alternate"
    );
    match writing_mode {
        "vertical_rl" => assert!(
            agent_json_bbox_x(&opening_bracket["bbox"]) < agent_json_bbox_x(&spring["bbox"]),
            "line-end-prohibited opening punctuation should move to the next vertical_rl column"
        ),
        "vertical_lr" => assert!(
            agent_json_bbox_x(&opening_bracket["bbox"]) > agent_json_bbox_x(&spring["bbox"]),
            "line-end-prohibited opening punctuation should move to the next vertical_lr column"
        ),
        other => panic!("unsupported writing mode for JLREQ opening punctuation crop: {other}"),
    }
    assert!(
        agent_json_bbox_y(&opening_bracket["bbox"]) < agent_json_bbox_y(&spring["bbox"]),
        "opening punctuation moved from a column end should restart near the column top"
    );
    assert_vertical_cluster_after(
        opening_bracket,
        person,
        "text after opening punctuation should continue in the same moved column",
    );
    opening_bracket
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_hanging_punctuation_mask_raw_crop() {
    assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop("mask");
}

#[test]
fn agent_observe_native_renderer_writes_vertical_lr_jlreq_hanging_punctuation_object_id_raw_crop() {
    assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop("object-id");
}

fn assert_native_vertical_lr_jlreq_hanging_punctuation_raw_crop(capture_kind: &str) {
    let fixture_name = format!("agent-observe-native-vertical-lr-jlreq-hanging-{capture_kind}");
    let path = temp_arcw(
        &fixture_name,
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_lr]天地、人人[/][p]
}
",
    );
    let dir = temp_dir(&fixture_name);
    let raw_path = dir.join(format!(
        "native-vertical-lr-jlreq-hanging-punctuation-{capture_kind}.rgba"
    ));

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg("object.dialogue.0.0.cluster.2.6.9")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native vertical_lr JLREQ hanging raw crop");

    assert!(
        output.status.success(),
        "native vertical_lr JLREQ hanging {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("native vertical_lr JLREQ hanging report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported vertical_lr JLREQ hanging capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");

    let comma = assert_native_vertical_lr_jlreq_hanging_punctuation_geometry(&json);
    assert_eq!(json["images"][0]["crop_origin"]["x"], comma["bbox"]["x"]);
    assert_eq!(json["images"][0]["crop_origin"]["y"], comma["bbox"]["y"]);
    assert_eq!(json["images"][0]["width"], comma["bbox"]["width"]);
    assert_eq!(json["images"][0]["height"], comma["bbox"]["height"]);

    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    if capture_kind == "object-id" {
        assert_raw_object_id_tint(
            &raw_path,
            agent_object_id_color_from_json(comma),
            content_pixels,
            "vertical_lr JLREQ hanging punctuation object-id crop",
        );
    } else {
        let bytes =
            fs::read(&raw_path).expect("read native vertical_lr JLREQ hanging mask raw crop");
        let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
        let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
        assert_eq!(opaque as u64, content_pixels);
        assert!(transparent > 0);
    }

    fs::remove_file(&path).expect("remove temp native vertical_lr JLREQ hanging source");
    fs::remove_dir_all(&dir).expect("remove temp native vertical_lr JLREQ hanging dir");
}

fn assert_native_vertical_lr_jlreq_hanging_punctuation_geometry(
    json: &serde_json::Value,
) -> &serde_json::Value {
    let earth = find_rich_text_cluster_object(json, "地", 3, 6);
    let comma = find_rich_text_cluster_object(json, "、", 6, 9);
    let next_person = find_rich_text_cluster_object(json, "人", 9, 12);
    assert_eq!(comma["rich_text_ref"]["orientation"], "upright");
    assert_eq!(comma["rich_text_ref"]["vertical_form"], "upright_alternate");
    assert_eq!(
        earth["bbox"]["x"], comma["bbox"]["x"],
        "vertical_lr hanging punctuation should remain in the previous column"
    );
    assert!(
        agent_json_bbox_y(&comma["bbox"]) > agent_json_bbox_y(&earth["bbox"]),
        "vertical_lr hanging punctuation should sit after the previous cluster"
    );
    assert!(
        agent_json_bbox_x(&next_person["bbox"]) > agent_json_bbox_x(&comma["bbox"])
            && agent_json_bbox_y(&next_person["bbox"]) < agent_json_bbox_y(&comma["bbox"]),
        "text after vertical_lr hanging punctuation should start the next column"
    );
    comma
}

fn observe_native_typewriter_cluster_mask_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    capture_time: &str,
) -> (serde_json::Value, Vec<u8>) {
    observe_native_typewriter_cluster_raw_at(source_path, raw_path, object_id, capture_time, "mask")
}

fn observe_native_typewriter_cluster_raw_at(
    source_path: &Path,
    raw_path: &Path,
    object_id: &str,
    capture_time: &str,
    capture_kind: &str,
) -> (serde_json::Value, Vec<u8>) {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg(capture_kind)
        .arg("--object")
        .arg(object_id)
        .arg("--capture-time")
        .arg(capture_time)
        .arg("--out")
        .arg(raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native typewriter raw crop");

    assert!(
        output.status.success(),
        "native typewriter {capture_kind} crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native typewriter raw report is JSON");
    match capture_kind {
        "mask" => {
            assert_eq!(json["images"][0]["kind"], "mask");
            assert_eq!(json["images"][0]["composition"], "mask_attachment");
        }
        "object-id" => {
            assert_eq!(json["images"][0]["kind"], "object_id");
            assert_eq!(json["images"][0]["composition"], "object_id_attachment");
        }
        other => panic!("unsupported native typewriter capture kind: {other}"),
    }
    assert_eq!(json["images"][0]["mime_type"], "application/octet-stream");
    let bytes = fs::read(raw_path).expect("read native typewriter raw crop");
    (json, bytes)
}

fn opaque_pixel_count(bytes: &[u8]) -> usize {
    bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count()
}

#[test]
fn agent_observe_native_typewriter_capture_time_changes_visibility_without_relayout() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]吾輩[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-capture-time");
    let hidden_path = dir.join("native-typewriter-hidden-mask.rgba");
    let visible_path = dir.join("native-typewriter-visible-mask.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.3",
        "4",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "吾", 0, 3);
    let visible_cluster = find_rich_text_cluster_object(&visible, "吾", 0, 3);
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_opaque = opaque_pixel_count(&hidden_bytes);
    let visible_opaque = opaque_pixel_count(&visible_bytes);
    assert_eq!(hidden_opaque, 0);
    assert_eq!(
        visible_opaque as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter dir");
}

#[test]
fn agent_observe_native_typewriter_text_combine_capture_time_controls_all_glyphs() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-text-combine-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]2026[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-text-combine-capture-time");
    let hidden_path = dir.join("native-typewriter-text-combine-hidden-mask.rgba");
    let visible_path = dir.join("native-typewriter-text-combine-visible-mask.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "4",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "2026", 0, 4);
    let visible_cluster = find_rich_text_cluster_object(&visible, "2026", 0, 4);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        visible_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_opaque = opaque_pixel_count(&hidden_bytes);
    let visible_opaque = opaque_pixel_count(&visible_bytes);
    assert_eq!(hidden_opaque, 0);
    assert_eq!(
        visible_opaque as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter text-combine source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter text-combine dir");
}

#[test]
fn agent_observe_native_typewriter_text_combine_capture_time_controls_object_id() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-text-combine-object-id-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]2026[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-text-combine-object-id-capture-time");
    let hidden_path = dir.join("native-typewriter-text-combine-hidden-object-id.rgba");
    let visible_path = dir.join("native-typewriter-text-combine-visible-object-id.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "0",
        "object-id",
    );
    let (visible, _visible_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.cluster.0.0.4",
        "4",
        "object-id",
    );
    let hidden_cluster = find_rich_text_cluster_object(&hidden, "2026", 0, 4);
    let visible_cluster = find_rich_text_cluster_object(&visible, "2026", 0, 4);
    assert_eq!(
        hidden_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(
        visible_cluster["rich_text_ref"]["orientation"],
        "text_combine_upright"
    );
    assert_eq!(hidden_cluster["bbox"], visible_cluster["bbox"]);
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    let visible_pixels = visible["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(visible_pixels > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_raw_object_id_tint(
        &visible_path,
        agent_object_id_color_from_json(visible_cluster),
        visible_pixels,
        "typewriter text-combine object-id capture-time crop",
    );

    fs::remove_file(&path).expect("remove temp native typewriter text-combine object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter text-combine object-id dir");
}

#[test]
fn agent_observe_native_typewriter_ruby_capture_time_controls_base_and_annotation() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-ruby-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-ruby-capture-time");
    let hidden_path = dir.join("native-typewriter-ruby-hidden-mask.rgba");
    let visible_path = dir.join("native-typewriter-ruby-visible-mask.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.ruby.0",
        "0",
    );
    let (visible, visible_bytes) = observe_native_typewriter_cluster_mask_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.ruby.0",
        "4",
    );
    assert_native_typewriter_ruby_capture_time_geometry(&hidden, &visible);
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    assert!(visible["images"][0]["content_pixels"].as_u64().unwrap() > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_eq!(
        opaque_pixel_count(&visible_bytes) as u64,
        visible["images"][0]["content_pixels"].as_u64().unwrap()
    );

    fs::remove_file(&path).expect("remove temp native typewriter ruby source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter ruby dir");
}

#[test]
fn agent_observe_native_typewriter_ruby_capture_time_controls_object_id() {
    let path = temp_arcw(
        "agent-observe-native-typewriter-ruby-object-id-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}
",
    );
    let dir = temp_dir("agent-observe-native-typewriter-ruby-object-id-capture-time");
    let hidden_path = dir.join("native-typewriter-ruby-hidden-object-id.rgba");
    let visible_path = dir.join("native-typewriter-ruby-visible-object-id.rgba");

    let (hidden, hidden_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &hidden_path,
        "object.dialogue.0.0.ruby.0",
        "0",
        "object-id",
    );
    let (visible, _visible_bytes) = observe_native_typewriter_cluster_raw_at(
        &path,
        &visible_path,
        "object.dialogue.0.0.ruby.0",
        "4",
        "object-id",
    );
    let visible_ruby = assert_native_typewriter_ruby_capture_time_geometry(&hidden, &visible);
    assert_eq!(hidden["images"][0]["content_pixels"], 0);
    let visible_pixels = visible["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(visible_pixels > 0);

    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_raw_object_id_tint(
        &visible_path,
        agent_object_id_color_from_json(visible_ruby),
        visible_pixels,
        "typewriter ruby object-id capture-time crop",
    );

    fs::remove_file(&path).expect("remove temp native typewriter ruby object-id source");
    fs::remove_dir_all(&dir).expect("remove temp native typewriter ruby object-id dir");
}

fn assert_native_typewriter_ruby_capture_time_geometry<'a>(
    hidden: &serde_json::Value,
    visible: &'a serde_json::Value,
) -> &'a serde_json::Value {
    let hidden_ruby = find_rich_text_ruby_object(hidden, 0);
    let visible_ruby = find_rich_text_ruby_object(visible, 0);
    assert_eq!(hidden_ruby["bbox"], visible_ruby["bbox"]);
    assert_eq!(
        hidden_ruby["rich_text_ref"]["ruby_base_bbox"],
        visible_ruby["rich_text_ref"]["ruby_base_bbox"]
    );
    assert_eq!(
        hidden_ruby["rich_text_ref"]["ruby_annotation_bbox"],
        visible_ruby["rich_text_ref"]["ruby_annotation_bbox"]
    );
    assert_eq!(
        hidden["images"][0]["crop_origin"],
        visible["images"][0]["crop_origin"]
    );
    assert_eq!(hidden["images"][0]["width"], visible["images"][0]["width"]);
    assert_eq!(
        hidden["images"][0]["height"],
        visible["images"][0]["height"]
    );
    visible_ruby
}

#[test]
fn agent_observe_native_renderer_writes_textbox_mask_as_glyph_geometry() {
    let path = temp_arcw(
        "agent-observe-native-textbox-mask",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-textbox-mask");
    let raw_path = dir.join("native-textbox-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--object")
        .arg("object.dialogue.0.0")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native textbox mask raw crop");

    assert!(
        output.status.success(),
        "native textbox mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native textbox mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "object");
    assert_eq!(json["images"][0]["scope"]["id"], "object.dialogue.0.0");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert_eq!(json["images"][0]["width"], 1088);
    assert_eq!(json["images"][0]["height"], 124);
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    assert!(content_pixels > 0);
    assert!(
        content_pixels < 1088 * 124,
        "native textbox mask should expose glyph geometry instead of filling the whole bbox"
    );
    let bytes = fs::read(&raw_path).expect("read native textbox mask raw crop");
    let opaque = bytes.chunks_exact(4).filter(|pixel| pixel[3] > 0).count();
    let transparent = bytes.chunks_exact(4).filter(|pixel| pixel[3] == 0).count();
    assert!(opaque > 0);
    assert!(transparent > 0);

    fs::remove_file(&path).expect("remove temp native textbox mask source");
    fs::remove_dir_all(&dir).expect("remove temp native textbox mask dir");
}

#[test]
fn agent_observe_native_renderer_writes_rich_text_layer_mask_attachment() {
    let path = temp_arcw(
        "agent-observe-native-rich-text-layer-mask",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let dir = temp_dir("agent-observe-native-rich-text-layer-mask");
    let raw_path = dir.join("native-rich-text-layer-mask.rgba");

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(&path)
        .arg("--json")
        .arg("--image")
        .arg("raw-rgba")
        .arg("--capture")
        .arg("mask")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--out")
        .arg(&raw_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native rich-text layer mask raw crop");

    assert!(
        output.status.success(),
        "native rich-text layer mask crop should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("native rich-text layer mask report is JSON");
    assert_eq!(json["images"][0]["kind"], "mask");
    assert_eq!(json["images"][0]["renderer"], "native");
    assert_eq!(json["images"][0]["scope"]["kind"], "layer");
    assert_eq!(json["images"][0]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(json["images"][0]["composition"], "mask_attachment");
    assert_eq!(json["images"][0]["crop_origin"]["space"], "viewport");
    assert!(json["images"][0]["width"].as_u64().unwrap() < 1088);
    assert!(json["images"][0]["height"].as_u64().unwrap() < 124);
    let content_pixels = json["images"][0]["content_pixels"].as_u64().unwrap();
    let width = json["images"][0]["width"].as_u64().unwrap();
    let height = json["images"][0]["height"].as_u64().unwrap();
    let content_bbox = &json["images"][0]["content_bbox"];
    let content_viewport_bbox = &json["images"][0]["content_viewport_bbox"];
    assert_eq!(
        content_viewport_bbox["x"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["x"].as_u64().unwrap()
            + content_bbox["x"].as_u64().unwrap()
    );
    assert_eq!(
        content_viewport_bbox["y"].as_u64().unwrap(),
        json["images"][0]["crop_origin"]["y"].as_u64().unwrap()
            + content_bbox["y"].as_u64().unwrap()
    );
    assert!(content_pixels > 0);
    assert!(content_pixels < width * height);
    let bytes = fs::read(&raw_path).expect("read native rich-text layer mask raw crop");
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] == 0));

    fs::remove_file(&path).expect("remove temp native rich-text layer mask source");
    fs::remove_dir_all(&dir).expect("remove temp native rich-text layer mask dir");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_observes_and_reads_rich_text_child_image() {
    let path = temp_arcw(
        "agent-mcp-rich-text-image",
        r#"
pub dialogue defaults @dialogue.defaults {
    font = serif
}

character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = agent_mcp_rich_text_requests(&path);
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp agent mcp source");
    assert!(
        output.status.success(),
        "agent mcp should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_agent_mcp_rich_text_capture_responses(&responses);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_lists_resource_templates_before_observe() {
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "resources/templates/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
    assert!(
        output.status.success(),
        "agent mcp resource templates should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    let templates = responses[1]["result"]["resourceTemplates"]
        .as_array()
        .expect("resource templates are listed");
    assert!(templates.iter().any(|template| {
        template["name"] == "viewport-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("/{capture}.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "layer-mask-capture"
            && template["uriTemplate"]
                .as_str()
                .is_some_and(|uri| uri.contains("layer.{layer_id}.mask.{extension}"))
    }));
    assert!(templates.iter().any(|template| {
        template["name"] == "object-color-capture"
            && template["description"]
                .as_str()
                .is_some_and(|description| description.contains("rich-text child objects"))
    }));
}

fn assert_agent_mcp_rich_text_capture_responses(responses: &[serde_json::Value]) {
    assert_eq!(responses.len(), 10);
    assert_eq!(
        responses[0]["result"]["serverInfo"]["name"],
        "arcweft-agent"
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.observe")
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.capture")
    );
    assert!(
        responses[1]["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "arcweft.session.info")
    );
    assert!(
        responses[2]["result"]["content"]
            .as_array()
            .unwrap()
            .iter()
            .any(|content| content["type"] == "resource_link"
                && content["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with("/objects.json")))
    );
    assert!(
        responses[3]["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png")))
    );
    assert!(
        responses[3]["result"]["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["uri"]
                .as_str()
                .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.ruby.0.png")))
    );
    assert_mcp_png_capture_content(&responses[4], "ruby capture metadata is JSON");
    assert_mcp_png_capture_content(&responses[5], "native ruby capture metadata is JSON");
    assert_mcp_raw_capture_content(&responses[6]);
    assert_mcp_session_info_after_capture(&responses[7]);
    assert_raw_resource_read_content(&responses[8], &responses[6]);
    assert_png_resource_read_content(&responses[9]);
}

fn assert_mcp_session_info_after_capture(response: &serde_json::Value) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let info = mcp_content_metadata(
        &response["result"]["content"][0],
        "session info content is JSON",
    );
    assert_eq!(info["observed"], true);
    assert_eq!(info["session_id"], "cli");
    assert_eq!(info["tick"], 0);
    assert!(info["resource_count"].as_u64().unwrap() > 0);
    assert!(info["latest_capture"]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(info["capture_resource_count"], 2);
    assert_eq!(info["native_capture_session_active"], true);
    assert_eq!(info["latest_capture"]["crop_origin"]["space"], "viewport");
    assert_eq!(
        info["latest_capture_uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_eq!(
        info["latest_capture_resource"]["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_eq!(
        info["latest_capture_resource"]["mimeType"],
        "application/octet-stream"
    );
    assert!(
        info["latest_capture_resource"]["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=mask")
                && description.contains("scope=object:object.dialogue.0.0.ruby.0"))
    );
    assert!(
        info["resource_templates"]
            .as_array()
            .unwrap()
            .iter()
            .any(|template| {
                template["name"] == "object-mask-capture"
                    && template["uriTemplate"]
                        .as_str()
                        .is_some_and(|uri| uri.contains("object.{object_id}.mask.{extension}"))
            })
    );
    assert!(info["layers"].as_array().unwrap().iter().any(|layer| {
        layer["id"] == "dialogue.rich_text"
            && layer["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"]
                        .as_str()
                        .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.mask.rgba"))
                })
    }));
    assert!(info["objects"].as_array().unwrap().iter().any(|object| {
        object["id"] == "object.dialogue.0.0.ruby.0"
            && object["rich_text_ref"]["kind"] == "ruby"
            && object["capture_refs"]["captures"]
                .as_array()
                .unwrap()
                .iter()
                .any(|capture| {
                    capture["uri"].as_str().is_some_and(|uri| {
                        uri.ends_with("/object.object.dialogue.0.0.ruby.0.mask.rgba")
                    })
                })
    }));
    assert!(
        info["resources"]
            .as_array()
            .unwrap()
            .iter()
            .any(|resource| resource["name"] == "objects.json")
    );
}

fn assert_mcp_png_capture_content(response: &serde_json::Value, metadata_context: &str) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(&response["result"]["content"][0], metadata_context);
    assert!(metadata["image"]["width"].as_u64().unwrap() > 0);
    assert!(metadata["image"]["height"].as_u64().unwrap() > 0);
    assert_eq!(metadata["image"]["kind"], "color");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        response["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
    assert_eq!(response["result"]["content"][1]["mimeType"], "image/png");
}

fn assert_mcp_raw_capture_content(response: &serde_json::Value) {
    assert_eq!(response["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(
        &response["result"]["content"][0],
        "raw capture metadata is JSON",
    );
    assert_eq!(
        metadata["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    assert_eq!(metadata["image"]["pixel_format"], "rgba8_unorm");
    assert_eq!(
        metadata["image"]["row_stride_bytes"],
        metadata["image"]["width"].as_u64().unwrap() * 4
    );
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        response["result"]["content"][1]["resource"]["blob"]
            .as_str()
            .is_some_and(|blob| !blob.is_empty())
    );
    assert_eq!(
        response["result"]["content"][1]["resource"]["mimeType"],
        "application/octet-stream"
    );
}

fn mcp_raw_capture_bytes(response: &serde_json::Value) -> Vec<u8> {
    let blob = response["result"]["content"][1]["resource"]["blob"]
        .as_str()
        .expect("raw capture response has a resource blob");
    general_purpose::STANDARD
        .decode(blob)
        .expect("raw capture blob is base64")
}

fn assert_raw_resource_read_content(
    response: &serde_json::Value,
    source_capture_response: &serde_json::Value,
) {
    let metadata = mcp_content_metadata(
        &source_capture_response["result"]["content"][0],
        "raw source capture metadata is JSON",
    );
    let expected_len = metadata["image"]["row_stride_bytes"].as_u64().unwrap()
        * metadata["image"]["height"].as_u64().unwrap();
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "application/octet-stream");
    assert_eq!(
        content["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
    );
    let blob = content["blob"].as_str().expect("raw resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("raw resource blob is base64");
    assert_eq!(
        u64::try_from(bytes.len()).unwrap(),
        expected_len,
        "resources/read should return the latest raw capture bytes for this URI"
    );
    assert!(bytes.chunks_exact(4).any(|pixel| pixel[3] > 0));
}

fn assert_png_resource_read_content(response: &serde_json::Value) {
    let content = &response["result"]["contents"][0];
    assert_eq!(content["mimeType"], "image/png");
    assert_eq!(
        content["uri"],
        "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png"
    );
    let blob = content["blob"].as_str().expect("PNG resource has a blob");
    let bytes = general_purpose::STANDARD
        .decode(blob)
        .expect("PNG resource blob is base64");
    assert!(
        bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "resources/read should keep earlier session capture bytes after later captures"
    );
}

fn mcp_content_metadata(block: &serde_json::Value, parse_message: &str) -> serde_json::Value {
    serde_json::from_str(block["text"].as_str().unwrap()).expect(parse_message)
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_without_prior_observe() {
    let path = temp_arcw(
        "agent-mcp-direct-capture",
        r#"
pub dialogue defaults @dialogue.defaults {
    font = serif
}

character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "layer": "dialogue.rich_text",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 3, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp direct capture source");
    assert!(
        output.status.success(),
        "agent mcp direct capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_agent_mcp_direct_capture_responses(&responses);
}

fn assert_agent_mcp_direct_capture_responses(responses: &[serde_json::Value]) {
    assert_eq!(responses.len(), 3);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    let direct_capture_metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "direct capture metadata is JSON",
    );
    assert!(
        direct_capture_metadata["image"]["width"]
            .as_u64()
            .is_some_and(|width| (130..220).contains(&width)),
        "direct rich-text layer capture width should come from observed native-layout child bboxes"
    );
    assert_eq!(direct_capture_metadata["image"]["renderer"], "native");
    assert!(
        matches!(
            direct_capture_metadata["image"]["composition"].as_str(),
            Some("isolated_regions" | "masked_framebuffer_crop" | "framebuffer_crop")
        ),
        "direct rich-text layer capture should use a native composition"
    );
    assert_eq!(direct_capture_metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(
        direct_capture_metadata["image"]["scope"]["id"],
        "dialogue.rich_text"
    );
    assert_eq!(
        direct_capture_metadata["image"]["crop_origin"]["space"],
        "viewport"
    );
    assert!(
        direct_capture_metadata["image"]["crop_origin"]["x"]
            .as_u64()
            .unwrap()
            >= 120
    );
    assert!(
        direct_capture_metadata["image"]["content_bbox"]["width"]
            .as_u64()
            .is_some_and(|width| width > 0)
    );
    assert!(
        direct_capture_metadata["image"]["content_bbox"]["height"]
            .as_u64()
            .is_some_and(|height| height > 0)
    );
    assert!(
        direct_capture_metadata["image"]["content_pixels"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(responses[1]["result"]["content"][1]["type"], "image");
    assert_eq!(
        responses[1]["result"]["content"][1]["mimeType"],
        "image/png"
    );
    assert!(
        responses[1]["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
    assert_agent_mcp_direct_capture_resources(&responses[2]);
}

fn assert_agent_mcp_direct_capture_resources(response: &serde_json::Value) {
    let resources = response["result"]["resources"].as_array().unwrap();
    let layer_image = resources
        .iter()
        .find(|resource| {
            let is_layer_png = resource["uri"]
                .as_str()
                .and_then(|uri| uri.split('?').next())
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png"));
            let is_native = resource["description"]
                .as_str()
                .is_some_and(|description| description.contains("renderer=native"));
            is_layer_png && is_native
        })
        .expect("direct capture should expose the selected layer image resource");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains("scope=layer:dialogue.rich_text")
                && description.contains("width=")
                && description.contains("height=")),
        "direct capture layer descriptor should expose image metadata"
    );
    assert!(
        resources.iter().any(|resource| resource["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/object.object.dialogue.0.0.ruby.0.png"))),
        "direct capture should populate latest observation resources"
    );
    assert!(
        resources.iter().any(|resource| resource["uri"]
            .as_str()
            .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.mask.rgba"))),
        "direct capture should expose layer capture refs"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": "arcweft://session/cli/frame/0/color.png"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP source");
    assert!(
        output.status.success(),
        "agent mcp native capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[1], "native capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert!(
        metadata["image"]["content_bbox"]["x"].as_u64().unwrap() >= 96,
        "native MCP capture should align with the observed textbox bbox"
    );
    assert_mcp_png_capture_content(&responses[2], "native capture resource metadata is JSON");
    let read_metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native capture resource metadata is JSON",
    );
    assert_eq!(read_metadata["image"]["renderer"], "native");
    assert_eq!(read_metadata["image"]["scope"]["kind"], "viewport");
    assert_eq!(read_metadata["image"]["composition"], "framebuffer");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_clear_after_page_object_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-page-object-capture",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.run.1",
                    "page": 1,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native page-object MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page object capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native page object capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native page object capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["page"], 1);
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(metadata["image"]["composition"], "isolated_regions");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_page_query_capture_ref_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-page-query-read",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: Before[clear]After[p]
}
",
    );
    let uri = "arcweft://session/cli/frame/0/object.object.dialogue.0.0.run.1.png?page=1";
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": uri
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native page-query MCP source");
    assert!(
        output.status.success(),
        "agent mcp native page-query read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[2], "native page-query read metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native page-query read metadata is JSON",
    );
    assert_eq!(metadata["uri"], uri);
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["page"], 1);
    assert_eq!(metadata["image"]["scope"]["kind"], "object");
    assert_eq!(
        metadata["image"]["scope"]["id"],
        "object.dialogue.0.0.run.1"
    );
    assert_eq!(metadata["image"]["composition"], "isolated_regions");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_object_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-object-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP object source");
    assert!(
        output.status.success(),
        "agent mcp native object capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native object capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native object capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["width"], 1088);
    assert_eq!(metadata["image"]["height"], 124);
    assert_eq!(metadata["image"]["composition"], "isolated_regions");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        metadata["image"]["content_pixels"].as_u64().unwrap() < 1088 * 124,
        "native object color capture should isolate glyph regions inside the textbox crop"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_layer_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-layer-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "layer": "dialogue.rich_text",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}}),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP layer source");
    assert!(
        output.status.success(),
        "agent mcp native layer capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 4);
    assert_mcp_png_capture_content(&responses[1], "native layer capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native layer capture metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(metadata["image"]["composition"], "isolated_regions");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 1088);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(metadata["image"]["crop_origin"]["x"].as_u64().unwrap() >= 96);
    assert_mcp_png_capture_content(&responses[2], "native layer resource metadata is JSON");
    let read_metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native layer resource metadata is JSON",
    );
    assert_eq!(read_metadata["image"]["renderer"], "native");
    assert_eq!(read_metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(read_metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(read_metadata["image"]["composition"], "isolated_regions");
    assert_native_layer_resource_descriptor(&responses[3]);
}

fn assert_native_layer_resource_descriptor(response: &serde_json::Value) {
    let resources = response["result"]["resources"].as_array().unwrap();
    let layer_image = resources
        .iter()
        .find(|resource| {
            let is_layer_png = resource["uri"]
                .as_str()
                .and_then(|uri| uri.split('?').next())
                .is_some_and(|uri| uri.ends_with("/layer.dialogue.rich_text.png"));
            let is_native = resource["description"]
                .as_str()
                .is_some_and(|description| description.contains("renderer=native"));
            is_layer_png && is_native
        })
        .expect("resources/list should expose the latest native layer capture");
    assert_eq!(layer_image["mimeType"], "image/png");
    assert!(
        layer_image["description"]
            .as_str()
            .is_some_and(|description| description.contains("kind=color")
                && description.contains("renderer=native")
                && description.contains("scope=layer:dialogue.rich_text")
                && description.contains("composition=isolated_regions")
                && description.contains("width=")
                && description.contains("height=")),
        "native layer descriptor should expose latest capture metadata"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_reads_latest_native_layer_image_resource() {
    let path = temp_arcw(
        "agent-mcp-native-layer-read-resource",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "image": "png",
                    "layer": "dialogue.rich_text",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.resource.read",
                "arguments": {
                    "uri": "arcweft://session/cli/frame/0/layer.dialogue.rich_text.png"
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP layer resource source");
    assert!(
        output.status.success(),
        "agent mcp native layer resource read should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);
    assert_mcp_png_capture_content(&responses[2], "native layer read metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "native layer read metadata is JSON",
    );
    assert_eq!(metadata["image"]["renderer"], "native");
    assert_eq!(metadata["image"]["scope"]["kind"], "layer");
    assert_eq!(metadata["image"]["scope"]["id"], "dialogue.rich_text");
    assert_eq!(metadata["image"]["composition"], "isolated_regions");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 1088);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 124);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_ruby_element_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-ruby-capture",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.ruby.0",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP ruby source");
    assert!(
        output.status.success(),
        "agent mcp native ruby capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_mcp_png_capture_content(&responses[1], "native ruby capture metadata is JSON");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native ruby capture metadata is JSON",
    );
    assert!(
        metadata["image"]["width"].as_u64().unwrap() < 180,
        "native ruby element crop should be much narrower than the textbox"
    );
    assert!(
        metadata["image"]["height"].as_u64().unwrap() < 120,
        "native ruby element crop should be much shorter than the textbox"
    );
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(
        metadata["image"]["crop_origin"]["x"].as_u64().unwrap() >= 96,
        "native ruby crop origin should map back to viewport coordinates"
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_captures_source_ruby_object_id_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-ruby-object-id",
        r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r]
}
"#,
    );
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.object-id.png",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP ruby object-id source");
    assert!(
        output.status.success(),
        "agent mcp native ruby object-id should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[1]["result"]["content"][0]["type"], "text");
    let metadata = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "native ruby object-id metadata is JSON",
    );
    assert_eq!(metadata["image"]["kind"], "object_id");
    assert_eq!(metadata["image"]["composition"], "object_id_attachment");
    assert!(metadata["image"]["width"].as_u64().unwrap() < 180);
    assert!(metadata["image"]["height"].as_u64().unwrap() < 120);
    assert_eq!(metadata["image"]["crop_origin"]["space"], "viewport");
    assert!(metadata["image"]["content_pixels"].as_u64().unwrap() > 0);
    assert!(
        responses[1]["result"]["content"][1]["data"]
            .as_str()
            .is_some_and(|blob| blob.starts_with("iVBORw0KGgo"))
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_capture_time_controls_text_combine_mask_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-typewriter-text-combine-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl][.typewriter cps=1]2026[/][/][p]
}
",
    );
    let object_id = "object.dialogue.0.0.cluster.0.0.4";
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "raw-rgba",
                    "capture": "mask",
                    "object": object_id,
                    "capture_time": 0.0,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "raw-rgba",
                    "capture": "mask",
                    "object": object_id,
                    "capture_time": 4.0,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP text-combine source");
    assert!(
        output.status.success(),
        "agent mcp native typewriter text-combine capture-time should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);

    let hidden = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "hidden text-combine mask metadata is JSON",
    );
    let visible = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "visible text-combine mask metadata is JSON",
    );
    assert_eq!(hidden["image"]["kind"], "mask");
    assert_eq!(visible["image"]["kind"], "mask");
    assert_eq!(hidden["image"]["composition"], "mask_attachment");
    assert_eq!(visible["image"]["composition"], "mask_attachment");
    assert_eq!(hidden["image"]["scope"]["kind"], "object");
    assert_eq!(hidden["image"]["scope"]["id"], object_id);
    assert_eq!(visible["image"]["scope"]["id"], object_id);
    assert_eq!(
        hidden["image"]["crop_origin"],
        visible["image"]["crop_origin"]
    );
    assert_eq!(hidden["image"]["width"], visible["image"]["width"]);
    assert_eq!(hidden["image"]["height"], visible["image"]["height"]);
    assert_eq!(hidden["image"]["content_pixels"], 0);
    assert!(visible["image"]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_bytes = mcp_raw_capture_bytes(&responses[1]);
    let visible_bytes = mcp_raw_capture_bytes(&responses[2]);
    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_eq!(
        opaque_pixel_count(&visible_bytes) as u64,
        visible["image"]["content_pixels"].as_u64().unwrap()
    );
}

#[test]
#[ignore = "tier 2 MCP stdio E2E: slow subprocess/native-capture coverage"]
fn agent_mcp_stdio_capture_time_controls_ruby_object_id_with_native_renderer() {
    let path = temp_arcw(
        "agent-mcp-native-typewriter-ruby-object-id-capture-time",
        r"
character @character.alice Alice as alice {}

flow @flow.main main {
    alice: [.vertical_rl]天地春夏秋冬[.typewriter cps=1]|[夢](ながいながいよみ)人外[/][/][p]
}
",
    );
    let object_id = "object.dialogue.0.0.ruby.0";
    let requests = [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "raw-rgba",
                    "capture": "object-id",
                    "object": object_id,
                    "capture_time": 0.0,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "source": path.display().to_string(),
                    "format": "raw-rgba",
                    "capture": "object-id",
                    "object": object_id,
                    "capture_time": 4.0,
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
    ];
    let output = run_agent_mcp_stdio(&requests);
    fs::remove_file(&path).expect("remove temp native MCP ruby object-id capture-time source");
    assert!(
        output.status.success(),
        "agent mcp native typewriter ruby object-id capture-time should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = agent_mcp_responses(&output.stdout);
    assert_eq!(responses.len(), 3);

    let hidden = mcp_content_metadata(
        &responses[1]["result"]["content"][0],
        "hidden ruby object-id metadata is JSON",
    );
    let visible = mcp_content_metadata(
        &responses[2]["result"]["content"][0],
        "visible ruby object-id metadata is JSON",
    );
    assert_eq!(hidden["image"]["kind"], "object_id");
    assert_eq!(visible["image"]["kind"], "object_id");
    assert_eq!(hidden["image"]["composition"], "object_id_attachment");
    assert_eq!(visible["image"]["composition"], "object_id_attachment");
    assert_eq!(hidden["image"]["scope"]["kind"], "object");
    assert_eq!(hidden["image"]["scope"]["id"], object_id);
    assert_eq!(visible["image"]["scope"]["id"], object_id);
    assert_eq!(
        hidden["image"]["crop_origin"],
        visible["image"]["crop_origin"]
    );
    assert_eq!(hidden["image"]["width"], visible["image"]["width"]);
    assert_eq!(hidden["image"]["height"], visible["image"]["height"]);
    assert_eq!(hidden["image"]["content_pixels"], 0);
    assert!(visible["image"]["content_pixels"].as_u64().unwrap() > 0);

    let hidden_bytes = mcp_raw_capture_bytes(&responses[1]);
    let visible_bytes = mcp_raw_capture_bytes(&responses[2]);
    assert_eq!(opaque_pixel_count(&hidden_bytes), 0);
    assert_eq!(
        opaque_pixel_count(&visible_bytes) as u64,
        visible["image"]["content_pixels"].as_u64().unwrap()
    );
}

fn agent_mcp_rich_text_requests(path: &std::path::Path) -> [serde_json::Value; 10] {
    [
        serde_json::json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
        serde_json::json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "arcweft.observe",
                "arguments": {
                    "source": path.display().to_string(),
                    "image": "png",
                    "layer": "dialogue.rich_text",
                    "steps": 4,
                    "max_ops": 64
                }
            }
        }),
        serde_json::json!({"jsonrpc": "2.0", "id": 4, "method": "resources/list", "params": {}}),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 5,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.ruby.0"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 6,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "format": "png",
                    "capture": "color",
                    "object": "object.dialogue.0.0.ruby.0"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 7,
            "method": "tools/call",
            "params": {
                "name": "arcweft.capture",
                "arguments": {
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
                }
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 8,
            "method": "tools/call",
            "params": {
                "name": "arcweft.session.info",
                "arguments": {}
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 9,
            "method": "resources/read",
            "params": {
                "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.mask.rgba"
            }
        }),
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 10,
            "method": "resources/read",
            "params": {
                "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.ruby.0.png"
            }
        }),
    ]
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
fn bundle_json_packages_save_files_and_run_bundle_executes_native_file_tasks() {
    let fixture = bundle_native_file_fixture();
    let bundle_stdout = run_bundle_package_command(&fixture);
    assert_bundle_package_output(&fixture, &bundle_stdout);

    let run_stdout = run_bundle_fixture_command(&fixture);
    assert_run_bundle_output(&fixture, &run_stdout);

    let build_stdout = run_build_bundle_command(&fixture);
    assert_build_bundle_output(&fixture, &build_stdout);
}

struct BundleNativeFileFixture {
    dir: PathBuf,
    source_path: PathBuf,
    bundle_path: PathBuf,
}

fn bundle_native_file_fixture() -> BundleNativeFileFixture {
    let dir = temp_dir("bundle-native-file-task");
    let source_path = dir.join("main.arcw");
    let save_dir = dir.join(".arcweft").join("save");
    let bundle_path = dir.join("game.awfb");
    fs::create_dir_all(&save_dir).expect("create virtual save root");
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
entry cli @entry.main { run(@flow.main) }
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
            && bundle_stdout.contains("\"virtual_files\": 1"),
        "bundle JSON should describe native requirements and packaged save input: {bundle_stdout}"
    );
    assert!(
        !bundle_stdout.contains(&fixture.dir.display().to_string()),
        "bundle JSON must not record absolute temp paths: {bundle_stdout}"
    );
    let bundle_json = fs::read_to_string(&fixture.bundle_path).expect("bundle JSON is written");
    assert!(
        bundle_json.contains("\"adapter_manifest_ids\"")
            && bundle_json.contains("\"adapter_manifests\"")
            && bundle_json.contains("\"bytecode\"")
            && bundle_json.contains("\"program\"")
            && bundle_json.contains("native-file")
            && bundle_json.contains("save")
            && bundle_json.contains("input.txt"),
        "bundle artifact should include executable bytecode, native-file adapter metadata, and relative save file: {bundle_json}"
    );
    assert!(
        !bundle_json.contains(&fixture.dir.display().to_string()),
        "bundle artifact must not record absolute temp paths: {bundle_json}"
    );
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
    measure iterations = 2 { start(@flow.bracket_pure) }
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
    measure iterations = 2 { start(@flow.bracket_pure_sum) }
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
    measure iterations = 2 { start(@flow.bracket_pure) }
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
    measure iterations = 2 { start(@flow.map_pure) }
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
    measure iterations = 2 { start(@flow.map_pure_sum) }
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
    measure iterations = 2 { start(@flow.map_pure) }
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
    measure iterations = 1 { start(@flow.threads) }
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
    measure iterations = 3 { start(@flow.infer_matmul_bias_add_f32) }
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
    measure iterations = 1 { start(@flow.threaded_reads) }
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

fn filesystem_safe_test_label(label: &str) -> String {
    label
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

fn imq_is_available() -> bool {
    Command::new("imq")
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn capture_native_png_report(source_path: &Path, png_path: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--out")
        .arg(png_path)
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("64")
        .output()
        .expect("arcw agent observe writes native PNG");

    assert!(
        output.status.success(),
        "native PNG capture should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let bytes = fs::read(png_path).expect("read native PNG");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    serde_json::from_slice(&output.stdout).expect("native capture report is JSON")
}

fn observe_native_rich_text_layer_report(source_path: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("agent")
        .arg("observe")
        .arg(source_path)
        .arg("--json")
        .arg("--image")
        .arg("png")
        .arg("--layer")
        .arg("dialogue.rich_text")
        .arg("--page")
        .arg("0")
        .arg("--mode")
        .arg("drain")
        .arg("--steps")
        .arg("4")
        .arg("--max-ops")
        .arg("128")
        .output()
        .expect("arcw agent observe reports native rich-text layer");

    assert!(
        output.status.success(),
        "native rich-text layer observe should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("native rich-text layer report is JSON")
}

fn assert_native_rich_text_layer_image_has_content(report: &serde_json::Value) {
    let image = &report["images"][0];
    assert_eq!(image["kind"], "color");
    assert_eq!(image["renderer"], "native");
    assert_eq!(image["scope"]["kind"], "layer");
    assert_eq!(image["scope"]["id"], "dialogue.rich_text");
    assert_eq!(image["composition"], "isolated_regions");
    assert_eq!(image["mime_type"], "image/png");
    assert!(image["content_pixels"].as_u64().unwrap() > 0);
}

fn find_rich_text_run_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "rich_text_run" && object["text"] == text)
        .unwrap_or_else(|| panic!("rich-text run `{text}` should be observed: {report}"))
}

fn find_rich_text_cluster_object<'a>(
    report: &'a serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
) -> &'a serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_cluster"
                && object["text"] == text
                && object["rich_text_ref"]["range"]["start"].as_u64() == Some(range_start)
                && object["rich_text_ref"]["range"]["end"].as_u64() == Some(range_end)
        })
        .unwrap_or_else(|| {
            panic!(
                "rich-text cluster `{text}` {range_start}..{range_end} should be observed: {report}"
            )
        })
}

fn find_rich_text_ruby_object(report: &serde_json::Value, index: u64) -> &serde_json::Value {
    report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| {
            object["role"] == "rich_text_ruby"
                && object["rich_text_ref"]["index"].as_u64() == Some(index)
        })
        .unwrap_or_else(|| panic!("rich-text ruby `{index}` should be observed: {report}"))
}

fn first_text_run_presentation_layout(report: &serde_json::Value) -> &serde_json::Value {
    let textbox = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .find(|object| object["role"] == "textbox")
        .unwrap_or_else(|| panic!("textbox object should be observed: {report}"));
    let run = textbox["rich_text"]["display_map"]["text_runs"]
        .as_array()
        .expect("text runs are reported")
        .first()
        .unwrap_or_else(|| panic!("first text run should be observed: {report}"));
    &run["presentation"]["layout"]
}

fn assert_rich_text_cluster_metadata(
    report: &serde_json::Value,
    text: &str,
    range_start: u64,
    range_end: u64,
    orientation: &str,
    vertical_form: &str,
) {
    let cluster = find_rich_text_cluster_object(report, text, range_start, range_end);
    assert_eq!(cluster["rich_text_ref"]["orientation"], orientation);
    assert_eq!(cluster["rich_text_ref"]["vertical_form"], vertical_form);
}

fn rich_text_cluster_column_count(report: &serde_json::Value) -> usize {
    let mut columns = report["objects"]
        .as_array()
        .expect("objects are reported")
        .iter()
        .filter(|object| object["role"] == "rich_text_cluster")
        .map(|object| agent_json_bbox_x(&object["bbox"]))
        .collect::<Vec<_>>();
    columns.sort_unstable();
    columns.dedup();
    columns.len()
}

fn assert_rich_text_object_has_mask_capture(object: &serde_json::Value, context: &str) {
    assert!(
        object["capture_refs"]["captures"]
            .as_array()
            .expect("rich-text object captures are reported")
            .iter()
            .any(|capture| capture["kind"] == "mask"
                && capture["uri"]
                    .as_str()
                    .is_some_and(|uri| uri.ends_with(".mask.rgba"))),
        "{context} should expose native mask capture refs: {object}"
    );
}

fn agent_json_bboxes_intersect(left: &serde_json::Value, right: &serde_json::Value) -> bool {
    agent_json_bbox_x(left) < agent_json_bbox_right(right)
        && agent_json_bbox_x(right) < agent_json_bbox_right(left)
        && agent_json_bbox_y(left) < agent_json_bbox_bottom(right)
        && agent_json_bbox_y(right) < agent_json_bbox_bottom(left)
}

fn agent_json_bbox_x(bbox: &serde_json::Value) -> u64 {
    bbox["x"].as_u64().expect("bbox x is reported")
}

fn agent_json_bbox_y(bbox: &serde_json::Value) -> u64 {
    bbox["y"].as_u64().expect("bbox y is reported")
}

fn agent_json_bbox_height(bbox: &serde_json::Value) -> u64 {
    bbox["height"].as_u64().expect("bbox height is reported")
}

fn agent_json_bbox_width(bbox: &serde_json::Value) -> u64 {
    bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_right(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_x(bbox) + bbox["width"].as_u64().expect("bbox width is reported")
}

fn agent_json_bbox_bottom(bbox: &serde_json::Value) -> u64 {
    agent_json_bbox_y(bbox) + bbox["height"].as_u64().expect("bbox height is reported")
}

fn assert_vertical_cluster_after(
    previous: &serde_json::Value,
    next: &serde_json::Value,
    context: &str,
) {
    assert_eq!(
        previous["bbox"]["x"], next["bbox"]["x"],
        "{context}: clusters should share the same vertical column"
    );
    let previous_y = previous["bbox"]["y"]
        .as_i64()
        .expect("previous cluster y is numeric");
    let next_y = next["bbox"]["y"]
        .as_i64()
        .expect("next cluster y is numeric");
    assert!(
        next_y > previous_y,
        "{context}: next cluster should advance downward within the column"
    );
}

fn rich_text_object_capture_uri<'a>(
    object: &'a serde_json::Value,
    kind: &str,
    mime_type: &str,
) -> &'a str {
    object["capture_refs"]["captures"]
        .as_array()
        .expect("rich-text object has capture refs")
        .iter()
        .find(|capture| capture["kind"] == kind && capture["mime_type"] == mime_type)
        .and_then(|capture| capture["uri"].as_str())
        .unwrap_or_else(|| {
            panic!("rich-text object should have {kind}/{mime_type} capture URI: {object}")
        })
}

fn assert_native_capture_has_content(report: &serde_json::Value, written_name: &str) {
    assert_eq!(report["images"][0]["kind"], "color");
    assert_eq!(report["images"][0]["renderer"], "native");
    assert_eq!(report["images"][0]["composition"], "framebuffer");
    assert_eq!(report["images"][0]["mime_type"], "image/png");
    assert_eq!(report["images"][0]["width"], 1280);
    assert_eq!(report["images"][0]["height"], 720);
    assert!(report["images"][0]["content_pixels"].as_u64().unwrap() > 0);
    assert_eq!(report["images"][0]["written"], written_name);
}

fn png_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    (bytes.len() >= 24 && &bytes[..8] == PNG_SIGNATURE && &bytes[12..16] == b"IHDR").then(|| {
        let width = u32::from_be_bytes(bytes[16..20].try_into().expect("IHDR width bytes"));
        let height = u32::from_be_bytes(bytes[20..24].try_into().expect("IHDR height bytes"));
        (width, height)
    })
}

fn metric_score(report: &serde_json::Value, metric_name: &str) -> f64 {
    metric_entry(report, metric_name)["score"]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name} score should be numeric: {report}"))
}

fn metric_detail(report: &serde_json::Value, metric_name: &str, detail_name: &str) -> f64 {
    metric_entry(report, metric_name)["details"][detail_name]
        .as_f64()
        .unwrap_or_else(|| panic!("{metric_name}.{detail_name} should be numeric: {report}"))
}

fn metric_entry<'a>(report: &'a serde_json::Value, metric_name: &str) -> &'a serde_json::Value {
    report["metrics"]
        .as_array()
        .and_then(|metrics| {
            metrics
                .iter()
                .find(|metric| metric["name"].as_str() == Some(metric_name))
        })
        .unwrap_or_else(|| panic!("{metric_name} metric should be present: {report}"))
}

fn assert_metric_close(actual: f64, expected: f64, epsilon: f64, label: &str) {
    assert!(
        (actual - expected).abs() <= epsilon,
        "{label} should be {expected}, got {actual}"
    );
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
        name: "mini_games.truck.score_to_rank".to_owned(),
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
