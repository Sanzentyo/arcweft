use super::commands::{AgentCommand, AgentRagCommand, AgentScriptCommand};
use super::project::ProfileOptions;
use super::runtime::{
    CliRuntimeExecutorTier, CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    CliRuntimeStepMode, parse_runtime_binding_arg, parse_runtime_pure_workers,
};
use super::shared::print_json;
use arcweft_agent_protocol::{
    AgentResource, AgentResourceBody, AgentResourceKind,
    artifact::RequiredEntity,
    ids::{AgentResourceUri, AgentRunId, PublicId as AgentPublicId, SessionId, StableHash},
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
    chunk::{
        ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass, SourceAnchor as DebugSourceAnchor,
    },
    event::{DebugEvent, DebugEventKind},
    rag::{RagContextItem, RagContextPack, RagQuery, SearchChannel, SearchHit},
    script::{DebugScriptRun, DebugScriptRunFinish, DebugScriptRunOutcome},
    session::{DebugSession, DebugSessionStatus},
    sink::DebugEventSink,
};
use arcweft_debug_sqlite::store::DebugStore;
use arcweft_id::PublicId as SemaPublicId;
use arcweft_lang_sema::{
    project_index::{
        EntitySymbol, ProgramHash, ProjectSemanticIndex, SemanticHash,
        project_semantic_index_from_hir,
    },
    types::{EntityKind, EntityType, TypeKind},
};
use arcweft_rag::fusion::{FusionConfig, reciprocal_rank_fusion};
use arcweft_runtime_host::NativeAdapterRegistrar;
use arcweft_source::{SourceAnchor, SourceName};
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
pub(super) struct AgentReplOptions {
    #[cfg(feature = "native-capture")]
    path: Option<PathBuf>,
    #[cfg(feature = "native-capture")]
    #[command(flatten)]
    profile: ProfileOptions,
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
    #[arg(long, default_value_t = 1)]
    steps: usize,
    #[cfg(feature = "native-capture")]
    #[arg(long = "capture-step")]
    capture_step: Option<usize>,
    #[cfg(feature = "native-capture")]
    #[arg(long, value_enum, default_value_t = CliRuntimeStepMode::Drain)]
    mode: CliRuntimeStepMode,
    #[cfg(feature = "native-capture")]
    #[arg(long, default_value_t = 64)]
    max_ops: usize,
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
    #[cfg(feature = "native-capture")]
    #[arg(long = "debug-db")]
    debug_db: Option<PathBuf>,
    #[arg(long)]
    trace: Option<PathBuf>,
    #[arg(long = "read-only")]
    read_only: bool,
    #[arg(long)]
    connect: Option<String>,
    #[arg(long)]
    input: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentRagQueryOptions {
    #[arg(long)]
    trace: Option<PathBuf>,
    #[arg(long)]
    source: Vec<PathBuf>,
    #[arg(long)]
    query: String,
    #[arg(long = "root")]
    roots: Vec<String>,
    #[arg(long, default_value_t = 1)]
    graph_depth: u32,
    #[arg(long, default_value_t = 8)]
    limit: usize,
    #[arg(long, default_value_t = 32 * 1024)]
    max_context_bytes: usize,
    #[arg(long, value_parser = parse_agent_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
    #[arg(long = "debug-db")]
    debug_db: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentRagExplainOptions {
    query_id: String,
    #[arg(long = "debug-db")]
    debug_db: PathBuf,
    #[arg(long, value_parser = parse_agent_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone, Debug)]
pub(super) struct AgentRagContextReadOptions {
    query_id: String,
    chunk_id: String,
    #[arg(long = "debug-db")]
    debug_db: PathBuf,
    #[arg(long, default_value_t = 4096)]
    max_bytes: usize,
    #[arg(long, value_parser = parse_agent_privacy_class, default_value = "project")]
    max_privacy: PrivacyClass,
    #[arg(long)]
    json: bool,
}

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
    #[arg(long = "blob-dir")]
    blob_dir: Option<PathBuf>,
    #[arg(long = "debug-db")]
    debug_db: Option<PathBuf>,
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
    #[arg(long = "blob-dir")]
    blob_dir: Option<PathBuf>,
    #[arg(long)]
    json: bool,
}

#[derive(Clone, Debug)]
pub(in crate::app::agent) struct AgentScriptSignalArg {
    id: String,
    value: AgentValue,
    ty: TypeKind,
}

#[derive(Clone, Debug)]
pub(in crate::app::agent) struct AgentScriptStateArg {
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
        AgentCommand::Rag { command } => agent_rag_command(command),
        AgentCommand::Script { command } => agent_script_command(*command, adapter_registrars),
        command => native::agent_command(command, adapter_registrars),
    }
}

#[cfg(not(feature = "native-capture"))]
pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Rag { command } => agent_rag_command(command),
        AgentCommand::Script { command } => agent_script_command(*command, adapter_registrars),
        AgentCommand::Observe(_)
        | AgentCommand::HitTest(_)
        | AgentCommand::Mcp(_)
        | AgentCommand::Repl(_) => {
            eprintln!("error: this arcw agent command requires the native-capture feature");
            Err(ExitCode::FAILURE)
        }
    }
}

fn agent_rag_command(command: AgentRagCommand) -> Result<(), ExitCode> {
    match command {
        AgentRagCommand::Query(options) => agent_rag_query_command(&options),
        AgentRagCommand::Explain(options) => agent_rag_explain_command(&options),
        AgentRagCommand::ContextRead(options) => agent_rag_context_read_command(&options),
    }
}

fn agent_rag_query_command(options: &AgentRagQueryOptions) -> Result<(), ExitCode> {
    match agent_rag_query_result(options) {
        Ok(result) => {
            if let Some(path) = &options.debug_db
                && let Err(error) = persist_agent_rag_query_result(path, &result)
            {
                eprintln!("{}: {error}", path.display());
                return Err(ExitCode::FAILURE);
            }
            let pack = result.pack;
            if options.json {
                print_json(&pack)?;
            } else {
                println!(
                    "{}: {} item(s), truncated={}",
                    agent_rag_query_input_label(options),
                    pack.items.len(),
                    pack.truncated
                );
                for item in &pack.items {
                    println!(
                        "- {} [{}] score={:.6}",
                        item.title,
                        item.chunk_id.as_str(),
                        item.fused_score
                    );
                }
            }
            Ok(())
        }
        Err(error) => {
            eprintln!("{}: {error}", agent_rag_query_input_label(options));
            Err(ExitCode::FAILURE)
        }
    }
}

