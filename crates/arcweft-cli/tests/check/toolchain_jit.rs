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

