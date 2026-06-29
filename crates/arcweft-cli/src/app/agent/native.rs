use super::super::runtime::entry::apply_runtime_entry_selection;
use super::super::runtime::executor::RuntimeExecutorInstance;
use super::super::runtime::profile::report_path;
use super::super::runtime::steps::NativeRunHost;
use super::mcp_stdio::{StdioMcpEndpoint, StdioMcpTransport};
use super::rag::source_index::{
    AgentSourceRagIndex, agent_program_summary_rag_candidate, agent_rag_program_hash,
    agent_rag_source_paths, agent_source_rag_index,
};
use super::rag::{
    AgentRagCandidate, agent_rag_index_graph, agent_rag_index_program_graph,
    agent_rag_select_context_chunk,
};
use super::script::{
    AgentCaptureBlob, AgentScriptRunInput, AgentScriptRunReport, CliAgentSession,
    CollectingDebugSink, agent_cli_session_id, agent_debug_finish_runtime_session,
    agent_debug_start_runtime_session, agent_project_entities, agent_project_graph,
    agent_script_project_entities_metadata, agent_script_project_graph_metadata,
    agent_script_project_index, agent_script_run_bundle, agent_script_run_input,
    agent_script_run_report_from_result, agent_script_runtime_policy,
    parse_agent_script_signal_arg, parse_agent_script_state_arg,
    read_and_validate_agent_trace_records, write_agent_capture_blobs,
};
use super::{
    AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT, AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH, AgentCommand,
    AgentContentPolicyMode, AgentControllerRunConfig, AgentHitTestOptions, AgentMcpOptions,
    AgentObserveCaptureKind, AgentObserveImageKind, AgentObserveMcpFormat, AgentObserveOptions,
    AgentObserveResourceKind, AgentReplOptions, AgentRunner, AgentRunnerConfig,
    AgentScriptRunOptions, AgentScriptSignalArg, AgentScriptStateArg, AgentSession,
    CliRuntimeExecutorTier, CliRuntimePureWorkers, CliRuntimeStepMode, ExitCode, FlowFiberStatus,
    LineDisplayCatalog, NativeAdapterRegistrar, NativeTaskBridge, NoopRagService, Path, PathBuf,
    ProfileOptions, RuntimeStepInput, RuntimeStepResult, fs, load_and_check_selection,
    lower_source_runtime_plan_with_stats_and_options, native_host_policy_for_selection,
    parse_runtime_binding_arg, parse_runtime_pure_workers, print_json, resolve_source_selection,
    runtime_plan_options_for_selection, runtime_pure_config_for_selection, step_options,
};
use crate::app::debug::debug_project_readback_json;
use crate::app::image_declarations::{
    DeclaredImageObject, load_declared_image_objects, merge_declared_image_args,
    public_image_ref_arg, runtime_arg_name,
};
use crate::app::local_embedding::{
    DEFAULT_LOCAL_EMBEDDING_DIMENSIONS, DEFAULT_LOCAL_EMBEDDING_MODEL_ID,
    DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION, MAX_LOCAL_EMBEDDING_DIMENSIONS,
    local_hash_query_embedding,
};
use arcweft_agent_mcp::{
    model::{McpCallToolResult, McpContentBlock, McpListResourcesResult, McpReadResourceResult},
    resources::{
        list_resource_templates_result, list_resources_result, read_resource_result,
        resource_descriptor, tool_result_for_resource, tool_result_for_resources, trace_resource,
    },
    tools::agent_tool_descriptors,
};
use arcweft_agent_mcp_client::{ConnectOptions, McpAgentSession};
use arcweft_agent_protocol::action::{AgentActionDispatch, AgentActionKind, AgentActionTarget};
use arcweft_agent_protocol::artifact::RequiredEntity;
use arcweft_agent_protocol::diagnostic::{AgentDiagnostic, AgentDiagnosticSeverity};
use arcweft_agent_protocol::geometry::{
    AgentBBox, AgentCoordinateSpace, AgentPoint, AgentRgbaColor, AgentViewport,
};
use arcweft_agent_protocol::hit_test::{AgentHitTestHit, AgentHitTestReport};
use arcweft_agent_protocol::ids::PublicId as AgentPublicId;
use arcweft_agent_protocol::ids::{AgentResourceUri, AgentRunId, PublicId, SessionId, StableHash};
use arcweft_agent_protocol::image::{
    AgentCaptureMaskAvailability, AgentCaptureSourceIdentity, AgentImageAlignment,
    AgentImageComposition, AgentImageContentBBox, AgentImageCropOrigin, AgentImageFit,
    AgentImageKind, AgentImageMetadata, AgentImageObjectParam, AgentImageObjectRef,
    AgentImageRenderer, AgentImageResource, AgentImageScope, AgentImageTransform,
    AgentLayerCaptureRef, AgentLayerCaptureRefs, AgentObjectCaptureRef, AgentObjectCaptureRefs,
    AgentSelectedCaptureMask, AgentSelectedCaptureMetadata,
};
use arcweft_agent_protocol::object::{
    AgentObservedImageContent, AgentObservedLayer, AgentObservedObject, AgentObservedObjectContent,
};
use arcweft_agent_protocol::observation::AgentObservationReport;
use arcweft_agent_protocol::predicate::{CompareOp, Predicate, Probe};
use arcweft_agent_protocol::presentation::{AgentPresentationTree, AgentPresentationTreeQuery};
use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentInvokeAction, AgentSessionInfo, CaptureFormat, CaptureRequest,
    CaptureResult, CaptureTarget, ObservationEnvelope, ObserveRequest,
};
use arcweft_agent_protocol::proxy::{
    AgentPresentationObjectProxyParamQuery, AgentPresentationObjectProxyRef,
};
use arcweft_agent_protocol::resource::{
    AgentBinaryEncoding, AgentResource, AgentResourceBody, AgentResourceKind,
};
use arcweft_agent_protocol::rich_text::{
    AgentGlyphOrientation, AgentGlyphVerticalForm, AgentHitRegion, AgentHitRegionKind,
    AgentRichTextElementKind, AgentRichTextElementRef,
};
use arcweft_agent_protocol::session::{AgentAssignment, AgentAudioState};
use arcweft_agent_protocol::trace::AgentTraceRecord;
use arcweft_agent_protocol::ui::AgentUiTree;
use arcweft_agent_protocol::value::AgentValue;
use arcweft_core::effect::RuntimeCall;
use arcweft_core::engine::FlowStatusLabelStyle;
use arcweft_core::plan::FlowEvent;
use arcweft_core::task::TaskEvent;
use arcweft_debug_model::{
    chunk::{ChunkId, ChunkSourceKind, DebugChunk, PrivacyClass},
    diagnostic::DebugDiagnostic,
    embedding::EmbeddingModelDescriptor,
    event::{DebugEvent, DebugEventKind},
    graph::{DebugGraphEdge, DebugGraphSymbol},
    rag::{RagContextItem, RagContextPack, RagQuery, SearchChannel, SearchHit},
    repl::DebugReplCell,
    script::DebugScriptRun,
    session::{DebugSession, DebugSessionStatus},
    sink::DebugEventSink,
    source::DebugSourceFile,
};
use arcweft_debug_sqlite::store::{
    ChunkSearchResult, DebugChunkSearchResult, DebugRagQueryAudit, DebugStore, DebugTimelineEvent,
};
use arcweft_host_adapter::HostCallPolicy;
use arcweft_interaction_model::{
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};
use arcweft_lang_sema::check::{TypeCheckReport, TypeJudgmentRule};
use arcweft_lang_sema::project_index::{ProgramHash, project_semantic_index_from_hir};
use arcweft_lang_syntax::parser::{ParseCompletion, ParsedFragment, ParsedFragmentKind};
use arcweft_layout::{
    CaptureComposition as LayoutCaptureComposition, CaptureCropBounds,
    CaptureMaskMetadata as LayoutCaptureMaskMetadata, CaptureMetadata as LayoutCaptureMetadata,
    CaptureRendererKind, CaptureScope as LayoutCaptureScope, LayoutCoordinateSpace, LayoutPoint,
    LayoutRect, LayoutSize, ScalePolicy,
};
use arcweft_presentation::image::{
    ImageObjectAlignment, ImageObjectParam, ImageObjectPlayback, ImageObjectProxy,
    ImageObjectTransform,
};
use arcweft_rag::fusion::{FusionConfig, reciprocal_rank_fusion};
use arcweft_render_text::{
    LineDisplayFrame, Milli, RichTextControl, RichTextNode, RichTextObjectProxy, RichTextParam,
    RichTextPresentation, RichTextRange, RichTextRubyAnnotation, RichTextTextRun,
    RichTextTextSource, RuntimeLineContext,
};
use arcweft_source::SourceName;
#[cfg(feature = "agent-repl")]
use arcweft_tooling::agent_repl::AgentReplCellCompletionKind;
use arcweft_tooling::agent_repl::{
    AgentReplCompletionContext, AgentReplCompletionEntity, agent_repl_classification_from_fragment,
    agent_repl_classify_cell, agent_repl_completions, agent_repl_highlight_tokens,
    agent_repl_parse_fragment,
};

