use crate::native_system::{HostSystemInfo, host_system_info};
use crate::native_task::{
    NativeAdapterRegistrar, NativeSchedulerStats, NativeTaskBridge, NativeTaskClassCounts,
    NativeTaskStats,
};
use crate::output::{
    AotProfileStats, BorrowCheckProfileStats, BytecodeProfileStats, CheckReport,
    RuntimeExecutorMathStatsSummary, RuntimeExecutorPureAccelerationSummary,
    RuntimeExecutorPureCompileStatsSummary, RuntimeExecutorPureConfigSummary,
    RuntimeExecutorPureWorkerSummary, RuntimeExecutorStats, RuntimeExecutorTier,
    RuntimePlanProfileStats, RuntimePlanReport, RuntimeProfileCompiler, RuntimeProfilePhase,
    RuntimeProfileReport, RuntimeProfileRuntime, RuntimePureCallStatsSummary, RuntimeRunReport,
    RuntimeStepRunSummary, RuntimeTypeValidationProfileStats, RuntimeTypeValidationReportSummary,
    ScriptBenchDeterministicSummary, ScriptBenchElapsedSummary, ScriptBenchMeasurementSummary,
    ScriptBenchPureHelperBatchSummary, ScriptBenchPureHelperDeterministicSummary,
    ScriptBenchPureHelperMeasurementSummary, ScriptBenchPureHelperRuntimeBatchSummary,
    ScriptBenchPureHelperStatsSummary, ScriptBenchPureHelperTimingSamples,
    ScriptBenchPureHelperTimingSummary, ScriptBenchRunReport, ScriptBenchRunSummary,
    ScriptBenchSectionRunSummary, ScriptTestFinalStatus, ScriptTestRunReport, ScriptTestRunSummary,
    ScriptTestStatus, TypeCheckProfileStats, VerifyTypesReport, VerifyTypesRuntimeSelfCheck,
    VerifyTypesVerifierSummary, flow_status_label,
};
use crate::server_adapter::{NativeHttpServerConfig, serve_native_http};
use crate::toolchain_profile::ToolchainProfileOptions;
use crate::{server_adapter, toolchain_profile};
use arcweft_adapter_context::{codec::AdapterManifestFile, manifest::AdapterManifest, standard};
use arcweft_core::aot::{AotProgram, AotProgramStats};
use arcweft_core::bytecode::{BytecodeProgram, BytecodeStats};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::math::{DenseMatrixF32, DenseMatrixF64, DenseTensorF32, DenseTensorF64};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
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
    lint::lint_id_policy,
    parser::parse_source,
};
use arcweft_launch::{
    LaunchKind, LaunchMathBackend, LaunchProfileManifest, LaunchPureBackend, ResolvedLaunchProfile,
};
use arcweft_runtime_accelerator::{
    RuntimePureAccelerator, RuntimePureAcceleratorConfig, RuntimePureBackendMode,
    RuntimePureCompileStats, RuntimePureWorkerCount,
    math::{RuntimeMathAutoSelectionReason, RuntimeMathBackend, RuntimeMathStats},
};
use arcweft_runtime_plan::flow::{
    RuntimePlanLowerStats, lower_runtime_plan, lower_runtime_plan_with_stats,
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
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::ffi::OsString;
use std::fs;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Instant;

#[derive(Debug, Parser)]
#[command(name = "arcw", about = "Arcweft language and runtime tooling")]
struct Cli {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Check(CheckOptions),
    Verify(VerifyOptions),
    VerifyTypes(VerifyTypesOptions),
    Unsafe(UnsafeOptions),
    Plan(PlanOptions),
    Run(RuntimeRunOptions),
    Profile(RuntimeProfileOptions),
    Cli(CliRunOptions),
    Serve(ServeOptions),
    Test(ScriptTestOptions),
    Bench(ScriptBenchOptions),
    ToolchainProfile(ToolchainProfileOptions),
    Jit {
        #[command(subcommand)]
        command: JitCommand,
    },
    Fmt(ToolingCommandOptions),
    Ids {
        #[command(subcommand)]
        command: IdsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum IdsCommand {
    Materialize(ToolingCommandOptions),
}

#[derive(Debug, Subcommand)]
enum JitCommand {
    Check(JitCheckOptions),
}

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

fn run_cli(cli: Cli, adapter_registrars: &[NativeAdapterRegistrar]) -> Result<(), ExitCode> {
    match cli.command {
        CliCommand::Check(options) => check_command(&options),
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
    let checked = load_and_check_with_env(path, &TypeCheckEnv::new(), Vec::new())?;
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

fn format_command(options: &ToolingCommandOptions) -> Result<(), ExitCode> {
    run_tooling_command(options, |source| {
        format_source(
            source,
            FormatOptions {
                expand_sugar: options.expand_sugar,
            },
        )
    })
}

fn ids_command(command: IdsCommand) -> Result<(), ExitCode> {
    match command {
        IdsCommand::Materialize(options) => run_tooling_command(&options, materialize_ids),
    }
}

fn run_tooling_command(
    options: &ToolingCommandOptions,
    mut run_one: impl FnMut(&str) -> Result<ToolingEditReport, arcweft_tooling::ToolingError>,
) -> Result<(), ExitCode> {
    let paths = collect_arcw_paths(&options.path)?;
    let mut reports = Vec::new();
    for path in paths {
        let source = fs::read_to_string(&path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        })?;
        let report = run_one(&source).map_err(|error| {
            eprintln!("error: failed to edit {}: {error}", path.display());
            ExitCode::FAILURE
        })?;
        if options.write && report.changed {
            fs::write(&path, &report.output).map_err(|error| {
                eprintln!("error: failed to write {}: {error}", path.display());
                ExitCode::FAILURE
            })?;
        }
        reports.push(ToolingFileReport {
            path: path.display().to_string(),
            changed: report.changed,
            edits: report.edits.len(),
            output: if options.write {
                None
            } else {
                Some(report.output)
            },
        });
    }
    if options.json {
        print_json(&ToolingCommandReport { files: reports })
    } else {
        for report in &reports {
            println!(
                "{}: {} edit(s){}",
                report.path,
                report.edits,
                if report.changed { "" } else { " (unchanged)" }
            );
            if !options.write
                && let Some(output) = &report.output
            {
                print!("{output}");
                if !output.ends_with('\n') {
                    println!();
                }
            }
        }
        Ok(())
    }
}

fn runtime_plan_command(options: &PlanOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let checked = load_and_check_selection(&selection, None)?;
    let report = RuntimePlanReport::from_checked(&checked);
    if options.json {
        print_json(&report)
    } else {
        for line in &report.lines {
            println!(
                "{} {} {} task_node={} child_task(s)={} effect(s)={}",
                line.flow_id.as_deref().unwrap_or("-"),
                line.line_id.as_deref().unwrap_or("-"),
                line.callee,
                line.root.kind,
                line.child_tasks,
                line.effects
            );
        }
        println!(
            "ok: {} ({} line task group(s), {} verifier obligation(s))",
            selection.path().display(),
            report.lines.len(),
            report.verifier_obligations
        );
        Ok(())
    }
}

fn runtime_run_command(
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    if let Some(profile) = selection.profile() {
        match profile.kind() {
            LaunchKind::Server => {
                return runtime_serve_selection(
                    &selection,
                    options.entry.as_deref(),
                    None,
                    RuntimeServeSelectionConfig {
                        listen: None,
                        once: false,
                        max_ops: options.max_ops,
                        pure_config,
                        json: options.json,
                    },
                    adapter_registrars,
                );
            }
            LaunchKind::Test => {
                return script_test_selection(
                    &selection,
                    runtime_step_run_config_from_run_options(options, pure_config),
                    adapter_registrars,
                    &options.values,
                    options.json,
                );
            }
            LaunchKind::Bench => {
                return runtime_run_bench_selection(&selection, options, adapter_registrars);
            }
            LaunchKind::Game | LaunchKind::Cli => {}
        }
    }

    let checked = load_and_check_selection(&selection, None)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let mut plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let trace = run_runtime_steps(
        plan,
        Some(selection.path()),
        runtime_step_run_config_from_run_options(options, pure_config),
        &host_policy,
        adapter_registrars,
        &options.values,
    )?;
    let report = RuntimeRunReport {
        host_system: host_system_info(),
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: flow_status_label(&trace.final_status),
    };
    if options.json {
        print_json(&report)
    } else {
        for step in &report.steps {
            println!(
                "step {}: {} flow event(s), {} effect(s), {} task request(s), {} diagnostic(s)",
                step.index,
                step.flow_events.len(),
                step.line_effects.len(),
                step.task_requests.len(),
                step.diagnostics.len()
            );
            for event in &step.flow_events {
                println!("  event {event}");
            }
            for effect in &step.line_effects {
                println!("  effect {effect}");
            }
        }
        println!(
            "ok: {} ({} step(s), final_status={})",
            selection.path().display(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}

fn runtime_run_bench_selection(
    selection: &SourceSelection,
    options: &RuntimeRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let bench_options = ScriptBenchOptions {
        path: None,
        profile: ProfileOptions::default(),
        steps: options.steps,
        mode: options.mode,
        max_ops: options.max_ops,
        iterations: 1,
        warmup: 0,
        samples: 5,
        input_seed: 0,
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        values: options.values.clone(),
        json: options.json,
    };
    script_bench_selection(selection, &bench_options, adapter_registrars)
}

fn runtime_profile_command(
    options: &RuntimeProfileOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut phases = Vec::new();
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    let env = typecheck_env_for_selection(&selection, options.adapter.as_deref(), &mut phases)?;
    let host_policy =
        native_host_policy_for_selection_with_adapter(&selection, options.adapter.as_deref())?;
    if !is_arcw_path(selection.path()) {
        eprintln!(
            "error: {} is not an .arcw source file",
            selection.path().display()
        );
        return Err(ExitCode::from(2));
    }

    let compiled = compile_profile_runtime_plan(&selection, &env, &mut phases)?;
    let mut plan = compiled.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let mut executor = run_profile_phase(&mut phases, "executor_prepare", || {
        Ok::<RuntimeExecutorInstance, ExitCode>(RuntimeExecutorInstance::new(
            plan,
            options.executor,
            pure_config,
        ))
    })?;
    let trace = run_profile_phase(&mut phases, "run", || {
        run_runtime_steps_with_executor(
            &mut executor,
            NativeRunHost {
                source_path: Some(selection.path()),
                policy: &host_policy,
                adapter_registrars,
            },
            options.steps,
            options.mode,
            options.max_ops,
            &options.values,
        )
    })?;
    let final_status = flow_status_label(&trace.final_status);
    let report = RuntimeProfileReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            syntax: compiled.syntax_stats.into(),
            typecheck: TypeCheckProfileStats::from(&compiled.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&compiled.typecheck_report.stats),
            runtime_plan: RuntimePlanProfileStats::from(compiled.runtime_plan_stats),
            runtime_type_validation: RuntimeTypeValidationProfileStats::from(
                &compiled.runtime_type_validation_stats,
            ),
            bytecode: BytecodeProfileStats::from(&compiled.bytecode_stats),
            aot: AotProfileStats::from(&compiled.aot_stats),
        },
        phases,
        runtime: RuntimeProfileRuntime {
            host_system: host_system_info(),
            executor: RuntimeExecutorTier::from(options.executor),
            executor_stats: trace.executor_stats,
            native_io: trace.native_io,
            steps: trace.steps,
            final_status,
        },
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} phase(s), {} step(s), final_status={})",
            report.source,
            report.phases.len(),
            report.runtime.steps.len(),
            report.runtime.final_status
        );
        for phase in &report.phases {
            println!("  phase {} {}ns", phase.name, phase.elapsed_ns);
        }
        Ok(())
    }
}

struct ProfileCompiledRuntimePlan {
    hir: arcweft_lang_hir::model::HirModule,
    plan: RuntimePlan,
    syntax_warnings: usize,
    syntax_stats: arcweft_lang_syntax::cst::SyntaxParseStats,
    line_task_groups: usize,
    typecheck_report: TypeCheckReport,
    runtime_plan_stats: RuntimePlanLowerStats,
    runtime_type_validation_stats: RuntimeTypeValidationStats,
    bytecode_stats: BytecodeStats,
    aot_stats: AotProgramStats,
}

fn compile_profile_runtime_plan(
    selection: &SourceSelection,
    env: &TypeCheckEnv,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ProfileCompiledRuntimePlan, ExitCode> {
    let source = run_profile_phase(phases, "read_source", || {
        fs::read_to_string(selection.path()).map_err(|error| {
            eprintln!(
                "error: failed to read {}: {error}",
                selection.path().display()
            );
            ExitCode::FAILURE
        })
    })?;
    let parsed = run_profile_phase(phases, "parse", || {
        catch_unwind(AssertUnwindSafe(|| parse_source(source))).map_err(|_| {
            eprintln!(
                "error: parser panicked while profiling {}",
                selection.path().display()
            );
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
    let syntax_warnings = run_profile_phase(phases, "lint", || {
        Ok::<usize, ExitCode>(lint_id_policy(&tree).len())
    })?;
    let hir = profile_lower_hir(&tree, phases)?;
    let typecheck_report = profile_validate_hir(&hir, env, phases)?;
    let line_task_groups = run_profile_phase(phases, "line_task_lower", || {
        lower_line_task_groups(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    let runtime_plan_report = run_profile_phase(phases, "runtime_plan_lower", || {
        lower_runtime_plan_with_stats(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    let plan = runtime_plan_report.plan;
    let runtime_plan_stats = runtime_plan_report.stats;
    let runtime_type_validation_stats = run_profile_phase(phases, "runtime_type_validate", || {
        let report = validate_runtime_plan_types(&plan, &typecheck_report);
        if report.has_errors() {
            for diagnostic in report.diagnostics {
                eprintln!("error: {}: {}", diagnostic.path, diagnostic.message);
            }
            Err(ExitCode::FAILURE)
        } else {
            Ok(report.stats)
        }
    })?;
    let aot = run_profile_phase(phases, "aot_lower", || {
        Ok::<AotProgram, ExitCode>(AotProgram::from_runtime_plan(&plan))
    })?;
    let aot_stats = aot.stats().clone();
    let bytecode = run_profile_phase(phases, "bytecode_lower", || {
        Ok::<BytecodeProgram, ExitCode>(BytecodeProgram::from_runtime_plan(plan))
    })?;
    let bytecode_stats = bytecode.stats();
    let plan = bytecode.into_runtime_plan().map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    Ok(ProfileCompiledRuntimePlan {
        hir,
        plan,
        syntax_warnings,
        syntax_stats,
        line_task_groups: line_task_groups.len(),
        typecheck_report,
        runtime_plan_stats,
        runtime_type_validation_stats,
        bytecode_stats,
        aot_stats,
    })
}

fn profile_lower_hir(
    tree: &arcweft_lang_syntax::ast::items::TypedSyntaxTree,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<arcweft_lang_hir::model::HirModule, ExitCode> {
    run_profile_phase(phases, "lower_hir", || {
        lower_to_hir(tree).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })
}

fn profile_validate_hir(
    hir: &arcweft_lang_hir::model::HirModule,
    env: &TypeCheckEnv,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<TypeCheckReport, ExitCode> {
    run_profile_phase(phases, "resolve", || {
        let registry = registry_from_hir(hir);
        validate_hir_references(hir, &registry).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(phases, "readiness", || {
        validate_typecheck_ready(hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
    run_profile_phase(phases, "typecheck", || {
        let report = analyze_types(hir, env);
        if report.diagnostics.is_empty() {
            Ok(report)
        } else {
            for error in report.diagnostics {
                eprintln!("error: {}", error.message());
            }
            Err(ExitCode::FAILURE)
        }
    })
}

fn run_runtime_steps(
    plan: RuntimePlan,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, ExitCode> {
    let mut executor = RuntimeExecutorInstance::new(plan, config.executor, config.pure_config);
    run_runtime_steps_with_executor(
        &mut executor,
        NativeRunHost {
            source_path,
            policy: host_policy,
            adapter_registrars,
        },
        config.steps,
        config.mode,
        config.max_ops,
        values,
    )
}

fn run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, ExitCode> {
    let mut host = host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })?;
    let mut task_events = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(mode, max_ops),
        );
        let (summary, task_requests) = RuntimeStepRunSummary::from_result_and_task_requests(
            step_index,
            result,
            executor.fiber(),
        );
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(RuntimeRunTrace {
        steps: summaries,
        final_status: executor.fiber().status.clone(),
        executor_stats: executor.executor_stats(),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

struct RuntimeRunTrace {
    steps: Vec<RuntimeStepRunSummary>,
    final_status: FlowFiberStatus,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

#[derive(Clone, Copy)]
struct NativeRunHost<'a> {
    source_path: Option<&'a Path>,
    policy: &'a HostCallPolicy,
    adapter_registrars: &'a [NativeAdapterRegistrar],
}

fn run_runtime_bench_steps_with_pure(
    mut executor: RuntimeExecutorCore,
    source_path: Option<&Path>,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
    pure: &mut RuntimePureAccelerator,
) -> Result<RuntimeBenchTrace, ExitCode> {
    let mut host = None;
    let mut task_events = Vec::new();
    let mut totals = RuntimeBenchStepTotals::default();
    for _ in 0..config.steps {
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            values,
            step_options(config.mode, config.max_ops),
            pure,
        );
        let RuntimeStepResult {
            mut output,
            fiber_status,
            stats,
            ..
        } = result;
        let task_requests = std::mem::take(&mut output.requests.tasks);
        totals.push(&stats, task_requests.len(), output.diagnostics.len());
        let done = matches!(
            fiber_status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if done {
            break;
        }
        if let Some(source_path) = source_path
            && !task_requests.is_empty()
        {
            if host.is_none() {
                host = Some(
                    NativeTaskBridge::try_new(source_path, host_policy.clone(), adapter_registrars)
                        .map_err(|error| {
                            eprintln!("error: {error}");
                            ExitCode::FAILURE
                        })?,
                );
            }
            if let Some(host) = host.as_mut() {
                task_events = host.complete_tasks(task_requests);
            }
        }
    }
    Ok(RuntimeBenchTrace {
        totals,
        executor_stats: runtime_executor_stats(executor.fast_path_ops(), pure),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    })
}

struct RuntimeBenchTrace {
    totals: RuntimeBenchStepTotals,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

#[derive(Default)]
struct RuntimeBenchStepTotals {
    executed_ops: usize,
    child_fiber_ticks: usize,
    max_child_fibers: usize,
    line_effects: usize,
    task_requests: usize,
    task_events_in: usize,
    diagnostics: usize,
    pure: RuntimePureCallStats,
}

impl RuntimeBenchStepTotals {
    fn push(&mut self, stats: &RuntimeStepStats, task_requests: usize, diagnostics: usize) {
        self.executed_ops += stats.executed_ops;
        self.child_fiber_ticks += stats.child_fibers;
        self.max_child_fibers = self.max_child_fibers.max(stats.child_fibers);
        self.line_effects += stats.line_effects;
        self.task_requests += task_requests;
        self.task_events_in += stats.task_events_in;
        self.diagnostics += diagnostics;
        add_pure_stats(&mut self.pure, stats.pure);
    }
}

fn add_pure_stats(total: &mut RuntimePureCallStats, stats: RuntimePureCallStats) {
    total.pure_calls += stats.pure_calls;
    total.math_calls += stats.math_calls;
    total.math_accelerated_calls += stats.math_accelerated_calls;
    total.batch_calls += stats.batch_calls;
    total.batch_items += stats.batch_items;
    total.flat_batch_calls += stats.flat_batch_calls;
    total.flat_batch_items += stats.flat_batch_items;
    total.flat_batch_bytes_borrowed += stats.flat_batch_bytes_borrowed;
    total.flatten_materializations += stats.flatten_materializations;
    total.flatten_bytes_copied += stats.flatten_bytes_copied;
    total.jit_calls += stats.jit_calls;
    total.aot_calls += stats.aot_calls;
    total.vm_calls += stats.vm_calls;
    total.arg_stack_packs += stats.arg_stack_packs;
    total.arg_vec_allocations += stats.arg_vec_allocations;
    total.arg_bytes_copied += stats.arg_bytes_copied;
    total.arg_bytes_borrowed += stats.arg_bytes_borrowed;
    total.result_bytes_copied += stats.result_bytes_copied;
    total.parallel_policy_checks += stats.parallel_policy_checks;
    total.parallel_work_units += stats.parallel_work_units;
    total.parallel_batches += stats.parallel_batches;
    total.parallel_skipped_backend += stats.parallel_skipped_backend;
    total.parallel_skipped_small += stats.parallel_skipped_small;
    total.thread_pool_jobs += stats.thread_pool_jobs;
    total.thread_pool_build_elapsed_ns += stats.thread_pool_build_elapsed_ns;
    total.fallbacks += stats.fallbacks;
}

enum RuntimeExecutorInstance {
    BytecodeVm {
        executor: BytecodeVmExecutor,
        pure: RuntimePureAccelerator,
    },
    Aot {
        executor: AotExecutor,
        pure: RuntimePureAccelerator,
    },
}

enum RuntimeExecutorCore {
    BytecodeVm(BytecodeVmExecutor),
    Aot(AotExecutor),
}

enum RuntimeExecutorTemplate {
    BytecodeVm {
        plan: RuntimePlan,
        program: BytecodeProgram,
    },
    Aot {
        plan: RuntimePlan,
        program: AotProgram,
    },
}

impl RuntimeExecutorTemplate {
    fn new(plan: &RuntimePlan, tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm {
                plan: plan.clone(),
                program: BytecodeProgram::from_runtime_plan(plan.clone()),
            },
            CliRuntimeExecutorTier::Aot => Self::Aot {
                plan: plan.clone(),
                program: AotProgram::from_runtime_plan(plan),
            },
        }
    }

    fn instantiate(&self) -> RuntimeExecutorCore {
        match self {
            Self::BytecodeVm { plan, program } => RuntimeExecutorCore::BytecodeVm(
                BytecodeVmExecutor::from_parts(program.clone(), plan.clone()),
            ),
            Self::Aot { plan, program } => {
                RuntimeExecutorCore::Aot(AotExecutor::from_parts(program.clone(), plan.clone()))
            }
        }
    }
}

impl RuntimeExecutorCore {
    fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
        pure: &mut RuntimePureAccelerator,
    ) -> RuntimeStepResult {
        match self {
            Self::BytecodeVm(executor) => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
            Self::Aot(executor) => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
        }
    }

    fn fast_path_ops(&self) -> usize {
        match self {
            Self::BytecodeVm(_) => 0,
            Self::Aot(executor) => executor.fast_path_ops(),
        }
    }
}

impl RuntimeExecutorInstance {
    fn new(
        plan: RuntimePlan,
        tier: CliRuntimeExecutorTier,
        pure_config: RuntimePureAcceleratorConfig,
    ) -> Self {
        let pure = RuntimePureAccelerator::with_config(pure_config, &plan.pure_helpers);
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm {
                executor: BytecodeVmExecutor::from_runtime_plan(plan),
                pure,
            },
            CliRuntimeExecutorTier::Aot => Self::Aot {
                executor: AotExecutor::new(plan),
                pure,
            },
        }
    }

    fn step_with_root_bindings(
        &mut self,
        input: RuntimeStepInput,
        root_bindings: &[RuntimeBinding],
        options: RuntimeStepOptions,
    ) -> RuntimeStepResult {
        match self {
            Self::BytecodeVm { executor, pure } => executor
                .step_with_root_bindings_and_pure_backend(input, root_bindings, options, pure),
            Self::Aot { executor, pure } => executor.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                pure,
            ),
        }
    }

    fn fiber(&self) -> &arcweft_core::engine::FlowFiber {
        match self {
            Self::BytecodeVm { executor, .. } => executor.fiber(),
            Self::Aot { executor, .. } => executor.fiber(),
        }
    }

    fn executor_stats(&self) -> RuntimeExecutorStats {
        match self {
            Self::BytecodeVm { pure, .. } => runtime_executor_stats(0, pure),
            Self::Aot { executor, pure } => runtime_executor_stats(executor.fast_path_ops(), pure),
        }
    }
}

fn runtime_executor_stats(
    aot_fast_path_ops: usize,
    pure: &RuntimePureAccelerator,
) -> RuntimeExecutorStats {
    let config = pure.config();
    let summary = pure.summary();
    let compile = pure.compile_stats();
    RuntimeExecutorStats {
        aot_fast_path_ops,
        pure_config: RuntimeExecutorPureConfigSummary {
            backend: runtime_pure_backend_label(config.backend),
            workers: match config.workers {
                RuntimePureWorkerCount::Auto => RuntimeExecutorPureWorkerSummary::Auto,
                RuntimePureWorkerCount::Fixed(value) => {
                    RuntimeExecutorPureWorkerSummary::Fixed(value)
                }
            },
            resolved_workers: pure.resolved_worker_count(),
            worker_pool_active: pure.has_worker_pool(),
            batch_min_len: config.batch_min_len,
            math_backend: runtime_math_backend_label(config.math.backend),
            math_wgpu_min_elements: config.math.wgpu_min_elements,
        },
        pure_acceleration: RuntimeExecutorPureAccelerationSummary {
            annotated: summary.annotated,
            inferred: summary.inferred,
            jit: summary.jit,
            aot: summary.aot,
            vm: summary.vm,
        },
        pure_compile: RuntimeExecutorPureCompileStatsSummary::from(compile),
        math: RuntimeExecutorMathStatsSummary::from(pure.math_stats()),
    }
}

fn run_profile_phase<T>(
    phases: &mut Vec<RuntimeProfilePhase>,
    name: &'static str,
    run: impl FnOnce() -> Result<T, ExitCode>,
) -> Result<T, ExitCode> {
    let started = Instant::now();
    let result = run();
    phases.push(RuntimeProfilePhase {
        name,
        elapsed_ns: started.elapsed().as_nanos(),
    });
    result
}

fn report_path(path: &Path) -> String {
    if let Ok(cwd) = std::env::current_dir()
        && let Ok(relative) = path.strip_prefix(cwd)
    {
        return relative.display().to_string();
    }
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn runtime_cli_command(
    options: &CliRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    require_profile_kind(&selection, LaunchKind::Cli, "cli")?;
    let checked = load_and_check_selection(&selection, None)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let mut plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {error}");
        }
        ExitCode::FAILURE
    })?;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_cli_entry_selection(&mut plan, entry)?;
    let mut bindings = options.values.clone();
    bindings.push(RuntimeBinding {
        name: "args".to_owned(),
        value: runtime_sequence_values(
            options
                .args
                .iter()
                .cloned()
                .map(RuntimeValue::String)
                .collect(),
        ),
    });
    bindings.push(RuntimeBinding {
        name: "argc".to_owned(),
        value: RuntimeValue::i64(i64::try_from(options.args.len()).unwrap_or(i64::MAX)),
    });

    let trace = run_runtime_steps(
        plan,
        Some(selection.path()),
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config,
        },
        &host_policy,
        adapter_registrars,
        &bindings,
    )?;
    let report = RuntimeRunReport {
        host_system: host_system_info(),
        executor: RuntimeExecutorTier::from(options.executor),
        executor_stats: trace.executor_stats,
        native_io: trace.native_io,
        steps: trace.steps,
        final_status: flow_status_label(&trace.final_status),
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} cli arg(s), {} step(s), final_status={})",
            selection.path().display(),
            options.args.len(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}

fn runtime_serve_command(
    options: &ServeOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Server, "serve")?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    runtime_serve_selection(
        &selection,
        options.entry.as_deref(),
        options.adapter.as_deref(),
        RuntimeServeSelectionConfig {
            listen: options.listen,
            once: options.once,
            max_ops: options.max_ops,
            pure_config,
            json: options.json,
        },
        adapter_registrars,
    )
}

fn runtime_serve_selection(
    selection: &SourceSelection,
    entry_override: Option<&str>,
    adapter_override: Option<&str>,
    config: RuntimeServeSelectionConfig,
    _adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let adapter = adapter_override
        .or(selection.adapter())
        .unwrap_or("sans-io");
    let checked = load_and_check_selection(selection, Some(adapter))?;
    let host_policy = native_host_policy_for_selection_with_adapter(selection, Some(adapter))?;
    let plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {error}");
        }
        ExitCode::FAILURE
    })?;
    let entry = select_server_entry(&plan, entry_override.or(selection.entry()))?;
    let routes = server_routes(entry);
    if routes.is_empty() {
        eprintln!(
            "error: server entry `{}` has no runnable routes",
            entry.id.0
        );
        return Err(ExitCode::FAILURE);
    }
    for route in &routes {
        if !plan.flows.iter().any(|flow| flow.id == route.target) {
            eprintln!(
                "error: server route {} {} targets unknown flow `{}`",
                route.method, route.path, route.target.0
            );
            return Err(ExitCode::FAILURE);
        }
    }
    let report = ServePlanReport {
        status: "planned".to_owned(),
        entry: entry.id.0.clone(),
        adapter: adapter.to_owned(),
        routes: routes
            .iter()
            .map(|route| ServeRouteReport {
                method: route.method.clone(),
                path: route.path.clone(),
                target: route.target.0.clone(),
            })
            .collect(),
    };
    let listen = match config.listen {
        Some(listen) => Some(listen),
        None => profile_listen_addr(selection)?,
    };
    if let Some(listen) = listen {
        let server_report = serve_native_http(
            &plan,
            &routes,
            &NativeHttpServerConfig {
                listen,
                once: config.once,
                max_ops: config.max_ops,
                pure_config: config.pure_config,
                host_policy,
            },
        )
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })?;
        let report = ServeRunReport {
            plan: report,
            server: server_report,
        };
        return if config.json {
            print_json(&report)
        } else {
            println!(
                "ok: served {} request(s) on {}",
                report.server.handled_requests, report.server.listen
            );
            Ok(())
        };
    }
    if config.json {
        print_json(&report)
    } else {
        for route in &report.routes {
            println!("{} {} -> {}", route.method, route.path, route.target);
        }
        println!(
            "ok: {} (server entry {}, adapter={}, {} route(s), status={})",
            selection.path().display(),
            report.entry,
            report.adapter,
            report.routes.len(),
            report.status
        );
        Ok(())
    }
}

fn apply_runtime_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
    flow: Option<&str>,
) -> Result<(), ExitCode> {
    if entry.is_some() && flow.is_some() {
        eprintln!("error: --entry and --flow are mutually exclusive");
        return Err(ExitCode::from(2));
    }
    if let Some(flow) = flow {
        let flow = FlowRuntimeId(normalize_flow_id(flow));
        if !plan.flows.iter().any(|candidate| candidate.id == flow) {
            eprintln!("error: unknown flow `{}`", flow.0);
            return Err(ExitCode::FAILURE);
        }
        plan.entry_flow = Some(flow);
        return Ok(());
    }
    if let Some(entry) = entry {
        let entry = normalize_entry_id(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            eprintln!("error: unknown entry `{entry}`");
            return Err(ExitCode::FAILURE);
        };
        let RuntimeEntryTarget::Flow(flow) = &spec.target else {
            eprintln!("error: entry `{entry}` does not select a single runnable flow");
            return Err(ExitCode::FAILURE);
        };
        plan.entry_flow = Some(flow.clone());
        return Ok(());
    }
    Ok(())
}

fn select_server_entry<'a>(
    plan: &'a RuntimePlan,
    entry: Option<&str>,
) -> Result<&'a RuntimeEntrySpec, ExitCode> {
    if let Some(entry) = entry {
        let entry = normalize_entry_id(entry);
        let Some(spec) = plan
            .entries
            .iter()
            .find(|candidate| candidate.id.0 == entry)
        else {
            eprintln!("error: unknown entry `{entry}`");
            return Err(ExitCode::FAILURE);
        };
        if spec.kind != RuntimeEntryKind::Server {
            eprintln!("error: entry `{entry}` is not a server entry");
            return Err(ExitCode::FAILURE);
        }
        return Ok(spec);
    }
    let Some(spec) = plan
        .entries
        .iter()
        .find(|candidate| candidate.kind == RuntimeEntryKind::Server)
    else {
        eprintln!("error: no server entry found; declare `entry server @entry.name`");
        return Err(ExitCode::FAILURE);
    };
    Ok(spec)
}

fn server_routes(entry: &RuntimeEntrySpec) -> Vec<RuntimeRouteSpec> {
    match &entry.target {
        RuntimeEntryTarget::Routes(routes) => routes.clone(),
        RuntimeEntryTarget::Flow(flow) => vec![RuntimeRouteSpec {
            method: "*".to_owned(),
            path: "*".to_owned(),
            target: flow.clone(),
            bindings: Vec::new(),
        }],
    }
}

fn apply_runtime_cli_entry_selection(
    plan: &mut RuntimePlan,
    entry: Option<&str>,
) -> Result<(), ExitCode> {
    if let Some(entry) = entry {
        return apply_runtime_entry_selection(plan, Some(entry), None);
    }
    let Some(spec) = plan
        .entries
        .iter()
        .find(|candidate| candidate.kind == RuntimeEntryKind::Cli)
    else {
        eprintln!("error: no cli entry found; declare `entry cli @entry.name` or pass --entry");
        return Err(ExitCode::FAILURE);
    };
    let RuntimeEntryTarget::Flow(flow) = &spec.target else {
        eprintln!(
            "error: cli entry `{}` does not select a single runnable flow",
            spec.id.0
        );
        return Err(ExitCode::FAILURE);
    };
    plan.entry_flow = Some(flow.clone());
    Ok(())
}

fn normalize_flow_id(value: &str) -> String {
    normalize_entity_selector(value, "flow")
}

fn normalize_entry_id(value: &str) -> String {
    normalize_entity_selector(value, "entry")
}

fn normalize_entity_selector(value: &str, family: &str) -> String {
    let value = value.trim().trim_start_matches('@');
    if value.contains('.') {
        value.to_owned()
    } else {
        format!("{family}.{value}")
    }
}

fn script_test_command(
    options: &ScriptTestOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Test, "test")?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    script_test_selection(
        &selection,
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config,
        },
        adapter_registrars,
        &options.values,
        options.json,
    )
}

fn script_test_selection(
    selection: &SourceSelection,
    config: RuntimeStepRunConfig,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
    json: bool,
) -> Result<(), ExitCode> {
    let checked = load_and_check_selection(selection, None)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let manifest = collect_script_tests(&checked.hir);
    let plan = lower_runtime_plan(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;
    let output = ScriptTestRunReport {
        tests: manifest
            .tests
            .iter()
            .map(|test| {
                run_script_test(
                    test,
                    &plan,
                    selection.path(),
                    config,
                    &host_policy,
                    adapter_registrars,
                    values,
                )
            })
            .collect(),
    };
    let failed = output
        .tests
        .iter()
        .any(|test| test.status == ScriptTestStatus::Failed);
    if json {
        print_json(&output)?;
    } else {
        for test in &output.tests {
            println!(
                "{} {} {} ({} step(s))",
                test.id, test.kind, test.status, test.steps_run
            );
            for diagnostic in &test.diagnostics {
                println!("  diagnostic {diagnostic}");
            }
        }
        println!(
            "ok: {} ({} script test(s))",
            selection.path().display(),
            output.tests.len()
        );
    }
    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn run_script_test(
    test: &ScriptTest,
    plan: &RuntimePlan,
    source_path: &Path,
    config: RuntimeStepRunConfig,
    host_policy: &HostCallPolicy,
    adapter_registrars: &[NativeAdapterRegistrar],
    values: &[RuntimeBinding],
) -> ScriptTestRunSummary {
    if test.kind != "scenario" {
        return ScriptTestRunSummary::skipped(
            test,
            format!(
                "headless execution for `{}` tests is not implemented",
                test.kind
            ),
        );
    }
    let Some(start) = test_start_flow(test) else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            ScriptTestFinalStatus::NotStarted,
            vec!["scenario test requires `start(@flow.id)`".to_owned()],
            Vec::new(),
        );
    };
    let mut plan = plan.clone();
    plan.entry_flow = Some(FlowRuntimeId(start));
    let Ok(trace) = run_runtime_steps(
        plan,
        Some(source_path),
        config,
        host_policy,
        adapter_registrars,
        values,
    ) else {
        return ScriptTestRunSummary::completed(
            test,
            false,
            ScriptTestFinalStatus::AdapterError,
            vec!["native adapter registration failed".to_owned()],
            Vec::new(),
        );
    };
    let final_status = flow_status_label(&trace.final_status);
    let mut diagnostics = trace
        .steps
        .iter()
        .flat_map(|step| step.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    diagnostics.extend(test_expectation_failures(test, &trace.steps));
    match trace.final_status {
        FlowFiberStatus::Done(_) => {}
        FlowFiberStatus::Failed(ref message) => {
            diagnostics.push(format!("runtime failed: {message}"));
        }
        FlowFiberStatus::Running
        | FlowFiberStatus::Waiting(_)
        | FlowFiberStatus::WaitingMany(_)
        | FlowFiberStatus::Choice(_) => diagnostics.push(format!(
            "scenario did not finish within {} step(s): {final_status}",
            config.steps
        )),
    }
    let passed = diagnostics.is_empty();
    ScriptTestRunSummary::completed(
        test,
        passed,
        ScriptTestFinalStatus::Flow(final_status),
        diagnostics,
        trace.steps,
    )
}

#[derive(Clone, Copy, Debug)]
struct RuntimeStepRunConfig {
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    executor: CliRuntimeExecutorTier,
    pure_config: RuntimePureAcceleratorConfig,
}

#[derive(Clone, Copy)]
struct BenchRuntimeContext<'a> {
    pure_config: RuntimePureAcceleratorConfig,
    host_policy: &'a HostCallPolicy,
    adapter_registrars: &'a [NativeAdapterRegistrar],
}

fn runtime_step_run_config_from_run_options(
    options: &RuntimeRunOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> RuntimeStepRunConfig {
    RuntimeStepRunConfig {
        steps: options.steps,
        mode: options.mode,
        max_ops: options.max_ops,
        executor: options.executor,
        pure_config,
    }
}

#[derive(Clone, Copy, Debug)]
struct RuntimeServeSelectionConfig {
    listen: Option<SocketAddr>,
    once: bool,
    max_ops: usize,
    pure_config: RuntimePureAcceleratorConfig,
    json: bool,
}

fn test_start_flow(test: &ScriptTest) -> Option<String> {
    test.steps
        .iter()
        .find_map(|step| parse_start_flow_call(&step.text))
}

fn test_expectation_failures(test: &ScriptTest, frames: &[RuntimeStepRunSummary]) -> Vec<String> {
    test.steps
        .iter()
        .filter(|step| step.command == "expect" || step.command.starts_with("expect."))
        .filter_map(|step| evaluate_test_expectation(step, frames).err())
        .collect()
}

fn evaluate_test_expectation(
    step: &ScriptStep,
    frames: &[RuntimeStepRunSummary],
) -> Result<(), String> {
    evaluate_runtime_expectation(step.text.trim(), &RuntimeExpectationView::new(frames))
}

struct RuntimeExpectationView<'a> {
    frames: &'a [RuntimeStepRunSummary],
    source_path: Option<&'a Path>,
}

impl<'a> RuntimeExpectationView<'a> {
    const fn new(frames: &'a [RuntimeStepRunSummary]) -> Self {
        Self {
            frames,
            source_path: None,
        }
    }

    const fn with_source_path(frames: &'a [RuntimeStepRunSummary], source_path: &'a Path) -> Self {
        Self {
            frames,
            source_path: Some(source_path),
        }
    }

    fn frames(&self) -> &[RuntimeStepRunSummary] {
        self.frames
    }

    fn signal_value(&self, target: &str) -> Option<&str> {
        self.frames
            .last()?
            .observations
            .signals
            .iter()
            .find(|signal| signal.target == target)
            .map(|signal| signal.value.as_str())
    }

    fn has_log(&self, level: &str, needle: &str) -> bool {
        self.frames.last().is_some_and(|frame| {
            frame
                .observations
                .logs
                .iter()
                .any(|log| log.level == level && log.message.contains(needle))
        })
    }

    fn file_text(&self, virtual_path: &str) -> Result<String, String> {
        let Some(source_path) = self.source_path else {
            return Err("file expectations require a source-backed runtime".to_owned());
        };
        NativeTaskBridge::read_text_snapshot(source_path, virtual_path)
    }
}

fn evaluate_runtime_expectation(
    text: &str,
    observations: &RuntimeExpectationView<'_>,
) -> Result<(), String> {
    if is_expect_no_assertion_failures_call(text) {
        if observations
            .frames()
            .iter()
            .all(|frame| frame.diagnostics.is_empty())
        {
            return Ok(());
        }
        return Err("expected no assertion/runtime diagnostics".to_owned());
    }
    if let Some((target, expected)) = parse_expect_signal_call(text) {
        let actual = observations.signal_value(&target);
        if actual == Some(expected.as_str()) {
            return Ok(());
        }
        return Err(format!(
            "expected signal {target} == {expected}, found {}",
            actual.unwrap_or("<missing>")
        ));
    }
    if let Some((level, needle)) = parse_expect_log_call(text) {
        if observations.has_log(&level, &needle) {
            return Ok(());
        }
        return Err(format!("expected log.{level} containing `{needle}`"));
    }
    if let Some((virtual_path, expected)) = parse_expect_file_call(text) {
        let actual = observations.file_text(&virtual_path)?;
        if actual == expected {
            return Ok(());
        }
        return Err(format!(
            "expected file {virtual_path} == `{expected}`, found `{actual}`"
        ));
    }
    Err(format!("unsupported runtime expectation `{text}`"))
}

fn parse_start_flow_call(text: &str) -> Option<String> {
    let Expr::Call { callee, args } = parse_expr(text).ok()? else {
        return None;
    };
    let Expr::Path(name) = callee.as_ref() else {
        return None;
    };
    if name != "start" {
        return None;
    }
    let [flow] = args.as_slice() else {
        return None;
    };
    entity_ref_label(flow.value())
}

fn parse_expect_signal_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "signal" {
        return None;
    }
    let [target, expected] = args.as_slice() else {
        return None;
    };
    Some((
        expectation_value_label(target.value())?,
        expectation_value_label(expected.value())?,
    ))
}

fn is_expect_no_assertion_failures_call(text: &str) -> bool {
    parse_expect_method_call(text)
        .is_some_and(|(method, args)| method == "no_assertion_failures" && args.is_empty())
}

fn parse_expect_log_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "log" {
        return None;
    }
    let [level, contains] = args.as_slice() else {
        return None;
    };
    let level = match level.value() {
        Expr::Path(path) => path.trim_start_matches('.').to_owned(),
        Expr::Field { target, field } if matches!(target.as_ref(), Expr::Path(path) if path == "log") => {
            field.clone()
        }
        _ => return None,
    };
    let CallArg::Named { name, value } = contains else {
        return None;
    };
    if name != "contains" {
        return None;
    }
    Some((level, string_literal_value(value)?))
}

fn parse_expect_file_call(text: &str) -> Option<(String, String)> {
    let (method, args) = parse_expect_method_call(text)?;
    if method != "file" {
        return None;
    }
    let [path, expected] = args.as_slice() else {
        return None;
    };
    let CallArg::Named { name, value } = expected else {
        return None;
    };
    if name != "equals" {
        return None;
    }
    Some((
        virtual_path_label(path.value())?,
        string_literal_value(value)?,
    ))
}

fn parse_expect_method_call(text: &str) -> Option<(String, Vec<CallArg>)> {
    let Expr::MethodCall {
        receiver,
        method,
        args,
    } = parse_expr(text).ok()?
    else {
        return None;
    };
    matches!(receiver.as_ref(), Expr::Path(path) if path == "expect").then_some((method, args))
}

fn virtual_path_label(expr: &Expr) -> Option<String> {
    let Expr::MethodCall {
        receiver,
        method,
        args,
    } = expr
    else {
        return None;
    };
    if !matches!(receiver.as_ref(), Expr::Path(path) if path == "path") {
        return None;
    }
    if !matches!(method.as_str(), "save" | "asset" | "temp" | "export") {
        return None;
    }
    let [relative] = args.as_slice() else {
        return None;
    };
    Some(format!(
        "{method}:{}",
        string_literal_value(relative.value())?
    ))
}

fn entity_ref_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(entity.body().to_owned()),
        _ => None,
    }
}