fn agent_rag_explain_command(options: &AgentRagExplainOptions) -> Result<(), ExitCode> {
    let query_id = options.query_id.trim();
    if query_id.is_empty() {
        eprintln!("agent rag explain: query id must not be empty");
        return Err(ExitCode::from(2));
    }
    match agent_rag_persisted_audit(&options.debug_db, query_id, options.max_privacy) {
        Ok(audit) => {
            let report = AgentRagExplainReport {
                path: options.debug_db.display().to_string(),
                query_id: query_id.to_owned(),
                max_privacy: options.max_privacy,
                status: audit.status,
                created_unix_ms: audit.created_unix_ms,
                query: audit.pack.query,
                item_count: audit.pack.items.len(),
                truncated: audit.pack.truncated,
                items: audit
                    .pack
                    .items
                    .into_iter()
                    .map(agent_rag_explain_item_report)
                    .collect(),
            };
            if options.json {
                return print_json(&report);
            }
            println!(
                "{}: RAG query {} status={} item(s)={} truncated={} max_privacy={}",
                report.path,
                report.query_id,
                report.status,
                report.item_count,
                report.truncated,
                report.max_privacy.as_str()
            );
            for item in &report.items {
                println!(
                    "- {} [{}] score={:.6}",
                    item.title,
                    item.chunk_id.as_str(),
                    item.fused_score
                );
            }
            Ok(())
        }
        Err(error) => {
            eprintln!("agent rag explain: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn agent_rag_context_read_command(options: &AgentRagContextReadOptions) -> Result<(), ExitCode> {
    let query_id = options.query_id.trim();
    if query_id.is_empty() {
        eprintln!("agent rag context-read: query id must not be empty");
        return Err(ExitCode::from(2));
    }
    if options.chunk_id.trim().is_empty() {
        eprintln!("agent rag context-read: chunk id must not be empty");
        return Err(ExitCode::from(2));
    }
    if options.max_bytes == 0 {
        eprintln!("agent rag context-read --max-bytes must be at least 1");
        return Err(ExitCode::from(2));
    }
    match agent_rag_context_read_report(options, query_id) {
        Ok(report) => {
            if options.json {
                return print_json(&report);
            }
            println!(
                "{}: RAG context {} from query {} bytes={} truncated={}",
                report.path,
                report.chunk_id.as_str(),
                report.query_id,
                report.item.body.len(),
                report.body_truncated
            );
            println!("{}", report.item.body);
            Ok(())
        }
        Err(error) => {
            eprintln!("agent rag context-read: {error}");
            Err(ExitCode::FAILURE)
        }
    }
}

fn agent_rag_context_read_report(
    options: &AgentRagContextReadOptions,
    query_id: &str,
) -> Result<AgentRagContextReadReport, String> {
    let audit = agent_rag_persisted_audit(&options.debug_db, query_id, options.max_privacy)?;
    let mut item = audit
        .pack
        .items
        .into_iter()
        .find(|item| item.chunk_id.as_str() == options.chunk_id.trim())
        .ok_or_else(|| {
            format!(
                "could not find chunk id `{}` in persisted RAG query `{query_id}`",
                options.chunk_id.trim()
            )
        })?;
    let (body, body_truncated) = truncate_utf8(&item.body, options.max_bytes);
    item.body = body;
    Ok(AgentRagContextReadReport {
        path: options.debug_db.display().to_string(),
        query_id: query_id.to_owned(),
        chunk_id: item.chunk_id.clone(),
        max_privacy: options.max_privacy,
        max_bytes: options.max_bytes,
        body_truncated,
        item,
    })
}

fn agent_rag_persisted_audit(
    path: &Path,
    query_id: &str,
    max_privacy: PrivacyClass,
) -> Result<arcweft_debug_sqlite::store::DebugRagQueryAudit, String> {
    let store = DebugStore::open(path).map_err(|error| {
        format!(
            "failed to open persisted RAG debug DB `{}`: {error}",
            path.display()
        )
    })?;
    store
        .rag_query_audit_with_max_privacy(query_id, max_privacy)
        .map_err(|error| {
            format!(
                "failed to read persisted RAG query `{query_id}` from `{}`: {error}",
                path.display()
            )
        })
}

fn agent_rag_explain_item_report(item: RagContextItem) -> AgentRagExplainItemReport {
    AgentRagExplainItemReport {
        chunk_id: item.chunk_id,
        kind: item.kind,
        title: item.title,
        fused_score: item.fused_score,
        channels: item.channels,
        entity_ids: item.entity_ids,
        source_anchor: item.source_anchor,
    }
}

fn agent_rag_query_input_label(options: &AgentRagQueryOptions) -> String {
    match (&options.trace, options.source.as_slice()) {
        (Some(trace), []) => trace.display().to_string(),
        (None, [source]) => source.display().to_string(),
        (Some(trace), [source]) => {
            format!("trace {} + source {}", trace.display(), source.display())
        }
        (Some(trace), sources) => {
            format!("trace {} + {} sources", trace.display(), sources.len())
        }
        (None, [_source, rest @ ..]) => format!("{} sources", rest.len().saturating_add(1)),
        (None, []) => "agent rag query".to_owned(),
    }
}

#[derive(Clone)]
struct AgentRagCandidate {
    chunk: DebugChunk,
    preferred_channel: SearchChannel,
}

struct AgentRagQueryResult {
    pack: RagContextPack,
    candidates: Vec<AgentRagCandidate>,
}

#[derive(serde::Serialize)]
struct AgentRagExplainReport {
    path: String,
    query_id: String,
    max_privacy: PrivacyClass,
    status: String,
    created_unix_ms: i64,
    query: RagQuery,
    item_count: usize,
    truncated: bool,
    items: Vec<AgentRagExplainItemReport>,
}

#[derive(serde::Serialize)]
struct AgentRagExplainItemReport {
    chunk_id: ChunkId,
    kind: ChunkSourceKind,
    title: String,
    fused_score: f64,
    channels: BTreeSet<SearchChannel>,
    entity_ids: Vec<AgentPublicId>,
    source_anchor: Option<DebugSourceAnchor>,
}

#[derive(serde::Serialize)]
struct AgentRagContextReadReport {
    path: String,
    query_id: String,
    chunk_id: ChunkId,
    max_privacy: PrivacyClass,
    max_bytes: usize,
    body_truncated: bool,
    item: RagContextItem,
}

fn agent_rag_query_result(options: &AgentRagQueryOptions) -> Result<AgentRagQueryResult, String> {
    let query_text = options.query.trim();
    if options.trace.is_none() && options.source.is_empty() {
        return Err("agent rag query requires --trace, --source, or both".to_owned());
    }
    if query_text.is_empty() {
        return Err("agent rag query requires a non-empty --query".to_owned());
    }
    if options.limit == 0 {
        return Err("agent rag query --limit must be at least 1".to_owned());
    }
    if options.max_context_bytes == 0 {
        return Err("agent rag query --max-context-bytes must be at least 1".to_owned());
    }
    let roots = agent_rag_roots(&options.roots)?;
    let mut candidates = Vec::new();
    let mut seed_parts = Vec::new();
    if let Some(trace) = &options.trace {
        let records = read_and_validate_agent_trace_records(trace)?;
        let trace_report = validate_agent_trace(trace, &records, None)?;
        seed_parts.push(agent_trace_rag_seed(trace, &records));
        candidates.extend(agent_trace_rag_candidates(&trace_report, &records)?);
    }
    let source_paths = agent_rag_source_paths(&options.source)?;
    for source in &source_paths {
        let source_index = agent_source_rag_index(source)?;
        seed_parts.push(source_index.seed);
        candidates.extend(source_index.candidates);
    }
    let query_candidates = agent_rag_query_allowed_candidates(options, &candidates);
    let program_hash = agent_rag_program_hash(&seed_parts)?;
    let query = RagQuery {
        query_id: agent_content_hash(format!(
            "{}:{}:{}:{}:{}:{}",
            query_text,
            program_hash.as_str(),
            options.graph_depth,
            options.limit,
            options.max_context_bytes,
            options.max_privacy.as_str()
        )),
        text: query_text.to_owned(),
        program_hash,
        roots,
        graph_depth: options.graph_depth,
        limit: options.limit,
        max_context_bytes: options.max_context_bytes,
    };
    let pack = agent_trace_rag_pack_from_candidates(options, query, &query_candidates);
    Ok(AgentRagQueryResult { pack, candidates })
}

fn agent_trace_rag_pack_from_candidates(
    options: &AgentRagQueryOptions,
    query: RagQuery,
    candidates: &[AgentRagCandidate],
) -> RagContextPack {
    let fused = reciprocal_rank_fusion(
        &agent_trace_rag_ranked_lists(candidates, &query),
        &FusionConfig::default(),
        candidates.len(),
    );
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.chunk.id.clone(), &candidate.chunk))
        .collect::<BTreeMap<_, _>>();
    let mut items = Vec::new();
    let mut used_bytes = 0usize;
    let mut truncated = false;
    let mut selected_semantic_hashes = BTreeSet::new();
    let mut selected_source_anchors = Vec::new();
    for hit in fused {
        if items.len() >= options.limit {
            break;
        }
        let Some(chunk) = by_id.get(&hit.chunk_id).copied() else {
            continue;
        };
        if !agent_rag_select_context_chunk(
            chunk,
            &mut selected_semantic_hashes,
            &mut selected_source_anchors,
        ) {
            continue;
        }
        let remaining = options.max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (body, body_truncated) = truncate_utf8(&chunk.body, remaining);
        truncated |= body_truncated;
        used_bytes = used_bytes.saturating_add(body.len());
        items.push(RagContextItem {
            chunk_id: chunk.id.clone(),
            kind: chunk.source_kind,
            title: chunk.title.clone(),
            body,
            fused_score: hit.fused_score,
            channels: hit.channels,
            entity_ids: chunk.entity_ids.clone(),
            source_anchor: chunk.source_anchor.clone(),
        });
        if body_truncated {
            break;
        }
    }
    RagContextPack {
        schema_version: 1,
        query,
        items,
        truncated,
    }
}

pub(in crate::app) fn agent_rag_select_context_chunk(
    chunk: &DebugChunk,
    selected_semantic_hashes: &mut BTreeSet<StableHash>,
    selected_source_anchors: &mut Vec<DebugSourceAnchor>,
) -> bool {
    if chunk
        .semantic_hash
        .as_ref()
        .is_some_and(|hash| selected_semantic_hashes.contains(hash))
    {
        return false;
    }
    if chunk.source_anchor.as_ref().is_some_and(|anchor| {
        selected_source_anchors
            .iter()
            .any(|selected| agent_rag_source_anchors_overlap(selected, anchor))
    }) {
        return false;
    }

    if let Some(hash) = &chunk.semantic_hash {
        selected_semantic_hashes.insert(hash.clone());
    }
    if let Some(anchor) = &chunk.source_anchor {
        selected_source_anchors.push(anchor.clone());
    }
    true
}

fn agent_rag_source_anchors_overlap(left: &DebugSourceAnchor, right: &DebugSourceAnchor) -> bool {
    left.path == right.path
        && ((left.start_byte == right.start_byte && left.end_byte == right.end_byte)
            || (left.start_byte < right.end_byte && right.start_byte < left.end_byte))
}

fn agent_rag_query_allowed_candidates(
    options: &AgentRagQueryOptions,
    candidates: &[AgentRagCandidate],
) -> Vec<AgentRagCandidate> {
    candidates
        .iter()
        .filter(|candidate| candidate.chunk.privacy.is_allowed_by(options.max_privacy))
        .cloned()
        .collect()
}

fn persist_agent_rag_query_result(path: &Path, result: &AgentRagQueryResult) -> Result<(), String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("agent rag query failed to open debug DB: {error}"))?;
    store
        .upsert_program(&result.pack.query.program_hash, None, None, 0)
        .map_err(|error| format!("agent rag query failed to index RAG program: {error}"))?;
    for candidate in &result.candidates {
        let mut chunk = candidate.chunk.clone();
        chunk.program_hash = Some(result.pack.query.program_hash.clone());
        store
            .upsert_chunk(&chunk)
            .map_err(|error| format!("agent rag query failed to index RAG chunk: {error}"))?;
    }
    store
        .record_rag_context_pack(&result.pack, None, None, None, "selected", 0)
        .map_err(|error| format!("agent rag query failed to record RAG audit: {error}"))
}