const AGENT_ROLE_DIALOGUE_TEXTBOX: &str = "dialogue_textbox";

fn agent_is_dialogue_textbox(object: &AgentObservedObject) -> bool {
    object.role == AGENT_ROLE_DIALOGUE_TEXTBOX
}
use arcweft_runtime_host::{UiFrameCommit, UiFrameImageItem};
use arcweft_ui::UiImageSourceTable;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::ValueEnum;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
#[cfg(feature = "agent-repl")]
use std::io::IsTerminal as _;
use std::io::{BufRead as _, Read as _, Write as _};
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;

#[derive(Clone, Debug)]
struct AgentObservationTrace {
    viewport: AgentViewport,
    objects: Vec<AgentObservedObject>,
    diagnostics: Vec<AgentDiagnostic>,
    task_request_count: usize,
    tick: usize,
}

#[derive(Clone, Debug)]
struct AgentLocalDevVisualClassifier;

impl arcweft_content_policy::ContentClassifier for AgentLocalDevVisualClassifier {
    fn identity(&self) -> arcweft_content_policy::ClassifierIdentity {
        arcweft_content_policy::ClassifierIdentity::new("arcweft.local-dev.visual", "2026-06-23")
    }

    fn classify(
        &self,
        input: arcweft_content_policy::PolicyInputRef<'_>,
    ) -> Result<arcweft_content_policy::ClassificationReport, arcweft_content_policy::PolicyError>
    {
        let runs = match input {
            arcweft_content_policy::PolicyInputRef::Text(_) => {
                vec![arcweft_content_policy::ClassifierRun::not_applicable(
                    self.identity(),
                )]
            }
            arcweft_content_policy::PolicyInputRef::Image(_)
            | arcweft_content_policy::PolicyInputRef::RenderedScene(_) => {
                vec![arcweft_content_policy::ClassifierRun::complete(
                    self.identity(),
                )]
            }
        };
        Ok(arcweft_content_policy::ClassificationReport {
            findings: Vec::new(),
            runs,
        })
    }
}