fn expectation_value_label(expr: &Expr) -> Option<String> {
    match expr {
        Expr::EntityRef(entity) => Some(format!("@{}", entity.body())),
        Expr::Path(path) => Some(path.clone()),
        Expr::Literal(Literal::Bool(value)) => Some(value.to_string()),
        Expr::Literal(Literal::Int { value, .. }) => Some(value.to_string()),
        Expr::Literal(Literal::Float { raw, .. } | Literal::String(raw)) => Some(raw.clone()),
        _ => None,
    }
}

fn string_literal_value(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(Literal::String(value)) => Some(value.clone()),
        _ => None,
    }
}

fn script_bench_command(
    options: &ScriptBenchOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Bench, "bench")?;
    script_bench_selection(&selection, options, adapter_registrars)
}

fn script_bench_selection(
    selection: &SourceSelection,
    options: &ScriptBenchOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let pure_config = runtime_pure_config_for_selection(
        selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    let mut phases = Vec::new();
    let env = typecheck_env_for_selection(selection, None, &mut phases)?;
    let compiled = compile_profile_runtime_plan(selection, &env, &mut phases)?;
    let host_policy = native_host_policy_for_selection(selection)?;
    let manifest = collect_script_tests(&compiled.hir);
    let pure_helpers = lower_pure_helper_candidates(&compiled.hir).map(|report| report.candidates);
    let runtime = BenchRuntimeContext {
        pure_config,
        host_policy: &host_policy,
        adapter_registrars,
    };
    let output = ScriptBenchRunReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            syntax: compiled.syntax_stats.into(),
            typecheck: TypeCheckProfileStats::from(&compiled.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&compiled.typecheck_report.stats),
            runtime_plan: RuntimePlanProfileStats::from(compiled.runtime_plan_stats),
            runtime_type_validation: RuntimeTypeValidationProfileStats::from(
                &compiled.runtime_type_validation_stats,
            ),
            bytecode: BytecodeProfileStats::from(&compiled.bytecode_stats),
            aot: AotProfileStats::from(&compiled.aot_stats),
        },
        phases,
        benches: manifest
            .benches
            .iter()
            .map(|bench| {
                run_script_bench(
                    bench,
                    &compiled.plan,
                    &pure_helpers,
                    selection.path(),
                    options,
                    runtime,
                )
            })
            .collect(),
    };
    let failed = output.benches.iter().any(|bench| bench.status == "failed");
    if options.json {
        print_json(&output)?;
    } else {
        for bench in &output.benches {
            println!(
                "{} {} ({} section(s))",
                bench.id,
                bench.status,
                bench.sections.len()
            );
            for diagnostic in &bench.diagnostics {
                println!("  diagnostic {diagnostic}");
            }
        }
        println!(
            "ok: {} ({} script bench(es))",
            selection.path().display(),
            output.benches.len()
        );
    }
    if failed {
        Err(ExitCode::FAILURE)
    } else {
        Ok(())
    }
}

