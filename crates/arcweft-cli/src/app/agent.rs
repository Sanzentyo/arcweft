use super::commands::{AgentCommand, AgentScriptCommand};
use super::project::ProfileOptions;
use super::runtime::{
    CliRuntimeExecutorTier, CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    CliRuntimeStepMode, parse_runtime_binding_arg, parse_runtime_pure_workers,
};
use super::shared::print_json;
use arcweft_agent_protocol::{
    AgentResource, AgentResourceBody, AgentResourceKind,
    ids::{AgentResourceUri, AgentRunId, SessionId, StableHash},
    protocol::{
        ActionResult, AgentAction, AgentHostResponse, AgentSessionInfo, CaptureFormat,
        CaptureRequest, CaptureResult, ObservationEnvelope, ObserveRequest,
    },
    trace::{AgentTraceKind, AgentTraceRecord},
    value::AgentValue,
};
use arcweft_agent_runner::{
    AgentControllerRunConfig, AgentControllerRunReport, AgentRunner, AgentRunnerConfig,
    AgentSession, NoopRagService, RuntimeAgentCapability, RuntimeAgentPolicy,
};
use arcweft_bundle::{ArcweftBundle, BundleKind};
use arcweft_core::value::RuntimeBinding;
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    sink::DebugEventSink,
};
use arcweft_id::PublicId as SemaPublicId;
use arcweft_lang_sema::{
    project_index::{EntitySymbol, ProgramHash, ProjectSemanticIndex, SemanticHash},
    types::{EntityKind, EntityType, TypeKind},
};
use arcweft_runtime_host::NativeAdapterRegistrar;
use arcweft_source::SourceAnchor;
use clap::{Args, ValueEnum};
use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{fs, path::Path};

#[cfg(feature = "native-capture")]
use super::project::{
    load_and_check_selection, native_host_policy_for_selection, resolve_source_selection,
    runtime_plan_options_for_selection, runtime_pure_config_for_selection,
};

#[cfg(feature = "native-capture")]
use super::runtime::step_options;
#[cfg(feature = "native-capture")]
use crate::output::flow_status_label;
#[cfg(feature = "native-capture")]
use arcweft_compiler::lower_source_runtime_plan_with_stats_and_options;
#[cfg(feature = "native-capture")]
use arcweft_core::engine::FlowFiberStatus;
#[cfg(feature = "native-capture")]
use arcweft_core::step::{RuntimeStepInput, RuntimeStepResult};
#[cfg(feature = "native-capture")]
use arcweft_lang_sema::project_index::project_semantic_index_from_hir;
#[cfg(feature = "native-capture")]
use arcweft_render_text::LineDisplayCatalog;
#[cfg(feature = "native-capture")]
use arcweft_runtime_host::NativeTaskBridge;
#[cfg(feature = "native-capture")]
use arcweft_source::SourceName;

pub(in crate::app) const AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH: u32 = 1280;
pub(in crate::app) const AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT: u32 = 720;

