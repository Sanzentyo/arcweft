mod agent;
mod bundle;
mod commands;
pub(in crate::app) mod jit;
pub(in crate::app) mod runtime;
mod tooling;
pub(in crate::app) mod verify;

use self::agent::agent_command;
use self::bundle::{bundle_command, run_bundle_command};
use self::commands::{AgentCommand, BuildCommand, Cli, CliCommand, IdsCommand, JitCommand};
use self::jit::jit_command;
use self::runtime::{
    NativeRunHost, RuntimeExecutorInstance, apply_runtime_entry_selection, profile_lower_hir,
    profile_validate_hir, report_path, run_profile_phase, run_runtime_steps_with_executor,
    runtime_cli_command, runtime_plan_command, runtime_profile_command, runtime_run_command,
    runtime_serve_command, script_bench_command, script_test_command,
};
use self::tooling::{format_command, ids_command};
use self::verify::{check_command, unsafe_command, verify_command, verify_types_command};
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

pub(crate) fn print_json<T: serde::Serialize>(value: &T) -> Result<(), ExitCode> {
    serde_json::to_writer_pretty(std::io::stdout(), value).map_err(|error| {
        eprintln!("error: failed to write JSON: {error}");
        ExitCode::FAILURE
    })?;
    println!();
    Ok(())
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