fn validate_script_bench(bench: &ScriptBench) -> ScriptBenchRunSummary {
    let sections = bench
        .sections
        .iter()
        .map(validate_bench_section)
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    if !bench
        .sections
        .iter()
        .any(|section| section.name == "measure")
    {
        diagnostics.push("bench requires a `measure` section".to_owned());
    }
    diagnostics.extend(
        sections
            .iter()
            .flat_map(|section| section.diagnostics.iter().cloned()),
    );
    let has_error = diagnostics
        .iter()
        .any(|diagnostic| diagnostic.starts_with("unknown bench section"))
        || !bench
            .sections
            .iter()
            .any(|section| section.name == "measure");
    let has_unsupported = sections
        .iter()
        .any(|section| section.status == "unsupported");
    let status = if has_error {
        "failed"
    } else if has_unsupported {
        "skipped"
    } else {
        "validated"
    };
    ScriptBenchRunSummary::new(bench, status, sections, diagnostics)
}

fn run_script_bench(
    bench: &ScriptBench,
    plan: &RuntimePlan,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchRunSummary {
    let mut summary = validate_script_bench(bench);
    if summary.status != "validated" {
        return summary;
    }
    let sections = bench
        .sections
        .iter()
        .map(|section| {
            run_bench_section(section, plan, pure_helpers, source_path, options, runtime)
        })
        .collect::<Vec<_>>();
    let section_failures = sections
        .iter()
        .filter(|section| section.status == "failed")
        .flat_map(|section| section.diagnostics.iter().cloned())
        .collect::<Vec<_>>();
    let has_measured = sections.iter().any(|section| section.status == "measured");
    if !section_failures.is_empty() {
        "failed".clone_into(&mut summary.status);
        summary.sections = sections;
        summary.diagnostics.extend(section_failures);
    } else if has_measured {
        "measured".clone_into(&mut summary.status);
        summary.sections = sections;
        let diagnostics = bench_expectation_failures(bench, plan, source_path, options, runtime);
        if !diagnostics.is_empty() {
            "failed".clone_into(&mut summary.status);
            summary.diagnostics.extend(diagnostics);
        }
    }
    summary
}

fn bench_expectation_failures(
    bench: &ScriptBench,
    plan: &RuntimePlan,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> Vec<String> {
    let assertions = bench
        .sections
        .iter()
        .filter(|section| section.name == "assert")
        .collect::<Vec<_>>();
    if assertions.is_empty() {
        return Vec::new();
    }
    let Some(flow) = bench.sections.iter().find_map(bench_start_flow) else {
        return vec![
            "bench assertions require a runnable `measure { start(@flow.id) }` section".to_owned(),
        ];
    };
    let mut assertion_plan = plan.clone();
    assertion_plan.entry_flow = Some(FlowRuntimeId(flow));
    let frames = run_runtime_steps(
        assertion_plan,
        Some(source_path),
        RuntimeStepRunConfig {
            steps: options.steps,
            mode: options.mode,
            max_ops: options.max_ops,
            executor: options.executor,
            pure_config: runtime.pure_config,
        },
        runtime.host_policy,
        runtime.adapter_registrars,
        &options.values,
    );
    let Ok(frames) = frames else {
        return vec!["native adapter registration failed".to_owned()];
    };
    assertions
        .into_iter()
        .flat_map(|section| bench_assertion_failures(section, &frames.steps, source_path))
        .collect()
}

fn bench_assertion_failures(
    section: &BenchSection,
    frames: &[RuntimeStepRunSummary],
    source_path: &Path,
) -> Vec<String> {
    match bench_assertion_text(section) {
        Ok(text) => evaluate_runtime_expectation(
            text,
            &RuntimeExpectationView::with_source_path(frames, source_path),
        )
        .err()
        .map(|failure| format!("bench assert failed: {failure}"))
        .into_iter()
        .collect(),
        Err(failure) => vec![failure],
    }
}

fn bench_assertion_text(section: &BenchSection) -> Result<&str, String> {
    let rest = section
        .text
        .trim()
        .strip_prefix("assert")
        .map(str::trim_start)
        .ok_or_else(|| format!("invalid assert section `{}`", section.text))?;
    let Some(body) = rest
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
    else {
        return Err("bench assert must use `assert { expect.*(...) }`".to_owned());
    };
    let body = body.trim();
    if body.is_empty() {
        return Err("bench assert body must contain an expectation call".to_owned());
    }
    Ok(body)
}

fn run_bench_pure_helper_section(
    section: &BenchSection,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
    validated: ScriptBenchSectionRunSummary,
) -> ScriptBenchSectionRunSummary {
    let Some(helper_name) = bench_pure_helper_name(section) else {
        return validated;
    };
    let candidates = match pure_helpers {
        Ok(candidates) => candidates,
        Err(errors) => {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                errors
                    .iter()
                    .map(|error| format!("pure helper lowering failed: {error}"))
                    .collect(),
            );
        }
    };
    let Some(candidate) = candidates
        .iter()
        .find(|candidate| candidate.name() == helper_name)
    else {
        return ScriptBenchSectionRunSummary::new(
            &section.name,
            "failed",
            vec![format!("pure helper `{helper_name}` was not found")],
        );
    };
    let target = match JitCheckTarget::from_candidate(candidate, None) {
        Ok(target) => target,
        Err(code) => {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                vec![format!(
                    "pure helper `{helper_name}` cannot be measured by the current JIT tier: exit code {code:?}"
                )],
            );
        }
    };
    match script_bench_pure_helper_summary(helper_name, &target, options, pure_config) {
        Ok(summary) => ScriptBenchSectionRunSummary::measured_pure_helper(
            &section.name,
            validated.diagnostics,
            summary,
        ),
        Err(message) => ScriptBenchSectionRunSummary::new(&section.name, "failed", vec![message]),
    }
}