#[derive(Args, Clone, Debug)]
pub(super) struct AgentObserveOptions {
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
    #[arg(long = "capture-step")]
    capture_step: Option<usize>,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long = "viewport-width", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH)]
    viewport_width: u32,
    #[arg(long = "viewport-height", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT)]
    viewport_height: u32,
    #[arg(long = "textbox-height")]
    textbox_height: Option<u32>,
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
    #[arg(long = "capture-time")]
    capture_time_seconds: Option<f32>,
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
pub(super) struct AgentHitTestOptions {
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
    #[arg(long = "capture-step")]
    capture_step: Option<usize>,
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[arg(long = "viewport-width", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH)]
    viewport_width: u32,
    #[arg(long = "viewport-height", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT)]
    viewport_height: u32,
    #[arg(long = "textbox-height")]
    textbox_height: Option<u32>,
    #[arg(long = "capture-time")]
    capture_time_seconds: Option<f32>,
    #[arg(long)]
    x: u32,
    #[arg(long)]
    y: u32,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentMcpOptions {}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentScriptCheckOptions {
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentScriptBuildOptions {
    path: PathBuf,
    #[arg(long, short = 'o')]
    output: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long = "signal", value_parser = parse_agent_script_signal_arg)]
    signals: Vec<AgentScriptSignalArg>,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentScriptRunOptions {
    path: PathBuf,
    #[arg(long)]
    json: bool,
    #[cfg(feature = "native-capture")]
    #[arg(long = "native-source")]
    native_source: Option<PathBuf>,
    #[cfg(feature = "native-capture")]
    #[command(flatten)]
    native_profile: ProfileOptions,
    #[cfg(feature = "native-capture")]
    #[arg(long, conflicts_with = "flow")]
    entry: Option<String>,
    #[cfg(feature = "native-capture")]
    #[arg(long, conflicts_with = "entry")]
    flow: Option<String>,
    #[cfg(feature = "native-capture")]
    #[arg(long, value_enum, default_value_t = CliRuntimeExecutorTier::BytecodeVm)]
    executor: CliRuntimeExecutorTier,
    #[cfg(feature = "native-capture")]
    #[arg(long, value_enum)]
    pure_backend: Option<CliRuntimePureBackend>,
    #[cfg(feature = "native-capture")]
    #[arg(long, value_parser = parse_runtime_pure_workers)]
    pure_workers: Option<CliRuntimePureWorkers>,
    #[cfg(feature = "native-capture")]
    #[arg(long)]
    pure_batch_min_len: Option<usize>,
    #[cfg(feature = "native-capture")]
    #[arg(long)]
    pure_object_artifacts: bool,
    #[cfg(feature = "native-capture")]
    #[arg(long, value_enum)]
    math_backend: Option<CliRuntimeMathBackend>,
    #[cfg(feature = "native-capture")]
    #[arg(long)]
    math_wgpu_min_elements: Option<usize>,
    #[cfg(feature = "native-capture")]
    #[arg(long = "native-steps", default_value_t = 8)]
    native_steps: usize,
    #[cfg(feature = "native-capture")]
    #[arg(long = "native-mode", value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    native_mode: CliRuntimeStepMode,
    #[cfg(feature = "native-capture")]
    #[arg(long = "native-max-ops", default_value_t = 64)]
    native_max_ops: usize,
    #[cfg(feature = "native-capture")]
    #[arg(long = "value", value_parser = parse_runtime_binding_arg)]
    values: Vec<RuntimeBinding>,
    #[cfg(feature = "native-capture")]
    #[arg(long = "viewport-width", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH)]
    viewport_width: u32,
    #[cfg(feature = "native-capture")]
    #[arg(long = "viewport-height", default_value_t = AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT)]
    viewport_height: u32,
    #[cfg(feature = "native-capture")]
    #[arg(long = "textbox-height")]
    textbox_height: Option<u32>,
    #[cfg(feature = "native-capture")]
    #[arg(long = "capture-time")]
    capture_time_seconds: Option<f32>,
    #[arg(long, default_value_t = 256)]
    max_steps: usize,
    #[arg(long, default_value_t = 1024)]
    max_ops: usize,
    #[arg(long = "signal", value_parser = parse_agent_script_signal_arg)]
    signals: Vec<AgentScriptSignalArg>,
    #[arg(long = "state", value_parser = parse_agent_script_state_arg)]
    states: Vec<AgentScriptStateArg>,
    #[arg(long = "trace-out")]
    trace_out: Option<PathBuf>,
    #[arg(long, default_value = "run.cli")]
    run_id: String,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentScriptReplayOptions {
    path: PathBuf,
    #[arg(long)]
    expect: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentScriptTraceOptions {
    path: PathBuf,
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug)]
struct AgentScriptSignalArg {
    id: String,
    value: AgentValue,
    ty: TypeKind,
}

#[derive(Clone, Debug)]
struct AgentScriptStateArg {
    path: String,
    value: AgentValue,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum AgentObserveResourceKind {
    Observation,
    Objects,
    PresentationTree,
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

#[cfg(feature = "native-capture")]
mod native;

#[cfg(feature = "native-capture")]
pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Script { command } => agent_script_command(command, adapter_registrars),
        command => native::agent_command(command, adapter_registrars),
    }
}

#[cfg(not(feature = "native-capture"))]
pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Script { command } => agent_script_command(command, adapter_registrars),
        AgentCommand::Observe(_) | AgentCommand::HitTest(_) | AgentCommand::Mcp(_) => {
            eprintln!("error: this arcw agent command requires the native-capture feature");
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_script_command(
    command: AgentScriptCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentScriptCommand::Build(options) => agent_script_build_command(&options),
        AgentScriptCommand::Check(options) => agent_script_check_command(&options),
        AgentScriptCommand::Replay(options) => agent_script_replay_command(&options),
        AgentScriptCommand::Run(options) => agent_script_run_command(&options, adapter_registrars),
        AgentScriptCommand::Trace(options) => agent_script_trace_command(&options),
    }
}

#[derive(serde::Serialize)]
struct AgentScriptCheckReport {
    path: String,
    ok: bool,
    agents: usize,
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentScriptBuildReport {
    path: String,
    output: String,
    ok: bool,
    agents: usize,
    agent_id: Option<String>,
    bundle_kind: Option<String>,
    bytecode_instructions: usize,
    bytes: usize,
    error: Option<String>,
}

fn agent_script_build_command(options: &AgentScriptBuildOptions) -> Result<(), ExitCode> {
    if !is_awfagent_path(&options.path) {
        eprintln!(
            "error: {} is not an .awfagent source file",
            options.path.display()
        );
        return Err(ExitCode::from(2));
    }
    if !is_awfb_path(&options.output) {
        eprintln!(
            "error: {} is not an .awfb bundle output path",
            options.output.display()
        );
        return Err(ExitCode::from(2));
    }
    let report =
        agent_script_build_report(options).unwrap_or_else(|error| AgentScriptBuildReport {
            path: options.path.display().to_string(),
            output: options.output.display().to_string(),
            ok: false,
            agents: 0,
            agent_id: None,
            bundle_kind: None,
            bytecode_instructions: 0,
            bytes: 0,
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: wrote {} ({} bytecode instruction(s))",
            report.path, report.output, report.bytecode_instructions
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn agent_script_build_report(
    options: &AgentScriptBuildOptions,
) -> Result<AgentScriptBuildReport, String> {
    let source = fs::read_to_string(&options.path)
        .map_err(|error| format!("failed to read {}: {error}", options.path.display()))?;
    let project = agent_script_project_index(&options.signals)?;
    let compiled = arcweft_compiler::compile_agent_bundle_with_project(source, &project)
        .map_err(|error| error.to_string())?;
    let bytes = compiled
        .bundle
        .to_json_bytes()
        .map_err(|error| error.to_string())?;
    write_agent_bundle(&options.output, &bytes)?;
    Ok(AgentScriptBuildReport {
        path: options.path.display().to_string(),
        output: options.output.display().to_string(),
        ok: true,
        agents: compiled.hir.agents().len(),
        agent_id: Some(compiled.manifest.agent_id.as_str().to_owned()),
        bundle_kind: Some(compiled.bundle.bundle_kind.to_string()),
        bytecode_instructions: compiled.bundle.manifest.runtime.bytecode_instructions,
        bytes: bytes.len(),
        error: None,
    })
}

fn agent_script_compile_project_index(
    options: &AgentScriptRunOptions,
) -> Result<ProjectSemanticIndex, String> {
    #[cfg(feature = "native-capture")]
    if agent_script_run_uses_native_session(options) {
        let selection =
            resolve_source_selection(options.native_source.as_ref(), &options.native_profile)
                .map_err(|code| {
                    format!("failed to resolve native source for Agent Script: {code:?}")
                })?;
        let checked = load_and_check_selection(&selection, None)
            .map_err(|code| format!("failed to check native source for Agent Script: {code:?}"))?;
        let mut project = project_semantic_index_from_hir(
            &checked.hir,
            ProgramHash::new(format!("native-source:{}", selection.path().display())),
            &SourceName::path(selection.path().display().to_string()),
        )
        .map_err(|error| error.to_string())?;
        for signal in &options.signals {
            let id = SemaPublicId::try_new(signal.id.clone()).map_err(|error| error.to_string())?;
            if project.entity(&id).is_none() {
                project = project.with_entity(agent_script_signal_symbol(signal, id));
            }
        }
        return Ok(project);
    }
    agent_script_project_index(&options.signals)
}

fn write_agent_bundle(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn agent_script_check_command(options: &AgentScriptCheckOptions) -> Result<(), ExitCode> {
    if !is_awfagent_path(&options.path) {
        eprintln!(
            "error: {} is not an .awfagent source file",
            options.path.display()
        );
        return Err(ExitCode::from(2));
    }
    let source = fs::read_to_string(&options.path).map_err(|error| {
        eprintln!("error: failed to read {}: {error}", options.path.display());
        ExitCode::FAILURE
    })?;
    let report = match arcweft_compiler::compile_agent_source(source) {
        Ok(compiled) => AgentScriptCheckReport {
            path: options.path.display().to_string(),
            ok: true,
            agents: compiled.hir.agents().len(),
            error: None,
        },
        Err(error) => AgentScriptCheckReport {
            path: options.path.display().to_string(),
            ok: false,
            agents: 0,
            error: Some(error.to_string()),
        },
    };
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!("{}: ok ({} agent item(s))", report.path, report.agents);
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

#[derive(serde::Serialize)]
struct AgentScriptRunReport {
    path: String,
    ok: bool,
    agents: usize,
    steps: usize,
    host_calls: usize,
    events_emitted: u64,
    final_status: Option<String>,
    trace_path: Option<String>,
    trace_records: usize,
    responses: Vec<AgentHostResponse>,
    error: Option<String>,
}

struct AgentScriptRunInput {
    path: String,
    agents: usize,
    bundle: ArcweftBundle,
}

#[derive(serde::Serialize)]
struct AgentScriptTraceReport {
    path: String,
    ok: bool,
    records: usize,
    run_id: Option<String>,
    sessions: Vec<String>,
    first_sequence: Option<u64>,
    last_sequence: Option<u64>,
    started: bool,
    finished: bool,
    blob_refs: usize,
    kinds: BTreeMap<String, usize>,
    error: Option<String>,
}

#[derive(serde::Serialize)]
struct AgentScriptReplayReport {
    path: String,
    ok: bool,
    records: usize,
    events: usize,
    expected_path: Option<String>,
    matched_expected: Option<bool>,
    first_mismatch: Option<AgentScriptReplayMismatch>,
    logical_sequence: Vec<AgentScriptReplayEvent>,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct AgentScriptReplayEvent {
    sequence: u64,
    kind: String,
    tick: Option<u64>,
    payload_hash: String,
    blob_refs: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
struct AgentScriptReplayMismatch {
    index: usize,
    actual: Option<AgentScriptReplayEvent>,
    expected: Option<AgentScriptReplayEvent>,
}

fn agent_script_run_command(
    options: &AgentScriptRunOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let report = match agent_script_run_input(options) {
        Ok(input) => agent_script_run_bundle(options, &input, adapter_registrars)?,
        Err(error) => AgentScriptRunReport {
            path: options.path.display().to_string(),
            ok: false,
            agents: 0,
            steps: 0,
            host_calls: 0,
            events_emitted: 0,
            final_status: None,
            trace_path: None,
            trace_records: 0,
            responses: Vec::new(),
            error: Some(error),
        },
    };
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: ok ({} step(s), {} host call(s))",
            report.path, report.steps, report.host_calls
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn agent_script_run_input(options: &AgentScriptRunOptions) -> Result<AgentScriptRunInput, String> {
    if is_awfagent_path(&options.path) {
        return agent_script_run_source_input(options);
    }
    if is_awfb_path(&options.path) {
        return agent_script_run_bundle_input(&options.path);
    }
    Err(format!(
        "{} is not an .awfagent source file or .awfb Agent bundle",
        options.path.display()
    ))
}

fn agent_script_run_source_input(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptRunInput, String> {
    let source = fs::read_to_string(&options.path)
        .map_err(|error| format!("failed to read {}: {error}", options.path.display()))?;
    let project = agent_script_compile_project_index(options)?;
    let compiled = arcweft_compiler::compile_agent_bundle_with_project(source, &project)
        .map_err(|error| error.to_string())?;
    Ok(AgentScriptRunInput {
        path: options.path.display().to_string(),
        agents: compiled.hir.agents().len(),
        bundle: compiled.bundle,
    })
}

fn agent_script_run_bundle_input(path: &Path) -> Result<AgentScriptRunInput, String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let bundle = ArcweftBundle::from_json_slice(&bytes)
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))?;
    if bundle.bundle_kind != BundleKind::AgentController {
        return Err(format!(
            "{} is a {} bundle, not an agent_controller bundle",
            path.display(),
            bundle.bundle_kind
        ));
    }
    let agents = usize::from(bundle.agent.is_some());
    Ok(AgentScriptRunInput {
        path: path.display().to_string(),
        agents,
        bundle,
    })
}

fn agent_script_run_bundle(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentScriptRunReport, ExitCode> {
    #[cfg(not(feature = "native-capture"))]
    let _ = adapter_registrars;
    #[cfg(feature = "native-capture")]
    if agent_script_run_uses_native_session(options) {
        return native::agent_script_run_native_bundle(options, input, adapter_registrars);
    }
    let session = CliAgentSession::new(options.signals.clone(), options.states.clone());
    let mut runner = AgentRunner::new(
        session,
        CollectingDebugSink::default(),
        NoopRagService,
        RuntimeAgentPolicy::new([
            RuntimeAgentCapability::Observe,
            RuntimeAgentCapability::Act,
            RuntimeAgentCapability::Capture,
            RuntimeAgentCapability::ResourceRead,
            RuntimeAgentCapability::Rag,
        ]),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        &input.bundle,
        AgentControllerRunConfig {
            max_steps: options.max_steps,
            max_ops_per_step: options.max_ops,
        },
    );
    let debug_events = runner.debug_mut().events.clone();
    let run_id = AgentRunId::new(options.run_id.clone()).map_err(|error| {
        eprintln!("error: invalid run id: {error}");
        ExitCode::from(2)
    })?;
    Ok(agent_script_run_report_from_result(
        options,
        input,
        run_result,
        &run_id,
        &debug_events,
    ))
}

#[cfg(feature = "native-capture")]
fn agent_script_run_uses_native_session(options: &AgentScriptRunOptions) -> bool {
    options.native_source.is_some() || options.native_profile.profile.is_some()
}

fn agent_script_run_report_from_result(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_result: Result<AgentControllerRunReport, impl std::fmt::Display>,
    run_id: &AgentRunId,
    debug_events: &[DebugEvent],
) -> AgentScriptRunReport {
    let trace_records = agent_trace_records(run_id, &agent_cli_session_id(), debug_events);
    let trace_result = options
        .trace_out
        .as_ref()
        .map(|path| write_agent_trace(path, &trace_records).map(|()| path.display().to_string()))
        .transpose();
    match (run_result, trace_result) {
        (Ok(run), Ok(trace_path)) => agent_script_run_success_report(
            &input.path,
            input.agents,
            run,
            trace_path,
            trace_records.len(),
        ),
        (Err(error), Ok(trace_path)) => agent_script_run_error_report(
            &input.path,
            input.agents,
            trace_path,
            trace_records.len(),
            error.to_string(),
        ),
        (_, Err(error)) => agent_script_run_error_report(
            &input.path,
            input.agents,
            options
                .trace_out
                .as_ref()
                .map(|path| path.display().to_string()),
            trace_records.len(),
            error,
        ),
    }
}

fn agent_script_run_success_report(
    path: &str,
    agents: usize,
    run: AgentControllerRunReport,
    trace_path: Option<String>,
    trace_records: usize,
) -> AgentScriptRunReport {
    AgentScriptRunReport {
        path: path.to_owned(),
        ok: true,
        agents,
        steps: run.steps,
        host_calls: run.host_calls,
        events_emitted: run.events_emitted,
        final_status: run.final_status.map(|status| format!("{status:?}")),
        trace_path,
        trace_records,
        responses: run.responses,
        error: None,
    }
}

fn agent_script_run_error_report(
    path: &str,
    agents: usize,
    trace_path: Option<String>,
    trace_records: usize,
    error: String,
) -> AgentScriptRunReport {
    AgentScriptRunReport {
        path: path.to_owned(),
        ok: false,
        agents,
        steps: 0,
        host_calls: 0,
        events_emitted: 0,
        final_status: None,
        trace_path,
        trace_records,
        responses: Vec::new(),
        error: Some(error),
    }
}

fn agent_cli_session_id() -> SessionId {
    SessionId::new("session.cli").expect("static session id")
}

fn agent_script_replay_command(options: &AgentScriptReplayOptions) -> Result<(), ExitCode> {
    let report =
        agent_script_replay_report(options).unwrap_or_else(|error| AgentScriptReplayReport {
            path: options.path.display().to_string(),
            ok: false,
            records: 0,
            events: 0,
            expected_path: options
                .expect
                .as_ref()
                .map(|path| path.display().to_string()),
            matched_expected: None,
            first_mismatch: None,
            logical_sequence: Vec::new(),
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        if let Some(expected) = &report.expected_path {
            println!(
                "{}: replay ok ({} event(s), matched {})",
                report.path, report.events, expected
            );
        } else {
            println!("{}: replay ok ({} event(s))", report.path, report.events);
        }
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn agent_script_replay_report(
    options: &AgentScriptReplayOptions,
) -> Result<AgentScriptReplayReport, String> {
    let records = read_and_validate_agent_trace_records(&options.path)?;
    let sequence = agent_trace_replay_sequence(&records);
    let expected = options
        .expect
        .as_ref()
        .map(|path| read_and_validate_agent_trace_records(path).map(|records| (path, records)))
        .transpose()?;
    let comparison = expected.as_ref().map(|(_, records)| {
        compare_agent_trace_replay(&sequence, &agent_trace_replay_sequence(records))
    });
    let matched_expected = comparison.as_ref().map(Option::is_none);
    let first_mismatch = comparison.flatten();
    Ok(AgentScriptReplayReport {
        path: options.path.display().to_string(),
        ok: first_mismatch.is_none(),
        records: records.len(),
        events: sequence.len(),
        expected_path: expected.map(|(path, _)| path.display().to_string()),
        matched_expected,
        first_mismatch: first_mismatch.clone(),
        logical_sequence: sequence,
        error: first_mismatch.map(|mismatch| {
            format!(
                "trace logical sequence diverged at replay event {}",
                mismatch.index
            )
        }),
    })
}

fn read_and_validate_agent_trace_records(path: &Path) -> Result<Vec<AgentTraceRecord>, String> {
    let records = read_agent_trace_records(path)?;
    validate_agent_trace(path, &records)?;
    Ok(records)
}

fn agent_trace_replay_sequence(records: &[AgentTraceRecord]) -> Vec<AgentScriptReplayEvent> {
    records
        .iter()
        .map(|record| AgentScriptReplayEvent {
            sequence: record.sequence,
            kind: agent_trace_kind_name(record.kind).to_owned(),
            tick: record.tick,
            payload_hash: record.payload_hash.as_str().to_owned(),
            blob_refs: record
                .blob_refs
                .iter()
                .map(|hash| hash.as_str().to_owned())
                .collect(),
        })
        .collect()
}

fn compare_agent_trace_replay(
    actual: &[AgentScriptReplayEvent],
    expected: &[AgentScriptReplayEvent],
) -> Option<AgentScriptReplayMismatch> {
    actual
        .iter()
        .zip(expected)
        .position(|(actual, expected)| !agent_trace_replay_events_match(actual, expected))
        .or_else(|| (actual.len() != expected.len()).then_some(actual.len().min(expected.len())))
        .map(|index| AgentScriptReplayMismatch {
            index,
            actual: actual.get(index).cloned(),
            expected: expected.get(index).cloned(),
        })
}

fn agent_trace_replay_events_match(
    actual: &AgentScriptReplayEvent,
    expected: &AgentScriptReplayEvent,
) -> bool {
    actual.kind == expected.kind
        && actual.tick == expected.tick
        && actual.payload_hash == expected.payload_hash
        && actual.blob_refs == expected.blob_refs
}

fn agent_script_trace_command(options: &AgentScriptTraceOptions) -> Result<(), ExitCode> {
    let report = read_agent_trace_records(&options.path)
        .and_then(|records| validate_agent_trace(&options.path, &records))
        .unwrap_or_else(|error| AgentScriptTraceReport {
            path: options.path.display().to_string(),
            ok: false,
            records: 0,
            run_id: None,
            sessions: Vec::new(),
            first_sequence: None,
            last_sequence: None,
            started: false,
            finished: false,
            blob_refs: 0,
            kinds: BTreeMap::new(),
            error: Some(error),
        });
    if options.json {
        print_json(&report)?;
    } else if report.ok {
        println!(
            "{}: ok ({} trace record(s), run {})",
            report.path,
            report.records,
            report.run_id.as_deref().unwrap_or("<unknown>")
        );
    } else if let Some(error) = &report.error {
        eprintln!("{}: {error}", report.path);
    }
    if report.ok {
        Ok(())
    } else {
        Err(ExitCode::FAILURE)
    }
}

fn read_agent_trace_records(path: &Path) -> Result<Vec<AgentTraceRecord>, String> {
    if !is_arcwx_path(path) {
        return Err(format!(
            "{} is not an .arcwx trace input path",
            path.display()
        ));
    }
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to decode {}: {error}", path.display()))
}

fn validate_agent_trace(
    path: &Path,
    records: &[AgentTraceRecord],
) -> Result<AgentScriptTraceReport, String> {
    let run_id = records
        .first()
        .map(|record| record.run_id.clone())
        .ok_or_else(|| "trace must contain at least one record".to_owned())?;
    let first_sequence = records.first().map(|record| record.sequence);
    let last_sequence = records.last().map(|record| record.sequence);
    validate_agent_trace_records(records, &run_id)?;
    Ok(AgentScriptTraceReport {
        path: path.display().to_string(),
        ok: true,
        records: records.len(),
        run_id: Some(run_id.as_str().to_owned()),
        sessions: agent_trace_sessions(records),
        first_sequence,
        last_sequence,
        started: records
            .first()
            .is_some_and(|record| record.kind == AgentTraceKind::RunStarted),
        finished: records
            .last()
            .is_some_and(|record| record.kind == AgentTraceKind::RunFinished),
        blob_refs: records.iter().map(|record| record.blob_refs.len()).sum(),
        kinds: agent_trace_kind_counts(records),
        error: None,
    })
}

fn validate_agent_trace_records(
    records: &[AgentTraceRecord],
    run_id: &AgentRunId,
) -> Result<(), String> {
    let first = records
        .first()
        .ok_or_else(|| "trace must contain at least one record".to_owned())?;
    if first.kind != AgentTraceKind::RunStarted {
        return Err("trace first record must be run_started".to_owned());
    }
    if !records
        .last()
        .is_some_and(|record| record.kind == AgentTraceKind::RunFinished)
    {
        return Err("trace last record must be run_finished".to_owned());
    }
    records
        .iter()
        .try_fold(None, |previous, record| {
            validate_agent_trace_record(record, run_id, previous)?;
            Ok(Some(record.sequence))
        })
        .map(|_| ())
}

fn validate_agent_trace_record(
    record: &AgentTraceRecord,
    run_id: &AgentRunId,
    previous_sequence: Option<u64>,
) -> Result<(), String> {
    if record.schema_version != 1 {
        return Err(format!(
            "trace record {} has unsupported schema_version {}",
            record.sequence, record.schema_version
        ));
    }
    if &record.run_id != run_id {
        return Err(format!(
            "trace record {} changes run_id from {} to {}",
            record.sequence,
            run_id.as_str(),
            record.run_id.as_str()
        ));
    }
    if previous_sequence.is_some_and(|sequence| record.sequence <= sequence) {
        return Err(format!(
            "trace record sequence {} is not strictly increasing",
            record.sequence
        ));
    }
    let expected_hash = stable_payload_hash(&record.payload);
    if record.payload_hash != expected_hash {
        return Err(format!(
            "trace record {} payload_hash mismatch: expected {}, got {}",
            record.sequence,
            expected_hash.as_str(),
            record.payload_hash.as_str()
        ));
    }
    if record.kind == AgentTraceKind::CaptureStored {
        validate_agent_trace_capture_blob_refs(record)?;
    }
    Ok(())
}

fn validate_agent_trace_capture_blob_refs(record: &AgentTraceRecord) -> Result<(), String> {
    let content_hash = record
        .payload
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            format!(
                "trace record {} capture payload is missing content_hash",
                record.sequence
            )
        })?;
    let content_hash = StableHash::new(content_hash.to_owned()).map_err(|error| {
        format!(
            "trace record {} capture content_hash is invalid: {error}",
            record.sequence
        )
    })?;
    if record.blob_refs.iter().any(|hash| hash == &content_hash) {
        return Ok(());
    }
    Err(format!(
        "trace record {} capture blob_refs does not include content_hash {}",
        record.sequence,
        content_hash.as_str()
    ))
}

fn agent_trace_sessions(records: &[AgentTraceRecord]) -> Vec<String> {
    records
        .iter()
        .filter_map(|record| {
            record
                .session_id
                .as_ref()
                .map(|session_id| session_id.as_str().to_owned())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn agent_trace_kind_counts(records: &[AgentTraceRecord]) -> BTreeMap<String, usize> {
    records.iter().fold(BTreeMap::new(), |mut counts, record| {
        *counts
            .entry(agent_trace_kind_name(record.kind).to_owned())
            .or_default() += 1;
        counts
    })
}

fn agent_trace_kind_name(kind: AgentTraceKind) -> &'static str {
    match kind {
        AgentTraceKind::RunStarted => "run_started",
        AgentTraceKind::VmStep => "vm_step",
        AgentTraceKind::HostCallRequested => "host_call_requested",
        AgentTraceKind::ObservationReceived => "observation_received",
        AgentTraceKind::ActionCompleted => "action_completed",
        AgentTraceKind::CaptureStored => "capture_stored",
        AgentTraceKind::AssertionEvaluated => "assertion_evaluated",
        AgentTraceKind::RagQueryCompleted => "rag_query_completed",
        AgentTraceKind::DiagnosticEmitted => "diagnostic_emitted",
        AgentTraceKind::RunFinished => "run_finished",
    }
}

fn is_awfagent_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "awfagent")
}

fn is_awfb_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "awfb")
}

fn is_arcwx_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "arcwx")
}

fn parse_agent_script_signal_arg(value: &str) -> Result<AgentScriptSignalArg, String> {
    let (id, raw_value) = value
        .split_once('=')
        .ok_or_else(|| "signal must be formatted as id=value".to_owned())?;
    let id = id.trim().trim_start_matches('@').to_owned();
    if id.is_empty() {
        return Err("signal id must not be empty".to_owned());
    }
    let (value, ty) = parse_agent_script_value(raw_value.trim());
    Ok(AgentScriptSignalArg { id, value, ty })
}

fn parse_agent_script_state_arg(value: &str) -> Result<AgentScriptStateArg, String> {
    let (path, raw_value) = value
        .split_once('=')
        .ok_or_else(|| "state must be formatted as path=value".to_owned())?;
    let path = path.trim().to_owned();
    if path.is_empty() {
        return Err("state path must not be empty".to_owned());
    }
    let (value, _) = parse_agent_script_value(raw_value.trim());
    Ok(AgentScriptStateArg { path, value })
}

fn parse_agent_script_value(raw_value: &str) -> (AgentValue, TypeKind) {
    match raw_value {
        "true" => (AgentValue::Bool(true), TypeKind::Bool),
        "false" => (AgentValue::Bool(false), TypeKind::Bool),
        _ => raw_value.parse::<i64>().map_or_else(
            |_| {
                (
                    AgentValue::String(
                        raw_value
                            .strip_prefix('"')
                            .and_then(|value| value.strip_suffix('"'))
                            .unwrap_or(raw_value)
                            .to_owned(),
                    ),
                    TypeKind::String,
                )
            },
            |value| (AgentValue::I64(value), TypeKind::I64),
        ),
    }
}

#[derive(Clone, Debug, Default)]
struct CollectingDebugSink {
    events: Vec<DebugEvent>,
}

impl DebugEventSink for CollectingDebugSink {
    type Error = Infallible;

    fn append(&mut self, event: &DebugEvent) -> Result<(), Self::Error> {
        self.events.push(event.clone());
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

fn agent_trace_records(
    run_id: &AgentRunId,
    session_id: &SessionId,
    events: &[DebugEvent],
) -> Vec<AgentTraceRecord> {
    let mut records = Vec::with_capacity(events.len() + 2);
    records.push(agent_trace_record(
        run_id,
        Some(session_id),
        0,
        None,
        AgentTraceKind::RunStarted,
        serde_json::json!({ "source": "arcw agent script run" }),
    ));
    records.extend(events.iter().map(|event| {
        agent_trace_record(
            run_id,
            Some(&event.session_id),
            event.sequence,
            event.tick,
            agent_trace_kind(event.kind),
            event.payload.clone(),
        )
    }));
    records.push(agent_trace_record(
        run_id,
        Some(session_id),
        events
            .last()
            .map_or(1, |event| event.sequence.saturating_add(1)),
        None,
        AgentTraceKind::RunFinished,
        serde_json::json!({ "debug_events": events.len() }),
    ));
    records
}

fn agent_trace_record(
    run_id: &AgentRunId,
    session_id: Option<&SessionId>,
    sequence: u64,
    tick: Option<u64>,
    kind: AgentTraceKind,
    payload: serde_json::Value,
) -> AgentTraceRecord {
    let blob_refs = agent_trace_blob_refs(kind, &payload);
    AgentTraceRecord {
        schema_version: 1,
        run_id: run_id.clone(),
        session_id: session_id.cloned(),
        sequence,
        tick,
        kind,
        payload_hash: stable_payload_hash(&payload),
        payload,
        blob_refs,
    }
}

fn agent_trace_blob_refs(kind: AgentTraceKind, payload: &serde_json::Value) -> Vec<StableHash> {
    if kind != AgentTraceKind::CaptureStored {
        return Vec::new();
    }
    payload
        .get("content_hash")
        .and_then(serde_json::Value::as_str)
        .and_then(|hash| StableHash::new(hash.to_owned()).ok())
        .into_iter()
        .collect()
}

fn agent_trace_kind(kind: DebugEventKind) -> AgentTraceKind {
    match kind {
        DebugEventKind::RunStarted | DebugEventKind::SessionStarted => AgentTraceKind::RunStarted,
        DebugEventKind::RunFinished | DebugEventKind::SessionFinished => {
            AgentTraceKind::RunFinished
        }
        DebugEventKind::StepStarted | DebugEventKind::StepFinished => AgentTraceKind::VmStep,
        DebugEventKind::Observation => AgentTraceKind::ObservationReceived,
        DebugEventKind::Action => AgentTraceKind::ActionCompleted,
        DebugEventKind::Capture => AgentTraceKind::CaptureStored,
        DebugEventKind::Diagnostic | DebugEventKind::ReplCell => AgentTraceKind::DiagnosticEmitted,
        DebugEventKind::RagQuery => AgentTraceKind::RagQueryCompleted,
    }
}

fn stable_payload_hash(payload: &serde_json::Value) -> StableHash {
    let bytes = serde_json::to_vec(payload).unwrap_or_default();
    StableHash::new(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
        .expect("generated trace payload hash is nonempty")
}

fn write_agent_trace(path: &Path, records: &[AgentTraceRecord]) -> Result<(), String> {
    if !is_arcwx_path(path) {
        return Err(format!(
            "{} is not an .arcwx trace output path",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
    }
    let bytes = serde_json::to_vec_pretty(records)
        .map_err(|error| format!("failed to encode trace: {error}"))?;
    fs::write(path, bytes).map_err(|error| format!("failed to write {}: {error}", path.display()))
}

fn agent_script_project_index(
    signals: &[AgentScriptSignalArg],
) -> Result<ProjectSemanticIndex, String> {
    let mut project = ProjectSemanticIndex::new(ProgramHash::new("cli-agent-run"));
    for signal in agent_script_signal_symbols(signals)? {
        project = project.with_entity(signal);
    }
    Ok(project)
}

fn agent_script_signal_symbols(
    signals: &[AgentScriptSignalArg],
) -> Result<Vec<EntitySymbol>, String> {
    signals
        .iter()
        .map(|signal| {
            let id = SemaPublicId::try_new(signal.id.clone()).map_err(|error| error.to_string())?;
            Ok(agent_script_signal_symbol(signal, id))
        })
        .collect()
}

fn agent_script_signal_symbol(signal: &AgentScriptSignalArg, id: SemaPublicId) -> EntitySymbol {
    EntitySymbol::new(
        id,
        EntityType::new(EntityKind::Signal, Some(signal.ty.clone())),
        SourceAnchor::generated(),
        SemanticHash::new(format!("cli-signal:{}", signal.id)),
    )
}

#[derive(Debug)]
struct CliAgentSession {
    tick: u64,
    signals: BTreeMap<String, AgentValue>,
    states: BTreeMap<String, AgentValue>,
    captures: u64,
}

impl CliAgentSession {
    fn new(signals: Vec<AgentScriptSignalArg>, states: Vec<AgentScriptStateArg>) -> Self {
        Self {
            tick: 0,
            signals: signals
                .into_iter()
                .map(|signal| (signal.id, signal.value))
                .collect(),
            states: states
                .into_iter()
                .map(|state| (state.path, state.value))
                .collect(),
            captures: 0,
        }
    }

    fn observation(&self) -> ObservationEnvelope {
        ObservationEnvelope {
            tick: self.tick,
            frame_id: format!("cli.frame.{}", self.tick),
            state_hash: format!("cli.state.{}", self.tick),
            render_hash: format!("cli.render.{}", self.tick),
            signals: self.signals.clone(),
            payload: serde_json::json!({
                "source": "arcw agent script run",
                "deterministic_cli_session": true,
                "state": agent_values_to_json(&self.states),
            }),
        }
    }
}

impl AgentSession for CliAgentSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.cli".to_owned(),
            program_hash: "cli-agent-run".to_owned(),
            profile: Some("cli".to_owned()),
            capabilities: vec![
                "agent.observe".to_owned(),
                "agent.wait".to_owned(),
                "agent.capture".to_owned(),
                "agent.act".to_owned(),
                "agent.resource.read".to_owned(),
            ],
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        Ok(self.observation())
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        let before_tick = self.tick;
        self.tick = self.tick.saturating_add(1);
        Ok(ActionResult {
            accepted: true,
            before_tick,
            after_tick: self.tick,
            before_state_hash: format!("cli.state.{before_tick}"),
            after_state_hash: format!("cli.state.{}", self.tick),
        })
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        self.captures = self.captures.saturating_add(1);
        let media_type = match request.format {
            CaptureFormat::Png => "image/png",
            CaptureFormat::RawRgba => "application/octet-stream",
            CaptureFormat::Svg => "image/svg+xml",
        }
        .to_owned();
        let uri = format!("agent://capture/cli/{}-{}", request.name, self.captures);
        Ok(CaptureResult {
            uri: AgentResourceUri::new(uri).expect("generated capture uri is nonempty"),
            content_hash: format!("cli-capture-{:016x}", self.captures),
            media_type,
            byte_len: 0,
        })
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        Ok(AgentResource {
            uri: uri.to_owned(),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "cli-resource".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({
                "uri": uri,
                "source": "arcw agent script run"
            })),
        })
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        self.tick = self.tick.saturating_add(u64::from(count.max(1)));
        Ok(self.observation())
    }
}

fn agent_values_to_json(values: &BTreeMap<String, AgentValue>) -> serde_json::Value {
    serde_json::Value::Object(
        values
            .iter()
            .map(|(key, value)| (key.clone(), agent_value_to_json(value)))
            .collect(),
    )
}

fn agent_value_to_json(value: &AgentValue) -> serde_json::Value {
    match value {
        AgentValue::Null => serde_json::Value::Null,
        AgentValue::Bool(value) => serde_json::Value::Bool(*value),
        AgentValue::I64(value) => serde_json::Value::Number((*value).into()),
        AgentValue::U64(value) => serde_json::Number::from(*value).into(),
        AgentValue::F64(value) => serde_json::Number::from_f64(*value)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        AgentValue::String(value) => serde_json::Value::String(value.clone()),
        AgentValue::Entity(value) => serde_json::Value::String(value.as_str().to_owned()),
        AgentValue::List(values) => {
            serde_json::Value::Array(values.iter().map(agent_value_to_json).collect())
        }
        AgentValue::Map(values) => agent_values_to_json(values),
    }
}
