use arcweft_adapter_context::native_http_server_context;
use arcweft_core::aot::{AotProgram, AotProgramStats};
use arcweft_core::bytecode::{BytecodeProgram, BytecodeStats};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{AotExecutor, BytecodeVmExecutor, RuntimeExecutor};
use arcweft_core::plan::{
    FlowRuntimeId, RuntimeEntryKind, RuntimeEntrySpec, RuntimeEntryTarget, RuntimePlan,
    RuntimeRouteSpec,
};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::{
    pure::{
        AotPureFunctionBackend, AotPureI64Plan, PureFunctionBackend, PureFunctionBackendKind,
        PureFunctionRequest, PureFunctionResult, PureFunctionStats, VmPureFunctionBackend,
        compare_pure_function_backend,
    },
    value::{RuntimeBinaryOp, RuntimeBinding, RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_jit_cranelift::{CompiledPureI64Inputs, CraneliftPureFunctionBackend};
use arcweft_lang_sema::check::{TypeCheckReport, analyze_types, validate_typecheck_ready};
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_sema::resolve::{registry_from_hir, validate_hir_references};
use arcweft_lang_syntax::{
    expr::{CallArg, Expr, Literal, parse_expr},
    lint::lint_id_policy,
    parser::parse_source,
};
use arcweft_launch::{LaunchKind, LaunchProfileManifest, ResolvedLaunchProfile};
use arcweft_runtime_plan::flow::lower_runtime_plan;
use arcweft_runtime_plan::line_task::{LoweredLineTaskGroup, lower_line_task_groups};
use arcweft_runtime_plan::pure::{
    PureHelperCandidate, PureHelperLowerError, lower_pure_helper_candidates,
};
use arcweft_test::{BenchSection, ScriptBench, ScriptStep, ScriptTest, collect_script_tests};
use arcweft_tooling::{FormatOptions, ToolingEditReport, format_source, materialize_ids};
use arcweft_verify::{
    BackendKind, RuntimeTypeValidationStats, SmtBackend, VerificationMode, VerificationPolicy,
    VerificationReport, emit_smt_lib, validate_runtime_plan_types, verify_module_with_env,
};
use arcweft_verify_oxiz::OxizBackend;
use arcweft_verify_z3::ExternalZ3Backend;
use clap::{Args, Parser, Subcommand, ValueEnum};
mod native_task;
mod output;
mod server_adapter;
use native_task::{NativeTaskBridge, NativeTaskStats};
use output::{
    AotProfileStats, BorrowCheckProfileStats, BytecodeProfileStats, CheckReport,
    RuntimeExecutorStats, RuntimeExecutorTier, RuntimePlanReport, RuntimeProfileCompiler,
    RuntimeProfilePhase, RuntimeProfileReport, RuntimeProfileRuntime, RuntimeRunReport,
    RuntimeStepRunSummary, RuntimeTypeValidationProfileStats, RuntimeTypeValidationReportSummary,
    ScriptBenchDeterministicSummary, ScriptBenchElapsedSummary, ScriptBenchMeasurementSummary,
    ScriptBenchPureHelperDeterministicSummary, ScriptBenchPureHelperMeasurementSummary,
    ScriptBenchPureHelperStatsSummary, ScriptBenchPureHelperTimingSamples,
    ScriptBenchPureHelperTimingSummary, ScriptBenchRunReport, ScriptBenchRunSummary,
    ScriptBenchSectionRunSummary, ScriptTestRunReport, ScriptTestRunSummary, TypeCheckProfileStats,
    VerifyTypesReport, VerifyTypesRuntimeSelfCheck, VerifyTypesVerifierSummary, flow_status_label,
};
use server_adapter::{NativeHttpServerConfig, serve_native_http};
use std::fs;
use std::net::SocketAddr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

const KNOWN_ADAPTERS: &[&str] = &["sans-io", "native-http"];

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

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn run(cli: Cli) -> Result<(), ExitCode> {
    match cli.command {
        CliCommand::Check(options) => check_command(&options),
        CliCommand::Verify(options) => verify_command(&options),
        CliCommand::VerifyTypes(options) => verify_types_command(&options),
        CliCommand::Unsafe(options) => unsafe_command(&options),
        CliCommand::Plan(options) => runtime_plan_command(&options),
        CliCommand::Run(options) => runtime_run_command(&options),
        CliCommand::Profile(options) => runtime_profile_command(&options),
        CliCommand::Cli(options) => runtime_cli_command(&options),
        CliCommand::Serve(options) => runtime_serve_command(&options),
        CliCommand::Test(options) => script_test_command(&options),
        CliCommand::Bench(options) => script_bench_command(&options),
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
        && measurement.aot.accumulator == measurement.vm.accumulator;
    JitCheckReport {
        status: if matches_vm { "ok" } else { "failed" }.to_owned(),
        helper: target.name.clone(),
        helper_source: target.source.as_str().to_owned(),
        source_compiler: target.source_compiler.clone(),
        input_bindings: target.input_names.clone(),
        dynamic_inputs: !target.input_names.is_empty(),
        input_seed: options.input_seed,
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
        deterministic: JitCheckDeterministicReport {
            aot: measurement.aot.accumulator,
            jit: measurement.jit.accumulator,
            vm: measurement.vm.accumulator,
        },
        vm_stats: PureFunctionStatsReport::from_stats(&conformance.vm.stats),
        aot_stats: PureFunctionStatsReport::from_stats(&conformance.aot.stats),
        jit_stats: PureFunctionStatsReport::from_stats(compiled.jit.stats()),
    }
}

fn print_jit_check_human_report(report: &JitCheckReport) {
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
}

fn jit_check_inputs(seed: u64, sample: usize, iteration: usize, arity: usize) -> Vec<i64> {
    let sample = u64::try_from(sample).unwrap_or_default();
    let iteration = u64::try_from(iteration).unwrap_or_default();
    (0..arity)
        .map(|index| {
            let index = u64::try_from(index).unwrap_or_default();
            let modulus = 5 + index % 5;
            i64::try_from(
                seed.saturating_mul(index + 1)
                    .saturating_add(sample.saturating_mul(3 + index))
                    .saturating_add(iteration)
                    % modulus,
            )
            .map_or(1, |value| value + 1)
        })
        .collect()
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
    aot_compile_elapsed_ns: u128,
    jit_compile_elapsed_ns: u128,
}

struct JitCheckMeasurements {
    aot: JitRepeatedMeasurement,
    jit: JitRepeatedMeasurement,
    vm: JitRepeatedMeasurement,
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

    Ok(JitCheckCompiledHelpers {
        aot,
        jit,
        aot_compile_elapsed_ns,
        jit_compile_elapsed_ns,
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
    warmup_jit_check_vm(
        target,
        VmPureFunctionBackend,
        options.warmup,
        options.input_seed,
    )?;

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
        vm: measure_jit_check_vm(
            target,
            VmPureFunctionBackend,
            options.samples,
            options.iterations,
            options.input_seed,
        )?,
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
    fn builtin() -> Self {
        Self {
            name: "score".to_owned(),
            source: JitCheckHelperSource::Builtin,
            source_compiler: None,
            input_names: vec!["base".to_owned(), "bonus".to_owned()],
            expr: RuntimeExpr::If {
                condition: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Ge,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(3))),
                }),
                then_expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                    op: RuntimeBinaryOp::Mul,
                    rhs: Box::new(RuntimeExpr::Call {
                        callee: "add".to_owned(),
                        args: vec![
                            RuntimeExpr::Local("bonus".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::Int(2)),
                        ],
                    }),
                }),
                else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
            },
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
                    value: RuntimeValue::Int(value),
                }),
        )
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
        || Ok(JitCheckTarget::builtin()),
        |path| jit_check_source_target(path, options.helper.as_deref()),
    )
}