fn script_bench_pure_helper_summary(
    helper_name: String,
    target: &JitCheckTarget,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> Result<ScriptBenchPureHelperMeasurementSummary, String> {
    let jit_options = JitCheckOptions {
        path: None,
        helper: Some(helper_name),
        case: JitBuiltinCase::Score,
        julia: false,
        iterations: options.iterations,
        warmup: options.warmup,
        samples: options.samples,
        input_seed: options.input_seed,
        json: false,
    };
    let report = run_jit_check(&jit_options, target).map_err(|code| {
        format!(
            "pure helper `{}` failed during VM/AOT/JIT measurement: exit code {code:?}",
            target.name
        )
    })?;
    if !report.matches_vm {
        return Err(format!(
            "pure helper `{}` did not match the VM reference",
            report.helper
        ));
    }
    let runtime_batch = measure_script_bench_runtime_pure_batch(target, &report, options, pure_config)
        .map_err(|code| {
            format!(
                "pure helper `{}` failed during runtime accelerator batch measurement: exit code {code:?}",
                target.name
            )
        })?;
    if !runtime_batch.matches_vm {
        return Err(format!(
            "pure helper `{}` runtime accelerator batch did not match the VM reference",
            target.name
        ));
    }
    let mut summary = ScriptBenchPureHelperMeasurementSummary::from(&report);
    summary.runtime_batch = Some(runtime_batch);
    Ok(summary)
}

fn measure_script_bench_runtime_pure_batch(
    target: &JitCheckTarget,
    report: &JitCheckReport,
    options: &ScriptBenchOptions,
    pure_config: RuntimePureAcceleratorConfig,
) -> Result<ScriptBenchPureHelperRuntimeBatchSummary, ExitCode> {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: target.name.clone(),
        input_names: target.input_names.clone(),
        input_types: vec![RuntimePureInputType::I64; target.input_names.len()],
        output_type: RuntimePureOutputType::I64,
        expr: target.expr.clone(),
        scalar_eval_supported: target.expr.supports_scalar_pure_eval(),
        origin: RuntimePureHelperOrigin::Annotated,
    };
    if options.warmup > 0 {
        let mut warmup_accelerator =
            RuntimePureAccelerator::with_config(pure_config, std::slice::from_ref(&helper));
        let mut rows = Vec::with_capacity(options.warmup.saturating_mul(target.input_names.len()));
        fill_runtime_flat_batch_inputs(&mut rows, target, options.input_seed, 0, options.warmup);
        let mut out = vec![0_i64; options.warmup];
        warmup_accelerator
            .call_i64_flat_batch(&helper, &rows, target.input_names.len(), &mut out)
            .map_err(|error| {
                eprintln!("error: runtime pure batch warmup failed: {error}");
                ExitCode::FAILURE
            })?;
    }

    let mut accelerator =
        RuntimePureAccelerator::with_config(pure_config, std::slice::from_ref(&helper));
    let mut elapsed = Vec::with_capacity(options.samples);
    let mut rows = Vec::with_capacity(options.iterations.saturating_mul(target.input_names.len()));
    let mut out = vec![0_i64; options.iterations];
    let mut accumulator = 0_i64;
    for sample in 0..options.samples {
        fill_runtime_flat_batch_inputs(
            &mut rows,
            target,
            options.input_seed,
            sample,
            options.iterations,
        );
        out.fill(0);
        let started = Instant::now();
        accelerator
            .call_i64_flat_batch(&helper, &rows, target.input_names.len(), &mut out)
            .map_err(|error| {
                eprintln!("error: runtime pure batch measurement failed: {error}");
                ExitCode::FAILURE
            })?;
        elapsed.push(started.elapsed().as_nanos());
        accumulator = out.iter().copied().fold(accumulator, i64::saturating_add);
    }
    let samples = timing_samples(elapsed);
    let executor_stats = runtime_executor_stats(0, &accelerator);
    Ok(ScriptBenchPureHelperRuntimeBatchSummary {
        matches_vm: accumulator == report.deterministic.vm,
        accumulator,
        elapsed_ns: samples.median,
        per_iteration_ns: per_iteration_ns(samples.median, options.iterations),
        speedup_x: speedup_x(report.timings.vm, samples.median),
        samples: ScriptBenchPureHelperTimingSamples::from(samples),
        config: executor_stats.pure_config,
        compile: executor_stats.pure_compile,
        stats: RuntimePureCallStatsSummary::from(accelerator.stats()),
    })
}

