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
use arcweft_agent_mcp::{
    McpCallToolResult, McpContentBlock, agent_tool_descriptors, list_resource_templates_result,
    list_resources_result, read_resource_result, resource_descriptor, tool_result_for_resource,
    tool_result_for_resources,
};
use arcweft_agent_protocol::{
    AgentActionDispatch, AgentActionKind, AgentActionTarget, AgentAssignment, AgentAudioState,
    AgentBBox, AgentCoordinateSpace, AgentDiagnostic, AgentDiagnosticSeverity,
    AgentGlyphOrientation, AgentGlyphVerticalForm, AgentHitRegion, AgentHitRegionKind,
    AgentImageComposition, AgentImageContentBBox, AgentImageCropOrigin, AgentImageKind,
    AgentImageRenderer, AgentImageResource, AgentImageScope, AgentLayerCaptureRef,
    AgentLayerCaptureRefs, AgentObjectCaptureRef, AgentObjectCaptureRefs, AgentObservationReport,
    AgentObservedLayer, AgentObservedObject, AgentResource, AgentRgbaColor,
    AgentRichTextElementKind, AgentRichTextElementRef, AgentUiTree, AgentViewport,
};
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
    FlowEvent, FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget,
    RuntimePlan, RuntimePureHelper, RuntimePureHelperId, RuntimePureHelperOrigin,
    RuntimePureInputType, RuntimePureOutputType, RuntimeRouteSpec,
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
use arcweft_render_text::{
    LineDisplayCatalog, LineDisplayFrame, RichTextControl, RichTextNode, RichTextRange,
    RichTextRubyAnnotation, RichTextTextRun, RichTextTextSource, RuntimeLineContext,
};
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
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead as _, Write as _};
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Component, Path, PathBuf};
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
    Agent {
        #[command(subcommand)]
        command: AgentCommand,
    },
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
    Bundle(BundleOptions),
    RunBundle(RunBundleOptions),
    Build {
        #[command(subcommand)]
        command: BuildCommand,
    },
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
#[allow(clippy::large_enum_variant)]
enum AgentCommand {
    Observe(AgentObserveOptions),
    Mcp(AgentMcpOptions),
}

#[derive(Debug, Subcommand)]
enum JitCommand {
    Check(JitCheckOptions),
}

#[derive(Debug, Subcommand)]
enum BuildCommand {
    Bundle(BundleOptions),
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

fn format_command(options: &ToolingCommandOptions) -> Result<(), ExitCode> {
    run_tooling_command(options, |source| {
        format_source(
            source,
            FormatOptions {
                expand_sugar: options.expand_sugar,
                canonical_rich_text: options.canonical_rich_text,
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
        options.pure_object_artifacts,
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

fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Observe(options) => agent_observe_command(&options, adapter_registrars),
        AgentCommand::Mcp(options) => agent_mcp_command(&options, adapter_registrars),
    }
}

fn agent_mcp_command(
    _options: &AgentMcpOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    let mut state = AgentMcpState::default();
    for line in stdin.lock().lines() {
        let line = line.map_err(|error| {
            eprintln!("error: failed to read MCP request: {error}");
            ExitCode::FAILURE
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<AgentMcpJsonRpcRequest>(&line) {
            Ok(request) => agent_mcp_handle_request(request, &mut state, adapter_registrars),
            Err(error) => Some(agent_mcp_error_response(
                None,
                -32700,
                &format!("parse error: {error}"),
            )),
        };
        if let Some(response) = response {
            serde_json::to_writer(&mut stdout, &response).map_err(|error| {
                eprintln!("error: failed to write MCP response: {error}");
                ExitCode::FAILURE
            })?;
            stdout.write_all(b"\n").map_err(|error| {
                eprintln!("error: failed to write MCP response newline: {error}");
                ExitCode::FAILURE
            })?;
            stdout.flush().map_err(|error| {
                eprintln!("error: failed to flush MCP response: {error}");
                ExitCode::FAILURE
            })?;
        }
    }
    Ok(())
}

#[derive(Default)]
struct AgentMcpState {
    report: Option<AgentObservationReport>,
    image_output: Option<AgentImageOutput>,
    capture_resources: Vec<AgentResource>,
    native_capture_session: Option<arcweft_player_native::native::NativeOffscreenCaptureSession>,
}

#[derive(serde::Deserialize)]
struct AgentMcpJsonRpcRequest {
    #[serde(default)]
    id: Option<serde_json::Value>,
    method: String,
    #[serde(default)]
    params: serde_json::Value,
}

fn agent_mcp_handle_request(
    request: AgentMcpJsonRpcRequest,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Option<serde_json::Value> {
    let id = request.id;
    let result = match request.method.as_str() {
        "notifications/initialized" => return None,
        "initialize" => Ok(serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {
                "tools": {},
                "resources": {}
            },
            "serverInfo": {
                "name": "arcweft-agent",
                "version": env!("CARGO_PKG_VERSION")
            }
        })),
        "tools/list" => Ok(serde_json::json!({
            "tools": agent_tool_descriptors()
        })),
        "resources/templates/list" => serde_json::to_value(list_resource_templates_result())
            .map_err(|error| format!("failed to serialize MCP resource templates: {error}")),
        "resources/list" => agent_mcp_resource_list(state),
        "resources/read" => agent_mcp_resource_read(&request.params, state),
        "tools/call" => agent_mcp_tool_call(&request.params, state, adapter_registrars),
        method => Err(format!("unsupported MCP method `{method}`")),
    };
    Some(match result {
        Ok(result) => agent_mcp_success_response(id.as_ref(), &result),
        Err(message) => agent_mcp_error_response(id.as_ref(), -32603, &message),
    })
}

fn agent_mcp_resource_list(state: &AgentMcpState) -> Result<serde_json::Value, String> {
    if state.report.is_none() {
        return Ok(serde_json::json!({ "resources": [] }));
    }
    let resources = agent_mcp_current_resources(state)
        .map_err(|_| "failed to build Agent resource list".to_owned())?;
    serde_json::to_value(list_resources_result(&resources))
        .map_err(|error| format!("failed to serialize MCP resource list: {error}"))
}

fn agent_mcp_resource_read(
    params: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<serde_json::Value, String> {
    let uri = params
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "resources/read requires params.uri".to_owned())?;
    let Some(report) = &state.report else {
        return Err("resources/read requires a prior arcweft.observe call".to_owned());
    };
    let resource = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(report, state.image_output.as_ref(), uri))
        .map_or_else(|| agent_observe_resource_by_uri(report, uri), Ok)
        .map_err(|_| format!("failed to read Agent resource `{uri}`"))?;
    let read = read_resource_result(&resource)
        .map_err(|error| format!("failed to serialize MCP resource: {error}"))?;
    serde_json::to_value(read).map_err(|error| format!("failed to serialize MCP read: {error}"))
}

fn agent_mcp_tool_call(
    params: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let name = params
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "tools/call requires params.name".to_owned())?;
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    match name {
        "arcweft.observe" => agent_mcp_call_observe(&arguments, state, adapter_registrars),
        "arcweft.session.info" => {
            let tool = agent_mcp_call_session_info(state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP session info: {error}"))
        }
        "arcweft.resource.read" => {
            let tool = agent_mcp_call_resource_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP tool result: {error}"))
        }
        "arcweft.capture" => {
            let tool = agent_mcp_call_capture(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP capture result: {error}"))
        }
        tool => Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    }
}

fn agent_mcp_call_observe(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let (report, image_output, resources) =
        agent_mcp_run_observation(arguments, adapter_registrars)?;
    state.report = Some(report);
    state.image_output = image_output;
    state.capture_resources.clear();
    let tool = tool_result_for_resources(&resources);
    serde_json::to_value(tool)
        .map_err(|error| format!("failed to serialize MCP tool result: {error}"))
}

fn agent_mcp_call_session_info(state: &AgentMcpState) -> Result<McpCallToolResult, String> {
    let info = if let Some(report) = &state.report {
        let resources = agent_mcp_current_resources(state)
            .map_err(|_| "failed to build Agent session resource list".to_owned())?;
        let descriptors = list_resources_result(&resources).resources;
        let latest_capture = agent_mcp_latest_capture_resource(state);
        let latest_capture_descriptor = latest_capture.map(resource_descriptor);
        serde_json::json!({
            "observed": true,
            "session_id": report.session_id,
            "tick": report.tick,
            "frame_id": report.frame_id,
            "source": report.source,
            "final_status": report.final_status,
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "resource_templates": list_resource_templates_result().resource_templates,
            "images": report.images,
            "layers": report.layers,
            "objects": report.objects,
            "capture_resource_count": state.capture_resources.len(),
            "native_capture_session_active": state.native_capture_session.is_some(),
            "latest_capture": latest_capture.and_then(|resource| resource.image.as_ref()),
            "latest_capture_uri": latest_capture.map(|resource| resource.uri.as_str()),
            "latest_capture_resource": latest_capture_descriptor,
        })
    } else {
        serde_json::json!({
            "observed": false,
            "resource_count": 0,
            "resources": [],
            "resource_templates": list_resource_templates_result().resource_templates,
            "images": [],
            "layers": [],
            "objects": [],
            "capture_resource_count": 0,
            "native_capture_session_active": false,
            "latest_capture": null,
            "latest_capture_uri": null,
            "latest_capture_resource": null,
        })
    };
    let text = serde_json::to_string(&info)
        .map_err(|error| format!("failed to serialize Agent session info: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

fn agent_mcp_run_observation(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<
    (
        AgentObservationReport,
        Option<AgentImageOutput>,
        Vec<AgentResource>,
    ),
    String,
> {
    let options = agent_mcp_observe_options(arguments)?;
    validate_agent_observe_options(&options).map_err(|_| "invalid observe options".to_owned())?;
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)
        .map_err(|_| "failed to resolve MCP observe source".to_owned())?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    )
    .map_err(|_| "failed to resolve runtime pure config".to_owned())?;
    let checked = load_and_check_selection(&selection, None)
        .map_err(|_| "failed to check MCP observe source".to_owned())?;
    let host_policy = native_host_policy_for_selection(&selection)
        .map_err(|_| "failed to resolve native host policy".to_owned())?;
    let lowered = lower_runtime_plan_with_stats(&checked.hir)
        .map_err(|_| "failed to lower runtime plan".to_owned())?;
    let mut plan = lowered.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())
        .map_err(|_| "failed to select runtime entry".to_owned())?;
    let mut executor = RuntimeExecutorInstance::new(plan, options.executor, pure_config);
    let mut report = run_agent_observation(
        &mut executor,
        &lowered.line_display_catalog,
        NativeRunHost {
            source_path: Some(selection.path()),
            policy: &host_policy,
            adapter_registrars,
        },
        &options,
        selection.path(),
    )
    .map_err(|error| error.to_string())?;
    let image_output = agent_observe_image_output(&mut report, &options)
        .map_err(|_| "failed to build MCP observe image output".to_owned())?;
    let resources = agent_observe_all_resources(&report, image_output.as_ref())
        .map_err(|_| "failed to build MCP observe resources".to_owned())?;
    Ok((report, image_output, resources))
}

fn agent_mcp_call_resource_read(
    arguments: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    let uri = arguments
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.resource.read requires arguments.uri".to_owned())?;
    let Some(report) = &state.report else {
        return Err("arcweft.resource.read requires a prior arcweft.observe call".to_owned());
    };
    let resource = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(report, state.image_output.as_ref(), uri))
        .map_or_else(|| agent_observe_resource_by_uri(report, uri), Ok)
        .map_err(|_| format!("failed to read Agent resource `{uri}`"))?;
    tool_result_for_resource(&resource)
        .map_err(|error| format!("failed to serialize MCP tool resource: {error}"))
}

fn agent_mcp_current_resources(state: &AgentMcpState) -> Result<Vec<AgentResource>, ExitCode> {
    let Some(report) = &state.report else {
        return Ok(Vec::new());
    };
    let mut resources = agent_observe_all_resources(report, state.image_output.as_ref())?;
    for capture in &state.capture_resources {
        resources.retain(|resource| resource.uri != capture.uri);
        resources.push(capture.clone());
    }
    Ok(resources)
}

fn agent_mcp_cached_capture_resource(state: &AgentMcpState, uri: &str) -> Option<AgentResource> {
    state
        .capture_resources
        .iter()
        .rev()
        .find(|resource| resource.uri == uri)
        .or_else(|| {
            if uri.contains('?') {
                return None;
            }
            state.capture_resources.iter().rev().find(|resource| {
                agent_uri_without_query(&resource.uri)
                    .is_some_and(|resource_uri| resource_uri == uri)
            })
        })
        .cloned()
}

fn agent_mcp_latest_capture_resource(state: &AgentMcpState) -> Option<&AgentResource> {
    state.capture_resources.last()
}

fn agent_uri_without_query(uri: &str) -> Option<&str> {
    uri.split_once('?').map(|(base, _)| base)
}

fn agent_mcp_call_capture(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    if arguments.get("source").is_some() {
        let (report, image_output, _) = agent_mcp_run_observation(
            &agent_mcp_capture_observe_arguments(arguments),
            adapter_registrars,
        )?;
        state.report = Some(report);
        state.image_output = image_output;
    }
    let Some(report) = state.report.clone() else {
        return Err(
            "arcweft.capture requires a prior arcweft.observe call or arguments.source".to_owned(),
        );
    };
    let request = agent_mcp_capture_request(arguments, &report)?;
    let resource = agent_mcp_capture_resource(&report, &request, state)
        .map_err(|_| format!("failed to capture Agent image `{}`", request.uri))?;
    state
        .capture_resources
        .retain(|cached| cached.uri != resource.uri);
    state.capture_resources.push(resource.clone());
    tool_result_for_resource(&resource)
        .map_err(|error| format!("failed to serialize MCP capture resource: {error}"))
}

fn agent_mcp_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    state: &mut AgentMcpState,
) -> Result<AgentResource, ExitCode> {
    let native_session = agent_mcp_native_capture_session(state)?;
    agent_native_capture_resource_with_session(report, request, native_session)
}

fn agent_mcp_native_capture_session(
    state: &mut AgentMcpState,
) -> Result<&mut arcweft_player_native::native::NativeOffscreenCaptureSession, ExitCode> {
    if state.native_capture_session.is_none() {
        state.native_capture_session = Some(
            arcweft_player_native::native::NativeOffscreenCaptureSession::new().map_err(
                |error| {
                    eprintln!("error: native capture failed: {error}");
                    ExitCode::FAILURE
                },
            )?,
        );
    }
    Ok(state
        .native_capture_session
        .as_mut()
        .expect("native capture session initialized above"))
}

fn agent_mcp_capture_observe_arguments(arguments: &serde_json::Value) -> serde_json::Value {
    let mut observe_arguments = arguments.clone();
    if let Some(object) = observe_arguments.as_object_mut() {
        object.remove("format");
        object.remove("capture");
        object.remove("image");
        object.remove("uri");
        object.remove("page");
    }
    observe_arguments
}

fn agent_mcp_capture_request(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
) -> Result<AgentCaptureReadRequest, String> {
    if let Some(uri) = arguments.get("uri").and_then(serde_json::Value::as_str) {
        for key in ["format", "capture", "layer", "object"] {
            if arguments.get(key).is_some() {
                return Err(
                    "arcweft.capture accepts arguments.uri or format/capture/layer/object selectors, not both"
                        .to_owned(),
                );
            }
        }
        let mut request = agent_capture_request_from_uri(report, uri)
            .ok_or_else(|| format!("unsupported Agent image capture URI `{uri}`"))?;
        if arguments.get("renderer").is_some() {
            return Err("arcweft.capture no longer accepts arguments.renderer".to_owned());
        }
        if arguments.get("page").is_some() {
            request.page = agent_mcp_capture_page(arguments)?;
        }
        request.capture_time_seconds =
            agent_mcp_capture_time_argument(arguments, "arcweft.capture")?.unwrap_or(60.0);
        return Ok(request);
    }
    let page = agent_mcp_capture_page(arguments)?;
    let capture_time_seconds =
        agent_mcp_capture_time_argument(arguments, "arcweft.capture")?.unwrap_or(60.0);
    let image_kind = arguments
        .get("format")
        .and_then(serde_json::Value::as_str)
        .map(agent_mcp_capture_image_kind)
        .transpose()?
        .unwrap_or(AgentObserveImageKind::Png);
    let capture_kind = arguments
        .get("capture")
        .and_then(serde_json::Value::as_str)
        .map(agent_mcp_capture_kind)
        .transpose()?
        .unwrap_or(AgentObserveCaptureKind::Color);
    if arguments.get("renderer").is_some() {
        return Err("arcweft.capture no longer accepts arguments.renderer".to_owned());
    }
    let layer = arguments
        .get("layer")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    let object = arguments
        .get("object")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if layer.is_some() && object.is_some() {
        return Err(
            "arcweft.capture accepts either arguments.layer or arguments.object, not both"
                .to_owned(),
        );
    }
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => {
            return Err("arcweft.capture supports format png or raw-rgba".to_owned());
        }
    };
    let (scope, name) = if let Some(object) = object {
        let name = agent_scoped_capture_name("object", &object, capture_kind.resource_name());
        (AgentCaptureScope::Object(object), name)
    } else if let Some(layer) = layer {
        let name = agent_scoped_capture_name("layer", &layer, capture_kind.resource_name());
        (AgentCaptureScope::Layer(layer), name)
    } else {
        (
            AgentCaptureScope::Viewport,
            capture_kind.resource_name().to_owned(),
        )
    };
    let uri =
        agent_frame_capture_uri_for_page(&report.session_id, report.tick, &name, extension, page);
    Ok(AgentCaptureReadRequest {
        uri,
        image_kind,
        capture_kind,
        scope,
        page,
        capture_time_seconds,
    })
}

fn agent_mcp_capture_page(arguments: &serde_json::Value) -> Result<usize, String> {
    agent_mcp_page_argument(arguments, "arcweft.capture")
}

fn agent_mcp_page_argument(arguments: &serde_json::Value, tool: &str) -> Result<usize, String> {
    let Some(value) = arguments.get("page") else {
        return Ok(0);
    };
    let page = value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument page must be a non-negative integer"))?;
    usize::try_from(page)
        .map_err(|_| format!("{tool} argument page is too large for this platform"))
}

fn agent_mcp_capture_time_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<Option<f32>, String> {
    let Some(value) = arguments.get("capture_time") else {
        return Ok(None);
    };
    let seconds = serde_json::from_value::<f32>(value.clone())
        .map_err(|_| format!("{tool} argument capture_time must be a number of seconds"))?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err(format!(
            "{tool} argument capture_time must be a finite non-negative number of seconds"
        ));
    }
    Ok(Some(seconds))
}

fn agent_mcp_capture_image_kind(value: &str) -> Result<AgentObserveImageKind, String> {
    match value {
        "png" => Ok(AgentObserveImageKind::Png),
        "raw-rgba" => Ok(AgentObserveImageKind::RawRgba),
        _ => Err(format!("unsupported capture format `{value}`")),
    }
}

fn agent_mcp_observe_options(arguments: &serde_json::Value) -> Result<AgentObserveOptions, String> {
    let source = arguments
        .get("source")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.observe requires arguments.source".to_owned())?;
    if arguments.get("renderer").is_some() {
        return Err("arcweft.observe no longer accepts arguments.renderer".to_owned());
    }
    Ok(AgentObserveOptions {
        path: Some(PathBuf::from(source)),
        profile: ProfileOptions {
            profile: arguments
                .get("profile")
                .and_then(serde_json::Value::as_str)
                .map(ToOwned::to_owned),
            manifest: arguments
                .get("manifest")
                .and_then(serde_json::Value::as_str)
                .map_or_else(|| PathBuf::from("arcw.toml"), PathBuf::from),
        },
        entry: arguments
            .get("entry")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        flow: arguments
            .get("flow")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        executor: CliRuntimeExecutorTier::BytecodeVm,
        pure_backend: None,
        pure_workers: None,
        pure_batch_min_len: None,
        pure_object_artifacts: false,
        math_backend: None,
        math_wgpu_min_elements: None,
        steps: agent_mcp_usize_argument(arguments, "steps").unwrap_or(8),
        mode: CliRuntimeStepMode::Drain,
        max_ops: agent_mcp_usize_argument(arguments, "max_ops").unwrap_or(64),
        values: Vec::new(),
        image: arguments
            .get("image")
            .and_then(serde_json::Value::as_str)
            .map(agent_mcp_image_kind)
            .transpose()?,
        capture: arguments
            .get("capture")
            .and_then(serde_json::Value::as_str)
            .map(agent_mcp_capture_kind)
            .transpose()?,
        layer: arguments
            .get("layer")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        object: arguments
            .get("object")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        page: arguments
            .get("page")
            .map(|_| agent_mcp_page_argument(arguments, "arcweft.observe"))
            .transpose()?,
        capture_time_seconds: agent_mcp_capture_time_argument(arguments, "arcweft.observe")?
            .unwrap_or(60.0),
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        out: None,
        json: false,
    })
}

fn agent_mcp_usize_argument(arguments: &serde_json::Value, name: &str) -> Option<usize> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

fn agent_mcp_image_kind(value: &str) -> Result<AgentObserveImageKind, String> {
    match value {
        "overlay" => Ok(AgentObserveImageKind::Overlay),
        "png" => Ok(AgentObserveImageKind::Png),
        "raw-rgba" => Ok(AgentObserveImageKind::RawRgba),
        _ => Err(format!("unsupported image kind `{value}`")),
    }
}

fn agent_mcp_capture_kind(value: &str) -> Result<AgentObserveCaptureKind, String> {
    match value {
        "color" => Ok(AgentObserveCaptureKind::Color),
        "object-id" => Ok(AgentObserveCaptureKind::ObjectId),
        "mask" => Ok(AgentObserveCaptureKind::Mask),
        _ => Err(format!("unsupported capture kind `{value}`")),
    }
}

fn agent_mcp_success_response(
    id: Option<&serde_json::Value>,
    result: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

fn agent_mcp_error_response(
    id: Option<&serde_json::Value>,
    code: i64,
    message: &str,
) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

fn agent_observe_command(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    validate_agent_observe_options(options)?;
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let pure_config = runtime_pure_config_for_selection(
        &selection,
        options.pure_backend,
        options.pure_workers,
        options.pure_batch_min_len,
        options.pure_object_artifacts,
        options.math_backend,
        options.math_wgpu_min_elements,
    )?;
    let checked = load_and_check_selection(&selection, None)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let lowered = lower_runtime_plan_with_stats(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {}", error.message());
        }
        ExitCode::FAILURE
    })?;
    let mut plan = lowered.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let mut executor = RuntimeExecutorInstance::new(plan, options.executor, pure_config);
    let report = run_agent_observation(
        &mut executor,
        &lowered.line_display_catalog,
        NativeRunHost {
            source_path: Some(selection.path()),
            policy: &host_policy,
            adapter_registrars,
        },
        options,
        selection.path(),
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    let mut report = report;
    let image_output = agent_observe_image_output(&mut report, options)?;
    if let Some(uri) = &options.read_uri {
        let resource = agent_observe_cached_image_resource(&report, image_output.as_ref(), uri)
            .map_or_else(
                || {
                    agent_observe_resource_by_uri_with_page_and_time(
                        &report,
                        uri,
                        options.page,
                        options.capture_time_seconds,
                    )
                },
                Ok,
            )?;
        if options.mcp {
            let resource = agent_observe_mcp_resource_output(
                AgentObserveResourceOutput::One(Box::new(resource)),
                options.mcp_format,
            )?;
            return print_json(&resource);
        }
        return print_json(&resource);
    }
    if let Some(out) = &options.out {
        let Some(image_output) = &image_output else {
            eprintln!("error: --out requires --image");
            return Err(ExitCode::from(2));
        };
        fs::write(out, &image_output.bytes).map_err(|error| {
            eprintln!("error: failed to write {}: {error}", out.display());
            ExitCode::FAILURE
        })?;
    }
    if let Some(resource) = options.resource {
        let resource = agent_observe_resource(&report, image_output.as_ref(), resource)?;
        if options.mcp {
            let resource = agent_observe_mcp_resource_output(resource, options.mcp_format)?;
            print_json(&resource)
        } else {
            print_json(&resource)
        }
    } else if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} object(s), {} diagnostic(s), render_hash={})",
            report.source,
            report.objects.len(),
            report.diagnostics.len(),
            report.render_hash
        );
        Ok(())
    }
}

