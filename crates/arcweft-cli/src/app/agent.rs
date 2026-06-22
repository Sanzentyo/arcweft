use super::commands::{AgentCommand, AgentRagCommand, AgentScriptCommand};
use super::local_embedding::{
    DEFAULT_LOCAL_EMBEDDING_DIMENSIONS, DEFAULT_LOCAL_EMBEDDING_MODEL_ID,
    DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION, MAX_LOCAL_EMBEDDING_DIMENSIONS,
    local_hash_query_embedding,
};
use super::project::ProfileOptions;
use super::runtime::options::{
    CliRuntimeExecutorTier, CliRuntimeMathBackend, CliRuntimePureBackend, CliRuntimePureWorkers,
    CliRuntimeStepMode,
};
use super::runtime::parse::{parse_runtime_binding_arg, parse_runtime_pure_workers};
use super::shared::print_json;
use arcweft_agent_protocol::{
    artifact::RequiredEntity,
    ids::{AgentResourceUri, AgentRunId, PublicId as AgentPublicId, SessionId, StableHash},
    protocol::{
        ActionResult, AgentAction, AgentHostResponse, AgentProjectGraph, AgentSessionInfo,
        CaptureFormat, CaptureRequest, CaptureResult, ObservationEnvelope, ObserveRequest,
    },
    resource::{AgentResource, AgentResourceBody, AgentResourceKind},
    trace::{AgentTraceKind, AgentTraceRecord},
    value::AgentValue,
};
use arcweft_agent_runner::{
    config::{AgentControllerRunConfig, AgentControllerRunReport, AgentRunnerConfig},
    policy::{RuntimeAgentCapability, RuntimeAgentPolicy},
    runner::AgentRunner,
    session::{AgentSession, NoopRagService},
};
use arcweft_bundle::{ArcweftBundle, BundleKind};
use arcweft_compiler::{agent, agent_project, hir, parse};
use arcweft_core::value::RuntimeBinding;
use arcweft_debug_model::{
    chunk::{
        ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass, SourceAnchor as DebugSourceAnchor,
    },
    diagnostic::DebugDiagnostic,
    embedding::EmbeddingModelDescriptor,
    event::{DebugEvent, DebugEventKind},
    graph::{DebugGraphEdge, DebugGraphSymbol},
    rag::{RagContextItem, RagContextPack, RagQuery, SearchChannel, SearchHit},
    script::{DebugScriptRun, DebugScriptRunFinish, DebugScriptRunOutcome},
    session::{DebugSession, DebugSessionStatus},
    sink::DebugEventSink,
    source::DebugSourceFile,
};
use arcweft_debug_sqlite::store::DebugStore;
use arcweft_id::PublicId as SemaPublicId;
use arcweft_lang_sema::{
    project_index::{
        AgentActionSignature, EntitySymbol, ProgramHash, ProjectCallableSymbol,
        ProjectSemanticIndex, QualifiedName, SemanticHash, project_semantic_index_from_hir,
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
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, path::Path};

#[cfg(feature = "native-capture")]
use super::project::{
    load_and_check_selection, native_host_policy_for_selection, resolve_source_selection,
    runtime_plan_options_for_selection, runtime_pure_config_for_selection,
};

#[cfg(feature = "native-capture")]
use super::runtime::parse::step_options;
#[cfg(feature = "native-capture")]
use arcweft_compiler::lower::lower_source_runtime_plan_with_stats_and_options;
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
const AGENT_DEBUG_RUNTIME_STALE_AFTER_MILLIS: i64 = 24 * 60 * 60 * 1000;
const AGENT_DEBUG_RUNTIME_STALE_REASON: &str = "runtime_session_start";

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
    #[arg(long, value_enum, default_value_t = AgentContentPolicyMode::Strict)]
    content_policy_mode: AgentContentPolicyMode,
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
pub(super) struct AgentMcpOptions {
    #[arg(long, value_enum, default_value_t = AgentContentPolicyMode::Strict)]
    content_policy_mode: AgentContentPolicyMode,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(super) enum AgentContentPolicyMode {
    #[default]
    Strict,
    LocalDev,
}

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
pub(super) struct AgentRagIndexOptions {
    #[arg(long)]
    source: Vec<PathBuf>,
    #[arg(long = "debug-db")]
    debug_db: PathBuf,
    #[arg(long)]
    changed: bool,
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
    #[arg(long = "local-embedding")]
    local_embedding: bool,
    #[arg(long = "local-embedding-model-id", default_value = DEFAULT_LOCAL_EMBEDDING_MODEL_ID)]
    local_embedding_model_id: String,
    #[arg(
        long = "local-embedding-model-revision",
        default_value = DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION
    )]
    local_embedding_model_revision: String,
    #[arg(long = "local-embedding-dimensions", default_value_t = DEFAULT_LOCAL_EMBEDDING_DIMENSIONS)]
    local_embedding_dimensions: u32,
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
mod mcp_stdio;
#[cfg(feature = "native-capture")]
mod native;

#[cfg(feature = "native-capture")]
pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Rag { command } => agent_rag_command(*command),
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
        AgentCommand::Rag { command } => agent_rag_command(*command),
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

mod rag;
mod script;

use rag::agent_rag_command;
use rag::source_index::parse_agent_privacy_class;
use script::{
    AgentScriptTraceReport, agent_script_command, agent_trace_kind_name,
    parse_agent_script_signal_arg, parse_agent_script_state_arg,
    read_and_validate_agent_trace_records, validate_agent_trace,
};

#[cfg(test)]
mod tests {
    use super::rag::{AgentRagCandidate, agent_trace_rag_pack_from_candidates};
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
            local_embedding: false,
            local_embedding_model_id: DEFAULT_LOCAL_EMBEDDING_MODEL_ID.to_owned(),
            local_embedding_model_revision: DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION.to_owned(),
            local_embedding_dimensions: DEFAULT_LOCAL_EMBEDDING_DIMENSIONS,
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
