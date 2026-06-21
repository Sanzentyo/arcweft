use super::project::{CheckedModule, load_and_check_with_env};
use super::shared::print_json;
use crate::output::{
    BorrowCheckProfileStats, RuntimeProfilePhase, ScriptBenchPureHelperBatchSummary,
    ScriptBenchPureHelperDeterministicSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperStatsSummary, ScriptBenchPureHelperTimingSamples,
    ScriptBenchPureHelperTimingSummary, TypeCheckProfileStats,
};
use arcweft_compiler::lower::lower_source_pure_helper_candidates;
use arcweft_core::{
    plan::{
        RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
        RuntimePureOutputType,
    },
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackendKind, PureFunctionRequest,
        PureFunctionResult, PureFunctionStats, RuntimeI64Args, VmPureFunctionBackend,
        VmPureFunctionScratch, compare_pure_function_backend,
    },
    value::{
        DenseSeq, RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeExpr,
        RuntimeIntrinsic, RuntimeSeq, RuntimeUnaryOp, RuntimeValue,
    },
};
use arcweft_lang_jit_cranelift::{
    CompiledPureI64Batch, CompiledPureI64Inputs, CraneliftPureFunctionBackend,
};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_runtime_host::{HostSystemInfo, host_system_info};
use arcweft_runtime_plan::pure::PureHelperCandidate;
use clap::{Args, ValueEnum};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

use super::commands::JitCommand;