fn validate_agent_observe_options(options: &AgentObserveOptions) -> Result<(), ExitCode> {
    if options.steps == 0 {
        eprintln!("error: --steps must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.layer.is_some() && options.object.is_some() {
        eprintln!("error: --layer and --object cannot be used together");
        return Err(ExitCode::from(2));
    }
    if options.out.is_some() && options.image.is_none() {
        eprintln!("error: --out requires --image");
        return Err(ExitCode::from(2));
    }
    if options.capture.is_some()
        && !matches!(
            options.image,
            Some(AgentObserveImageKind::Png | AgentObserveImageKind::RawRgba)
        )
    {
        eprintln!("error: --capture requires --image png or --image raw-rgba");
        return Err(ExitCode::from(2));
    }
    if matches!(options.resource, Some(AgentObserveResourceKind::Overlay))
        && !matches!(options.image, Some(AgentObserveImageKind::Overlay))
    {
        eprintln!("error: --resource overlay requires --image overlay");
        return Err(ExitCode::from(2));
    }
    if options.read_uri.is_some() && options.resource.is_some() {
        eprintln!("error: --read-uri and --resource cannot be used together");
        return Err(ExitCode::from(2));
    }
    if options.mcp && options.resource.is_none() && options.read_uri.is_none() {
        eprintln!("error: --mcp requires --resource or --read-uri");
        return Err(ExitCode::from(2));
    }
    if !options.mcp && options.mcp_format != AgentObserveMcpFormat::Read {
        eprintln!("error: --mcp-format requires --mcp");
        return Err(ExitCode::from(2));
    }
    if !options.capture_time_seconds.is_finite() || options.capture_time_seconds < 0.0 {
        eprintln!("error: --capture-time must be a finite non-negative number of seconds");
        return Err(ExitCode::from(2));
    }
    Ok(())
}

fn agent_observe_resource_by_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time(report, uri, None, 60.0)
}