fn jit_check_source_target(
    path: &Path,
    helper_name: Option<&str>,
) -> Result<JitCheckTarget, ExitCode> {
    let checked = load_and_check_with_env(path, &TypeCheckEnv::new())?;
    let candidates = lower_pure_helper_candidates(&checked.hir).map_err(|errors| {
        for error in errors {
            eprintln!("error: {error}");
        }
        ExitCode::FAILURE
    })?;
    let candidate = select_jit_helper_candidate(&candidates, helper_name)?;
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
    for index in 0..warmup {
        let inputs = jit_check_inputs(input_seed, 0, index, compiled.param_names().len());
        let _ = compiled.call(&inputs);
    }
}

fn measure_jit_check_jit(
    compiled: &CompiledPureI64Inputs,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_inputs(input_seed, sample, index, compiled.param_names().len());
        compiled.call(&inputs).map_err(|error| {
            eprintln!("error: JIT evaluation failed: {error}");
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
    for index in 0..warmup {
        let inputs = jit_check_inputs(input_seed, 0, index, arity);
        let _ = compiled.call_with_inputs(&inputs).map_err(|error| {
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
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_inputs(input_seed, sample, index, arity);
        compiled
            .call_with_inputs(&inputs)
            .map(|(value, _stats)| value)
            .map_err(|error| {
                eprintln!("error: AOT evaluation failed: {error}");
                ExitCode::FAILURE
            })
    })
}

fn warmup_jit_check_vm(
    target: &JitCheckTarget,
    vm_backend: VmPureFunctionBackend,
    warmup: usize,
    input_seed: u64,
) -> Result<(), ExitCode> {
    for index in 0..warmup {
        let inputs = jit_check_inputs(input_seed, 0, index, target.input_names.len());
        let request = target.request_with_inputs(&inputs);
        let _ = vm_backend.evaluate(&request).map_err(|error| {
            eprintln!("error: VM warmup failed: {error}");
            ExitCode::FAILURE
        })?;
    }
    Ok(())
}

fn measure_jit_check_vm(
    target: &JitCheckTarget,
    vm_backend: VmPureFunctionBackend,
    samples: usize,
    iterations: usize,
    input_seed: u64,
) -> Result<JitRepeatedMeasurement, ExitCode> {
    measure_repeated(samples, iterations, |sample, index| {
        let inputs = jit_check_inputs(input_seed, sample, index, target.input_names.len());
        let request = target.request_with_inputs(&inputs);
        let value = vm_backend.evaluate(&request).map_err(|error| {
            eprintln!("error: VM evaluation failed: {error}");
            ExitCode::FAILURE
        })?;
        if let RuntimeValue::Int(value) = value.value {
            Ok(value)
        } else {
            Ok(0)
        }
    })
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

fn runtime_run_command(options: &RuntimeRunOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    if let Some(profile) = selection.profile() {
        match profile.kind() {
            LaunchKind::Server => {
                return runtime_serve_selection(
                    &selection,
                    options.entry.as_deref(),
                    None,
                    None,
                    false,
                    options.max_ops,
                    options.json,
                );
            }
            LaunchKind::Test => {
                return script_test_selection(
                    &selection,
                    options.steps,
                    options.mode,
                    options.max_ops,
                    &options.values,
                    options.executor,
                    options.json,
                );
            }
            LaunchKind::Bench => return runtime_run_bench_selection(&selection, options),
            LaunchKind::Game | LaunchKind::Cli => {}
        }
    }

    let checked = load_and_check_selection(&selection, None)?;
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
        options.steps,
        options.mode,
        options.max_ops,
        &options.values,
        options.executor,
    );
    let report = RuntimeRunReport {
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
        values: options.values.clone(),
        json: options.json,
    };
    script_bench_selection(selection, &bench_options)
}

fn runtime_profile_command(options: &RuntimeProfileOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    let adapter = options.adapter.as_deref().or(selection.adapter());
    let env = typecheck_env_for_adapter(adapter)?;
    if !is_arcw_path(selection.path()) {
        eprintln!(
            "error: {} is not an .arcw source file",
            selection.path().display()
        );
        return Err(ExitCode::from(2));
    }

    let mut phases = Vec::new();
    let compiled = compile_profile_runtime_plan(&selection, &env, &mut phases)?;
    let mut plan = compiled.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    let trace = run_profile_phase(&mut phases, "run", || {
        Ok::<RuntimeRunTrace, ExitCode>(run_runtime_steps(
            plan,
            Some(selection.path()),
            options.steps,
            options.mode,
            options.max_ops,
            &options.values,
            options.executor,
        ))
    })?;
    let final_status = flow_status_label(&trace.final_status);
    let report = RuntimeProfileReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            typecheck: TypeCheckProfileStats::from(&compiled.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&compiled.typecheck_report.stats),
            runtime_type_validation: RuntimeTypeValidationProfileStats::from(
                &compiled.runtime_type_validation_stats,
            ),
            bytecode: BytecodeProfileStats::from(&compiled.bytecode_stats),
            aot: AotProfileStats::from(&compiled.aot_stats),
        },
        phases,
        runtime: RuntimeProfileRuntime {
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
    line_task_groups: usize,
    typecheck_report: TypeCheckReport,
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
    let plan = run_profile_phase(phases, "runtime_plan_lower", || {
        lower_runtime_plan(&hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })
    })?;
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
        Ok::<AotProgram, ExitCode>(AotProgram::from_runtime_plan(plan.clone()))
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
        line_task_groups: line_task_groups.len(),
        typecheck_report,
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
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
    executor_tier: CliRuntimeExecutorTier,
) -> RuntimeRunTrace {
    let mut executor = RuntimeExecutorInstance::new(plan, executor_tier);
    let mut host = source_path.map(NativeTaskBridge::new);
    let mut task_events = Vec::new();
    let mut summaries = Vec::new();
    for step_index in 0..steps {
        let result = executor.step(
            RuntimeStepInput {
                bindings: values.to_vec(),
                task_events: std::mem::take(&mut task_events),
                ..RuntimeStepInput::default()
            },
            step_options(mode, max_ops),
        );
        let task_requests = result.output.requests.tasks.clone();
        let summary = RuntimeStepRunSummary::from_result(step_index, result, executor.fiber());
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        summaries.push(summary);
        if done {
            break;
        }
        if let Some(host) = host.as_mut() {
            task_events = host.complete_tasks(&task_requests);
        }
    }
    RuntimeRunTrace {
        steps: summaries,
        final_status: executor.fiber().status.clone(),
        executor_stats: executor.executor_stats(),
        native_io: host
            .as_ref()
            .map_or_else(NativeTaskStats::default, NativeTaskBridge::stats),
    }
}

struct RuntimeRunTrace {
    steps: Vec<RuntimeStepRunSummary>,
    final_status: FlowFiberStatus,
    executor_stats: RuntimeExecutorStats,
    native_io: NativeTaskStats,
}

enum RuntimeExecutorInstance {
    BytecodeVm(BytecodeVmExecutor),
    Aot(AotExecutor),
}

impl RuntimeExecutorInstance {
    fn new(plan: RuntimePlan, tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => {
                Self::BytecodeVm(BytecodeVmExecutor::from_runtime_plan(plan))
            }
            CliRuntimeExecutorTier::Aot => Self::Aot(AotExecutor::new(plan)),
        }
    }

    fn step(&mut self, input: RuntimeStepInput, options: RuntimeStepOptions) -> RuntimeStepResult {
        match self {
            Self::BytecodeVm(executor) => executor.step(input, options),
            Self::Aot(executor) => executor.step(input, options),
        }
    }

    fn fiber(&self) -> &arcweft_core::engine::FlowFiber {
        match self {
            Self::BytecodeVm(executor) => executor.fiber(),
            Self::Aot(executor) => executor.fiber(),
        }
    }

    fn executor_stats(&self) -> RuntimeExecutorStats {
        match self {
            Self::BytecodeVm(_) => RuntimeExecutorStats::default(),
            Self::Aot(executor) => RuntimeExecutorStats {
                aot_fast_path_ops: executor.fast_path_ops(),
            },
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

fn runtime_cli_command(options: &CliRunOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Cli, "cli")?;
    let checked = load_and_check_selection(&selection, None)?;
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
        value: RuntimeValue::BracketSeq(
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
        value: RuntimeValue::Int(i64::try_from(options.args.len()).unwrap_or(i64::MAX)),
    });

    let trace = run_runtime_steps(
        plan,
        Some(selection.path()),
        options.steps,
        options.mode,
        options.max_ops,
        &bindings,
        options.executor,
    );
    let report = RuntimeRunReport {
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

fn runtime_serve_command(options: &ServeOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Server, "serve")?;
    runtime_serve_selection(
        &selection,
        options.entry.as_deref(),
        options.adapter.as_deref(),
        options.listen,
        options.once,
        options.max_ops,
        options.json,
    )
}

fn runtime_serve_selection(
    selection: &SourceSelection,
    entry_override: Option<&str>,
    adapter_override: Option<&str>,
    listen_override: Option<SocketAddr>,
    once: bool,
    max_ops: usize,
    json: bool,
) -> Result<(), ExitCode> {
    let adapter = adapter_override
        .or(selection.adapter())
        .unwrap_or("sans-io");
    let checked = load_and_check_selection(selection, Some(adapter))?;
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
    let listen = match listen_override {
        Some(listen) => Some(listen),
        None => profile_listen_addr(selection)?,
    };
    if let Some(listen) = listen {
        let server_report = serve_native_http(
            &plan,
            &routes,
            NativeHttpServerConfig {
                listen,
                once,
                max_ops,
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
        return if json {
            print_json(&report)
        } else {
            println!(
                "ok: served {} request(s) on {}",
                report.server.handled_requests, report.server.listen
            );
            Ok(())
        };
    }
    if json {
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

fn script_test_command(options: &ScriptTestOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Test, "test")?;
    script_test_selection(
        &selection,
        options.steps,
        options.mode,
        options.max_ops,
        &options.values,
        options.executor,
        options.json,
    )
}

fn script_test_selection(
    selection: &SourceSelection,
    step_limit: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    values: &[RuntimeBinding],
    executor: CliRuntimeExecutorTier,
    json: bool,
) -> Result<(), ExitCode> {
    let checked = load_and_check_selection(selection, None)?;
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
                let config = RuntimeStepRunConfig {
                    steps: step_limit,
                    mode,
                    max_ops,
                    executor,
                };
                run_script_test(test, &plan, selection.path(), config, values)
            })
            .collect(),
    };
    let failed = output.tests.iter().any(|test| test.status == "failed");
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
            "not_started".to_owned(),
            vec!["scenario test requires `start(@flow.id)`".to_owned()],
            Vec::new(),
        );
    };
    let mut plan = plan.clone();
    plan.entry_flow = Some(FlowRuntimeId(start));
    let trace = run_runtime_steps(
        plan,
        Some(source_path),
        config.steps,
        config.mode,
        config.max_ops,
        values,
        config.executor,
    );
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
        FlowFiberStatus::Running | FlowFiberStatus::Waiting(_) | FlowFiberStatus::Choice(_) => {
            diagnostics.push(format!(
                "scenario did not finish within {} step(s): {final_status}",
                config.steps
            ));
        }
    }
    let passed = diagnostics.is_empty();
    ScriptTestRunSummary::completed(test, passed, final_status, diagnostics, trace.steps)
}

