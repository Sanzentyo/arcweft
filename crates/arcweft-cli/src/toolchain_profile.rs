use arcweft_runtime_host::{HostSystemInfo, host_system_info};
use clap::{Args, ValueEnum};
use serde::Serialize;
use std::collections::BTreeMap;
use std::process::{Command, ExitCode};
use std::time::Instant;

#[derive(Args, Clone, Debug)]
pub(crate) struct ToolchainProfileOptions {
    #[arg(long = "command", value_enum)]
    pub(crate) commands: Vec<ToolchainProfileCommand>,
    #[arg(long, default_value_t = 1, value_parser = parse_positive_usize)]
    pub(crate) repeat: usize,
    #[arg(long, default_value_t = 0)]
    pub(crate) warmup: usize,
    #[arg(long)]
    pub(crate) dry_run: bool,
    #[arg(long)]
    pub(crate) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ToolchainProfileCommand {
    Fmt,
    Check,
    CheckFull,
    TestBuild,
    Clippy,
    Test,
    #[value(name = "bench-003")]
    Bench003,
    #[value(name = "bench-009")]
    Bench009,
    #[value(name = "bench-009-aot-object")]
    Bench009AotObject,
    #[value(name = "bench-033-width-jit")]
    Bench033WidthJit,
    #[value(name = "bench-033-width-aot")]
    Bench033WidthAot,
    #[value(name = "bench-033-width-vm")]
    Bench033WidthVm,
    #[value(name = "bench-040-width-jit")]
    Bench040WidthJit,
    #[value(name = "bench-040-width-aot")]
    Bench040WidthAot,
    #[value(name = "bench-040-width-vm")]
    Bench040WidthVm,
    #[value(name = "bench-033-width-aot-object")]
    Bench033WidthAotObject,
    #[value(name = "bench-040-width-aot-object")]
    Bench040WidthAotObject,
    #[value(name = "flow-math-matmul-glam")]
    FlowMathMatmulGlam,
    #[value(name = "flow-math-matrix-add-ndarray")]
    FlowMathMatrixAddNdarray,
    #[value(name = "flow-math-tensor-add-ndarray")]
    FlowMathTensorAddNdarray,
    #[value(name = "flow-math-matmul-f64-ndarray")]
    FlowMathMatmulF64Ndarray,
    #[value(name = "flow-math-matrix-add-f64-ndarray")]
    FlowMathMatrixAddF64Ndarray,
    #[value(name = "flow-math-tensor-add-f64-ndarray")]
    FlowMathTensorAddF64Ndarray,
    #[value(name = "flow-math-matmul-auto-wgpu")]
    FlowMathMatmulAutoWgpu,
    #[value(name = "math-matmul-bias")]
    MathMatmulBias,
    #[value(name = "math-matrix-add")]
    MathMatrixAdd,
    #[value(name = "math-tensor-add")]
    MathTensorAdd,
    #[value(name = "math-matmul-f64")]
    MathMatmulF64,
    #[value(name = "math-matrix-add-f64")]
    MathMatrixAddF64,
    #[value(name = "math-tensor-add-f64")]
    MathTensorAddF64,
    #[value(name = "math-matmul-bias-wgpu-reuse")]
    MathMatmulBiasWgpuReuse,
    #[value(name = "math-matrix-add-wgpu-reuse")]
    MathMatrixAddWgpuReuse,
    #[value(name = "math-tensor-add-wgpu-reuse")]
    MathTensorAddWgpuReuse,
    #[value(name = "math-matmul-auto-wgpu")]
    MathMatmulAutoWgpu,
    #[value(name = "math-matmul-bias-auto-wgpu-reuse")]
    MathMatmulBiasAutoWgpuReuse,
    #[value(name = "math-matrix-add-auto-wgpu-reuse")]
    MathMatrixAddAutoWgpuReuse,
    #[value(name = "math-tensor-add-auto-wgpu-reuse")]
    MathTensorAddAutoWgpuReuse,
}

#[derive(Serialize)]
struct ToolchainProfileReport {
    status: String,
    host_system: HostSystemInfo,
    commands: Vec<ToolchainCommandReport>,
}

#[derive(Serialize)]
struct ToolchainCommandReport {
    label: &'static str,
    argv: Vec<&'static str>,
    status: &'static str,
    exit_code: Option<i32>,
    repeat: usize,
    warmup: usize,
    elapsed_ns: u128,
    timing: ToolchainTimingReport,
    stdout_lines: usize,
    stderr_lines: usize,
    arcweft_bench: Option<ToolchainArcweftBenchReport>,
    math_bench: Option<ToolchainMathBenchReport>,
    warmup_samples: Vec<ToolchainCommandSample>,
    samples: Vec<ToolchainCommandSample>,
}

#[derive(Serialize)]
struct ToolchainTimingReport {
    min: u128,
    median: u128,
    max: u128,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainCommandSample {
    index: usize,
    status: &'static str,
    exit_code: Option<i32>,
    elapsed_ns: u128,
    stdout_lines: usize,
    stderr_lines: usize,
    arcweft_bench: Option<ToolchainArcweftBenchSample>,
    math_bench: Option<ToolchainMathBenchSample>,
}

#[derive(Clone, Copy, Debug)]
struct ToolchainCommandSpec {
    label: &'static str,
    args: &'static [&'static str],
    kind: ToolchainCommandKind,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ToolchainCommandKind {
    Cargo,
    ArcweftBench,
    MathBench,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainArcweftBenchSample {
    source: String,
    bench_id: String,
    bench_status: String,
    executor: String,
    bench_elapsed_ns: u64,
    per_executed_op_ns: u64,
    pure_calls: u64,
    pure_batch_calls: u64,
    pure_batch_items: u64,
    pure_jit_calls: u64,
    pure_aot_calls: u64,
    pure_vm_calls: u64,
    pure_fallbacks: u64,
    pure_arg_vec_allocations: u64,
    pure_arg_bytes_borrowed: u64,
    pure_compile_elapsed_ns: u64,
    pure_object_attempts: u64,
    pure_object_successes: u64,
    pure_object_failures: u64,
    pure_object_bytes: u64,
    math_calls: u64,
    math_accelerated_calls: u64,
    math_scalar_calls: u64,
    math_glam_calls: u64,
    math_ndarray_calls: u64,
    math_wgpu_calls: u64,
    math_fused_matmul_bias_add_calls: u64,
    math_fallback_calls: u64,
    math_bytes_borrowed: u64,
    math_bytes_copied: u64,
    math_bytes_uploaded: u64,
    math_bytes_downloaded: u64,
    math_gpu_buffer_reuse_hits: u64,
    math_gpu_reused_dispatches: u64,
    math_last_backend: Option<String>,
    math_last_auto_reason: Option<String>,
}

#[derive(Serialize)]
struct ToolchainArcweftBenchReport {
    source: String,
    bench_id: String,
    bench_status: String,
    executor: String,
    median_bench_elapsed_ns: u64,
    min_bench_elapsed_ns: u64,
    max_bench_elapsed_ns: u64,
    median_per_executed_op_ns: u64,
    median_pure_calls: u64,
    median_pure_batch_calls: u64,
    median_pure_batch_items: u64,
    median_pure_jit_calls: u64,
    median_pure_aot_calls: u64,
    median_pure_vm_calls: u64,
    median_pure_fallbacks: u64,
    median_pure_arg_vec_allocations: u64,
    median_pure_arg_bytes_borrowed: u64,
    median_pure_compile_elapsed_ns: u64,
    median_pure_object_attempts: u64,
    median_pure_object_successes: u64,
    median_pure_object_failures: u64,
    median_pure_object_bytes: u64,
    median_math_calls: u64,
    median_math_accelerated_calls: u64,
    median_math_scalar_calls: u64,
    median_math_glam_calls: u64,
    median_math_ndarray_calls: u64,
    median_math_wgpu_calls: u64,
    median_math_fused_matmul_bias_add_calls: u64,
    median_math_fallback_calls: u64,
    median_math_bytes_borrowed: u64,
    median_math_bytes_copied: u64,
    median_math_bytes_uploaded: u64,
    median_math_bytes_downloaded: u64,
    median_math_gpu_buffer_reuse_hits: u64,
    median_math_gpu_reused_dispatches: u64,
    math_last_backend: Option<String>,
    math_last_auto_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainMathBenchSample {
    op: String,
    size: u64,
    build_mode: String,
    #[serde(flatten)]
    reuse_options: ToolchainMathReuseOptions,
    submit_only: bool,
    results: Vec<ToolchainMathBackendSample>,
}

#[derive(Clone, Copy, Debug, Serialize)]
struct ToolchainMathReuseOptions {
    reuse: bool,
    reuse_update_inputs: bool,
    reuse_capacity: bool,
}

#[derive(Clone, Debug, Serialize)]
struct ToolchainMathBackendSample {
    backend: String,
    status: String,
    median_ns: Option<u64>,
    speedup_vs_scalar: Option<f64>,
    scalar_calls: Option<u64>,
    glam_calls: Option<u64>,
    ndarray_calls: Option<u64>,
    wgpu_calls: Option<u64>,
    fallback_calls: Option<u64>,
    bytes_borrowed: Option<u64>,
    bytes_copied: Option<u64>,
    bytes_uploaded: Option<u64>,
    bytes_downloaded: Option<u64>,
    gpu_buffer_reuse_hits: Option<u64>,
    gpu_reused_dispatches: Option<u64>,
    last_backend: Option<String>,
    last_auto_reason: Option<String>,
}

#[derive(Serialize)]
struct ToolchainMathBenchReport {
    op: String,
    size: u64,
    build_mode: String,
    #[serde(flatten)]
    reuse_options: ToolchainMathReuseOptions,
    submit_only: bool,
    results: Vec<ToolchainMathBackendReport>,
}

#[derive(Serialize)]
struct ToolchainMathBackendReport {
    backend: String,
    status: String,
    median_ns: Option<u64>,
    min_ns: Option<u64>,
    max_ns: Option<u64>,
    median_speedup_vs_scalar: Option<f64>,
    median_scalar_calls: Option<u64>,
    median_glam_calls: Option<u64>,
    median_ndarray_calls: Option<u64>,
    median_wgpu_calls: Option<u64>,
    median_fallback_calls: Option<u64>,
    median_bytes_borrowed: Option<u64>,
    median_bytes_copied: Option<u64>,
    median_bytes_uploaded: Option<u64>,
    median_bytes_downloaded: Option<u64>,
    median_gpu_buffer_reuse_hits: Option<u64>,
    median_gpu_reused_dispatches: Option<u64>,
    last_backend: Option<String>,
    last_auto_reason: Option<String>,
}

const CHECK: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace",
    args: &["check", "--workspace"],
    kind: ToolchainCommandKind::Cargo,
};

const CHECK_FULL: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_check_workspace_all_targets_all_features",
    args: &["check", "--workspace", "--all-targets", "--all-features"],
    kind: ToolchainCommandKind::Cargo,
};

const FMT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_fmt_all_check",
    args: &["fmt", "--all", "--check"],
    kind: ToolchainCommandKind::Cargo,
};

const CLIPPY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_clippy_workspace_all_targets_all_features",
    args: &["clippy", "--workspace", "--all-targets", "--all-features"],
    kind: ToolchainCommandKind::Cargo,
};