fn agent_observe_resource_by_uri_with_page_and_time(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
) -> Result<AgentResource, ExitCode> {
    if uri
        == format!(
            "arcweft://session/{}/observation/latest.json",
            report.session_id
        )
    {
        return report
            .observation_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri
        == format!(
            "arcweft://session/{}/frame/{}/objects.json",
            report.session_id, report.tick
        )
    {
        return report
            .objects_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri
        == format!(
            "arcweft://session/{}/frame/{}/overlay.svg",
            report.session_id, report.tick
        )
    {
        let selected = report.objects.iter().collect::<Vec<_>>();
        let overlay = agent_overlay_svg(&report.viewport, &selected);
        return Ok(AgentResource {
            uri: uri.to_owned(),
            kind: arcweft_agent_protocol::AgentResourceKind::OverlaySvg,
            mime_type: "image/svg+xml".to_owned(),
            hash: hash_hex(overlay.as_bytes()),
            image: None,
            body: arcweft_agent_protocol::AgentResourceBody::Text(overlay),
        });
    }
    if uri == format!("arcweft://session/{}/logs.ndjson", report.session_id) {
        return report
            .logs_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri == format!("arcweft://session/{}/signals.json", report.session_id) {
        return report
            .signals_resource()
            .map_err(|error| agent_json_error(&error));
    }
    if uri == format!("arcweft://session/{}/audio.json", report.session_id) {
        return report
            .audio_resource()
            .map_err(|error| agent_json_error(&error));
    }
    let Some(request) = agent_capture_request_from_uri(report, uri) else {
        eprintln!("error: unsupported Agent resource URI: {uri}");
        return Err(ExitCode::from(2));
    };
    let request = AgentCaptureReadRequest {
        page: page_override.unwrap_or(request.page),
        capture_time_seconds,
        ..request
    };
    agent_observe_capture_resource(report, &request)
}

#[derive(Clone, Debug)]
struct AgentCaptureReadRequest {
    uri: String,
    image_kind: AgentObserveImageKind,
    capture_kind: AgentObserveCaptureKind,
    scope: AgentCaptureScope,
    page: usize,
    capture_time_seconds: f32,
}

#[derive(Clone, Debug)]
enum AgentCaptureScope {
    Viewport,
    Layer(String),
    Object(String),
}

fn agent_capture_request_from_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Option<AgentCaptureReadRequest> {
    let (uri_without_query, page) = agent_capture_uri_query(uri)?;
    let prefix = format!(
        "arcweft://session/{}/frame/{}/",
        report.session_id, report.tick
    );
    let name = uri_without_query.strip_prefix(&prefix)?;
    let (stem, extension) = name.rsplit_once('.')?;
    let image_kind = match extension {
        "png" => AgentObserveImageKind::Png,
        "rgba" => AgentObserveImageKind::RawRgba,
        _ => return None,
    };
    let (capture_stem, capture_kind) = if let Some(base) = stem.strip_suffix(".object-id") {
        (base, AgentObserveCaptureKind::ObjectId)
    } else if let Some(base) = stem.strip_suffix(".mask") {
        (base, AgentObserveCaptureKind::Mask)
    } else if stem == "object-id" {
        ("", AgentObserveCaptureKind::ObjectId)
    } else if stem == "mask" {
        ("", AgentObserveCaptureKind::Mask)
    } else {
        (stem, AgentObserveCaptureKind::Color)
    };
    let scope = if capture_stem.is_empty() || capture_stem == "color" {
        AgentCaptureScope::Viewport
    } else if let Some(layer) = capture_stem.strip_prefix("layer.") {
        AgentCaptureScope::Layer(layer.to_owned())
    } else if let Some(object) = capture_stem.strip_prefix("object.") {
        AgentCaptureScope::Object(object.to_owned())
    } else {
        return None;
    };
    Some(AgentCaptureReadRequest {
        uri: uri.to_owned(),
        image_kind,
        capture_kind,
        scope,
        page,
        capture_time_seconds: 60.0,
    })
}

fn agent_capture_uri_query(uri: &str) -> Option<(&str, usize)> {
    let Some((base, query)) = uri.split_once('?') else {
        return Some((uri, 0));
    };
    let mut page = 0;
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=')?;
        match key {
            "page" => {
                page = value.parse::<usize>().ok()?;
            }
            _ => return None,
        }
    }
    Some((base, page))
}

fn agent_observe_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentResource, ExitCode> {
    agent_native_capture_resource(report, request)
}

fn agent_native_capture_resource(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentResource, ExitCode> {
    let (image, bytes) = agent_native_capture_image(report, request)?;
    Ok(report.image_resource(&image, &bytes))
}

fn agent_native_capture_resource_with_session(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_player_native::native::NativeOffscreenCaptureSession,
) -> Result<AgentResource, ExitCode> {
    let (image, bytes) = agent_native_capture_image_with_session(report, request, native_session)?;
    Ok(report.image_resource(&image, &bytes))
}

fn agent_native_capture_image(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<(AgentImageResource, Vec<u8>), ExitCode> {
    let mut native_session = arcweft_player_native::native::NativeOffscreenCaptureSession::new()
        .map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    agent_native_capture_image_with_session(report, request, &mut native_session)
}

fn agent_native_capture_image_with_session(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_player_native::native::NativeOffscreenCaptureSession,
) -> Result<(AgentImageResource, Vec<u8>), ExitCode> {
    let Some(textbox) = agent_native_textbox_for_capture(report, &request.scope) else {
        eprintln!("error: native renderer requires an observed textbox frame");
        return Err(ExitCode::from(2));
    };
    let (left, top) = agent_native_text_origin(textbox);
    let capture = native_session
        .capture_frame_rgba_in(
            &textbox.rich_text,
            arcweft_player_native::native::NativeCaptureViewport::new(
                report.viewport.width,
                report.viewport.height,
                left,
                top,
                request.page,
            )
            .with_time_seconds(request.capture_time_seconds),
        )
        .map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    let capture = agent_native_scoped_capture(
        &capture,
        AgentNativeCaptureContext {
            frame: &textbox.rich_text,
            left,
            top,
            objects: &report.objects,
            page_index: request.page,
            capture_time_seconds: request.capture_time_seconds,
        },
        &request.scope,
        request.capture_kind,
        Some(native_session),
    )?;
    let (mime_type, bytes) = match request.image_kind {
        AgentObserveImageKind::Png => ("image/png", agent_encode_png(&capture)?),
        AgentObserveImageKind::RawRgba => ("application/octet-stream", capture.rgba.clone()),
        AgentObserveImageKind::Overlay => unreachable!("overlay is not a raster capture"),
    };
    let stats = capture.content_stats();
    let content_viewport_bbox = agent_content_viewport_bbox(capture.crop_origin, stats.bbox);
    let image = AgentImageResource {
        kind: agent_image_kind(request.capture_kind),
        renderer: AgentImageRenderer::Native,
        scope: agent_image_scope_for_capture_scope(&request.scope),
        composition: capture.composition,
        page: request.page,
        uri: request.uri.clone(),
        mime_type: mime_type.to_owned(),
        width: capture.width,
        height: capture.height,
        hash: hash_hex(&bytes),
        crop_origin: capture.crop_origin,
        content_bbox: stats.bbox,
        content_viewport_bbox,
        content_pixels: Some(stats.content_pixels),
        written: None,
    };
    Ok((image, bytes))
}

fn agent_native_textbox_for_capture<'a>(
    report: &'a AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<&'a AgentObservedObject> {
    if let AgentCaptureScope::Object(object_id) = scope {
        let object = report
            .objects
            .iter()
            .find(|object| object.id == *object_id)?;
        if object.role == "textbox" {
            return Some(object);
        }
        if let Some(parent_id) = agent_rich_text_child_parent_object_id(&object.id) {
            return report
                .objects
                .iter()
                .find(|candidate| candidate.role == "textbox" && candidate.id == parent_id);
        }
    }
    report
        .objects
        .iter()
        .find(|object| object.role == "textbox")
}

fn agent_rich_text_child_parent_object_id(object_id: &str) -> Option<&str> {
    object_id
        .split_once(".run.")
        .or_else(|| object_id.split_once(".ruby."))
        .or_else(|| object_id.split_once(".cluster."))
        .map(|(parent, _)| parent)
}

#[allow(clippy::cast_precision_loss)]
fn agent_native_text_origin(textbox: &AgentObservedObject) -> (f32, f32) {
    (
        textbox.bbox.x.saturating_add(24) as f32,
        textbox.bbox.y.saturating_add(24) as f32,
    )
}

#[derive(Clone, Copy)]
struct AgentNativeCaptureContext<'a> {
    frame: &'a LineDisplayFrame,
    left: f32,
    top: f32,
    objects: &'a [AgentObservedObject],
    page_index: usize,
    capture_time_seconds: f32,
}

fn agent_native_scoped_capture(
    capture: &arcweft_player_native::native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    scope: &AgentCaptureScope,
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_player_native::native::NativeOffscreenCaptureSession>,
) -> Result<AgentRasterCapture, ExitCode> {
    let full = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: AgentImageComposition::Framebuffer,
        background: [0, 0, 0, 255],
        rgba: capture.rgba.clone(),
    };
    let selected = agent_capture_objects_for_scope(context.objects, scope)?;
    let selected = agent_native_capture_objects_for_page(
        capture.width,
        capture.height,
        context,
        scope,
        selected,
    )?;
    if capture_kind == AgentObserveCaptureKind::Color {
        let AgentCaptureScope::Viewport = scope else {
            if matches!(scope, AgentCaptureScope::Layer(_))
                && selected
                    .iter()
                    .any(|object| !object.role.starts_with("rich_text_"))
            {
                let (x, y, width, height) =
                    agent_native_scope_rect(capture.width, capture.height, context, &selected)?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            if let Some(isolated) =
                agent_native_color_capture(capture, context, &selected, native_session)?
            {
                let mut rgba = isolated.rgba;
                make_nontransparent_pixels_opaque(&mut rgba);
                let full = AgentRasterCapture {
                    width: isolated.width,
                    height: isolated.height,
                    crop_origin: None,
                    composition: AgentImageComposition::IsolatedRegions,
                    background: [0, 0, 0, 0],
                    rgba,
                };
                let (x, y, width, height) =
                    agent_native_scope_rect(capture.width, capture.height, context, &selected)?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            return agent_native_masked_framebuffer_capture(capture, context, &selected);
        };
        return Ok(full);
    }

    let debug =
        agent_native_debug_capture(capture, context, &selected, capture_kind, native_session)?;
    let full = AgentRasterCapture {
        width: debug.capture.width,
        height: debug.capture.height,
        crop_origin: None,
        composition: debug.composition,
        background: [0, 0, 0, 0],
        rgba: debug.capture.rgba,
    };
    if !matches!(scope, AgentCaptureScope::Viewport) {
        let (x, y, width, height) =
            agent_native_scope_rect(capture.width, capture.height, context, &selected)?;
        return Ok(agent_crop_raster_capture(&full, x, y, width, height));
    }
    Ok(full)
}

fn make_nontransparent_pixels_opaque(rgba: &mut [u8]) {
    for pixel in rgba.chunks_exact_mut(4) {
        if pixel[3] > 0 {
            pixel[3] = 255;
        }
    }
}

fn agent_native_capture_objects_for_page<'a>(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'a>,
    scope: &AgentCaptureScope,
    selected: Vec<&'a AgentObservedObject>,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    if !matches!(scope, AgentCaptureScope::Layer(_)) {
        return Ok(selected);
    }
    selected
        .into_iter()
        .filter_map(|object| {
            match agent_native_object_is_visible_on_page(
                capture_width,
                capture_height,
                context,
                object,
            ) {
                Ok(true) => Some(Ok(object)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            }
        })
        .collect()
}

fn agent_native_object_is_visible_on_page(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Result<bool, ExitCode> {
    if !object.role.starts_with("rich_text_") {
        return Ok(true);
    }
    agent_native_rich_text_child_rect(
        capture_width,
        capture_height,
        context.objects,
        object,
        context.page_index,
    )
    .map(|rect| rect.is_some())
}

struct AgentNativeDebugCapture {
    capture: arcweft_player_native::native::NativeFrameCapture,
    composition: AgentImageComposition,
}

fn agent_native_color_capture(
    capture: &arcweft_player_native::native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[&AgentObservedObject],
    native_session: Option<&mut arcweft_player_native::native::NativeOffscreenCaptureSession>,
) -> Result<Option<arcweft_player_native::native::NativeFrameCapture>, ExitCode> {
    let mut regions = Vec::new();
    for object in selected {
        let object_regions = agent_native_regions_for_object(
            capture.width,
            capture.height,
            context,
            object,
            [0, 0, 0, 0],
        )?;
        if object_regions.iter().any(|region| region.element.is_none()) {
            return Ok(None);
        }
        regions.extend(object_regions);
    }
    let capture_result = if let Some(native_session) = native_session {
        native_session.capture_frame_color_regions_in(
            context.frame,
            arcweft_player_native::native::NativeCaptureViewport::new(
                capture.width,
                capture.height,
                context.left,
                context.top,
                context.page_index,
            )
            .with_time_seconds(context.capture_time_seconds),
            &regions,
        )
    } else {
        arcweft_player_native::native::capture_frame_color_regions_at_page(
            context.frame,
            capture.width,
            capture.height,
            context.left,
            context.top,
            context.page_index,
            &regions,
        )
    };
    capture_result.map(Some).map_err(|error| {
        eprintln!("error: native color region capture failed: {error}");
        ExitCode::FAILURE
    })
}

fn agent_native_debug_capture(
    capture: &arcweft_player_native::native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[&AgentObservedObject],
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_player_native::native::NativeOffscreenCaptureSession>,
) -> Result<AgentNativeDebugCapture, ExitCode> {
    let mut regions = Vec::new();
    for object in selected {
        let color = match capture_kind {
            AgentObserveCaptureKind::Color => {
                unreachable!("native geometry capture is debug-only")
            }
            AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
            AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
        };
        regions.extend(agent_native_regions_for_object(
            capture.width,
            capture.height,
            context,
            object,
            color,
        )?);
    }
    let composition = match capture_kind {
        AgentObserveCaptureKind::Color => {
            unreachable!("native geometry capture is debug-only")
        }
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
    };
    let capture_result = if let Some(native_session) = native_session {
        native_session.capture_frame_debug_regions_in(
            context.frame,
            arcweft_player_native::native::NativeCaptureViewport::new(
                capture.width,
                capture.height,
                context.left,
                context.top,
                context.page_index,
            )
            .with_time_seconds(context.capture_time_seconds),
            &regions,
        )
    } else {
        arcweft_player_native::native::capture_frame_debug_regions_at_page(
            context.frame,
            capture.width,
            capture.height,
            context.left,
            context.top,
            context.page_index,
            &regions,
        )
    };
    capture_result
        .map(|capture| AgentNativeDebugCapture {
            capture,
            composition,
        })
        .map_err(|error| {
            eprintln!("error: native debug capture failed: {error}");
            ExitCode::FAILURE
        })
}

fn agent_native_masked_framebuffer_capture(
    capture: &arcweft_player_native::native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[&AgentObservedObject],
) -> Result<AgentRasterCapture, ExitCode> {
    let mut masked = AgentRasterCapture::new(
        capture.width,
        capture.height,
        [0, 0, 0, 0],
        AgentImageComposition::MaskedFramebufferCrop,
    );
    for object in selected {
        let (x, y, width, height) =
            agent_native_object_rect(capture.width, capture.height, context, object)?;
        agent_copy_native_framebuffer_rect(&mut masked, capture, x, y, width, height);
    }
    let (x, y, width, height) =
        agent_native_scope_rect(capture.width, capture.height, context, selected)?;
    Ok(agent_crop_raster_capture(&masked, x, y, width, height))
}

fn agent_copy_native_framebuffer_rect(
    target: &mut AgentRasterCapture,
    source: &arcweft_player_native::native::NativeFrameCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) {
    let source_width = usize::try_from(source.width).unwrap_or(0);
    let target_width = usize::try_from(target.width).unwrap_or(0);
    let copy_width = usize::try_from(width).unwrap_or(0);
    let row_bytes = copy_width.saturating_mul(4);
    for row in 0..height {
        let source_y = y.saturating_add(row);
        let source_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(source_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let target_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(target_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let Some(source_row) = source
            .rgba
            .get(source_start..source_start.saturating_add(row_bytes))
        else {
            continue;
        };
        let Some(target_row) = target
            .rgba
            .get_mut(target_start..target_start.saturating_add(row_bytes))
        else {
            continue;
        };
        target_row.copy_from_slice(source_row);
    }
}

fn agent_native_regions_for_object(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    color: [u8; 4],
) -> Result<Vec<arcweft_player_native::native::NativeFrameDebugRegion>, ExitCode> {
    let (x, y, width, height) =
        agent_native_object_rect(capture_width, capture_height, context, object)?;
    let fallback_bbox = arcweft_player_native::native::NativeFrameContentBBox {
        x,
        y,
        width,
        height,
    };
    let elements = agent_native_elements_for_object(object);
    if elements.is_empty() {
        return Ok(vec![
            arcweft_player_native::native::NativeFrameDebugRegion {
                element: None,
                fallback_bbox,
                color,
            },
        ]);
    }
    Ok(elements
        .into_iter()
        .map(
            |element| arcweft_player_native::native::NativeFrameDebugRegion {
                element: Some(element),
                fallback_bbox,
                color,
            },
        )
        .collect())
}

fn agent_native_elements_for_object(
    object: &AgentObservedObject,
) -> Vec<arcweft_player_native::native::NativeFrameElement> {
    if object.role == "textbox" {
        return object
            .rich_text
            .display_map
            .text_runs
            .iter()
            .enumerate()
            .map(|(index, _)| arcweft_player_native::native::NativeFrameElement::TextRun { index })
            .chain(
                object
                    .rich_text
                    .display_map
                    .ruby_annotations
                    .iter()
                    .enumerate()
                    .map(
                        |(index, _)| arcweft_player_native::native::NativeFrameElement::Ruby {
                            index,
                        },
                    ),
            )
            .collect();
    }
    agent_native_element_for_object(object)
        .into_iter()
        .collect()
}

fn agent_native_scope_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    selected: &[&AgentObservedObject],
) -> Result<(u32, u32, u32, u32), ExitCode> {
    let mut min_x = capture_width;
    let mut min_y = capture_height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for object in selected {
        let (x, y, width, height) =
            agent_native_object_rect(capture_width, capture_height, context, object)?;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x.saturating_add(width));
        max_y = max_y.max(y.saturating_add(height));
    }
    let x = min_x.min(capture_width.saturating_sub(1));
    let y = min_y.min(capture_height.saturating_sub(1));
    let width = max_x
        .saturating_sub(x)
        .min(capture_width.saturating_sub(x))
        .max(1);
    let height = max_y
        .saturating_sub(y)
        .min(capture_height.saturating_sub(y))
        .max(1);
    Ok((x, y, width, height))
}

fn agent_native_object_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    if object.role.starts_with("rich_text_")
        && let Some(rect) = agent_native_rich_text_child_rect(
            capture_width,
            capture_height,
            context.objects,
            object,
            context.page_index,
        )?
    {
        return Ok(rect);
    }
    Ok(agent_clamped_bbox_rect(
        capture_width,
        capture_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    ))
}

fn agent_native_rich_text_child_rect(
    capture_width: u32,
    capture_height: u32,
    objects: &[AgentObservedObject],
    object: &AgentObservedObject,
    page_index: usize,
) -> Result<Option<(u32, u32, u32, u32)>, ExitCode> {
    let Some(element) = agent_native_element_for_object(object) else {
        return Ok(None);
    };
    let Some(textbox) = agent_native_textbox_for_rich_text_child(objects, object) else {
        return Ok(None);
    };
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = arcweft_player_native::native::measure_frame_elements_at_page(
        &textbox.rich_text,
        capture_width,
        capture_height,
        left,
        top,
        page_index,
    )
    .map_err(|error| {
        eprintln!("error: native text layout measurement failed: {error}");
        ExitCode::FAILURE
    })?;
    Ok(bounds
        .into_iter()
        .find(|bounds| bounds.element == element)
        .map(|bounds| {
            agent_clamped_bbox_rect(
                capture_width,
                capture_height,
                bounds.bbox.x,
                bounds.bbox.y,
                bounds.bbox.width,
                bounds.bbox.height,
            )
        }))
}

fn agent_native_textbox_for_rich_text_child<'a>(
    objects: &'a [AgentObservedObject],
    object: &AgentObservedObject,
) -> Option<&'a AgentObservedObject> {
    let parent_id = agent_rich_text_child_parent_object_id(&object.id)?;
    objects
        .iter()
        .find(|candidate| candidate.role == "textbox" && candidate.id == parent_id)
}

fn agent_native_element_for_object(
    object: &AgentObservedObject,
) -> Option<arcweft_player_native::native::NativeFrameElement> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return agent_native_element_for_object_id(&object.id);
    };
    match rich_text_ref.kind {
        AgentRichTextElementKind::TextRun | AgentRichTextElementKind::Ruby => {
            agent_native_element_for_object_id(&object.id)
        }
        AgentRichTextElementKind::GlyphCluster => Some(
            arcweft_player_native::native::NativeFrameElement::GlyphCluster {
                index: rich_text_ref.index,
                range_start: rich_text_ref.range.start,
                range_end: rich_text_ref.range.end,
            },
        ),
    }
}