fn agent_trace_rag_candidates(
    trace_report: &AgentScriptTraceReport,
    records: &[AgentTraceRecord],
) -> Result<Vec<AgentRagCandidate>, String> {
    let mut candidates = Vec::with_capacity(records.len() + 1);
    candidates.push(agent_trace_rag_json_candidate(
        "trace.summary",
        "Agent trace summary",
        ChunkSourceKind::GraphSummary,
        SearchChannel::Summary,
        &serde_json::to_value(trace_report)
            .map_err(|error| format!("failed to serialize trace summary: {error}"))?,
        Vec::new(),
        PrivacyClass::Project,
    )?);
    candidates.extend(
        records
            .iter()
            .map(|record| {
                agent_trace_rag_json_candidate(
                    &format!("trace.record.{}", record.sequence),
                    &format!(
                        "Trace record {} {}",
                        record.sequence,
                        agent_trace_kind_name(record.kind)
                    ),
                    ChunkSourceKind::AgentTrace,
                    SearchChannel::Trace,
                    &serde_json::to_value(record)
                        .map_err(|error| format!("failed to serialize trace record: {error}"))?,
                    agent_trace_record_entity_ids(record),
                    agent_trace_record_privacy(record),
                )
            })
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(candidates)
}

fn agent_trace_rag_json_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    value: &serde_json::Value,
    entity_ids: Vec<AgentPublicId>,
    privacy: PrivacyClass,
) -> Result<AgentRagCandidate, String> {
    let body = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize RAG candidate body: {error}"))?;
    Ok(agent_rag_candidate(
        source_key,
        title,
        source_kind,
        preferred_channel,
        body,
        AgentRagCandidateMeta {
            entity_ids,
            privacy,
            source_anchor: None,
            semantic_hash: None,
            metadata: BTreeMap::new(),
        },
    ))
}

struct AgentSourceRagIndex {
    seed: String,
    candidates: Vec<AgentRagCandidate>,
}

fn agent_rag_source_paths(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut files = BTreeSet::new();
    for input in inputs {
        if input.is_dir() {
            for path in agent_rag_arcw_files_in_dir(input)? {
                files.insert(path);
            }
        } else {
            files.insert(input.clone());
        }
    }
    Ok(files.into_iter().collect())
}

fn agent_rag_arcw_files_in_dir(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut pending = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = pending.pop() {
        let entries = fs::read_dir(&dir).map_err(|error| {
            format!("agent rag query failed to read {}: {error}", dir.display())
        })?;
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "agent rag query failed to read entry under {}: {error}",
                    dir.display()
                )
            })?;
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if is_arcw_path(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn is_arcw_path(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
}

fn agent_source_rag_index(path: &Path) -> Result<AgentSourceRagIndex, String> {
    let source = fs::read_to_string(path)
        .map_err(|error| format!("agent rag query failed to read source: {error}"))?;
    let parsed = arcweft_compiler::parse_source_text(source.clone());
    if !parsed.errors().is_empty() {
        return Err(format!(
            "agent rag query source parse errors: {:?}",
            parsed.errors()
        ));
    }
    let hir = arcweft_compiler::lower_source_tree(parsed.typed_tree())
        .map_err(|errors| format!("agent rag query source HIR errors: {errors:?}"))?;
    let source_hash = agent_content_hash(&source);
    let source_name = SourceName::path(path.display().to_string());
    let project =
        project_semantic_index_from_hir(&hir, ProgramHash::new(source_hash.clone()), &source_name)
            .map_err(|error| format!("agent rag query failed to build project index: {error}"))?;
    let source_key_prefix = agent_source_rag_key_prefix(path);
    let mut candidates = agent_source_text_rag_candidates(path, &source, &source_key_prefix)?;
    candidates.extend(agent_project_semantic_rag_candidates(
        path,
        &project,
        &source_key_prefix,
    )?);
    Ok(AgentSourceRagIndex {
        seed: format!("source:{}:{source_hash}", path.display()),
        candidates,
    })
}

fn agent_source_rag_key_prefix(path: &Path) -> String {
    format!("source.{}", agent_content_hash(path.display().to_string()))
}

fn agent_source_text_rag_candidates(
    path: &Path,
    source: &str,
    source_key_prefix: &str,
) -> Result<Vec<AgentRagCandidate>, String> {
    agent_source_text_ranges(source)
        .into_iter()
        .enumerate()
        .map(|(index, range)| {
            let body = source[range.clone()].trim().to_owned();
            let source_key = format!("{source_key_prefix}.text.{index}");
            let mut metadata = BTreeMap::new();
            metadata.insert(
                "path".to_owned(),
                serde_json::Value::String(path.display().to_string()),
            );
            Ok(agent_rag_candidate(
                &source_key,
                &format!("Source text {}", path.display()),
                ChunkSourceKind::Source,
                SearchChannel::Lexical,
                body,
                AgentRagCandidateMeta {
                    entity_ids: Vec::new(),
                    privacy: PrivacyClass::Project,
                    source_anchor: Some(debug_source_anchor(path, range)?),
                    semantic_hash: None,
                    metadata,
                },
            ))
        })
        .collect()
}

fn agent_source_text_ranges(source: &str) -> Vec<std::ops::Range<usize>> {
    let mut ranges = Vec::new();
    let mut start = None;
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let line_start = offset;
        let line_end = offset.saturating_add(line.len());
        if line.trim().is_empty() {
            if let Some(start) = start.take()
                && start < line_start
            {
                ranges.push(agent_trim_source_range(source, start..line_start));
            }
        } else if start.is_none() {
            start = Some(line_start);
        }
        offset = line_end;
    }
    if let Some(start) = start
        && start < source.len()
    {
        ranges.push(agent_trim_source_range(source, start..source.len()));
    }
    if ranges.is_empty() && !source.is_empty() {
        ranges.push(agent_trim_source_range(source, 0..source.len()));
    }
    ranges
        .into_iter()
        .filter(|range| range.start < range.end)
        .collect()
}

