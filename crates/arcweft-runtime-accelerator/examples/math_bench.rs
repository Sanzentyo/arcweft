use arcweft_core::math::{DenseMatrixF32, DenseTensorF32};
use arcweft_runtime_accelerator::math::{
    RuntimeMathAccelerator, RuntimeMathAcceleratorConfig, RuntimeMathAutoSelectionReason,
    RuntimeMathBackend, RuntimeMathStats,
};
use serde::Serialize;
use std::time::Instant;

fn main() {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    let options = BenchOptions::parse(&args);
    let backends = match options.backend {
        BenchBackend::All => vec![
            RuntimeMathBackend::Scalar,
            RuntimeMathBackend::Glam,
            RuntimeMathBackend::Ndarray,
            RuntimeMathBackend::Wgpu,
            RuntimeMathBackend::Auto,
        ],
        BenchBackend::One(backend) => vec![backend],
    };
    let reports = backends
        .into_iter()
        .map(|backend| run_backend(backend, &options))
        .collect::<Vec<_>>();

    let report = MathBenchReport::new(&options, reports);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("math bench report serializes")
    );
}

fn run_backend(backend: RuntimeMathBackend, options: &BenchOptions) -> BackendReport {
    let mut accelerator = RuntimeMathAccelerator::new(RuntimeMathAcceleratorConfig {
        backend,
        wgpu_min_elements: options.wgpu_min_elements,
    });
    match options.op {
        BenchOp::Matmul => run_matmul(&mut accelerator, backend, options),
        BenchOp::MatrixAdd => run_matrix_add(&mut accelerator, backend, options),
        BenchOp::TensorAdd => run_tensor_add(&mut accelerator, backend, options),
    }
}