fn agent_native_element_for_object_id(
    object_id: &str,
) -> Option<arcweft_player_native::native::NativeFrameElement> {
    if let Some((_, index)) = object_id.rsplit_once(".run.") {
        return index
            .parse()
            .ok()
            .map(|index| arcweft_player_native::native::NativeFrameElement::TextRun { index });
    }
    if let Some((_, index)) = object_id.rsplit_once(".ruby.") {
        return index
            .parse()
            .ok()
            .map(|index| arcweft_player_native::native::NativeFrameElement::Ruby { index });
    }
    None
}

fn agent_clamped_bbox_rect(
    capture_width: u32,
    capture_height: u32,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> (u32, u32, u32, u32) {
    let x = x.min(capture_width.saturating_sub(1));
    let y = y.min(capture_height.saturating_sub(1));
    let width = width.min(capture_width.saturating_sub(x)).max(1);
    let height = height.min(capture_height.saturating_sub(y)).max(1);
    (x, y, width, height)
}

fn agent_crop_raster_capture(
    source: &AgentRasterCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
) -> AgentRasterCapture {
    let mut crop = AgentRasterCapture::new(
        width,
        height,
        source.background,
        agent_cropped_composition(source.composition),
    );
    crop.crop_origin = Some(agent_crop_origin(source.crop_origin, x, y));
    let source_width = usize::try_from(source.width).unwrap_or(0);
    let crop_width = usize::try_from(width).unwrap_or(0);
    let row_bytes = crop_width.saturating_mul(4);
    for row in 0..height {
        let source_y = y.saturating_add(row);
        let source_start = usize::try_from(source_y)
            .unwrap_or(0)
            .saturating_mul(source_width)
            .saturating_add(usize::try_from(x).unwrap_or(0))
            .saturating_mul(4);
        let crop_start = usize::try_from(row)
            .unwrap_or(0)
            .saturating_mul(crop_width)
            .saturating_mul(4);
        let Some(source_row) = source
            .rgba
            .get(source_start..source_start.saturating_add(row_bytes))
        else {
            continue;
        };
        let Some(crop_row) = crop
            .rgba
            .get_mut(crop_start..crop_start.saturating_add(row_bytes))
        else {
            continue;
        };
        crop_row.copy_from_slice(source_row);
    }
    crop
}

fn agent_cropped_composition(composition: AgentImageComposition) -> AgentImageComposition {
    match composition {
        AgentImageComposition::Framebuffer => AgentImageComposition::FramebufferCrop,
        composition => composition,
    }
}

fn agent_crop_origin(
    source_origin: Option<AgentImageCropOrigin>,
    x: u32,
    y: u32,
) -> AgentImageCropOrigin {
    let source_origin = source_origin.unwrap_or(AgentImageCropOrigin {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
    });
    AgentImageCropOrigin {
        space: source_origin.space,
        x: source_origin.x.saturating_add(x),
        y: source_origin.y.saturating_add(y),
    }
}

fn agent_content_viewport_bbox(
    crop_origin: Option<AgentImageCropOrigin>,
    content_bbox: Option<AgentImageContentBBox>,
) -> Option<AgentImageContentBBox> {
    let content_bbox = content_bbox?;
    let origin = crop_origin.unwrap_or(AgentImageCropOrigin {
        space: AgentCoordinateSpace::Viewport,
        x: 0,
        y: 0,
    });
    (origin.space == AgentCoordinateSpace::Viewport).then_some(AgentImageContentBBox {
        x: origin.x.saturating_add(content_bbox.x),
        y: origin.y.saturating_add(content_bbox.y),
        width: content_bbox.width,
        height: content_bbox.height,
    })
}

fn agent_capture_objects_for_scope<'a>(
    objects: &'a [AgentObservedObject],
    scope: &AgentCaptureScope,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    match scope {
        AgentCaptureScope::Viewport => Ok(objects.iter().collect()),
        AgentCaptureScope::Layer(layer) => {
            let selected = objects
                .iter()
                .filter(|object| object.layer == *layer)
                .collect::<Vec<_>>();
            if selected.is_empty() {
                eprintln!("error: no observed object matches resource layer {layer}");
                return Err(ExitCode::from(2));
            }
            Ok(selected)
        }
        AgentCaptureScope::Object(object_id) => {
            let Some(object) = objects.iter().find(|object| object.id == *object_id) else {
                eprintln!("error: no observed object matches resource object {object_id}");
                return Err(ExitCode::from(2));
            };
            Ok(vec![object])
        }
    }
}

