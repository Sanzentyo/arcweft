mod agent;
mod bundle;
mod commands;
pub(in crate::app) mod runtime;
mod tooling;

use self::agent::agent_command;
use self::bundle::{bundle_command, run_bundle_command};
use self::commands::{AgentCommand, BuildCommand, Cli, CliCommand, IdsCommand, JitCommand};
use self::runtime::{
    NativeRunHost, RuntimeExecutorInstance, apply_runtime_entry_selection, profile_lower_hir,
    profile_validate_hir, report_path, run_profile_phase, run_runtime_steps_with_executor,
    runtime_cli_command, runtime_plan_command, runtime_profile_command, runtime_run_command,
    runtime_serve_command, script_bench_command, script_test_command,
};
use self::tooling::{format_command, ids_command};
use crate::output::{
    AotProfileStats, BorrowCheckProfileStats, BytecodeProfileStats, CheckReport,
    RuntimeExecutorTier, RuntimePlanProfileStats, RuntimePlanReport, RuntimeProfileCompiler,
    RuntimeProfilePhase, RuntimeProfileReport, RuntimeProfileRuntime, RuntimePureCallStatsSummary,
    RuntimeRunReport, RuntimeStepRunSummary, RuntimeTypeValidationProfileStats,
    RuntimeTypeValidationReportSummary, ScriptBenchDeterministicSummary, ScriptBenchElapsedSummary,
    ScriptBenchMeasurementSummary, ScriptBenchPureHelperBatchSummary,
    ScriptBenchPureHelperDeterministicSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperRuntimeBatchSummary, ScriptBenchPureHelperStatsSummary,
    ScriptBenchPureHelperTimingSamples, ScriptBenchPureHelperTimingSummary, ScriptBenchRunReport,
    ScriptBenchRunSummary, ScriptBenchSectionRunSummary, ScriptTestFinalStatus,
    ScriptTestRunReport, ScriptTestRunSummary, ScriptTestStatus, TypeCheckProfileStats,
    VerifyTypesReport, VerifyTypesRuntimeSelfCheck, VerifyTypesVerifierSummary, flow_status_label,
};
use crate::server_adapter::{NativeHttpServerConfig, serve_native_http};
use crate::toolchain_profile::ToolchainProfileOptions;
use crate::{server_adapter, toolchain_profile};
use arcweft_adapter_context::{codec::AdapterManifestFile, manifest::AdapterManifest, standard};
use arcweft_bundle::{
    ArcweftBundle, BundleAdapterHostCall, BundleAdapterManifest, BundleLaunchKind, BundleManifest,
    BundleRuntimeSummary, BundleSource, BundleVirtualFile, BundleVirtualFileSpace,
};
use arcweft_core::aot::{AotProgram, AotProgramStats};
use arcweft_core::bytecode::{BytecodeProgram, BytecodeStats};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_core::plan::{
    FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType,
    RuntimePureOutputType, RuntimeRouteSpec,
};
use arcweft_core::step::{
    RuntimePureCallStats, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepResult, RuntimeStepStats,
};
use arcweft_core::{
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackendKind, PureFunctionRequest,
        PureFunctionResult, PureFunctionStats, RuntimeI64Args, RuntimePureCallBackend,
        VmPureFunctionBackend, VmPureFunctionScratch, compare_pure_function_backend,
    },
    value::{
        DenseSeq, RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeExpr,
        RuntimeIntrinsic, RuntimeSeq, RuntimeUnaryOp, RuntimeValue, runtime_sequence_dense_f32,
        runtime_sequence_values,
    },
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_jit_cranelift::{
    CompiledPureI64Batch, CompiledPureI64Inputs, CraneliftPureFunctionBackend,
};
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types, validate_typecheck_ready};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::resolve::{registry_from_hir, validate_hir_references};
use arcweft_lang_syntax::{
    expr::{CallArg, Expr, Literal, parse_expr},
    lint::{SyntaxLint, SyntaxLintSeverity, lint_id_policy},
    parser::parse_source,
};
use arcweft_launch::{
    LaunchKind, LaunchMathBackend, LaunchProfileManifest, LaunchPureBackend, ResolvedLaunchProfile,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
    RuntimePureWorkerCount, math::RuntimeMathBackend,
};
use arcweft_runtime_host::{
    BundleRunnerError, BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerPhase,
    BundleRunnerStepMode, BundleRunnerStepSummary, HostSystemInfo, INTERNAL_SCHEDULER_ADAPTER_ID,
    NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskBridge, NativeTaskClassCounts,
    NativeTaskStats, RuntimeExecutorMathStatsSummary, RuntimeExecutorStats, host_system_info,
    internal_scheduler_manifest, run_bundle_file_with_native_adapters, runtime_executor_stats,
};
use arcweft_runtime_plan::flow::{
    RuntimePlanLowerOptions, RuntimePlanLowerReport, RuntimePlanLowerStats,
    lower_runtime_plan_with_options, lower_runtime_plan_with_stats_and_options,
};
use arcweft_runtime_plan::line_task::{LoweredLineTaskGroup, lower_line_task_groups};
use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperLowerError, lower_pure_helper_candidates,
};
use arcweft_rust_abi::ArcweftRustManifest;
use arcweft_test::{BenchSection, ScriptBench, ScriptStep, ScriptTest, collect_script_tests};
use arcweft_tooling::{FormatOptions, ToolingEditReport, format_source, materialize_ids};
use arcweft_verify::{
    BackendKind, RuntimeTypeValidationStats, SmtBackend, VerificationMode, VerificationPolicy,
    VerificationReport, emit_smt_lib, validate_runtime_plan_types, verify_module_with_env,
};
use arcweft_verify_oxiz::OxizBackend;
use arcweft_verify_z3::ExternalZ3Backend;
use clap::{Args, Parser, ValueEnum};
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

const AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH: u32 = 1280;
const AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT: u32 = 720;

/// Runs the Arcweft CLI with the standard native adapters.
pub fn run<I, T>(args: I) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_with_native_adapters(args, &[])
}

