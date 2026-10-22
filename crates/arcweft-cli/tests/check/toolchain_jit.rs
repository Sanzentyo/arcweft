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
fn score(base: i64, bonus: i64, scale: i64) -> i64 effects {} {
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

const RUNTIME_PURE_JIT_SOURCE: &str = r"
fn score(base: i64, bonus: i64) -> i64 effects {} {
    return base * (bonus + 2)
}

entry cli @entry.cli.main { goto @flow.main }

flow main() -> i64 effects {} {
    return score(3, 4)
}
";

#[test]
fn run_json_uses_jit_for_runtime_pure_calls_without_arg_vec_allocation() {
    let path = temp_arcw("runtime-pure-jit", RUNTIME_PURE_JIT_SOURCE);
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .args(["--entry", "entry.cli.main"])
        .arg("--json")
        .arg("--mode")
        .arg("drain")
        // One caller dispatch and the original one-operation Return body.
        .arg("--max-ops")
        .arg("2")
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
    let steps = json["steps"]
        .as_array()
        .expect("run output includes the runtime step deltas");
    assert!(!steps.is_empty());
    let pure_counter = |key: &str| {
        steps
            .iter()
            .map(|step| {
                step["stats"]["pure"][key]
                    .as_u64()
                    .expect("each step includes its typed runtime pure counter")
            })
            .sum::<u64>()
    };
    assert_eq!(pure_counter("pure_calls"), 1);
    assert_eq!(pure_counter("jit_calls"), 1);
    assert_eq!(pure_counter("arg_stack_packs"), 0);
    assert_eq!(pure_counter("arg_vec_allocations"), 0);
    assert_eq!(pure_counter("arg_bytes_copied"), 0);
    assert_eq!(pure_counter("arg_bytes_borrowed"), 16);
    assert_eq!(pure_counter("result_bytes_copied"), 0);
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
fn run_json_retains_original_budget_and_one_op_scalar_continuations() {
    let path = temp_arcw("runtime-pure-jit-budget", RUNTIME_PURE_JIT_SOURCE);
    for backend in ["jit", "auto"] {
        for mode in ["drain", "one-op"] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_arcw"));
            command
                .arg("run")
                .arg(&path)
                .args(["--entry", "entry.cli.main"])
                .arg("--json")
                .args(["--mode", mode])
                .args(["--steps", "5"])
                .args(["--pure-backend", backend])
                .args(["--pure-workers", "1"]);
            if mode == "one-op" {
                // The mode owns suspension after one op even with enough cost.
                command.args(["--max-ops", "2"]);
            }
            // Drain intentionally retains the original omitted max_ops=1.
            let output = command.output().expect("run original bounded source");
            assert!(
                output.status.success(),
                "{backend}/{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["final_status"], "done Return(\"18\")");
            let steps = json["steps"].as_array().expect("actual step deltas");
            assert!(
                steps.len() > 1,
                "the original call remains resumable: {json}"
            );
            let pure_counter = |key: &str| {
                steps
                    .iter()
                    .map(|step| {
                        step["stats"]["pure"][key]
                            .as_u64()
                            .expect("every step has the actual u64 counter")
                    })
                    .sum::<u64>()
            };
            for step in steps {
                assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= 1);
                assert_eq!(step["diagnostics"], serde_json::json!([]));
            }
            assert_eq!(pure_counter("pure_calls"), 1, "semantic interpreted call");
            assert_eq!(pure_counter("vm_calls"), 1);
            assert_eq!(pure_counter("jit_calls"), 0);
            assert_eq!(pure_counter("aot_calls"), 0);
            assert_eq!(pure_counter("arg_bytes_borrowed"), 0);
            let prepared = &json["executor_stats"]["pure_compile"];
            assert_eq!(prepared["auto_jit_promotions"], 0);
            if backend == "jit" {
                assert_eq!(prepared["jit_attempts"], 1, "explicit JIT preparation");
                assert_eq!(prepared["jit_successes"], 1);
            } else {
                assert_eq!(prepared["jit_attempts"], 0);
                assert_eq!(prepared["jit_successes"], 0);
                assert_eq!(
                    prepared["aot_successes"], 1,
                    "Auto retains its prepared baseline"
                );
            }
        }
    }
    fs::remove_file(path).expect("remove original bounded source");
}
#[test]
fn run_json_can_record_aot_object_artifact_stats_when_requested() {
    let path = temp_arcw(
        "runtime-aot-object-artifacts",
        r"
fn score(base: i32, bonus: i32) -> i32 effects {} {
    return base * (bonus + 2i32)
}

entry cli @entry.cli.main { goto @flow.main }

flow main() -> i32 effects {} {
    return score(3i32, 4i32)
}
",
    );
    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .args(["--entry", "entry.cli.main"])
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

const RUNTIME_FOR_PURE_JIT_SOURCE: &str = r#"
fn score(base: i64, bonus: i64) -> i64 effects {} {
    return base * (bonus + 2i64)
}

entry cli @entry.cli.main { goto @flow.for_pure }

flow for_pure() -> String effects { log.write } {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    for item in values {
        let scored = score(item, 2i64)
        log.info(scored)
    }
    return "done"
}
"#;

#[test]
fn run_json_uses_jit_for_for_loop_pure_calls_without_arg_vec_allocation() {
    let path = temp_arcw("runtime-for-pure-jit", RUNTIME_FOR_PURE_JIT_SOURCE);

    let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
        .arg("run")
        .arg(&path)
        .args(["--entry", "entry.cli.main"])
        .arg("--mode")
        .arg("drain")
        .args(["--max-ops", "64"])
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
    let steps = json["steps"]
        .as_array()
        .expect("run output includes the runtime step deltas");
    assert!(!steps.is_empty());
    let pure_counter = |key: &str| {
        steps
            .iter()
            .map(|step| {
                step["stats"]["pure"][key]
                    .as_u64()
                    .expect("each step includes its typed runtime pure counter")
            })
            .sum::<u64>()
    };
    assert_eq!(pure_counter("pure_calls"), 4);
    assert_eq!(pure_counter("jit_calls"), 4);
    assert_eq!(pure_counter("arg_stack_packs"), 0);
    assert_eq!(pure_counter("arg_vec_allocations"), 0);
    assert_eq!(pure_counter("arg_bytes_copied"), 0);
    assert_eq!(pure_counter("arg_bytes_borrowed"), 64);
    assert_eq!(pure_counter("result_bytes_copied"), 0);
    assert_eq!(json["executor_stats"]["pure_config"]["backend"], "jit");
    assert_eq!(
        json["executor_stats"]["pure_config"]["worker_pool_active"],
        false
    );
    assert_eq!(json["executor_stats"]["pure_compile"]["jit_successes"], 1);
}

#[test]
fn run_json_retains_original_for_loop_one_op_budget_and_workload() {
    let path = temp_arcw("runtime-for-pure-jit-original", RUNTIME_FOR_PURE_JIT_SOURCE);
    for backend in ["jit", "auto"] {
        for mode in ["one-op", "drain"] {
            let output = Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args(["--entry", "entry.cli.main", "--json"])
                .args(["--mode", mode, "--steps", "32"])
                .args(["--pure-backend", backend])
                // Keep the original omitted max_ops=1 for the same four rows.
                .output()
                .expect("original bounded for-loop executes");
            assert!(
                output.status.success(),
                "{backend}/{mode}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["final_status"], "done Return(\"done\")", "{json}");
            let steps = json["steps"].as_array().expect("actual step deltas");
            assert!(steps.len() > 1 && steps.len() <= 32, "{json}");
            for step in steps {
                assert!(step["stats"]["executed_ops"].as_u64().unwrap() <= 1);
                assert_eq!(step["diagnostics"], serde_json::json!([]));
            }
            let pure_counter = |key: &str| {
                steps
                    .iter()
                    .map(|step| step["stats"]["pure"][key].as_u64().unwrap())
                    .sum::<u64>()
            };
            assert_eq!(pure_counter("pure_calls"), 4);
            assert_eq!(pure_counter("vm_calls"), 4);
            assert_eq!(pure_counter("jit_calls"), 0);
            assert_eq!(pure_counter("aot_calls"), 0);
            assert_eq!(pure_counter("arg_stack_packs"), 0);
            assert_eq!(pure_counter("arg_vec_allocations"), 0);
            assert_eq!(pure_counter("arg_bytes_copied"), 0);
            assert_eq!(pure_counter("arg_bytes_borrowed"), 0);
            assert_eq!(pure_counter("result_bytes_copied"), 0);
        }
    }
    fs::remove_file(path).expect("remove original bounded for-loop source");
}

mod standalone_pure_batch_owner_tests {
    use arcweft_core::pure::{RuntimePureCallBackend, RuntimePureFunctionRef};
    use arcweft_core::step::RuntimeStepBudget;
    use arcweft_core::value::RuntimeEvalError;
    use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureBackendMode};

    fn selected(source: &str) -> arcweft_compiler::project::CompiledDeterministicFunction {
        let compiled = arcweft_compiler::source::compile_source(source).unwrap();
        compiled
            .analysis
            .compile_deterministic_function(
                Some("score"),
                &arcweft_compiler::lower::ProjectInstantiationControl::default(),
            )
            .unwrap()
    }

    const DIRECT: &str = r"
fn score(base: i64, unused: i64) -> i64 effects {} {
    let value = base + 4i64
    return value + unused
}
";
    const NESTED: &str = r"
fn inner(base: i64) -> i64 effects {} { return base + 4i64 }
fn score(base: i64, unused: i64) -> i64 effects {} {
    let value = inner(base)
    return value + unused
}
";

    #[test]
    fn standalone_batch_original_control_cost_bounds_vm_aot_jit_auto_before_completion() {
        let selected = selected(DIRECT);
        let program = selected.program();
        let function =
            RuntimePureFunctionRef::resolve(program.plan(), program.function_site().unwrap())
                .unwrap();
        assert!(function.body.is_executable());
        let cost = function.total_numeric_completion_control_ops().unwrap();
        assert!(cost > 0);
        for mode in [
            RuntimePureBackendMode::Vm,
            RuntimePureBackendMode::Aot,
            RuntimePureBackendMode::Jit,
            RuntimePureBackendMode::Auto,
        ] {
            let mut backend = RuntimePureAccelerator::new(mode, program.plan());
            let mut complete = [-1_i64; 3];
            backend
                .call_i64_flat_batch_invocations(
                    function,
                    &[1, 40, 2, 30, 3, 20],
                    2,
                    &mut complete,
                    RuntimeStepBudget { max_ops: cost },
                )
                .unwrap();
            assert_eq!(complete, [45, 36, 27]);
            assert_eq!(backend.stats().pure_calls, 3);
            match mode {
                RuntimePureBackendMode::Vm => assert_eq!(backend.stats().vm_calls, 3),
                RuntimePureBackendMode::Aot => assert_eq!(backend.stats().aot_calls, 3),
                RuntimePureBackendMode::Jit => assert_eq!(backend.stats().jit_calls, 3),
                RuntimePureBackendMode::Auto => {
                    assert_eq!(backend.stats().aot_calls + backend.stats().jit_calls, 3,)
                }
            }
            backend.reset_runtime_counters();
            let mut denied = [91, 92, 93];
            let error = backend
                .call_i64_flat_batch_invocations(
                    function,
                    &[1, 40, 2, 30, 3, 20],
                    2,
                    &mut denied,
                    RuntimeStepBudget { max_ops: cost - 1 },
                )
                .unwrap_err();
            assert!(
                matches!(error, RuntimeEvalError::UnsupportedPure { ref reason, .. }
                if reason.contains("exhausted its explicit"))
            );
            assert_eq!(denied, [91, 92, 93]);
            assert_eq!(backend.stats().pure_calls, 1);
            assert_eq!(backend.stats().vm_calls, 1);
            assert_eq!(backend.stats().aot_calls, 0);
            assert_eq!(backend.stats().jit_calls, 0);
            assert_eq!(backend.stats().result_bytes_copied, 0);
        }
    }

    #[test]
    fn standalone_batch_nested_control_uses_bounded_vm_on_explicit_aot_auto_decline() {
        let selected = selected(NESTED);
        let program = selected.program();
        let function =
            RuntimePureFunctionRef::resolve(program.plan(), program.function_site().unwrap())
                .unwrap();
        assert!(function.body.is_executable());
        assert_eq!(function.total_numeric_completion_control_ops(), None);
        for mode in [
            RuntimePureBackendMode::Vm,
            RuntimePureBackendMode::Aot,
            RuntimePureBackendMode::Auto,
        ] {
            let mut backend = RuntimePureAccelerator::new(mode, program.plan());
            let mut out = [0_i64; 3];
            backend
                .call_i64_flat_batch_invocations(
                    function,
                    &[1, 40, 2, 30, 3, 20],
                    2,
                    &mut out,
                    RuntimeStepBudget { max_ops: 64 },
                )
                .unwrap();
            assert_eq!(out, [45, 36, 27]);
            assert_eq!(backend.stats().pure_calls, 3);
            assert_eq!(backend.stats().vm_calls, 3);
            assert_eq!(backend.stats().aot_calls, 0);
            assert_eq!(backend.stats().jit_calls, 0);
            backend.reset_runtime_counters();
            let error = backend
                .call_i64_flat_batch_invocations(
                    function,
                    &[1, 40, 2, 30, 3, 20],
                    2,
                    &mut out,
                    RuntimeStepBudget { max_ops: 0 },
                )
                .unwrap_err();
            assert!(
                matches!(error, RuntimeEvalError::UnsupportedPure { ref reason, .. }
                if reason.contains("exhausted its explicit 0-operation budget"))
            );
            assert_eq!(out, [45, 36, 27]);
            assert_eq!(backend.stats().pure_calls, 1);
            assert_eq!(backend.stats().vm_calls, 1);
            assert_eq!(backend.stats().result_bytes_copied, 0);
        }
    }

    #[test]
    fn standalone_batch_mapping_fault_preserves_completed_prefix_without_native_retry() {
        let selected = selected(
            r"
fn score(base: i64, divisor: i64) -> i64 effects {} { return base / divisor }
",
        );
        let program = selected.program();
        let function =
            RuntimePureFunctionRef::resolve(program.plan(), program.function_site().unwrap())
                .unwrap();
        assert_eq!(function.total_numeric_completion_control_ops(), None);
        let budget = RuntimeStepBudget { max_ops: 64 };
        let request = arcweft_core::pure::PureFunctionRequest::try_new(
            std::sync::Arc::clone(program.plan()),
            function.id(),
            [
                arcweft_core::value::RuntimeValue::i64(8),
                arcweft_core::value::RuntimeValue::i64(0),
            ],
        )
        .unwrap();
        let expected = arcweft_core::pure::VmPureFunctionBackend
            .evaluate_invocation(&request, budget)
            .unwrap_err();
        assert!(
            matches!(&expected, RuntimeEvalError::UnsupportedPure { reason, .. }
            if reason.contains("division by zero"))
        );
        for mode in [
            RuntimePureBackendMode::Vm,
            RuntimePureBackendMode::Aot,
            RuntimePureBackendMode::Jit,
            RuntimePureBackendMode::Auto,
        ] {
            let mut backend = RuntimePureAccelerator::new(mode, program.plan());
            let mut out = [91_i64, 92, 93];
            let error = backend
                .call_i64_flat_batch_invocations(function, &[9, 3, 8, 0, 7, 1], 2, &mut out, budget)
                .unwrap_err();
            assert_eq!(error, expected);
            assert_eq!(out, [3, 92, 93]);
            assert_eq!(backend.stats().pure_calls, 2);
            assert_eq!(backend.stats().vm_calls, 2);
            assert_eq!(backend.stats().aot_calls, 0);
            assert_eq!(backend.stats().jit_calls, 0);
            assert_eq!(backend.stats().result_bytes_copied, 8);
        }
    }

    #[test]
    fn standalone_batch_foreign_plan_and_incomplete_formals_reject_before_execution() {
        let first = selected(DIRECT);
        let second = selected(DIRECT);
        let function = RuntimePureFunctionRef::resolve(
            first.program().plan(),
            first.program().function_site().unwrap(),
        )
        .unwrap();
        let foreign = RuntimePureFunctionRef::resolve(
            second.program().plan(),
            second.program().function_site().unwrap(),
        )
        .unwrap();
        let mut backend =
            RuntimePureAccelerator::new(RuntimePureBackendMode::Vm, first.program().plan());
        let mut out = [91_i64; 1];
        assert!(matches!(backend.call_i64_flat_batch_invocations(
            foreign, &[1, 40], 2, &mut out, RuntimeStepBudget { max_ops: 64 },
        ), Err(RuntimeEvalError::UnsupportedPure { reason, .. })
            if reason == "standalone batch belongs to another admitted plan"));
        assert_eq!(out, [91]);
        assert_eq!(backend.stats().pure_calls, 0);
        assert!(matches!(backend.call_i64_flat_batch_invocations(
            function, &[1], 1, &mut out, RuntimeStepBudget { max_ops: 64 },
        ), Err(RuntimeEvalError::UnsupportedPure { reason, .. })
            if reason == "standalone batch requires the complete direct i64 function ABI"));
        assert_eq!(out, [91]);
        assert_eq!(backend.stats().pure_calls, 0);
    }

    #[test]
    fn compiler_named_function_query_uses_one_checked_root_and_strict_uniqueness() {
        let source = r"
fn score(base: i64, unused: i64) -> i64 { return base + unused }
fn other() -> i64 { return 2i64 }
";
        let compiled = arcweft_compiler::source::compile_source(source).unwrap();
        let control = arcweft_compiler::lower::ProjectInstantiationControl::default();
        let selected = compiled
            .analysis
            .compile_deterministic_function(Some("score"), &control)
            .unwrap();
        assert_eq!(selected.name(), "score");
        assert_eq!(selected.input_labels(), ["base", "unused"]);
        assert!(selected.program().plan().pure_helpers().is_empty());
        assert_eq!(selected.program().plan().pure_programs().len(), 1);
        assert!(selected.program().plan().entries().is_empty());
        assert!(matches!(
            compiled
                .analysis
                .compile_deterministic_function(Some("missing"), &control),
            Err(arcweft_compiler::project::DeterministicFunctionCompileError::Missing { .. })
        ));
        assert!(matches!(
            compiled
                .analysis
                .compile_deterministic_function(None, &control),
            Err(arcweft_compiler::project::DeterministicFunctionCompileError::Ambiguous { .. })
        ));
    }
}

mod scalar_live_budget_owner_tests {
    use arcweft_core::engine::{Engine, FlowFiberStatus};
    use arcweft_core::plan::FlowOp;
    use arcweft_core::pure::{RuntimePureCallBackend, VmRuntimePureCallBackend};
    use arcweft_core::step::{
        RuntimeStepBudget, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStopReason,
    };
    use arcweft_core::value::RuntimeValue;
    use arcweft_runtime_accelerator::{RuntimePureAccelerator, RuntimePureBackendMode};
    use std::sync::Arc;

    const SOURCE: &str = r#"
fn score(base: i64, bonus: i64, unused: i64) -> i64 effects {} {
    let boosted = bonus + 2i64
    return base * boosted
}
pub fn root(value: i64, unused: i64) -> i64 effects {} {
    let result = score(value, 4i64, unused)
    return result
}
flow main() -> String { return "ok" }
"#;

    fn source_program(source: &str) -> arcweft_compiler::lower::CompiledDeterministicProgram {
        use arcweft_lang_hir::project::HirDeclarationBodyRootRole;
        use arcweft_lang_sema::final_analysis::{
            CheckedExecutionBodyOwner, CheckedExecutionSource,
        };
        let compiled = arcweft_compiler::source::compile_source(source).unwrap();
        let lease = &compiled.analysis;
        let declaration = lease
            .final_analysis()
            .hir_topology()
            .modules()
            .iter()
            .flat_map(|module| module.entries())
            .filter_map(|entry| entry.body())
            .find(|body| body.declaration().name() == "root")
            .unwrap()
            .declaration()
            .clone();
        lease
            .compile_deterministic_program(
                CheckedExecutionSource::InvokeBody(CheckedExecutionBodyOwner::Declaration {
                    declaration,
                    role: HirDeclarationBodyRootRole::FunctionBody,
                }),
                None,
                &arcweft_compiler::lower::ProjectInstantiationControl::default(),
            )
            .unwrap()
    }

    fn native(
        program: &arcweft_compiler::lower::CompiledDeterministicProgram,
        value: i64,
    ) -> Engine {
        Engine::for_program_invocation(
            Arc::clone(program.plan()),
            program.program(),
            vec![RuntimeValue::i64(value), RuntimeValue::i64(77)],
        )
        .unwrap()
    }

    fn until_call(engine: &mut Engine, backend: &mut impl arcweft_core::pure::RuntimeCallBackend) {
        for _ in 0..64 {
            if matches!(
                engine.active_flow_operations().next(),
                Some(FlowOp::ProjectCall { .. })
            ) {
                return;
            }
            let step = engine.step_with_pure_backend(
                Default::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::OneOp,
                    budget: RuntimeStepBudget { max_ops: 64 },
                    ..Default::default()
                },
                backend,
            );
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            assert_eq!(step.stats.executed_ops, 1);
            assert!(engine.take_program_result().unwrap().is_none());
        }
        panic!("the authored root must reach its accepted project call");
    }

    fn finish(
        engine: &mut Engine,
        backend: &mut impl arcweft_core::pure::RuntimeCallBackend,
        options: RuntimeStepOptions,
    ) -> (RuntimeValue, usize) {
        let mut executed = 0_usize;
        for _ in 0..128 {
            let step = engine.step_with_pure_backend(Default::default(), options, backend);
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            assert!(step.stats.executed_ops <= options.budget.max_ops);
            if options.mode == RuntimeStepMode::OneOp {
                assert!(step.stats.executed_ops <= 1);
            }
            executed += step.stats.executed_ops;
            if let Some((_, value)) = engine.take_program_result().unwrap() {
                assert!(engine.take_program_result().unwrap().is_none());
                return (value, executed);
            }
            assert!(
                matches!(step.fiber_status, FlowFiberStatus::Running),
                "{step:?}"
            );
        }
        panic!("the bounded source invocation must complete");
    }

    #[test]
    fn single_return_source_call_span_is_exactly_dispatch_plus_original_return() {
        let selected = source_program(
            r#"
fn score(base: i64, bonus: i64) -> i64 effects {} {
    return base * (bonus + 2)
}
pub fn root(value: i64, unused: i64) -> i64 effects {} {
    let result = score(value, 4)
    return result
}
flow main() -> String { return "ok" }
"#,
        );
        let mut probe = native(&selected, 3);
        let mut vm = VmRuntimePureCallBackend::default();
        until_call(&mut probe, &mut vm);
        let Some(FlowOp::ProjectCall { site }) = probe.active_flow_operations().next() else {
            panic!("original checked source call")
        };
        let result = selected
            .plan()
            .project_call_sites()
            .get(*site)
            .unwrap()
            .result()
            .binding_declarations()
            .next()
            .unwrap()
            .local();
        assert!(probe.fiber().frame.env.get(result).is_none());
        for _ in 0..2 {
            let step = probe.step_with_pure_backend(
                Default::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::OneOp,
                    budget: RuntimeStepBudget { max_ops: 2 },
                    ..Default::default()
                },
                &mut vm,
            );
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            assert_eq!(step.stats.executed_ops, 1);
        }
        assert_eq!(
            probe.fiber().frame.env.get(result),
            Some(&RuntimeValue::i64(18))
        );
        for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
            let mut engine = native(&selected, 3);
            let mut physical = RuntimePureAccelerator::new(mode, selected.plan());
            until_call(&mut engine, &mut physical);
            let step = engine.step_with_pure_backend(
                Default::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::Drain,
                    budget: RuntimeStepBudget { max_ops: 2 },
                    ..Default::default()
                },
                &mut physical,
            );
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            assert_eq!(step.stats.executed_ops, 2);
            assert_eq!(
                engine.fiber().frame.env.get(result),
                Some(&RuntimeValue::i64(18))
            );
            assert_eq!(physical.stats().pure_calls, 1);
            assert_eq!(physical.stats().aot_calls + physical.stats().jit_calls, 1);
            assert_eq!(physical.stats().arg_bytes_borrowed, 16);
            assert_eq!(physical.stats().arg_vec_allocations, 0);
            assert_eq!(physical.stats().arg_bytes_copied, 0);
        }
        for options in [
            RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
            RuntimeStepOptions {
                mode: RuntimeStepMode::OneOp,
                budget: RuntimeStepBudget { max_ops: 2 },
                ..Default::default()
            },
        ] {
            for mode in [RuntimePureBackendMode::Auto, RuntimePureBackendMode::Jit] {
                let mut backend = RuntimePureAccelerator::new(mode, selected.plan());
                let (value, executed) = finish(&mut native(&selected, 3), &mut backend, options);
                assert_eq!(value, RuntimeValue::i64(18));
                assert!(executed > 1);
                assert_eq!(backend.stats().jit_calls + backend.stats().aot_calls, 0);
            }
            let mut plan = selected.plan().as_ref().clone();
            plan.bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([84; 32]).unwrap(),
            )
            .unwrap();
            let product = Arc::new(
                arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                    &plan,
                    &selected.lowering_report().dialogue_content_catalog,
                    "single-return-budget.arcw",
                )
                .lower()
                .unwrap()
                .program,
            );
            let mut executor =
                arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_program_invocation(
                    product,
                    selected.program(),
                    vec![RuntimeValue::i64(3), RuntimeValue::i64(77)],
                    arcweft_core::task::GenerationId::new(0),
                    1,
                )
                .unwrap();
            let mut backend = VmRuntimePureCallBackend::default();
            let mut observed = None;
            for _ in 0..128 {
                let step =
                    executor.step_with_pure_backend(Default::default(), options, &mut backend);
                assert!(step.output.diagnostics.is_empty(), "{step:?}");
                assert!(step.stats.executed_ops <= 1);
                if let Some((program, value)) = executor.take_program_result().unwrap() {
                    assert_eq!(program, selected.program());
                    observed = Some(value);
                    break;
                }
            }
            assert_eq!(observed, Some(RuntimeValue::i64(18)));
            assert_eq!(backend.stats().jit_calls + backend.stats().aot_calls, 0);
            assert!(executor.take_program_result().unwrap().is_none());
        }
    }
    #[test]
    fn physical_source_body_charges_the_same_native_driver_control_ops() {
        let selected = source_program(SOURCE);
        assert!(selected.plan().pure_helpers().is_empty());
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..Default::default()
        };
        let (expected, expected_ops) = finish(
            &mut native(&selected, 3),
            &mut VmRuntimePureCallBackend::default(),
            options,
        );
        assert_eq!(expected, RuntimeValue::i64(18));
        for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
            let mut pure = RuntimePureAccelerator::new(mode, selected.plan());
            let (value, ops) = finish(&mut native(&selected, 3), &mut pure, options);
            assert_eq!(value, expected);
            assert_eq!(
                ops, expected_ops,
                "physical completion must charge the actual original control span"
            );
            assert_eq!(pure.stats().pure_calls, 1);
            match mode {
                RuntimePureBackendMode::Aot => assert_eq!(pure.stats().aot_calls, 1),
                RuntimePureBackendMode::Jit => assert_eq!(pure.stats().jit_calls, 1),
                _ => unreachable!(),
            }
            assert_eq!(pure.stats().arg_vec_allocations, 0);
            assert_eq!(pure.stats().arg_bytes_copied, 0);
            assert_eq!(
                pure.stats().arg_bytes_borrowed,
                24,
                "the unused whole formal remains in the physical row"
            );
        }
    }

    #[test]
    fn successful_scoped_branch_completion_charges_generated_scopes_and_discards_return_suffixes() {
        let selected = source_program(
            r#"
fn score(base: i64, bonus: i64, unused: i64) -> i64 effects {} {
    scope selected_branch {
        if base > 0i64 {
            let boosted = bonus + 2i64
            return base * boosted
        } else {
            let boosted = bonus + 2i64
            return -base * boosted
        }
    }
    return 0i64
}
pub fn root(value: i64, unused: i64) -> i64 effects {} {
    let result = score(value, 4i64, unused)
    return result
}
flow main() -> String { return "ok" }
"#,
        );
        // Inspect the original admitted rows, whose emitted lexical scope is
        // distinct from the Engine's generated branch frame. Namespace text
        // never selects a runtime exit.
        assert!(
            selected
                .plan()
                .function_sites()
                .iter_with_ids()
                .any(|(_, site)| {
                    if site.role() != arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary
                        || site.inputs().len() != 3
                    {
                        return false;
                    }
                    let Some(result) = selected.plan().type_table().get(site.result()) else {
                        return false;
                    };
                    if !matches!(
                        result.projection(),
                        arcweft_core::plan::RuntimePlanTypeProjection::Signed(
                            arcweft_core::value::RuntimeSignedIntWidth::I64
                        )
                    ) {
                        return false;
                    }
                    let arcweft_core::plan::RuntimeFunctionSiteBody::Executable(body) = site.body()
                    else {
                        return false;
                    };
                    let [
                        FlowOp::EnterScope { .. },
                        FlowOp::If {
                            then_ops, else_ops, ..
                        },
                        FlowOp::ExitScope,
                    ] = body.ops()
                    else {
                        return false;
                    };
                    [then_ops, else_ops].into_iter().all(|ops| {
                        matches!(ops.as_slice(), [FlowOp::Let { .. }, FlowOp::ReturnExpr(_)])
                    })
                }),
            "the admitted three-formal numeric body retains emitted scope entry, both early-return arms, and its discarded lexical exit"
        );
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..Default::default()
        };
        for input in [3, -3] {
            let (expected, expected_ops) = finish(
                &mut native(&selected, input),
                &mut VmRuntimePureCallBackend::default(),
                options,
            );
            assert_eq!(
                expected,
                RuntimeValue::i64(18),
                "the trailing return must be discarded"
            );
            for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
                let mut backend = RuntimePureAccelerator::new(mode, selected.plan());
                let (value, ops) = finish(&mut native(&selected, input), &mut backend, options);
                assert_eq!(value, expected);
                assert_eq!(
                    ops, expected_ops,
                    "generated scope entry/exit and early-return discard have the exact VM cost"
                );
                assert_eq!(backend.stats().pure_calls, 1);
                assert_eq!(backend.stats().vm_calls, 0);
                match mode {
                    RuntimePureBackendMode::Aot => assert_eq!(backend.stats().aot_calls, 1),
                    RuntimePureBackendMode::Jit => assert_eq!(backend.stats().jit_calls, 1),
                    _ => unreachable!(),
                }
                assert_eq!(backend.stats().arg_vec_allocations, 0);
                assert_eq!(backend.stats().arg_bytes_copied, 0);
                assert_eq!(backend.stats().arg_bytes_borrowed, 24);
            }
        }
    }
    #[test]
    fn successful_fallthrough_scopes_charge_generated_exit_operations() {
        let selected = source_program(
            r#"
fn score(base: i64, bonus: i64, unused: i64) -> i64 effects {} {
    scope numeric_prefix {
        let boosted = bonus + 2i64
        if base > 0i64 {
            let branch_value = base + boosted
        } else {
            let branch_value = -base + boosted
        }
    }
    return base * bonus
}
pub fn root(value: i64, unused: i64) -> i64 effects {} {
    let result = score(value, 4i64, unused)
    return result
}
flow main() -> String { return "ok" }
"#,
        );
        fn unit_continuation(plan: &arcweft_core::plan::RuntimePlan, ops: &[FlowOp]) -> bool {
            let [
                FlowOp::Let { .. },
                FlowOp::ExitScopeBind { pattern, expr },
                FlowOp::Let {
                    pattern: discard,
                    expr: moved,
                },
                FlowOp::ReturnExpr(result),
            ] = ops
            else {
                return false;
            };
            let Some(unit) = plan.type_table().get(expr.ty()) else {
                return false;
            };
            if !matches!(
                unit.projection(),
                arcweft_core::plan::RuntimePlanTypeProjection::Unit
            ) || !matches!(
                expr.kind(),
                arcweft_core::value::RuntimeExprKind::Value(RuntimeValue::Unit)
            ) || pattern.ty() != expr.ty()
                || discard.ty() != expr.ty()
                || moved.ty() != expr.ty()
                || result.ty() == expr.ty()
            {
                return false;
            }
            let arcweft_core::pattern::RuntimePatternKind::Bind { binding, .. } = pattern.kind()
            else {
                return false;
            };
            let arcweft_core::value::RuntimeExprKind::Local(read) = moved.kind() else {
                return false;
            };
            matches!(
                discard.kind(),
                arcweft_core::pattern::RuntimePatternKind::Discard
            ) && read.local() == binding.local()
                && read.fields().is_empty()
                && read.mode() == arcweft_core::value::RuntimeLocalReadMode::Move
        }
        assert!(
            selected
                .plan()
                .function_sites()
                .iter_with_ids()
                .any(|(_, site)| {
                    if site.role() != arcweft_core::plan::RuntimeFunctionSemanticRole::Ordinary
                        || site.inputs().len() != 3
                    {
                        return false;
                    }
                    let Some(result) = selected.plan().type_table().get(site.result()) else {
                        return false;
                    };
                    if !matches!(
                        result.projection(),
                        arcweft_core::plan::RuntimePlanTypeProjection::Signed(
                            arcweft_core::value::RuntimeSignedIntWidth::I64
                        )
                    ) {
                        return false;
                    }
                    let arcweft_core::plan::RuntimeFunctionSiteBody::Executable(body) = site.body()
                    else {
                        return false;
                    };
                    let [
                        FlowOp::EnterScope { .. },
                        FlowOp::Let { .. },
                        FlowOp::If {
                            then_ops, else_ops, ..
                        },
                    ] = body.ops()
                    else {
                        return false;
                    };
                    unit_continuation(selected.plan(), then_ops)
                        && unit_continuation(selected.plan(), else_ops)
                }),
            "both admitted branches exit the emitted scope, transfer and consume its typed Unit result, then reach the outer numeric return"
        );
        let options = RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 64 },
            ..Default::default()
        };
        for input in [3, -3] {
            let (expected, expected_ops) = finish(
                &mut native(&selected, input),
                &mut VmRuntimePureCallBackend::default(),
                options,
            );
            assert_eq!(expected, RuntimeValue::i64(input * 4));
            for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
                let mut backend = RuntimePureAccelerator::new(mode, selected.plan());
                let (value, ops) = finish(&mut native(&selected, input), &mut backend, options);
                assert_eq!(value, expected);
                assert_eq!(
                    ops, expected_ops,
                    "every generated branch and outer scope Exit is charged"
                );
                assert_eq!(backend.stats().pure_calls, 1);
                assert_eq!(backend.stats().vm_calls, 0);
                match mode {
                    RuntimePureBackendMode::Aot => assert_eq!(backend.stats().aot_calls, 1),
                    RuntimePureBackendMode::Jit => assert_eq!(backend.stats().jit_calls, 1),
                    _ => unreachable!(),
                }
                assert_eq!(backend.stats().arg_vec_allocations, 0);
                assert_eq!(backend.stats().arg_bytes_copied, 0);
                assert_eq!(backend.stats().arg_bytes_borrowed, 24);
            }
        }
    }
    #[test]
    fn physical_source_body_declines_when_its_exact_remaining_budget_does_not_fit() {
        let selected = source_program(SOURCE);
        let mut probe = native(&selected, 3);
        let mut vm = VmRuntimePureCallBackend::default();
        until_call(&mut probe, &mut vm);
        let Some(FlowOp::ProjectCall { site }) = probe.active_flow_operations().next() else {
            panic!("accepted caller dispatch")
        };
        let result_local = selected
            .plan()
            .project_call_sites()
            .get(*site)
            .unwrap()
            .result()
            .binding_declarations()
            .next()
            .unwrap()
            .local();
        assert!(probe.fiber().frame.env.get(result_local).is_none());
        let mut call_span = 0_usize;
        for _ in 0..64 {
            let step = probe.step_with_pure_backend(
                Default::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::OneOp,
                    budget: RuntimeStepBudget { max_ops: 64 },
                    ..Default::default()
                },
                &mut vm,
            );
            assert!(step.output.diagnostics.is_empty(), "{step:?}");
            call_span += step.stats.executed_ops;
            if probe.fiber().frame.env.get(result_local).is_some() {
                break;
            }
        }
        assert!(
            probe.fiber().frame.env.get(result_local).is_some(),
            "the original call must publish its owned result before the probe stops"
        );
        assert!(
            call_span > 1,
            "the target is a real executable body, beyond its caller dispatch"
        );
        assert!(probe.take_program_result().unwrap().is_none());
        for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
            let mut engine = native(&selected, 3);
            let mut pure = RuntimePureAccelerator::new(mode, selected.plan());
            until_call(&mut engine, &mut pure);
            let step = engine.step_with_pure_backend(
                Default::default(),
                RuntimeStepOptions {
                    mode: RuntimeStepMode::Drain,
                    budget: RuntimeStepBudget {
                        max_ops: call_span - 1,
                    },
                    ..Default::default()
                },
                &mut pure,
            );
            assert_eq!(step.stats.executed_ops, call_span - 1);
            assert_eq!(step.stop_reason, RuntimeStepStopReason::BudgetExhausted);
            assert!(matches!(step.fiber_status, FlowFiberStatus::Running));
            assert!(engine.take_program_result().unwrap().is_none());
            assert_eq!(pure.stats().jit_calls + pure.stats().aot_calls, 0);
            let (value, _) = finish(
                &mut engine,
                &mut pure,
                RuntimeStepOptions {
                    mode: RuntimeStepMode::Drain,
                    budget: RuntimeStepBudget { max_ops: 64 },
                    ..Default::default()
                },
            );
            assert_eq!(value, RuntimeValue::i64(18));
            assert_eq!(pure.stats().jit_calls + pure.stats().aot_calls, 0);
        }
    }

    #[test]
    fn one_op_with_a_large_budget_keeps_native_and_awbc_source_continuations() {
        let selected = source_program(SOURCE);
        for options in [
            RuntimeStepOptions {
                mode: RuntimeStepMode::OneOp,
                budget: RuntimeStepBudget { max_ops: 64 },
                ..Default::default()
            },
            RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 1 },
                ..Default::default()
            },
        ] {
            for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
                let mut pure = RuntimePureAccelerator::new(mode, selected.plan());
                let (value, ops) = finish(&mut native(&selected, 3), &mut pure, options);
                assert_eq!(value, RuntimeValue::i64(18));
                assert!(ops > 1);
                assert_eq!(pure.stats().aot_calls + pure.stats().jit_calls, 0);
            }
            let mut plan = selected.plan().as_ref().clone();
            plan.bind_artifact(
                arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([83; 32]).unwrap(),
            )
            .unwrap();
            let product = Arc::new(
                arcweft_runtime_plan::awbc_lower::AwbcLowerer::new(
                    &plan,
                    &selected.lowering_report().dialogue_content_catalog,
                    "scalar-budget.arcw",
                )
                .lower()
                .unwrap()
                .program,
            );
            let mut executor =
                arcweft_core::awbc::product_step::AwbcProductStepExecutor::for_program_invocation(
                    product,
                    selected.program(),
                    vec![RuntimeValue::i64(3), RuntimeValue::i64(77)],
                    arcweft_core::task::GenerationId::new(0),
                    1,
                )
                .unwrap();
            let mut backend = VmRuntimePureCallBackend::default();
            let mut result = None;
            for _ in 0..256 {
                let step =
                    executor.step_with_pure_backend(Default::default(), options, &mut backend);
                assert!(step.output.diagnostics.is_empty(), "{step:?}");
                assert!(
                    step.stats.executed_ops <= 1,
                    "the product must retain the owning one-operation boundary"
                );
                if let Some((id, value)) = executor.take_program_result().unwrap() {
                    assert_eq!(id, selected.program());
                    result = Some(value);
                    break;
                }
            }
            assert_eq!(result, Some(RuntimeValue::i64(18)));
            assert!(executor.take_program_result().unwrap().is_none());
        }
    }

    #[test]
    fn unequal_early_return_costs_decline_physical_source_completion() {
        let selected = source_program(
            r#"
fn score(value: i64, unused: i64) -> i64 effects {} {
    if value > 0i64 { return value + 1i64 }
    return unused
}
pub fn root(value: i64, unused: i64) -> i64 effects {} {
    let result = score(value, unused)
    return result
}
flow main() -> String { return "ok" }
"#,
        );
        for (input, expected) in [(3, 4), (-1, 77)] {
            for mode in [RuntimePureBackendMode::Aot, RuntimePureBackendMode::Jit] {
                let mut pure = RuntimePureAccelerator::new(mode, selected.plan());
                let (value, _) = finish(
                    &mut native(&selected, input),
                    &mut pure,
                    RuntimeStepOptions {
                        mode: RuntimeStepMode::Drain,
                        budget: RuntimeStepBudget { max_ops: 64 },
                        ..Default::default()
                    },
                );
                assert_eq!(value, RuntimeValue::i64(expected));
                assert_eq!(pure.stats().aot_calls + pure.stats().jit_calls, 0);
                assert_eq!(
                    pure.stats().vm_calls,
                    1,
                    "an ambiguous physical control cost retains its actual interpreted call"
                );
            }
        }
    }
    #[test]
    fn pending_joined_source_work_keeps_the_original_scalar_return_continuation() {
        let path = super::temp_arcw(
            "scalar-pending-joined-owner",
            r#"
fn score(base: i64, bonus: i64, unused: i64) -> i64 effects {} {
    let boosted = bonus + 2i64
    return base * boosted
}
entry cli @entry.cli.scalar_join { goto @flow.root }
flow root() -> String effects { control.spawn } {
    let linear_prefix = 1i64
    thread first {
        let value0 = 0i64
        let value1 = value0 + 1i64
        let value2 = value1 + 1i64
        let value3 = value2 + 1i64
        let value4 = value3 + 1i64
        let value5 = value4 + 1i64
        let value6 = value5 + 1i64
        let value7 = value6 + 1i64
        let value8 = value7 + 1i64
        let value9 = value8 + 1i64
        let value10 = value9 + 1i64
        let value11 = value10 + 1i64
    }
    goto @flow.linear
}
flow linear() -> String effects {} {
    let scored = score(3i64, 4i64, 77i64)
    if scored != 18i64 { return "wrong" }
    return "done"
}
"#,
        );
        for executor in ["bytecode-vm", "aot"] {
            let probe = std::process::Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.cli.scalar_join",
                    "--mode",
                    "one-op",
                    "--max-ops",
                    "64",
                    "--steps",
                    "128",
                    "--executor",
                    executor,
                    "--json",
                ])
                .output()
                .expect("bounded source probe observes the real joined child");
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            let observed: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
            assert!(
                observed["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|step| step["stats"]["child_fibers"].as_u64().unwrap() > 0),
                "the actual source must publish and schedule its joined child: {observed}"
            );
            let mut returned = false;
            for step in observed["steps"].as_array().unwrap() {
                if step["flow_events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event == "return done")
                {
                    assert_eq!(
                        step["stats"]["child_fibers"], 0,
                        "the actual parent Return event must wait for all joined children: {observed}"
                    );
                    returned = true;
                }
            }
            assert!(
                returned,
                "the bounded OneOp probe must observe the actual parent return event: {observed}"
            );
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.cli.scalar_join",
                    "--mode",
                    "drain",
                    "--max-ops",
                    "64",
                    "--steps",
                    "8",
                    "--pure-backend",
                    "jit",
                    "--executor",
                    executor,
                    "--json",
                ])
                .output()
                .expect("the authored joined-work/goto fixture runs");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["final_status"], "done Return(\"done\")");
            if executor == "aot" {
                assert_eq!(json["executor"], "aot");
                assert!(
                    json["executor_stats"]["aot_fast_path_ops"]
                        .as_u64()
                        .unwrap()
                        > 0,
                    "the linear prefix must actually enter the AOT owner before the scheduler fallback: {json}"
                );
            }
            assert_eq!(super::sum_step_pure_counter(&json, "jit_calls"), 0);
            assert_eq!(super::sum_step_pure_counter(&json, "aot_calls"), 0);
            assert_eq!(super::sum_step_pure_counter(&json, "vm_calls"), 1);
            assert_eq!(super::sum_step_pure_counter(&json, "pure_calls"), 1);
        }
    }

    #[test]
    fn source_goto_linear_return_drains_joined_children_before_native_or_aot_completion() {
        let path = super::temp_arcw(
            "scalar-linear-join-owner",
            r#"
entry cli @entry.cli.linear_join { goto @flow.root }
flow root() -> String effects { control.spawn } {
    let linear_prefix = 1i64
    thread first {
        let value0 = 0i64
        let value1 = value0 + 1i64
        let value2 = value1 + 1i64
        let value3 = value2 + 1i64
        let value4 = value3 + 1i64
        let value5 = value4 + 1i64
        let value6 = value5 + 1i64
        let value7 = value6 + 1i64
        let value8 = value7 + 1i64
        let value9 = value8 + 1i64
        let value10 = value9 + 1i64
        let value11 = value10 + 1i64
    }
    goto @flow.linear
}
flow linear() -> String effects {} { return "done" }
"#,
        );
        for executor in ["bytecode-vm", "aot"] {
            let probe = std::process::Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.cli.linear_join",
                    "--mode",
                    "one-op",
                    "--max-ops",
                    "64",
                    "--steps",
                    "128",
                    "--executor",
                    executor,
                    "--json",
                ])
                .output()
                .expect("bounded source probe observes the real joined child");
            assert!(
                probe.status.success(),
                "{}",
                String::from_utf8_lossy(&probe.stderr)
            );
            let observed: serde_json::Value = serde_json::from_slice(&probe.stdout).unwrap();
            assert!(
                observed["steps"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|step| step["stats"]["child_fibers"].as_u64().unwrap() > 0),
                "the actual source must publish and schedule its joined child: {observed}"
            );
            let mut returned = false;
            for step in observed["steps"].as_array().unwrap() {
                if step["flow_events"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|event| event == "return done")
                {
                    assert_eq!(
                        step["stats"]["child_fibers"], 0,
                        "the actual parent Return event must wait for all joined children: {observed}"
                    );
                    returned = true;
                }
            }
            assert!(
                returned,
                "the bounded OneOp probe must observe the actual parent return event: {observed}"
            );
            let output = std::process::Command::new(env!("CARGO_BIN_EXE_arcw"))
                .arg("run")
                .arg(&path)
                .args([
                    "--entry",
                    "entry.cli.linear_join",
                    "--mode",
                    "drain",
                    "--max-ops",
                    "64",
                    "--steps",
                    "8",
                    "--executor",
                    executor,
                    "--json",
                ])
                .output()
                .expect("the authored linear return/join fixture runs");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(json["final_status"], "done Return(\"done\")");
            if executor == "aot" {
                assert_eq!(json["executor"], "aot");
                assert!(
                    json["executor_stats"]["aot_fast_path_ops"]
                        .as_u64()
                        .unwrap()
                        > 0,
                    "the linear prefix must actually enter the AOT owner before the scheduler fallback: {json}"
                );
            }
            let steps = json["steps"].as_array().unwrap();
            assert!(!steps.is_empty());
            assert_eq!(
                steps.last().unwrap()["stats"]["child_fibers"],
                0,
                "parent completion must retain all joined child turns and closure: {json}"
            );
        }
    }
}