fn fill_runtime_flat_batch_inputs(
    inputs: &mut Vec<i64>,
    target: &JitCheckTarget,
    input_seed: u64,
    sample: usize,
    iterations: usize,
) {
    let arity = target.input_names.len();
    inputs.clear();
    inputs.reserve(iterations.saturating_mul(arity));
    for iteration in 0..iterations {
        inputs.extend_from_slice(
            &jit_check_input_array(input_seed, sample, iteration, arity)[..arity],
        );
    }
}

fn bench_pure_helper_name(section: &BenchSection) -> Option<String> {
    let Expr::Call { callee, args } = parse_expr(bench_measure_body(section)?).ok()? else {
        return None;
    };
    let Expr::Path(callee) = callee.as_ref() else {
        return None;
    };
    if callee != "pure" {
        return None;
    }
    let [helper] = args.as_slice() else {
        return None;
    };
    match helper.value() {
        Expr::Path(name) => Some(name.clone()),
        _ => None,
    }
}

fn bench_measure_body(section: &BenchSection) -> Option<&str> {
    let rest = section.text.trim().strip_prefix("measure")?;
    let open = rest.find('{')?;
    rest[open + 1..].strip_suffix('}').map(str::trim)
}

fn run_bench_section(
    section: &BenchSection,
    plan: &RuntimePlan,
    pure_helpers: &Result<Vec<PureHelperCandidate>, Vec<PureHelperLowerError>>,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
) -> ScriptBenchSectionRunSummary {
    let validated = validate_bench_section(section);
    if section.name != "measure" || validated.status != "validated" {
        return validated;
    }
    if bench_pure_helper_name(section).is_some() {
        return run_bench_pure_helper_section(
            section,
            pure_helpers,
            options,
            runtime.pure_config,
            validated,
        );
    }
    let Some(flow) = bench_start_flow(section) else {
        return validated;
    };
    run_bench_flow_section(
        section,
        plan,
        &flow,
        source_path,
        options,
        runtime,
        validated,
    )
}