fn agent_observe_mcp_resource_output(
    resource: AgentObserveResourceOutput,
    format: AgentObserveMcpFormat,
) -> Result<AgentObserveMcpResourceOutput, ExitCode> {
    let resources = match resource {
        AgentObserveResourceOutput::One(resource) => vec![*resource],
        AgentObserveResourceOutput::Many(resources) => resources,
    };
    match format {
        AgentObserveMcpFormat::Read => {
            let mut read_results = resources
                .into_iter()
                .map(|resource| {
                    read_resource_result(&resource).map_err(|error| agent_json_error(&error))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if read_results.len() == 1 {
                Ok(AgentObserveMcpResourceOutput::OneRead(
                    read_results.remove(0),
                ))
            } else {
                Ok(AgentObserveMcpResourceOutput::ManyRead(read_results))
            }
        }
        AgentObserveMcpFormat::List => Ok(AgentObserveMcpResourceOutput::List(
            list_resources_result(&resources),
        )),
        AgentObserveMcpFormat::ToolResult => {
            if resources.len() == 1 {
                let resource = resources.first().expect("length checked");
                Ok(AgentObserveMcpResourceOutput::ToolResult(
                    tool_result_for_resource(resource).map_err(|error| agent_json_error(&error))?,
                ))
            } else {
                Ok(AgentObserveMcpResourceOutput::ToolResult(
                    tool_result_for_resources(&resources),
                ))
            }
        }
    }
}

fn agent_observe_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
    resource: AgentObserveResourceKind,
) -> Result<AgentObserveResourceOutput, ExitCode> {
    let resource = match resource {
        AgentObserveResourceKind::Observation => AgentObserveResourceOutput::One(Box::new(
            report
                .observation_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Objects => AgentObserveResourceOutput::One(Box::new(
            report
                .objects_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Overlay => {
            let Some(resource) = report.overlay_svg_resource() else {
                eprintln!("error: overlay resource was not generated");
                return Err(ExitCode::from(2));
            };
            AgentObserveResourceOutput::One(Box::new(resource))
        }
        AgentObserveResourceKind::Image => {
            let Some(resource) = agent_observe_image_resource(report, image_output) else {
                eprintln!("error: --resource image requires --image");
                return Err(ExitCode::from(2));
            };
            AgentObserveResourceOutput::One(Box::new(resource))
        }
        AgentObserveResourceKind::Logs => AgentObserveResourceOutput::One(Box::new(
            report
                .logs_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Signals => AgentObserveResourceOutput::One(Box::new(
            report
                .signals_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::Audio => AgentObserveResourceOutput::One(Box::new(
            report
                .audio_resource()
                .map_err(|error| agent_json_error(&error))?,
        )),
        AgentObserveResourceKind::All => {
            AgentObserveResourceOutput::Many(agent_observe_all_resources(report, image_output)?)
        }
    };
    Ok(resource)
}

fn agent_observe_all_resources(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = vec![
        report
            .observation_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .objects_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .logs_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .signals_resource()
            .map_err(|error| agent_json_error(&error))?,
        report
            .audio_resource()
            .map_err(|error| agent_json_error(&error))?,
    ];
    if let Some(overlay) = report.overlay_svg_resource() {
        resources.push(overlay);
    }
    if let Some(image) = agent_observe_image_resource(report, image_output) {
        resources.push(image);
    }
    let mut known = resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect::<BTreeSet<_>>();
    for uri in report.layers.iter().flat_map(|layer| {
        layer
            .capture_refs
            .captures
            .iter()
            .map(|capture| capture.uri.as_str())
    }) {
        if known.insert(uri.to_owned()) {
            resources.push(agent_observe_resource_by_uri(report, uri)?);
        }
    }
    for uri in report.objects.iter().flat_map(|object| {
        object
            .capture_refs
            .captures
            .iter()
            .map(|capture| capture.uri.as_str())
    }) {
        if known.insert(uri.to_owned()) {
            resources.push(agent_observe_resource_by_uri(report, uri)?);
        }
    }
    Ok(resources)
}

fn agent_observe_image_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Option<AgentResource> {
    let image = report.images.first()?;
    let output = image_output?;
    if image.uri != output.uri {
        return None;
    }
    Some(report.image_resource(image, &output.bytes))
}

fn agent_observe_cached_image_resource(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
    uri: &str,
) -> Option<AgentResource> {
    let output = image_output?;
    if output.uri != uri {
        return None;
    }
    let image = report.images.iter().find(|image| image.uri == uri)?;
    Some(report.image_resource(image, &output.bytes))
}

fn agent_json_error(error: &serde_json::Error) -> ExitCode {
    eprintln!("error: failed to build agent resource JSON: {error}");
    ExitCode::FAILURE
}

fn run_agent_observation(
    executor: &mut RuntimeExecutorInstance,
    catalog: &LineDisplayCatalog,
    host_config: NativeRunHost<'_>,
    options: &AgentObserveOptions,
    source_path: &Path,
) -> Result<AgentObservationReport, arcweft_host_adapter::HostAdapterError> {
    let viewport = AgentViewport {
        width: 1280,
        height: 720,
        scale: 1.0,
    };
    let mut host = host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut task_events = Vec::new();
    let mut objects: Vec<AgentObservedObject> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut task_request_count = 0usize;
    let mut tick = 0usize;
    for step_index in 0..options.steps {
        tick = step_index;
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            &options.values,
            step_options(options.mode, options.max_ops),
        );
        let RuntimeStepResult { mut output, .. } = result;
        diagnostics.extend(output.diagnostics.iter().map(|diagnostic| AgentDiagnostic {
            step: step_index,
            severity: AgentDiagnosticSeverity::Error,
            message: diagnostic.message.clone(),
        }));
        for event in &output.flow_events {
            if let FlowEvent::DialogueLine { line, bindings } = event {
                let Some(spec) = catalog.find(line) else {
                    diagnostics.push(AgentDiagnostic {
                        step: step_index,
                        severity: AgentDiagnosticSeverity::Warning,
                        message: format!("missing display catalog entry for line {}", line.0),
                    });
                    continue;
                };
                match spec.resolve_frame(&RuntimeLineContext::new(bindings.clone())) {
                    Ok(frame) => {
                        let index = objects
                            .iter()
                            .filter(|object| object.role == "textbox")
                            .count();
                        let textbox = agent_textbox_object(step_index, index, frame, &viewport);
                        let native_bounds =
                            agent_native_rich_text_element_bboxes(&textbox, &viewport);
                        let children = agent_rich_text_child_objects(
                            step_index,
                            index,
                            &textbox,
                            &native_bounds,
                        );
                        objects.push(textbox);
                        objects.extend(children);
                    }
                    Err(error) => diagnostics.push(AgentDiagnostic {
                        step: step_index,
                        severity: AgentDiagnosticSeverity::Error,
                        message: error.to_string(),
                    }),
                }
            }
        }
        let task_requests = std::mem::take(&mut output.requests.tasks);
        task_request_count += task_requests.len();
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(finish_agent_observation_report(
        executor,
        source_path,
        AgentObservationTrace {
            viewport,
            objects,
            diagnostics,
            task_request_count,
            tick,
        },
        options,
    ))
}

fn finish_agent_observation_report(
    executor: &RuntimeExecutorInstance,
    source_path: &Path,
    trace: AgentObservationTrace,
    _options: &AgentObserveOptions,
) -> AgentObservationReport {
    let AgentObservationTrace {
        viewport,
        objects,
        diagnostics,
        task_request_count,
        tick,
    } = trace;
    let object_refs = objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&viewport, &object_refs);
    let render_hash = hash_hex(overlay_svg.as_bytes());
    let observations = &executor.fiber().observations;
    let signals = observations
        .signals
        .iter()
        .map(|(name, value)| AgentAssignment {
            name: name.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    let metrics = observations
        .metrics
        .iter()
        .map(|(name, value)| AgentAssignment {
            name: name.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>();
    let actions = objects
        .iter()
        .map(|object| AgentActionTarget {
            id: format!("action.advance_text.{}", object.id),
            target: object.id.clone(),
            action: AgentActionKind::AdvanceText,
            kind: AgentActionDispatch::Semantic,
            enabled: true,
        })
        .collect::<Vec<_>>();
    let layers = agent_observed_layers("cli", tick, &objects);
    let status = flow_status_label(&executor.fiber().status);
    let state_hash = hash_hex(
        format!(
            "{}:{}:{}:{}:{}",
            status,
            tick,
            objects.len(),
            diagnostics.len(),
            task_request_count
        )
        .as_bytes(),
    );
    AgentObservationReport {
        status: if matches!(executor.fiber().status, FlowFiberStatus::Failed(_)) {
            "failed".to_owned()
        } else {
            "ok".to_owned()
        },
        session_id: "cli".to_owned(),
        tick,
        frame_id: format!("frame.{tick}"),
        state_hash,
        render_hash: render_hash.clone(),
        source: report_path(source_path),
        viewport,
        images: Vec::new(),
        layers,
        objects,
        actions,
        ui_tree: AgentUiTree {
            root: "ui.root".to_owned(),
            children: vec!["dialogue.layer".to_owned()],
        },
        scene_graph: Vec::new(),
        audio_state: AgentAudioState {
            active_voices: Vec::new(),
            pending_events: Vec::new(),
        },
        logs: observations.logs.clone(),
        signals,
        metrics,
        events: observations.events.clone(),
        diagnostics,
        steps: tick + 1,
        task_requests: task_request_count,
        final_status: status,
        overlay_svg: None,
    }
}

#[derive(Clone, Debug)]
struct AgentImageOutput {
    uri: String,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct AgentRasterCapture {
    width: u32,
    height: u32,
    crop_origin: Option<AgentImageCropOrigin>,
    composition: AgentImageComposition,
    background: [u8; 4],
    rgba: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct AgentRasterContentStats {
    bbox: Option<AgentImageContentBBox>,
    content_pixels: u64,
}

impl AgentRasterCapture {
    fn new(width: u32, height: u32, color: [u8; 4], composition: AgentImageComposition) -> Self {
        let pixel_count = usize::try_from(width)
            .unwrap_or(0)
            .saturating_mul(usize::try_from(height).unwrap_or(0));
        let mut rgba = Vec::with_capacity(pixel_count.saturating_mul(4));
        for _ in 0..pixel_count {
            rgba.extend_from_slice(&color);
        }
        Self {
            width,
            height,
            crop_origin: None,
            composition,
            background: color,
            rgba,
        }
    }

    fn content_stats(&self) -> AgentRasterContentStats {
        let mut min_x = self.width;
        let mut min_y = self.height;
        let mut max_x = 0;
        let mut max_y = 0;
        let mut count = 0_u64;
        for y in 0..self.height {
            for x in 0..self.width {
                let index = usize::try_from(y)
                    .unwrap_or(0)
                    .saturating_mul(usize::try_from(self.width).unwrap_or(0))
                    .saturating_add(usize::try_from(x).unwrap_or(0))
                    .saturating_mul(4)
                    .saturating_add(3);
                let Some(pixel) = self
                    .rgba
                    .get(index.saturating_sub(3)..index.saturating_add(1))
                else {
                    continue;
                };
                if pixel == self.background {
                    continue;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                count = count.saturating_add(1);
            }
        }
        AgentRasterContentStats {
            bbox: (count > 0).then_some(AgentImageContentBBox {
                x: min_x,
                y: min_y,
                width: max_x.saturating_sub(min_x).saturating_add(1),
                height: max_y.saturating_sub(min_y).saturating_add(1),
            }),
            content_pixels: count,
        }
    }
}

fn agent_observe_image_output(
    report: &mut AgentObservationReport,
    options: &AgentObserveOptions,
) -> Result<Option<AgentImageOutput>, ExitCode> {
    let Some(image) = options.image else {
        return Ok(None);
    };
    match image {
        AgentObserveImageKind::Overlay => {
            let overlay_svg = {
                let selected = select_agent_capture_objects(&report.objects, options)?;
                agent_overlay_svg(&report.viewport, &selected)
            };
            let hash = hash_hex(overlay_svg.as_bytes());
            report.render_hash.clone_from(&hash);
            let uri = agent_capture_uri(report, "overlay", "svg", options);
            report.images = vec![AgentImageResource {
                kind: AgentImageKind::OverlaySvg,
                renderer: AgentImageRenderer::Native,
                scope: agent_image_scope_for_capture_scope(&agent_capture_scope_for_options(
                    options,
                )),
                composition: AgentImageComposition::OverlayVector,
                page: 0,
                uri: uri.clone(),
                mime_type: "image/svg+xml".to_owned(),
                width: report.viewport.width,
                height: report.viewport.height,
                hash,
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                written: options.out.as_deref().map(report_path),
            }];
            report.overlay_svg = Some(overlay_svg.clone());
            Ok(Some(AgentImageOutput {
                uri,
                bytes: overlay_svg.into_bytes(),
            }))
        }
        AgentObserveImageKind::RawRgba | AgentObserveImageKind::Png => {
            let request = agent_capture_request_for_options(report, image, options);
            let (mut image, bytes) = agent_native_capture_image(report, &request)?;
            image.written = options.out.as_deref().map(report_path);
            report.render_hash.clone_from(&image.hash);
            let uri = image.uri.clone();
            report.images = vec![image];
            Ok(Some(AgentImageOutput { uri, bytes }))
        }
    }
}

fn agent_capture_request_for_options(
    report: &AgentObservationReport,
    image_kind: AgentObserveImageKind,
    options: &AgentObserveOptions,
) -> AgentCaptureReadRequest {
    let capture_kind = agent_capture_kind(options);
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => "svg",
    };
    AgentCaptureReadRequest {
        uri: agent_capture_uri(report, capture_kind.resource_name(), extension, options),
        image_kind,
        capture_kind,
        scope: agent_capture_scope_for_options(options),
        page: options.page.unwrap_or(0),
        capture_time_seconds: options.capture_time_seconds,
    }
}

fn agent_capture_scope_for_options(options: &AgentObserveOptions) -> AgentCaptureScope {
    if let Some(object_id) = &options.object {
        AgentCaptureScope::Object(object_id.clone())
    } else if let Some(layer) = &options.layer {
        AgentCaptureScope::Layer(layer.clone())
    } else {
        AgentCaptureScope::Viewport
    }
}

fn agent_image_scope_for_capture_scope(scope: &AgentCaptureScope) -> AgentImageScope {
    match scope {
        AgentCaptureScope::Viewport => AgentImageScope::Viewport,
        AgentCaptureScope::Layer(id) => AgentImageScope::Layer { id: id.clone() },
        AgentCaptureScope::Object(id) => AgentImageScope::Object { id: id.clone() },
    }
}

fn select_agent_capture_objects<'a>(
    objects: &'a [AgentObservedObject],
    options: &AgentObserveOptions,
) -> Result<Vec<&'a AgentObservedObject>, ExitCode> {
    if let Some(object_id) = &options.object {
        let Some(object) = objects.iter().find(|object| object.id == *object_id) else {
            eprintln!("error: no observed object matches --object {object_id}");
            return Err(ExitCode::from(2));
        };
        return Ok(vec![object]);
    }
    if let Some(layer) = &options.layer {
        let selected = objects
            .iter()
            .filter(|object| object.layer == *layer)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            eprintln!("error: no observed object matches --layer {layer}");
            return Err(ExitCode::from(2));
        }
        return Ok(selected);
    }
    Ok(objects.iter().collect())
}

fn agent_capture_kind(options: &AgentObserveOptions) -> AgentObserveCaptureKind {
    options.capture.unwrap_or(AgentObserveCaptureKind::Color)
}

fn agent_image_kind(capture: AgentObserveCaptureKind) -> AgentImageKind {
    match capture {
        AgentObserveCaptureKind::Color => AgentImageKind::Color,
        AgentObserveCaptureKind::ObjectId => AgentImageKind::ObjectId,
        AgentObserveCaptureKind::Mask => AgentImageKind::Mask,
    }
}

impl AgentObserveCaptureKind {
    fn resource_name(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::ObjectId => "object-id",
            Self::Mask => "mask",
        }
    }
}

fn agent_object_id_color(id: &str) -> [u8; 4] {
    let color = agent_object_id_rgba_color(id);
    [color.red, color.green, color.blue, color.alpha]
}

fn agent_object_id_rgba_color(id: &str) -> AgentRgbaColor {
    let hash = blake3::hash(id.as_bytes());
    let bytes = hash.as_bytes();
    AgentRgbaColor {
        red: bytes[0].saturating_div(2).saturating_add(64),
        green: bytes[1].saturating_div(2).saturating_add(64),
        blue: bytes[2].saturating_div(2).saturating_add(64),
        alpha: 255,
    }
}

fn agent_encode_png(capture: &AgentRasterCapture) -> Result<Vec<u8>, ExitCode> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, capture.width, capture.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder
            .write_header()
            .map_err(|error| agent_png_error(&error))?;
        writer
            .write_image_data(&capture.rgba)
            .map_err(|error| agent_png_error(&error))?;
        writer.finish().map_err(|error| agent_png_error(&error))?;
    }
    Ok(bytes)
}

fn agent_png_error(error: &png::EncodingError) -> ExitCode {
    eprintln!("error: failed to encode PNG capture: {error}");
    ExitCode::FAILURE
}

fn agent_capture_uri(
    report: &AgentObservationReport,
    default_name: &str,
    extension: &str,
    options: &AgentObserveOptions,
) -> String {
    let name = if let Some(object_id) = &options.object {
        agent_scoped_capture_name("object", object_id, default_name)
    } else if let Some(layer) = &options.layer {
        agent_scoped_capture_name("layer", layer, default_name)
    } else {
        default_name.to_owned()
    };
    agent_frame_capture_uri_for_page(
        &report.session_id,
        report.tick,
        &name,
        extension,
        options.page.unwrap_or(0),
    )
}

fn agent_frame_capture_uri(session_id: &str, tick: usize, name: &str, extension: &str) -> String {
    agent_frame_capture_uri_for_page(session_id, tick, name, extension, 0)
}

fn agent_frame_capture_uri_for_page(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    page: usize,
) -> String {
    let base = agent_frame_capture_uri_base(session_id, tick, name, extension);
    if page == 0 {
        return base;
    }
    format!("{base}?page={page}")
}

fn agent_frame_capture_uri_base(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
) -> String {
    format!("arcweft://session/{session_id}/frame/{tick}/{name}.{extension}")
}

fn agent_scoped_capture_name(prefix: &str, scope: &str, default_name: &str) -> String {
    let scope = agent_uri_component(scope);
    if default_name == "color" {
        format!("{prefix}.{scope}")
    } else {
        format!("{prefix}.{scope}.{default_name}")
    }
}

fn agent_uri_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn agent_textbox_object(
    step: usize,
    index: usize,
    frame: LineDisplayFrame,
    viewport: &AgentViewport,
) -> AgentObservedObject {
    let width = viewport.width.saturating_sub(192);
    let lines = u32::try_from(frame.text.lines().count().max(1)).unwrap_or(u32::MAX);
    let height = (96 + lines * 28).min(220);
    let object_slot = u32::try_from(index % 4).unwrap_or(0);
    let bottom_margin = 48 + object_slot * 10;
    let y = viewport
        .height
        .saturating_sub(height)
        .saturating_sub(bottom_margin);
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 96,
        y,
        width,
        height,
    };
    let object_id = format!("object.dialogue.{step}.{index}");
    let capture_refs = agent_object_capture_refs("cli", step, &object_id, &bbox);
    AgentObservedObject {
        id: object_id,
        entity: Some(frame.callee.clone()),
        layer: "dialogue".to_owned(),
        role: "textbox".to_owned(),
        visible: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs,
        text: Some(frame.text.clone()),
        rich_text_ref: None,
        rich_text: frame,
    }
}

fn agent_rich_text_child_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_player_native::native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    for (run_index, run) in textbox.rich_text.display_map.text_runs.iter().enumerate() {
        if matches!(
            run.source,
            RichTextTextSource::ControlHardBreak | RichTextTextSource::ControlRaw
        ) {
            continue;
        }
        if let Some(object) =
            agent_rich_text_run_object(step, index, run_index, textbox, run, native_bounds)
        {
            children.push(object);
        }
    }
    for (ruby_index, ruby) in textbox
        .rich_text
        .display_map
        .ruby_annotations
        .iter()
        .enumerate()
    {
        if let Some(object) =
            agent_rich_text_ruby_object(step, index, ruby_index, textbox, ruby, native_bounds)
        {
            children.push(object);
        }
    }
    children.extend(agent_rich_text_cluster_objects(
        step,
        index,
        textbox,
        native_bounds,
    ));
    children
}

fn agent_native_rich_text_element_bboxes(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
) -> BTreeMap<arcweft_player_native::native::NativeFrameElement, AgentNativeRichTextElementBounds> {
    let (left, top) = agent_native_text_origin(textbox);
    let mut bboxes = BTreeMap::new();
    for page_index in 0.. {
        let bounds = match arcweft_player_native::native::measure_frame_elements_at_page(
            &textbox.rich_text,
            viewport.width,
            viewport.height,
            left,
            top,
            page_index,
        ) {
            Ok(bounds) => bounds,
            Err(arcweft_player_native::native::NativeWindowError::EmptyPages) => break,
            Err(_) => return BTreeMap::new(),
        };
        for bounds in bounds {
            bboxes
                .entry(bounds.element)
                .or_insert(AgentNativeRichTextElementBounds {
                    bbox: agent_bbox_from_native(bounds.bbox),
                    glyph: bounds.glyph,
                    ruby: bounds.ruby.map(agent_ruby_geometry_from_native),
                });
        }
    }
    bboxes
}

#[derive(Clone, Debug)]
struct AgentNativeRichTextElementBounds {
    bbox: AgentBBox,
    glyph: Option<arcweft_player_native::native::NativeGlyphClusterMetadata>,
    ruby: Option<AgentRubyElementGeometry>,
}

#[derive(Clone, Debug)]
struct AgentRubyElementGeometry {
    base_bbox: AgentBBox,
    annotation_bbox: AgentBBox,
}

fn agent_rich_text_run_object(
    step: usize,
    index: usize,
    run_index: usize,
    textbox: &AgentObservedObject,
    run: &RichTextTextRun,
    native_bounds: &BTreeMap<
        arcweft_player_native::native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let text = textbox
        .rich_text
        .text
        .get(valid_rich_text_range(run.range, &textbox.rich_text.text)?)?;
    if text.trim().is_empty() {
        return None;
    }
    let bbox = native_bounds
        .get(&arcweft_player_native::native::NativeFrameElement::TextRun { index: run_index })
        .map(|bounds| bounds.bbox.clone())?;
    let object_id = format!("object.dialogue.{step}.{index}.run.{run_index}");
    let page = agent_rich_text_page_for_range(&textbox.rich_text, run.range);
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            role: "rich_text_run",
            text: text.to_owned(),
            bbox: &bbox,
            rich_text_ref: AgentRichTextElementRef {
                kind: AgentRichTextElementKind::TextRun,
                index: run_index,
                page,
                range: run.range,
                node_index: run.node_index,
                source: Some(run.source),
                ruby: None,
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: None,
                ruby_annotation_bbox: None,
                hit_regions: vec![agent_hit_region(
                    AgentHitRegionKind::TextRun,
                    &bbox,
                    run.range,
                )],
            },
            page,
        },
    ))
}

fn agent_rich_text_ruby_object(
    step: usize,
    index: usize,
    ruby_index: usize,
    textbox: &AgentObservedObject,
    ruby: &RichTextRubyAnnotation,
    native_bounds: &BTreeMap<
        arcweft_player_native::native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let base_range = valid_rich_text_range(ruby.base_range, &textbox.rich_text.text)?;
    let base_text = textbox.rich_text.text.get(base_range)?;
    let bbox = native_bounds
        .get(&arcweft_player_native::native::NativeFrameElement::Ruby { index: ruby_index })
        .cloned()?;
    let object_id = format!("object.dialogue.{step}.{index}.ruby.{ruby_index}");
    let page = agent_rich_text_page_for_range(&textbox.rich_text, ruby.base_range);
    let hit_regions = agent_ruby_hit_regions(&bbox, ruby.base_range);
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            role: "rich_text_ruby",
            text: format!("{base_text} ({})", ruby.ruby),
            bbox: &bbox.bbox,
            rich_text_ref: AgentRichTextElementRef {
                kind: AgentRichTextElementKind::Ruby,
                index: ruby_index,
                page,
                range: ruby.base_range,
                node_index: ruby.node_index,
                source: None,
                ruby: Some(ruby.ruby.clone()),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: bbox.ruby.as_ref().map(|ruby| ruby.base_bbox.clone()),
                ruby_annotation_bbox: bbox.ruby.as_ref().map(|ruby| ruby.annotation_bbox.clone()),
                hit_regions,
            },
            page,
        },
    ))
}