fn agent_trim_source_range(source: &str, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
    let mut start = range.start;
    let mut end = range.end;
    while start < end {
        let Some(character) = source[start..end].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        start = start.saturating_add(character.len_utf8());
    }
    while start < end {
        let Some(character) = source[start..end].chars().next_back() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        end = end.saturating_sub(character.len_utf8());
    }
    start..end
}

fn agent_project_semantic_rag_candidates(
    path: &Path,
    project: &ProjectSemanticIndex,
    source_key_prefix: &str,
) -> Result<Vec<AgentRagCandidate>, String> {
    let mut candidates = Vec::new();
    candidates.push(agent_project_summary_rag_candidate(
        path,
        project,
        source_key_prefix,
    )?);
    for entity in project.entities().values() {
        candidates.push(agent_project_entity_rag_candidate(
            entity,
            source_key_prefix,
        )?);
    }
    for (name, query) in project.debug_queries() {
        candidates.push(agent_project_debug_query_rag_candidate(
            name,
            query,
            source_key_prefix,
        )?);
    }
    Ok(candidates)
}

fn agent_project_summary_rag_candidate(
    path: &Path,
    project: &ProjectSemanticIndex,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": project.schema_version(),
        "kind": "project_semantic_index",
        "program_hash": project.program_hash().as_str(),
        "bundle_hash": project.bundle_hash().map(arcweft_lang_sema::project_index::BundleHash::as_str),
        "counts": {
            "entities": project.entities().len(),
            "callables": project.callables().len(),
            "types": project.types().len(),
            "debug_queries": project.debug_queries().len(),
        },
    }))
    .map_err(|error| format!("failed to serialize project RAG summary: {error}"))?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "path".to_owned(),
        serde_json::Value::String(path.display().to_string()),
    );
    Ok(agent_rag_candidate(
        &format!("{source_key_prefix}.project.summary"),
        "Project semantic index summary",
        ChunkSourceKind::GraphSummary,
        SearchChannel::Summary,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: None,
            semantic_hash: Some(
                StableHash::new(project.program_hash().as_str())
                    .map_err(|error| format!("invalid project semantic hash: {error}"))?,
            ),
            metadata,
        },
    ))
}

fn agent_project_entity_rag_candidate(
    entity: &EntitySymbol,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let entity_id = agent_public_id_from_sema(entity.id())?;
    let actions = entity
        .agent_actions()
        .iter()
        .map(|action| {
            serde_json::json!({
                "action": action.action().as_str(),
                "params": action.params().iter().map(|param| {
                    serde_json::json!({
                        "name": param.name(),
                        "type": format!("{:?}", param.ty()),
                        "has_default": param.has_default(),
                    })
                }).collect::<Vec<_>>(),
                "return_type": format!("{:?}", action.return_type()),
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "kind": "project_entity",
        "id": entity.id().as_str(),
        "entity_kind": format!("{:?}", entity.ty().kind()),
        "value_type": entity.ty().value().map(|ty| format!("{ty:?}")),
        "source": source_anchor_json(entity.source()),
        "semantic_hash": entity.semantic_hash().as_str(),
        "agent_actions": actions,
    }))
    .map_err(|error| format!("failed to serialize project entity RAG chunk: {error}"))?;
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "entity_kind".to_owned(),
        serde_json::Value::String(format!("{:?}", entity.ty().kind())),
    );
    Ok(agent_rag_candidate(
        &format!(
            "{source_key_prefix}.project.entity.{}",
            entity.id().as_str()
        ),
        &format!(
            "Project entity {} {:?}",
            entity.id().as_str(),
            entity.ty().kind()
        ),
        ChunkSourceKind::Symbol,
        SearchChannel::Graph,
        body,
        AgentRagCandidateMeta {
            entity_ids: vec![entity_id],
            privacy: PrivacyClass::Project,
            source_anchor: debug_anchor_from_source_anchor(entity.source())?,
            semantic_hash: Some(
                StableHash::new(entity.semantic_hash().as_str())
                    .map_err(|error| format!("invalid entity semantic hash: {error}"))?,
            ),
            metadata,
        },
    ))
}

fn agent_project_debug_query_rag_candidate(
    name: &arcweft_lang_sema::project_index::QualifiedName,
    query: &arcweft_lang_sema::project_index::DebugQuerySymbol,
    source_key_prefix: &str,
) -> Result<AgentRagCandidate, String> {
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "kind": "project_debug_query",
        "name": name.as_str(),
        "signature": format!("{:?}", query.signature()),
    }))
    .map_err(|error| format!("failed to serialize project debug query RAG chunk: {error}"))?;
    Ok(agent_rag_candidate(
        &format!("{source_key_prefix}.project.debug_query.{}", name.as_str()),
        &format!("Project debug query {}", name.as_str()),
        ChunkSourceKind::Symbol,
        SearchChannel::Graph,
        body,
        AgentRagCandidateMeta {
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            source_anchor: None,
            semantic_hash: Some(
                StableHash::new(agent_content_hash(name.as_str()))
                    .map_err(|error| format!("invalid debug query semantic hash: {error}"))?,
            ),
            metadata: BTreeMap::new(),
        },
    ))
}

struct AgentRagCandidateMeta {
    entity_ids: Vec<AgentPublicId>,
    privacy: PrivacyClass,
    source_anchor: Option<DebugSourceAnchor>,
    semantic_hash: Option<StableHash>,
    metadata: BTreeMap<String, serde_json::Value>,
}

fn agent_rag_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    body: String,
    meta: AgentRagCandidateMeta,
) -> AgentRagCandidate {
    let content_hash = agent_content_hash(&body);
    AgentRagCandidate {
        chunk: DebugChunk {
            id: ChunkId::new(format!("cli:{source_key}:{content_hash}")),
            program_hash: None,
            source_kind,
            source_key: source_key.to_owned(),
            title: title.to_owned(),
            body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: meta.semantic_hash,
            source_anchor: meta.source_anchor,
            entity_ids: meta.entity_ids,
            privacy: meta.privacy,
            metadata: meta.metadata,
            created_unix_ms: 0,
        },
        preferred_channel,
    }
}

fn agent_public_id_from_sema(id: &SemaPublicId) -> Result<AgentPublicId, String> {
    AgentPublicId::new(id.as_str().to_owned())
        .map_err(|error| format!("failed to convert project id `{}`: {error}", id.as_str()))
}

fn debug_anchor_from_source_anchor(
    anchor: &SourceAnchor,
) -> Result<Option<DebugSourceAnchor>, String> {
    match anchor.source() {
        SourceName::Path(path) => Ok(Some(DebugSourceAnchor {
            path: path.clone(),
            start_byte: u64::try_from(anchor.byte_range().start)
                .map_err(|_| "source anchor start byte overflowed u64".to_owned())?,
            end_byte: u64::try_from(anchor.byte_range().end)
                .map_err(|_| "source anchor end byte overflowed u64".to_owned())?,
        })),
        SourceName::Generated => Ok(None),
    }
}

fn debug_source_anchor(
    path: &Path,
    range: std::ops::Range<usize>,
) -> Result<DebugSourceAnchor, String> {
    Ok(DebugSourceAnchor {
        path: path.display().to_string(),
        start_byte: u64::try_from(range.start)
            .map_err(|_| "source chunk start byte overflowed u64".to_owned())?,
        end_byte: u64::try_from(range.end)
            .map_err(|_| "source chunk end byte overflowed u64".to_owned())?,
    })
}