fn run_bench_flow_section(
    section: &BenchSection,
    plan: &RuntimePlan,
    flow: &str,
    source_path: &Path,
    options: &ScriptBenchOptions,
    runtime: BenchRuntimeContext<'_>,
    validated: ScriptBenchSectionRunSummary,
) -> ScriptBenchSectionRunSummary {
    let mut samples = RuntimeBenchSamples::with_capacity(options.iterations);
    let mut selected_plan = plan.clone();
    selected_plan.entry_flow = Some(FlowRuntimeId(flow.to_owned()));
    let executor_template = RuntimeExecutorTemplate::new(&selected_plan, options.executor);
    let mut pure =
        RuntimePureAccelerator::with_config(runtime.pure_config, &selected_plan.pure_helpers);
    for iteration in 0..options.warmup + options.iterations {
        pure.reset_runtime_counters();
        let executor = executor_template.instantiate();
        let started = Instant::now();
        let trace = run_runtime_bench_steps_with_pure(
            executor,
            Some(source_path),
            RuntimeStepRunConfig {
                steps: options.steps,
                mode: options.mode,
                max_ops: options.max_ops,
                executor: options.executor,
                pure_config: runtime.pure_config,
            },
            runtime.host_policy,
            runtime.adapter_registrars,
            &options.values,
            &mut pure,
        );
        let Ok(trace) = trace else {
            return ScriptBenchSectionRunSummary::new(
                &section.name,
                "failed",
                vec!["native adapter registration failed".to_owned()],
            );
        };
        let elapsed_ns = started.elapsed().as_nanos();
        if iteration < options.warmup {
            continue;
        }
        samples.push(elapsed_ns, &trace);
    }
    ScriptBenchSectionRunSummary::measured(
        &section.name,
        validated.diagnostics,
        ScriptBenchMeasurementSummary {
            host_system: host_system_info(),
            executor: RuntimeExecutorTier::from(options.executor),
            executor_stats: samples.executor_stats(),
            native_io: samples.native_io.median(),
            warmup: options.warmup,
            iterations: options.iterations,
            steps: options.steps,
            per_executed_op_ns: samples.per_executed_op_ns(),
            elapsed_ns: samples.elapsed_summary(),
            deterministic: samples.deterministic_summary(),
        },
    )
}

#[derive(Default)]
struct RuntimeBenchSamples {
    elapsed: Vec<u128>,
    executed_ops: Vec<usize>,
    child_fiber_ticks: Vec<usize>,
    max_child_fibers: Vec<usize>,
    line_effects: Vec<usize>,
    task_requests: Vec<usize>,
    task_events_in: Vec<usize>,
    pure_calls: Vec<usize>,
    math_calls: Vec<usize>,
    math_accelerated_calls: Vec<usize>,
    pure_batch_calls: Vec<usize>,
    pure_batch_items: Vec<usize>,
    pure_flat_batch_calls: Vec<usize>,
    pure_flat_batch_items: Vec<usize>,
    pure_flat_batch_bytes_borrowed: Vec<usize>,
    pure_flatten_materializations: Vec<usize>,
    pure_flatten_bytes_copied: Vec<usize>,
    pure_jit_calls: Vec<usize>,
    pure_aot_calls: Vec<usize>,
    pure_vm_calls: Vec<usize>,
    pure_parallel_policy_checks: Vec<usize>,
    pure_parallel_work_units: Vec<usize>,
    pure_parallel_batches: Vec<usize>,
    pure_parallel_skipped_backend: Vec<usize>,
    pure_parallel_skipped_small: Vec<usize>,
    pure_thread_pool_jobs: Vec<usize>,
    pure_thread_pool_build_elapsed_ns: Vec<u128>,
    pure_arg_stack_packs: Vec<usize>,
    pure_arg_vec_allocations: Vec<usize>,
    pure_arg_bytes_copied: Vec<usize>,
    pure_arg_bytes_borrowed: Vec<usize>,
    pure_result_bytes_copied: Vec<usize>,
    pure_fallbacks: Vec<usize>,
    aot_fast_path_ops: Vec<usize>,
    executor_stats_samples: Vec<RuntimeExecutorStats>,
    native_io: NativeTaskStatsSamples,
    diagnostics: usize,
}

impl RuntimeBenchSamples {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            elapsed: Vec::with_capacity(capacity),
            executed_ops: Vec::with_capacity(capacity),
            child_fiber_ticks: Vec::with_capacity(capacity),
            max_child_fibers: Vec::with_capacity(capacity),
            line_effects: Vec::with_capacity(capacity),
            task_requests: Vec::with_capacity(capacity),
            task_events_in: Vec::with_capacity(capacity),
            pure_calls: Vec::with_capacity(capacity),
            math_calls: Vec::with_capacity(capacity),
            math_accelerated_calls: Vec::with_capacity(capacity),
            pure_batch_calls: Vec::with_capacity(capacity),
            pure_batch_items: Vec::with_capacity(capacity),
            pure_flat_batch_calls: Vec::with_capacity(capacity),
            pure_flat_batch_items: Vec::with_capacity(capacity),
            pure_flat_batch_bytes_borrowed: Vec::with_capacity(capacity),
            pure_flatten_materializations: Vec::with_capacity(capacity),
            pure_flatten_bytes_copied: Vec::with_capacity(capacity),
            pure_jit_calls: Vec::with_capacity(capacity),
            pure_aot_calls: Vec::with_capacity(capacity),
            pure_vm_calls: Vec::with_capacity(capacity),
            pure_parallel_policy_checks: Vec::with_capacity(capacity),
            pure_parallel_work_units: Vec::with_capacity(capacity),
            pure_parallel_batches: Vec::with_capacity(capacity),
            pure_parallel_skipped_backend: Vec::with_capacity(capacity),
            pure_parallel_skipped_small: Vec::with_capacity(capacity),
            pure_thread_pool_jobs: Vec::with_capacity(capacity),
            pure_thread_pool_build_elapsed_ns: Vec::with_capacity(capacity),
            pure_arg_stack_packs: Vec::with_capacity(capacity),
            pure_arg_vec_allocations: Vec::with_capacity(capacity),
            pure_arg_bytes_copied: Vec::with_capacity(capacity),
            pure_arg_bytes_borrowed: Vec::with_capacity(capacity),
            pure_result_bytes_copied: Vec::with_capacity(capacity),
            pure_fallbacks: Vec::with_capacity(capacity),
            aot_fast_path_ops: Vec::with_capacity(capacity),
            executor_stats_samples: Vec::with_capacity(capacity),
            native_io: NativeTaskStatsSamples::with_capacity(capacity),
            diagnostics: 0,
        }
    }

    fn push(&mut self, elapsed_ns: u128, trace: &RuntimeBenchTrace) {
        self.elapsed.push(elapsed_ns);
        self.push_step_stats(trace);
        self.push_pure_stats(trace);
        self.aot_fast_path_ops
            .push(trace.executor_stats.aot_fast_path_ops);
        self.executor_stats_samples.push(trace.executor_stats);
        self.native_io.push(&trace.native_io);
    }

    fn push_step_stats(&mut self, trace: &RuntimeBenchTrace) {
        self.executed_ops.push(trace.totals.executed_ops);
        self.child_fiber_ticks.push(trace.totals.child_fiber_ticks);
        self.max_child_fibers.push(trace.totals.max_child_fibers);
        self.line_effects.push(trace.totals.line_effects);
        self.task_requests.push(trace.totals.task_requests);
        self.task_events_in.push(trace.totals.task_events_in);
        self.diagnostics += trace.totals.diagnostics;
    }

    fn push_pure_stats(&mut self, trace: &RuntimeBenchTrace) {
        self.pure_calls.push(trace.totals.pure.pure_calls);
        self.math_calls.push(trace.totals.pure.math_calls);
        self.math_accelerated_calls
            .push(trace.totals.pure.math_accelerated_calls);
        self.pure_batch_items.push(trace.totals.pure.batch_items);
        self.pure_batch_calls.push(trace.totals.pure.batch_calls);
        self.pure_flat_batch_calls
            .push(trace.totals.pure.flat_batch_calls);
        self.pure_flat_batch_items
            .push(trace.totals.pure.flat_batch_items);
        self.pure_flat_batch_bytes_borrowed
            .push(trace.totals.pure.flat_batch_bytes_borrowed);
        self.pure_flatten_materializations
            .push(trace.totals.pure.flatten_materializations);
        self.pure_flatten_bytes_copied
            .push(trace.totals.pure.flatten_bytes_copied);
        self.pure_jit_calls.push(trace.totals.pure.jit_calls);
        self.pure_aot_calls.push(trace.totals.pure.aot_calls);
        self.pure_vm_calls.push(trace.totals.pure.vm_calls);
        self.pure_parallel_policy_checks
            .push(trace.totals.pure.parallel_policy_checks);
        self.pure_parallel_work_units
            .push(trace.totals.pure.parallel_work_units);
        self.pure_parallel_batches
            .push(trace.totals.pure.parallel_batches);
        self.pure_parallel_skipped_backend
            .push(trace.totals.pure.parallel_skipped_backend);
        self.pure_parallel_skipped_small
            .push(trace.totals.pure.parallel_skipped_small);
        self.pure_thread_pool_jobs
            .push(trace.totals.pure.thread_pool_jobs);
        self.pure_thread_pool_build_elapsed_ns
            .push(trace.totals.pure.thread_pool_build_elapsed_ns);
        self.pure_arg_stack_packs
            .push(trace.totals.pure.arg_stack_packs);
        self.pure_arg_vec_allocations
            .push(trace.totals.pure.arg_vec_allocations);
        self.pure_arg_bytes_copied
            .push(trace.totals.pure.arg_bytes_copied);
        self.pure_arg_bytes_borrowed
            .push(trace.totals.pure.arg_bytes_borrowed);
        self.pure_result_bytes_copied
            .push(trace.totals.pure.result_bytes_copied);
        self.pure_fallbacks.push(trace.totals.pure.fallbacks);
    }

    fn executor_stats(&mut self) -> RuntimeExecutorStats {
        let mut executor_stats = self
            .executor_stats_samples
            .first()
            .copied()
            .unwrap_or_else(RuntimeExecutorStats::default);
        executor_stats.aot_fast_path_ops = median_usize(&mut self.aot_fast_path_ops);
        executor_stats.math = median_executor_math_stats(&self.executor_stats_samples);
        executor_stats
    }

    fn elapsed_summary(&mut self) -> ScriptBenchElapsedSummary {
        ScriptBenchElapsedSummary {
            min: *self.elapsed.iter().min().unwrap_or(&0),
            median: median_u128(&mut self.elapsed),
            max: *self.elapsed.iter().max().unwrap_or(&0),
        }
    }

    fn per_executed_op_ns(&mut self) -> u128 {
        let elapsed = median_u128(&mut self.elapsed);
        let executed_ops = median_usize(&mut self.executed_ops);
        if executed_ops == 0 {
            0
        } else {
            elapsed / executed_ops as u128
        }
    }

    fn deterministic_summary(&mut self) -> ScriptBenchDeterministicSummary {
        ScriptBenchDeterministicSummary {
            executed_ops_median: median_usize(&mut self.executed_ops),
            child_fiber_ticks_median: median_usize(&mut self.child_fiber_ticks),
            max_child_fibers_median: median_usize(&mut self.max_child_fibers),
            line_effects_median: median_usize(&mut self.line_effects),
            task_requests_median: median_usize(&mut self.task_requests),
            task_events_in_median: median_usize(&mut self.task_events_in),
            pure_calls_median: median_usize(&mut self.pure_calls),
            math_calls_median: median_usize(&mut self.math_calls),
            math_accelerated_calls_median: median_usize(&mut self.math_accelerated_calls),
            pure_batch_calls_median: median_usize(&mut self.pure_batch_calls),
            pure_batch_items_median: median_usize(&mut self.pure_batch_items),
            pure_flat_batch_calls_median: median_usize(&mut self.pure_flat_batch_calls),
            pure_flat_batch_items_median: median_usize(&mut self.pure_flat_batch_items),
            pure_flat_batch_bytes_borrowed_median: median_usize(
                &mut self.pure_flat_batch_bytes_borrowed,
            ),
            pure_flatten_materializations_median: median_usize(
                &mut self.pure_flatten_materializations,
            ),
            pure_flatten_bytes_copied_median: median_usize(&mut self.pure_flatten_bytes_copied),
            pure_jit_calls_median: median_usize(&mut self.pure_jit_calls),
            pure_aot_calls_median: median_usize(&mut self.pure_aot_calls),
            pure_vm_calls_median: median_usize(&mut self.pure_vm_calls),
            pure_parallel_policy_checks_median: median_usize(&mut self.pure_parallel_policy_checks),
            pure_parallel_work_units_median: median_usize(&mut self.pure_parallel_work_units),
            pure_parallel_batches_median: median_usize(&mut self.pure_parallel_batches),
            pure_parallel_skipped_backend_median: median_usize(
                &mut self.pure_parallel_skipped_backend,
            ),
            pure_parallel_skipped_small_median: median_usize(&mut self.pure_parallel_skipped_small),
            pure_thread_pool_jobs_median: median_usize(&mut self.pure_thread_pool_jobs),
            pure_thread_pool_build_elapsed_ns_median: median_u128(
                &mut self.pure_thread_pool_build_elapsed_ns,
            ),
            pure_arg_stack_packs_median: median_usize(&mut self.pure_arg_stack_packs),
            pure_arg_vec_allocations_median: median_usize(&mut self.pure_arg_vec_allocations),
            pure_arg_bytes_copied_median: median_usize(&mut self.pure_arg_bytes_copied),
            pure_arg_bytes_borrowed_median: median_usize(&mut self.pure_arg_bytes_borrowed),
            pure_result_bytes_copied_median: median_usize(&mut self.pure_result_bytes_copied),
            pure_fallbacks_median: median_usize(&mut self.pure_fallbacks),
            diagnostics: self.diagnostics,
        }
    }
}