fn agent_rich_text_cluster_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_player_native::native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    native_bounds
        .iter()
        .filter_map(|(element, bounds)| {
            let arcweft_player_native::native::NativeFrameElement::GlyphCluster {
                index: cluster_index,
                range_start,
                range_end,
            } = *element
            else {
                return None;
            };
            let range = RichTextRange::new(range_start, range_end);
            let text = textbox
                .rich_text
                .text
                .get(valid_rich_text_range(range, &textbox.rich_text.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let run = textbox
                .rich_text
                .display_map
                .text_runs
                .iter()
                .find(|run| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.cluster.{cluster_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(&textbox.rich_text, range);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    role: "rich_text_cluster",
                    text: text.to_owned(),
                    bbox: &bounds.bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::GlyphCluster,
                        index: cluster_index,
                        page,
                        range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        orientation: bounds
                            .glyph
                            .map(|glyph| agent_glyph_orientation_from_native(glyph.orientation)),
                        vertical_form: bounds.glyph.map(|glyph| {
                            agent_glyph_vertical_form_from_native(glyph.vertical_form)
                        }),
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        hit_regions: vec![agent_hit_region(
                            AgentHitRegionKind::GlyphCluster,
                            &bounds.bbox,
                            range,
                        )],
                    },
                    page,
                },
            ))
        })
        .collect()
}

fn agent_bbox_from_native(
    bbox: arcweft_player_native::native::NativeFrameContentBBox,
) -> AgentBBox {
    AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: bbox.x,
        y: bbox.y,
        width: bbox.width,
        height: bbox.height,
    }
}

fn agent_hit_region(
    kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
) -> AgentHitRegion {
    AgentHitRegion {
        kind,
        bbox: bbox.clone(),
        range,
    }
}

fn agent_ruby_hit_regions(
    bounds: &AgentNativeRichTextElementBounds,
    range: RichTextRange,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_hit_region(
        AgentHitRegionKind::RubyObject,
        &bounds.bbox,
        range,
    )];
    if let Some(ruby) = &bounds.ruby {
        regions.push(agent_hit_region(
            AgentHitRegionKind::RubyBase,
            &ruby.base_bbox,
            range,
        ));
        regions.push(agent_hit_region(
            AgentHitRegionKind::RubyAnnotation,
            &ruby.annotation_bbox,
            range,
        ));
    }
    regions
}

fn agent_ruby_geometry_from_native(
    value: arcweft_player_native::native::NativeRubyElementGeometry,
) -> AgentRubyElementGeometry {
    AgentRubyElementGeometry {
        base_bbox: agent_bbox_from_native(value.base_bbox),
        annotation_bbox: agent_bbox_from_native(value.annotation_bbox),
    }
}

const fn agent_glyph_orientation_from_native(
    value: arcweft_player_native::native::NativeGlyphOrientation,
) -> AgentGlyphOrientation {
    match value {
        arcweft_player_native::native::NativeGlyphOrientation::Upright => {
            AgentGlyphOrientation::Upright
        }
        arcweft_player_native::native::NativeGlyphOrientation::SidewaysCw => {
            AgentGlyphOrientation::SidewaysCw
        }
        arcweft_player_native::native::NativeGlyphOrientation::TextCombineUpright => {
            AgentGlyphOrientation::TextCombineUpright
        }
    }
}

const fn agent_glyph_vertical_form_from_native(
    value: arcweft_player_native::native::NativeGlyphVerticalForm,
) -> AgentGlyphVerticalForm {
    match value {
        arcweft_player_native::native::NativeGlyphVerticalForm::None => {
            AgentGlyphVerticalForm::None
        }
        arcweft_player_native::native::NativeGlyphVerticalForm::UprightAlternate => {
            AgentGlyphVerticalForm::UprightAlternate
        }
        arcweft_player_native::native::NativeGlyphVerticalForm::RotatedAlternate => {
            AgentGlyphVerticalForm::RotatedAlternate
        }
    }
}

struct AgentRichTextChildObjectSpec<'a> {
    object_id: &'a str,
    role: &'a str,
    text: String,
    bbox: &'a AgentBBox,
    rich_text_ref: AgentRichTextElementRef,
    page: usize,
}

fn agent_rich_text_child_object(
    step: usize,
    textbox: &AgentObservedObject,
    spec: AgentRichTextChildObjectSpec<'_>,
) -> AgentObservedObject {
    AgentObservedObject {
        id: spec.object_id.to_owned(),
        entity: textbox.entity.clone(),
        layer: "dialogue.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: textbox.visible,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_object_capture_refs_for_page(
            "cli",
            step,
            spec.object_id,
            spec.bbox,
            spec.page,
        ),
        text: Some(spec.text.clone()),
        rich_text_ref: Some(spec.rich_text_ref),
        rich_text: agent_child_line_display_frame(&textbox.rich_text, spec.text),
    }
}

fn agent_rich_text_page_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> usize {
    let Some(valid_range) = valid_rich_text_range(range, &frame.text) else {
        return 0;
    };
    agent_rich_text_page_ranges(frame)
        .into_iter()
        .filter(|page_range| !page_range.is_empty())
        .position(|page_range| {
            valid_range.start >= page_range.start && valid_range.end <= page_range.end
        })
        .unwrap_or(0)
}

fn agent_rich_text_page_ranges(frame: &LineDisplayFrame) -> Vec<std::ops::Range<usize>> {
    let mut break_offsets = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::Page | RichTextControl::LineWait | RichTextControl::Clear
            )
        })
        .map(|marker| agent_display_map_offset_before_node(frame, marker.node_index))
        .filter(|offset| *offset <= frame.text.len() && frame.text.is_char_boundary(*offset))
        .collect::<Vec<_>>();
    break_offsets.sort_unstable();
    break_offsets.dedup();

    let mut start = 0;
    let mut ranges = Vec::with_capacity(break_offsets.len() + 1);
    for end in break_offsets {
        if start <= end {
            ranges.push(start..end);
            start = end;
        }
    }
    ranges.push(start..frame.text.len());
    ranges
}

fn agent_display_map_offset_before_node(frame: &LineDisplayFrame, node_index: usize) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| run.node_index < node_index)
        .map(|run| run.range.end)
        .max()
        .unwrap_or(0)
}