fn source_anchor_json(anchor: &SourceAnchor) -> serde_json::Value {
    match anchor.source() {
        SourceName::Path(path) => serde_json::json!({
            "path": path,
            "start_byte": anchor.byte_range().start,
            "end_byte": anchor.byte_range().end,
        }),
        SourceName::Generated => serde_json::json!({
            "generated": true,
        }),
    }
}

fn agent_trace_record_privacy(record: &AgentTraceRecord) -> PrivacyClass {
    record
        .payload
        .get("privacy_class")
        .or_else(|| record.payload.get("privacy"))
        .or_else(|| {
            record
                .payload
                .get("payload")
                .and_then(|payload| payload.get("privacy_class"))
        })
        .or_else(|| {
            record
                .payload
                .get("payload")
                .and_then(|payload| payload.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

fn agent_trace_record_entity_ids(record: &AgentTraceRecord) -> Vec<AgentPublicId> {
    [
        Some(record.run_id.as_str()),
        record.session_id.as_ref().map(SessionId::as_str),
        Some(agent_trace_kind_name(record.kind)),
    ]
    .into_iter()
    .flatten()
    .filter_map(|value| AgentPublicId::new(value.to_owned()).ok())
    .collect()
}

fn agent_trace_rag_ranked_lists(
    candidates: &[AgentRagCandidate],
    query: &RagQuery,
) -> Vec<Vec<SearchHit>> {
    [
        SearchChannel::ExactEntity,
        SearchChannel::Lexical,
        SearchChannel::Graph,
        SearchChannel::Trace,
        SearchChannel::Summary,
    ]
    .into_iter()
    .filter_map(|channel| {
        let mut scored = candidates
            .iter()
            .filter_map(|candidate| {
                agent_trace_rag_score(candidate, query, channel).map(|score| (candidate, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.chunk.id.cmp(&right.0.chunk.id))
        });
        (!scored.is_empty()).then(|| {
            scored
                .into_iter()
                .enumerate()
                .map(|(index, (candidate, score))| SearchHit {
                    chunk_id: candidate.chunk.id.clone(),
                    channel,
                    rank: index + 1,
                    score: Some(score),
                })
                .collect()
        })
    })
    .collect()
}

fn agent_trace_rag_score(
    candidate: &AgentRagCandidate,
    query: &RagQuery,
    channel: SearchChannel,
) -> Option<f64> {
    let haystack = agent_trace_rag_haystack(candidate);
    match channel {
        SearchChannel::ExactEntity => {
            let root_match = query.roots.iter().any(|root| {
                candidate
                    .chunk
                    .entity_ids
                    .iter()
                    .any(|entity| entity == root)
                    || candidate.chunk.source_key == root.as_str()
                    || candidate.chunk.title.contains(root.as_str())
            });
            let query_match = candidate
                .chunk
                .entity_ids
                .iter()
                .any(|entity| entity.as_str() == query.text)
                || candidate.chunk.source_key == query.text
                || candidate.chunk.title == query.text;
            (root_match || query_match).then_some(1.0)
        }
        SearchChannel::Lexical => {
            let query_lower = query.text.to_lowercase();
            let phrase = f64::from(u8::from(haystack.contains(&query_lower)));
            let token_score = agent_count_as_f64(
                agent_rag_tokens(&query.text)
                    .into_iter()
                    .filter(|token| haystack.contains(token))
                    .count(),
            );
            (phrase + token_score > 0.0).then_some(phrase.mul_add(4.0, token_score))
        }
        SearchChannel::Graph => {
            let root_score = if query.graph_depth > 0 {
                agent_count_as_f64(
                    query
                        .roots
                        .iter()
                        .filter(|root| haystack.contains(&root.as_str().to_lowercase()))
                        .count(),
                )
            } else {
                0.0
            };
            let channel_score = f64::from(u8::from(
                candidate.preferred_channel == SearchChannel::Graph,
            ));
            (root_score + channel_score > 0.0).then_some(root_score + channel_score)
        }
        SearchChannel::Trace | SearchChannel::Summary => {
            if candidate.preferred_channel != channel {
                return None;
            }
            let token_score = agent_count_as_f64(
                agent_rag_tokens(&query.text)
                    .into_iter()
                    .filter(|token| haystack.contains(token))
                    .count(),
            );
            (token_score > 0.0).then_some(token_score)
        }
        SearchChannel::Vector | SearchChannel::History | SearchChannel::Diagnostics => None,
    }
}

fn agent_trace_rag_haystack(candidate: &AgentRagCandidate) -> String {
    let mut haystack = format!(
        "{}\n{}\n{}",
        candidate.chunk.source_key, candidate.chunk.title, candidate.chunk.body
    )
    .to_lowercase();
    for entity in &candidate.chunk.entity_ids {
        haystack.push('\n');
        haystack.push_str(&entity.as_str().to_lowercase());
    }
    haystack
}

fn agent_rag_roots(values: &[String]) -> Result<Vec<AgentPublicId>, String> {
    values
        .iter()
        .map(|root| {
            let root = root.trim();
            AgentPublicId::new(root.to_owned())
                .map_err(|_| "agent rag query --root values must not be empty".to_owned())
        })
        .collect()
}

fn parse_agent_privacy_class(value: &str) -> Result<PrivacyClass, String> {
    PrivacyClass::parse(value).ok_or_else(|| {
        format!("privacy class must be one of public, project, sensitive, or secret: `{value}`")
    })
}

fn agent_rag_tokens(text: &str) -> BTreeSet<String> {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '.' || character == '_' || character == '-')
    })
    .map(str::trim)
    .filter(|token| !token.is_empty())
    .map(str::to_lowercase)
    .collect()
}

fn agent_trace_rag_seed(path: &Path, records: &[AgentTraceRecord]) -> String {
    let seed = records
        .iter()
        .map(|record| record.payload_hash.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    format!("trace:{}:{seed}", path.display())
}

fn agent_rag_program_hash(seed_parts: &[String]) -> Result<StableHash, String> {
    StableHash::new(agent_content_hash(seed_parts.join("\n")))
        .map_err(|_| "failed to build Agent RAG program hash".to_owned())
}

fn agent_content_hash(bytes: impl AsRef<[u8]>) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_ref()).to_hex())
}

fn agent_count_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

fn truncate_utf8(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
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
    blob_dir: Option<String>,
    debug_db: Option<String>,
    blobs_written: usize,
    blob_bytes: u64,
    responses: Vec<AgentHostResponse>,
    error: Option<String>,
}

pub(in crate::app) struct AgentScriptRunInput {
    path: String,
    agents: usize,
    program_hash: String,
    project_entities: Vec<RequiredEntity>,
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
    blobs_validated: usize,
    blob_bytes: u64,
    kinds: BTreeMap<String, usize>,
    error: Option<String>,
}

#[derive(Clone, Debug, Default)]
struct AgentBlobWriteReport {
    dir: Option<String>,
    count: usize,
    bytes: u64,
}

#[derive(Clone, Debug)]
struct AgentCaptureBlob {
    content_hash: String,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default)]
struct AgentTraceBlobValidation {
    count: usize,
    bytes: u64,
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
            blob_dir: options
                .blob_dir
                .as_ref()
                .map(|path| path.display().to_string()),
            debug_db: options
                .debug_db
                .as_ref()
                .map(|path| path.display().to_string()),
            blobs_written: 0,
            blob_bytes: 0,
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
        return agent_script_run_bundle_input(options);
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
    let program_hash = project.program_hash().as_str().to_owned();
    let project_entities = agent_project_entities(&project)?;
    let compiled = arcweft_compiler::compile_agent_bundle_with_project(source, &project)
        .map_err(|error| error.to_string())?;
    Ok(AgentScriptRunInput {
        path: options.path.display().to_string(),
        agents: compiled.hir.agents().len(),
        program_hash,
        project_entities,
        bundle: compiled.bundle,
    })
}