#[derive(Default)]
struct NativeTaskStatsSamples {
    completed_tasks: Vec<usize>,
    failed_tasks: Vec<usize>,
    read_ops: Vec<usize>,
    write_ops: Vec<usize>,
    system_info_ops: Vec<usize>,
    bytes_read: Vec<usize>,
    bytes_written: Vec<usize>,
    parallel_batches: Vec<usize>,
    parallel_tasks: Vec<usize>,
    parallel_io_tasks: Vec<usize>,
    parallel_system_info_tasks: Vec<usize>,
    parallel_marker_tasks: Vec<usize>,
    parallel_workers: Vec<usize>,
    scheduler_submit_elapsed_ns: Vec<u128>,
    scheduler_dispatch_elapsed_ns: Vec<u128>,
    host_complete_elapsed_ns: Vec<u128>,
    event_build_elapsed_ns: Vec<u128>,
    scheduler_complete_elapsed_ns: Vec<u128>,
    scheduler: NativeSchedulerStatsSamples,
}

impl NativeTaskStatsSamples {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            completed_tasks: Vec::with_capacity(capacity),
            failed_tasks: Vec::with_capacity(capacity),
            read_ops: Vec::with_capacity(capacity),
            write_ops: Vec::with_capacity(capacity),
            system_info_ops: Vec::with_capacity(capacity),
            bytes_read: Vec::with_capacity(capacity),
            bytes_written: Vec::with_capacity(capacity),
            parallel_batches: Vec::with_capacity(capacity),
            parallel_tasks: Vec::with_capacity(capacity),
            parallel_io_tasks: Vec::with_capacity(capacity),
            parallel_system_info_tasks: Vec::with_capacity(capacity),
            parallel_marker_tasks: Vec::with_capacity(capacity),
            parallel_workers: Vec::with_capacity(capacity),
            scheduler_submit_elapsed_ns: Vec::with_capacity(capacity),
            scheduler_dispatch_elapsed_ns: Vec::with_capacity(capacity),
            host_complete_elapsed_ns: Vec::with_capacity(capacity),
            event_build_elapsed_ns: Vec::with_capacity(capacity),
            scheduler_complete_elapsed_ns: Vec::with_capacity(capacity),
            scheduler: NativeSchedulerStatsSamples::with_capacity(capacity),
        }
    }

    fn push(&mut self, stats: &NativeTaskStats) {
        self.completed_tasks.push(stats.completed_tasks);
        self.failed_tasks.push(stats.failed_tasks);
        self.read_ops.push(stats.read_ops);
        self.write_ops.push(stats.write_ops);
        self.system_info_ops.push(stats.system_info_ops);
        self.bytes_read.push(stats.bytes_read);
        self.bytes_written.push(stats.bytes_written);
        self.parallel_batches.push(stats.parallel_batches);
        self.parallel_tasks.push(stats.parallel_tasks);
        self.parallel_io_tasks.push(stats.parallel_io_tasks);
        self.parallel_system_info_tasks
            .push(stats.parallel_system_info_tasks);
        self.parallel_marker_tasks.push(stats.parallel_marker_tasks);
        self.parallel_workers.push(stats.parallel_workers);
        self.scheduler_submit_elapsed_ns
            .push(stats.scheduler_submit_elapsed_ns);
        self.scheduler_dispatch_elapsed_ns
            .push(stats.scheduler_dispatch_elapsed_ns);
        self.host_complete_elapsed_ns
            .push(stats.host_complete_elapsed_ns);
        self.event_build_elapsed_ns
            .push(stats.event_build_elapsed_ns);
        self.scheduler_complete_elapsed_ns
            .push(stats.scheduler_complete_elapsed_ns);
        self.scheduler.push(&stats.scheduler);
    }

    fn median(&mut self) -> NativeTaskStats {
        NativeTaskStats {
            completed_tasks: median_usize(&mut self.completed_tasks),
            failed_tasks: median_usize(&mut self.failed_tasks),
            read_ops: median_usize(&mut self.read_ops),
            write_ops: median_usize(&mut self.write_ops),
            system_info_ops: median_usize(&mut self.system_info_ops),
            bytes_read: median_usize(&mut self.bytes_read),
            bytes_written: median_usize(&mut self.bytes_written),
            parallel_batches: median_usize(&mut self.parallel_batches),
            parallel_tasks: median_usize(&mut self.parallel_tasks),
            parallel_io_tasks: median_usize(&mut self.parallel_io_tasks),
            parallel_system_info_tasks: median_usize(&mut self.parallel_system_info_tasks),
            parallel_marker_tasks: median_usize(&mut self.parallel_marker_tasks),
            parallel_workers: median_usize(&mut self.parallel_workers),
            scheduler_submit_elapsed_ns: median_u128(&mut self.scheduler_submit_elapsed_ns),
            scheduler_dispatch_elapsed_ns: median_u128(&mut self.scheduler_dispatch_elapsed_ns),
            host_complete_elapsed_ns: median_u128(&mut self.host_complete_elapsed_ns),
            event_build_elapsed_ns: median_u128(&mut self.event_build_elapsed_ns),
            scheduler_complete_elapsed_ns: median_u128(&mut self.scheduler_complete_elapsed_ns),
            scheduler: self.scheduler.median(),
        }
    }
}

#[derive(Default)]
struct NativeSchedulerStatsSamples {
    submitted: Vec<usize>,
    joined: Vec<usize>,
    dispatched: Vec<usize>,
    completed: Vec<usize>,
    failed: Vec<usize>,
    cancelled: Vec<usize>,
    cancel_requested: Vec<usize>,
    joined_completed: Vec<usize>,
    in_flight: Vec<usize>,
    max_in_flight: Vec<usize>,
    dispatch_sorts: Vec<usize>,
    dispatch_sort_items: Vec<usize>,
    completion_sorts: Vec<usize>,
    completion_sort_items: Vec<usize>,
    completion_normalization_passes: Vec<usize>,
    completion_normalization_checks: Vec<usize>,
    completion_events_in: Vec<usize>,
    completion_events_joined: Vec<usize>,
    completion_events_out: Vec<usize>,
    completion_sort_skipped_items: Vec<usize>,
    completion_sort_performed_items: Vec<usize>,
    joined_completion_events_emitted: Vec<usize>,
    submitted_by_class: Vec<NativeTaskClassCounts>,
    dispatched_by_class: Vec<NativeTaskClassCounts>,
    completed_by_class: Vec<NativeTaskClassCounts>,
}

impl NativeSchedulerStatsSamples {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            submitted: Vec::with_capacity(capacity),
            joined: Vec::with_capacity(capacity),
            dispatched: Vec::with_capacity(capacity),
            completed: Vec::with_capacity(capacity),
            failed: Vec::with_capacity(capacity),
            cancelled: Vec::with_capacity(capacity),
            cancel_requested: Vec::with_capacity(capacity),
            joined_completed: Vec::with_capacity(capacity),
            in_flight: Vec::with_capacity(capacity),
            max_in_flight: Vec::with_capacity(capacity),
            dispatch_sorts: Vec::with_capacity(capacity),
            dispatch_sort_items: Vec::with_capacity(capacity),
            completion_sorts: Vec::with_capacity(capacity),
            completion_sort_items: Vec::with_capacity(capacity),
            completion_normalization_passes: Vec::with_capacity(capacity),
            completion_normalization_checks: Vec::with_capacity(capacity),
            completion_events_in: Vec::with_capacity(capacity),
            completion_events_joined: Vec::with_capacity(capacity),
            completion_events_out: Vec::with_capacity(capacity),
            completion_sort_skipped_items: Vec::with_capacity(capacity),
            completion_sort_performed_items: Vec::with_capacity(capacity),
            joined_completion_events_emitted: Vec::with_capacity(capacity),
            submitted_by_class: Vec::with_capacity(capacity),
            dispatched_by_class: Vec::with_capacity(capacity),
            completed_by_class: Vec::with_capacity(capacity),
        }
    }

    fn push(&mut self, stats: &NativeSchedulerStats) {
        self.submitted.push(stats.submitted);
        self.joined.push(stats.joined);
        self.dispatched.push(stats.dispatched);
        self.completed.push(stats.completed);
        self.failed.push(stats.failed);
        self.cancelled.push(stats.cancelled);
        self.cancel_requested.push(stats.cancel_requested);
        self.joined_completed.push(stats.joined_completed);
        self.in_flight.push(stats.in_flight);
        self.max_in_flight.push(stats.max_in_flight);
        self.dispatch_sorts.push(stats.dispatch_sorts);
        self.dispatch_sort_items.push(stats.dispatch_sort_items);
        self.completion_sorts.push(stats.completion_sorts);
        self.completion_sort_items.push(stats.completion_sort_items);
        self.completion_normalization_passes
            .push(stats.completion_normalization_passes);
        self.completion_normalization_checks
            .push(stats.completion_normalization_checks);
        self.completion_events_in.push(stats.completion_events_in);
        self.completion_events_joined
            .push(stats.completion_events_joined);
        self.completion_events_out.push(stats.completion_events_out);
        self.completion_sort_skipped_items
            .push(stats.completion_sort_skipped_items);
        self.completion_sort_performed_items
            .push(stats.completion_sort_performed_items);
        self.joined_completion_events_emitted
            .push(stats.joined_completion_events_emitted);
        self.submitted_by_class.push(stats.submitted_by_class);
        self.dispatched_by_class.push(stats.dispatched_by_class);
        self.completed_by_class.push(stats.completed_by_class);
    }

    fn median(&mut self) -> NativeSchedulerStats {
        NativeSchedulerStats {
            submitted: median_usize(&mut self.submitted),
            joined: median_usize(&mut self.joined),
            dispatched: median_usize(&mut self.dispatched),
            completed: median_usize(&mut self.completed),
            failed: median_usize(&mut self.failed),
            cancelled: median_usize(&mut self.cancelled),
            cancel_requested: median_usize(&mut self.cancel_requested),
            joined_completed: median_usize(&mut self.joined_completed),
            in_flight: median_usize(&mut self.in_flight),
            max_in_flight: median_usize(&mut self.max_in_flight),
            dispatch_sorts: median_usize(&mut self.dispatch_sorts),
            dispatch_sort_items: median_usize(&mut self.dispatch_sort_items),
            completion_sorts: median_usize(&mut self.completion_sorts),
            completion_sort_items: median_usize(&mut self.completion_sort_items),
            completion_normalization_passes: median_usize(
                &mut self.completion_normalization_passes,
            ),
            completion_normalization_checks: median_usize(
                &mut self.completion_normalization_checks,
            ),
            completion_events_in: median_usize(&mut self.completion_events_in),
            completion_events_joined: median_usize(&mut self.completion_events_joined),
            completion_events_out: median_usize(&mut self.completion_events_out),
            completion_sort_skipped_items: median_usize(&mut self.completion_sort_skipped_items),
            completion_sort_performed_items: median_usize(
                &mut self.completion_sort_performed_items,
            ),
            joined_completion_events_emitted: median_usize(
                &mut self.joined_completion_events_emitted,
            ),
            submitted_by_class: median_task_class_counts(&mut self.submitted_by_class),
            dispatched_by_class: median_task_class_counts(&mut self.dispatched_by_class),
            completed_by_class: median_task_class_counts(&mut self.completed_by_class),
        }
    }
}