#[derive(Clone, Copy, Debug)]
struct RuntimeStepRunConfig {
    steps: usize,
    mode: CliRuntimeStepMode,
    max_ops: usize,
    executor: CliRuntimeExecutorTier,
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

fn script_bench_command(options: &ScriptBenchOptions) -> Result<(), ExitCode> {
    let selection = resolve_source_selection(options.path.as_ref(), &options.profile)?;
    require_profile_kind(&selection, LaunchKind::Bench, "bench")?;
    script_bench_selection(&selection, options)
}

fn script_bench_selection(
    selection: &SourceSelection,
    options: &ScriptBenchOptions,
) -> Result<(), ExitCode> {
    let env = typecheck_env_for_adapter(selection.adapter())?;
    let mut phases = Vec::new();
    let compiled = compile_profile_runtime_plan(selection, &env, &mut phases)?;
    let manifest = collect_script_tests(&compiled.hir);
    let pure_helpers = lower_pure_helper_candidates(&compiled.hir);
    let output = ScriptBenchRunReport {
        source: report_path(selection.path()),
        syntax_warnings: compiled.syntax_warnings,
        line_task_groups: compiled.line_task_groups,
        compiler: RuntimeProfileCompiler {
            typecheck: TypeCheckProfileStats::from(&compiled.typecheck_report),
            borrow_check: BorrowCheckProfileStats::from(&compiled.typecheck_report.stats),
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
) -> ScriptBenchRunSummary {
    let mut summary = validate_script_bench(bench);
    if summary.status != "validated" {
        return summary;
    }
    let sections = bench
        .sections
        .iter()
        .map(|section| run_bench_section(section, plan, pure_helpers, source_path, options))
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
        let diagnostics = bench_expectation_failures(bench, plan, source_path, options);
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
        options.steps,
        options.mode,
        options.max_ops,
        &options.values,
        options.executor,
    );
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
    let jit_options = JitCheckOptions {
        path: None,
        helper: Some(helper_name),
        iterations: options.iterations,
        warmup: options.warmup,
        samples: options.samples,
        input_seed: options.input_seed,
        json: false,
    };
    match run_jit_check(&jit_options, &target) {
        Ok(report) if report.matches_vm => ScriptBenchSectionRunSummary::measured_pure_helper(
            &section.name,
            validated.diagnostics,
            ScriptBenchPureHelperMeasurementSummary::from(&report),
        ),
        Ok(report) => ScriptBenchSectionRunSummary::new(
            &section.name,
            "failed",
            vec![format!(
                "pure helper `{}` did not match the VM reference",
                report.helper
            )],
        ),
        Err(code) => ScriptBenchSectionRunSummary::new(
            &section.name,
            "failed",
            vec![format!(
                "pure helper `{}` failed during VM/AOT/JIT measurement: exit code {code:?}",
                target.name
            )],
        ),
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
) -> ScriptBenchSectionRunSummary {
    let validated = validate_bench_section(section);
    if section.name != "measure" || validated.status != "validated" {
        return validated;
    }
    if bench_pure_helper_name(section).is_some() {
        return run_bench_pure_helper_section(section, pure_helpers, options, validated);
    }
    let Some(flow) = bench_start_flow(section) else {
        return validated;
    };
    let mut elapsed = Vec::new();
    let mut executed_ops = Vec::new();
    let mut line_effects = Vec::new();
    let mut task_requests = Vec::new();
    let mut task_events_in = Vec::new();
    let mut aot_fast_path_ops = Vec::new();
    let mut native_io = NativeTaskStatsSamples::default();
    let mut diagnostics = 0usize;
    for iteration in 0..options.warmup + options.iterations {
        let mut iteration_plan = plan.clone();
        iteration_plan.entry_flow = Some(FlowRuntimeId(flow.clone()));
        let started = Instant::now();
        let trace = run_runtime_steps(
            iteration_plan,
            Some(source_path),
            options.steps,
            options.mode,
            options.max_ops,
            &options.values,
            options.executor,
        );
        let elapsed_ns = started.elapsed().as_nanos();
        if iteration < options.warmup {
            continue;
        }
        elapsed.push(elapsed_ns);
        executed_ops.push(trace.steps.iter().map(|step| step.stats.executed_ops).sum());
        line_effects.push(trace.steps.iter().map(|step| step.stats.line_effects).sum());
        task_requests.push(
            trace
                .steps
                .iter()
                .map(|step| step.task_requests.len())
                .sum(),
        );
        task_events_in.push(
            trace
                .steps
                .iter()
                .map(|step| step.stats.task_events_in)
                .sum(),
        );
        aot_fast_path_ops.push(trace.executor_stats.aot_fast_path_ops);
        native_io.push(trace.native_io);
        diagnostics += trace
            .steps
            .iter()
            .map(|step| step.diagnostics.len())
            .sum::<usize>();
    }
    ScriptBenchSectionRunSummary::measured(
        &section.name,
        validated.diagnostics,
        ScriptBenchMeasurementSummary {
            executor: RuntimeExecutorTier::from(options.executor),
            executor_stats: RuntimeExecutorStats {
                aot_fast_path_ops: median_usize(&mut aot_fast_path_ops),
            },
            native_io: native_io.median(),
            warmup: options.warmup,
            iterations: options.iterations,
            steps: options.steps,
            elapsed_ns: ScriptBenchElapsedSummary {
                min: *elapsed.iter().min().unwrap_or(&0),
                median: median_u128(&mut elapsed),
                max: *elapsed.iter().max().unwrap_or(&0),
            },
            deterministic: ScriptBenchDeterministicSummary {
                executed_ops_median: median_usize(&mut executed_ops),
                line_effects_median: median_usize(&mut line_effects),
                task_requests_median: median_usize(&mut task_requests),
                task_events_in_median: median_usize(&mut task_events_in),
                diagnostics,
            },
        },
    )
}

#[derive(Default)]
struct NativeTaskStatsSamples {
    completed_tasks: Vec<usize>,
    failed_tasks: Vec<usize>,
    read_ops: Vec<usize>,
    write_ops: Vec<usize>,
    bytes_read: Vec<usize>,
    bytes_written: Vec<usize>,
}

impl NativeTaskStatsSamples {
    fn push(&mut self, stats: NativeTaskStats) {
        self.completed_tasks.push(stats.completed_tasks);
        self.failed_tasks.push(stats.failed_tasks);
        self.read_ops.push(stats.read_ops);
        self.write_ops.push(stats.write_ops);
        self.bytes_read.push(stats.bytes_read);
        self.bytes_written.push(stats.bytes_written);
    }

    fn median(&mut self) -> NativeTaskStats {
        NativeTaskStats {
            completed_tasks: median_usize(&mut self.completed_tasks),
            failed_tasks: median_usize(&mut self.failed_tasks),
            read_ops: median_usize(&mut self.read_ops),
            write_ops: median_usize(&mut self.write_ops),
            bytes_read: median_usize(&mut self.bytes_read),
            bytes_written: median_usize(&mut self.bytes_written),
        }
    }
}

fn bench_start_flow(section: &BenchSection) -> Option<String> {
    let start = section.text.find("start(")?;
    let tail = &section.text[start..];
    let close = tail.find(')')?;
    parse_start_flow_call(&tail[..=close])
}

fn median_u128(values: &mut [u128]) -> u128 {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or_default()
}

fn median_usize(values: &mut [usize]) -> usize {
    values.sort_unstable();
    values.get(values.len() / 2).copied().unwrap_or_default()
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

fn verify_types_command(options: &VerifyTypesOptions) -> Result<(), ExitCode> {
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
    let runtime = verify_types_runtime_self_check(runtime_plan, &selection, options, &mut checked)?;
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
) -> Result<Option<VerifyTypesRuntimeSelfCheck>, ExitCode> {
    if !options.run {
        return Ok(None);
    }
    let trace = run_profile_phase(&mut checked.phases, "run", || {
        Ok(run_runtime_steps(
            runtime_plan,
            Some(selection.path()),
            options.steps,
            options.runtime_mode,
            options.max_ops,
            &options.values,
            options.executor,
        ))
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
    Profile(ResolvedLaunchProfile),
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
            let resolved = manifest
                .resolve_profile_with_adapters(profile_id, manifest_dir, KNOWN_ADAPTERS)
                .map_err(|error| {
                    eprintln!("error: {error}");
                    ExitCode::FAILURE
                })?;
            Ok(SourceSelection::Profile(resolved))
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
    let adapter = adapter_override.or(selection.adapter());
    let env = typecheck_env_for_adapter(adapter)?;
    load_and_check_with_env(selection.path(), &env)
}

fn typecheck_env_for_adapter(adapter: Option<&str>) -> Result<TypeCheckEnv, ExitCode> {
    match adapter {
        None | Some("sans-io") => Ok(TypeCheckEnv::new()),
        Some("native-http") => Ok(server_adapter_typecheck_env()),
        Some(adapter) => {
            eprintln!("error: unknown adapter `{adapter}`");
            Err(ExitCode::from(2))
        }
    }
}

struct CheckedModule {
    hir: arcweft_lang_hir::model::HirModule,
    env: TypeCheckEnv,
    syntax_warnings: usize,
    line_task_groups: Vec<LoweredLineTaskGroup>,
    typecheck_report: TypeCheckReport,
    phases: Vec<RuntimeProfilePhase>,
}

fn load_and_check_with_env(path: &Path, env: &TypeCheckEnv) -> Result<CheckedModule, ExitCode> {
    if !is_arcw_path(path) {
        eprintln!("error: {} is not an .arcw source file", path.display());
        return Err(ExitCode::from(2));
    }
    let mut phases = Vec::new();
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
        line_task_groups,
        typecheck_report,
        phases,
    })
}

fn server_adapter_typecheck_env() -> TypeCheckEnv {
    native_http_server_context().apply_to_env(TypeCheckEnv::new())
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

impl From<CliRuntimeExecutorTier> for RuntimeExecutorTier {
    fn from(tier: CliRuntimeExecutorTier) -> Self {
        match tier {
            CliRuntimeExecutorTier::BytecodeVm => Self::BytecodeVm,
            CliRuntimeExecutorTier::Aot => Self::Aot,
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
    input_bindings: Vec<String>,
    dynamic_inputs: bool,
    input_seed: u64,
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
    deterministic: JitCheckDeterministicReport,
    vm_stats: PureFunctionStatsReport,
    aot_stats: PureFunctionStatsReport,
    jit_stats: PureFunctionStatsReport,
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
    #[serde(rename = "vm_accumulator")]
    vm: i64,
}

impl From<&JitCheckReport> for ScriptBenchPureHelperMeasurementSummary {
    fn from(report: &JitCheckReport) -> Self {
        Self {
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
            deterministic: ScriptBenchPureHelperDeterministicSummary {
                aot: report.deterministic.aot,
                jit: report.deterministic.jit,
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
        RuntimeValue::Float(value) | RuntimeValue::String(value) => value.clone(),
        RuntimeValue::Char(value) => value.to_string(),
        RuntimeValue::Duration(value) => format!("{}ns", value.as_nanos()),
        RuntimeValue::EntityRef(value) => format!("@{value}"),
        RuntimeValue::Tuple(values) => format!("tuple/{}", values.len()),
        RuntimeValue::BracketSeq(values) => format!("bracket_seq/{}", values.len()),
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
        value: parse_runtime_value(raw),
    })
}

fn parse_runtime_value(raw: &str) -> RuntimeValue {
    match raw {
        "true" => RuntimeValue::Bool(true),
        "false" => RuntimeValue::Bool(false),
        "()" => RuntimeValue::Unit,
        value if value.starts_with('@') => RuntimeValue::EntityRef(value[1..].to_owned()),
        value => value.parse::<i64>().map_or_else(
            |_| RuntimeValue::String(value.to_owned()),
            RuntimeValue::Int,
        ),
    }
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

fn print_json<T: serde::Serialize>(value: &T) -> Result<(), ExitCode> {
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