fn agent_child_line_display_frame(parent: &LineDisplayFrame, text: String) -> LineDisplayFrame {
    LineDisplayFrame {
        line: parent.line.clone(),
        callee: parent.callee.clone(),
        text: text.clone(),
        base_styles: parent.base_styles.clone(),
        default_inline_failure_policy: parent.default_inline_failure_policy.clone(),
        nodes: vec![RichTextNode::Text { text }],
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn valid_rich_text_range(range: RichTextRange, text: &str) -> Option<std::ops::Range<usize>> {
    if range.start <= range.end
        && range.end <= text.len()
        && text.is_char_boundary(range.start)
        && text.is_char_boundary(range.end)
    {
        Some(range.start..range.end)
    } else {
        None
    }
}

#[derive(Clone, Debug)]
struct AgentLayerAccumulator {
    visible: bool,
    bbox: AgentBBox,
    object_count: usize,
}

fn agent_observed_layers(
    session_id: &str,
    tick: usize,
    objects: &[AgentObservedObject],
) -> Vec<AgentObservedLayer> {
    let mut layers = BTreeMap::<String, AgentLayerAccumulator>::new();
    for object in objects {
        layers
            .entry(object.layer.clone())
            .and_modify(|layer| {
                layer.visible |= object.visible;
                layer.object_count = layer.object_count.saturating_add(1);
                layer.bbox = agent_union_bbox(&layer.bbox, &object.bbox);
            })
            .or_insert_with(|| AgentLayerAccumulator {
                visible: object.visible,
                bbox: object.bbox.clone(),
                object_count: 1,
            });
    }
    layers
        .into_iter()
        .map(|(id, layer)| AgentObservedLayer {
            capture_refs: agent_layer_capture_refs(session_id, tick, &id, &layer.bbox),
            id,
            visible: layer.visible,
            bbox: layer.bbox,
            object_count: layer.object_count,
        })
        .collect()
}

fn agent_union_bbox(left: &AgentBBox, right: &AgentBBox) -> AgentBBox {
    let x = left.x.min(right.x);
    let y = left.y.min(right.y);
    let max_x = left
        .x
        .saturating_add(left.width)
        .max(right.x.saturating_add(right.width));
    let max_y = left
        .y
        .saturating_add(left.height)
        .max(right.y.saturating_add(right.height));
    AgentBBox {
        space: left.space,
        x,
        y,
        width: max_x.saturating_sub(x).max(1),
        height: max_y.saturating_sub(y).max(1),
    }
}

fn agent_layer_capture_refs(
    session_id: &str,
    tick: usize,
    layer_id: &str,
    bbox: &AgentBBox,
) -> AgentLayerCaptureRefs {
    let name = agent_scoped_capture_name("layer", layer_id, "color");
    let object_id_name = agent_scoped_capture_name("layer", layer_id, "object-id");
    let mask_name = agent_scoped_capture_name("layer", layer_id, "mask");
    AgentLayerCaptureRefs {
        captures: vec![
            agent_layer_capture_ref(session_id, tick, &name, "png", AgentImageKind::Color, bbox),
            agent_layer_capture_ref(session_id, tick, &name, "rgba", AgentImageKind::Color, bbox),
            agent_layer_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "png",
                AgentImageKind::ObjectId,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "rgba",
                AgentImageKind::ObjectId,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &mask_name,
                "png",
                AgentImageKind::Mask,
                bbox,
            ),
            agent_layer_capture_ref(
                session_id,
                tick,
                &mask_name,
                "rgba",
                AgentImageKind::Mask,
                bbox,
            ),
        ],
    }
}

fn agent_layer_capture_ref(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    kind: AgentImageKind,
    bbox: &AgentBBox,
) -> AgentLayerCaptureRef {
    AgentLayerCaptureRef {
        kind,
        uri: agent_frame_capture_uri(session_id, tick, name, extension),
        mime_type: agent_capture_mime_type(extension).to_owned(),
        page: 0,
        width: bbox.width.max(1),
        height: bbox.height.max(1),
    }
}

fn agent_object_capture_refs(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
) -> AgentObjectCaptureRefs {
    agent_object_capture_refs_for_page(session_id, tick, object_id, bbox, 0)
}

fn agent_object_capture_refs_for_page(
    session_id: &str,
    tick: usize,
    object_id: &str,
    bbox: &AgentBBox,
    page: usize,
) -> AgentObjectCaptureRefs {
    let name = agent_scoped_capture_name("object", object_id, "color");
    let object_id_name = agent_scoped_capture_name("object", object_id, "object-id");
    let mask_name = agent_scoped_capture_name("object", object_id, "mask");
    AgentObjectCaptureRefs {
        object_id_color: agent_object_id_rgba_color(object_id),
        captures: vec![
            agent_object_capture_ref(
                session_id,
                tick,
                &name,
                "png",
                AgentImageKind::Color,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &name,
                "rgba",
                AgentImageKind::Color,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "png",
                AgentImageKind::ObjectId,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &object_id_name,
                "rgba",
                AgentImageKind::ObjectId,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &mask_name,
                "png",
                AgentImageKind::Mask,
                bbox,
                page,
            ),
            agent_object_capture_ref(
                session_id,
                tick,
                &mask_name,
                "rgba",
                AgentImageKind::Mask,
                bbox,
                page,
            ),
        ],
    }
}

fn agent_object_capture_ref(
    session_id: &str,
    tick: usize,
    name: &str,
    extension: &str,
    kind: AgentImageKind,
    bbox: &AgentBBox,
    page: usize,
) -> AgentObjectCaptureRef {
    AgentObjectCaptureRef {
        kind,
        uri: agent_frame_capture_uri_for_page(session_id, tick, name, extension, page),
        mime_type: agent_capture_mime_type(extension).to_owned(),
        page,
        width: bbox.width.max(1),
        height: bbox.height.max(1),
    }
}

fn agent_capture_mime_type(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}

fn agent_overlay_svg(viewport: &AgentViewport, objects: &[&AgentObservedObject]) -> String {
    let mut svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 {} {}"><rect width="100%" height="100%" fill="#101418"/>"##,
        viewport.width, viewport.height, viewport.width, viewport.height
    );
    for object in objects {
        let _ = write!(
            svg,
            r##"<rect x="{}" y="{}" width="{}" height="{}" rx="8" fill="#1f2630" stroke="#76d7c4" stroke-width="2"/>"##,
            object.bbox.x, object.bbox.y, object.bbox.width, object.bbox.height
        );
        if let Some(text) = &object.text {
            let escaped = escape_xml(text);
            let _ = write!(
                svg,
                r##"<text x="{}" y="{}" fill="#f4f7fb" font-family="sans-serif" font-size="24">{}</text>"##,
                object.bbox.x + 24,
                object.bbox.y + 48,
                escaped
            );
        }
    }
    svg.push_str("</svg>");
    svg
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn hash_hex(bytes: &[u8]) -> String {
    blake3::hash(bytes).to_hex().to_string()
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
        pure_object_artifacts: options.pure_object_artifacts,
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
        options.pure_object_artifacts,
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
    bytecode: BytecodeProgram,
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
    let plan = bytecode.clone().into_runtime_plan().map_err(|error| {
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
        bytecode,
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
    try_run_runtime_steps_with_executor(executor, host_config, steps, mode, max_ops, values)
        .map_err(|error| {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        })
}

fn try_run_runtime_steps_with_executor(
    executor: &mut RuntimeExecutorInstance,
    host_config: NativeRunHost<'_>,
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
) -> Result<RuntimeRunTrace, arcweft_host_adapter::HostAdapterError> {
    let mut host = host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                host_config.policy.clone(),
                host_config.adapter_registrars,
            )
        })
        .transpose()?;
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

#[derive(serde::Serialize)]
struct BundleCommandReport {
    bundle: String,
    source: String,
    required_host_calls: Vec<String>,
    adapter_manifests: usize,
    bytecode_instructions: usize,
    virtual_files: usize,
    phases: Vec<RuntimeProfilePhase>,
    runtime: BundleRuntimeSummary,
}

#[derive(serde::Serialize)]
struct BundleRunReport {
    bundle: String,
    source: String,
    bytecode_instructions: usize,
    adapter_manifests: usize,
    phases: Vec<BundleRunnerPhase>,
    executor: RuntimeExecutorTier,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
    steps: Vec<BundleRunnerStepSummary>,
    final_status: String,
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
        options.pure_object_artifacts,
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
        options.pure_object_artifacts,
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
        options.pure_object_artifacts,
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

fn bundle_command(options: &BundleOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let mut phases = Vec::new();
    let bundle = compile_bundle_artifact(&selection, options, &mut phases)?;
    let bytes = run_profile_phase(&mut phases, "encode_bundle", || {
        bundle.to_json_bytes().map_err(|error| {
            eprintln!("error: failed to encode bundle: {error}");
            ExitCode::FAILURE
        })
    })?;
    write_bundle_artifact(&options.output, bytes, &mut phases)?;
    if options.json {
        print_json(&bundle_command_report(&options.output, &bundle, phases))
    } else {
        println!(
            "ok: {} (source={}, {} virtual file(s))",
            options.output.display(),
            bundle.manifest.source_label,
            bundle.virtual_files.len()
        );
        Ok(())
    }
}

fn compile_bundle_artifact(
    selection: &SourceSelection,
    options: &BundleOptions,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<ArcweftBundle, ExitCode> {
    let env = typecheck_env_for_selection(selection, None, phases)?;
    let compiled = compile_profile_runtime_plan(selection, &env, phases)?;
    let source = fs::read_to_string(selection.path()).map_err(|error| {
        eprintln!(
            "error: failed to read bundle source {}: {error}",
            selection.path().display()
        );
        ExitCode::FAILURE
    })?;
    let source_label = report_path(selection.path());
    let required_host_calls = bundle_required_host_calls(&compiled.plan);
    let adapter_manifest = adapter_manifest_for_selection(selection, None)?;
    let adapter_manifest_ids = bundle_adapter_manifest_ids(
        adapter_manifest.id().as_str(),
        required_host_calls.iter().map(String::as_str),
    );
    let adapter_manifests = bundle_adapter_manifests(
        &adapter_manifest,
        required_host_calls.iter().map(String::as_str),
    );
    Ok(ArcweftBundle::new(
        bundle_manifest(
            selection,
            source_label.clone(),
            &compiled,
            adapter_manifest_ids,
            required_host_calls,
        ),
        BundleSource {
            label: source_label,
            text: source,
        },
        compiled.bytecode,
    )
    .with_adapter_manifests(adapter_manifests)
    .with_virtual_files(collect_bundle_virtual_files(
        selection.path(),
        options.include_spaces(),
    )?))
}

fn bundle_required_host_calls(plan: &RuntimePlan) -> Vec<String> {
    let mut required_host_calls = plan
        .flows
        .iter()
        .flat_map(|flow| flow.ops.iter())
        .flat_map(collect_flow_op_host_calls)
        .collect::<Vec<_>>();
    required_host_calls.sort();
    required_host_calls.dedup();
    required_host_calls
}

fn bundle_manifest(
    selection: &SourceSelection,
    source_label: String,
    compiled: &ProfileCompiledRuntimePlan,
    adapter_manifest_ids: Vec<String>,
    required_host_calls: Vec<String>,
) -> BundleManifest {
    BundleManifest {
        source_label,
        profile_id: selection
            .profile()
            .map(|profile| profile.id().as_str().to_owned()),
        profile_kind: selection
            .profile()
            .map(|profile| bundle_launch_kind(profile.kind())),
        entry: selection.entry().map(str::to_owned),
        adapter: selection.adapter().map(str::to_owned),
        adapter_manifest_ids,
        required_host_calls,
        runtime: BundleRuntimeSummary {
            entry_flow: compiled.plan.entry_flow.as_ref().map(|flow| flow.0.clone()),
            flows: compiled.bytecode_stats.flows,
            bytecode_instructions: compiled.bytecode_stats.instructions,
            line_task_groups: compiled.bytecode_stats.line_task_groups,
            stream_plans: compiled.bytecode_stats.stream_plans,
            source_plans: compiled.bytecode_stats.source_plans,
        },
    }
}

fn write_bundle_artifact(
    output: &Path,
    bytes: Vec<u8>,
    phases: &mut Vec<RuntimeProfilePhase>,
) -> Result<(), ExitCode> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!(
                "error: failed to create bundle output directory {}: {error}",
                parent.display()
            );
            ExitCode::FAILURE
        })?;
    }
    run_profile_phase(phases, "write_bundle", || {
        fs::write(output, bytes).map_err(|error| {
            eprintln!(
                "error: failed to write bundle {}: {error}",
                output.display()
            );
            ExitCode::FAILURE
        })
    })
}

fn bundle_command_report(
    output: &Path,
    bundle: &ArcweftBundle,
    phases: Vec<RuntimeProfilePhase>,
) -> BundleCommandReport {
    BundleCommandReport {
        bundle: report_path(output),
        source: bundle.manifest.source_label.clone(),
        required_host_calls: bundle.manifest.required_host_calls.clone(),
        adapter_manifests: bundle.adapter_manifests.len(),
        bytecode_instructions: bundle.manifest.runtime.bytecode_instructions,
        virtual_files: bundle.virtual_files.len(),
        phases,
        runtime: bundle.manifest.runtime.clone(),
    }
}

fn run_bundle_command(
    options: &RunBundleOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let runner_options = BundleRunnerOptions::from(options);
    let execution =
        run_bundle_file_with_native_adapters(&options.bundle, &runner_options, adapter_registrars)
            .map_err(|error| {
                eprintln!("error: {error}");
                bundle_runner_error_exit_code(&error)
            })?;
    let report = BundleRunReport {
        bundle: report_path(&options.bundle),
        source: execution.source,
        bytecode_instructions: execution.bytecode_instructions,
        adapter_manifests: execution.adapter_manifests,
        phases: execution.phases,
        executor: RuntimeExecutorTier::from(CliRuntimeExecutorTier::from(execution.executor)),
        executor_stats: execution.executor_stats,
        native_io: execution.native_io,
        steps: execution.steps,
        final_status: execution.final_status,
    };
    if options.json {
        print_json(&report)
    } else {
        println!(
            "ok: {} ({} step(s), final_status={})",
            options.bundle.display(),
            report.steps.len(),
            report.final_status
        );
        Ok(())
    }
}

fn bundle_runner_error_exit_code(error: &BundleRunnerError) -> ExitCode {
    match error {
        BundleRunnerError::ConflictingEntrySelection => ExitCode::from(2),
        BundleRunnerError::ReadBundle { .. }
        | BundleRunnerError::DecodeBundle(_)
        | BundleRunnerError::DecodeBytecode(_)
        | BundleRunnerError::CreateWorkspace(_)
        | BundleRunnerError::CreateSourceDirectory(_)
        | BundleRunnerError::MaterializeSource(_)
        | BundleRunnerError::CreateVirtualFileDirectory(_)
        | BundleRunnerError::MaterializeVirtualFile(_)
        | BundleRunnerError::InvalidVirtualFilePath
        | BundleRunnerError::UnknownFlow { .. }
        | BundleRunnerError::UnknownEntry { .. }
        | BundleRunnerError::NonFlowEntry { .. }
        | BundleRunnerError::NativeAdapter(_) => ExitCode::FAILURE,
    }
}

fn collect_flow_op_host_calls(op: &FlowOp) -> Vec<String> {
    match op {
        FlowOp::Await { target, .. } => vec![host_call_id_for_template(
            target.request.capability.0.as_str(),
            target.request.operation.as_str(),
        )],
        FlowOp::AwaitMany { target, .. } => vec![host_call_id_for_template(
            target.request.capability.0.as_str(),
            target.request.operation.as_str(),
        )],
        FlowOp::LetElse { else_ops, .. } => collect_flow_ops_host_calls(else_ops),
        FlowOp::If {
            then_ops, else_ops, ..
        }
        | FlowOp::IfLet {
            then_ops, else_ops, ..
        } => collect_flow_ops_host_calls(then_ops)
            .into_iter()
            .chain(collect_flow_ops_host_calls(else_ops))
            .collect(),
        FlowOp::Match { arms, .. } => arms
            .iter()
            .flat_map(|arm| collect_flow_ops_host_calls(&arm.ops))
            .collect(),
        FlowOp::Loop { body }
        | FlowOp::LetLoop { body, .. }
        | FlowOp::While { body, .. }
        | FlowOp::WhileLet { body, .. }
        | FlowOp::For { body, .. }
        | FlowOp::Thread { body, .. } => {
            let mut calls = collect_flow_ops_host_calls(body);
            if matches!(op, FlowOp::Thread { .. }) {
                calls.push("flow_thread.run_child".to_owned());
            }
            calls
        }
        FlowOp::LoopNext { body }
        | FlowOp::WhileNext { body, .. }
        | FlowOp::WhileLetNext { body, .. }
        | FlowOp::ForNext { body, .. } => collect_flow_ops_host_calls(body.as_ref().iter()),
        FlowOp::Scope(ops) | FlowOp::LetScope { ops, .. } => collect_flow_ops_host_calls(ops),
        FlowOp::Bind(_)
        | FlowOp::Let { .. }
        | FlowOp::Dialogue { .. }
        | FlowOp::Choice { .. }
        | FlowOp::Break(_)
        | FlowOp::Continue
        | FlowOp::Goto(_)
        | FlowOp::GotoExpr(_)
        | FlowOp::Return(_)
        | FlowOp::ReturnExpr(_)
        | FlowOp::Effect(_)
        | FlowOp::EnterScope
        | FlowOp::ExitScope
        | FlowOp::ExitScopeBind { .. }
        | FlowOp::Noop => Vec::new(),
    }
}