/// Runs the Arcweft CLI and registers additional native host adapters.
pub fn run_with_native_adapters<I, T>(
    args: I,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> ExitCode
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match run_cli(Cli::parse_from(args), adapter_registrars) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

impl From<BundleRunnerExecutor> for CliRuntimeExecutorTier {
    fn from(value: BundleRunnerExecutor) -> Self {
        match value {
            BundleRunnerExecutor::BytecodeVm => Self::BytecodeVm,
            BundleRunnerExecutor::Aot => Self::Aot,
        }
    }
}

impl From<CliRuntimeExecutorTier> for BundleRunnerExecutor {
    fn from(value: CliRuntimeExecutorTier) -> Self {
        match value {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm,
            CliRuntimeExecutorTier::Aot => Self::Aot,
        }
    }
}

impl From<BundleRunnerStepMode> for CliRuntimeStepMode {
    fn from(value: BundleRunnerStepMode) -> Self {
        match value {
            BundleRunnerStepMode::OneOp => Self::OneOp,
            BundleRunnerStepMode::Drain => Self::Drain,
            BundleRunnerStepMode::Game => Self::Game,
            BundleRunnerStepMode::Server => Self::Server,
        }
    }
}

impl From<CliRuntimeStepMode> for BundleRunnerStepMode {
    fn from(value: CliRuntimeStepMode) -> Self {
        match value {
            CliRuntimeStepMode::OneOp => Self::OneOp,
            CliRuntimeStepMode::Drain => Self::Drain,
            CliRuntimeStepMode::Game => Self::Game,
            CliRuntimeStepMode::Server => Self::Server,
        }
    }
}

fn run_cli(cli: Cli, adapter_registrars: &[NativeAdapterRegistrar]) -> Result<(), ExitCode> {
    match cli.command {
        CliCommand::Check(options) => check_command(&options),
        CliCommand::Agent { command } => agent_command(command, adapter_registrars),
        CliCommand::Verify(options) => verify_command(&options),
        CliCommand::VerifyTypes(options) => verify_types_command(&options, adapter_registrars),
        CliCommand::Unsafe(options) => unsafe_command(&options),
        CliCommand::Plan(options) => runtime_plan_command(&options),
        CliCommand::Run(options) => runtime_run_command(&options, adapter_registrars),
        CliCommand::Profile(options) => runtime_profile_command(&options, adapter_registrars),
        CliCommand::Cli(options) => runtime_cli_command(&options, adapter_registrars),
        CliCommand::Serve(options) => runtime_serve_command(&options, adapter_registrars),
        CliCommand::Test(options) => script_test_command(&options, adapter_registrars),
        CliCommand::Bench(options) => script_bench_command(&options, adapter_registrars),
        CliCommand::Bundle(options) => bundle_command(&options),
        CliCommand::RunBundle(options) => run_bundle_command(&options, adapter_registrars),
        CliCommand::Build { command } => match command {
            BuildCommand::Bundle(options) => bundle_command(&options),
        },
        CliCommand::ToolchainProfile(options) => toolchain_profile::run(&options),
        CliCommand::Jit { command } => jit_command(command),
        CliCommand::Fmt(options) => format_command(&options),
        CliCommand::Ids { command } => ids_command(command),
    }
}

fn jit_command(command: JitCommand) -> Result<(), ExitCode> {
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

fn run_jit_check(
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

fn jit_check_input_array(seed: u64, sample: usize, iteration: usize, arity: usize) -> [i64; 4] {
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

struct JitCheckTarget {
    name: String,
    source: JitCheckHelperSource,
    source_compiler: Option<JitCheckSourceCompilerReport>,
    input_names: Vec<String>,
    expr: RuntimeExpr,
}

#[derive(Clone, serde::Serialize)]
struct JitCheckSourceCompilerReport {
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

    fn from_candidate(
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
    let pure_report = lower_pure_helper_candidates(&checked.hir).map_err(|errors| {
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

fn timing_samples(mut values: Vec<u128>) -> JitTimingSamples {
    values.sort_unstable();
    let len = values.len();
    JitTimingSamples {
        min: values.first().copied().unwrap_or_default(),
        median: values[len / 2],
        max: values.last().copied().unwrap_or_default(),
    }
}

fn per_iteration_ns(elapsed_ns: u128, iterations: usize) -> u128 {
    elapsed_ns / iterations.max(1) as u128
}

fn speedup_x(vm_elapsed_ns: u128, jit_elapsed_ns: u128) -> String {
    if jit_elapsed_ns == 0 {
        return "0.000".to_owned();
    }
    let milli = vm_elapsed_ns.saturating_mul(1000) / jit_elapsed_ns;
    format!("{}.{:03}", milli / 1000, milli % 1000)
}

fn check_command(options: &CheckOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut checked = load_and_check_selection(&selection, None)?;
    let report = run_profile_phase(&mut checked.phases, "verify", || {
        Ok::<arcweft_verify::VerificationReport, ExitCode>(verify_module_with_env(
            &checked.hir,
            &checked.env,
            VerificationPolicy {
                mode: VerificationMode::Dev,
                backend: BackendKind::Emit,
            },
        ))
    })?;
    if options.json {
        print_json(&CheckReport::from_checked(&checked, &report))?;
    } else {
        print_human_diagnostics(&report);
    }
    if report.has_errors() {
        return Err(ExitCode::FAILURE);
    }

    if !options.json {
        println!(
            "ok: {} ({} flow(s), {} line task group(s), {} warning(s), {} obligation(s))",
            selection.path().display(),
            checked.hir.flows().len(),
            checked.line_task_groups.len(),
            checked.syntax_warnings,
            report.obligations.len()
        );
    }
    Ok(())
}

fn verify_command(options: &VerifyOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let mut report = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: options.mode,
            backend: options.backend,
        },
    );

    if let Some(path) = options.emit_obligations.as_ref() {
        write_json(path, &report.obligations)?;
    }
    if let Some(path) = options.emit_smt.as_ref() {
        emit_smt(path, &report)?;
    }
    if matches!(options.backend, BackendKind::Oxiz | BackendKind::Z3) {
        solve_report(&mut report, options.backend, options.z3_command.as_deref());
    }
    if options.json {
        print_json(&report)?;
    } else {
        print_human_diagnostics(&report);
        println!(
            "ok: {} ({} obligation(s), {} unsafe audit(s))",
            selection.path().display(),
            report.obligations.len(),
            report.unsafe_audit_count()
        );
    }
    if report.has_errors() || report.has_solver_failures() {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn verify_types_command(
    options: &VerifyTypesOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    if options.run && options.steps == 0 {
        eprintln!("error: --steps must be greater than zero when --run is used");
        return Err(ExitCode::from(2));
    }
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut checked = load_and_check_selection(&selection, None)?;
    let runtime_plan = verify_types_runtime_plan(&mut checked, &selection, options)?;
    let runtime_type_validation =
        verify_types_runtime_type_validation(&mut checked, &runtime_plan)?;
    let verification = verify_types_semantics(&mut checked, options.mode)?;
    let runtime = verify_types_runtime_self_check(
        runtime_plan,
        &selection,
        options,
        &mut checked,
        adapter_registrars,
    )?;
    let runtime_failed = runtime
        .as_ref()
        .is_some_and(|runtime| runtime.failed || runtime.diagnostics > 0);
    let status =
        if runtime_type_validation.has_errors() || verification.has_errors() || runtime_failed {
            "failed"
        } else {
            "ok"
        };
    let report = VerifyTypesReport {
        status: status.to_owned(),
        source: report_path(selection.path()),
        syntax_warnings: checked.syntax_warnings,
        line_task_groups: checked.line_task_groups.len(),
        phases: checked.phases.clone(),
        typecheck: TypeCheckProfileStats::from(&checked.typecheck_report),
        borrow_check: BorrowCheckProfileStats::from(&checked.typecheck_report.stats),
        runtime_type_validation: RuntimeTypeValidationReportSummary {
            diagnostics: runtime_type_validation.diagnostics.len(),
            errors: runtime_type_validation
                .diagnostics
                .iter()
                .filter(|diagnostic| diagnostic.severity == arcweft_verify::Severity::Error)
                .count(),
            stats: RuntimeTypeValidationProfileStats::from(&runtime_type_validation.stats),
        },
        verifier: VerifyTypesVerifierSummary {
            diagnostics: verification.diagnostics.len(),
            obligations: verification.obligations.len(),
            unsafe_audits: verification.unsafe_audit_count(),
        },
        runtime,
    };
    if options.json {
        print_json(&report)?;
    } else {
        println!(
            "{}: {} (type_judgments={}, runtime_type_errors={}, obligations={})",
            report.status,
            report.source,
            report.typecheck.judgments,
            report.runtime_type_validation.errors,
            report.verifier.obligations
        );
    }
    if status == "ok" {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn verify_types_runtime_plan(
    checked: &mut CheckedModule,
    selection: &SourceSelection,
    options: &VerifyTypesOptions,
) -> Result<RuntimePlan, ExitCode> {
    let mut runtime_plan = run_profile_phase(&mut checked.phases, "runtime_plan_lower", || {
        let runtime_options = runtime_plan_options_for_selection(selection);
        lower_runtime_plan_with_options(&checked.hir, &runtime_options).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut runtime_plan, entry, options.flow.as_deref())?;
    Ok(runtime_plan)
}

fn verify_types_runtime_type_validation(
    checked: &mut CheckedModule,
    runtime_plan: &RuntimePlan,
) -> Result<arcweft_verify::RuntimeTypeValidationReport, ExitCode> {
    run_profile_phase(&mut checked.phases, "runtime_type_validate", || {
        Ok(validate_runtime_plan_types(
            runtime_plan,
            &checked.typecheck_report,
        ))
    })
}

fn verify_types_semantics(
    checked: &mut CheckedModule,
    mode: VerificationMode,
) -> Result<arcweft_verify::VerificationReport, ExitCode> {
    run_profile_phase(&mut checked.phases, "verify", || {
        Ok(verify_module_with_env(
            &checked.hir,
            &checked.env,
            VerificationPolicy {
                mode,
                backend: BackendKind::Emit,
            },
        ))
    })
}

fn verify_types_runtime_self_check(
    runtime_plan: RuntimePlan,
    selection: &SourceSelection,
    options: &VerifyTypesOptions,
    checked: &mut CheckedModule,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<Option<VerifyTypesRuntimeSelfCheck>, ExitCode> {
    if !options.run {
        return Ok(None);
    }
    let pure_config = runtime_pure_config_for_selection(
        selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    let mut executor = run_profile_phase(&mut checked.phases, "executor_prepare", || {
        Ok::<RuntimeExecutorInstance, ExitCode>(RuntimeExecutorInstance::new(
            runtime_plan,
            options.executor,
            pure_config,
        ))
    })?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let trace = run_profile_phase(&mut checked.phases, "run", || {
        run_runtime_steps_with_executor(
            &mut executor,
            NativeRunHost {
                source_path: Some(selection.path()),
                policy: &host_policy,
                adapter_registrars,
            },
            options.steps,
            options.runtime_mode,
            options.max_ops,
            &options.values,
        )
    })?;
    Ok(Some(VerifyTypesRuntimeSelfCheck {
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps_run: trace.steps.len(),
        final_status: flow_status_label(&trace.final_status),
        diagnostics: trace.steps.iter().map(|step| step.diagnostics.len()).sum(),
        failed: matches!(trace.final_status, FlowFiberStatus::Failed(_)),
        steps: trace.steps,
    }))
}

fn unsafe_command(options: &UnsafeOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let report = verify_module_with_env(
        &checked.hir,
        &checked.env,
        VerificationPolicy {
            mode: options.mode,
            backend: BackendKind::Emit,
        },
    );
    if options.json {
        print_json(&report.unsafe_audits)?;
    } else {
        for audit in &report.unsafe_audits {
            println!(
                "{} reason={} safety_doc={}",
                audit.id, audit.has_reason, audit.has_safety_doc
            );
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
enum SourceSelection {
    Direct { path: PathBuf },
    Profile(Box<ResolvedLaunchProfile>),
}

impl SourceSelection {
    fn path(&self) -> &Path {
        match self {
            Self::Direct { path } => path,
            Self::Profile(profile) => profile.source(),
        }
    }

    fn profile(&self) -> Option<&ResolvedLaunchProfile> {
        match self {
            Self::Direct { .. } => None,
            Self::Profile(profile) => Some(profile),
        }
    }

    fn entry(&self) -> Option<&str> {
        self.profile().and_then(ResolvedLaunchProfile::entry)
    }

    fn adapter(&self) -> Option<&str> {
        self.profile().and_then(ResolvedLaunchProfile::adapter)
    }
}

fn runtime_plan_options_for_selection(selection: &SourceSelection) -> RuntimePlanLowerOptions {
    selection
        .profile()
        .and_then(ResolvedLaunchProfile::dialogue_defaults)
        .map_or_else(RuntimePlanLowerOptions::default, |id| {
            RuntimePlanLowerOptions::default().with_dialogue_defaults(id)
        })
}

fn runtime_pure_config_for_selection(
    selection: &SourceSelection,
    backend: Option<CliRuntimePureBackend>,
    workers: Option<CliRuntimePureWorkers>,
    batch_min_len: Option<usize>,
    object_artifacts: bool,
    math_backend: Option<CliRuntimeMathBackend>,
    math_wgpu_min_elements: Option<usize>,
) -> Result<RuntimePureAcceleratorConfig, ExitCode> {
    let mut config = RuntimePureAcceleratorConfig::default();
    if let Some(profile) = selection.profile().and_then(ResolvedLaunchProfile::pure) {
        if let Some(backend) = profile.backend() {
            config.backend = launch_pure_backend_mode(backend);
        }
        if let Some(backend) = profile.math_backend() {
            config.math.backend = launch_math_backend_mode(backend);
        }
        if let Some(min_elements) = profile.math_wgpu_min_elements() {
            config.math.wgpu_min_elements = min_elements;
        }
        if let Some(workers) = profile.workers() {
            config.workers = parse_runtime_pure_workers(workers)
                .map(RuntimePureWorkerCount::from)
                .map_err(|message| {
                    eprintln!("error: invalid launch profile pure.workers: {message}");
                    ExitCode::from(2)
                })?;
        }
        if let Some(batch_min_len) = profile.batch_min_len() {
            config.batch_min_len = batch_min_len;
        }
        if let Some(object_artifacts) = profile.object_artifacts() {
            config.emit_object_artifacts = object_artifacts;
        }
    }
    if let Some(backend) = backend {
        config.backend = backend.into();
    }
    if let Some(workers) = workers {
        config.workers = workers.into();
    }
    if let Some(batch_min_len) = batch_min_len {
        config.batch_min_len = batch_min_len;
    }
    if object_artifacts {
        config.emit_object_artifacts = true;
    }
    if let Some(backend) = math_backend {
        config.math.backend = backend.into();
    }
    if let Some(min_elements) = math_wgpu_min_elements {
        config.math.wgpu_min_elements = min_elements;
    }
    Ok(config)
}

fn launch_pure_backend_mode(value: LaunchPureBackend) -> RuntimePureBackendMode {
    match value {
        LaunchPureBackend::Auto => RuntimePureBackendMode::Auto,
        LaunchPureBackend::Vm => RuntimePureBackendMode::Vm,
        LaunchPureBackend::Aot => RuntimePureBackendMode::Aot,
        LaunchPureBackend::Jit => RuntimePureBackendMode::Jit,
    }
}

fn launch_math_backend_mode(value: LaunchMathBackend) -> RuntimeMathBackend {
    match value {
        LaunchMathBackend::Auto => RuntimeMathBackend::Auto,
        LaunchMathBackend::Scalar => RuntimeMathBackend::Scalar,
        LaunchMathBackend::Glam => RuntimeMathBackend::Glam,
        LaunchMathBackend::Ndarray => RuntimeMathBackend::Ndarray,
        LaunchMathBackend::Wgpu => RuntimeMathBackend::Wgpu,
    }
}

fn resolve_source_selection(
    path: Option<&PathBuf>,
    profile: &ProfileOptions,
) -> Result<SourceSelection, ExitCode> {
    match (path, profile.profile.as_deref()) {
        (Some(_), Some(_)) => {
            eprintln!("error: source path and --profile cannot be used together");
            Err(ExitCode::from(2))
        }
        (Some(path), None) => Ok(SourceSelection::Direct { path: path.clone() }),
        (None, Some(profile_id)) => {
            let source = fs::read_to_string(&profile.manifest).map_err(|error| {
                eprintln!(
                    "error: failed to read launch manifest {}: {error}",
                    profile.manifest.display()
                );
                ExitCode::FAILURE
            })?;
            let manifest = LaunchProfileManifest::parse_toml(&source).map_err(|error| {
                eprintln!("error: {error}");
                ExitCode::FAILURE
            })?;
            let manifest_dir = profile.manifest.parent().unwrap_or_else(|| Path::new("."));
            let adapter_registry = standard::standard_registry();
            let adapter_ids = adapter_registry.adapter_ids();
            let resolved = manifest
                .resolve_profile_with_adapters(profile_id, manifest_dir, &adapter_ids)
                .map_err(|error| {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                })?;
            Ok(SourceSelection::Profile(Box::new(resolved)))
        }
        (None, None) => {
            eprintln!("error: expected .arcw source path or --profile");
            Err(ExitCode::from(2))
        }
    }
}

fn require_profile_kind(
    selection: &SourceSelection,
    expected: LaunchKind,
    command: &str,
) -> Result<(), ExitCode> {
    let Some(profile) = selection.profile() else {
        return Ok(());
    };
    if profile.kind() == expected {
        return Ok(());
    }
    eprintln!(
        "error: launch profile `{}` has kind {:?}; use an `{command}` profile for `arcw {command}`",
        profile.id().as_str(),
        profile.kind()
    );
    Err(ExitCode::from(2))
}

fn profile_listen_addr(selection: &SourceSelection) -> Result<Option<SocketAddr>, ExitCode> {
    let Some(raw) = selection.profile().and_then(ResolvedLaunchProfile::listen) else {
        return Ok(None);
    };
    raw.parse::<SocketAddr>().map(Some).map_err(|error| {
        eprintln!("error: invalid launch profile listen address `{raw}`: {error}");
        ExitCode::from(2)
    })
}

fn load_and_check_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<CheckedModule, ExitCode> {
    let mut phases = Vec::new();
    let env = typecheck_env_for_selection(selection, adapter_override, &mut phases)?;
    load_and_check_with_env(selection.path(), &env, phases)
}

fn typecheck_env_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<TypeCheckEnv, ExitCode> {
    let mut manifest = adapter_manifest_for_selection(selection, adapter_override)?;
    if adapter_override.is_none() && selection.profile().is_some() {
        let manifests = run_profile_phase(phases, "rust_metadata", || {
            rust_metadata_for_selection(selection)
        })?;
        for rust_manifest in manifests {
            manifest = manifest.with_rust_manifest(&rust_manifest);
        }
    }
    Ok(manifest.apply_to_env(TypeCheckEnv::standard()))
}

fn adapter_manifest_for_selection(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter_override.or(selection.adapter());
    let registry = adapter_registry_for_selection(selection)?;
    adapter_manifest_from_registry(&registry, adapter_id)
}

fn adapter_manifest_from_registry(
    registry: &arcweft_adapter_context::manifest::AdapterRegistry,
    adapter: Option<&str>,
) -> Result<AdapterManifest, ExitCode> {
    let adapter_id = adapter.unwrap_or(standard::SANS_IO_ADAPTER_ID);
    if let Some(manifest) = registry.get(adapter_id) {
        return Ok(manifest.clone());
    }
    eprintln!("error: unknown adapter `{adapter_id}`");
    Err(ExitCode::from(2))
}

fn native_host_policy_for_selection(
    selection: &SourceSelection,
) -> Result<HostCallPolicy, ExitCode> {
    native_host_policy_for_selection_with_adapter(selection, None)
}

fn native_host_policy_for_selection_with_adapter(
    selection: &SourceSelection,
    adapter_override: Option<&str>,
) -> Result<HostCallPolicy, ExitCode> {
    let selected = adapter_manifest_for_selection(selection, adapter_override)?;
    Ok(NativeTaskBridge::standard_cli_policy_for_manifest(
        &selected,
    ))
}

fn adapter_registry_for_selection(
    selection: &SourceSelection,
) -> Result<arcweft_adapter_context::manifest::AdapterRegistry, ExitCode> {
    let registry = standard::standard_registry();
    let Some(profile) = selection.profile() else {
        return Ok(registry);
    };
    profile
        .adapter_manifests()
        .iter()
        .try_fold(registry, |registry, path| {
            read_adapter_manifest(path).map(|manifest| registry.with_manifest(manifest))
        })
}

fn read_adapter_manifest(path: &Path) -> Result<AdapterManifest, ExitCode> {
    let source = fs::read_to_string(path).map_err(|error| {
        eprintln!(
            "error: failed to read adapter manifest {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    let file = match path.extension().and_then(|extension| extension.to_str()) {
        Some("json") => AdapterManifestFile::from_json(&source),
        _ => AdapterManifestFile::from_toml(&source),
    }
    .map_err(|error| {
        eprintln!(
            "error: failed to parse adapter manifest {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    Ok(file.into_manifest())
}

fn rust_metadata_for_selection(
    selection: &SourceSelection,
) -> Result<Vec<ArcweftRustManifest>, ExitCode> {
    let Some(profile) = selection.profile() else {
        return Ok(Vec::new());
    };
    profile
        .rust_metadata()
        .iter()
        .map(|path| {
            let source = fs::read_to_string(path).map_err(|error| {
                eprintln!(
                    "error: failed to read Rust ABI metadata {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })?;
            ArcweftRustManifest::from_json(&source).map_err(|error| {
                eprintln!(
                    "error: failed to parse Rust ABI metadata {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })
        })
        .collect()
}

pub(crate) struct CheckedModule {
    pub(crate) hir: arcweft_lang_hir::model::HirModule,
    pub(crate) env: TypeCheckEnv,
    pub(crate) syntax_warnings: usize,
    pub(crate) syntax_stats: arcweft_lang_syntax::cst::SyntaxParseStats,
    pub(crate) line_task_groups: Vec<LoweredLineTaskGroup>,
    pub(crate) typecheck_report: TypeCheckReport,
    pub(crate) phases: Vec<RuntimeProfilePhase>,
}

fn load_and_check_with_env(
    path: &Path,
    env: &TypeCheckEnv,
    mut phases: Vec<RuntimeProfilePhase>,
) -> Result<CheckedModule, ExitCode> {
    if !is_arcw_path(path) {
        eprintln!("error: {} is not an .arcw source file", path.display());
        return Err(ExitCode::from(2));
    }
    let source = run_profile_phase(&mut phases, "read_source", || {
        fs::read_to_string(path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })
    })?;

    let parsed = run_profile_phase(&mut phases, "parse", || {
        catch_unwind(AssertUnwindSafe(|| parse_source(source))).map_err(|_| {
            eprintln!("error: parser panicked while checking {}", path.display());
            ExitCode::FAILURE
        })
    })?;
    if !parsed.errors().is_empty() {
        for error in parsed.errors() {
            eprintln!("error: {}", error.message());
        }
        return Err(ExitCode::FAILURE);
    }

    let syntax_stats = parsed.syntax_stats();
    let tree = parsed.into_typed_tree();
    let lints = run_profile_phase(&mut phases, "lint", || {
        Ok::<Vec<arcweft_lang_syntax::lint::SyntaxLint>, ExitCode>(lint_id_policy(&tree))
    })?;
    for lint in &lints {
        eprintln!(
            "{}[{} {}]: {}",
            lint.severity().label(),
            lint.code().stable_code(),
            lint.code().domain_name(),
            lint.message()
        );
    }
    if has_error_lints(&lints) {
        return Err(ExitCode::FAILURE);
    }

    let hir = profile_lower_hir(&tree, &mut phases)?;

    let typecheck_report = profile_validate_hir(&hir, env, &mut phases)?;

    let line_task_groups = run_profile_phase(&mut phases, "line_task_lower", || {
        lower_line_task_groups(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;

    Ok(CheckedModule {
        hir,
        env: env.clone(),
        syntax_warnings: count_warning_lints(&lints),
        syntax_stats,
        line_task_groups,
        typecheck_report,
        phases,
    })
}

fn count_warning_lints(lints: &[SyntaxLint]) -> usize {
    lints
        .iter()
        .filter(|lint| matches!(lint.severity(), SyntaxLintSeverity::Warning))
        .count()
}

fn has_error_lints(lints: &[SyntaxLint]) -> bool {
    lints
        .iter()
        .any(|lint| matches!(lint.severity(), SyntaxLintSeverity::Error))
}

#[derive(Args, Clone, Debug)]
struct VerifyOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_parser = parse_verification_mode, default_value = "test")]
    mode: VerificationMode,
    #[arg(long, alias = "solver", value_parser = parse_backend_kind, default_value = "emit")]
    backend: BackendKind,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    emit_obligations: Option<PathBuf>,
    #[arg(long)]
    emit_smt: Option<PathBuf>,
    #[arg(long, alias = "z3-command")]
    z3_command: Option<String>,
}

#[derive(Args, Clone, Debug)]
struct VerifyTypesOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long)]
    flow: Option<String>,
    #[arg(long, value_parser = parse_verification_mode, default_value = "test")]
    mode: VerificationMode,
    #[arg(long)]
    run: bool,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    runtime_mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct UnsafeOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_parser = parse_verification_mode, default_value = "dev")]
    mode: VerificationMode,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct CheckOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct PlanOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct RuntimeRunOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, conflicts_with = "flow")]
    entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    flow: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::OneOp)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 1)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct RuntimeProfileOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, conflicts_with = "flow")]
    entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    flow: Option<String>,
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct CliRunOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
    #[arg(last = true)]
    args: Vec<String>,
}

#[derive(Args, Clone, Debug)]
struct ServeOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long)]
    entry: Option<String>,
    #[arg(long)]
    adapter: Option<String>,
    #[arg(long)]
    listen: Option<SocketAddr>,
    #[arg(long)]
    once: bool,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 128)]
    max_ops: usize,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct ScriptTestOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 32)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct ScriptBenchOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[arg(long)]
    pure_object_artifacts: bool,
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[arg(long, default_value_t = 32)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long, default_value_t = 1)]
    iterations: usize,
    #[arg(long, default_value_t = 0)]
    warmup: usize,
    #[arg(long, default_value_t = 5)]
    samples: usize,
    #[arg(long, default_value_t = 0)]
    input_seed: u64,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct BundleOptions {
    path: Option<PathBuf>,
    #[command(flatten)]
    profile: ProfileOptions,
    #[arg(short, long)]
    output: PathBuf,
    #[command(flatten)]
    virtual_files: BundleVirtualFileOptions,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct BundleVirtualFileOptions {
    #[arg(long)]
    include_save: bool,
    #[arg(long)]
    include_temp: bool,
    #[arg(long)]
    include_export: bool,
}

#[derive(Args, Clone, Debug)]
struct RunBundleOptions {
    bundle: PathBuf,
    #[arg(long, conflicts_with = "flow")]
    entry: Option<String>,
    #[arg(long, conflicts_with = "entry")]
    flow: Option<String>,
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[arg(long, default_value_t = 8)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 32)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long)]
    json: bool,
}

impl BundleOptions {
    fn include_spaces(&self) -> Vec<BundleVirtualFileSpace> {
        let mut spaces = vec![BundleVirtualFileSpace::Asset];
        if self.virtual_files.include_save {
            spaces.push(BundleVirtualFileSpace::Save);
        }
        if self.virtual_files.include_temp {
            spaces.push(BundleVirtualFileSpace::Temp);
        }
        if self.virtual_files.include_export {
            spaces.push(BundleVirtualFileSpace::Export);
        }
        spaces
    }
}

impl From<&RunBundleOptions> for BundleRunnerOptions {
    fn from(options: &RunBundleOptions) -> Self {
        Self {
            entry: options.entry.clone(),
            flow: options.flow.clone(),
            executor: options.executor.into(),
            steps: options.steps,
            mode: options.mode.into(),
            max_ops: options.max_ops,
            values: options.values.clone(),
            pure_config: RuntimePureAcceleratorConfig::default(),
        }
    }
}

fn bundle_launch_kind(kind: LaunchKind) -> BundleLaunchKind {
    match kind {
        LaunchKind::Game => BundleLaunchKind::Game,
        LaunchKind::Cli => BundleLaunchKind::Cli,
        LaunchKind::Server => BundleLaunchKind::Server,
        LaunchKind::Test => BundleLaunchKind::Test,
        LaunchKind::Bench => BundleLaunchKind::Bench,
    }
}

#[derive(Args, Clone, Debug)]
struct JitCheckOptions {
    path: Option<PathBuf>,
    #[arg(long)]
    helper: Option<String>,
    #[arg(long = "case", value_enum, default_value = "score")]
    case: JitBuiltinCase,
    #[arg(long)]
    julia: bool,
    #[arg(long, default_value_t = 1000)]
    iterations: usize,
    #[arg(long, default_value_t = 10)]
    warmup: usize,
    #[arg(long, default_value_t = 5)]
    samples: usize,
    #[arg(long, default_value_t = 0)]
    input_seed: u64,
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum JitBuiltinCase {
    Score,
    BranchMix,
    LetChain,
    FourInputMix,
    AccumulationMix,
}

#[derive(Args, Clone, Debug, Default)]
struct ProfileOptions {
    #[arg(long)]
    profile: Option<String>,
    #[arg(long, default_value = "arcw.toml")]
    manifest: PathBuf,
}

#[derive(Args, Clone, Debug)]
#[allow(clippy::struct_excessive_bools)]
struct ToolingCommandOptions {
    path: PathBuf,
    #[arg(long)]
    expand_sugar: bool,
    #[arg(long)]
    canonical_rich_text: bool,
    #[arg(long)]
    write: bool,
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CliRuntimeStepMode {
    OneOp,
    Drain,
    Game,
    Server,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliRuntimeExecutorTier {
    BytecodeVm,
    Aot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliRuntimePureBackend {
    Auto,
    Vm,
    Aot,
    Jit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum CliRuntimeMathBackend {
    Auto,
    Scalar,
    Glam,
    Ndarray,
    Wgpu,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CliRuntimePureWorkers {
    Auto,
    Fixed(usize),
}

impl From<CliRuntimeExecutorTier> for RuntimeExecutorTier {
    fn from(tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm,
            CliRuntimeExecutorTier::Aot => Self::Aot,
        }
    }
}

impl From<CliRuntimePureBackend> for RuntimePureBackendMode {
    fn from(value: CliRuntimePureBackend) -> Self {
        match value {
            CliRuntimePureBackend::Auto => Self::Auto,
            CliRuntimePureBackend::Vm => Self::Vm,
            CliRuntimePureBackend::Aot => Self::Aot,
            CliRuntimePureBackend::Jit => Self::Jit,
        }
    }
}

impl From<CliRuntimeMathBackend> for RuntimeMathBackend {
    fn from(value: CliRuntimeMathBackend) -> Self {
        match value {
            CliRuntimeMathBackend::Auto => Self::Auto,
            CliRuntimeMathBackend::Scalar => Self::Scalar,
            CliRuntimeMathBackend::Glam => Self::Glam,
            CliRuntimeMathBackend::Ndarray => Self::Ndarray,
            CliRuntimeMathBackend::Wgpu => Self::Wgpu,
        }
    }
}

impl From<CliRuntimePureWorkers> for RuntimePureWorkerCount {
    fn from(value: CliRuntimePureWorkers) -> Self {
        match value {
            CliRuntimePureWorkers::Auto => Self::Auto,
            CliRuntimePureWorkers::Fixed(value) => Self::Fixed(value),
        }
    }
}

#[derive(serde::Serialize)]
struct JitCheckReport {
    status: String,
    helper: String,
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
    matches_vm: bool,
    vm_value: String,
    aot_value: String,
    jit_value: String,
    warmup: usize,
    iterations: usize,
    samples: usize,
    timings: JitCheckTimingReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    julia: Option<JitCheckJuliaReport>,
    deterministic: JitCheckDeterministicReport,
    jit_batch: JitCheckBatchReport,
    vm_stats: PureFunctionStatsReport,
    aot_stats: PureFunctionStatsReport,
    jit_stats: PureFunctionStatsReport,
}

#[derive(serde::Serialize)]
struct JitCheckWorkloadReport {
    case: String,
    loop_kind: String,
    inputs_per_iteration: usize,
    batch_iterations: usize,
}

#[derive(serde::Serialize)]
struct JitCheckTimingReport {
    #[serde(rename = "aot_compile_elapsed_ns")]
    aot_compile: u128,
    #[serde(rename = "compile_elapsed_ns")]
    compile: u128,
    #[serde(rename = "aot_elapsed_ns")]
    aot: u128,
    #[serde(rename = "jit_elapsed_ns")]
    jit: u128,
    #[serde(rename = "vm_elapsed_ns")]
    vm: u128,
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
struct JitCheckBatchReport {
    backend: String,
    #[serde(rename = "compile_elapsed_ns")]
    compile: u128,
    matches_vm: bool,
    #[serde(rename = "elapsed_ns")]
    elapsed: u128,
    #[serde(rename = "per_iteration_ns")]
    per_iteration: u128,
    speedup_x: String,
    jit_call_speedup_x: String,
    samples: JitTimingSamples,
}

#[derive(Clone, Copy, Debug, serde::Serialize)]
struct JitTimingSamples {
    min: u128,
    median: u128,
    max: u128,
}

#[derive(Clone, Copy, Debug)]
struct JitRepeatedMeasurement {
    elapsed: JitTimingSamples,
    accumulator: i64,
}

#[derive(serde::Serialize)]
struct JitCheckDeterministicReport {
    #[serde(rename = "aot_accumulator")]
    aot: i64,
    #[serde(rename = "jit_accumulator")]
    jit: i64,
    #[serde(rename = "jit_batch_accumulator")]
    jit_batch: i64,
    #[serde(rename = "vm_accumulator")]
    vm: i64,
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
struct PureFunctionStatsReport {
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

#[derive(serde::Serialize)]
struct ToolingCommandReport {
    files: Vec<ToolingFileReport>,
}

#[derive(serde::Serialize)]
struct ToolingFileReport {
    path: String,
    changed: bool,
    edits: usize,
    output: Option<String>,
}

#[derive(serde::Serialize)]
struct ServePlanReport {
    status: String,
    entry: String,
    adapter: String,
    routes: Vec<ServeRouteReport>,
}

#[derive(serde::Serialize)]
struct ServeRouteReport {
    method: String,
    path: String,
    target: String,
}

#[derive(serde::Serialize)]
struct ServeRunReport {
    plan: ServePlanReport,
    server: server_adapter::NativeHttpServerReport,
}

fn parse_runtime_binding_arg(value: &str) -> Result<RuntimeBinding, String> {
    let Some((name, raw)) = value.split_once('=') else {
        return Err("expected name=value".to_owned());
    };
    if name.is_empty() {
        return Err("binding name must not be empty".to_owned());
    }
    Ok(RuntimeBinding {
        name: name.to_owned(),
        value: parse_runtime_value(raw)?,
    })
}

fn parse_runtime_value(raw: &str) -> Result<RuntimeValue, String> {
    match raw {
        "true" => Ok(RuntimeValue::Bool(true)),
        "false" => Ok(RuntimeValue::Bool(false)),
        "()" => Ok(RuntimeValue::Unit),
        value if value.starts_with("matrix/f32/") => parse_runtime_matrix_f32(value),
        value if value.starts_with("matrix/f64/") => parse_runtime_matrix_f64(value),
        value if value.starts_with("tensor/f32/") => parse_runtime_tensor_f32(value),
        value if value.starts_with("tensor/f64/") => parse_runtime_tensor_f64(value),
        value if value.starts_with("seq/f32:") => parse_runtime_f32_sequence(value),
        value if value.starts_with('@') => Ok(RuntimeValue::EntityRef(value[1..].to_owned())),
        value => value
            .parse::<i64>()
            .map(RuntimeValue::i64)
            .or_else(|_| Ok(RuntimeValue::String(value.to_owned()))),
    }
}

fn parse_runtime_matrix_f32(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("matrix/f32/")
        .split_once(':')
        .ok_or_else(|| "matrix/f32 value must be matrix/f32/<rows>x<cols>:<csv>".to_owned())?;
    let (rows, cols) = shape
        .split_once('x')
        .ok_or_else(|| "matrix/f32 shape must be <rows>x<cols>".to_owned())?;
    let rows = parse_nonzero_usize(rows, "matrix/f32 rows")?;
    let cols = parse_nonzero_usize(cols, "matrix/f32 cols")?;
    let values = parse_f32_csv(values, "matrix/f32")?;
    DenseMatrixF32::new(rows, cols, values)
        .map(RuntimeValue::MatrixF32)
        .map_err(|error| error.to_string())
}

fn parse_runtime_tensor_f32(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("tensor/f32/")
        .split_once(':')
        .ok_or_else(|| "tensor/f32 value must be tensor/f32/<dims>:<csv>".to_owned())?;
    let dims = shape
        .split('x')
        .map(|dim| parse_nonzero_usize(dim, "tensor/f32 dim"))
        .collect::<Result<Vec<_>, _>>()?;
    let values = parse_f32_csv(values, "tensor/f32")?;
    DenseTensorF32::new(dims, values)
        .map(RuntimeValue::TensorF32)
        .map_err(|error| error.to_string())
}

fn parse_runtime_matrix_f64(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("matrix/f64/")
        .split_once(':')
        .ok_or_else(|| "matrix/f64 value must be matrix/f64/<rows>x<cols>:<csv>".to_owned())?;
    let (rows, cols) = shape
        .split_once('x')
        .ok_or_else(|| "matrix/f64 shape must be <rows>x<cols>".to_owned())?;
    let rows = parse_nonzero_usize(rows, "matrix/f64 rows")?;
    let cols = parse_nonzero_usize(cols, "matrix/f64 cols")?;
    let values = parse_f64_csv(values, "matrix/f64")?;
    DenseMatrixF64::new(rows, cols, values)
        .map(RuntimeValue::MatrixF64)
        .map_err(|error| error.to_string())
}

fn parse_runtime_tensor_f64(raw: &str) -> Result<RuntimeValue, String> {
    let (shape, values) = raw
        .trim_start_matches("tensor/f64/")
        .split_once(':')
        .ok_or_else(|| "tensor/f64 value must be tensor/f64/<dims>:<csv>".to_owned())?;
    let dims = shape
        .split('x')
        .map(|dim| parse_nonzero_usize(dim, "tensor/f64 dim"))
        .collect::<Result<Vec<_>, _>>()?;
    let values = parse_f64_csv(values, "tensor/f64")?;
    DenseTensorF64::new(dims, values)
        .map(RuntimeValue::TensorF64)
        .map_err(|error| error.to_string())
}

fn parse_runtime_f32_sequence(raw: &str) -> Result<RuntimeValue, String> {
    let values = raw
        .strip_prefix("seq/f32:")
        .ok_or_else(|| "not an f32 sequence".to_owned())
        .and_then(|values| parse_f32_csv(values, "seq/f32"))?;
    Ok(runtime_sequence_dense_f32(values))
}

fn parse_nonzero_usize(raw: &str, label: &str) -> Result<usize, String> {
    let value = raw
        .parse::<usize>()
        .map_err(|_| format!("{label} must be a positive integer, got `{raw}`"))?;
    if value == 0 {
        return Err(format!("{label} must be greater than zero"));
    }
    Ok(value)
}

fn parse_f32_csv(raw: &str, label: &str) -> Result<Vec<f32>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f32>()
                .map_err(|_| format!("{label} element must be f32, got `{value}`"))
        })
        .collect()
}

fn parse_f64_csv(raw: &str, label: &str) -> Result<Vec<f64>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(|value| {
            value
                .trim()
                .parse::<f64>()
                .map_err(|_| format!("{label} element must be f64, got `{value}`"))
        })
        .collect()
}

fn parse_runtime_pure_workers(raw: &str) -> Result<CliRuntimePureWorkers, String> {
    if raw == "auto" {
        return Ok(CliRuntimePureWorkers::Auto);
    }
    let value = raw.parse::<usize>().map_err(|_| {
        format!("pure worker count must be `auto` or a positive integer, got `{raw}`")
    })?;
    if value == 0 {
        return Err("pure worker count must be greater than zero".to_owned());
    }
    Ok(CliRuntimePureWorkers::Fixed(value))
}

fn parse_verification_mode(value: &str) -> Result<VerificationMode, String> {
    match value {
        "dev" => Ok(VerificationMode::Dev),
        "test" => Ok(VerificationMode::Test),
        "release" => Ok(VerificationMode::Release),
        other => Err(format!("unknown verification mode `{other}`")),
    }
}

fn parse_backend_kind(value: &str) -> Result<BackendKind, String> {
    match value {
        "emit" => Ok(BackendKind::Emit),
        "oxiz" => Ok(BackendKind::Oxiz),
        "z3" => Ok(BackendKind::Z3),
        other => Err(format!("unknown verifier backend `{other}`")),
    }
}

fn step_options(mode: CliRuntimeStepMode, max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: mode.into(),
        budget: RuntimeStepBudget { max_ops },
    }
}

impl From<CliRuntimeStepMode> for RuntimeStepMode {
    fn from(value: CliRuntimeStepMode) -> Self {
        match value {
            CliRuntimeStepMode::OneOp => Self::OneOp,
            CliRuntimeStepMode::Drain => Self::Drain,
            CliRuntimeStepMode::Game => Self::Game,
            CliRuntimeStepMode::Server => Self::Server,
        }
    }
}

fn print_human_diagnostics(report: &VerificationReport) {
    for diagnostic in &report.diagnostics {
        eprintln!("{:?}: {}", diagnostic.severity, diagnostic.message);
    }
}

pub(crate) fn print_json<T: serde::Serialize>(value: &T) -> Result<(), ExitCode> {
    serde_json::to_writer_pretty(std::io::stdout(), value).map_err(|error| {
        eprintln!("error: failed to write JSON: {error}");
        ExitCode::FAILURE
    })?;
    println!();
    Ok(())
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) -> Result<(), ExitCode> {
    let json = serde_json::to_string_pretty(value).map_err(|error| {
        eprintln!("error: failed to encode JSON: {error}");
        ExitCode::FAILURE
    })?;
    fs::write(path, json).map_err(|error| {
        eprintln!("error: failed to write {}: {error}", path.display());
        ExitCode::FAILURE
    })
}

fn collect_arcw_paths(path: &Path) -> Result<Vec<PathBuf>, ExitCode> {
    if path.is_file() {
        if !is_arcw_path(path) {
            eprintln!("error: {} is not an .arcw source file", path.display());
            return Err(ExitCode::from(2));
        }
        return Ok(vec![path.to_path_buf()]);
    }
    if !path.is_dir() {
        eprintln!("error: {} is not a file or directory", path.display());
        return Err(ExitCode::from(2));
    }
    let mut paths = Vec::new();
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", dir.display());
            ExitCode::FAILURE
        })? {
            let entry = entry.map_err(|error| {
                eprintln!("error: failed to read directory entry: {error}");
                ExitCode::FAILURE
            })?;
            let entry_path = entry.path();
            if entry_path.is_dir() {
                stack.push(entry_path);
            } else if is_arcw_path(&entry_path) {
                paths.push(entry_path);
            }
        }
    }
    paths.sort();
    Ok(paths)
}

fn is_arcw_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "arcw")
}

fn emit_smt(path: &Path, report: &VerificationReport) -> Result<(), ExitCode> {
    fs::create_dir_all(path).map_err(|error| {
        eprintln!("error: failed to create {}: {error}", path.display());
        ExitCode::FAILURE
    })?;
    for obligation in &report.obligations {
        let Some(problem) = &obligation.smt else {
            continue;
        };
        let file = path.join(format!("{}.smt2", obligation.id));
        fs::write(&file, emit_smt_lib(problem)).map_err(|error| {
            eprintln!("error: failed to write {}: {error}", file.display());
            ExitCode::FAILURE
        })?;
    }
    Ok(())
}

fn solve_report(report: &mut VerificationReport, backend: BackendKind, z3_command: Option<&str>) {
    let checks = report
        .obligations
        .iter()
        .filter_map(|obligation| {
            obligation
                .smt
                .clone()
                .map(|problem| (obligation.id.clone(), problem))
        })
        .collect::<Vec<_>>();
    for (obligation, problem) in checks {
        let outcome = match backend {
            BackendKind::Emit => continue,
            BackendKind::Oxiz => OxizBackend.check(&problem),
            BackendKind::Z3 => {
                let backend =
                    z3_command.map_or_else(ExternalZ3Backend::default, ExternalZ3Backend::new);
                backend.check(&problem)
            }
        };
        match &outcome {
            Ok(outcome) => eprintln!("solver[{backend:?}] {obligation}: {outcome:?}"),
            Err(error) => eprintln!("solver[{backend:?}] {obligation}: {error}"),
        }
        report.record_solver_check(&obligation, backend, outcome);
    }
}