fn agent_script_run_bundle_input(
    options: &AgentScriptRunOptions,
) -> Result<AgentScriptRunInput, String> {
    let path = &options.path;
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
    let project = agent_script_compile_project_index(options)?;
    let program_hash = project.program_hash().as_str().to_owned();
    let project_entities = agent_project_entities(&project)?;
    let agents = usize::from(bundle.agent.is_some());
    Ok(AgentScriptRunInput {
        path: path.display().to_string(),
        agents,
        program_hash,
        project_entities,
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
    let session = CliAgentSession::new(
        options.signals.clone(),
        options.states.clone(),
        input.program_hash.clone(),
        input.project_entities.clone(),
    );
    let mut runner = AgentRunner::new(
        session,
        CollectingDebugSink::default(),
        NoopRagService,
        agent_script_runtime_policy(input),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        &input.bundle,
        AgentControllerRunConfig {
            max_steps: options.max_steps,
            max_ops_per_step: options.max_ops,
        },
    );
    let blob_result = write_agent_capture_blobs(
        options.blob_dir.as_deref(),
        runner.session_mut().capture_blobs(),
    );
    let debug_events = runner.debug_mut().events.clone();
    let run_id = AgentRunId::new(options.run_id.clone()).map_err(|error| {
        eprintln!("error: invalid run id: {error}");
        ExitCode::from(2)
    })?;
    agent_script_run_report_from_result(
        options,
        input,
        run_result,
        &run_id,
        &debug_events,
        blob_result,
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

pub(in crate::app) fn agent_script_runtime_policy(
    input: &AgentScriptRunInput,
) -> RuntimeAgentPolicy {
    agent_script_runtime_policy_for_bundle(&input.bundle)
}

pub(in crate::app) fn agent_script_runtime_policy_for_bundle(
    bundle: &ArcweftBundle,
) -> RuntimeAgentPolicy {
    let mut capabilities = vec![
        RuntimeAgentCapability::Observe,
        RuntimeAgentCapability::Act,
        RuntimeAgentCapability::Capture,
        RuntimeAgentCapability::ResourceRead,
        RuntimeAgentCapability::DebugRecord,
        RuntimeAgentCapability::Rag,
    ];
    if agent_bundle_declares_effect(bundle, "agent.act.physical") {
        capabilities.push(RuntimeAgentCapability::ActPhysical);
    }
    RuntimeAgentPolicy::new(capabilities)
}

fn agent_bundle_declares_effect(bundle: &ArcweftBundle, effect: &str) -> bool {
    bundle.agent.as_ref().is_some_and(|manifest| {
        manifest
            .declared_effects
            .iter()
            .any(|declared| declared.as_str() == effect)
    })
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
    blob_result: Result<AgentBlobWriteReport, String>,
) -> Result<AgentScriptRunReport, String> {
    let trace_records = agent_trace_records(run_id, &agent_cli_session_id(), debug_events);
    let trace_result = options
        .trace_out
        .as_ref()
        .map(|path| write_agent_trace(path, &trace_records).map(|()| path.display().to_string()))
        .transpose();
    let mut report = match (run_result, trace_result, blob_result) {
        (Ok(run), Ok(trace_path), Ok(blob_report)) => agent_script_run_success_report(
            &input.path,
            input.agents,
            run,
            trace_path,
            trace_records.len(),
            blob_report,
        ),
        (Err(error), Ok(trace_path), Ok(blob_report)) => agent_script_run_error_report(
            &input.path,
            input.agents,
            trace_path,
            trace_records.len(),
            blob_report,
            error.to_string(),
        ),
        (_, Err(error), blob_result) => agent_script_run_error_report(
            &input.path,
            input.agents,
            options
                .trace_out
                .as_ref()
                .map(|path| path.display().to_string()),
            trace_records.len(),
            blob_result.unwrap_or_default(),
            error,
        ),
        (_, _, Err(error)) => agent_script_run_error_report(
            &input.path,
            input.agents,
            options
                .trace_out
                .as_ref()
                .map(|path| path.display().to_string()),
            trace_records.len(),
            AgentBlobWriteReport::default(),
            error,
        ),
    };
    report.debug_db = options
        .debug_db
        .as_ref()
        .map(|path| path.display().to_string());
    agent_script_persist_debug_run(options, input, run_id, debug_events, &report)?;
    Ok(report)
}

fn agent_script_persist_debug_run(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_id: &AgentRunId,
    debug_events: &[DebugEvent],
    report: &AgentScriptRunReport,
) -> Result<(), String> {
    let Some(mut store) = agent_script_debug_store(options)? else {
        return Ok(());
    };
    let (session_id, base_sequence) = agent_script_start_debug_run(&store, options, input, run_id)?;
    agent_script_append_debug_events(&mut store, run_id, base_sequence, debug_events)?;
    agent_script_finish_debug_run(
        &store,
        &session_id,
        run_id,
        base_sequence,
        debug_events,
        report,
    )?;
    store
        .flush()
        .map_err(|error| format!("failed to flush Agent Script debug database: {error}"))
}

fn agent_script_debug_store(options: &AgentScriptRunOptions) -> Result<Option<DebugStore>, String> {
    let Some(path) = &options.debug_db else {
        return Ok(None);
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create debug database directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let store = DebugStore::open(path).map_err(|error| {
        format!(
            "failed to open Agent Script debug database {}: {error}",
            path.display()
        )
    })?;
    Ok(Some(store))
}

fn agent_script_start_debug_run(
    store: &DebugStore,
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    run_id: &AgentRunId,
) -> Result<(SessionId, u64), String> {
    let program_hash = StableHash::new(input.program_hash.clone())
        .map_err(|error| format!("invalid Agent Script program hash: {error}"))?;
    store
        .upsert_program(&program_hash, None, None, 0)
        .map_err(|error| format!("failed to persist Agent Script debug program: {error}"))?;
    let session_id = agent_cli_session_id();
    let mut session_metadata = BTreeMap::new();
    session_metadata.insert("path".to_owned(), serde_json::json!(input.path));
    session_metadata.insert(
        "native".to_owned(),
        serde_json::json!(agent_script_run_uses_native_session_for_metadata(options)),
    );
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: Some(program_hash),
            profile: "script".to_owned(),
            transport: if agent_script_run_uses_native_session_for_metadata(options) {
                "native".to_owned()
            } else {
                "cli".to_owned()
            },
            started_unix_ms: 0,
            ended_unix_ms: None,
            status: DebugSessionStatus::Running,
            metadata: session_metadata,
        })
        .map_err(|error| format!("failed to persist Agent Script debug session: {error}"))?;
    let base_sequence = store
        .next_session_sequence(&session_id)
        .map_err(|error| format!("failed to allocate Agent Script debug sequence: {error}"))?;
    let manifest = input.bundle.agent.as_ref();
    let project_binding_mode = manifest
        .map_or("unknown", |manifest| match manifest.project_binding.mode {
            arcweft_agent_protocol::artifact::ProjectBindingMode::Strict => "strict",
            arcweft_agent_protocol::artifact::ProjectBindingMode::Compatible => "compatible",
        })
        .to_owned();
    store
        .upsert_script_run(&DebugScriptRun {
            run_id: run_id.clone(),
            session_id: session_id.clone(),
            agent_id: manifest.map(|manifest| manifest.agent_id.clone()),
            artifact_hash: None,
            source_hash: manifest.map(|manifest| manifest.source_hash.clone()),
            project_binding_mode,
            started_sequence: base_sequence,
            finished_sequence: None,
            outcome: DebugScriptRunOutcome::Running,
            partially_effectful: false,
            trace_uri: None,
            error: None,
            metadata: BTreeMap::new(),
        })
        .map_err(|error| format!("failed to persist Agent Script run start: {error}"))?;
    Ok((session_id, base_sequence))
}

fn agent_script_append_debug_events(
    store: &mut DebugStore,
    run_id: &AgentRunId,
    base_sequence: u64,
    debug_events: &[DebugEvent],
) -> Result<(), String> {
    for event in debug_events {
        let mut event = event.clone();
        event.run_id = Some(run_id.clone());
        event.sequence = event.sequence.saturating_add(base_sequence);
        store
            .append(&event)
            .map_err(|error| format!("failed to persist Agent Script debug event: {error}"))?;
    }
    Ok(())
}

fn agent_script_finish_debug_run(
    store: &DebugStore,
    session_id: &SessionId,
    run_id: &AgentRunId,
    base_sequence: u64,
    debug_events: &[DebugEvent],
    report: &AgentScriptRunReport,
) -> Result<(), String> {
    let finished_sequence = debug_events
        .last()
        .map_or(base_sequence.saturating_add(1), |event| {
            base_sequence
                .saturating_add(event.sequence)
                .saturating_add(1)
        });
    let outcome = if report.ok {
        DebugScriptRunOutcome::Done
    } else {
        DebugScriptRunOutcome::Failed
    };
    let mut run_metadata = BTreeMap::new();
    run_metadata.insert("steps".to_owned(), serde_json::json!(report.steps));
    run_metadata.insert(
        "host_calls".to_owned(),
        serde_json::json!(report.host_calls),
    );
    run_metadata.insert(
        "events_emitted".to_owned(),
        serde_json::json!(report.events_emitted),
    );
    run_metadata.insert(
        "trace_records".to_owned(),
        serde_json::json!(report.trace_records),
    );
    let error = report
        .error
        .as_ref()
        .map(|message| serde_json::json!({ "message": message }));
    store
        .finish_script_run(
            run_id,
            &DebugScriptRunFinish {
                outcome,
                finished_sequence,
                partially_effectful: report.host_calls > 0,
                trace_uri: report.trace_path.clone(),
                error,
                metadata: run_metadata,
            },
        )
        .map_err(|error| format!("failed to persist Agent Script run finish: {error}"))?;
    let mut session_finish_metadata = BTreeMap::new();
    session_finish_metadata.insert("runs".to_owned(), serde_json::json!(1));
    session_finish_metadata.insert("last_run_id".to_owned(), serde_json::json!(run_id.as_str()));
    session_finish_metadata.insert("ok".to_owned(), serde_json::json!(report.ok));
    store
        .finish_session(
            session_id,
            if report.ok {
                DebugSessionStatus::Finished
            } else {
                DebugSessionStatus::Failed
            },
            0,
            &session_finish_metadata,
        )
        .map_err(|error| format!("failed to finish Agent Script debug session: {error}"))
}

fn agent_script_run_uses_native_session_for_metadata(options: &AgentScriptRunOptions) -> bool {
    #[cfg(feature = "native-capture")]
    {
        agent_script_run_uses_native_session(options)
    }
    #[cfg(not(feature = "native-capture"))]
    {
        let _ = options;
        false
    }
}

fn agent_script_run_success_report(
    path: &str,
    agents: usize,
    run: AgentControllerRunReport,
    trace_path: Option<String>,
    trace_records: usize,
    blob_report: AgentBlobWriteReport,
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
        blob_dir: blob_report.dir,
        debug_db: None,
        blobs_written: blob_report.count,
        blob_bytes: blob_report.bytes,
        responses: run.responses,
        error: None,
    }
}

fn agent_script_run_error_report(
    path: &str,
    agents: usize,
    trace_path: Option<String>,
    trace_records: usize,
    blob_report: AgentBlobWriteReport,
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
        blob_dir: blob_report.dir,
        debug_db: None,
        blobs_written: blob_report.count,
        blob_bytes: blob_report.bytes,
        responses: Vec::new(),
        error: Some(error),
    }
}

fn agent_cli_session_id() -> SessionId {
    SessionId::new("session.cli").expect("static session id")
}

fn write_agent_capture_blobs(
    blob_dir: Option<&Path>,
    blobs: &[AgentCaptureBlob],
) -> Result<AgentBlobWriteReport, String> {
    let Some(blob_dir) = blob_dir else {
        return Ok(AgentBlobWriteReport::default());
    };
    let mut report = AgentBlobWriteReport {
        dir: Some(blob_dir.display().to_string()),
        count: 0,
        bytes: 0,
    };
    for blob in blobs {
        let path = agent_blob_path(blob_dir, &blob.content_hash)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("failed to create {}: {error}", parent.display()))?;
        }
        fs::write(&path, &blob.bytes)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
        report.count += 1;
        report.bytes = report
            .bytes
            .checked_add(u64::try_from(blob.bytes.len()).map_err(|_| {
                format!(
                    "capture blob {} is too large to count as u64 bytes",
                    blob.content_hash
                )
            })?)
            .ok_or_else(|| "capture blob byte count overflowed u64".to_owned())?;
    }
    Ok(report)
}