fn agent_publish_resource_with_mode(
    mode: AgentContentPolicyMode,
    resource: AgentResource,
) -> Result<arcweft_agent_policy::PublishedAgentResource, String> {
    let result = match mode {
        AgentContentPolicyMode::Strict => {
            arcweft_agent_policy::AgentContentPolicyGate::strict_builtin().publish(resource)
        }
        AgentContentPolicyMode::LocalDev => {
            if resource.kind == AgentResourceKind::Image
                && !matches!(
                    resource.image.as_ref().map(|metadata| metadata.kind),
                    Some(AgentImageKind::Color)
                )
            {
                return arcweft_agent_policy::AgentContentPolicyGate::strict_builtin()
                    .publish(resource)
                    .map_err(|error| format!("Agent content-policy gate failed: {error}"));
            }
            let classifier = arcweft_content_policy::CompositeClassifier::new(
                arcweft_content_policy::RuleClassifier::strict_builtin(),
                AgentLocalDevVisualClassifier,
            );
            let engine = arcweft_content_policy::ContentPolicyEngine::new(
                classifier,
                arcweft_content_policy::PolicyProfile::strict_default(),
            );
            arcweft_agent_policy::AgentContentPolicyGate::new(engine).publish(resource)
        }
    };
    result.map_err(|error| format!("Agent content-policy gate failed: {error}"))
}

fn agent_publish_resources_with_mode(
    mode: AgentContentPolicyMode,
    resources: Vec<AgentResource>,
) -> Result<Vec<arcweft_agent_policy::PublishedAgentResource>, String> {
    resources
        .into_iter()
        .map(|resource| agent_publish_resource_with_mode(mode, resource))
        .collect()
}

mod capture;
mod image_mapping;
mod mcp_debug;
mod mcp_protocol;
mod mcp_rag;
mod mcp_resources;
pub(in crate::app::agent) mod observe;
mod observe_resources;
mod repl;
mod repl_command_bridge;
mod repl_command_format;
mod repl_project_binding;
#[cfg(test)]
mod repl_snapshot;
mod runtime_observation;
#[cfg(test)]
mod tests;