const TEST: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace",
    args: &["test", "--workspace"],
    kind: ToolchainCommandKind::Cargo,
};

const TEST_BUILD: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "cargo_test_workspace_no_run",
    args: &["test", "--workspace", "--no-run"],
    kind: ToolchainCommandKind::Cargo,
};

const BENCH_003: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_003_for_pure_jit",
    args: &[
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
        "jit",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_009: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_009_nonuniform_map_pure_batch",
    args: &[
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
        "64",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_009_AOT_OBJECT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_009_aot_object_artifacts",
    args: &[
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
        "--pure-object-artifacts",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_033_WIDTH_JIT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_033_mixed_width_jit",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
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
        "jit",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_033_WIDTH_AOT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_033_mixed_width_aot",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
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
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_033_WIDTH_VM: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_033_mixed_width_vm",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
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
        "vm",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_040_WIDTH_JIT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_040_mixed_width_jit",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
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
        "jit",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_040_WIDTH_AOT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_040_mixed_width_aot",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
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
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_040_WIDTH_VM: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_040_mixed_width_vm",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
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
        "vm",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_033_WIDTH_AOT_OBJECT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_033_mixed_width_aot_object",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/033_mixed_for_iter_pure_jit.arcw",
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
        "--pure-object-artifacts",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const BENCH_040_WIDTH_AOT_OBJECT: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_bench_040_mixed_width_aot_object",
    args: &[
        "run",
        "-p",
        "arcweft-cli",
        "--quiet",
        "--",
        "bench",
        "tests/fixtures/arcw/spec_should_pass/bench/040_mixed_width_for_iter_pure_jit.arcw",
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
        "--pure-object-artifacts",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_MATMUL_GLAM: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_matmul_glam",
    args: &[
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
        "rhs=matrix/f32/4x4:2,0,0,0,0,2,0,0,0,0,2,0,0,0,0,2",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_MATRIX_ADD_NDARRAY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_matrix_add_ndarray",
    args: &[
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
        "rhs=matrix/f32/4x4:16,15,14,13,12,11,10,9,8,7,6,5,4,3,2,1",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_TENSOR_ADD_NDARRAY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_tensor_add_ndarray",
    args: &[
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
        "rhs=tensor/f32/2x2x2:8,7,6,5,4,3,2,1",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_MATMUL_F64_NDARRAY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_matmul_f64_ndarray",
    args: &[
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
        "rhs=matrix/f64/2x2:5,6.5,7,8.25",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_MATRIX_ADD_F64_NDARRAY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_matrix_add_f64_ndarray",
    args: &[
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
        "rhs=matrix/f64/2x2:5,6.25,7.5,8.75",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_TENSOR_ADD_F64_NDARRAY: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_tensor_add_f64_ndarray",
    args: &[
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
        "rhs=tensor/f64/2x2:5,6.25,7.5,8.75",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const FLOW_MATH_MATMUL_AUTO_WGPU: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "arcw_flow_math_matmul_auto_wgpu",
    args: &[
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
        "rhs=matrix/f32/8x8:2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2,2",
    ],
    kind: ToolchainCommandKind::ArcweftBench,
};

const MATH_MATMUL_BIAS: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matmul_bias_add",
    args: &[
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
        "2",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATRIX_ADD: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matrix_add",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_TENSOR_ADD: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_tensor_add",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATMUL_F64: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matmul_f64",
    args: &[
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
        "2",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATRIX_ADD_F64: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matrix_add_f64",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_TENSOR_ADD_F64: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_tensor_add_f64",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATMUL_BIAS_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matmul_bias_wgpu_reuse",
    args: &[
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
        "--reuse",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATRIX_ADD_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matrix_add_wgpu_reuse",
    args: &[
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
        "--reuse",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_TENSOR_ADD_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_tensor_add_wgpu_reuse",
    args: &[
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
        "--reuse",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATMUL_AUTO_WGPU: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matmul_auto_wgpu",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATMUL_BIAS_AUTO_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matmul_bias_auto_wgpu_reuse",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_MATRIX_ADD_AUTO_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_matrix_add_auto_wgpu_reuse",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

const MATH_TENSOR_ADD_AUTO_WGPU_REUSE: ToolchainCommandSpec = ToolchainCommandSpec {
    label: "math_bench_tensor_add_auto_wgpu_reuse",
    args: &[
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
        "1",
    ],
    kind: ToolchainCommandKind::MathBench,
};

pub(crate) fn run(options: &ToolchainProfileOptions) -> Result<(), ExitCode> {
    let reports = selected_commands(options)
        .into_iter()
        .map(|spec| profile_command(spec, options.dry_run, options.repeat, options.warmup))
        .collect::<Vec<_>>();
    let failed = reports
        .iter()
        .any(|report| matches!(report.status, "failed" | "spawn_failed"));
    let report = ToolchainProfileReport {
        status: if failed { "failed" } else { "ok" }.to_owned(),
        host_system: host_system_info(),
        commands: reports,
    };

    if options.json {
        crate::print_json(&report)?;
    } else {
        print_human_report(&report);
    }

    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn selected_commands(options: &ToolchainProfileOptions) -> Vec<ToolchainCommandSpec> {
    if options.commands.is_empty() {
        return vec![CHECK];
    }
    options
        .commands
        .iter()
        .copied()
        .map(ToolchainCommandSpec::from)
        .collect()
}

impl From<ToolchainProfileCommand> for ToolchainCommandSpec {
    fn from(command: ToolchainProfileCommand) -> Self {
        match command {
            ToolchainProfileCommand::Fmt => FMT,
            ToolchainProfileCommand::Check => CHECK,
            ToolchainProfileCommand::CheckFull => CHECK_FULL,
            ToolchainProfileCommand::TestBuild => TEST_BUILD,
            ToolchainProfileCommand::Clippy => CLIPPY,
            ToolchainProfileCommand::Test => TEST,
            ToolchainProfileCommand::Bench003 => BENCH_003,
            ToolchainProfileCommand::Bench009 => BENCH_009,
            ToolchainProfileCommand::Bench009AotObject => BENCH_009_AOT_OBJECT,
            ToolchainProfileCommand::Bench033WidthJit => BENCH_033_WIDTH_JIT,
            ToolchainProfileCommand::Bench033WidthAot => BENCH_033_WIDTH_AOT,
            ToolchainProfileCommand::Bench033WidthVm => BENCH_033_WIDTH_VM,
            ToolchainProfileCommand::Bench040WidthJit => BENCH_040_WIDTH_JIT,
            ToolchainProfileCommand::Bench040WidthAot => BENCH_040_WIDTH_AOT,
            ToolchainProfileCommand::Bench040WidthVm => BENCH_040_WIDTH_VM,
            ToolchainProfileCommand::Bench033WidthAotObject => BENCH_033_WIDTH_AOT_OBJECT,
            ToolchainProfileCommand::Bench040WidthAotObject => BENCH_040_WIDTH_AOT_OBJECT,
            ToolchainProfileCommand::FlowMathMatmulGlam => FLOW_MATH_MATMUL_GLAM,
            ToolchainProfileCommand::FlowMathMatrixAddNdarray => FLOW_MATH_MATRIX_ADD_NDARRAY,
            ToolchainProfileCommand::FlowMathTensorAddNdarray => FLOW_MATH_TENSOR_ADD_NDARRAY,
            ToolchainProfileCommand::FlowMathMatmulF64Ndarray => FLOW_MATH_MATMUL_F64_NDARRAY,
            ToolchainProfileCommand::FlowMathMatrixAddF64Ndarray => {
                FLOW_MATH_MATRIX_ADD_F64_NDARRAY
            }
            ToolchainProfileCommand::FlowMathTensorAddF64Ndarray => {
                FLOW_MATH_TENSOR_ADD_F64_NDARRAY
            }
            ToolchainProfileCommand::FlowMathMatmulAutoWgpu => FLOW_MATH_MATMUL_AUTO_WGPU,
            ToolchainProfileCommand::MathMatmulBias => MATH_MATMUL_BIAS,
            ToolchainProfileCommand::MathMatrixAdd => MATH_MATRIX_ADD,
            ToolchainProfileCommand::MathTensorAdd => MATH_TENSOR_ADD,
            ToolchainProfileCommand::MathMatmulF64 => MATH_MATMUL_F64,
            ToolchainProfileCommand::MathMatrixAddF64 => MATH_MATRIX_ADD_F64,
            ToolchainProfileCommand::MathTensorAddF64 => MATH_TENSOR_ADD_F64,
            ToolchainProfileCommand::MathMatmulBiasWgpuReuse => MATH_MATMUL_BIAS_WGPU_REUSE,
            ToolchainProfileCommand::MathMatrixAddWgpuReuse => MATH_MATRIX_ADD_WGPU_REUSE,
            ToolchainProfileCommand::MathTensorAddWgpuReuse => MATH_TENSOR_ADD_WGPU_REUSE,
            ToolchainProfileCommand::MathMatmulAutoWgpu => MATH_MATMUL_AUTO_WGPU,
            ToolchainProfileCommand::MathMatmulBiasAutoWgpuReuse => {
                MATH_MATMUL_BIAS_AUTO_WGPU_REUSE
            }
            ToolchainProfileCommand::MathMatrixAddAutoWgpuReuse => MATH_MATRIX_ADD_AUTO_WGPU_REUSE,
            ToolchainProfileCommand::MathTensorAddAutoWgpuReuse => MATH_TENSOR_ADD_AUTO_WGPU_REUSE,
        }
    }
}

fn profile_command(
    spec: ToolchainCommandSpec,
    dry_run: bool,
    repeat: usize,
    warmup: usize,
) -> ToolchainCommandReport {
    if dry_run {
        let mut warmup_samples = Vec::with_capacity(warmup);
        for index in 0..warmup {
            warmup_samples.push(planned_command_sample(index));
        }
        let mut samples = Vec::with_capacity(repeat);
        for index in 0..repeat {
            samples.push(planned_command_sample(index));
        }
        return command_report_from_samples(spec, warmup_samples, samples);
    }

    let mut warmup_samples = Vec::with_capacity(warmup);
    for index in 0..warmup {
        warmup_samples.push(profile_command_sample(spec, index));
    }
    let mut samples = Vec::with_capacity(repeat);
    for index in 0..repeat {
        samples.push(profile_command_sample(spec, index));
    }
    command_report_from_samples(spec, warmup_samples, samples)
}

const fn planned_command_sample(index: usize) -> ToolchainCommandSample {
    ToolchainCommandSample {
        index,
        status: "planned",
        exit_code: None,
        elapsed_ns: 0,
        stdout_lines: 0,
        stderr_lines: 0,
        arcweft_bench: None,
        math_bench: None,
    }
}

fn profile_command_sample(spec: ToolchainCommandSpec, index: usize) -> ToolchainCommandSample {
    let start = Instant::now();
    match Command::new("cargo").args(spec.args).output() {
        Ok(output) => ToolchainCommandSample {
            index,
            status: if output.status.success() {
                "ok"
            } else {
                "failed"
            },
            exit_code: output.status.code(),
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: count_lines(&output.stdout),
            stderr_lines: count_lines(&output.stderr),
            arcweft_bench: if output.status.success()
                && spec.kind == ToolchainCommandKind::ArcweftBench
            {
                arcweft_bench_sample(&output.stdout)
            } else {
                None
            },
            math_bench: if output.status.success() && spec.kind == ToolchainCommandKind::MathBench {
                math_bench_sample(&output.stdout)
            } else {
                None
            },
        },
        Err(_) => ToolchainCommandSample {
            index,
            status: "spawn_failed",
            exit_code: None,
            elapsed_ns: start.elapsed().as_nanos(),
            stdout_lines: 0,
            stderr_lines: 0,
            arcweft_bench: None,
            math_bench: None,
        },
    }
}

fn command_report_from_samples(
    spec: ToolchainCommandSpec,
    warmup_samples: Vec<ToolchainCommandSample>,
    samples: Vec<ToolchainCommandSample>,
) -> ToolchainCommandReport {
    let status = aggregate_status(&warmup_samples, &samples);
    let exit_code = warmup_samples
        .iter()
        .chain(samples.iter())
        .find(|sample| sample.exit_code.is_some_and(|code| code != 0))
        .and_then(|sample| sample.exit_code)
        .or_else(|| {
            warmup_samples
                .iter()
                .chain(samples.iter())
                .find_map(|sample| sample.exit_code)
        });
    let stdout_lines = warmup_samples
        .iter()
        .chain(samples.iter())
        .map(|sample| sample.stdout_lines)
        .sum();
    let stderr_lines = warmup_samples
        .iter()
        .chain(samples.iter())
        .map(|sample| sample.stderr_lines)
        .sum();
    let mut elapsed = samples
        .iter()
        .map(|sample| sample.elapsed_ns)
        .collect::<Vec<_>>();
    let timing = timing_report(&mut elapsed);
    let arcweft_bench = arcweft_bench_report(&samples);
    let math_bench = math_bench_report(&samples);

    ToolchainCommandReport {
        label: spec.label,
        argv: argv_for(spec),
        status,
        exit_code,
        repeat: samples.len(),
        warmup: warmup_samples.len(),
        elapsed_ns: timing.median,
        timing,
        stdout_lines,
        stderr_lines,
        arcweft_bench,
        math_bench,
        warmup_samples,
        samples,
    }
}

fn arcweft_bench_sample(bytes: &[u8]) -> Option<ToolchainArcweftBenchSample> {
    let json = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    let section = json
        .get("benches")?
        .as_array()?
        .first()?
        .get("sections")?
        .as_array()?
        .first()?;
    let measurement = section.get("measurement")?;
    let deterministic = measurement.get("deterministic")?;
    let executor_stats = measurement.get("executor_stats")?;
    let pure_compile = executor_stats.get("pure_compile")?;
    let math = executor_stats.get("math")?;
    Some(ToolchainArcweftBenchSample {
        source: json.get("source")?.as_str()?.to_owned(),
        bench_id: json
            .get("benches")?
            .as_array()?
            .first()?
            .get("id")?
            .as_str()?
            .to_owned(),
        bench_status: section.get("status")?.as_str()?.to_owned(),
        executor: measurement.get("executor")?.as_str()?.to_owned(),
        bench_elapsed_ns: measurement.get("elapsed_ns")?.get("median")?.as_u64()?,
        per_executed_op_ns: measurement.get("per_executed_op_ns")?.as_u64()?,
        pure_calls: deterministic.get("pure_calls_median")?.as_u64()?,
        pure_batch_calls: deterministic.get("pure_batch_calls_median")?.as_u64()?,
        pure_batch_items: deterministic.get("pure_batch_items_median")?.as_u64()?,
        pure_jit_calls: deterministic.get("pure_jit_calls_median")?.as_u64()?,
        pure_aot_calls: deterministic.get("pure_aot_calls_median")?.as_u64()?,
        pure_vm_calls: deterministic.get("pure_vm_calls_median")?.as_u64()?,
        pure_fallbacks: deterministic.get("pure_fallbacks_median")?.as_u64()?,
        pure_arg_vec_allocations: deterministic
            .get("pure_arg_vec_allocations_median")?
            .as_u64()?,
        pure_arg_bytes_borrowed: deterministic
            .get("pure_arg_bytes_borrowed_median")?
            .as_u64()?,
        pure_compile_elapsed_ns: pure_compile.get("compile_elapsed_ns")?.as_u64()?,
        pure_object_attempts: pure_compile.get("object_attempts")?.as_u64()?,
        pure_object_successes: pure_compile.get("object_successes")?.as_u64()?,
        pure_object_failures: pure_compile.get("object_failures")?.as_u64()?,
        pure_object_bytes: pure_compile.get("object_bytes")?.as_u64()?,
        math_calls: deterministic.get("math_calls_median")?.as_u64()?,
        math_accelerated_calls: deterministic
            .get("math_accelerated_calls_median")?
            .as_u64()?,
        math_scalar_calls: math.get("scalar_calls")?.as_u64()?,
        math_glam_calls: math.get("glam_calls")?.as_u64()?,
        math_ndarray_calls: math.get("ndarray_calls")?.as_u64()?,
        math_wgpu_calls: math.get("wgpu_calls")?.as_u64()?,
        math_fused_matmul_bias_add_calls: math.get("fused_matmul_bias_add_calls")?.as_u64()?,
        math_fallback_calls: math.get("fallback_calls")?.as_u64()?,
        math_bytes_borrowed: math.get("bytes_borrowed")?.as_u64()?,
        math_bytes_copied: math.get("bytes_copied")?.as_u64()?,
        math_bytes_uploaded: math.get("bytes_uploaded")?.as_u64()?,
        math_bytes_downloaded: math.get("bytes_downloaded")?.as_u64()?,
        math_gpu_buffer_reuse_hits: math.get("gpu_buffer_reuse_hits")?.as_u64()?,
        math_gpu_reused_dispatches: math.get("gpu_reused_dispatches")?.as_u64()?,
        math_last_backend: math
            .get("last_backend")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        math_last_auto_reason: math
            .get("last_auto_reason")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

struct ArcweftBenchTimingSummary {
    bench_median: u64,
    bench_min: u64,
    bench_max: u64,
    per_op_median: u64,
}

struct ArcweftBenchPureSummary {
    calls: u64,
    batch_calls: u64,
    batch_items: u64,
    jit_calls: u64,
    aot_calls: u64,
    vm_calls: u64,
    fallbacks: u64,
    arg_vec_allocations: u64,
    arg_bytes_borrowed: u64,
    compile_elapsed_ns: u64,
    object_attempts: u64,
    object_successes: u64,
    object_failures: u64,
    object_bytes: u64,
}

struct ArcweftBenchMathSummary {
    calls: u64,
    accelerated_calls: u64,
    scalar_calls: u64,
    glam_calls: u64,
    ndarray_calls: u64,
    wgpu_calls: u64,
    fused_matmul_bias_add_calls: u64,
    fallback_calls: u64,
    bytes_borrowed: u64,
    bytes_copied: u64,
    bytes_uploaded: u64,
    bytes_downloaded: u64,
    gpu_buffer_reuse_hits: u64,
    gpu_reused_dispatches: u64,
    last_backend: Option<String>,
    last_auto_reason: Option<String>,
}

fn arcweft_bench_report(samples: &[ToolchainCommandSample]) -> Option<ToolchainArcweftBenchReport> {
    let bench_samples = samples
        .iter()
        .filter_map(|sample| sample.arcweft_bench.as_ref())
        .collect::<Vec<_>>();
    let first = bench_samples.first()?;
    let timing = arcweft_bench_timing_summary(&bench_samples);
    let pure = arcweft_bench_pure_summary(&bench_samples);
    let math = arcweft_bench_math_summary(&bench_samples);

    Some(ToolchainArcweftBenchReport {
        source: first.source.clone(),
        bench_id: first.bench_id.clone(),
        bench_status: first.bench_status.clone(),
        executor: first.executor.clone(),
        median_bench_elapsed_ns: timing.bench_median,
        min_bench_elapsed_ns: timing.bench_min,
        max_bench_elapsed_ns: timing.bench_max,
        median_per_executed_op_ns: timing.per_op_median,
        median_pure_calls: pure.calls,
        median_pure_batch_calls: pure.batch_calls,
        median_pure_batch_items: pure.batch_items,
        median_pure_jit_calls: pure.jit_calls,
        median_pure_aot_calls: pure.aot_calls,
        median_pure_vm_calls: pure.vm_calls,
        median_pure_fallbacks: pure.fallbacks,
        median_pure_arg_vec_allocations: pure.arg_vec_allocations,
        median_pure_arg_bytes_borrowed: pure.arg_bytes_borrowed,
        median_pure_compile_elapsed_ns: pure.compile_elapsed_ns,
        median_pure_object_attempts: pure.object_attempts,
        median_pure_object_successes: pure.object_successes,
        median_pure_object_failures: pure.object_failures,
        median_pure_object_bytes: pure.object_bytes,
        median_math_calls: math.calls,
        median_math_accelerated_calls: math.accelerated_calls,
        median_math_scalar_calls: math.scalar_calls,
        median_math_glam_calls: math.glam_calls,
        median_math_ndarray_calls: math.ndarray_calls,
        median_math_wgpu_calls: math.wgpu_calls,
        median_math_fused_matmul_bias_add_calls: math.fused_matmul_bias_add_calls,
        median_math_fallback_calls: math.fallback_calls,
        median_math_bytes_borrowed: math.bytes_borrowed,
        median_math_bytes_copied: math.bytes_copied,
        median_math_bytes_uploaded: math.bytes_uploaded,
        median_math_bytes_downloaded: math.bytes_downloaded,
        median_math_gpu_buffer_reuse_hits: math.gpu_buffer_reuse_hits,
        median_math_gpu_reused_dispatches: math.gpu_reused_dispatches,
        math_last_backend: math.last_backend,
        math_last_auto_reason: math.last_auto_reason,
    })
}

fn arcweft_bench_timing_summary(
    samples: &[&ToolchainArcweftBenchSample],
) -> ArcweftBenchTimingSummary {
    ArcweftBenchTimingSummary {
        bench_median: median_bench_sample_by(samples, |sample| sample.bench_elapsed_ns),
        bench_min: samples
            .iter()
            .map(|sample| sample.bench_elapsed_ns)
            .min()
            .unwrap_or_default(),
        bench_max: samples
            .iter()
            .map(|sample| sample.bench_elapsed_ns)
            .max()
            .unwrap_or_default(),
        per_op_median: median_bench_sample_by(samples, |sample| sample.per_executed_op_ns),
    }
}

fn arcweft_bench_pure_summary(samples: &[&ToolchainArcweftBenchSample]) -> ArcweftBenchPureSummary {
    ArcweftBenchPureSummary {
        calls: median_bench_sample_by(samples, |sample| sample.pure_calls),
        batch_calls: median_bench_sample_by(samples, |sample| sample.pure_batch_calls),
        batch_items: median_bench_sample_by(samples, |sample| sample.pure_batch_items),
        jit_calls: median_bench_sample_by(samples, |sample| sample.pure_jit_calls),
        aot_calls: median_bench_sample_by(samples, |sample| sample.pure_aot_calls),
        vm_calls: median_bench_sample_by(samples, |sample| sample.pure_vm_calls),
        fallbacks: median_bench_sample_by(samples, |sample| sample.pure_fallbacks),
        arg_vec_allocations: median_bench_sample_by(samples, |sample| {
            sample.pure_arg_vec_allocations
        }),
        arg_bytes_borrowed: median_bench_sample_by(samples, |sample| {
            sample.pure_arg_bytes_borrowed
        }),
        compile_elapsed_ns: median_bench_sample_by(samples, |sample| {
            sample.pure_compile_elapsed_ns
        }),
        object_attempts: median_bench_sample_by(samples, |sample| sample.pure_object_attempts),
        object_successes: median_bench_sample_by(samples, |sample| sample.pure_object_successes),
        object_failures: median_bench_sample_by(samples, |sample| sample.pure_object_failures),
        object_bytes: median_bench_sample_by(samples, |sample| sample.pure_object_bytes),
    }
}

fn arcweft_bench_math_summary(samples: &[&ToolchainArcweftBenchSample]) -> ArcweftBenchMathSummary {
    ArcweftBenchMathSummary {
        calls: median_bench_sample_by(samples, |sample| sample.math_calls),
        accelerated_calls: median_bench_sample_by(samples, |sample| sample.math_accelerated_calls),
        scalar_calls: median_bench_sample_by(samples, |sample| sample.math_scalar_calls),
        glam_calls: median_bench_sample_by(samples, |sample| sample.math_glam_calls),
        ndarray_calls: median_bench_sample_by(samples, |sample| sample.math_ndarray_calls),
        wgpu_calls: median_bench_sample_by(samples, |sample| sample.math_wgpu_calls),
        fused_matmul_bias_add_calls: median_bench_sample_by(samples, |sample| {
            sample.math_fused_matmul_bias_add_calls
        }),
        fallback_calls: median_bench_sample_by(samples, |sample| sample.math_fallback_calls),
        bytes_borrowed: median_bench_sample_by(samples, |sample| sample.math_bytes_borrowed),
        bytes_copied: median_bench_sample_by(samples, |sample| sample.math_bytes_copied),
        bytes_uploaded: median_bench_sample_by(samples, |sample| sample.math_bytes_uploaded),
        bytes_downloaded: median_bench_sample_by(samples, |sample| sample.math_bytes_downloaded),
        gpu_buffer_reuse_hits: median_bench_sample_by(samples, |sample| {
            sample.math_gpu_buffer_reuse_hits
        }),
        gpu_reused_dispatches: median_bench_sample_by(samples, |sample| {
            sample.math_gpu_reused_dispatches
        }),
        last_backend: modal_bench_sample_text_by(samples, |sample| {
            sample.math_last_backend.as_deref()
        }),
        last_auto_reason: modal_bench_sample_text_by(samples, |sample| {
            sample.math_last_auto_reason.as_deref()
        }),
    }
}

fn median_bench_sample_by(
    samples: &[&ToolchainArcweftBenchSample],
    field: impl Fn(&ToolchainArcweftBenchSample) -> u64,
) -> u64 {
    let mut values = samples
        .iter()
        .map(|sample| field(sample))
        .collect::<Vec<_>>();
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or_default()
}

fn modal_bench_sample_text_by(
    samples: &[&ToolchainArcweftBenchSample],
    field: impl Fn(&ToolchainArcweftBenchSample) -> Option<&str>,
) -> Option<String> {
    let mut counts = BTreeMap::new();
    for sample in samples {
        if let Some(value) = field(sample) {
            *counts.entry(value).or_insert(0usize) += 1;
        }
    }

    let mut best = None;
    for (value, count) in counts {
        match best {
            Some((_, best_count)) if count <= best_count => {}
            _ => best = Some((value, count)),
        }
    }
    best.map(|(value, _)| value.to_owned())
}

fn math_bench_sample(bytes: &[u8]) -> Option<ToolchainMathBenchSample> {
    let json = serde_json::from_slice::<serde_json::Value>(bytes).ok()?;
    Some(ToolchainMathBenchSample {
        op: json.get("op")?.as_str()?.to_owned(),
        size: json.get("size")?.as_u64()?,
        build_mode: json.get("build_mode")?.as_str()?.to_owned(),
        reuse_options: ToolchainMathReuseOptions {
            reuse: json.get("reuse")?.as_bool()?,
            reuse_update_inputs: json.get("reuse_update_inputs")?.as_bool()?,
            reuse_capacity: json.get("reuse_capacity")?.as_bool()?,
        },
        submit_only: json.get("submit_only")?.as_bool()?,
        results: json
            .get("results")?
            .as_array()?
            .iter()
            .filter_map(math_backend_sample)
            .collect(),
    })
}

fn math_backend_sample(json: &serde_json::Value) -> Option<ToolchainMathBackendSample> {
    let stats = json.get("stats");
    Some(ToolchainMathBackendSample {
        backend: json.get("backend")?.as_str()?.to_owned(),
        status: json.get("status")?.as_str()?.to_owned(),
        median_ns: json.get("median_ns").and_then(serde_json::Value::as_u64),
        speedup_vs_scalar: json
            .get("speedup_vs_scalar")
            .and_then(serde_json::Value::as_f64),
        scalar_calls: stats
            .and_then(|stats| stats.get("scalar_calls"))
            .and_then(serde_json::Value::as_u64),
        glam_calls: stats
            .and_then(|stats| stats.get("glam_calls"))
            .and_then(serde_json::Value::as_u64),
        ndarray_calls: stats
            .and_then(|stats| stats.get("ndarray_calls"))
            .and_then(serde_json::Value::as_u64),
        wgpu_calls: stats
            .and_then(|stats| stats.get("wgpu_calls"))
            .and_then(serde_json::Value::as_u64),
        fallback_calls: stats
            .and_then(|stats| stats.get("fallback_calls"))
            .and_then(serde_json::Value::as_u64),
        bytes_borrowed: stats
            .and_then(|stats| stats.get("bytes_borrowed"))
            .and_then(serde_json::Value::as_u64),
        bytes_copied: stats
            .and_then(|stats| stats.get("bytes_copied"))
            .and_then(serde_json::Value::as_u64),
        bytes_uploaded: stats
            .and_then(|stats| stats.get("bytes_uploaded"))
            .and_then(serde_json::Value::as_u64),
        bytes_downloaded: stats
            .and_then(|stats| stats.get("bytes_downloaded"))
            .and_then(serde_json::Value::as_u64),
        gpu_buffer_reuse_hits: stats
            .and_then(|stats| stats.get("gpu_buffer_reuse_hits"))
            .and_then(serde_json::Value::as_u64),
        gpu_reused_dispatches: stats
            .and_then(|stats| stats.get("gpu_reused_dispatches"))
            .and_then(serde_json::Value::as_u64),
        last_backend: stats
            .and_then(|stats| stats.get("last_backend"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        last_auto_reason: stats
            .and_then(|stats| stats.get("last_auto_reason"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
    })
}

fn math_bench_report(samples: &[ToolchainCommandSample]) -> Option<ToolchainMathBenchReport> {
    let math_samples = samples
        .iter()
        .filter_map(|sample| sample.math_bench.as_ref())
        .collect::<Vec<_>>();
    let first = math_samples.first()?;
    Some(ToolchainMathBenchReport {
        op: first.op.clone(),
        size: first.size,
        build_mode: first.build_mode.clone(),
        reuse_options: first.reuse_options,
        submit_only: first.submit_only,
        results: math_backend_reports(&math_samples),
    })
}

fn math_backend_reports(samples: &[&ToolchainMathBenchSample]) -> Vec<ToolchainMathBackendReport> {
    let Some(first) = samples.first() else {
        return Vec::new();
    };
    first
        .results
        .iter()
        .map(|result| math_backend_report(samples, &result.backend))
        .collect()
}

fn math_backend_report(
    samples: &[&ToolchainMathBenchSample],
    backend: &str,
) -> ToolchainMathBackendReport {
    let backend_samples = samples
        .iter()
        .filter_map(|sample| {
            sample
                .results
                .iter()
                .find(|result| result.backend == backend)
        })
        .collect::<Vec<_>>();
    let first = backend_samples
        .first()
        .expect("backend exists in first sample");
    let median_values = backend_samples
        .iter()
        .filter_map(|sample| sample.median_ns)
        .collect::<Vec<_>>();
    ToolchainMathBackendReport {
        backend: first.backend.clone(),
        status: first.status.clone(),
        median_ns: median_u64(median_values.clone()),
        min_ns: median_values.iter().copied().min(),
        max_ns: median_values.iter().copied().max(),
        median_speedup_vs_scalar: median_f64(
            backend_samples
                .iter()
                .filter_map(|sample| sample.speedup_vs_scalar)
                .collect(),
        ),
        median_scalar_calls: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.scalar_calls
        }),
        median_glam_calls: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.glam_calls
        }),
        median_ndarray_calls: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.ndarray_calls
        }),
        median_wgpu_calls: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.wgpu_calls
        }),
        median_fallback_calls: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.fallback_calls
        }),
        median_bytes_borrowed: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.bytes_borrowed
        }),
        median_bytes_copied: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.bytes_copied
        }),
        median_bytes_uploaded: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.bytes_uploaded
        }),
        median_bytes_downloaded: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.bytes_downloaded
        }),
        median_gpu_buffer_reuse_hits: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.gpu_buffer_reuse_hits
        }),
        median_gpu_reused_dispatches: median_math_backend_sample_by(&backend_samples, |sample| {
            sample.gpu_reused_dispatches
        }),
        last_backend: first.last_backend.clone(),
        last_auto_reason: first.last_auto_reason.clone(),
    }
}

fn median_math_backend_sample_by(
    samples: &[&ToolchainMathBackendSample],
    field: impl Fn(&ToolchainMathBackendSample) -> Option<u64>,
) -> Option<u64> {
    median_u64(samples.iter().filter_map(|sample| field(sample)).collect())
}

fn median_u64(mut values: Vec<u64>) -> Option<u64> {
    values.sort_unstable();
    values.get(values.len() / 2).copied()
}

fn median_f64(mut values: Vec<f64>) -> Option<f64> {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied()
}

fn aggregate_status(
    warmup_samples: &[ToolchainCommandSample],
    samples: &[ToolchainCommandSample],
) -> &'static str {
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.any(|sample| sample.status == "spawn_failed") {
        return "spawn_failed";
    }
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.any(|sample| sample.status == "failed") {
        return "failed";
    }
    let mut all_samples = warmup_samples.iter().chain(samples.iter());
    if all_samples.all(|sample| sample.status == "planned") {
        return "planned";
    }
    "ok"
}

fn timing_report(elapsed: &mut [u128]) -> ToolchainTimingReport {
    elapsed.sort_unstable();
    ToolchainTimingReport {
        min: elapsed.first().copied().unwrap_or_default(),
        median: elapsed.get(elapsed.len() / 2).copied().unwrap_or_default(),
        max: elapsed.last().copied().unwrap_or_default(),
    }
}

fn argv_for(spec: ToolchainCommandSpec) -> Vec<&'static str> {
    std::iter::once("cargo")
        .chain(spec.args.iter().copied())
        .collect()
}