fn agent_blob_path(root: &Path, content_hash: &str) -> Result<PathBuf, String> {
    let Some(hex) = content_hash.strip_prefix("blake3:") else {
        return Err(format!(
            "capture blob hash `{content_hash}` is not a blake3 content hash"
        ));
    };
    if hex.is_empty()
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!(
            "capture blob hash `{content_hash}` has an invalid blake3 digest"
        ));
    }
    Ok(root.join("blake3").join(hex))
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
    validate_agent_trace(path, &records, None)?;
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
        .and_then(|records| {
            validate_agent_trace(&options.path, &records, options.blob_dir.as_deref())
        })
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
            blobs_validated: 0,
            blob_bytes: 0,
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
    blob_dir: Option<&Path>,
) -> Result<AgentScriptTraceReport, String> {
    let run_id = records
        .first()
        .map(|record| record.run_id.clone())
        .ok_or_else(|| "trace must contain at least one record".to_owned())?;
    let first_sequence = records.first().map(|record| record.sequence);
    let last_sequence = records.last().map(|record| record.sequence);
    let blob_validation = validate_agent_trace_records(records, &run_id, blob_dir)?;
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
        blobs_validated: blob_validation.count,
        blob_bytes: blob_validation.bytes,
        kinds: agent_trace_kind_counts(records),
        error: None,
    })
}

fn validate_agent_trace_records(
    records: &[AgentTraceRecord],
    run_id: &AgentRunId,
    blob_dir: Option<&Path>,
) -> Result<AgentTraceBlobValidation, String> {
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
    let mut previous = None;
    let mut blob_validation = AgentTraceBlobValidation::default();
    for record in records {
        if let Some(bytes) = validate_agent_trace_record(record, run_id, previous, blob_dir)? {
            blob_validation.count += 1;
            blob_validation.bytes = blob_validation
                .bytes
                .checked_add(bytes)
                .ok_or_else(|| "validated blob byte count overflowed u64".to_owned())?;
        }
        previous = Some(record.sequence);
    }
    Ok(blob_validation)
}

fn validate_agent_trace_record(
    record: &AgentTraceRecord,
    run_id: &AgentRunId,
    previous_sequence: Option<u64>,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
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
        return validate_agent_trace_capture_blob_refs(record, blob_dir);
    }
    Ok(None)
}

fn validate_agent_trace_capture_blob_refs(
    record: &AgentTraceRecord,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
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
        return validate_agent_trace_capture_blob_bytes(record, &content_hash, blob_dir);
    }
    Err(format!(
        "trace record {} capture blob_refs does not include content_hash {}",
        record.sequence,
        content_hash.as_str()
    ))
}