fn collect_flow_ops_host_calls<'a>(ops: impl IntoIterator<Item = &'a FlowOp>) -> Vec<String> {
    ops.into_iter()
        .flat_map(collect_flow_op_host_calls)
        .collect()
}

fn host_call_id_for_template(capability: &str, operation: &str) -> String {
    format!("{capability}.{operation}")
}

fn bundle_adapter_manifest_ids<'a>(
    selected_adapter_id: &str,
    required_host_calls: impl IntoIterator<Item = &'a str>,
) -> Vec<String> {
    let mut ids = std::iter::once(selected_adapter_id)
        .chain(required_host_calls.into_iter().filter_map(|host_call| {
            host_call
                .strip_prefix("fs.")
                .map(|_| standard::NATIVE_FILE_ADAPTER_ID)
                .or_else(|| {
                    host_call
                        .strip_prefix("system.")
                        .map(|_| standard::SYSTEM_INFO_ADAPTER_ID)
                })
                .or_else(|| {
                    matches!(host_call, "line_task.run_child" | "flow_thread.run_child")
                        .then_some(INTERNAL_SCHEDULER_ADAPTER_ID)
                })
        }))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    ids.sort();
    ids.dedup();
    ids
}

fn bundle_adapter_manifests<'a>(
    selected: &AdapterManifest,
    required_host_calls: impl IntoIterator<Item = &'a str>,
) -> Vec<BundleAdapterManifest> {
    let required = required_host_calls.into_iter().collect::<Vec<_>>();
    let mut manifests = vec![bundle_adapter_manifest_from_context(selected)];
    if required
        .iter()
        .any(|host_call| host_call.starts_with("fs."))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &standard::native_file_manifest(),
        ));
    }
    if required
        .iter()
        .any(|host_call| host_call.starts_with("system."))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &standard::system_info_manifest(),
        ));
    }
    if required
        .iter()
        .any(|host_call| matches!(*host_call, "line_task.run_child" | "flow_thread.run_child"))
    {
        manifests.push(bundle_adapter_manifest_from_context(
            &internal_scheduler_manifest(),
        ));
    }
    manifests.sort_by(|left, right| left.id.cmp(&right.id));
    manifests.dedup_by(|left, right| left.id == right.id);
    manifests
}

fn bundle_adapter_manifest_from_context(manifest: &AdapterManifest) -> BundleAdapterManifest {
    BundleAdapterManifest {
        id: manifest.id().as_str().to_owned(),
        display_name: manifest.display_name().to_owned(),
        effects: manifest
            .effects()
            .iter()
            .map(|effect| effect.as_str().to_owned())
            .collect(),
        host_calls: manifest
            .host_calls()
            .iter()
            .map(|host_call| BundleAdapterHostCall {
                id: host_call.id().to_owned(),
                effects: host_call
                    .effects()
                    .iter()
                    .map(|effect| effect.as_str().to_owned())
                    .collect(),
            })
            .collect(),
    }
}

fn collect_bundle_virtual_files(
    source_path: &Path,
    spaces: impl IntoIterator<Item = BundleVirtualFileSpace>,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    let root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".arcweft");
    spaces
        .into_iter()
        .map(|space| collect_bundle_virtual_files_for_space(&root, space))
        .collect::<Result<Vec<_>, _>>()
        .map(|groups| groups.into_iter().flatten().collect())
}

fn collect_bundle_virtual_files_for_space(
    root: &Path,
    space: BundleVirtualFileSpace,
) -> Result<Vec<BundleVirtualFile>, ExitCode> {
    let dir = root.join(space.as_str());
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    collect_bundle_virtual_files_from_dir(&dir, &dir, space, &mut files)?;
    Ok(files)
}

fn collect_bundle_virtual_files_from_dir(
    root: &Path,
    dir: &Path,
    space: BundleVirtualFileSpace,
    files: &mut Vec<BundleVirtualFile>,
) -> Result<(), ExitCode> {
    let entries = fs::read_dir(dir).map_err(|error| {
        eprintln!(
            "error: failed to read virtual file directory {}: {error}",
            dir.display()
        );
        ExitCode::FAILURE
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            eprintln!("error: failed to read virtual file entry: {error}");
            ExitCode::FAILURE
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_bundle_virtual_files_from_dir(root, &path, space, files)?;
        } else if path.is_file() {
            let relative = normalized_relative_path(root, &path)?;
            let bytes = fs::read(&path).map_err(|error| {
                eprintln!(
                    "error: failed to read virtual file {}: {error}",
                    path.display()
                );
                ExitCode::FAILURE
            })?;
            files.push(BundleVirtualFile {
                space,
                path: relative,
                bytes,
            });
        }
    }
    Ok(())
}

fn normalized_relative_path(root: &Path, path: &Path) -> Result<String, ExitCode> {
    let relative = path.strip_prefix(root).map_err(|error| {
        eprintln!(
            "error: virtual file {} is outside {}: {error}",
            path.display(),
            root.display()
        );
        ExitCode::FAILURE
    })?;
    validate_relative_virtual_path(relative)?;
    Ok(relative
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/"))
}

fn validate_relative_virtual_path(path: &Path) -> Result<(), ExitCode> {
    let valid = path
        .components()
        .all(|component| matches!(component, Component::Normal(_)));
    if valid {
        Ok(())
    } else {
        eprintln!("error: bundle virtual file path must be relative and normalized");
        Err(ExitCode::FAILURE)
    }
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
        options.pure_object_artifacts,
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
        fused_matmul_bias_add_calls: median_executor_math_field(samples, |math| {
            math.fused_matmul_bias_add_calls
        }),
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
struct AgentObserveOptions {
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
    #[arg(long, default_value_t = 8)]
    steps: usize,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long, value_enum)]
    image: Option<AgentObserveImageKind>,
    #[arg(long, value_enum)]
    capture: Option<AgentObserveCaptureKind>,
    #[arg(long)]
    layer: Option<String>,
    #[arg(long)]
    object: Option<String>,
    #[arg(long)]
    page: Option<usize>,
    #[arg(long = "capture-time", default_value_t = 60.0)]
    capture_time_seconds: f32,
    #[arg(long, value_enum)]
    resource: Option<AgentObserveResourceKind>,
    #[arg(long)]
    read_uri: Option<String>,
    #[arg(long)]
    mcp: bool,
    #[arg(long, value_enum, default_value_t = AgentObserveMcpFormat::Read)]
    mcp_format: AgentObserveMcpFormat,
    #[arg(long)]
    out: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
struct AgentMcpOptions {}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AgentObserveImageKind {
    Overlay,
    RawRgba,
    Png,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AgentObserveCaptureKind {
    Color,
    ObjectId,
    Mask,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_line_display_frame() -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("line.test".to_owned()),
            callee: "test".to_owned(),
            text: String::new(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap::default(),
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    fn test_resolved_line_display_frame() -> LineDisplayFrame {
        let spec = arcweft_render_text::LineDisplaySpec {
            line: arcweft_core::plan::RuntimeLineId("line.test".to_owned()),
            callee: "test".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            args: Vec::new(),
            content: arcweft_render_text::RichTextDocument::new(vec![RichTextNode::Text {
                text: "native attachment seed".to_owned(),
            }]),
        };
        spec.resolve_frame(&RuntimeLineContext::default())
            .expect("test frame resolves")
    }

    fn test_observed_object(
        id: &str,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> AgentObservedObject {
        let bbox = AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x,
            y,
            width,
            height,
        };
        AgentObservedObject {
            id: id.to_owned(),
            entity: None,
            layer: "ui".to_owned(),
            role: "panel".to_owned(),
            visible: true,
            polygon: bbox.polygon(),
            bbox,
            capture_refs: AgentObjectCaptureRefs {
                object_id_color: AgentRgbaColor {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 255,
                },
                captures: Vec::new(),
            },
            text: None,
            rich_text_ref: None,
            rich_text: test_line_display_frame(),
        }
    }

    fn pixel_at(capture: &AgentRasterCapture, x: u32, y: u32) -> &[u8] {
        let index = usize::try_from(y)
            .unwrap()
            .saturating_mul(usize::try_from(capture.width).unwrap())
            .saturating_add(usize::try_from(x).unwrap())
            .saturating_mul(4);
        &capture.rgba[index..index + 4]
    }

    #[test]
    fn native_masked_framebuffer_crop_keeps_selected_rects_and_transparent_gap() {
        let source = arcweft_player_native::native::NativeFrameCapture {
            width: 8,
            height: 4,
            rgba: [9, 8, 7, 255].repeat(32),
            content_bbox: None,
            content_pixels: 0,
        };
        let objects = vec![
            test_observed_object("object.ui.left", 1, 1, 2, 2),
            test_observed_object("object.ui.right", 5, 1, 2, 2),
        ];
        let selected = objects.iter().collect::<Vec<_>>();
        let frame = test_line_display_frame();
        let context = AgentNativeCaptureContext {
            frame: &frame,
            left: 0.0,
            top: 0.0,
            objects: &objects,
            page_index: 0,
            capture_time_seconds: 60.0,
        };

        let capture = agent_native_masked_framebuffer_capture(&source, context, &selected).unwrap();

        assert_eq!(capture.width, 6);
        assert_eq!(capture.height, 2);
        assert_eq!(
            capture.composition,
            AgentImageComposition::MaskedFramebufferCrop
        );
        assert_eq!(
            capture.crop_origin,
            Some(AgentImageCropOrigin {
                space: AgentCoordinateSpace::Viewport,
                x: 1,
                y: 1,
            })
        );
        assert_eq!(pixel_at(&capture, 0, 0), &[9, 8, 7, 255]);
        assert_eq!(pixel_at(&capture, 1, 1), &[9, 8, 7, 255]);
        assert_eq!(pixel_at(&capture, 2, 0), &[0, 0, 0, 0]);
        assert_eq!(pixel_at(&capture, 3, 1), &[0, 0, 0, 0]);
        assert_eq!(pixel_at(&capture, 4, 0), &[9, 8, 7, 255]);
        assert_eq!(pixel_at(&capture, 5, 1), &[9, 8, 7, 255]);
    }

    #[test]
    fn native_non_text_debug_capture_reports_dedicated_attachments() {
        let source = arcweft_player_native::native::NativeFrameCapture {
            width: 32,
            height: 24,
            rgba: [0, 0, 0, 255].repeat(32 * 24),
            content_bbox: None,
            content_pixels: 0,
        };
        let objects = vec![test_observed_object("object.ui.panel", 4, 5, 7, 6)];
        let selected = objects.iter().collect::<Vec<_>>();
        let frame = test_resolved_line_display_frame();
        let context = AgentNativeCaptureContext {
            frame: &frame,
            left: 0.0,
            top: 0.0,
            objects: &objects,
            page_index: 0,
            capture_time_seconds: 60.0,
        };

        let object_id = agent_native_debug_capture(
            &source,
            context,
            &selected,
            AgentObserveCaptureKind::ObjectId,
            None,
        )
        .unwrap();
        assert_eq!(
            object_id.composition,
            AgentImageComposition::ObjectIdAttachment
        );
        assert_eq!(
            object_id.capture.content_bbox,
            Some(arcweft_player_native::native::NativeFrameContentBBox {
                x: 4,
                y: 5,
                width: 7,
                height: 6,
            })
        );
        let object_id_color = agent_object_id_color("object.ui.panel");
        assert_eq!(
            pixel_at(
                &AgentRasterCapture {
                    width: object_id.capture.width,
                    height: object_id.capture.height,
                    crop_origin: None,
                    composition: object_id.composition,
                    background: [0, 0, 0, 0],
                    rgba: object_id.capture.rgba.clone(),
                },
                4,
                5,
            ),
            object_id_color.as_slice()
        );

        let mask = agent_native_debug_capture(
            &source,
            context,
            &selected,
            AgentObserveCaptureKind::Mask,
            None,
        )
        .unwrap();
        assert_eq!(mask.composition, AgentImageComposition::MaskAttachment);
        assert_eq!(mask.capture.content_pixels, 42);
        assert_eq!(
            pixel_at(
                &AgentRasterCapture {
                    width: mask.capture.width,
                    height: mask.capture.height,
                    crop_origin: None,
                    composition: mask.composition,
                    background: [0, 0, 0, 0],
                    rgba: mask.capture.rgba,
                },
                10,
                10,
            ),
            &[255, 255, 255, 255]
        );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AgentObserveResourceKind {
    Observation,
    Objects,
    Overlay,
    Image,
    Logs,
    Signals,
    Audio,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AgentObserveMcpFormat {
    Read,
    List,
    ToolResult,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(untagged)]
enum AgentObserveResourceOutput {
    One(Box<AgentResource>),
    Many(Vec<AgentResource>),
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(untagged)]
enum AgentObserveMcpResourceOutput {
    OneRead(arcweft_agent_mcp::McpReadResourceResult),
    ManyRead(Vec<arcweft_agent_mcp::McpReadResourceResult>),
    List(arcweft_agent_mcp::McpListResourcesResult),
    ToolResult(arcweft_agent_mcp::McpCallToolResult),
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

#[derive(Clone, Debug)]
struct AgentObservationTrace {
    viewport: AgentViewport,
    objects: Vec<AgentObservedObject>,
    diagnostics: Vec<AgentDiagnostic>,
    task_request_count: usize,
    tick: usize,
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