fn run_matmul(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let lhs = matrix_fixture(options.size, options.size, 1.0);
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let reference = match lhs.matmul_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matmul_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matmul_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-3) {
            return BackendReport::failed(backend, "matmul result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_matrix_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let lhs = matrix_fixture(options.size, options.size, 1.0);
    let rhs = matrix_fixture(options.size, options.size, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    if options.reuse {
        return run_prepared_matrix_add(accelerator, backend, options, &lhs, &rhs, &reference);
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.matrix_add_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.matrix_add_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-6) {
            return BackendReport::failed(backend, "matrix add result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_tensor_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
) -> BackendReport {
    let elements = options.size * options.size;
    let lhs = tensor_fixture(elements, 1.0);
    let rhs = tensor_fixture(elements, 0.25);
    let reference = match lhs.add_scalar(&rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::failed(backend, error.to_string()),
    };

    if options.reuse {
        return run_prepared_tensor_add(accelerator, backend, options, &lhs, &rhs, &reference);
    }

    for _ in 0..options.warmup {
        if let Err(error) = accelerator.tensor_add_f32(&lhs, &rhs) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }

    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        let output = match accelerator.tensor_add_f32(&lhs, &rhs) {
            Ok(value) => value,
            Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
        };
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(output.values(), reference.values(), 1.0e-6) {
            return BackendReport::failed(backend, "tensor add result mismatch".to_owned());
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_prepared_matrix_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseMatrixF32,
    rhs: &DenseMatrixF32,
    reference: &DenseMatrixF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match accelerator.prepare_matrix_add_f32(lhs, rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let mut output = vec![0.0; lhs.values().len()];
    for _ in 0..options.warmup {
        if let Err(error) = accelerator.run_prepared_matrix_add_f32_into(&prepared, &mut output) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        if let Err(error) = accelerator.run_prepared_matrix_add_f32_into(&prepared, &mut output) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(&output, reference.values(), 1.0e-6) {
            return BackendReport::failed(
                backend,
                "prepared matrix add result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn run_prepared_tensor_add(
    accelerator: &mut RuntimeMathAccelerator,
    backend: RuntimeMathBackend,
    options: &BenchOptions,
    lhs: &DenseTensorF32,
    rhs: &DenseTensorF32,
    reference: &DenseTensorF32,
) -> BackendReport {
    if backend != RuntimeMathBackend::Wgpu {
        return BackendReport::skipped_or_failed(
            backend,
            "prepared GPU reuse is only available for the wgpu backend".to_owned(),
        );
    }
    let prepared = match accelerator.prepare_tensor_add_f32(lhs, rhs) {
        Ok(value) => value,
        Err(error) => return BackendReport::skipped_or_failed(backend, error.to_string()),
    };
    let mut output = vec![0.0; lhs.values().len()];
    for _ in 0..options.warmup {
        if let Err(error) = accelerator.run_prepared_tensor_add_f32_into(&prepared, &mut output) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
    }
    let mut samples = Vec::with_capacity(options.iterations);
    for _ in 0..options.iterations {
        let started = Instant::now();
        if let Err(error) = accelerator.run_prepared_tensor_add_f32_into(&prepared, &mut output) {
            return BackendReport::skipped_or_failed(backend, error.to_string());
        }
        let elapsed = started.elapsed().as_nanos();
        if !approx_eq(&output, reference.values(), 1.0e-6) {
            return BackendReport::failed(
                backend,
                "prepared tensor add result mismatch".to_owned(),
            );
        }
        samples.push(elapsed);
    }
    BackendReport::measured(backend, samples, accelerator.stats())
}

fn matrix_fixture(rows: usize, cols: usize, scale: f32) -> DenseMatrixF32 {
    DenseMatrixF32::new(
        rows,
        cols,
        (0..rows * cols)
            .map(|index| {
                let value = small_f32(index % 31) - 15.0;
                value * scale
            })
            .collect(),
    )
    .expect("fixture shape is valid")
}

fn tensor_fixture(elements: usize, scale: f32) -> DenseTensorF32 {
    DenseTensorF32::new(
        vec![elements],
        (0..elements)
            .map(|index| (small_f32(index % 127) - 63.0) * scale)
            .collect(),
    )
    .expect("fixture shape is valid")
}

fn approx_eq(lhs: &[f32], rhs: &[f32], epsilon: f32) -> bool {
    lhs.len() == rhs.len()
        && lhs
            .iter()
            .zip(rhs.iter())
            .all(|(lhs, rhs)| (lhs - rhs).abs() <= epsilon)
}

#[derive(Clone, Copy)]
enum BenchOp {
    Matmul,
    MatrixAdd,
    TensorAdd,
}

impl BenchOp {
    const fn label(self) -> &'static str {
        match self {
            Self::Matmul => "matmul_f32",
            Self::MatrixAdd => "matrix_add_f32",
            Self::TensorAdd => "tensor_add_f32",
        }
    }
}

enum BenchBackend {
    All,
    One(RuntimeMathBackend),
}

struct BenchOptions {
    backend: BenchBackend,
    op: BenchOp,
    size: usize,
    iterations: usize,
    warmup: usize,
    wgpu_min_elements: usize,
    reuse: bool,
}

impl BenchOptions {
    fn parse(args: &[String]) -> Self {
        let mut options = Self {
            backend: BenchBackend::All,
            op: BenchOp::Matmul,
            size: 64,
            iterations: 10,
            warmup: 2,
            wgpu_min_elements: RuntimeMathAcceleratorConfig::default().wgpu_min_elements,
            reuse: false,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--backend" => {
                    index += 1;
                    options.backend = match args.get(index).map(String::as_str) {
                        Some("all") | None => BenchBackend::All,
                        Some(value) => BenchBackend::One(parse_backend(value)),
                    };
                }
                "--op" => {
                    index += 1;
                    options.op = match args.get(index).map(String::as_str) {
                        Some("matrix-add" | "matrix_add" | "matrix_add_f32") => BenchOp::MatrixAdd,
                        Some("tensor-add" | "tensor_add" | "tensor_add_f32") => BenchOp::TensorAdd,
                        _ => BenchOp::Matmul,
                    };
                }
                "--size" => {
                    index += 1;
                    options.size = parse_usize(args.get(index), options.size);
                }
                "--iterations" => {
                    index += 1;
                    options.iterations = parse_usize(args.get(index), options.iterations);
                }
                "--warmup" => {
                    index += 1;
                    options.warmup = parse_usize(args.get(index), options.warmup);
                }
                "--wgpu-min-elements" => {
                    index += 1;
                    options.wgpu_min_elements =
                        parse_usize(args.get(index), options.wgpu_min_elements);
                }
                "--reuse" => {
                    options.reuse = true;
                }
                _ => {}
            }
            index += 1;
        }
        options
    }
}

fn parse_backend(value: &str) -> RuntimeMathBackend {
    match value {
        "scalar" => RuntimeMathBackend::Scalar,
        "glam" => RuntimeMathBackend::Glam,
        "ndarray" => RuntimeMathBackend::Ndarray,
        "wgpu" => RuntimeMathBackend::Wgpu,
        _ => RuntimeMathBackend::Auto,
    }
}

fn parse_usize(value: Option<&String>, fallback: usize) -> usize {
    value
        .and_then(|value| value.parse().ok())
        .unwrap_or(fallback)
}

struct BackendReport {
    backend: RuntimeMathBackend,
    status: &'static str,
    median_ns: Option<u128>,
    min_ns: Option<u128>,
    max_ns: Option<u128>,
    stats: Option<arcweft_runtime_accelerator::math::RuntimeMathStats>,
    diagnostic: Option<String>,
}

#[derive(Serialize)]
struct MathBenchReport {
    bench: &'static str,
    op: &'static str,
    size: usize,
    iterations: usize,
    warmup: usize,
    reuse: bool,
    results: Vec<BackendReportJson>,
}

impl MathBenchReport {
    fn new(options: &BenchOptions, results: Vec<BackendReport>) -> Self {
        Self {
            bench: "runtime_math",
            op: options.op.label(),
            size: options.size,
            iterations: options.iterations,
            warmup: options.warmup,
            reuse: options.reuse,
            results: results.into_iter().map(BackendReport::into_json).collect(),
        }
    }
}

#[derive(Serialize)]
struct BackendReportJson {
    backend: &'static str,
    status: &'static str,
    median_ns: Option<u128>,
    min_ns: Option<u128>,
    max_ns: Option<u128>,
    stats: Option<RuntimeMathStatsJson>,
    diagnostic: Option<String>,
}

#[derive(Serialize)]
struct RuntimeMathStatsJson {
    scalar_calls: usize,
    glam_calls: usize,
    ndarray_calls: usize,
    wgpu_calls: usize,
    fallback_calls: usize,
    bytes_borrowed: usize,
    bytes_copied: usize,
    bytes_uploaded: usize,
    bytes_downloaded: usize,
    gpu_buffer_creations: usize,
    gpu_buffer_reuse_hits: usize,
    gpu_reused_dispatches: usize,
    last_backend: Option<&'static str>,
    last_auto_reason: Option<&'static str>,
}

impl From<RuntimeMathStats> for RuntimeMathStatsJson {
    fn from(stats: RuntimeMathStats) -> Self {
        Self {
            scalar_calls: stats.scalar_calls,
            glam_calls: stats.glam_calls,
            ndarray_calls: stats.ndarray_calls,
            wgpu_calls: stats.wgpu_calls,
            fallback_calls: stats.fallback_calls,
            bytes_borrowed: stats.bytes_borrowed,
            bytes_copied: stats.bytes_copied,
            bytes_uploaded: stats.bytes_uploaded,
            bytes_downloaded: stats.bytes_downloaded,
            gpu_buffer_creations: stats.gpu_buffer_creations,
            gpu_buffer_reuse_hits: stats.gpu_buffer_reuse_hits,
            gpu_reused_dispatches: stats.gpu_reused_dispatches,
            last_backend: stats.last_backend.map(backend_label),
            last_auto_reason: stats.last_auto_reason.map(auto_reason_label),
        }
    }
}

impl BackendReport {
    fn measured(
        backend: RuntimeMathBackend,
        mut samples: Vec<u128>,
        stats: arcweft_runtime_accelerator::math::RuntimeMathStats,
    ) -> Self {
        samples.sort_unstable();
        Self {
            backend,
            status: "measured",
            median_ns: samples.get(samples.len() / 2).copied(),
            min_ns: samples.first().copied(),
            max_ns: samples.last().copied(),
            stats: Some(stats),
            diagnostic: None,
        }
    }

    fn skipped_or_failed(backend: RuntimeMathBackend, diagnostic: String) -> Self {
        let status = if backend == RuntimeMathBackend::Wgpu {
            "skipped"
        } else {
            "failed"
        };
        Self {
            backend,
            status,
            median_ns: None,
            min_ns: None,
            max_ns: None,
            stats: None,
            diagnostic: Some(diagnostic),
        }
    }

    fn failed(backend: RuntimeMathBackend, diagnostic: String) -> Self {
        Self {
            backend,
            status: "failed",
            median_ns: None,
            min_ns: None,
            max_ns: None,
            stats: None,
            diagnostic: Some(diagnostic),
        }
    }

    fn into_json(self) -> BackendReportJson {
        BackendReportJson {
            backend: backend_label(self.backend),
            status: self.status,
            median_ns: self.median_ns,
            min_ns: self.min_ns,
            max_ns: self.max_ns,
            stats: self.stats.map(Into::into),
            diagnostic: self.diagnostic,
        }
    }
}

const fn backend_label(value: RuntimeMathBackend) -> &'static str {
    match value {
        RuntimeMathBackend::Scalar => "scalar",
        RuntimeMathBackend::Glam => "glam",
        RuntimeMathBackend::Ndarray => "ndarray",
        RuntimeMathBackend::Wgpu => "wgpu",
        RuntimeMathBackend::Auto => "auto",
    }
}

const fn auto_reason_label(value: RuntimeMathAutoSelectionReason) -> &'static str {
    match value {
        RuntimeMathAutoSelectionReason::Matmul4x4Glam => "matmul_4x4_glam",
        RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold => "matmul_wgpu_work_threshold",
        RuntimeMathAutoSelectionReason::MatmulCpuDefault => "matmul_cpu_default",
        RuntimeMathAutoSelectionReason::ElementwiseCpuDefault => "elementwise_cpu_default",
    }
}

fn small_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).expect("fixture residue fits u16"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_serializes_auto_reason_without_host_paths() {
        let options = BenchOptions {
            backend: BenchBackend::One(RuntimeMathBackend::Auto),
            op: BenchOp::Matmul,
            size: 4,
            iterations: 1,
            warmup: 0,
            wgpu_min_elements: RuntimeMathAcceleratorConfig::default().wgpu_min_elements,
            reuse: false,
        };
        let report = MathBenchReport::new(
            &options,
            vec![BackendReport::measured(
                RuntimeMathBackend::Auto,
                vec![100],
                RuntimeMathStats {
                    glam_calls: 1,
                    last_backend: Some(RuntimeMathBackend::Glam),
                    last_auto_reason: Some(RuntimeMathAutoSelectionReason::Matmul4x4Glam),
                    ..RuntimeMathStats::default()
                },
            )],
        );

        let json = serde_json::to_string(&report).expect("report serializes");

        assert!(json.contains("\"last_auto_reason\":\"matmul_4x4_glam\""));
        let windows_drive_prefixes = ["C:", "D:"].map(|drive| format!("{drive}\\"));
        for prefix in windows_drive_prefixes {
            assert!(!json.contains(&prefix));
        }
        assert!(!json.contains(&["/", "home", "/"].concat()));
        assert!(!json.contains("/tmp/"));
    }
}