#[derive(Args, Clone, Debug)]
pub(in crate::app) struct JitCheckOptions {
    pub(in crate::app) path: Option<PathBuf>,
    #[arg(long)]
    pub(in crate::app) helper: Option<String>,
    #[arg(long = "case", value_enum, default_value = "score")]
    pub(in crate::app) case: JitBuiltinCase,
    #[arg(long)]
    pub(in crate::app) julia: bool,
    #[arg(long, default_value_t = 1000)]
    pub(in crate::app) iterations: usize,
    #[arg(long, default_value_t = 10)]
    pub(in crate::app) warmup: usize,
    #[arg(long, default_value_t = 5)]
    pub(in crate::app) samples: usize,
    #[arg(long, default_value_t = 0)]
    pub(in crate::app) input_seed: u64,
    #[arg(long)]
    pub(in crate::app) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(in crate::app) enum JitBuiltinCase {
    Score,
    BranchMix,
    LetChain,
    FourInputMix,
    AccumulationMix,
}

#[derive(serde::Serialize)]
pub(in crate::app) struct JitCheckReport {
    status: String,
    pub(in crate::app) helper: String,
    helper_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_compiler: Option<JitCheckSourceCompilerReport>,
    workload: JitCheckWorkloadReport,
    input_bindings: Vec<String>,
    dynamic_inputs: bool,
    input_seed: u64,
    host_system: HostSystemInfo,
    vm_backend: String,
    aot_backend: String,
    jit_backend: String,
    pub(in crate::app) matches_vm: bool,
    vm_value: String,
    aot_value: String,
    jit_value: String,
    warmup: usize,
    iterations: usize,
    samples: usize,
    pub(in crate::app) timings: JitCheckTimingReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    julia: Option<JitCheckJuliaReport>,
    pub(in crate::app) deterministic: JitCheckDeterministicReport,
    pub(in crate::app) jit_batch: JitCheckBatchReport,
    pub(in crate::app) vm_stats: PureFunctionStatsReport,
    pub(in crate::app) aot_stats: PureFunctionStatsReport,
    pub(in crate::app) jit_stats: PureFunctionStatsReport,
}

#[derive(serde::Serialize)]
struct JitCheckWorkloadReport {
    case: String,
    loop_kind: String,
    inputs_per_iteration: usize,
    batch_iterations: usize,
}

#[derive(serde::Serialize)]
pub(in crate::app) struct JitCheckTimingReport {
    #[serde(rename = "aot_compile_elapsed_ns")]
    aot_compile: u128,
    #[serde(rename = "compile_elapsed_ns")]
    compile: u128,
    #[serde(rename = "aot_elapsed_ns")]
    aot: u128,
    #[serde(rename = "jit_elapsed_ns")]
    jit: u128,
    #[serde(rename = "vm_elapsed_ns")]
    pub(in crate::app) vm: u128,
    #[serde(rename = "aot_per_iteration_ns")]
    aot_per_iteration: u128,
    #[serde(rename = "jit_per_iteration_ns")]
    jit_per_iteration: u128,
    #[serde(rename = "vm_per_iteration_ns")]
    vm_per_iteration: u128,
    aot_speedup_x: String,
    speedup_x: String,
    aot_samples: JitTimingSamples,
    jit_samples: JitTimingSamples,
    vm_samples: JitTimingSamples,
}

#[derive(serde::Serialize)]
struct JitCheckJuliaReport {
    backend: String,
    version: String,
    matches_vm: bool,
    #[serde(rename = "elapsed_ns")]
    elapsed: u128,
    #[serde(rename = "per_iteration_ns")]
    per_iteration: u128,
    samples: JitTimingSamples,
    accumulator: i64,
    jit_vs_julia_x: String,
    julia_vs_jit_x: String,
    jit_batch_vs_julia_x: String,
    julia_vs_jit_batch_x: String,
}

#[derive(serde::Serialize)]
pub(in crate::app) struct JitCheckBatchReport {
    backend: String,
    #[serde(rename = "compile_elapsed_ns")]
    compile: u128,
    matches_vm: bool,
    #[serde(rename = "elapsed_ns")]
    pub(in crate::app) elapsed: u128,
    #[serde(rename = "per_iteration_ns")]
    per_iteration: u128,
    speedup_x: String,
    jit_call_speedup_x: String,
    samples: JitTimingSamples,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
pub(in crate::app) struct JitTimingSamples {
    pub(in crate::app) min: u128,
    pub(in crate::app) median: u128,
    pub(in crate::app) max: u128,
}

#[derive(Clone, Copy, Debug)]
struct JitRepeatedMeasurement {
    elapsed: JitTimingSamples,
    accumulator: i64,
}

#[derive(serde::Serialize)]
pub(in crate::app) struct JitCheckDeterministicReport {
    #[serde(rename = "aot_accumulator")]
    pub(in crate::app) aot: i64,
    #[serde(rename = "jit_accumulator")]
    pub(in crate::app) jit: i64,
    #[serde(rename = "jit_batch_accumulator")]
    pub(in crate::app) jit_batch: i64,
    #[serde(rename = "vm_accumulator")]
    pub(in crate::app) vm: i64,
}

impl From<&JitCheckReport> for ScriptBenchPureHelperMeasurementSummary {
    fn from(report: &JitCheckReport) -> Self {
        Self {
            host_system: host_system_info(),
            helper: report.helper.clone(),
            input_bindings: report.input_bindings.clone(),
            matches_vm: report.matches_vm,
            warmup: report.warmup,
            iterations: report.iterations,
            samples: report.samples,
            timings: ScriptBenchPureHelperTimingSummary {
                aot_compile_elapsed_ns: report.timings.aot_compile,
                compile_elapsed_ns: report.timings.compile,
                aot_elapsed_ns: report.timings.aot,
                jit_elapsed_ns: report.timings.jit,
                vm_elapsed_ns: report.timings.vm,
                aot_per_iteration_ns: report.timings.aot_per_iteration,
                jit_per_iteration_ns: report.timings.jit_per_iteration,
                vm_per_iteration_ns: report.timings.vm_per_iteration,
                aot_speedup_x: report.timings.aot_speedup_x.clone(),
                speedup_x: report.timings.speedup_x.clone(),
                aot_samples: ScriptBenchPureHelperTimingSamples::from(report.timings.aot_samples),
                jit_samples: ScriptBenchPureHelperTimingSamples::from(report.timings.jit_samples),
                vm_samples: ScriptBenchPureHelperTimingSamples::from(report.timings.vm_samples),
            },
            jit_batch: ScriptBenchPureHelperBatchSummary {
                compile_elapsed_ns: report.jit_batch.compile,
                elapsed_ns: report.jit_batch.elapsed,
                per_iteration_ns: report.jit_batch.per_iteration,
                speedup_x: report.jit_batch.speedup_x.clone(),
                jit_call_speedup_x: report.jit_batch.jit_call_speedup_x.clone(),
                samples: ScriptBenchPureHelperTimingSamples::from(report.jit_batch.samples),
            },
            runtime_batch: None,
            deterministic: ScriptBenchPureHelperDeterministicSummary {
                aot: report.deterministic.aot,
                jit: report.deterministic.jit,
                jit_batch: report.deterministic.jit_batch,
                vm: report.deterministic.vm,
            },
            vm_stats: ScriptBenchPureHelperStatsSummary::from(&report.vm_stats),
            aot_stats: ScriptBenchPureHelperStatsSummary::from(&report.aot_stats),
            jit_stats: ScriptBenchPureHelperStatsSummary::from(&report.jit_stats),
        }
    }
}

impl From<JitTimingSamples> for ScriptBenchPureHelperTimingSamples {
    fn from(samples: JitTimingSamples) -> Self {
        Self {
            min: samples.min,
            median: samples.median,
            max: samples.max,
        }
    }
}

#[derive(serde::Serialize)]
pub(in crate::app) struct PureFunctionStatsReport {
    #[serde(rename = "evaluated_exprs")]
    exprs: usize,
    #[serde(rename = "evaluated_calls")]
    calls: usize,
    #[serde(rename = "evaluated_method_calls")]
    method_calls: usize,
    #[serde(rename = "evaluated_binary_ops")]
    binary_ops: usize,
}

impl From<&PureFunctionStatsReport> for ScriptBenchPureHelperStatsSummary {
    fn from(stats: &PureFunctionStatsReport) -> Self {
        Self {
            exprs: stats.exprs,
            calls: stats.calls,
            method_calls: stats.method_calls,
            binary_ops: stats.binary_ops,
        }
    }
}

impl PureFunctionStatsReport {
    fn from_stats(stats: &PureFunctionStats) -> Self {
        Self {
            exprs: stats.evaluated_exprs,
            calls: stats.evaluated_calls,
            method_calls: stats.evaluated_method_calls,
            binary_ops: stats.evaluated_binary_ops,
        }
    }
}

fn backend_label(kind: PureFunctionBackendKind) -> &'static str {
    match kind {
        PureFunctionBackendKind::Vm => "vm",
        PureFunctionBackendKind::Aot => "aot",
        PureFunctionBackendKind::Jit => "jit",
    }
}

fn runtime_value_summary(value: &RuntimeValue) -> String {
    match value {
        RuntimeValue::Unit => "()".to_owned(),
        RuntimeValue::Bool(value) => value.to_string(),
        RuntimeValue::Int(value) => value.to_string(),
        RuntimeValue::UInt(value) => value.to_string(),
        RuntimeValue::F32(value) => value.to_string(),
        RuntimeValue::F64(value) => value.to_string(),
        RuntimeValue::MatrixF32(value) => {
            format!("matrix/f32/{}x{}", value.rows(), value.cols())
        }
        RuntimeValue::MatrixF64(value) => {
            format!("matrix/f64/{}x{}", value.rows(), value.cols())
        }
        RuntimeValue::TensorF32(value) => format!("tensor/f32/{:?}", value.shape().dims()),
        RuntimeValue::TensorF64(value) => format!("tensor/f64/{:?}", value.shape().dims()),
        RuntimeValue::String(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::EntityRef(value) => format!("@{value}"),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::Seq(RuntimeSeq::Values(values)) => format!("seq/values/{}", values.len()),
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Units(len))) => format!("seq/units/{len}"),
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I8(values))) => {
            format!("seq/i8/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I16(values))) => {
            format!("seq/i16/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I32(values))) => {
            format!("seq/i32/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I64(values))) => {
            format!("seq/i64/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::I128(values))) => {
            format!("seq/i128/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::ISize(values))) => {
            format!("seq/isize/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U8(values))) => {
            format!("seq/u8/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U16(values))) => {
            format!("seq/u16/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U32(values))) => {
            format!("seq/u32/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U64(values))) => {
            format!("seq/u64/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::U128(values))) => {
            format!("seq/u128/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::USize(values))) => {
            format!("seq/usize/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F32(values))) => {
            format!("seq/f32/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::F64(values))) => {
            format!("seq/f64/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bool(values))) => {
            format!("seq/bool/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Bytes(values))) => {
            format!("seq/bytes/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Chars(values))) => {
            format!("seq/chars/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Durations(values))) => {
            format!("seq/durations/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Strings(values))) => {
            format!("seq/strings/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::EntityRefs(values))) => {
            format!("seq/entity_refs/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::TupleColumns(values)) => {
            format!("seq/tuple_columns/{}", values.len())
        }
        RuntimeValue::Seq(RuntimeSeq::RecordColumns(values)) => {
            format!("seq/record_columns/{}", values.len())
        }
        RuntimeValue::Record(fields) => format!("record/{}", fields.len()),
        RuntimeValue::Variant { name, payload, .. } => {
            if payload.is_some() {
                format!(".{name}(...)")
            } else {
                format!(".{name}")
            }
        }
    }
}

pub(super) fn jit_command(command: JitCommand) -> Result<(), ExitCode> {
    match command {
        JitCommand::Check(options) => jit_check_command(&options),
    }
}

fn jit_check_command(options: &JitCheckOptions) -> Result<(), ExitCode> {
    if options.iterations == 0 {
        eprintln!("error: --iterations must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.path.is_some() && options.case != JitBuiltinCase::Score {
        eprintln!("error: --case selects a builtin workload and cannot be combined with PATH");
        return Err(ExitCode::from(2));
    }

    let target = jit_check_target(options)?;
    let report = run_jit_check(options, &target)?;

    if options.json {
        print_json(&report)?;
    } else {
        print_jit_check_human_report(&report);
    }

    if report.matches_vm {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

pub(in crate::app) fn run_jit_check(
    options: &JitCheckOptions,
    target: &JitCheckTarget,
) -> Result<JitCheckReport, ExitCode> {
    let first_inputs = jit_check_inputs(options.input_seed, 0, 0, target.input_names.len());
    let request = target.request_with_inputs(&first_inputs);
    let conformance = collect_jit_check_conformance(&request)?;
    let compiled = compile_jit_check_helpers(&request, target)?;
    let measurement = measure_jit_check_helpers(options, target, &compiled)?;
    Ok(jit_check_report(
        options,
        target,
        &conformance,
        &compiled,
        &measurement,
    ))
}

fn jit_check_report(
    options: &JitCheckOptions,
    target: &JitCheckTarget,
    conformance: &JitCheckConformanceSet,
    compiled: &JitCheckCompiledHelpers,
    measurement: &JitCheckMeasurements,
) -> JitCheckReport {
    let matches_vm = conformance.aot_matches_vm
        && conformance.jit_matches_vm
        && measurement.jit.accumulator == measurement.vm.accumulator
        && measurement.jit_batch.accumulator == measurement.vm.accumulator
        && measurement.aot.accumulator == measurement.vm.accumulator
        && measurement
            .julia
            .as_ref()
            .is_none_or(|julia| julia.accumulator == measurement.vm.accumulator);
    JitCheckReport {
        status: if matches_vm { "ok" } else { "failed" }.to_owned(),
        helper: target.name.clone(),
        helper_source: target.source.as_str().to_owned(),
        source_compiler: target.source_compiler.clone(),
        workload: JitCheckWorkloadReport {
            case: target.name.clone(),
            loop_kind: "deterministic_input_series".to_owned(),
            inputs_per_iteration: target.input_names.len(),
            batch_iterations: options.iterations,
        },
        input_bindings: target.input_names.clone(),
        dynamic_inputs: !target.input_names.is_empty(),
        input_seed: options.input_seed,
        host_system: host_system_info(),
        vm_backend: backend_label(conformance.vm.backend).to_owned(),
        aot_backend: backend_label(conformance.aot.backend).to_owned(),
        jit_backend: backend_label(conformance.jit.backend).to_owned(),
        matches_vm,
        vm_value: runtime_value_summary(&conformance.vm.value),
        aot_value: runtime_value_summary(&conformance.aot.value),
        jit_value: runtime_value_summary(&conformance.jit.value),
        warmup: options.warmup,
        iterations: options.iterations,
        samples: options.samples,
        timings: JitCheckTimingReport {
            aot_compile: compiled.aot_compile_elapsed_ns,
            compile: compiled.jit_compile_elapsed_ns,
            aot: measurement.aot.elapsed.median,
            jit: measurement.jit.elapsed.median,
            vm: measurement.vm.elapsed.median,
            aot_per_iteration: per_iteration_ns(measurement.aot.elapsed.median, options.iterations),
            jit_per_iteration: per_iteration_ns(measurement.jit.elapsed.median, options.iterations),
            vm_per_iteration: per_iteration_ns(measurement.vm.elapsed.median, options.iterations),
            aot_speedup_x: speedup_x(
                measurement.vm.elapsed.median,
                measurement.aot.elapsed.median,
            ),
            speedup_x: speedup_x(
                measurement.vm.elapsed.median,
                measurement.jit.elapsed.median,
            ),
            aot_samples: measurement.aot.elapsed,
            jit_samples: measurement.jit.elapsed,
            vm_samples: measurement.vm.elapsed,
        },
        julia: measurement
            .julia
            .as_ref()
            .map(|julia| jit_check_julia_report(options, measurement, julia)),
        deterministic: JitCheckDeterministicReport {
            aot: measurement.aot.accumulator,
            jit: measurement.jit.accumulator,
            jit_batch: measurement.jit_batch.accumulator,
            vm: measurement.vm.accumulator,
        },
        jit_batch: JitCheckBatchReport {
            backend: "jit_batch".to_owned(),
            compile: compiled.jit_batch_compile_elapsed_ns,
            matches_vm: measurement.jit_batch.accumulator == measurement.vm.accumulator,
            elapsed: measurement.jit_batch.elapsed.median,
            per_iteration: per_iteration_ns(
                measurement.jit_batch.elapsed.median,
                options.iterations,
            ),
            speedup_x: speedup_x(
                measurement.vm.elapsed.median,
                measurement.jit_batch.elapsed.median,
            ),
            jit_call_speedup_x: speedup_x(
                measurement.jit.elapsed.median,
                measurement.jit_batch.elapsed.median,
            ),
            samples: measurement.jit_batch.elapsed,
        },
        vm_stats: PureFunctionStatsReport::from_stats(&conformance.vm.stats),
        aot_stats: PureFunctionStatsReport::from_stats(&conformance.aot.stats),
        jit_stats: PureFunctionStatsReport::from_stats(compiled.jit.stats()),
    }
}

fn jit_check_julia_report(
    options: &JitCheckOptions,
    measurement: &JitCheckMeasurements,
    julia: &JitJuliaMeasurement,
) -> JitCheckJuliaReport {
    JitCheckJuliaReport {
        backend: "julia".to_owned(),
        version: julia.version.clone(),
        matches_vm: julia.accumulator == measurement.vm.accumulator,
        elapsed: julia.elapsed.median,
        per_iteration: per_iteration_ns(julia.elapsed.median, options.iterations),
        samples: julia.elapsed,
        accumulator: julia.accumulator,
        jit_vs_julia_x: speedup_x(julia.elapsed.median, measurement.jit.elapsed.median),
        julia_vs_jit_x: speedup_x(measurement.jit.elapsed.median, julia.elapsed.median),
        jit_batch_vs_julia_x: speedup_x(julia.elapsed.median, measurement.jit_batch.elapsed.median),
        julia_vs_jit_batch_x: speedup_x(measurement.jit_batch.elapsed.median, julia.elapsed.median),
    }
}

fn print_jit_check_human_report(report: &JitCheckReport) {
    let julia = report.julia.as_ref().map_or(String::new(), |julia| {
        format!(
            " julia_median_ns={} jit_vs_julia_x={}",
            julia.elapsed, julia.jit_vs_julia_x
        )
    });
    println!(
        "ok: jit check helper={} matches_vm={} aot_compile_ns={} jit_compile_ns={} aot_median_ns={} jit_median_ns={} vm_median_ns={} jit_speedup_x={}",
        report.helper,
        report.matches_vm,
        report.timings.aot_compile,
        report.timings.compile,
        report.timings.aot,
        report.timings.jit,
        report.timings.vm,
        report.timings.speedup_x
    );
    println!(
        "jit_batch_median_ns={} jit_batch_speedup_x={} jit_call_speedup_x={}",
        report.jit_batch.elapsed, report.jit_batch.speedup_x, report.jit_batch.jit_call_speedup_x
    );
    if !julia.is_empty() {
        println!("{julia}");
    }
}

fn jit_check_inputs(seed: u64, sample: usize, iteration: usize, arity: usize) -> Vec<i64> {
    (0..arity)
        .map(|index| jit_check_input_value(seed, sample, iteration, index))
        .collect()
}

pub(in crate::app) fn jit_check_input_array(
    seed: u64,
    sample: usize,
    iteration: usize,
    arity: usize,
) -> [i64; 4] {
    let mut values = [0_i64; 4];
    for (index, slot) in values.iter_mut().enumerate().take(arity) {
        *slot = jit_check_input_value(seed, sample, iteration, index);
    }
    values
}

fn jit_check_input_value(seed: u64, sample: usize, iteration: usize, index: usize) -> i64 {
    let sample = u64::try_from(sample).unwrap_or_default();
    let iteration = u64::try_from(iteration).unwrap_or_default();
    let index = u64::try_from(index).unwrap_or_default();
    let modulus = 5 + index % 5;
    i64::try_from(
        seed.saturating_mul(index + 1)
            .saturating_add(sample.saturating_mul(3 + index))
            .saturating_add(iteration)
            % modulus,
    )
    .map_or(1, |value| value + 1)
}

struct JitCheckConformanceSet {
    vm: PureFunctionResult,
    aot: PureFunctionResult,
    jit: PureFunctionResult,
    aot_matches_vm: bool,
    jit_matches_vm: bool,
}

struct JitCheckCompiledHelpers {
    aot: AotPureI64Plan,
    jit: CompiledPureI64Inputs,
    jit_batch: CompiledPureI64Batch,
    aot_compile_elapsed_ns: u128,
    jit_compile_elapsed_ns: u128,
    jit_batch_compile_elapsed_ns: u128,
}

struct JitCheckMeasurements {
    aot: JitRepeatedMeasurement,
    jit: JitRepeatedMeasurement,
    jit_batch: JitRepeatedMeasurement,
    vm: JitRepeatedMeasurement,
    julia: Option<JitJuliaMeasurement>,
}

struct JitJuliaMeasurement {
    version: String,
    elapsed: JitTimingSamples,
    accumulator: i64,
}

fn collect_jit_check_conformance(
    request: &PureFunctionRequest,
) -> Result<JitCheckConformanceSet, ExitCode> {
    let vm_backend = VmPureFunctionBackend;
    let aot = compare_pure_function_backend(&vm_backend, &AotPureFunctionBackend::new(), request)
        .map_err(|error| {
        eprintln!("error: AOT/VM conformance check failed: {error}");
        ExitCode::FAILURE
    })?;
    let jit = compare_pure_function_backend(&vm_backend, &CraneliftPureFunctionBackend, request)
        .map_err(|error| {
            eprintln!("error: JIT/VM conformance check failed: {error}");
            ExitCode::FAILURE
        })?;
    Ok(JitCheckConformanceSet {
        vm: jit.vm,
        aot: aot.candidate,
        jit: jit.candidate,
        aot_matches_vm: aot.matches_vm,
        jit_matches_vm: jit.matches_vm,
    })
}

fn compile_jit_check_helpers(
    request: &PureFunctionRequest,
    target: &JitCheckTarget,
) -> Result<JitCheckCompiledHelpers, ExitCode> {
    let aot_started = Instant::now();
    let aot = AotPureFunctionBackend::new()
        .compile_i64_with_inputs(request, target.input_names.iter().map(String::as_str))
        .map_err(|error| {
            eprintln!("error: failed to compile AOT helper: {error}");
            ExitCode::FAILURE
        })?;
    let aot_compile_elapsed_ns = aot_started.elapsed().as_nanos();

    let jit_started = Instant::now();
    let jit = CraneliftPureFunctionBackend
        .compile_i64_with_inputs(request, target.input_names.iter().map(String::as_str))
        .map_err(|error| {
            eprintln!("error: failed to compile JIT helper: {error}");
            ExitCode::FAILURE
        })?;
    let jit_compile_elapsed_ns = jit_started.elapsed().as_nanos();

    let jit_batch_started = Instant::now();
    let jit_batch = CraneliftPureFunctionBackend
        .compile_i64_batch(request, target.input_names.iter().map(String::as_str))
        .map_err(|error| {
            eprintln!("error: failed to compile JIT batch helper: {error}");
            ExitCode::FAILURE
        })?;
    let jit_batch_compile_elapsed_ns = jit_batch_started.elapsed().as_nanos();

    Ok(JitCheckCompiledHelpers {
        aot,
        jit,
        jit_batch,
        aot_compile_elapsed_ns,
        jit_compile_elapsed_ns,
        jit_batch_compile_elapsed_ns,
    })
}

fn measure_jit_check_helpers(
    options: &JitCheckOptions,
    target: &JitCheckTarget,
    compiled: &JitCheckCompiledHelpers,
) -> Result<JitCheckMeasurements, ExitCode> {
    warmup_jit_check_jit(&compiled.jit, options.warmup, options.input_seed);
    warmup_jit_check_aot(
        &compiled.aot,
        target.input_names.len(),
        options.warmup,
        options.input_seed,
    )?;
    warmup_jit_check_vm(target, options.warmup, options.input_seed)?;

    Ok(JitCheckMeasurements {
        aot: measure_jit_check_aot(
            &compiled.aot,
            target.input_names.len(),
            options.samples,
            options.iterations,
            options.input_seed,
        )?,
        jit: measure_jit_check_jit(
            &compiled.jit,
            options.samples,
            options.iterations,
            options.input_seed,
        )?,
        jit_batch: measure_jit_check_batch(
            &compiled.jit_batch,
            options.samples,
            options.iterations,
            options.input_seed,
        )?,
        vm: measure_jit_check_vm(
            target,
            options.samples,
            options.iterations,
            options.input_seed,
        )?,
        julia: options
            .julia
            .then(|| measure_jit_check_julia(target, options))
            .transpose()?,
    })
}

pub(in crate::app) struct JitCheckTarget {
    pub(in crate::app) name: String,
    source: JitCheckHelperSource,
    source_compiler: Option<JitCheckSourceCompilerReport>,
    pub(in crate::app) input_names: Vec<String>,
    pub(in crate::app) expr: RuntimeExpr,
}

#[derive(Clone, serde::Serialize)]
pub(in crate::app) struct JitCheckSourceCompilerReport {
    typecheck: TypeCheckProfileStats,
    borrow_check: BorrowCheckProfileStats,
    phases: Vec<RuntimeProfilePhase>,
}

impl From<&CheckedModule> for JitCheckSourceCompilerReport {
    fn from(checked: &CheckedModule) -> Self {
        Self {
            typecheck: TypeCheckProfileStats::from(&checked.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&checked.typecheck_report.stats),
            phases: checked.phases.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum JitCheckHelperSource {
    Builtin,
    Source,
}

impl JitCheckTarget {
    fn builtin(case: JitBuiltinCase) -> Self {
        match case {
            JitBuiltinCase::Score => Self::builtin_score(),
            JitBuiltinCase::BranchMix => Self::builtin_branch_mix(),
            JitBuiltinCase::LetChain => Self::builtin_let_chain(),
            JitBuiltinCase::FourInputMix => Self::builtin_four_input_mix(),
            JitBuiltinCase::AccumulationMix => Self::builtin_accumulation_mix(),
        }
    }

    fn builtin_score() -> Self {
        Self {
            name: "score".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: if_i64(
                binary(local("base"), RuntimeBinaryOp::Ge, int(3)),
                binary(
                    local("base"),
                    RuntimeBinaryOp::Mul,
                    RuntimeExpr::Call {
                        callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                        args: vec![local("bonus"), int(2)],
                    },
                ),
                int(0),
            ),
        }
    }

    fn builtin_branch_mix() -> Self {
        Self {
            name: "branch_mix".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec![
                "base".to_owned(),
                "bonus".to_owned(),
                "scale".to_owned(),
                "offset".to_owned(),
            ],
            expr: let_in(
                "boosted",
                binary(local("bonus"), RuntimeBinaryOp::Add, int(2)),
                let_in(
                    "weighted",
                    binary(local("base"), RuntimeBinaryOp::Mul, local("boosted")),
                    let_in(
                        "shifted",
                        binary(local("weighted"), RuntimeBinaryOp::Sub, local("offset")),
                        if_i64(
                            binary(local("shifted"), RuntimeBinaryOp::Ge, local("scale")),
                            binary(local("shifted"), RuntimeBinaryOp::Div, local("scale")),
                            RuntimeExpr::Unary {
                                op: RuntimeUnaryOp::Neg,
                                expr: Box::new(local("shifted")),
                            },
                        ),
                    ),
                ),
            ),
        }
    }

    fn builtin_let_chain() -> Self {
        Self {
            name: "let_chain".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec!["a".to_owned(), "b".to_owned(), "c".to_owned()],
            expr: let_in(
                "x",
                binary(local("a"), RuntimeBinaryOp::Mul, local("b")),
                let_in(
                    "y",
                    binary(local("x"), RuntimeBinaryOp::Add, local("c")),
                    let_in(
                        "z",
                        binary(local("y"), RuntimeBinaryOp::Sub, local("a")),
                        if_i64(
                            binary(local("z"), RuntimeBinaryOp::Gt, local("b")),
                            binary(local("z"), RuntimeBinaryOp::Mul, int(3)),
                            binary(local("z"), RuntimeBinaryOp::Add, local("b")),
                        ),
                    ),
                ),
            ),
        }
    }

    fn builtin_four_input_mix() -> Self {
        Self {
            name: "four_input_mix".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned(),
            ],
            expr: let_in(
                "left",
                binary(
                    binary(local("a"), RuntimeBinaryOp::Add, local("b")),
                    RuntimeBinaryOp::Mul,
                    binary(local("c"), RuntimeBinaryOp::Sub, local("d")),
                ),
                let_in(
                    "right",
                    binary(
                        binary(local("c"), RuntimeBinaryOp::Add, int(3)),
                        RuntimeBinaryOp::Mul,
                        binary(local("d"), RuntimeBinaryOp::Add, int(1)),
                    ),
                    if_i64(
                        binary(local("left"), RuntimeBinaryOp::Ne, local("right")),
                        binary(local("left"), RuntimeBinaryOp::Sub, local("right")),
                        binary(local("left"), RuntimeBinaryOp::Add, local("right")),
                    ),
                ),
            ),
        }
    }

    fn builtin_accumulation_mix() -> Self {
        let pair_ab = binary(local("a"), RuntimeBinaryOp::Mul, local("b"));
        let pair_cd = binary(local("c"), RuntimeBinaryOp::Mul, local("d"));
        Self {
            name: "accumulation_mix".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned(),
            ],
            expr: let_in(
                "sum0",
                binary(pair_ab.clone(), RuntimeBinaryOp::Add, pair_cd.clone()),
                let_in(
                    "sum1",
                    binary(
                        binary(local("sum0"), RuntimeBinaryOp::Add, local("a")),
                        RuntimeBinaryOp::Sub,
                        local("d"),
                    ),
                    let_in(
                        "sum2",
                        binary(
                            binary(local("sum1"), RuntimeBinaryOp::Mul, int(3)),
                            RuntimeBinaryOp::Add,
                            binary(local("b"), RuntimeBinaryOp::Mul, local("c")),
                        ),
                        let_in(
                            "sum3",
                            binary(
                                binary(local("sum2"), RuntimeBinaryOp::Sub, pair_ab),
                                RuntimeBinaryOp::Add,
                                pair_cd,
                            ),
                            binary(
                                binary(local("sum3"), RuntimeBinaryOp::Add, local("sum2")),
                                RuntimeBinaryOp::Sub,
                                local("sum1"),
                            ),
                        ),
                    ),
                ),
            ),
        }
    }

    pub(in crate::app) fn from_candidate(
        candidate: &PureHelperCandidate,
        source_compiler: Option<JitCheckSourceCompilerReport>,
    ) -> Result<Self, ExitCode> {
        let input_names = candidate.input_names().to_vec();
        if input_names.len() > 4 {
            eprintln!(
                "error: pure helper `{}` has {} input(s); current JIT check supports at most 4",
                candidate.name(),
                input_names.len()
            );
            return Err(ExitCode::from(2));
        }
        Ok(Self {
            name: candidate.name().to_owned(),
            source: JitCheckHelperSource::Source,
            source_compiler,
            input_names,
            expr: candidate.expr().clone(),
        })
    }

    fn request_with_inputs(&self, inputs: &[i64]) -> PureFunctionRequest {
        PureFunctionRequest::new(
            self.name.clone(),
            self.expr.clone(),
            self.input_names
                .iter()
                .cloned()
                .zip(inputs.iter().copied())
                .map(|(name, value)| RuntimeBinding {
                    name,
                    value: RuntimeValue::i64(value),
                }),
        )
    }

    fn runtime_helper(&self) -> RuntimePureHelper {
        RuntimePureHelper {
            id: RuntimePureHelperId(0),
            name: self.name.clone(),
            input_names: self.input_names.clone(),
            input_types: vec![RuntimePureInputType::I64; self.input_names.len()],
            output_type: RuntimePureOutputType::I64,
            expr: self.expr.clone(),
            scalar_eval_supported: self.expr.supports_scalar_pure_eval(),
            origin: RuntimePureHelperOrigin::Annotated,
        }
    }
}

impl JitCheckHelperSource {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Builtin => "builtin",
            Self::Source => "source",
        }
    }
}

fn jit_check_target(options: &JitCheckOptions) -> Result<JitCheckTarget, ExitCode> {
    options.path.as_ref().map_or_else(
        || Ok(JitCheckTarget::builtin(options.case)),
        |path| jit_check_source_target(path, options.helper.as_deref()),
    )
}

fn local(name: &str) -> RuntimeExpr {
    RuntimeExpr::Local(name.to_owned())
}

fn int(value: i64) -> RuntimeExpr {
    RuntimeExpr::Value(RuntimeValue::i64(value))
}

fn binary(lhs: RuntimeExpr, op: RuntimeBinaryOp, rhs: RuntimeExpr) -> RuntimeExpr {
    RuntimeExpr::Binary {
        lhs: Box::new(lhs),
        op,
        rhs: Box::new(rhs),
    }
}

fn let_in(name: &str, expr: RuntimeExpr, body: RuntimeExpr) -> RuntimeExpr {
    RuntimeExpr::Let {
        name: name.to_owned(),
        expr: Box::new(expr),
        body: Box::new(body),
    }
}

fn if_i64(condition: RuntimeExpr, then_expr: RuntimeExpr, else_expr: RuntimeExpr) -> RuntimeExpr {
    RuntimeExpr::If {
        condition: Box::new(condition),
        then_expr: Box::new(then_expr),
        else_expr: Box::new(else_expr),
    }
}

fn jit_check_source_target(
    path: &Path,
    helper_name: Option<&str>,
) -> Result<JitCheckTarget, ExitCode> {
    let checked = load_and_check_with_env(path, &TypeCheckEnv::standard(), Vec::new())?;
    let pure_report = lower_source_pure_helper_candidates(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {error}");
        }
        ExitCode::FAILURE
    })?;
    let candidate = select_jit_helper_candidate(&pure_report.candidates, helper_name)?;
    JitCheckTarget::from_candidate(
        candidate,
        Some(JitCheckSourceCompilerReport::from(&checked)),
    )
}

fn select_jit_helper_candidate<'a>(
    candidates: &'a [PureHelperCandidate],
    helper_name: Option<&str>,
) -> Result<&'a PureHelperCandidate, ExitCode> {
    if let Some(name) = helper_name {
        return candidates
            .iter()
            .find(|candidate| candidate.name() == name)
            .ok_or_else(|| {
                eprintln!("error: pure helper `{name}` was not found");
                ExitCode::FAILURE
            });
    }
    match candidates {
        [candidate] => Ok(candidate),
        [] => {
            eprintln!("error: no `#[pure] fn` helper candidates were found");
            Err(ExitCode::FAILURE)
        }
        _ => {
            eprintln!("error: multiple `#[pure] fn` helper candidates found; pass --helper NAME");
            Err(ExitCode::from(2))
        }
    }
}

fn warmup_jit_check_jit(compiled: &CompiledPureI64Inputs, warmup: usize, input_seed: u64) {
    let arity = compiled.param_names().len();
    for index in 0..warmup {
        let inputs = jit_check_input_array(input_seed, 0, index, arity);
        let _ = compiled.call_i64_args(RuntimeI64Args::new(inputs, arity));
    }
}

fn measure_jit_check_jit(
    compiled: &CompiledPureI64Inputs,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    let arity = compiled.param_names().len();
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_input_array(input_seed, sample, index, arity);
        compiled
            .call_i64_args(RuntimeI64Args::new(inputs, arity))
            .map_err(|error| {
                eprintln!("error: JIT evaluation failed: {error}");
                ExitCode::FAILURE
            })
    })
}

fn measure_jit_check_batch(
    compiled: &CompiledPureI64Batch,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    measure_repeated_samples(samples, |sample| {
        compiled
            .call(input_seed, sample, iterations)
            .map_err(|error| {
                eprintln!("error: JIT batch evaluation failed: {error}");
                ExitCode::FAILURE
            })
    })
}

fn warmup_jit_check_aot(
    compiled: &AotPureI64Plan,
    arity: usize,
    warmup: usize,
    input_seed: u64,
) -> Result<(), ExitCode> {
    let mut slots = Vec::new();
    for index in 0..warmup {
        let inputs = jit_check_input_array(input_seed, 0, index, arity);
        let _ = compiled
            .call_with_inputs_scratch(&inputs[..arity], &mut slots)
            .map_err(|error| {
                eprintln!("error: AOT warmup failed: {error}");
                ExitCode::FAILURE
            })?;
    }
    Ok(())
}

fn measure_jit_check_aot(
    compiled: &AotPureI64Plan,
    arity: usize,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    let mut slots = Vec::new();
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_input_array(input_seed, sample, index, arity);
        compiled
            .call_with_inputs_scratch(&inputs[..arity], &mut slots)
            .map(|(value, _stats)| value)
            .map_err(|error| {
                eprintln!("error: AOT evaluation failed: {error}");
                ExitCode::FAILURE
            })
    })
}

fn warmup_jit_check_vm(
    target: &JitCheckTarget,
    warmup: usize,
    input_seed: u64,
) -> Result<(), ExitCode> {
    let helper = target.runtime_helper();
    let mut scratch = VmPureFunctionScratch::default();
    for index in 0..warmup {
        let inputs = jit_check_input_array(input_seed, 0, index, target.input_names.len());
        let _ = scratch
            .evaluate_i64_slice(&helper, &inputs[..target.input_names.len()])
            .map_err(|error| {
                eprintln!("error: VM warmup failed: {error}");
                ExitCode::FAILURE
            })?;
    }
    Ok(())
}

fn measure_jit_check_vm(
    target: &JitCheckTarget,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    let helper = target.runtime_helper();
    let mut scratch = VmPureFunctionScratch::default();
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_input_array(input_seed, sample, index, target.input_names.len());
        let value = scratch
            .evaluate_i64_slice(&helper, &inputs[..target.input_names.len()])
            .map_err(|error| {
                eprintln!("error: VM evaluation failed: {error}");
                ExitCode::FAILURE
            })?;
        if let RuntimeValue::Int(value) = value {
            Ok(value.exact_i64().unwrap_or(0))
        } else {
            Ok(0)
        }
    })
}

fn measure_jit_check_julia(
    target: &JitCheckTarget,
    options: &JitCheckOptions,
) -> Result<JitJuliaMeasurement, ExitCode> {
    let code = julia_benchmark_source(target, options)?;
    let output = Command::new("julia")
        .arg("--startup-file=no")
        .arg("--history-file=no")
        .arg("-e")
        .arg(code)
        .output()
        .map_err(|error| {
            eprintln!("error: failed to run Julia baseline: {error}");
            ExitCode::FAILURE
        })?;
    if !output.status.success() {
        eprintln!(
            "error: Julia baseline failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(ExitCode::FAILURE);
    }
    parse_julia_measurement(&String::from_utf8_lossy(&output.stdout))
}

fn parse_julia_measurement(stdout: &str) -> Result<JitJuliaMeasurement, ExitCode> {
    let mut version = None;
    let mut accumulator = None;
    let mut min = None;
    let mut median = None;
    let mut max = None;
    for line in stdout.lines() {
        let Some((key, value)) = line.split_once('\t') else {
            continue;
        };
        match key {
            "version" => version = Some(value.to_owned()),
            "accumulator" => accumulator = value.parse::<i64>().ok(),
            "min_ns" => min = value.parse::<u128>().ok(),
            "median_ns" => median = value.parse::<u128>().ok(),
            "max_ns" => max = value.parse::<u128>().ok(),
            _ => {}
        }
    }
    let Some(version) = version else {
        eprintln!("error: Julia baseline did not report a version");
        return Err(ExitCode::FAILURE);
    };
    let Some(accumulator) = accumulator else {
        eprintln!("error: Julia baseline did not report an accumulator");
        return Err(ExitCode::FAILURE);
    };
    let Some(min) = min else {
        eprintln!("error: Julia baseline did not report min_ns");
        return Err(ExitCode::FAILURE);
    };
    let Some(median) = median else {
        eprintln!("error: Julia baseline did not report median_ns");
        return Err(ExitCode::FAILURE);
    };
    let Some(max) = max else {
        eprintln!("error: Julia baseline did not report max_ns");
        return Err(ExitCode::FAILURE);
    };
    Ok(JitJuliaMeasurement {
        version,
        elapsed: JitTimingSamples { min, median, max },
        accumulator,
    })
}

fn julia_benchmark_source(
    target: &JitCheckTarget,
    options: &JitCheckOptions,
) -> Result<String, ExitCode> {
    let params = target
        .input_names
        .iter()
        .map(|name| julia_identifier(name))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|message| {
            eprintln!("error: {message}");
            ExitCode::from(2)
        })?;
    let expr = julia_i64_expr(&target.expr).map_err(|message| {
        eprintln!(
            "error: Julia baseline cannot lower helper `{}`: {message}",
            target.name
        );
        ExitCode::from(2)
    })?;
    let call_args = (1..=params.len())
        .map(|index| format!("arcweft_input(seed, sample, iteration, {index})"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        r#"
function arcweft_score({params})::Int64
    return {expr}
end

function arcweft_input(seed::UInt64, sample::Int, iteration::Int, index::Int)::Int64
    zero_based = UInt64(index - 1)
    modulus = UInt64(5) + zero_based % UInt64(5)
    value = (seed * UInt64(index) + UInt64(sample) * (UInt64(3) + zero_based) + UInt64(iteration)) % modulus
    return Int64(value) + Int64(1)
end

seed = UInt64({seed})
warmup = {warmup}
iterations = {iterations}
samples = {samples}

function arcweft_run(seed::UInt64, warmup::Int, iterations::Int, samples::Int)
    accumulator = Int64(0)
    sample = 0
    for iteration in 0:(warmup - 1)
        arcweft_score({call_args})
    end

    elapsed = Vector{{UInt128}}(undef, samples)
    for sample in 0:(samples - 1)
        started = UInt128(time_ns())
        for iteration in 0:(iterations - 1)
            accumulator += arcweft_score({call_args})
        end
        elapsed[sample + 1] = UInt128(time_ns()) - started
    end
    sort!(elapsed)
    return accumulator, elapsed
end

arcweft_run(seed, warmup, 1, 1)
accumulator, elapsed = arcweft_run(seed, warmup, iterations, samples)
println("version\t", string(VERSION))
println("accumulator\t", accumulator)
println("min_ns\t", elapsed[1])
println("median_ns\t", elapsed[(length(elapsed) ÷ 2) + 1])
println("max_ns\t", elapsed[end])
"#,
        params = params
            .iter()
            .map(|name| format!("{name}::Int64"))
            .collect::<Vec<_>>()
            .join(", "),
        seed = options.input_seed,
        warmup = options.warmup,
        iterations = options.iterations,
        samples = options.samples,
    ))
}

fn julia_i64_expr(expr: &RuntimeExpr) -> Result<String, String> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Int(value)) => Ok(value.to_string()),
        RuntimeExpr::Local(name) => julia_identifier(name),
        RuntimeExpr::Let { name, expr, body } => Ok(format!(
            "(let {} = {}; {} end)",
            julia_identifier(name)?,
            julia_i64_expr(expr)?,
            julia_i64_expr(body)?
        )),
        RuntimeExpr::Call { callee, args }
            if callee.as_intrinsic() == Some(RuntimeIntrinsic::Add) && args.len() == 2 =>
        {
            Ok(format!(
                "(({}) + ({}))",
                julia_i64_expr(&args[0])?,
                julia_i64_expr(&args[1])?
            ))
        }
        RuntimeExpr::Unary {
            op: RuntimeUnaryOp::Neg,
            expr,
        } => Ok(format!("(-({}))", julia_i64_expr(expr)?)),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            let lhs = julia_i64_expr(lhs)?;
            let rhs = julia_i64_expr(rhs)?;
            match op {
                RuntimeBinaryOp::Add => Ok(format!("(({lhs}) + ({rhs}))")),
                RuntimeBinaryOp::Sub => Ok(format!("(({lhs}) - ({rhs}))")),
                RuntimeBinaryOp::Mul => Ok(format!("(({lhs}) * ({rhs}))")),
                RuntimeBinaryOp::Div => Ok(format!("div(({lhs}), ({rhs}))")),
                _ => Err(format!(
                    "binary operator `{op:?}` is not an i64 Julia result"
                )),
            }
        }
        RuntimeExpr::If {
            condition,
            then_expr,
            else_expr,
        } => Ok(format!(
            "(({}) ? ({}) : ({}))",
            julia_bool_expr(condition)?,
            julia_i64_expr(then_expr)?,
            julia_i64_expr(else_expr)?
        )),
        other => Err(format!(
            "expression `{other:?}` is outside the Julia baseline subset"
        )),
    }
}

fn julia_bool_expr(expr: &RuntimeExpr) -> Result<String, String> {
    match expr {
        RuntimeExpr::Value(RuntimeValue::Bool(value)) => Ok(value.to_string()),
        RuntimeExpr::Binary { lhs, op, rhs } => {
            let lhs = julia_i64_expr(lhs)?;
            let rhs = julia_i64_expr(rhs)?;
            match op {
                RuntimeBinaryOp::Eq => Ok(format!("(({lhs}) == ({rhs}))")),
                RuntimeBinaryOp::Ne => Ok(format!("(({lhs}) != ({rhs}))")),
                RuntimeBinaryOp::Lt => Ok(format!("(({lhs}) < ({rhs}))")),
                RuntimeBinaryOp::Le => Ok(format!("(({lhs}) <= ({rhs}))")),
                RuntimeBinaryOp::Gt => Ok(format!("(({lhs}) > ({rhs}))")),
                RuntimeBinaryOp::Ge => Ok(format!("(({lhs}) >= ({rhs}))")),
                _ => Err(format!(
                    "condition operator `{op:?}` is outside the Julia baseline subset"
                )),
            }
        }
        other => Err(format!(
            "condition `{other:?}` is outside the Julia baseline subset"
        )),
    }
}

fn julia_identifier(name: &str) -> Result<String, String> {
    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        return Err("Julia baseline input names must be non-empty".to_owned());
    };
    if !(first == '_' || first.is_ascii_alphabetic())
        || !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
    {
        return Err(format!(
            "Julia baseline input `{name}` is not a simple identifier"
        ));
    }
    Ok(name.to_owned())
}

fn measure_repeated(
    samples: usize,
    iterations: usize,
    mut call: impl FnMut(usize, usize) -> Result<i64, ExitCode>,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    if samples == 0 {
        eprintln!("error: --samples must be greater than zero");
        return Err(ExitCode::from(2));
    }
    let mut elapsed = Vec::with_capacity(samples);
    let mut accumulator = 0_i64;
    for sample in 0..samples {
        let started = Instant::now();
        for iteration in 0..iterations {
            accumulator = accumulator.saturating_add(call(sample, iteration)?);
        }
        elapsed.push(started.elapsed().as_nanos());
    }
    Ok(JitRepeatedMeasurement {
        elapsed: timing_samples(elapsed),
        accumulator,
    })
}

fn measure_repeated_samples(
    samples: usize,
    mut call: impl FnMut(usize) -> Result<i64, ExitCode>,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    if samples == 0 {
        eprintln!("error: --samples must be greater than zero");
        return Err(ExitCode::from(2));
    }
    let mut elapsed = Vec::with_capacity(samples);
    let mut accumulator = 0_i64;
    for sample in 0..samples {
        let started = Instant::now();
        accumulator = accumulator.saturating_add(call(sample)?);
        elapsed.push(started.elapsed().as_nanos());
    }
    Ok(JitRepeatedMeasurement {
        elapsed: timing_samples(elapsed),
        accumulator,
    })
}

pub(in crate::app) fn timing_samples(mut values: Vec<u128>) -> JitTimingSamples {
    values.sort_unstable();
    let len = values.len();
    JitTimingSamples {
        min: values.first().copied().unwrap_or_default(),
        median: values[len / 2],
        max: values.last().copied().unwrap_or_default(),
    }
}

pub(in crate::app) fn per_iteration_ns(elapsed_ns: u128, iterations: usize) -> u128 {
    elapsed_ns / iterations.max(1) as u128
}

pub(in crate::app) fn speedup_x(vm_elapsed_ns: u128, jit_elapsed_ns: u128) -> String {
    if jit_elapsed_ns == 0 {
        return "0.000".to_owned();
    }
    let milli = vm_elapsed_ns.saturating_mul(1000) / jit_elapsed_ns;
    format!("{}.{:03}", milli / 1000, milli % 1000)
}
