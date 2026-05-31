use arcweft_core::math::{DenseMatrixF32, DenseTensorF32};
use arcweft_runtime_accelerator::math::{
    RuntimeMathAccelerator, RuntimeMathAcceleratorConfig, RuntimeMathAutoSelectionReason,
    RuntimeMathBackend,
};
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

    println!("{{");
    println!("  \"bench\": \"runtime_math\",");
    println!("  \"op\": \"{}\",", options.op.label());
    println!("  \"size\": {},", options.size);
    println!("  \"iterations\": {},", options.iterations);
    println!("  \"warmup\": {},", options.warmup);
    println!("  \"reuse\": {},", options.reuse);
    println!("  \"results\": [");
    for (index, report) in reports.iter().enumerate() {
        let comma = if index + 1 == reports.len() { "" } else { "," };
        report.print(comma);
    }
    println!("  ]");
    println!("}}");
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

    fn print(&self, comma: &str) {
        println!("    {{");
        println!("      \"backend\": \"{}\",", backend_label(self.backend));
        println!("      \"status\": \"{}\",", self.status);
        print_optional_u128("median_ns", self.median_ns, true);
        print_optional_u128("min_ns", self.min_ns, true);
        print_optional_u128("max_ns", self.max_ns, true);
        if let Some(stats) = self.stats {
            println!("      \"stats\": {{");
            println!("        \"scalar_calls\": {},", stats.scalar_calls);
            println!("        \"glam_calls\": {},", stats.glam_calls);
            println!("        \"ndarray_calls\": {},", stats.ndarray_calls);
            println!("        \"wgpu_calls\": {},", stats.wgpu_calls);
            println!("        \"fallback_calls\": {},", stats.fallback_calls);
            println!("        \"bytes_borrowed\": {},", stats.bytes_borrowed);
            println!("        \"bytes_copied\": {},", stats.bytes_copied);
            println!("        \"bytes_uploaded\": {},", stats.bytes_uploaded);
            println!("        \"bytes_downloaded\": {},", stats.bytes_downloaded);
            println!(
                "        \"gpu_buffer_creations\": {},",
                stats.gpu_buffer_creations
            );
            println!(
                "        \"gpu_buffer_reuse_hits\": {},",
                stats.gpu_buffer_reuse_hits
            );
            println!(
                "        \"gpu_reused_dispatches\": {},",
                stats.gpu_reused_dispatches
            );
            match stats.last_backend {
                Some(backend) => {
                    println!("        \"last_backend\": \"{}\",", backend_label(backend));
                }
                None => println!("        \"last_backend\": null,"),
            }
            match stats.last_auto_reason {
                Some(reason) => {
                    println!(
                        "        \"last_auto_reason\": \"{}\"",
                        auto_reason_label(reason)
                    );
                }
                None => println!("        \"last_auto_reason\": null"),
            }
            println!("      }},");
        } else {
            println!("      \"stats\": null,");
        }
        match &self.diagnostic {
            Some(diagnostic) => println!("      \"diagnostic\": \"{}\"", json_escape(diagnostic)),
            None => println!("      \"diagnostic\": null"),
        }
        println!("    }}{comma}");
    }
}

fn print_optional_u128(key: &str, value: Option<u128>, comma: bool) {
    let suffix = if comma { "," } else { "" };
    match value {
        Some(value) => println!("      \"{key}\": {value}{suffix}"),
        None => println!("      \"{key}\": null{suffix}"),
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

fn json_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn small_f32(value: usize) -> f32 {
    f32::from(u16::try_from(value).expect("fixture residue fits u16"))
}