fn count_lines(bytes: &[u8]) -> usize {
    if bytes.is_empty() {
        return 0;
    }
    let segments = bytes.split(|byte| *byte == b'\n').count();
    if bytes.ends_with(b"\n") {
        segments.saturating_sub(1)
    } else {
        segments
    }
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    match value.parse::<usize>() {
        Ok(parsed) if parsed > 0 => Ok(parsed),
        Ok(_) => Err("value must be greater than zero".to_owned()),
        Err(error) => Err(error.to_string()),
    }
}

fn print_human_report(report: &ToolchainProfileReport) {
    for command in &report.commands {
        println!(
            "{}: {} (median {} ns, min {} ns, max {} ns, warmup {}, repeat {}, stdout lines {}, stderr lines {})",
            command.label,
            command.status,
            command.elapsed_ns,
            command.timing.min,
            command.timing.max,
            command.warmup,
            command.repeat,
            command.stdout_lines,
            command.stderr_lines
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        arcweft_bench_report, arcweft_bench_sample, count_lines, math_bench_report,
        math_bench_sample, planned_command_sample,
    };

    #[test]
    fn count_lines_does_not_allocate_utf8_strings() {
        assert_eq!(count_lines(b""), 0);
        assert_eq!(count_lines(b"one"), 1);
        assert_eq!(count_lines(b"one\n"), 1);
        assert_eq!(count_lines(b"one\ntwo"), 2);
        assert_eq!(count_lines(b"one\r\ntwo\r\n"), 2);
    }

    #[test]
    fn arcweft_bench_sample_extracts_path_free_runtime_counters() {
        let sample = arcweft_bench_sample(
            br#"{
  "source": "tests/fixtures/arcw/spec_should_pass/bench/003_for_pure_jit.arcw",
  "benches": [{
    "id": "bench.for_pure",
    "sections": [{
      "status": "measured",
      "measurement": {
        "executor": "bytecode_vm",
        "executor_stats": {
          "pure_compile": {
            "object_attempts": 0,
            "object_successes": 0,
            "object_failures": 0,
            "object_bytes": 0,
            "compile_elapsed_ns": 1234
          },
          "math": {
            "scalar_calls": 0,
            "glam_calls": 0,
            "ndarray_calls": 0,
            "wgpu_calls": 0,
            "fused_matmul_bias_add_calls": 0,
            "fallback_calls": 0,
            "bytes_borrowed": 0,
            "bytes_copied": 0,
            "bytes_uploaded": 0,
            "bytes_downloaded": 0,
            "gpu_buffer_reuse_hits": 0,
            "gpu_reused_dispatches": 0,
            "last_backend": null,
            "last_auto_reason": null
          }
        },
        "per_executed_op_ns": 700,
        "elapsed_ns": { "min": 10000, "median": 20000, "max": 30000 },
        "deterministic": {
          "pure_calls_median": 16,
          "pure_batch_calls_median": 0,
          "pure_batch_items_median": 0,
          "pure_jit_calls_median": 16,
          "pure_aot_calls_median": 0,
          "pure_vm_calls_median": 0,
          "pure_fallbacks_median": 0,
          "pure_arg_vec_allocations_median": 0,
          "pure_arg_bytes_borrowed_median": 256,
          "math_calls_median": 0,
          "math_accelerated_calls_median": 0
        }
      }
    }]
  }]
}"#,
        )
        .expect("sample should parse");

        assert_eq!(sample.bench_id, "bench.for_pure");
        assert_eq!(sample.bench_status, "measured");
        assert_eq!(sample.executor, "bytecode_vm");
        assert_eq!(sample.bench_elapsed_ns, 20000);
        assert_eq!(sample.pure_jit_calls, 16);
        assert_eq!(sample.pure_arg_vec_allocations, 0);
        assert_eq!(sample.pure_compile_elapsed_ns, 1234);
        assert_eq!(sample.pure_object_attempts, 0);
        assert_eq!(sample.math_calls, 0);
        assert_eq!(sample.math_bytes_borrowed, 0);
    }

    #[test]
    fn arcweft_bench_report_summarizes_measured_samples_only() {
        let mut planned = planned_command_sample(0);
        let mut first = planned_command_sample(1);
        first.arcweft_bench = Some(super::ToolchainArcweftBenchSample {
            source: "003_for_pure_jit.arcw".to_owned(),
            bench_id: "bench.for_pure".to_owned(),
            bench_status: "measured".to_owned(),
            executor: "bytecode_vm".to_owned(),
            bench_elapsed_ns: 300,
            per_executed_op_ns: 30,
            pure_calls: 16,
            pure_batch_calls: 0,
            pure_batch_items: 0,
            pure_jit_calls: 16,
            pure_aot_calls: 0,
            pure_vm_calls: 0,
            pure_fallbacks: 0,
            pure_arg_vec_allocations: 0,
            pure_arg_bytes_borrowed: 256,
            pure_compile_elapsed_ns: 2000,
            pure_object_attempts: 1,
            pure_object_successes: 1,
            pure_object_failures: 0,
            pure_object_bytes: 467,
            math_calls: 1,
            math_accelerated_calls: 1,
            math_scalar_calls: 0,
            math_glam_calls: 1,
            math_ndarray_calls: 0,
            math_wgpu_calls: 0,
            math_fused_matmul_bias_add_calls: 0,
            math_fallback_calls: 0,
            math_bytes_borrowed: 128,
            math_bytes_copied: 0,
            math_bytes_uploaded: 0,
            math_bytes_downloaded: 0,
            math_gpu_buffer_reuse_hits: 0,
            math_gpu_reused_dispatches: 0,
            math_last_backend: Some("glam".to_owned()),
            math_last_auto_reason: None,
        });
        let mut second = first.clone();
        {
            let sample = second.arcweft_bench.as_mut().expect("bench sample");
            sample.bench_elapsed_ns = 100;
            sample.math_last_backend = Some("ndarray".to_owned());
            sample.math_last_auto_reason = Some("elementwise_ndarray_cpu_default".to_owned());
        }
        let mut third = first.clone();
        {
            let sample = third.arcweft_bench.as_mut().expect("bench sample");
            sample.bench_elapsed_ns = 200;
            sample.math_last_backend = Some("ndarray".to_owned());
            sample.math_last_auto_reason = Some("elementwise_ndarray_cpu_default".to_owned());
        }
        planned.arcweft_bench = None;

        let report = arcweft_bench_report(&[planned, first, second, third])
            .expect("report should summarize bench samples");

        assert_eq!(report.source, "003_for_pure_jit.arcw");
        assert_eq!(report.median_bench_elapsed_ns, 200);
        assert_eq!(report.min_bench_elapsed_ns, 100);
        assert_eq!(report.max_bench_elapsed_ns, 300);
        assert_eq!(report.median_pure_jit_calls, 16);
        assert_eq!(report.median_pure_compile_elapsed_ns, 2000);
        assert_eq!(report.median_pure_object_successes, 1);
        assert_eq!(report.median_pure_object_bytes, 467);
        assert_eq!(report.median_math_calls, 1);
        assert_eq!(report.median_math_glam_calls, 1);
        assert_eq!(report.median_math_bytes_borrowed, 128);
        assert_eq!(report.math_last_backend.as_deref(), Some("ndarray"));
        assert_eq!(
            report.math_last_auto_reason.as_deref(),
            Some("elementwise_ndarray_cpu_default")
        );
    }

    #[test]
    fn math_bench_sample_extracts_backend_counters() {
        let sample = math_bench_sample(
            br#"{
  "bench": "runtime_math",
  "build_mode": "optimized",
  "op": "matrix_add_f32",
  "size": 4096,
  "reuse": false,
  "reuse_update_inputs": false,
  "reuse_capacity": false,
  "submit_only": false,
  "results": [
    {
      "backend": "scalar",
      "status": "measured",
      "median_ns": 100,
      "speedup_vs_scalar": 1.0,
      "stats": {
        "scalar_calls": 6,
        "glam_calls": 0,
        "ndarray_calls": 0,
        "wgpu_calls": 0,
        "fallback_calls": 0,
        "bytes_borrowed": 805306368,
        "bytes_copied": 0,
        "bytes_uploaded": 0,
        "bytes_downloaded": 0,
        "gpu_buffer_reuse_hits": 0,
        "gpu_reused_dispatches": 0,
        "last_backend": "scalar",
        "last_auto_reason": null
      }
    },
    {
      "backend": "auto",
      "status": "measured",
      "median_ns": 105,
      "speedup_vs_scalar": 0.95,
      "stats": {
        "scalar_calls": 6,
        "glam_calls": 0,
        "ndarray_calls": 0,
        "wgpu_calls": 0,
        "fallback_calls": 0,
        "bytes_borrowed": 805306368,
        "bytes_copied": 0,
        "bytes_uploaded": 0,
        "bytes_downloaded": 0,
        "gpu_buffer_reuse_hits": 0,
        "gpu_reused_dispatches": 0,
        "last_backend": "scalar",
        "last_auto_reason": "elementwise_scalar_cpu_default"
      }
    }
  ]
}"#,
        )
        .expect("math bench sample should parse");

        assert_eq!(sample.op, "matrix_add_f32");
        assert_eq!(sample.size, 4096);
        let auto = sample
            .results
            .iter()
            .find(|result| result.backend == "auto")
            .expect("auto backend sample");
        assert_eq!(auto.median_ns, Some(105));
        assert_eq!(auto.scalar_calls, Some(6));
        assert_eq!(auto.ndarray_calls, Some(0));
        assert_eq!(
            auto.last_auto_reason.as_deref(),
            Some("elementwise_scalar_cpu_default")
        );
    }

    #[test]
    fn math_bench_report_summarizes_backend_timings() {
        let mut first = planned_command_sample(0);
        first.math_bench = Some(super::ToolchainMathBenchSample {
            op: "matmul_f32".to_owned(),
            size: 64,
            build_mode: "optimized".to_owned(),
            reuse_options: super::ToolchainMathReuseOptions {
                reuse: false,
                reuse_update_inputs: false,
                reuse_capacity: false,
            },
            submit_only: false,
            results: vec![super::ToolchainMathBackendSample {
                backend: "auto".to_owned(),
                status: "measured".to_owned(),
                median_ns: Some(300),
                speedup_vs_scalar: Some(1.0),
                scalar_calls: Some(12),
                glam_calls: Some(0),
                ndarray_calls: Some(0),
                wgpu_calls: Some(0),
                fallback_calls: Some(0),
                bytes_borrowed: Some(393_216),
                bytes_copied: Some(0),
                bytes_uploaded: Some(0),
                bytes_downloaded: Some(0),
                gpu_buffer_reuse_hits: Some(0),
                gpu_reused_dispatches: Some(0),
                last_backend: Some("scalar".to_owned()),
                last_auto_reason: Some("matmul_scalar_small_work".to_owned()),
            }],
        });
        let mut second = first.clone();
        second.math_bench.as_mut().expect("math sample").results[0].median_ns = Some(100);
        let mut third = first.clone();
        third.math_bench.as_mut().expect("math sample").results[0].median_ns = Some(200);

        let report =
            math_bench_report(&[first, second, third]).expect("math report should summarize");

        assert_eq!(report.op, "matmul_f32");
        assert_eq!(report.results[0].backend, "auto");
        assert_eq!(report.results[0].median_ns, Some(200));
        assert_eq!(report.results[0].min_ns, Some(100));
        assert_eq!(report.results[0].max_ns, Some(300));
        assert_eq!(report.results[0].median_scalar_calls, Some(12));
        assert_eq!(report.results[0].median_ndarray_calls, Some(0));
        assert_eq!(report.results[0].median_bytes_borrowed, Some(393_216));
        assert_eq!(
            report.results[0].last_auto_reason.as_deref(),
            Some("matmul_scalar_small_work")
        );
    }
}