fn validate_agent_trace_capture_blob_bytes(
    record: &AgentTraceRecord,
    content_hash: &StableHash,
    blob_dir: Option<&Path>,
) -> Result<Option<u64>, String> {
    let Some(blob_dir) = blob_dir else {
        return Ok(None);
    };
    let expected_len = record
        .payload
        .get("byte_len")
        .and_then(serde_json::Value::as_u64);
    let path = agent_blob_path(blob_dir, content_hash.as_str())?;
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let actual_len = u64::try_from(bytes.len()).map_err(|_| {
        format!(
            "trace record {} capture blob {} is too large to count as u64 bytes",
            record.sequence,
            path.display()
        )
    })?;
    if expected_len.is_some_and(|expected_len| expected_len != actual_len) {
        return Err(format!(
            "trace record {} capture blob byte_len mismatch for {}: expected {}, got {}",
            record.sequence,
            content_hash.as_str(),
            expected_len.unwrap_or_default(),
            actual_len
        ));
    }
    let actual_hash = StableHash::new(format!("blake3:{}", blake3::hash(&bytes).to_hex()))
        .expect("generated blob hash is nonempty");
    if &actual_hash != content_hash {
        return Err(format!(
            "trace record {} capture blob hash mismatch for {}: got {}",
            record.sequence,
            content_hash.as_str(),
            actual_hash.as_str()
        ));
    }
    Ok(Some(actual_len))
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
        AgentTraceKind::ResourceReadCompleted => "resource_read_completed",
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

pub(in crate::app::agent) fn parse_agent_script_signal_arg(
    value: &str,
) -> Result<AgentScriptSignalArg, String> {
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

pub(in crate::app::agent) fn parse_agent_script_state_arg(
    value: &str,
) -> Result<AgentScriptStateArg, String> {
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
        DebugEventKind::ResourceRead => AgentTraceKind::ResourceReadCompleted,
        DebugEventKind::Assertion => AgentTraceKind::AssertionEvaluated,
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

fn agent_project_entities(project: &ProjectSemanticIndex) -> Result<Vec<RequiredEntity>, String> {
    arcweft_compiler::agent_required_entities_from_project(project)
        .map_err(|error| error.to_string())
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
    program_hash: String,
    project_entities: Vec<RequiredEntity>,
    tick: u64,
    signals: BTreeMap<String, AgentValue>,
    states: BTreeMap<String, AgentValue>,
    captures: u64,
    capture_blobs: Vec<AgentCaptureBlob>,
}

impl CliAgentSession {
    fn new(
        signals: Vec<AgentScriptSignalArg>,
        states: Vec<AgentScriptStateArg>,
        program_hash: String,
        project_entities: Vec<RequiredEntity>,
    ) -> Self {
        Self {
            program_hash,
            project_entities,
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
            capture_blobs: Vec::new(),
        }
    }

    fn observation(&self) -> ObservationEnvelope {
        ObservationEnvelope {
            tick: self.tick,
            frame_id: format!("cli.frame.{}", self.tick),
            state_hash: format!("cli.state.{}", self.tick),
            render_hash: format!("cli.render.{}", self.tick),
            actions: Vec::new(),
            signals: self.signals.clone(),
            payload: serde_json::json!({
                "source": "arcw agent script run",
                "deterministic_cli_session": true,
                "state": agent_values_to_json(&self.states),
            }),
        }
    }

    fn capture_blobs(&self) -> &[AgentCaptureBlob] {
        &self.capture_blobs
    }
}

impl AgentSession for CliAgentSession {
    type Error = Infallible;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.cli".to_owned(),
            program_hash: self.program_hash.clone(),
            project_entities: self.project_entities.clone(),
            profile: Some("cli".to_owned()),
            capabilities: vec![
                "agent.observe".to_owned(),
                "agent.wait".to_owned(),
                "agent.capture".to_owned(),
                "agent.act.semantic".to_owned(),
                "agent.resource.read".to_owned(),
                "debug.record".to_owned(),
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
        let (media_type, bytes) = cli_capture_blob_bytes(&request);
        let content_hash = format!("blake3:{}", blake3::hash(&bytes).to_hex());
        let byte_len = u64::try_from(bytes.len()).expect("capture blob length fits u64");
        self.capture_blobs.push(AgentCaptureBlob {
            content_hash: content_hash.clone(),
            bytes,
        });
        let uri = format!("agent://capture/cli/{}-{}", request.name, self.captures);
        Ok(CaptureResult {
            uri: AgentResourceUri::new(uri).expect("generated capture uri is nonempty"),
            content_hash,
            media_type,
            byte_len,
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

fn cli_capture_blob_bytes(request: &CaptureRequest) -> (String, Vec<u8>) {
    match request.format {
        CaptureFormat::Png => ("image/png".to_owned(), CLI_TRANSPARENT_PNG.to_vec()),
        CaptureFormat::RawRgba => ("application/octet-stream".to_owned(), vec![0, 0, 0, 0]),
        CaptureFormat::Svg => (
            "image/svg+xml".to_owned(),
            b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"1\" height=\"1\"><title>arcweft-cli-capture</title></svg>".to_vec(),
        ),
    }
}

const CLI_TRANSPARENT_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0a, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x00, 0x01, 0x00, 0x00,
    0x05, 0x00, 0x01, 0x0d, 0x0a, 0x2d, 0xb4, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae,
    0x42, 0x60, 0x82,
];

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

#[cfg(test)]
mod tests {
    use super::*;

    fn test_hash(value: &str) -> StableHash {
        StableHash::new(format!("blake3:{value}")).expect("test hash is nonempty")
    }

    fn test_rag_options() -> AgentRagQueryOptions {
        AgentRagQueryOptions {
            trace: None,
            source: vec![PathBuf::from("test.arcw")],
            query: "alpha".to_owned(),
            roots: Vec::new(),
            graph_depth: 1,
            limit: 2,
            max_context_bytes: 4096,
            max_privacy: PrivacyClass::Project,
            debug_db: None,
            json: true,
        }
    }

    fn test_rag_query() -> RagQuery {
        RagQuery {
            query_id: "query.test".to_owned(),
            text: "alpha".to_owned(),
            program_hash: test_hash("program"),
            roots: Vec::new(),
            graph_depth: 1,
            limit: 2,
            max_context_bytes: 4096,
        }
    }

    fn test_chunk(
        id: &str,
        semantic_hash: Option<StableHash>,
        source_anchor: Option<DebugSourceAnchor>,
    ) -> DebugChunk {
        DebugChunk {
            id: ChunkId::new(id),
            program_hash: None,
            source_kind: ChunkSourceKind::Source,
            source_key: id.to_owned(),
            title: id.to_owned(),
            body: format!("alpha context {id}"),
            content_hash: test_hash(id),
            semantic_hash,
            source_anchor,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        }
    }

    fn test_candidate(chunk: DebugChunk) -> AgentRagCandidate {
        AgentRagCandidate {
            chunk,
            preferred_channel: SearchChannel::Lexical,
        }
    }

    fn item_ids(pack: &RagContextPack) -> Vec<&str> {
        pack.items
            .iter()
            .map(|item| item.chunk_id.as_str())
            .collect()
    }

    #[test]
    fn agent_rag_context_pack_deduplicates_semantic_hashes() {
        let candidates = vec![
            test_candidate(test_chunk(
                "chunk:a",
                Some(test_hash("same-semantic")),
                None,
            )),
            test_candidate(test_chunk(
                "chunk:b",
                Some(test_hash("same-semantic")),
                None,
            )),
            test_candidate(test_chunk(
                "chunk:c",
                Some(test_hash("unique-semantic")),
                None,
            )),
        ];

        let pack = agent_trace_rag_pack_from_candidates(
            &test_rag_options(),
            test_rag_query(),
            &candidates,
        );

        assert_eq!(item_ids(&pack), vec!["chunk:a", "chunk:c"]);
    }

    #[test]
    fn agent_rag_context_pack_deduplicates_overlapping_source_spans() {
        let candidates = vec![
            test_candidate(test_chunk(
                "chunk:a",
                None,
                Some(DebugSourceAnchor {
                    path: "game.arcw".to_owned(),
                    start_byte: 0,
                    end_byte: 20,
                }),
            )),
            test_candidate(test_chunk(
                "chunk:b",
                None,
                Some(DebugSourceAnchor {
                    path: "game.arcw".to_owned(),
                    start_byte: 10,
                    end_byte: 30,
                }),
            )),
            test_candidate(test_chunk(
                "chunk:c",
                None,
                Some(DebugSourceAnchor {
                    path: "game.arcw".to_owned(),
                    start_byte: 30,
                    end_byte: 40,
                }),
            )),
        ];

        let pack = agent_trace_rag_pack_from_candidates(
            &test_rag_options(),
            test_rag_query(),
            &candidates,
        );

        assert_eq!(item_ids(&pack), vec!["chunk:a", "chunk:c"]);
    }
}
