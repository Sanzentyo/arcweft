use super::commands::{AgentCommand, AgentScriptCommand};
use super::project::ProfileOptions;
use super::runtime::{
    CliRuntimeExecutorTier, CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    CliRuntimeStepMode, parse_runtime_binding_arg, parse_runtime_pure_workers,
};
use super::shared::print_json;
use arcweft_agent_protocol::{
    AgentResource, AgentResourceBody, AgentResourceKind,
    ids::{AgentResourceUri, SessionId},
    protocol::{
        ActionResult, AgentAction, AgentHostResponse, AgentSessionInfo, CaptureFormat,
        CaptureRequest, CaptureResult, ObservationEnvelope, ObserveRequest,
    },
    value::AgentValue,
};
use arcweft_agent_runner::{
    AgentControllerRunConfig, AgentRunner, AgentRunnerConfig, AgentSession, NoopRagService,
    RuntimeAgentCapability, RuntimeAgentPolicy,
};
use arcweft_core::value::RuntimeBinding;
use arcweft_debug_model::sink::NullDebugEventSink;
use arcweft_id::PublicId as SemaPublicId;
use arcweft_lang_sema::{
    project_index::{EntitySymbol, ProgramHash, ProjectSemanticIndex, SemanticHash},
    types::{EntityKind, EntityType, TypeKind},
};
use arcweft_runtime_host::NativeAdapterRegistrar;
use arcweft_source::SourceAnchor;
use clap::{Args, ValueEnum};
use std::collections::BTreeMap;
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
use arcweft_render_text::LineDisplayCatalog;
#[cfg(feature = "native-capture")]
use arcweft_runtime_host::NativeTaskBridge;

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
pub(super) struct AgentScriptRunOptions {
    path: PathBuf,
    #[arg(long)]
    json: bool,
    #[arg(long, default_value_t = 256)]
    max_steps: usize,
    #[arg(long, default_value_t = 1024)]
    max_ops: usize,
    #[arg(long = "signal", value_parser = parse_agent_script_signal_arg)]
    signals: Vec<AgentScriptSignalArg>,
}

#[derive(Clone, Debug)]
struct AgentScriptSignalArg {
    id: String,
    value: AgentValue,
    ty: TypeKind,
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
        AgentCommand::Script { command } => agent_script_command(command),
        command => native::agent_command(command, adapter_registrars),
    }
}

#[cfg(not(feature = "native-capture"))]
pub(super) fn agent_command(
    command: AgentCommand,
    _adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Script { command } => agent_script_command(command),
        AgentCommand::Observe(_) | AgentCommand::HitTest(_) | AgentCommand::Mcp(_) => {
            eprintln!("error: this arcw agent command requires the native-capture feature");
            Err(ExitCode::FAILURE)
        }
    }
}

pub(super) fn agent_script_command(command: AgentScriptCommand) -> Result<(), ExitCode> {
    match command {
        AgentScriptCommand::Check(options) => agent_script_check_command(&options),
        AgentScriptCommand::Run(options) => agent_script_run_command(&options),
    }
}

#[derive(serde::Serialize)]
struct AgentScriptCheckReport {
    path: String,
    ok: bool,
    agents: usize,
    error: Option<String>,
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
    responses: Vec<AgentHostResponse>,
    error: Option<String>,
}

fn agent_script_run_command(options: &AgentScriptRunOptions) -> Result<(), ExitCode> {
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
    let project = agent_script_project_index(&options.signals).map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::from(2)
    })?;
    let report = match arcweft_compiler::compile_agent_bundle_with_project(source, &project) {
        Ok(compiled) => {
            let session = CliAgentSession::new(options.signals.clone());
            let mut runner = AgentRunner::new(
                session,
                NullDebugEventSink,
                NoopRagService,
                RuntimeAgentPolicy::new([
                    RuntimeAgentCapability::Observe,
                    RuntimeAgentCapability::Act,
                    RuntimeAgentCapability::Capture,
                    RuntimeAgentCapability::ResourceRead,
                    RuntimeAgentCapability::Rag,
                ]),
                AgentRunnerConfig::new(SessionId::new("session.cli").expect("static session id")),
            );
            match runner.run_controller_bundle(
                &compiled.bundle,
                AgentControllerRunConfig {
                    max_steps: options.max_steps,
                    max_ops_per_step: options.max_ops,
                },
            ) {
                Ok(run) => AgentScriptRunReport {
                    path: options.path.display().to_string(),
                    ok: true,
                    agents: compiled.hir.agents().len(),
                    steps: run.steps,
                    host_calls: run.host_calls,
                    events_emitted: run.events_emitted,
                    final_status: run.final_status.map(|status| format!("{status:?}")),
                    responses: run.responses,
                    error: None,
                },
                Err(error) => AgentScriptRunReport {
                    path: options.path.display().to_string(),
                    ok: false,
                    agents: compiled.hir.agents().len(),
                    steps: 0,
                    host_calls: 0,
                    events_emitted: 0,
                    final_status: None,
                    responses: Vec::new(),
                    error: Some(error.to_string()),
                },
            }
        }
        Err(error) => AgentScriptRunReport {
            path: options.path.display().to_string(),
            ok: false,
            agents: 0,
            steps: 0,
            host_calls: 0,
            events_emitted: 0,
            final_status: None,
            responses: Vec::new(),
            error: Some(error.to_string()),
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

fn is_awfagent_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == "awfagent")
}

fn parse_agent_script_signal_arg(value: &str) -> Result<AgentScriptSignalArg, String> {
    let (id, raw_value) = value
        .split_once('=')
        .ok_or_else(|| "signal must be formatted as id=value".to_owned())?;
    let id = id.trim().trim_start_matches('@').to_owned();
    if id.is_empty() {
        return Err("signal id must not be empty".to_owned());
    }
    let raw_value = raw_value.trim();
    let (value, ty) = match raw_value {
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
    };
    Ok(AgentScriptSignalArg { id, value, ty })
}

fn agent_script_project_index(
    signals: &[AgentScriptSignalArg],
) -> Result<ProjectSemanticIndex, String> {
    signals.iter().try_fold(
        ProjectSemanticIndex::new(ProgramHash::new("cli-agent-run")),
        |project, signal| {
            let id = SemaPublicId::try_new(signal.id.clone()).map_err(|error| error.to_string())?;
            Ok(project.with_entity(EntitySymbol::new(
                id,
                EntityType::new(EntityKind::Signal, Some(signal.ty.clone())),
                SourceAnchor::generated(),
                SemanticHash::new(format!("cli-signal:{}", signal.id)),
            )))
        },
    )
}

#[derive(Debug)]
struct CliAgentSession {
    tick: u64,
    signals: BTreeMap<String, AgentValue>,
    captures: u64,
}

impl CliAgentSession {
    fn new(signals: Vec<AgentScriptSignalArg>) -> Self {
        Self {
            tick: 0,
            signals: signals
                .into_iter()
                .map(|signal| (signal.id, signal.value))
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
                "deterministic_cli_session": true
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