use capture::{
    AgentCaptureReadRequest, AgentCaptureScope, AgentImageFrameStore, AgentStoredImageFrame,
    AgentStoredImagePlacement, AgentUiImageObservation, agent_capture_request_from_uri,
    agent_image_object_for_capture_scope, agent_native_capture_image,
    agent_native_capture_image_with_frame_store,
    agent_native_capture_resource_with_session_and_frame_store, agent_native_text_origin,
    agent_observe_capture_resource,
};
use image_mapping::{
    AgentSelectedCaptureMetadataSpec, agent_capture_uri, agent_encode_png,
    agent_frame_capture_uri_for_page, agent_image_observation_from_ui_frame,
    agent_layout_rect_from_bbox, agent_measure_frame_elements_with_session,
    agent_native_rich_text_element_bboxes, agent_native_textbox_capture_bbox_for_page,
    agent_object_capture_refs_for_page, agent_object_id_color, agent_object_layers,
    agent_object_matches_layer, agent_observed_layers, agent_observed_rich_text, agent_overlay_svg,
    agent_rich_text_child_objects, agent_rich_text_ranges_overlap, agent_scoped_capture_name,
    agent_selected_capture_metadata_for_ref, agent_textbox_object, hash_hex,
};
use mcp_debug::{
    agent_mcp_call_debug_close_stale_sessions, agent_mcp_call_debug_graph_inventory,
    agent_mcp_call_debug_repl_cells, agent_mcp_call_debug_script_runs, agent_mcp_call_debug_search,
    agent_mcp_call_debug_session_timeline, agent_mcp_call_debug_source_files,
    agent_mcp_debug_store_path, agent_mcp_non_empty_string_argument,
    agent_mcp_search_channel_label,
};
use mcp_protocol::{
    AgentMcpFrame, AgentMcpObservation, AgentMcpProjectContext, AgentMcpState,
    AgentObservationRunContext, AgentObservationRunOutput, AgentObservationState,
    AgentPublishedResourceCache, NativeAgentObservedSnapshot, NativeAgentRuntimeState,
    agent_mcp_agent_value, agent_mcp_bool_argument, agent_mcp_cached_published_resource,
    agent_mcp_command, agent_mcp_project_context_from_hir, agent_mcp_store_observation,
    agent_publish_resource_for_state,
};
use mcp_rag::{
    agent_mcp_call_rag_context_read, agent_mcp_call_rag_explain, agent_mcp_call_rag_query,
    agent_mcp_content_hash, agent_mcp_json_privacy, agent_mcp_max_privacy_argument,
    agent_mcp_observation_debug_read_privacy_error, agent_mcp_optional_debug_store_path,
    agent_mcp_privacy_class_argument, agent_mcp_rag_context_pack,
};
use mcp_resources::{
    agent_json_path, agent_mcp_arguments_request_observe, agent_mcp_cached_capture_resource,
    agent_mcp_cached_trace_resource, agent_mcp_call_capture, agent_mcp_call_resource_read,
    agent_mcp_call_trace_read, agent_mcp_capture_time_argument, agent_mcp_current_resources,
    agent_mcp_error_response, agent_mcp_json_tool_error, agent_mcp_json_tool_result,
    agent_mcp_latest_capture_resource, agent_mcp_observation_state_summary,
    agent_mcp_observe_if_requested, agent_mcp_observe_runtime, agent_mcp_predicate_matches,
    agent_mcp_resource_read_privacy_error, agent_mcp_resource_read_privacy_message,
    agent_mcp_run_observation, agent_mcp_session_context_resource_for_uri,
    agent_mcp_success_response, agent_mcp_u32_argument, agent_mcp_u64_argument,
    agent_mcp_uncached_resource_by_uri, agent_mcp_usize_argument, agent_mcp_wait_report_value,
    agent_native_capture_session_for_hir, extend_agent_observation_with_runtime_images,
};
use observe::{
    NativeAgentScriptSession, agent_assignment_value, agent_capture_time_millis,
    agent_capture_time_seconds_from_step, agent_hit_test_command, agent_hit_test_report,
    agent_observation_for_options, agent_observation_report_for_options,
    agent_observe_capture_time_seconds, agent_observe_command, agent_observe_effective_steps,
    agent_observe_report_capture_time_millis, agent_observe_resource_by_uri,
    agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store,
    agent_report_capture_time_seconds, native_agent_action_input_events,
    validate_agent_observe_options,
};
use observe_resources::{
    AgentObserveResourceOutput, agent_json_error, agent_observe_cached_image_resource,
    agent_observe_image_resource, agent_observe_list_resources, agent_observe_mcp_resource_output,
    agent_observe_resource,
};
use repl::agent_repl_command;
use runtime_observation::{
    AgentImageOutput, AgentRasterCapture, agent_image_kind, agent_image_scope_for_capture_scope,
    agent_native_visual_diagnostics, agent_observe_image_output,
    agent_refresh_observation_object_indexes, agent_runtime_presentation_image_observation,
    run_agent_observation,
};

pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Observe(options) => agent_observe_command(&options, adapter_registrars),
        AgentCommand::HitTest(options) => agent_hit_test_command(&options, adapter_registrars),
        AgentCommand::Mcp(options) => agent_mcp_command(&options, adapter_registrars),
        AgentCommand::Repl(options) => agent_repl_command(&options, adapter_registrars),
        AgentCommand::Rag { command } => super::agent_rag_command(*command),
        AgentCommand::Script { command } => {
            super::agent_script_command(*command, adapter_registrars)
        }
    }
}