fn median_task_class_counts(values: &mut [NativeTaskClassCounts]) -> NativeTaskClassCounts {
    NativeTaskClassCounts {
        local_ui: median_task_class_field(values, |value| value.local_ui),
        io: median_task_class_field(values, |value| value.io),
        cpu: median_task_class_field(values, |value| value.cpu),
        gpu_prepare: median_task_class_field(values, |value| value.gpu_prepare),
        shader_compile: median_task_class_field(values, |value| value.shader_compile),
        wasm_call: median_task_class_field(values, |value| value.wasm_call),
        asset_decode: median_task_class_field(values, |value| value.asset_decode),
        audio_decode: median_task_class_field(values, |value| value.audio_decode),
        audio_render: median_task_class_field(values, |value| value.audio_render),
        tts_synthesis: median_task_class_field(values, |value| value.tts_synthesis),
        bgm_precompose: median_task_class_field(values, |value| value.bgm_precompose),
        lsp: median_task_class_field(values, |value| value.lsp),
        background: median_task_class_field(values, |value| value.background),
    }
}

fn median_task_class_field(
    values: &[NativeTaskClassCounts],
    field: impl Fn(&NativeTaskClassCounts) -> usize,
) -> usize {
    let mut counts = values.iter().map(field).collect::<Vec<_>>();
    median_usize(&mut counts)
}

fn bench_start_flow(section: &BenchSection) -> Option<String> {
    let start = section.text.find("start(")?;
    let tail = &section.text[start..];
    let close = tail.find(')')?;
    parse_start_flow_call(&tail[..=close])
}

fn median_u128(values: &mut [u128]) -> u128 {
    if values.is_empty() {
        return 0;
    }
    let mid = values.len() / 2;
    *values.select_nth_unstable(mid).1
}

fn median_usize(values: &mut [usize]) -> usize {
    if values.is_empty() {
        return 0;
    }
    let mid = values.len() / 2;
    *values.select_nth_unstable(mid).1
}

fn median_executor_math_stats(samples: &[RuntimeExecutorStats]) -> RuntimeExecutorMathStatsSummary {
    RuntimeExecutorMathStatsSummary {
        scalar_calls: median_executor_math_field(samples, |math| math.scalar_calls),
        glam_calls: median_executor_math_field(samples, |math| math.glam_calls),
        ndarray_calls: median_executor_math_field(samples, |math| math.ndarray_calls),
        wgpu_calls: median_executor_math_field(samples, |math| math.wgpu_calls),
        fallback_calls: median_executor_math_field(samples, |math| math.fallback_calls),
        bytes_borrowed: median_executor_math_field(samples, |math| math.bytes_borrowed),
        bytes_copied: median_executor_math_field(samples, |math| math.bytes_copied),
        bytes_uploaded: median_executor_math_field(samples, |math| math.bytes_uploaded),
        bytes_downloaded: median_executor_math_field(samples, |math| math.bytes_downloaded),
        gpu_buffer_creations: median_executor_math_field(samples, |math| math.gpu_buffer_creations),
        gpu_buffer_reuse_hits: median_executor_math_field(samples, |math| {
            math.gpu_buffer_reuse_hits
        }),
        gpu_staging_buffer_creations: median_executor_math_field(samples, |math| {
            math.gpu_staging_buffer_creations
        }),
        gpu_staging_buffer_reuse_hits: median_executor_math_field(samples, |math| {
            math.gpu_staging_buffer_reuse_hits
        }),
        gpu_reused_dispatches: median_executor_math_field(samples, |math| {
            math.gpu_reused_dispatches
        }),
        last_backend: modal_executor_math_label(samples, |math| math.last_backend),
        last_auto_reason: modal_executor_math_label(samples, |math| math.last_auto_reason),
    }
}

fn median_executor_math_field(
    samples: &[RuntimeExecutorStats],
    field: impl Fn(RuntimeExecutorMathStatsSummary) -> usize,
) -> usize {
    let mut values = samples
        .iter()
        .map(|sample| field(sample.math))
        .collect::<Vec<_>>();
    median_usize(&mut values)
}

fn modal_executor_math_label(
    samples: &[RuntimeExecutorStats],
    field: impl Fn(RuntimeExecutorMathStatsSummary) -> Option<&'static str>,
) -> Option<&'static str> {
    let mut counts: Vec<(Option<&'static str>, usize, usize)> = Vec::new();
    for (index, sample) in samples.iter().enumerate() {
        let label = field(sample.math);
        if let Some((_, count, _)) = counts
            .iter_mut()
            .find(|(candidate, _, _)| *candidate == label)
        {
            *count += 1;
        } else {
            counts.push((label, 1, index));
        }
    }
    counts
        .into_iter()
        .max_by(|(_, lhs_count, lhs_first), (_, rhs_count, rhs_first)| {
            lhs_count
                .cmp(rhs_count)
                .then_with(|| rhs_first.cmp(lhs_first))
        })
        .and_then(|(label, _, _)| label)
}

fn validate_bench_section(section: &BenchSection) -> ScriptBenchSectionRunSummary {
    let mut diagnostics = Vec::new();
    if !is_known_bench_section(&section.name) {
        diagnostics.push(format!("unknown bench section `{}`", section.name));
        return ScriptBenchSectionRunSummary::new(&section.name, "unknown", diagnostics);
    }
    if let Some(reason) = unsupported_headless_bench_reason(&section.text) {
        diagnostics.push(reason);
        return ScriptBenchSectionRunSummary::new(&section.name, "unsupported", diagnostics);
    }
    ScriptBenchSectionRunSummary::new(&section.name, "validated", diagnostics)
}

fn is_known_bench_section(name: &str) -> bool {
    matches!(name, "setup" | "measure" | "assert" | "report")
}

fn unsupported_headless_bench_reason(text: &str) -> Option<String> {
    const UNSUPPORTED_MARKERS: &[&str] = &[
        "render_audio_offline",
        "capture.image",
        "snapshot.image",
        "screenshot",
        "audio.",
        "voice.",
        "bgm.",
        "render.",
    ];
    let lowered = text.to_lowercase();
    UNSUPPORTED_MARKERS
        .iter()
        .find(|marker| lowered.contains(**marker))
        .map(|marker| {
            format!("headless bench validation does not execute adapter-only operation `{marker}`")
        })
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
        lower_runtime_plan(&checked.hir).map_err(|errors| {
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

fn runtime_pure_config_for_selection(
    selection: &SourceSelection,
    backend: Option<CliRuntimePureBackend>,
    workers: Option<CliRuntimePureWorkers>,
    batch_min_len: Option<usize>,
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

fn runtime_pure_backend_label(value: RuntimePureBackendMode) -> &'static str {
    match value {
        RuntimePureBackendMode::Auto => "auto",
        RuntimePureBackendMode::Vm => "vm",
        RuntimePureBackendMode::Aot => "aot",
        RuntimePureBackendMode::Jit => "jit",
    }
}

fn runtime_math_backend_label(value: RuntimeMathBackend) -> &'static str {
    match value {
        RuntimeMathBackend::Auto => "auto",
        RuntimeMathBackend::Scalar => "scalar",
        RuntimeMathBackend::Glam => "glam",
        RuntimeMathBackend::Ndarray => "ndarray",
        RuntimeMathBackend::Wgpu => "wgpu",
    }
}

fn runtime_math_auto_reason_label(value: RuntimeMathAutoSelectionReason) -> &'static str {
    match value {
        RuntimeMathAutoSelectionReason::Matmul4x4Glam => "matmul_4x4_glam",
        RuntimeMathAutoSelectionReason::MatmulWgpuWorkThreshold => "matmul_wgpu_work_threshold",
        RuntimeMathAutoSelectionReason::MatmulCpuDefault => "matmul_cpu_default",
        RuntimeMathAutoSelectionReason::ElementwiseWgpuWorkThreshold => {
            "elementwise_wgpu_work_threshold"
        }
        RuntimeMathAutoSelectionReason::ElementwiseCpuDefault => "elementwise_cpu_default",
    }
}

impl From<RuntimePureCompileStats> for RuntimeExecutorPureCompileStatsSummary {
    fn from(stats: RuntimePureCompileStats) -> Self {
        Self {
            jit_attempts: stats.jit_attempts,
            jit_successes: stats.jit_successes,
            jit_failures: stats.jit_failures,
            aot_attempts: stats.aot_attempts,
            aot_successes: stats.aot_successes,
            aot_failures: stats.aot_failures,
            auto_jit_selected: stats.auto_jit_selected,
            auto_aot_selected: stats.auto_aot_selected,
            auto_vm_selected: stats.auto_vm_selected,
            auto_jit_deferred: stats.auto_jit_deferred,
            auto_jit_promotions: stats.auto_jit_promotions,
            auto_jit_skipped_small: stats.auto_jit_skipped_small,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            compile_elapsed_ns: stats.compile_elapsed_ns,
        }
    }
}

impl From<RuntimeMathStats> for RuntimeExecutorMathStatsSummary {
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
            gpu_staging_buffer_creations: stats.gpu_staging_buffer_creations,
            gpu_staging_buffer_reuse_hits: stats.gpu_staging_buffer_reuse_hits,
            gpu_reused_dispatches: stats.gpu_reused_dispatches,
            last_backend: stats.last_backend.map(runtime_math_backend_label),
            last_auto_reason: stats.last_auto_reason.map(runtime_math_auto_reason_label),
        }
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
    Ok(manifest.apply_to_env(TypeCheckEnv::new()))
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
        eprintln!("warning[{:?}]: {}", lint.code(), lint.message());
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
        syntax_warnings: lints.len(),
        syntax_stats,
        line_task_groups,
        typecheck_report,
        phases,
    })
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
struct ToolingCommandOptions {
    path: PathBuf,
    #[arg(long)]
    expand_sugar: bool,
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
