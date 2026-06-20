use super::super::runtime::{
    NativeRunHost, RuntimeExecutorInstance, apply_runtime_entry_selection, report_path,
};
use super::{
    AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT, AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH, AgentCaptureBlob,
    AgentCommand, AgentControllerRunConfig, AgentControllerRunReport, AgentHitTestOptions,
    AgentMcpOptions, AgentObserveCaptureKind, AgentObserveImageKind, AgentObserveMcpFormat,
    AgentObserveOptions, AgentObserveResourceKind, AgentReplOptions, AgentRunner,
    AgentRunnerConfig, AgentScriptRunInput, AgentScriptRunOptions, AgentScriptRunReport,
    AgentSession, AgentSourceRagIndex, CliAgentSession, CliRuntimeExecutorTier,
    CliRuntimePureWorkers, CliRuntimeStepMode, CollectingDebugSink, ExitCode, FlowFiberStatus,
    LineDisplayCatalog, NativeAdapterRegistrar, NativeTaskBridge, NoopRagService, Path, PathBuf,
    ProfileOptions, RuntimeStepInput, RuntimeStepResult, agent_cli_session_id,
    agent_debug_finish_runtime_session, agent_debug_start_runtime_session,
    agent_program_summary_rag_candidate, agent_rag_index_graph, agent_rag_index_program_graph,
    agent_rag_program_hash, agent_rag_select_context_chunk, agent_rag_source_paths,
    agent_script_project_index, agent_script_run_bundle, agent_script_run_input,
    agent_script_run_report_from_result, agent_script_runtime_policy,
    agent_script_runtime_policy_for_bundle, agent_source_rag_index, flow_status_label, fs,
    load_and_check_selection, lower_source_runtime_plan_with_stats_and_options,
    native_host_policy_for_selection, parse_agent_script_signal_arg, parse_agent_script_state_arg,
    parse_runtime_binding_arg, parse_runtime_pure_workers, print_json, resolve_source_selection,
    runtime_plan_options_for_selection, runtime_pure_config_for_selection, step_options,
};
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
    McpCallToolResult, McpContentBlock, agent_tool_descriptors, list_resource_templates_result,
    list_resources_result, read_resource_result, resource_descriptor, tool_result_for_resource,
    tool_result_for_resources, trace_resource,
};
use arcweft_agent_protocol::artifact::RequiredEntity;
use arcweft_agent_protocol::ids::{AgentResourceUri, AgentRunId, PublicId, SessionId, StableHash};
use arcweft_agent_protocol::predicate::{CompareOp, Predicate, Probe};
use arcweft_agent_protocol::protocol::{
    ActionResult, AgentAction, AgentInvokeAction, AgentSessionInfo, CaptureFormat, CaptureRequest,
    CaptureResult, CaptureTarget, ObservationEnvelope, ObserveRequest,
};
use arcweft_agent_protocol::trace::AgentTraceRecord;
use arcweft_agent_protocol::value::AgentValue;
use arcweft_agent_protocol::{
    AgentActionDispatch, AgentActionKind, AgentActionTarget, AgentAssignment, AgentAudioState,
    AgentBBox, AgentCoordinateSpace, AgentDiagnostic, AgentDiagnosticSeverity,
    AgentGlyphOrientation, AgentGlyphVerticalForm, AgentHitRegion, AgentHitRegionKind,
    AgentHitTestHit, AgentHitTestReport, AgentImageAlignment, AgentImageComposition,
    AgentImageContentBBox, AgentImageCropOrigin, AgentImageFit, AgentImageKind, AgentImageMetadata,
    AgentImageObjectParam, AgentImageObjectRef, AgentImageRenderer, AgentImageResource,
    AgentImageScope, AgentImageTransform, AgentLayerCaptureRef, AgentLayerCaptureRefs,
    AgentObjectCaptureRef, AgentObjectCaptureRefs, AgentObservationReport,
    AgentObservedImageContent, AgentObservedLayer, AgentObservedObject, AgentObservedObjectContent,
    AgentPoint, AgentPresentationObjectProxyParamQuery, AgentPresentationObjectProxyRef,
    AgentPresentationTree, AgentPresentationTreeQuery, AgentResource, AgentResourceBody,
    AgentResourceKind, AgentRgbaColor, AgentRichTextElementKind, AgentRichTextElementRef,
    AgentUiTree, AgentViewport,
};
use arcweft_core::effect::RuntimeCall;
use arcweft_core::plan::FlowEvent;
use arcweft_core::step::InputEvent as RuntimeInputEvent;
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
use arcweft_lang_sema::check::{TypeCheckReport, TypeJudgmentRule};
use arcweft_lang_syntax::ast::{
    flow::Stmt,
    ids::EntityRefSyntax,
    pattern::{Pattern, VariantPatternPayload},
};
use arcweft_lang_syntax::expr::{CallArg, Expr, Literal};
use arcweft_lang_syntax::parser::{ParseCompletion, ParsedFragment, ParsedFragmentKind};
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

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_debug_model::{
        diagnostic::DebugDiagnostic,
        graph::{DebugGraphEdge, DebugGraphSymbol},
        history::DebugHistoryEntry,
        test_result::DebugTestResult,
    };

    #[test]
    fn agent_mcp_script_run_options_accept_native_runtime_arguments() {
        let options = agent_mcp_script_run_options(&serde_json::json!({
            "path": "samples/agent-script/cli-run-smoke.awfagent",
            "native_source": "samples/rich-text-showcase.arcw",
            "executor": "aot",
            "pure_backend": "jit",
            "pure_workers": 2,
            "pure_batch_min_len": 4,
            "pure_object_artifacts": true,
            "math_backend": "ndarray",
            "math_wgpu_min_elements": 16,
            "native_mode": "game",
            "values": {
                "ready": true,
                "route": "@flow.opening",
                "count": 3
            }
        }))
        .expect("script.run options parse native runtime arguments");

        assert_eq!(options.executor, CliRuntimeExecutorTier::Aot);
        assert_eq!(
            options.pure_backend,
            Some(crate::app::runtime::CliRuntimePureBackend::Jit)
        );
        assert!(matches!(
            options.pure_workers,
            Some(CliRuntimePureWorkers::Fixed(2))
        ));
        assert_eq!(options.pure_batch_min_len, Some(4));
        assert!(options.pure_object_artifacts);
        assert_eq!(
            options.math_backend,
            Some(crate::app::runtime::CliRuntimeMathBackend::Ndarray)
        );
        assert_eq!(options.math_wgpu_min_elements, Some(16));
        assert!(matches!(options.native_mode, CliRuntimeStepMode::Game));
        assert_eq!(options.values.len(), 3);
        assert!(options.values.iter().any(|binding| binding.name == "ready"
            && matches!(binding.value, arcweft_core::value::RuntimeValue::Bool(true))));
        assert!(options.values.iter().any(|binding| binding.name == "route"
            && matches!(
                &binding.value,
                arcweft_core::value::RuntimeValue::EntityRef(value) if value == "flow.opening"
            )));
        assert!(options.values.iter().any(|binding| binding.name == "count"
            && matches!(
                binding.value,
                arcweft_core::value::RuntimeValue::Int(arcweft_core::value::RuntimeInt::I64(3))
            )));
    }

    fn test_agent_observation_report(capture_time_millis: Option<u32>) -> AgentObservationReport {
        AgentObservationReport {
            status: "ok".to_owned(),
            session_id: "cli".to_owned(),
            tick: 3,
            frame_id: "frame.3".to_owned(),
            state_hash: "state".to_owned(),
            render_hash: "render".to_owned(),
            source: "test.arcw".to_owned(),
            viewport: AgentViewport {
                width: 1280,
                height: 720,
                scale: 1.0,
            },
            images: Vec::new(),
            layers: Vec::new(),
            objects: Vec::new(),
            presentation_tree: AgentPresentationTree::from_layers_and_objects(&[], &[]),
            actions: Vec::new(),
            ui_tree: AgentUiTree {
                root: "ui.root".to_owned(),
                children: Vec::new(),
            },
            scene_graph: Vec::new(),
            audio_state: AgentAudioState {
                active_voices: Vec::new(),
                pending_events: Vec::new(),
            },
            logs: Vec::new(),
            signals: Vec::new(),
            metrics: Vec::new(),
            events: Vec::new(),
            diagnostics: Vec::new(),
            steps: 3,
            capture_time_millis,
            task_requests: 0,
            final_status: "done".to_owned(),
            overlay_svg: None,
        }
    }

    fn test_line_display_frame() -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("line.test".to_owned()),
            callee: "test".to_owned(),
            text: String::new(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
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
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: arcweft_render_text::RichTextDocument::new(vec![RichTextNode::Text {
                text: "native attachment seed".to_owned(),
            }]),
        };
        spec.resolve_frame(&RuntimeLineContext::default())
            .expect("test frame resolves")
    }

    fn test_ruby_split_page_boundary_frame() -> LineDisplayFrame {
        LineDisplayFrame {
            line: arcweft_core::plan::RuntimeLineId("line.page.ruby.atomic".to_owned()),
            callee: "test".to_owned(),
            text: "ABCDE".to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap {
                text_runs: vec![
                    RichTextTextRun {
                        range: RichTextRange::new(0, 2),
                        source: RichTextTextSource::Text,
                        node_index: 0,
                        styles: Vec::new(),
                        presentation: arcweft_render_text::RichTextPresentation::default(),
                    },
                    RichTextTextRun {
                        range: RichTextRange::new(2, 5),
                        source: RichTextTextSource::Text,
                        node_index: 2,
                        styles: Vec::new(),
                        presentation: arcweft_render_text::RichTextPresentation::default(),
                    },
                ],
                ruby_annotations: vec![RichTextRubyAnnotation {
                    base_range: RichTextRange::new(1, 4),
                    ruby: "ruby".to_owned(),
                    node_index: 1,
                    styles: Vec::new(),
                    presentation: arcweft_render_text::RichTextPresentation::default(),
                }],
                controls: vec![arcweft_render_text::RichTextControlMarker {
                    node_index: 2,
                    control: RichTextControl::Page,
                    range: None,
                }],
                host_events: Vec::new(),
            },
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    #[test]
    fn agent_page_ranges_do_not_split_ruby_base_ranges() {
        let frame = test_ruby_split_page_boundary_frame();

        assert_eq!(agent_rich_text_page_ranges(&frame), vec![0..4, 4..5]);
        assert_eq!(
            agent_rich_text_page_for_range(&frame, RichTextRange::new(1, 4)),
            0
        );
    }

    fn assert_seconds_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < f32::EPSILON,
            "expected {expected} seconds, got {actual}"
        );
    }

    #[test]
    fn mcp_capture_time_prefers_explicit_time_then_capture_step_then_report_time() {
        let report = test_agent_observation_report(Some(2500));

        assert_seconds_close(
            agent_mcp_capture_time_seconds(
                &serde_json::json!({"capture_time": 0.125, "capture_step": 9}),
                &report,
                "arcweft.capture",
            )
            .expect("explicit capture time is valid"),
            0.125,
        );
        assert_seconds_close(
            agent_mcp_capture_time_seconds(
                &serde_json::json!({"capture_step": 9}),
                &report,
                "arcweft.capture",
            )
            .expect("capture step time is valid"),
            9.0,
        );
        assert_seconds_close(
            agent_mcp_capture_time_seconds(&serde_json::json!({}), &report, "arcweft.capture")
                .expect("report capture time is valid"),
            2.5,
        );
        assert_seconds_close(
            agent_mcp_capture_time_seconds(
                &serde_json::json!({}),
                &test_agent_observation_report(None),
                "arcweft.capture",
            )
            .expect("default capture time is valid"),
            60.0,
        );
    }

    #[test]
    fn native_agent_advance_text_requires_enabled_semantic_action() {
        let mut report = test_agent_observation_report(None);
        report.actions.push(AgentActionTarget {
            id: "action.advance_text.object.dialogue.0.0".to_owned(),
            target: "object.dialogue.0.0".to_owned(),
            action: AgentActionKind::AdvanceText,
            kind: AgentActionDispatch::Semantic,
            enabled: false,
        });

        assert!(matches!(
            native_agent_advance_text_input_events(&report),
            Err(NativeAgentScriptSessionError::ActionUnavailable)
        ));

        report.actions[0].enabled = true;
        let events = native_agent_advance_text_input_events(&report)
            .expect("enabled semantic advance_text action dispatches");
        assert_eq!(
            events,
            vec![RuntimeInputEvent {
                kind: "advance".to_owned(),
                payload: None,
            }]
        );
    }

    #[test]
    fn native_agent_invoke_requires_enabled_semantic_action() {
        let mut report = test_agent_observation_report(None);
        let target = PublicId::new("target.sample.pulse").expect("valid target id");
        report.actions.push(AgentActionTarget {
            id: "action.inspect.pulse".to_owned(),
            target: "target.sample.pulse".to_owned(),
            action: AgentActionKind::Invoke,
            kind: AgentActionDispatch::Semantic,
            enabled: false,
        });

        assert!(matches!(
            native_agent_invoke_input_events(&report, &target, "action.inspect.pulse"),
            Err(NativeAgentScriptSessionError::ActionUnavailable)
        ));

        report.actions[0].enabled = true;
        let events = native_agent_invoke_input_events(&report, &target, "action.inspect.pulse")
            .expect("enabled semantic invoke action dispatches");
        assert_eq!(
            events,
            vec![RuntimeInputEvent {
                kind: "invoke".to_owned(),
                payload: Some("action.inspect.pulse".to_owned()),
            }]
        );

        assert!(matches!(
            native_agent_invoke_input_events(&report, &target, "action.inspect.other"),
            Err(NativeAgentScriptSessionError::ActionUnavailable)
        ));
    }

    #[test]
    fn agent_mcp_debug_read_tools_return_cached_observation_data() {
        let mut report = test_agent_observation_report(None);
        report.final_status = "running".to_owned();
        report.signals.push(AgentAssignment {
            name: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        });
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "info".to_owned(),
            message: "opened route".to_owned(),
            fields: Vec::new(),
        });
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "debug".to_owned(),
            message: "layout pass".to_owned(),
            fields: Vec::new(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            ..AgentMcpState::default()
        };

        let state_result = agent_mcp_call_get_state(
            &serde_json::json!({"path": "final_status"}),
            &mut state,
            &[],
        )
        .expect("state read succeeds");
        assert_eq!(
            mcp_text_json(&state_result)["value"],
            serde_json::json!("running")
        );

        let signal_result = agent_mcp_call_signal_get(
            &serde_json::json!({"name": "signal.current_flow"}),
            &mut state,
            &[],
        )
        .expect("signal read succeeds");
        assert_eq!(
            mcp_text_json(&signal_result)["value"],
            serde_json::json!("flow.opening")
        );

        let log_result = agent_mcp_call_log_query(
            &serde_json::json!({"level": "info", "contains": "route", "limit": 5}),
            &mut state,
            &[],
        )
        .expect("log query succeeds");
        let logs = mcp_text_json(&log_result);
        assert_eq!(logs["count"], serde_json::json!(1));
        assert_eq!(
            logs["logs"][0]["message"],
            serde_json::json!("opened route")
        );
    }

    #[test]
    fn agent_mcp_debug_read_tools_enforce_max_privacy() {
        let mut report = test_agent_observation_report(None);
        report.final_status = "running".to_owned();
        report.signals.push(AgentAssignment {
            name: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        });
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "info".to_owned(),
            message: "opened route".to_owned(),
            fields: Vec::new(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            ..AgentMcpState::default()
        };

        let state_result = agent_mcp_call_get_state(
            &serde_json::json!({"path": "final_status", "max_privacy": "public"}),
            &mut state,
            &[],
        )
        .expect("state privacy block serializes");
        assert!(state_result.is_error);
        assert_eq!(
            mcp_text_json(&state_result)["max_privacy"],
            serde_json::json!("public")
        );

        let signal_result = agent_mcp_call_signal_get(
            &serde_json::json!({"name": "signal.current_flow", "max_privacy": "public"}),
            &mut state,
            &[],
        )
        .expect("signal privacy block serializes");
        assert!(signal_result.is_error);
        assert_eq!(
            mcp_text_json(&signal_result)["privacy"],
            serde_json::json!("project")
        );

        let log_result = agent_mcp_call_log_query(
            &serde_json::json!({"level": "info", "max_privacy": "public"}),
            &mut state,
            &[],
        )
        .expect("log privacy block serializes");
        assert!(log_result.is_error);
    }

    #[test]
    fn agent_mcp_resource_read_enforces_capture_privacy() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-resource-read-audit-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let mut state = AgentMcpState {
            report: Some(test_agent_observation_report(None)),
            capture_resources: vec![AgentResource {
                uri: "arcweft://session/cli/frame/0/color.png".to_owned(),
                kind: AgentResourceKind::Image,
                mime_type: "image/png".to_owned(),
                hash: "blake3:test".to_owned(),
                image: None,
                body: AgentResourceBody::BytesBase64(
                    arcweft_agent_protocol::AgentBinaryResourceBody {
                        encoding: arcweft_agent_protocol::AgentBinaryEncoding::Base64,
                        data: "iVBORw0KGgo=".to_owned(),
                    },
                ),
            }],
            ..AgentMcpState::default()
        };

        let default_result = agent_mcp_call_resource_read(
            &serde_json::json!({
                "uri": "arcweft://session/cli/frame/0/color.png",
                "path": db_path.display().to_string()
            }),
            &mut state,
        )
        .expect("resource privacy block serializes");
        assert!(default_result.is_error);
        let default_error = mcp_text_json(&default_result);
        assert_eq!(default_error["privacy"], serde_json::json!("sensitive"));
        assert_eq!(default_error["max_privacy"], serde_json::json!("project"));

        let allowed_result = agent_mcp_call_resource_read(
            &serde_json::json!({
                "uri": "arcweft://session/cli/frame/0/color.png",
                "max_privacy": "sensitive",
                "path": db_path.display().to_string()
            }),
            &mut state,
        )
        .expect("sensitive resource read succeeds");
        assert!(!allowed_result.is_error);
        let store = DebugStore::open(&db_path).expect("debug store opens");
        let events = store
            .session_timeline_with_max_privacy(
                Some("session.mcp.resource_read"),
                None,
                10,
                PrivacyClass::Sensitive,
            )
            .expect("resource read audit timeline");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_kind, "resource_read");
        assert_eq!(events[0].payload["outcome"], serde_json::json!("blocked"));
        assert_eq!(events[0].payload["privacy"], serde_json::json!("sensitive"));
        assert_eq!(
            events[0].payload["max_privacy"],
            serde_json::json!("project")
        );
        assert_eq!(events[1].payload["outcome"], serde_json::json!("allowed"));

        let direct_default = agent_mcp_resource_read(
            &serde_json::json!({"uri": "arcweft://session/cli/frame/0/color.png"}),
            &mut state,
        )
        .expect_err("direct resources/read blocks sensitive resources by default");
        assert!(direct_default.contains("resources/read resource"));
        assert!(direct_default.contains("sensitive"));

        let direct_allowed = agent_mcp_resource_read(
            &serde_json::json!({
                "uri": "arcweft://session/cli/frame/0/color.png",
                "max_privacy": "sensitive"
            }),
            &mut state,
        )
        .expect("direct resources/read accepts explicit sensitive read");
        assert_eq!(
            direct_allowed["contents"][0]["uri"],
            serde_json::json!("arcweft://session/cli/frame/0/color.png")
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_session_context_resource_redacts_source_and_enforces_privacy() {
        let mut report = test_agent_observation_report(Some(1500));
        report.source = "C:\\Users\\sanze\\secret\\game.arcw".to_owned();
        report.signals.push(AgentAssignment {
            name: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            trace_resources: vec![AgentResource {
                uri: "arcweft://run/run.test/trace.arcwx".to_owned(),
                kind: AgentResourceKind::Trace,
                mime_type: "application/vnd.arcweft.agent-trace+json".to_owned(),
                hash: "trace:test".to_owned(),
                image: None,
                body: AgentResourceBody::Json(serde_json::json!([])),
            }],
            ..AgentMcpState::default()
        };

        let resources = agent_mcp_current_resources(&state).expect("resources build");
        let context = resources
            .iter()
            .find(|resource| resource.kind == AgentResourceKind::SessionContext)
            .expect("session context resource is listed");
        assert_eq!(context.uri, "arcweft://session/cli/context.json");
        let AgentResourceBody::Json(body) = &context.body else {
            panic!("session context is JSON");
        };
        assert_eq!(body["privacy_class"], serde_json::json!("project"));
        assert_eq!(body["latest_observation"]["tick"], serde_json::json!(3));
        assert_eq!(
            body["latest_observation"]["source_present"],
            serde_json::json!(true)
        );
        assert_eq!(
            body["resources"]["trace_resource_count"],
            serde_json::json!(1)
        );
        assert!(
            !serde_json::to_string(body)
                .expect("context serializes")
                .contains("secret")
        );

        let blocked = agent_mcp_call_resource_read(
            &serde_json::json!({
                "uri": "arcweft://session/cli/context.json",
                "max_privacy": "public"
            }),
            &mut state,
        )
        .expect("privacy block serializes");
        assert!(blocked.is_error);
        assert_eq!(
            mcp_text_json(&blocked)["privacy"],
            serde_json::json!("project")
        );

        let allowed = agent_mcp_call_resource_read(
            &serde_json::json!({
                "uri": "arcweft://session/cli/context.json",
                "max_privacy": "project"
            }),
            &mut state,
        )
        .expect("project context read succeeds");
        assert!(!allowed.is_error);
    }

    #[test]
    fn agent_mcp_session_context_resource_is_available_for_trace_only_session() {
        let mut state = AgentMcpState {
            trace_resources: vec![AgentResource {
                uri: "arcweft://run/run.trace/trace.arcwx".to_owned(),
                kind: AgentResourceKind::Trace,
                mime_type: "application/vnd.arcweft.agent-trace+json".to_owned(),
                hash: "trace:run.trace".to_owned(),
                image: None,
                body: AgentResourceBody::Json(serde_json::json!([])),
            }],
            ..AgentMcpState::default()
        };

        let resources = agent_mcp_current_resources(&state).expect("resources build");
        assert!(resources.iter().any(|resource| {
            resource.kind == AgentResourceKind::SessionContext
                && resource.uri == "arcweft://session/mcp/context.json"
        }));
        let read = agent_mcp_resource_read(
            &serde_json::json!({"uri": "arcweft://session/mcp/context.json"}),
            &mut state,
        )
        .expect("trace-only context reads through resources/read");
        let contents = read["contents"].as_array().expect("contents array");
        assert!(
            contents[0]["text"]
                .as_str()
                .expect("text resource")
                .contains("\"observed\":false")
        );
    }

    #[test]
    fn agent_mcp_rag_query_returns_explainable_context_pack() {
        let mut report = test_agent_observation_report(None);
        report.final_status = "running".to_owned();
        report.signals.push(AgentAssignment {
            name: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        });
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "info".to_owned(),
            message: "opened route".to_owned(),
            fields: Vec::new(),
        });
        report.diagnostics.push(AgentDiagnostic {
            step: 3,
            severity: AgentDiagnosticSeverity::Warning,
            source: Some("flow.opening".to_owned()),
            code: Some("layout-warning".to_owned()),
            effect_id: None,
            message: "route label needs layout review".to_owned(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            ..AgentMcpState::default()
        };

        let rag_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "current_flow route",
                "roots": ["signal.current_flow"],
                "limit": 4,
                "max_context_bytes": 4096
            }),
            &mut state,
            &[],
        )
        .expect("RAG query succeeds");
        let pack = mcp_text_json(&rag_result);

        assert_eq!(pack["schema_version"], serde_json::json!(1));
        assert_eq!(
            pack["query"]["text"],
            serde_json::json!("current_flow route")
        );
        assert_eq!(
            pack["query"]["roots"],
            serde_json::json!(["signal.current_flow"])
        );
        assert!(
            pack["items"]
                .as_array()
                .is_some_and(|items| !items.is_empty())
        );
        assert!(pack["items"].as_array().unwrap().iter().any(|item| {
            item["title"] == serde_json::json!("Signal signal.current_flow")
                && item["channels"]
                    .as_array()
                    .is_some_and(|channels| channels.contains(&serde_json::json!("exact_entity")))
        }));
        assert_eq!(state.rag_context_packs.len(), 1);
    }

    #[test]
    fn agent_mcp_rag_explain_and_context_read_use_cached_pack() {
        let mut report = test_agent_observation_report(None);
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "info".to_owned(),
            message: "opening route debug context body".to_owned(),
            fields: Vec::new(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            ..AgentMcpState::default()
        };
        let rag_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "opening route debug",
                "limit": 4,
                "max_context_bytes": 4096
            }),
            &mut state,
            &[],
        )
        .expect("RAG query succeeds");
        let pack = mcp_text_json(&rag_result);
        let chunk_id = pack["items"][0]["chunk_id"]
            .as_str()
            .expect("first item chunk id")
            .to_owned();

        let explain_result =
            agent_mcp_call_rag_explain(&serde_json::json!({}), &state).expect("explain succeeds");
        let explain = mcp_text_json(&explain_result);
        assert_eq!(
            explain["query"]["text"],
            serde_json::json!("opening route debug")
        );
        assert!(explain["items"][0].get("body").is_none());
        assert!(
            explain["items"][0]["body_bytes"]
                .as_u64()
                .is_some_and(|bytes| bytes > 0)
        );

        let read_result = agent_mcp_call_rag_context_read(
            &serde_json::json!({
                "chunk_id": chunk_id,
                "max_bytes": 12
            }),
            &state,
        )
        .expect("context read succeeds");
        let read = mcp_text_json(&read_result);
        assert_eq!(read["returned_bytes"], serde_json::json!(12));
        assert_eq!(read["truncated"], serde_json::json!(true));
    }

    #[test]
    fn agent_mcp_rag_query_persists_observation_context_to_debug_store() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-query-persist-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let mut report = test_agent_observation_report(None);
        report.logs.push(arcweft_core::effect::RuntimeLog {
            level: "info".to_owned(),
            message: "persisted observation route context body".to_owned(),
            fields: Vec::new(),
        });
        let mut state = AgentMcpState {
            report: Some(report),
            ..AgentMcpState::default()
        };
        let query_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "persisted observation route",
                "path": db_path.display().to_string(),
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            &mut state,
            &[],
        )
        .expect("persisted MCP RAG query succeeds");
        let pack = mcp_text_json(&query_result);
        let query_id = pack["query"]["query_id"]
            .as_str()
            .expect("query id")
            .to_owned();
        let chunk_id = pack["items"]
            .as_array()
            .expect("items")
            .iter()
            .find(|item| {
                item["body"]
                    .as_str()
                    .is_some_and(|body| body.contains("persisted observation route context body"))
            })
            .and_then(|item| item["chunk_id"].as_str())
            .expect("persisted observation chunk id")
            .to_owned();
        assert_eq!(state.rag_context_packs.len(), 1);
        let store = DebugStore::open(&db_path).expect("debug store opens");
        let db_counts = store.stats().expect("debug stats");
        assert_eq!(db_counts.rag_queries, 1);
        assert!(db_counts.chunks > 0);
        drop(store);

        let empty_state = AgentMcpState::default();
        let explain_result = agent_mcp_call_rag_explain(
            &serde_json::json!({
                "query_id": query_id,
                "path": db_path.display().to_string(),
                "max_privacy": "project"
            }),
            &empty_state,
        )
        .expect("persisted MCP RAG explain succeeds");
        let explain = mcp_text_json(&explain_result);
        assert_eq!(explain["source"], serde_json::json!("debug_store"));
        assert_eq!(
            explain["query"]["text"],
            serde_json::json!("persisted observation route")
        );

        let read_result = agent_mcp_call_rag_context_read(
            &serde_json::json!({
                "query_id": explain["query"]["query_id"].as_str().unwrap(),
                "chunk_id": chunk_id,
                "path": db_path.display().to_string(),
                "max_privacy": "project",
                "max_bytes": 4096
            }),
            &empty_state,
        )
        .expect("persisted MCP RAG context read succeeds");
        let read = mcp_text_json(&read_result);
        assert!(
            read["body"]
                .as_str()
                .is_some_and(|body| body.contains("persisted observation route context body"))
        );
        assert_eq!(read["truncated"], serde_json::json!(false));
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_reads_preindexed_debug_store_chunks() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-preindexed-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let program_hash = StableHash::new("blake3:mcp-rag-preindexed-program").expect("hash");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        store
            .upsert_chunk(&DebugChunk {
                id: ChunkId::new("chunk:mcp-preindexed-choice"),
                program_hash: Some(program_hash.clone()),
                source_kind: ChunkSourceKind::Symbol,
                source_key: "choice.opening.listen".to_owned(),
                title: "choice.opening.listen".to_owned(),
                body: "Preindexed source symbol for opening choice listen action".to_owned(),
                content_hash: StableHash::new("blake3:mcp-rag-preindexed-chunk").expect("hash"),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: vec![PublicId::new("choice.opening.listen").expect("public id")],
                privacy: PrivacyClass::Project,
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            })
            .expect("chunk");
        drop(store);

        let mut state = AgentMcpState::default();
        let query_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "choice.opening",
                "roots": ["choice.opening.listen"],
                "path": db_path.display().to_string(),
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            &mut state,
            &[],
        )
        .expect("MCP RAG query reads preindexed chunks");
        let pack = mcp_text_json(&query_result);
        assert!(
            pack["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["chunk_id"] == "chunk:mcp-preindexed-choice"
                        && item["kind"] == "symbol"
                        && item["channels"].as_array().is_some_and(|channels| {
                            channels.contains(&serde_json::json!("exact_entity"))
                        })
                })
            }),
            "MCP RAG query should return the preindexed project symbol chunk: {pack}"
        );
        assert_eq!(state.rag_context_packs.len(), 1);
        let store = DebugStore::open(&db_path).expect("debug store reopens");
        assert_eq!(store.stats().expect("stats").rag_queries, 1);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_includes_source_program_summary() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-source-program-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let choice_source = workspace_root.join("samples/agent-script/native-choice-dispatch.arcw");
        let rich_text_source = workspace_root.join("samples/rich-text-showcase.arcw");
        let mut state = AgentMcpState::default();

        let query_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "program_rag_index",
                "sources": [
                    choice_source.display().to_string(),
                    rich_text_source.display().to_string()
                ],
                "path": db_path.display().to_string(),
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            &mut state,
            &[],
        )
        .expect("MCP RAG query builds source program summary");
        let pack = mcp_text_json(&query_result);

        assert!(
            pack["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["kind"] == "graph_summary"
                        && item["chunk_id"]
                            .as_str()
                            .is_some_and(|id| id.contains("mcp:program."))
                        && item["body"].as_str().is_some_and(|body| {
                            body.contains("\"program_rag_index\"")
                                && body.contains("\"source_graph_symbol_kinds\"")
                                && body.contains("\"source_graph_edge_kinds\"")
                                && body.contains("\"graph_symbol_kinds\"")
                                && body.contains("\"graph_edge_kinds\"")
                                && body.contains("\"flow_control_counts\"")
                                && body.contains("\"flow_control_symbols\"")
                        })
                })
            }),
            "MCP source RAG query should include the program-level summary: {pack}"
        );
        assert_eq!(state.rag_context_packs.len(), 1);
        let program_hash = pack["query"]["program_hash"]
            .as_str()
            .expect("program hash");
        let sources = mcp_text_json(
            &agent_mcp_call_debug_source_files(&serde_json::json!({
                "path": db_path.display().to_string(),
                "program_hash": program_hash
            }))
            .expect("MCP source RAG persists source file inventory"),
        );
        assert_eq!(sources["sources"].as_array().map(Vec::len), Some(2));
        let graph = mcp_text_json(
            &agent_mcp_call_debug_graph_inventory(&serde_json::json!({
                "path": db_path.display().to_string(),
                "program_hash": program_hash
            }))
            .expect("MCP source RAG persists graph inventory"),
        );
        assert!(
            graph["symbols"].as_array().is_some_and(|symbols| {
                symbols.iter().any(|symbol| symbol["kind"] == "program")
                    && symbols
                        .iter()
                        .filter(|symbol| symbol["kind"] == "source_file")
                        .count()
                        == 2
            }),
            "MCP source RAG should persist program and source-file graph symbols: {graph}"
        );
        assert!(
            graph["edges"].as_array().is_some_and(|edges| {
                edges
                    .iter()
                    .filter(|edge| edge["edge_kind"] == "contains_source_file")
                    .count()
                    == 2
            }),
            "MCP source RAG should persist program-to-source graph edges: {graph}"
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_uses_local_embedding_debug_store_channel() {
        use crate::app::local_embedding::LocalHashEmbeddingProvider;
        use arcweft_debug_model::embedding::{EmbeddingInput, EmbeddingProvider};

        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-local-embedding-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let program_hash = StableHash::new("blake3:mcp-rag-local-embedding-program").expect("hash");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        let chunk = DebugChunk {
            id: ChunkId::new("chunk:mcp-local-embedding"),
            program_hash: Some(program_hash),
            source_kind: ChunkSourceKind::Documentation,
            source_key: "mcp-local-embedding".to_owned(),
            title: "MCP local embedding context".to_owned(),
            body: "MCP local vector body for Agent RAG".to_owned(),
            content_hash: StableHash::new("blake3:mcp-rag-local-embedding-chunk").expect("hash"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: PrivacyClass::Project,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        };
        store.upsert_chunk(&chunk).expect("chunk");
        let model = EmbeddingModelDescriptor {
            model_id: "fixture-local-hash".to_owned(),
            model_revision: "1".to_owned(),
            dimensions: 8,
        };
        let mut provider = LocalHashEmbeddingProvider::new(model);
        let embeddings = provider
            .embed(&[EmbeddingInput::from_chunk(&chunk)])
            .expect("local embedding");
        for embedding in &embeddings {
            store.upsert_embedding(embedding).expect("embedding");
        }
        drop(store);

        let mut state = AgentMcpState::default();
        let query_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "MCP local vector body",
                "path": db_path.display().to_string(),
                "local_embedding": true,
                "local_embedding_model_id": "fixture-local-hash",
                "local_embedding_model_revision": "1",
                "local_embedding_dimensions": 8,
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            &mut state,
            &[],
        )
        .expect("MCP RAG query uses local embedding channel");
        let pack = mcp_text_json(&query_result);
        assert!(
            pack["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["chunk_id"] == "chunk:mcp-local-embedding"
                        && item["channels"]
                            .as_array()
                            .is_some_and(|channels| channels.contains(&serde_json::json!("vector")))
                })
            }),
            "MCP RAG query should return vector-enriched context: {pack}"
        );
        let store = DebugStore::open(&db_path).expect("debug store reopens");
        let query_id = pack["query"]["query_id"].as_str().expect("query id");
        let audit = store
            .rag_query_audit_with_max_privacy(query_id, PrivacyClass::Project)
            .expect("RAG audit");
        let session_id = audit.session_id.expect("MCP RAG session id");
        assert!(session_id.as_str().starts_with("session.rag.mcp.blake3."));
        assert_eq!(audit.run_id, None);
        let session = store
            .session(&session_id)
            .expect("read MCP RAG session")
            .expect("persisted MCP RAG session");
        assert_eq!(session.profile, "rag");
        assert_eq!(session.transport, "mcp");
        assert_eq!(session.status, DebugSessionStatus::Finished);
        assert_eq!(session.metadata["query_id"], query_id);
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_records_local_embedding_fallback_diagnostic() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-local-embedding-fallback-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let program_hash =
            StableHash::new("blake3:mcp-rag-local-embedding-fallback-program").expect("hash");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        store
            .upsert_chunk(&DebugChunk {
                id: ChunkId::new("chunk:mcp-local-embedding-fallback"),
                program_hash: Some(program_hash),
                source_kind: ChunkSourceKind::Documentation,
                source_key: "mcp-local-embedding-fallback".to_owned(),
                title: "MCP local embedding fallback context".to_owned(),
                body: "MCP local fallback lexical body".to_owned(),
                content_hash: StableHash::new("blake3:mcp-rag-local-embedding-fallback-chunk")
                    .expect("hash"),
                semantic_hash: None,
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Project,
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            })
            .expect("chunk");
        drop(store);

        let mut state = AgentMcpState::default();
        let query_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "MCP local fallback lexical body",
                "path": db_path.display().to_string(),
                "local_embedding": true,
                "local_embedding_model_id": "fixture-local-hash",
                "local_embedding_model_revision": "1",
                "local_embedding_dimensions": 8,
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            &mut state,
            &[],
        )
        .expect("MCP RAG query falls back from missing local embeddings");
        let pack = mcp_text_json(&query_result);
        assert!(
            pack["items"].as_array().is_some_and(|items| {
                items
                    .iter()
                    .any(|item| item["chunk_id"] == "chunk:mcp-local-embedding-fallback")
            }),
            "MCP RAG query should still return lexical context: {pack}"
        );
        let store = DebugStore::open(&db_path).expect("debug store reopens");
        let diagnostics = store
            .diagnostic_search_with_max_privacy(
                "AGENT_RAG_EMBEDDING_FALLBACK",
                4,
                PrivacyClass::Project,
            )
            .expect("diagnostic search");
        assert_eq!(diagnostics.len(), 1);
        assert!(
            diagnostics[0]
                .hit
                .chunk_id
                .as_str()
                .starts_with("diagnostic:agent-mcp-rag-local-embedding-fallback:")
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_reads_debug_store_graph_and_history_channels() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-graph-history-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        seed_mcp_rag_graph_history_debug_store(&db_path);

        let mut state = AgentMcpState::default();
        assert_mcp_rag_debug_store_item(
            &db_path,
            &mut state,
            "offers_choice",
            "graph_summary",
            "graph:1",
            "graph",
        );
        assert_mcp_rag_debug_store_item(
            &db_path,
            &mut state,
            "change-opening-route-fix",
            "history",
            "history:history:opening-route-fix",
            "history",
        );
        assert_mcp_rag_debug_store_item(
            &db_path,
            &mut state,
            "glyph_wobble",
            "diagnostic",
            "diagnostic:diag:mcp-missing-shader",
            "diagnostics",
        );
        assert_mcp_rag_debug_store_item(
            &db_path,
            &mut state,
            "mcp-rich-text-visual",
            "test_result",
            "test_result:test:mcp-visual-regression",
            "diagnostics",
        );
        let _ = std::fs::remove_file(&db_path);
    }

    fn assert_mcp_rag_debug_store_item(
        db_path: &std::path::Path,
        state: &mut AgentMcpState,
        query: &str,
        kind: &str,
        chunk_id: &str,
        channel: &str,
    ) {
        let result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": query,
                "path": db_path.display().to_string(),
                "limit": 4,
                "max_context_bytes": 4096,
                "max_privacy": "project"
            }),
            state,
            &[],
        )
        .unwrap_or_else(|error| panic!("MCP RAG query `{query}` succeeds: {error}"));
        let pack = mcp_text_json(&result);
        assert!(
            pack["items"].as_array().is_some_and(|items| {
                items.iter().any(|item| {
                    item["kind"] == kind
                        && item["chunk_id"] == chunk_id
                        && item["channels"]
                            .as_array()
                            .is_some_and(|channels| channels.contains(&serde_json::json!(channel)))
                })
            }),
            "MCP RAG query should expose {kind} debug-store context: {pack}"
        );
    }

    fn seed_mcp_rag_graph_history_debug_store(path: &std::path::Path) {
        let store = DebugStore::open(path).expect("debug store");
        let program_hash = StableHash::new("blake3:mcp-rag-graph-history-program").expect("hash");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        seed_mcp_rag_graph(&store, &program_hash);
        store
            .upsert_history_entry(&DebugHistoryEntry {
                history_id: "history:opening-route-fix".to_owned(),
                program_hash: Some(program_hash.clone()),
                symbol_id: Some("symbol:choice.listen".to_owned()),
                change_id: "change-opening-route-fix".to_owned(),
                operation_id: Some("op.route".to_owned()),
                ordinal: 9,
                semantic_hash_before: None,
                semantic_hash_after: None,
                summary: "Fixed opening route choice action".to_owned(),
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            })
            .expect("history entry");
        store
            .upsert_diagnostic(&DebugDiagnostic {
                diagnostic_id: "diag:mcp-missing-shader".to_owned(),
                program_hash: Some(program_hash.clone()),
                session_id: None,
                run_id: None,
                sequence: Some(5),
                code: Some("MCP_SHADER_MISSING".to_owned()),
                severity: "error".to_owned(),
                phase: "render".to_owned(),
                message: "missing MCP glyph wobble shader".to_owned(),
                source_path: Some("samples/rich-text-effects-animation.arcw".to_owned()),
                start_byte: Some(20),
                end_byte: Some(40),
                related_ids: vec![PublicId::new("@effect.wobble").expect("public id")],
                payload: serde_json::json!({ "shader": "glyph_wobble" }),
                created_unix_ms: 0,
            })
            .expect("diagnostic");
        store
            .upsert_test_result(&DebugTestResult {
                test_result_id: "test:mcp-visual-regression".to_owned(),
                program_hash: Some(program_hash),
                run_id: None,
                test_id: "mcp-rich-text-visual".to_owned(),
                kind: "visual".to_owned(),
                outcome: "failed".to_owned(),
                duration_millis: Some(88),
                diagnostic_ids: vec!["diag:mcp-missing-shader".to_owned()],
                artifact_refs: vec!["blob:mcp-visual-diff".to_owned()],
                summary: "MCP visual regression detected missing shader output".to_owned(),
                created_unix_ms: 0,
            })
            .expect("test result");
    }

    fn seed_mcp_rag_graph(store: &DebugStore, program_hash: &StableHash) {
        store
            .upsert_graph_symbol(&DebugGraphSymbol {
                symbol_id: "symbol:flow.opening".to_owned(),
                program_hash: program_hash.clone(),
                public_id: Some(PublicId::new("flow.opening").expect("public id")),
                qualified_name: Some("flow.opening".to_owned()),
                kind: "flow".to_owned(),
                type_json: None,
                source_path: None,
                source_content_hash: None,
                start_byte: None,
                end_byte: None,
                semantic_hash: None,
                summary: "Opening flow".to_owned(),
                metadata: BTreeMap::new(),
            })
            .expect("graph source");
        store
            .upsert_graph_symbol(&DebugGraphSymbol {
                symbol_id: "symbol:choice.listen".to_owned(),
                program_hash: program_hash.clone(),
                public_id: Some(PublicId::new("choice.opening.listen").expect("public id")),
                qualified_name: Some("choice.opening.listen".to_owned()),
                kind: "choice".to_owned(),
                type_json: None,
                source_path: None,
                source_content_hash: None,
                start_byte: None,
                end_byte: None,
                semantic_hash: None,
                summary: "Listen choice target".to_owned(),
                metadata: BTreeMap::new(),
            })
            .expect("graph target");
        store
            .upsert_graph_edge(&DebugGraphEdge {
                program_hash: program_hash.clone(),
                from_symbol_id: "symbol:flow.opening".to_owned(),
                to_symbol_id: "symbol:choice.listen".to_owned(),
                edge_kind: "offers_choice".to_owned(),
                weight: 1.25,
                metadata: BTreeMap::new(),
            })
            .expect("graph edge");
    }

    #[test]
    fn agent_mcp_rag_explain_and_context_read_use_persisted_audit() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-rag-audit-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let program_hash = StableHash::new("blake3:mcp-rag-audit-program").expect("hash");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        let chunk = DebugChunk {
            id: ChunkId::new("chunk:mcp-rag-audit"),
            program_hash: Some(program_hash.clone()),
            source_kind: ChunkSourceKind::Documentation,
            source_key: "docs.mcp".to_owned(),
            title: "MCP RAG persisted audit".to_owned(),
            body: "persisted debug store context body".to_owned(),
            content_hash: StableHash::new("blake3:mcp-rag-audit-chunk").expect("hash"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: vec![PublicId::new("@flow.persisted").expect("public id")],
            privacy: PrivacyClass::Public,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        };
        store.upsert_chunk(&chunk).expect("chunk");
        let pack = RagContextPack {
            schema_version: 1,
            query: RagQuery {
                query_id: "rag:mcp:persisted".to_owned(),
                text: "persisted context".to_owned(),
                program_hash,
                roots: vec![PublicId::new("@flow.persisted").expect("root")],
                graph_depth: 1,
                limit: 4,
                max_context_bytes: 4096,
            },
            items: vec![RagContextItem {
                chunk_id: chunk.id.clone(),
                kind: chunk.source_kind,
                title: chunk.title.clone(),
                body: chunk.body.clone(),
                fused_score: 3.5,
                channels: BTreeSet::from([SearchChannel::Lexical]),
                entity_ids: chunk.entity_ids.clone(),
                source_anchor: None,
            }],
            truncated: false,
        };
        store
            .record_rag_context_pack(&pack, None, None, None, "selected", 77)
            .expect("record audit");
        drop(store);

        let state = AgentMcpState::default();
        let explain_result = agent_mcp_call_rag_explain(
            &serde_json::json!({
                "query_id": "rag:mcp:persisted",
                "path": db_path.display().to_string(),
                "max_privacy": "public"
            }),
            &state,
        )
        .expect("persisted explain succeeds");
        let explain = mcp_text_json(&explain_result);
        assert_eq!(explain["source"], serde_json::json!("debug_store"));
        assert_eq!(explain["status"], serde_json::json!("selected"));
        assert_eq!(explain["created_unix_ms"], serde_json::json!(77));
        assert_eq!(
            explain["query"]["text"],
            serde_json::json!("persisted context")
        );
        assert_eq!(
            explain["items"][0]["chunk_id"],
            serde_json::json!("chunk:mcp-rag-audit")
        );
        assert!(explain["items"][0].get("body").is_none());

        let read_result = agent_mcp_call_rag_context_read(
            &serde_json::json!({
                "query_id": "rag:mcp:persisted",
                "chunk_id": "chunk:mcp-rag-audit",
                "path": db_path.display().to_string(),
                "max_privacy": "public",
                "max_bytes": 9
            }),
            &state,
        )
        .expect("persisted context read succeeds");
        let read = mcp_text_json(&read_result);
        assert_eq!(read["body"], serde_json::json!("persisted"));
        assert_eq!(read["truncated"], serde_json::json!(true));
        assert_eq!(read["returned_bytes"], serde_json::json!(9));
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_context_pack_deduplicates_semantic_hashes() {
        fn chunk(id: &str, semantic_hash: &str) -> DebugChunk {
            DebugChunk {
                id: ChunkId::new(id),
                program_hash: None,
                source_kind: ChunkSourceKind::Source,
                source_key: id.to_owned(),
                title: id.to_owned(),
                body: format!("alpha context {id}"),
                content_hash: StableHash::new(format!("blake3:{id}")).expect("hash"),
                semantic_hash: Some(StableHash::new(semantic_hash).expect("semantic hash")),
                source_anchor: None,
                entity_ids: Vec::new(),
                privacy: PrivacyClass::Project,
                metadata: BTreeMap::new(),
                created_unix_ms: 0,
            }
        }
        let candidates = vec![
            AgentMcpRagCandidate {
                chunk: chunk("chunk:a", "blake3:same-semantic"),
                preferred_channel: SearchChannel::Lexical,
            },
            AgentMcpRagCandidate {
                chunk: chunk("chunk:b", "blake3:same-semantic"),
                preferred_channel: SearchChannel::Lexical,
            },
            AgentMcpRagCandidate {
                chunk: chunk("chunk:c", "blake3:unique-semantic"),
                preferred_channel: SearchChannel::Lexical,
            },
        ];
        let query = RagQuery {
            query_id: "query.mcp.semantic.dedupe".to_owned(),
            text: "alpha".to_owned(),
            program_hash: StableHash::new("blake3:mcp-semantic-dedupe").expect("hash"),
            roots: Vec::new(),
            graph_depth: 1,
            limit: 2,
            max_context_bytes: 4096,
        };

        let pack = agent_mcp_rag_context_pack_from_candidates(query, &candidates, 2, 4096);

        let ids = pack
            .items
            .iter()
            .map(|item| item.chunk_id.as_str())
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["chunk:a", "chunk:c"]);
    }

    #[cfg(feature = "agent-repl")]
    #[test]
    fn agent_repl_reedline_validator_uses_fragment_completion() {
        assert!(matches!(
            reedline::Validator::validate(&AgentReplReedlineValidator, "let value ="),
            reedline::ValidationResult::Incomplete
        ));
        assert!(matches!(
            reedline::Validator::validate(&AgentReplReedlineValidator, "let value = 1u32"),
            reedline::ValidationResult::Complete
        ));
    }

    #[cfg(feature = "agent-repl")]
    #[test]
    fn agent_repl_reedline_completer_uses_tooling_candidates() {
        let mut completer = AgentReplReedlineCompleter {
            context: AgentReplCompletionContext::default(),
        };

        let suggestions = reedline::Completer::complete(&mut completer, "state_", 6);

        assert!(suggestions.iter().any(|suggestion| {
            suggestion.value == "state_path"
                && suggestion.display_value() == "state_path"
                && suggestion.span.start == 0
                && suggestion.span.end == 6
        }));
    }

    #[test]
    fn agent_mcp_debug_repl_cells_reads_persisted_cells() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-repl-cells-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let session_id = SessionId::new("session.mcp.repl").expect("session id");
        store
            .start_session(&session_id, None, "repl", "mcp", 0)
            .expect("session");
        for (ordinal, source) in [(1, "let observed = observe()"), (2, ":bindings")] {
            store
                .upsert_repl_cell(&DebugReplCell {
                    cell_id: format!("repl:{}:{ordinal}", session_id.as_str()),
                    session_id: session_id.clone(),
                    run_id: None,
                    ordinal,
                    source: source.to_owned(),
                    source_hash: StableHash::new(format!("blake3:mcp-repl-cell-{ordinal}"))
                        .expect("hash"),
                    status: "ok".to_owned(),
                    inferred_type: None,
                    display: Some(serde_json::json!({ "ordinal": ordinal })),
                    partially_effectful: ordinal == 1,
                    diagnostic_ids: vec![format!("diag.{ordinal}")],
                    created_unix_ms: ordinal,
                })
                .expect("repl cell");
        }
        drop(store);

        let result = agent_mcp_call_debug_repl_cells(&serde_json::json!({
            "path": db_path.display().to_string(),
            "session_id": "session.mcp.repl",
            "limit": 1
        }))
        .expect("debug REPL cells succeeds");
        let value = mcp_text_json(&result);
        assert_eq!(value["session_id"], serde_json::json!("session.mcp.repl"));
        assert_eq!(value["limit"], serde_json::json!(1));
        assert_eq!(value["cells"].as_array().expect("cells").len(), 1);
        assert_eq!(
            value["cells"][0]["cell_id"],
            serde_json::json!("repl:session.mcp.repl:1")
        );
        assert_eq!(
            value["cells"][0]["source"],
            serde_json::json!("let observed = observe()")
        );
        assert_eq!(
            value["cells"][0]["display"]["ordinal"],
            serde_json::json!(1)
        );
        assert_eq!(
            value["cells"][0]["partially_effectful"],
            serde_json::json!(true)
        );
        assert_eq!(
            value["cells"][0]["diagnostic_ids"][0],
            serde_json::json!("diag.1")
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_debug_source_files_reads_program_inventory() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-source-files-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let program_hash = StableHash::new("blake3:mcp-source-program").expect("program hash");
        let content_hash = StableHash::new("blake3:mcp-source-content").expect("content hash");
        let store = DebugStore::open(&db_path).expect("debug store");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        store
            .upsert_source_file(&DebugSourceFile {
                program_hash: program_hash.clone(),
                path: "samples/agent-script/native-choice-dispatch.arcw".to_owned(),
                language: "arcw".to_owned(),
                content_hash: content_hash.clone(),
                byte_len: 1234,
                metadata: BTreeMap::from([("extension".to_owned(), serde_json::json!("arcw"))]),
            })
            .expect("source file");
        drop(store);

        let result = agent_mcp_call_debug_source_files(&serde_json::json!({
            "path": db_path.display().to_string(),
            "program_hash": program_hash.as_str()
        }))
        .expect("debug source files succeeds");
        let value = mcp_text_json(&result);
        assert_eq!(
            value["program_hash"],
            serde_json::json!(program_hash.as_str())
        );
        assert_eq!(value["max_privacy"], serde_json::json!("project"));
        let sources = value["sources"].as_array().expect("sources");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            sources[0]["path"],
            serde_json::json!("samples/agent-script/native-choice-dispatch.arcw")
        );
        assert_eq!(sources[0]["language"], serde_json::json!("arcw"));
        assert_eq!(
            sources[0]["content_hash"],
            serde_json::json!(content_hash.as_str())
        );
        assert_eq!(sources[0]["byte_len"], serde_json::json!(1234));
        assert_eq!(
            sources[0]["metadata"]["extension"],
            serde_json::json!("arcw")
        );

        let public_result = agent_mcp_call_debug_source_files(&serde_json::json!({
            "path": db_path.display().to_string(),
            "program_hash": program_hash.as_str(),
            "max_privacy": "public"
        }))
        .expect("public debug source files succeeds");
        let public_value = mcp_text_json(&public_result);
        assert_eq!(public_value["max_privacy"], serde_json::json!("public"));
        assert_eq!(
            public_value["sources"].as_array().map(Vec::len),
            Some(0),
            "project-private source inventory should be omitted at public ceiling"
        );
        let _ = std::fs::remove_file(&db_path);
    }

    fn seed_mcp_debug_graph_inventory(db_path: &std::path::Path) -> (StableHash, StableHash) {
        let program_hash = StableHash::new("blake3:mcp-graph-program").expect("program hash");
        let content_hash = StableHash::new("blake3:mcp-graph-content").expect("content hash");
        let store = DebugStore::open(db_path).expect("debug store");
        store
            .upsert_program(&program_hash, None, Some("."), 0)
            .expect("program");
        store
            .upsert_source_file(&DebugSourceFile {
                program_hash: program_hash.clone(),
                path: "samples/agent-script/native-choice-dispatch.arcw".to_owned(),
                language: "arcw".to_owned(),
                content_hash: content_hash.clone(),
                byte_len: 1234,
                metadata: BTreeMap::new(),
            })
            .expect("source file");
        store
            .upsert_graph_symbol(&DebugGraphSymbol {
                symbol_id: "symbol:flow.opening".to_owned(),
                program_hash: program_hash.clone(),
                public_id: Some(PublicId::new("flow.opening").expect("public id")),
                qualified_name: Some("flow.opening".to_owned()),
                kind: "flow".to_owned(),
                type_json: Some(serde_json::json!({"returns": "String"})),
                source_path: Some("samples/agent-script/native-choice-dispatch.arcw".to_owned()),
                source_content_hash: Some(content_hash.clone()),
                start_byte: Some(8),
                end_byte: Some(40),
                semantic_hash: Some(StableHash::new("blake3:mcp-graph-symbol").expect("hash")),
                summary: "Opening flow".to_owned(),
                metadata: BTreeMap::from([("role".to_owned(), serde_json::json!("entry"))]),
            })
            .expect("flow symbol");
        store
            .upsert_graph_symbol(&DebugGraphSymbol {
                symbol_id: "symbol:choice.listen".to_owned(),
                program_hash: program_hash.clone(),
                public_id: Some(PublicId::new("choice.listen").expect("public id")),
                qualified_name: Some("choice.listen".to_owned()),
                kind: "choice".to_owned(),
                type_json: None,
                source_path: None,
                source_content_hash: None,
                start_byte: None,
                end_byte: None,
                semantic_hash: None,
                summary: "Listen choice".to_owned(),
                metadata: BTreeMap::new(),
            })
            .expect("choice symbol");
        store
            .upsert_graph_edge(&DebugGraphEdge {
                program_hash: program_hash.clone(),
                from_symbol_id: "symbol:flow.opening".to_owned(),
                to_symbol_id: "symbol:choice.listen".to_owned(),
                edge_kind: "offers_choice".to_owned(),
                weight: 0.75,
                metadata: BTreeMap::from([("via".to_owned(), serde_json::json!("test"))]),
            })
            .expect("edge");
        drop(store);
        (program_hash, content_hash)
    }

    #[test]
    fn agent_mcp_debug_graph_inventory_reads_program_symbols_and_edges() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-graph-inventory-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let (program_hash, content_hash) = seed_mcp_debug_graph_inventory(&db_path);

        let result = agent_mcp_call_debug_graph_inventory(&serde_json::json!({
            "path": db_path.display().to_string(),
            "program_hash": program_hash.as_str()
        }))
        .expect("debug graph inventory succeeds");
        let value = mcp_text_json(&result);
        assert_eq!(
            value["program_hash"],
            serde_json::json!(program_hash.as_str())
        );
        assert_eq!(value["max_privacy"], serde_json::json!("project"));
        assert_eq!(value["symbol_count"], serde_json::json!(2));
        assert_eq!(value["edge_count"], serde_json::json!(1));
        assert!(
            value["symbols"]
                .as_array()
                .expect("symbols")
                .iter()
                .any(|symbol| {
                    symbol["symbol_id"] == "symbol:flow.opening"
                        && symbol["source_content_hash"] == content_hash.as_str()
                        && symbol["metadata"]["role"] == "entry"
                })
        );
        assert_eq!(
            value["edges"][0]["edge_kind"],
            serde_json::json!("offers_choice")
        );
        assert_eq!(
            value["edges"][0]["metadata"]["via"],
            serde_json::json!("test")
        );

        let public_result = agent_mcp_call_debug_graph_inventory(&serde_json::json!({
            "path": db_path.display().to_string(),
            "program_hash": program_hash.as_str(),
            "max_privacy": "public"
        }))
        .expect("public debug graph inventory succeeds");
        let public_value = mcp_text_json(&public_result);
        assert_eq!(public_value["max_privacy"], serde_json::json!("public"));
        assert_eq!(public_value["symbol_count"], serde_json::json!(0));
        assert_eq!(public_value["edge_count"], serde_json::json!(0));
        assert_eq!(
            public_value["symbols"].as_array().map(Vec::len),
            Some(0),
            "project-private graph symbols should be omitted at public ceiling"
        );
        assert_eq!(
            public_value["edges"].as_array().map(Vec::len),
            Some(0),
            "project-private graph edges should be omitted at public ceiling"
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_debug_close_stale_sessions_abandons_running_sessions() {
        let db_path = std::env::temp_dir().join(format!(
            "arcweft-agent-mcp-close-stale-sessions-{}.sqlite3",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&db_path);
        let store = DebugStore::open(&db_path).expect("debug store");
        let stale = SessionId::new("session.mcp.stale").expect("stale session id");
        let fresh = SessionId::new("session.mcp.fresh").expect("fresh session id");
        store
            .start_session(&stale, None, "agent", "mcp", 0)
            .expect("stale session");
        store
            .start_session(&fresh, None, "agent", "mcp", i64::MAX / 2)
            .expect("fresh session");
        drop(store);

        let dry_run = agent_mcp_call_debug_close_stale_sessions(&serde_json::json!({
            "path": db_path.display().to_string(),
            "stale_after_millis": 1,
            "reason": "mcp-test-stale",
            "dry_run": true
        }))
        .expect("debug close stale dry-run succeeds");
        let dry_value = mcp_text_json(&dry_run);
        assert_eq!(dry_value["dry_run"], serde_json::json!(true));
        assert_eq!(
            dry_value["matched_sessions"][0]["session_id"],
            serde_json::json!("session.mcp.stale")
        );
        assert!(
            dry_value["closed_sessions"]
                .as_array()
                .expect("closed sessions")
                .is_empty()
        );

        let result = agent_mcp_call_debug_close_stale_sessions(&serde_json::json!({
            "path": db_path.display().to_string(),
            "stale_after_millis": 1,
            "reason": "mcp-test-stale"
        }))
        .expect("debug close stale succeeds");
        let value = mcp_text_json(&result);
        assert_eq!(value["dry_run"], serde_json::json!(false));
        assert_eq!(
            value["closed_sessions"][0]["status"],
            serde_json::json!("abandoned")
        );
        assert_eq!(
            value["closed_sessions"][0]["metadata"]["lifecycle_policy"]["reason"],
            serde_json::json!("mcp-test-stale")
        );

        let store = DebugStore::open(&db_path).expect("debug store reopens");
        assert_eq!(
            store
                .session(&stale)
                .expect("read stale")
                .expect("stale exists")
                .status,
            DebugSessionStatus::Abandoned
        );
        assert_eq!(
            store
                .session(&fresh)
                .expect("read fresh")
                .expect("fresh exists")
                .status,
            DebugSessionStatus::Running
        );
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn agent_mcp_rag_query_enforces_max_privacy() {
        let mut state = AgentMcpState {
            trace_resources: vec![AgentResource {
                uri: "arcweft://run/run.test/trace.arcwx".to_owned(),
                kind: arcweft_agent_protocol::AgentResourceKind::Trace,
                mime_type: "application/vnd.arcweft.agent-trace+json".to_owned(),
                hash: "trace:test".to_owned(),
                image: None,
                body: AgentResourceBody::Json(serde_json::json!([
                    {
                        "kind": "diagnostic_emitted",
                        "privacy_class": "public",
                        "payload": { "message": "public route note" }
                    },
                    {
                        "kind": "diagnostic_emitted",
                        "privacy_class": "secret",
                        "payload": { "message": "secret token should not appear" }
                    }
                ])),
            }],
            ..AgentMcpState::default()
        };

        let rag_result = agent_mcp_call_rag_query(
            &serde_json::json!({
                "query": "diagnostic_emitted",
                "max_privacy": "public",
                "limit": 8
            }),
            &mut state,
            &[],
        )
        .expect("RAG query succeeds");
        let pack = mcp_text_json(&rag_result);
        let items = pack["items"].as_array().expect("items array");

        assert_eq!(items.len(), 1);
        assert!(
            items[0]["body"]
                .as_str()
                .expect("body string")
                .contains("public route note")
        );
        assert!(
            !serde_json::to_string(&pack)
                .expect("pack serializes")
                .contains("secret token")
        );
    }

    fn mcp_text_json(result: &McpCallToolResult) -> serde_json::Value {
        let [McpContentBlock::Text { text }] = result.content.as_slice() else {
            panic!("expected one text block");
        };
        serde_json::from_str(text).expect("MCP text block is JSON")
    }

    #[test]
    fn uri_capture_request_preserves_report_capture_time() {
        let report = test_agent_observation_report(Some(2000));
        let request = agent_capture_request_from_uri(
            &report,
            "arcweft://session/cli/frame/3/object.object.dialogue.0.0.mask.rgba",
        )
        .expect("object mask URI should parse");

        assert_eq!(request.capture_step, 3);
        assert_seconds_close(request.capture_time_seconds, 2.0);
        assert_eq!(request.image_kind, AgentObserveImageKind::RawRgba);
        assert_eq!(request.capture_kind, AgentObserveCaptureKind::Mask);
        let AgentCaptureScope::Object(object_id) = request.scope else {
            panic!("request should target an object scope");
        };
        assert_eq!(object_id, "object.dialogue.0.0");
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
            parent_id: None,
            entity: None,
            layer: "ui".to_owned(),
            role: "panel".to_owned(),
            visible: true,
            enabled: true,
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
            object_layer: None,
            object_depth: None,
            text: None,
            rich_text_ref: None,
            content: AgentObservedObjectContent::RichText {
                frame: Box::new(test_line_display_frame()),
            },
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
    fn agent_hit_test_reports_generic_image_object_bbox_hit() {
        let mut object = test_observed_object("object.image.logo", 10, 20, 30, 40);
        object.role = "image".to_owned();
        object.object_layer = Some("hud.foreground".to_owned());
        object.object_depth = Some(2500);
        let mut report = test_agent_observation_report(None);
        report.objects = vec![object];

        let hit_report = agent_hit_test_report(&report, 20, 30);

        assert_eq!(
            hit_report.top_object_id.as_deref(),
            Some("object.image.logo")
        );
        assert_eq!(hit_report.hits.len(), 1);
        assert_eq!(hit_report.hits[0].layer, "hud.foreground");
        assert_eq!(hit_report.hits[0].role, "image");
        assert_eq!(hit_report.hits[0].depth, Some(2500));
        assert_eq!(hit_report.hits[0].region.kind, AgentHitRegionKind::Object);
        assert_eq!(
            hit_report.hits[0].object.object_layer.as_deref(),
            Some("hud.foreground")
        );
        assert!(hit_report.hits[0].rich_text_ref.is_none());
    }

    #[test]
    fn agent_hit_test_uses_image_object_polygon_inside_bbox() {
        let mut object = test_observed_object("object.image.diamond", 0, 0, 100, 100);
        object.role = "image".to_owned();
        object.polygon = vec![
            AgentPoint { x: 50, y: 0 },
            AgentPoint { x: 100, y: 50 },
            AgentPoint { x: 50, y: 100 },
            AgentPoint { x: 0, y: 50 },
        ];
        let mut report = test_agent_observation_report(None);
        report.objects = vec![object];

        assert!(
            agent_hit_test_report(&report, 50, 50)
                .top_object_id
                .is_some()
        );
        assert!(agent_hit_test_report(&report, 5, 5).top_object_id.is_none());
    }

    #[test]
    fn agent_image_object_mask_capture_uses_observed_geometry_without_textbox() {
        let mut object = test_observed_object("object.image.logo", 10, 20, 30, 40);
        object.entity = Some("ui.image.7".to_owned());
        object.layer = "hud".to_owned();
        object.role = "image".to_owned();
        object.object_layer = Some("hud".to_owned());
        object.content = AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
            source: "ui.image.7".to_owned(),
            object: None,
            target: None,
            asset: Some("asset.ui.logo".to_owned()),
            frame_index: Some(0),
            local_time_millis: Some(0),
            opacity_milli: None,
            fit: None,
            alignment: None,
            transform: None,
            intrinsic_width: Some(30),
            intrinsic_height: Some(40),
            actions: Vec::new(),
            params: BTreeMap::new(),
            proxies: Vec::new(),
        }));
        let mut report = test_agent_observation_report(None);
        report.objects = vec![object];

        let result = agent_native_image_object_geometry_capture(
            &report,
            &AgentCaptureReadRequest {
                uri: "arcweft://session/cli/frame/3/object.object.image.logo.mask.rgba".to_owned(),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Mask,
                scope: AgentCaptureScope::Object("object.image.logo".to_owned()),
                page: 0,
                capture_step: 3,
                capture_time_seconds: 0.0,
            },
        )
        .unwrap()
        .expect("image object mask capture is produced from observed geometry");

        assert_eq!(
            result.image.composition,
            AgentImageComposition::MaskAttachment
        );
        assert_eq!(result.image.width, 30);
        assert_eq!(result.image.height, 40);
        assert_eq!(
            result.image.crop_origin,
            Some(AgentImageCropOrigin {
                space: AgentCoordinateSpace::Viewport,
                x: 10,
                y: 20,
            })
        );
        assert_eq!(
            result.image.content_bbox,
            Some(AgentImageContentBBox {
                x: 0,
                y: 0,
                width: 30,
                height: 40,
            })
        );
        assert_eq!(result.image.content_pixels, Some(1200));
        assert!(
            result
                .bytes
                .chunks_exact(4)
                .all(|pixel| pixel == [255, 255, 255, 255])
        );
    }

    #[test]
    fn agent_image_object_color_capture_requires_image_pixels() {
        let mut object = test_observed_object("object.image.logo", 10, 20, 30, 40);
        object.role = "image".to_owned();
        object.content = AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
            source: "ui.image.7".to_owned(),
            object: None,
            target: None,
            asset: None,
            frame_index: Some(0),
            local_time_millis: Some(0),
            opacity_milli: None,
            fit: None,
            alignment: None,
            transform: None,
            intrinsic_width: Some(30),
            intrinsic_height: Some(40),
            actions: Vec::new(),
            params: BTreeMap::new(),
            proxies: Vec::new(),
        }));
        let mut report = test_agent_observation_report(None);
        report.objects = vec![object];

        let result = agent_native_image_object_geometry_capture(
            &report,
            &AgentCaptureReadRequest {
                uri: "arcweft://session/cli/frame/3/object.object.image.logo.rgba".to_owned(),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Color,
                scope: AgentCaptureScope::Object("object.image.logo".to_owned()),
                page: 0,
                capture_step: 3,
                capture_time_seconds: 0.0,
            },
        );

        assert!(result.is_err());
    }

    #[test]
    fn agent_image_object_color_capture_uses_stored_native_image_frame() {
        let mut object = test_observed_object("object.image.logo", 10, 20, 2, 2);
        object.role = "image".to_owned();
        object.content = AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
            source: "ui.image.7".to_owned(),
            object: None,
            target: None,
            asset: None,
            frame_index: Some(0),
            local_time_millis: Some(0),
            opacity_milli: None,
            fit: None,
            alignment: None,
            transform: None,
            intrinsic_width: Some(2),
            intrinsic_height: Some(2),
            actions: Vec::new(),
            params: BTreeMap::new(),
            proxies: Vec::new(),
        }));
        let mut report = test_agent_observation_report(None);
        report.objects = vec![object];
        let mut frames = AgentImageFrameStore::default();
        frames.insert(
            "object.image.logo",
            2,
            2,
            vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 255],
        );
        let mut native_session =
            arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();

        let result = agent_native_capture_image_with_frame_store(
            &report,
            &AgentCaptureReadRequest {
                uri: "arcweft://session/cli/frame/3/object.object.image.logo.rgba".to_owned(),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Color,
                scope: AgentCaptureScope::Object("object.image.logo".to_owned()),
                page: 0,
                capture_step: 3,
                capture_time_seconds: 0.0,
            },
            &mut native_session,
            &frames,
        )
        .unwrap();

        assert_eq!(
            result.image.composition,
            AgentImageComposition::FramebufferCrop
        );
        assert_eq!(result.image.width, 2);
        assert_eq!(result.image.height, 2);
        assert_eq!(result.bytes.len(), 16);
        assert_eq!(&result.bytes[0..4], &[255, 0, 0, 255]);
        assert_eq!(&result.bytes[4..8], &[0, 0, 0, 0]);
        assert_eq!(&result.bytes[8..12], &[0, 255, 0, 255]);
        assert_eq!(&result.bytes[12..16], &[0, 0, 255, 255]);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn agent_ui_image_items_become_typed_image_objects_with_active_frame() {
        use arcweft_id::PublicId;
        use arcweft_image::{
            DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
        };
        use arcweft_presentation::layer::{
            LayerId, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
        };
        use arcweft_runtime_host::UiFrameCommitBuilder;
        use arcweft_ui::{
            DisplayList, FragmentKind, ImageId, ImagePlayback, LayoutBox, LayoutLength,
            LayoutPoint, LayoutResults, LayoutSize, LayoutTree, NodeKey, StyleId, UiImageSource,
            UiImageSourceTable, UiLayerOutput, UiSemanticFragment, ViewFragmentBuilder,
        };

        fn public_id(value: &str) -> PublicId {
            PublicId::try_new(value).unwrap()
        }

        fn layer_id(value: &str) -> LayerId {
            LayerId::new(public_id(value))
        }

        let dimensions = ImageDimensions::new(2, 1).unwrap();
        let image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            vec![
                DecodedImageFrame::new(0, dimensions, 100, vec![0, 0, 0, 255, 1, 1, 1, 255])
                    .unwrap(),
                DecodedImageFrame::new(1, dimensions, 100, vec![2, 2, 2, 255, 3, 3, 3, 255])
                    .unwrap(),
            ],
        )
        .unwrap();
        let mut image_sources = UiImageSourceTable::default();
        image_sources
            .insert_with_id(
                ImageId(7),
                UiImageSource::new(image).with_playback(ImagePlayback::new(0)),
            )
            .unwrap();

        let root = layer_id("layer.root");
        let hud = layer_id("layer.hud");
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            LayerOrder {
                phase: RenderPhase::Background,
                z: 0,
                stable_index: 0,
            },
        ));
        layers
            .insert(
                LayerNode::new(
                    hud.clone(),
                    LayerKind::GameUi,
                    LayerOrder {
                        phase: RenderPhase::GameUi,
                        z: 0,
                        stable_index: 0,
                    },
                )
                .with_parent(root),
            )
            .unwrap();

        let mut fragment = ViewFragmentBuilder::default();
        let node = fragment
            .push_node(
                NodeKey(1),
                FragmentKind::Image(ImageId(7)),
                StyleId(0),
                &[],
                &[],
                None,
            )
            .unwrap();
        let fragment = fragment.finish();
        let layout_tree = LayoutTree::from_fragment(&fragment).unwrap();
        let mut layouts = LayoutResults::new(&layout_tree);
        layouts
            .set(
                node,
                LayoutBox::new(
                    LayoutPoint::new(LayoutLength(10_500), LayoutLength::px(20)),
                    LayoutSize::new(LayoutLength(20_100), LayoutLength::px(10)),
                ),
            )
            .unwrap();
        let output = UiLayerOutput::new(
            DisplayList::from_fragment(&fragment, &layouts).unwrap(),
            UiSemanticFragment::default(),
        );
        let mut builder = UiFrameCommitBuilder::new(&layers);
        builder.push_layer(hud, output).unwrap();
        let commit = builder.finish();

        let observation = agent_image_observation_from_ui_frame(
            "cli",
            4,
            &AgentViewport {
                width: 320,
                height: 180,
                scale: 1.0,
            },
            &commit,
            &image_sources,
            150,
        );

        let objects = &observation.objects;
        assert_eq!(objects.len(), 1);
        let object = &objects[0];
        assert_eq!(object.role, "image");
        assert_eq!(object.layer, "layer.hud");
        assert_eq!(object.bbox.x, 10);
        assert_eq!(object.bbox.y, 20);
        assert_eq!(object.bbox.width, 21);
        assert_eq!(object.bbox.height, 10);
        assert!(object.rich_text_ref.is_none());
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("UI image item should become image object content");
        };
        assert_eq!(content.source, "ui.image.7");
        assert_eq!(content.frame_index, Some(1));
        assert_eq!(content.local_time_millis, Some(150));
        assert_eq!(content.intrinsic_width, Some(2));
        assert_eq!(content.intrinsic_height, Some(1));

        let object_only_bridge = agent_image_objects_from_ui_frame(
            "cli",
            4,
            &AgentViewport {
                width: 320,
                height: 180,
                scale: 1.0,
            },
            &commit,
            &image_sources,
            150,
        );
        assert_eq!(object_only_bridge, *objects);

        let stored_frame = observation.image_frames.get(&object.id).unwrap();
        assert_eq!(stored_frame.width, 2);
        assert_eq!(stored_frame.height, 1);
        assert_eq!(stored_frame.rgba, vec![2, 2, 2, 255, 3, 3, 3, 255]);

        let mut report = test_agent_observation_report(None);
        report.viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let mut capture_object = object.clone();
        capture_object.bbox.width = 2;
        capture_object.bbox.height = 1;
        capture_object.polygon = capture_object.bbox.polygon();
        report.objects = vec![capture_object];
        let mut native_session =
            arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();
        let result = agent_native_capture_image_with_frame_store(
            &report,
            &AgentCaptureReadRequest {
                uri: format!("arcweft://session/cli/frame/4/object.{}.rgba", object.id),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Color,
                scope: AgentCaptureScope::Object(object.id.clone()),
                page: 0,
                capture_step: 4,
                capture_time_seconds: 0.15,
            },
            &mut native_session,
            &observation.image_frames,
        )
        .unwrap();
        assert_eq!(result.image.width, 2);
        assert_eq!(result.image.height, 1);
        assert_eq!(result.bytes, vec![2, 2, 2, 255, 3, 3, 3, 255]);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn agent_captures_presentation_image_objects_lowered_through_ui_frame() {
        use arcweft_id::PublicId;
        use arcweft_image::{
            DecodedImage, DecodedImageFrame, ImageDimensions, ImageFormat, ImageRepetition,
        };
        use arcweft_presentation::hit::HitRect;
        use arcweft_presentation::image::{
            ImageAssetRef, ImageObjectId, ImageObjectPlayback, ImageObjectTransform,
            ImagePresentationObject,
        };
        use arcweft_presentation::input::InteractionTarget;
        use arcweft_presentation::layer::{
            LayerId, LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase,
        };
        use arcweft_runtime_host::UiFrameCommitBuilder;
        use arcweft_ui::{UiImagePresentationFrame, UiImagePresentationInput};

        fn public_id(value: &str) -> PublicId {
            PublicId::try_new(value).unwrap()
        }

        let dimensions = ImageDimensions::new(2, 1).unwrap();
        let image = DecodedImage::new(
            ImageFormat::Gif,
            dimensions,
            ImageRepetition::Infinite,
            vec![
                DecodedImageFrame::new(0, dimensions, 100, vec![8, 8, 8, 255, 9, 9, 9, 255])
                    .unwrap(),
                DecodedImageFrame::new(1, dimensions, 100, vec![30, 40, 50, 255, 60, 70, 80, 255])
                    .unwrap(),
            ],
        )
        .unwrap();
        let root = LayerId::new(public_id("layer.root"));
        let hud = LayerId::new(public_id("layer.hud"));
        let mut layers = LayerTree::new(LayerNode::new(
            root.clone(),
            LayerKind::Root,
            LayerOrder {
                phase: RenderPhase::Background,
                z: 0,
                stable_index: 0,
            },
        ));
        layers
            .insert(
                LayerNode::new(
                    hud.clone(),
                    LayerKind::GameUi,
                    LayerOrder {
                        phase: RenderPhase::GameUi,
                        z: 0,
                        stable_index: 0,
                    },
                )
                .with_parent(root),
            )
            .unwrap();
        let object = ImagePresentationObject::new(
            ImageObjectId::new(public_id("image.logo")),
            ImageAssetRef::new(public_id("asset.logo")),
            hud,
            InteractionTarget::new(public_id("target.logo")),
            HitRect::new(10.0, 20.0, 2.0, 1.0),
        )
        .with_opacity_milli(750)
        .with_transform(ImageObjectTransform::translation_milli(5_000, 6_000))
        .with_playback(ImageObjectPlayback::new(0).pinned_local_time(150));
        let frame =
            UiImagePresentationFrame::from_inputs([UiImagePresentationInput::new(object, image)])
                .unwrap();
        let (outputs, image_sources) = frame.into_parts();
        let mut builder = UiFrameCommitBuilder::new(&layers);
        for (layer, output) in outputs {
            builder.push_layer(layer, output).unwrap();
        }
        let commit = builder.finish();

        let observation = agent_image_observation_from_ui_frame(
            "cli",
            5,
            &AgentViewport {
                width: 64,
                height: 64,
                scale: 1.0,
            },
            &commit,
            &image_sources,
            0,
        );

        assert_eq!(observation.objects.len(), 1);
        let object = &observation.objects[0];
        assert_eq!(object.bbox.x, 15);
        assert_eq!(object.bbox.y, 26);
        assert_eq!(object.bbox.width, 2);
        assert_eq!(object.bbox.height, 1);
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("presentation image should become Agent image content");
        };
        assert_eq!(content.frame_index, Some(1));
        assert_eq!(content.local_time_millis, Some(150));
        assert_eq!(content.opacity_milli, Some(750));
        assert_eq!(
            content.transform,
            Some(AgentImageTransform {
                m11_milli: 1_000,
                m12_milli: 0,
                m21_milli: 0,
                m22_milli: 1_000,
                tx_milli: 5_000,
                ty_milli: 6_000,
            })
        );
        let mut report = test_agent_observation_report(None);
        report.viewport = AgentViewport {
            width: 64,
            height: 64,
            scale: 1.0,
        };
        report.objects = observation.objects.clone();
        let mut native_session =
            arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();
        let result = agent_native_capture_image_with_frame_store(
            &report,
            &AgentCaptureReadRequest {
                uri: format!("arcweft://session/cli/frame/5/object.{}.rgba", object.id),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Color,
                scope: AgentCaptureScope::Object(object.id.clone()),
                page: 0,
                capture_step: 5,
                capture_time_seconds: 0.0,
            },
            &mut native_session,
            &observation.image_frames,
        )
        .unwrap();
        assert_eq!(result.bytes, vec![25, 34, 43, 191, 52, 60, 69, 191]);
    }

    #[test]
    fn agent_lowers_runtime_background_call_into_image_observation() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[
                RuntimeCall {
                    callee: "bg".to_owned(),
                    args: vec!["@asset.bg.missing".to_owned()],
                },
                RuntimeCall {
                    callee: "bg".to_owned(),
                    args: vec!["@asset.bg.room".to_owned()],
                },
            ],
            0.06,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code.as_deref(),
            Some("image_asset_unavailable")
        );
        assert_eq!(observation.objects.len(), 1);
        let object = &observation.objects[0];
        assert_eq!(object.id, "object.image.layer.background.0.0");
        assert_eq!(object.layer, "layer.background");
        assert_eq!(object.bbox.width, 320);
        assert_eq!(object.bbox.height, 180);
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("background call should become Agent image content");
        };
        assert_eq!(content.source, "ui.image.0");
        assert_eq!(content.frame_index, Some(0));
        assert_eq!(content.local_time_millis, Some(60));
        assert_eq!(content.intrinsic_width, Some(2));
        assert_eq!(content.intrinsic_height, Some(1));

        let stored_frame = observation.image_frames.get(&object.id).unwrap();
        assert_eq!(stored_frame.width, 2);
        assert_eq!(stored_frame.height, 1);
        assert_eq!(stored_frame.rgba, vec![30, 80, 200, 255, 220, 60, 40, 255]);
    }

    #[test]
    fn agent_background_call_accepts_fit_alignment_and_opacity() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[RuntimeCall {
                callee: "bg".to_owned(),
                args: vec![
                    "@asset.bg.poster".to_owned(),
                    "fit = intrinsic".to_owned(),
                    "alignment.x = right".to_owned(),
                    "alignment.y = bottom".to_owned(),
                    "opacity = 0.5".to_owned(),
                ],
            }],
            0.0,
        );

        assert!(diagnostics.is_empty());
        let object = &observation.objects[0];
        assert_eq!(object.bbox.x, 318);
        assert_eq!(object.bbox.y, 179);
        assert_eq!(object.bbox.width, 2);
        assert_eq!(object.bbox.height, 1);
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("background call should become Agent image content");
        };
        assert_eq!(content.fit, Some(AgentImageFit::Intrinsic));
        assert_eq!(
            content.alignment,
            Some(AgentImageAlignment {
                x_milli: 1_000,
                y_milli: 1_000,
            })
        );
        assert_eq!(content.opacity_milli, Some(500));
    }

    #[test]
    fn agent_runtime_background_call_uses_capture_time_for_animated_images() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let call = RuntimeCall {
            callee: "bg".to_owned(),
            args: vec!["@asset.bg.pulse".to_owned()],
        };
        let (first, first_diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            std::slice::from_ref(&call),
            0.05,
        );
        let (second, second_diagnostics) =
            agent_runtime_presentation_image_observation(&source, 2, &viewport, &[call], 0.15);

        assert!(first_diagnostics.is_empty());
        assert!(second_diagnostics.is_empty());
        let first_object = &first.objects[0];
        let second_object = &second.objects[0];
        let AgentObservedObjectContent::Image(first_content) = &first_object.content else {
            panic!("animated background should become Agent image content");
        };
        let AgentObservedObjectContent::Image(second_content) = &second_object.content else {
            panic!("animated background should become Agent image content");
        };
        assert_eq!(first_content.frame_index, Some(0));
        assert_eq!(first_content.local_time_millis, Some(50));
        assert_eq!(second_content.frame_index, Some(1));
        assert_eq!(second_content.local_time_millis, Some(150));
        assert_eq!(
            first.image_frames.get(&first_object.id).unwrap().rgba,
            vec![10, 40, 220, 255, 40, 220, 120, 255]
        );
        assert_eq!(
            second.image_frames.get(&second_object.id).unwrap().rgba,
            vec![240, 180, 20, 255, 220, 30, 180, 255]
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn agent_runtime_image_call_builds_bounded_layered_image_object() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[RuntimeCall {
                callee: "image".to_owned(),
                args: vec![
                    "asset = @asset.bg.pulse".to_owned(),
                    "id = @image.test.pulse".to_owned(),
                    "target = @target.test.pulse".to_owned(),
                    "layer = @layer.foreground".to_owned(),
                    "x = 12px".to_owned(),
                    "y = 34px".to_owned(),
                    "width = 56px".to_owned(),
                    "height = 78px".to_owned(),
                    "fit = stretch".to_owned(),
                    "opacity = 0.5".to_owned(),
                    "transform.tx = 24px".to_owned(),
                    "transform.ty = 12px".to_owned(),
                    "depth = 2500".to_owned(),
                    "enabled = false".to_owned(),
                    "visible = true".to_owned(),
                    "action = action.inspect.pulse".to_owned(),
                    "param.role = animated-hotspot".to_owned(),
                    "param.hit_channel = channel.preview".to_owned(),
                    "proxy.id = @proxy.pulse.hotspot".to_owned(),
                    "proxy.type = PulseHotspot".to_owned(),
                    "proxy.role = inspect".to_owned(),
                    "proxy.layer = @layer.hit".to_owned(),
                    "proxy.depth = 2600".to_owned(),
                    "proxy.hit_test = true".to_owned(),
                    "proxy.param.channel = preview".to_owned(),
                ],
            }],
            0.15,
        );

        assert!(diagnostics.is_empty());
        assert_eq!(observation.objects.len(), 1);
        let object = &observation.objects[0];
        assert_eq!(object.id, "object.image.layer.foreground.0.0");
        assert_eq!(object.layer, "layer.foreground");
        assert_eq!(object.entity.as_deref(), Some("image.test.pulse"));
        assert_eq!(object.object_layer.as_deref(), Some("layer.foreground"));
        assert_eq!(object.object_depth, Some(2500));
        assert!(!object.enabled);
        assert_eq!(object.bbox.x, 36);
        assert_eq!(object.bbox.y, 46);
        assert_eq!(object.bbox.width, 56);
        assert_eq!(object.bbox.height, 78);
        assert_eq!(
            object.polygon,
            vec![
                AgentPoint { x: 36, y: 46 },
                AgentPoint { x: 92, y: 46 },
                AgentPoint { x: 92, y: 124 },
                AgentPoint { x: 36, y: 124 },
            ]
        );
        assert!(
            object
                .capture_refs
                .captures
                .iter()
                .all(|capture| capture.uri.contains("/frame/2/")),
            "image capture refs should use the observation tick: {:?}",
            object.capture_refs
        );
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("image call should become Agent image content");
        };
        assert_eq!(content.object.as_deref(), Some("image.test.pulse"));
        assert_eq!(content.target.as_deref(), Some("target.test.pulse"));
        assert_eq!(content.asset.as_deref(), Some("asset.bg.pulse"));
        assert_eq!(content.frame_index, Some(1));
        assert_eq!(content.local_time_millis, Some(150));
        assert_eq!(content.opacity_milli, Some(500));
        assert_eq!(
            content.transform,
            Some(AgentImageTransform {
                m11_milli: 1_000,
                m12_milli: 0,
                m21_milli: 0,
                m22_milli: 1_000,
                tx_milli: 24_000,
                ty_milli: 12_000,
            })
        );
        assert_eq!(content.actions, vec!["action.inspect.pulse"]);
        assert_eq!(
            agent_action_targets(&observation.objects),
            vec![AgentActionTarget {
                id: "action.inspect.pulse".to_owned(),
                target: "target.test.pulse".to_owned(),
                action: AgentActionKind::Invoke,
                kind: AgentActionDispatch::Semantic,
                enabled: false,
            }]
        );
        assert_eq!(
            content.params.get("param.role"),
            Some(&AgentImageObjectParam::Text {
                value: "animated-hotspot".to_owned()
            })
        );
        assert_eq!(
            content.params.get("param.hit_channel"),
            Some(&AgentImageObjectParam::Text {
                value: "channel.preview".to_owned()
            })
        );
        assert_eq!(content.proxies.len(), 1);
        assert_eq!(content.proxies[0].id, "proxy.pulse.hotspot");
        assert_eq!(
            content.proxies[0].type_name.as_deref(),
            Some("PulseHotspot")
        );
        assert_eq!(content.proxies[0].role.as_deref(), Some("inspect"));
        assert_eq!(content.proxies[0].layer.as_deref(), Some("layer.hit"));
        assert_eq!(content.proxies[0].depth, Some(2600));
        assert!(content.proxies[0].hit_test);
        assert_eq!(
            content.proxies[0].params.get("param.channel"),
            Some(&RichTextParam::Text {
                value: "preview".to_owned()
            })
        );
        assert_eq!(
            observation.image_frames.get(&object.id).unwrap().rgba,
            vec![240, 180, 20, 255, 220, 30, 180, 255]
        );

        let mut report = test_agent_observation_report(None);
        report.viewport = viewport;
        report.objects = observation.objects.clone();
        let hit_report = agent_hit_test_report(&report, 40, 50);
        assert_eq!(hit_report.hits.len(), 2);
        assert_eq!(
            hit_report.hits[0].region.kind,
            AgentHitRegionKind::ObjectProxy
        );
        assert_eq!(
            hit_report.hits[0].region.proxy_id.as_deref(),
            Some("proxy.pulse.hotspot")
        );
        assert_eq!(hit_report.hits[0].layer, "layer.hit");
        assert_eq!(hit_report.hits[0].depth, Some(2600));

        let mut native_session =
            arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();
        let result = agent_native_capture_image_with_frame_store(
            &report,
            &AgentCaptureReadRequest {
                uri: "arcweft://session/cli/frame/2/layer.layer.foreground.rgba".to_owned(),
                image_kind: AgentObserveImageKind::RawRgba,
                capture_kind: AgentObserveCaptureKind::Color,
                scope: AgentCaptureScope::Layer("layer.foreground".to_owned()),
                page: 0,
                capture_step: 2,
                capture_time_seconds: 0.15,
            },
            &mut native_session,
            &observation.image_frames,
        )
        .unwrap();
        assert_eq!(result.image.width, 56);
        assert_eq!(result.image.height, 78);
        assert_eq!(&result.bytes[0..4], &[176, 131, 11, 127]);
    }

    #[test]
    fn agent_runtime_image_call_playback_pins_local_time_for_bounded_object() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[RuntimeCall {
                callee: "image".to_owned(),
                args: vec![
                    "asset = @asset.bg.pulse".to_owned(),
                    "id = @image.test.pinned_pulse".to_owned(),
                    "x = 12px".to_owned(),
                    "y = 34px".to_owned(),
                    "width = 56px".to_owned(),
                    "height = 78px".to_owned(),
                    "playback.local_time = 50ms".to_owned(),
                ],
            }],
            0.15,
        );

        assert!(diagnostics.is_empty());
        let object = &observation.objects[0];
        let AgentObservedObjectContent::Image(content) = &object.content else {
            panic!("image call should become Agent image content");
        };
        assert_eq!(content.frame_index, Some(0));
        assert_eq!(content.local_time_millis, Some(50));
        assert_eq!(
            observation.image_frames.get(&object.id).unwrap().rgba,
            vec![10, 40, 220, 255, 40, 220, 120, 255]
        );
    }

    #[test]
    fn agent_runtime_image_call_alignment_controls_fitted_object_geometry() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[RuntimeCall {
                callee: "image".to_owned(),
                args: vec![
                    "asset = @asset.bg.poster".to_owned(),
                    "id = @image.test.aligned_poster".to_owned(),
                    "x = 10px".to_owned(),
                    "y = 20px".to_owned(),
                    "width = 100px".to_owned(),
                    "height = 100px".to_owned(),
                    "fit = intrinsic".to_owned(),
                    "alignment.x = right".to_owned(),
                    "alignment.y = bottom".to_owned(),
                ],
            }],
            0.0,
        );

        assert!(diagnostics.is_empty());
        let object = &observation.objects[0];
        assert_eq!(object.bbox.x, 108);
        assert_eq!(object.bbox.y, 119);
        assert_eq!(object.bbox.width, 2);
        assert_eq!(object.bbox.height, 1);
        assert_eq!(
            object.polygon,
            vec![
                AgentPoint { x: 108, y: 119 },
                AgentPoint { x: 110, y: 119 },
                AgentPoint { x: 110, y: 120 },
                AgentPoint { x: 108, y: 120 },
            ]
        );
    }

    #[test]
    fn agent_image_call_alignment_parses_ratio_milli_and_keywords() {
        let call = RuntimeCall {
            callee: "image".to_owned(),
            args: vec![
                "alignment.x = 0.25".to_owned(),
                "align.y = bottom".to_owned(),
            ],
        };
        let alignment = agent_image_call_alignment(&call);

        assert_eq!(alignment.x_milli(), 250);
        assert_eq!(alignment.y_milli(), 1_000);
        assert_eq!(agent_image_alignment_component_milli("500", "x"), Some(500));
        assert_eq!(agent_image_alignment_component_milli("1", "y"), Some(1_000));
    }

    #[test]
    fn agent_image_call_playback_parses_time_and_rate_arguments() {
        let call = RuntimeCall {
            callee: "image".to_owned(),
            args: vec![
                "playback.start = 50ms".to_owned(),
                "playback.rate = 0.5".to_owned(),
                "playback.paused_at = 0.25s".to_owned(),
            ],
        };
        let playback = agent_image_call_playback(&call);

        assert_eq!(playback.start_time_millis(), 50);
        assert_eq!(playback.rate_milli(), 500);
        assert_eq!(playback.paused_at_millis(), Some(250));
        assert_eq!(playback.local_time_millis(1_000), 100);
    }

    #[test]
    fn agent_image_call_opacity_accepts_ratio_and_milli_forms() {
        assert_eq!(agent_image_call_opacity_milli("0"), Some(0));
        assert_eq!(agent_image_call_opacity_milli("1"), Some(1_000));
        assert_eq!(agent_image_call_opacity_milli("0.5"), Some(500));
        assert_eq!(agent_image_call_opacity_milli("500"), Some(500));
        assert_eq!(agent_image_call_opacity_milli("2.0"), Some(1_000));
    }

    #[test]
    fn agent_runtime_image_call_omits_invisible_image_object() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let viewport = AgentViewport {
            width: 320,
            height: 180,
            scale: 1.0,
        };
        let (observation, diagnostics) = agent_runtime_presentation_image_observation(
            &source,
            2,
            &viewport,
            &[RuntimeCall {
                callee: "image".to_owned(),
                args: vec![
                    "asset = @asset.bg.pulse".to_owned(),
                    "id = @image.test.hidden".to_owned(),
                    "x = 12px".to_owned(),
                    "y = 34px".to_owned(),
                    "width = 56px".to_owned(),
                    "height = 78px".to_owned(),
                    "visible = false".to_owned(),
                    "action = action.inspect.hidden".to_owned(),
                ],
            }],
            0.15,
        );

        assert!(diagnostics.is_empty());
        assert!(observation.objects.is_empty());
        assert!(observation.image_frames.frames_by_object.is_empty());
    }

    #[test]
    fn agent_source_image_decode_cache_reuses_decoded_animated_assets() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("samples")
            .join("image-animation.arcw");
        let mut cache = AgentSourceImageDecodeCache::default();

        let first = cache
            .decode_source_image_asset(&source, "asset.bg.pulse")
            .expect("animated gif asset decodes");
        let second = cache
            .decode_source_image_asset(&source, "asset.bg.pulse")
            .expect("animated gif asset is returned from cache");

        assert_eq!(cache.misses(), 1);
        assert_eq!(cache.hits(), 1);
        assert_eq!(first, second);
        assert!(second.is_animated());
        assert_eq!(second.frame_at_time_millis(150).unwrap().index(), 1);
    }

    #[test]
    fn agent_asset_call_parser_accepts_only_public_asset_refs() {
        assert_eq!(
            agent_asset_id_from_call_arg("@asset.bg.room")
                .map(|asset| asset.to_string())
                .as_deref(),
            Some("asset.bg.room")
        );
        assert!(agent_asset_id_from_call_arg("@core.bg.room").is_none());
        assert!(agent_asset_id_from_call_arg("@asset.bg room").is_none());
    }

    #[test]
    fn native_masked_framebuffer_crop_keeps_selected_rects_and_transparent_gap() {
        let source = arcweft_render_native::NativeFrameCapture {
            width: 8,
            height: 4,
            rgba: [9, 8, 7, 255].repeat(32),
            content_bbox: None,
            content_pixels: 0,
            diagnostics: Vec::new(),
        };
        let objects = vec![
            test_observed_object("object.ui.left", 1, 1, 2, 2),
            test_observed_object("object.ui.right", 5, 1, 2, 2),
        ];
        let selected = objects
            .iter()
            .map(AgentNativeCaptureTarget::Observed)
            .collect::<Vec<_>>();
        let frame = test_line_display_frame();
        let context = AgentNativeCaptureContext {
            frame: &frame,
            left: 0.0,
            top: 0.0,
            objects: &objects,
            page_index: 0,
            capture_time_seconds: 60.0,
        };

        let capture =
            agent_native_masked_framebuffer_capture(&source, context, &selected, None).unwrap();

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
        let source = arcweft_render_native::NativeFrameCapture {
            width: 32,
            height: 24,
            rgba: [0, 0, 0, 255].repeat(32 * 24),
            content_bbox: None,
            content_pixels: 0,
            diagnostics: Vec::new(),
        };
        let objects = vec![test_observed_object("object.ui.panel", 4, 5, 7, 6)];
        let selected = objects
            .iter()
            .map(AgentNativeCaptureTarget::Observed)
            .collect::<Vec<_>>();
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
            Some(arcweft_render_native::NativeFrameContentBBox {
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
                    diagnostics: Vec::new(),
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
                    diagnostics: Vec::new(),
                },
                10,
                10,
            ),
            &[255, 255, 255, 255]
        );
    }
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

#[derive(Default)]
struct AgentReplState {
    report: Option<AgentObservationReport>,
    history: Vec<AgentReplHistoryEntry>,
    bindings: BTreeMap<String, AgentReplBinding>,
    connection: Option<AgentReplConnection>,
    debug_store: Option<DebugStore>,
    trace_path: Option<String>,
    trace_records: usize,
    trace_resources: Vec<AgentResource>,
    read_only: bool,
    persisted_cells: u64,
    debug_db_path: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum AgentReplConnection {
    Source { path: String },
    Profile { id: String, manifest: String },
}

#[derive(Debug, serde::Serialize)]
struct AgentReplHistoryEntry {
    index: usize,
    input: String,
    kind: String,
    status: String,
}

#[derive(Clone, Debug, serde::Serialize)]
struct AgentReplBinding {
    name: String,
    binding_kind: String,
    source: String,
    status: String,
    final_status: Option<String>,
    host_calls: usize,
    responses: usize,
    serializable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    serialized_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    non_serializable_reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentReplSerializedBinding {
    source: String,
    snapshot_kind: String,
}

#[derive(Debug, serde::Serialize)]
struct AgentReplRunReport {
    ok: bool,
    cells: Vec<AgentReplCellReport>,
    final_tick: Option<usize>,
    connection: Option<AgentReplConnection>,
    debug_db: Option<String>,
    trace_path: Option<String>,
    trace_records: usize,
    read_only: bool,
    persisted_cells: u64,
}

#[derive(Debug, serde::Serialize)]
struct AgentReplCellReport {
    index: usize,
    input: String,
    kind: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<serde_json::Value>,
    #[serde(default)]
    quit: bool,
}

impl AgentReplCellReport {
    fn with_persistence(mut self, result: Result<bool, String>) -> Self {
        match result {
            Ok(false) => self,
            Ok(true) => {
                if let Some(serde_json::Value::Object(value)) = &mut self.value {
                    value.insert("persisted".to_owned(), serde_json::Value::Bool(true));
                }
                self
            }
            Err(error) => Self {
                index: self.index,
                input: self.input,
                kind: self.kind,
                status: "error".to_owned(),
                message: Some(error),
                value: self.value,
                quit: false,
            },
        }
    }
}

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

fn agent_repl_command(
    options: &AgentReplOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let mut state = agent_repl_initial_state(options)?;
    #[cfg(feature = "agent-repl")]
    {
        if agent_repl_uses_interactive_editor(options) {
            return agent_repl_interactive_command(options, state, adapter_registrars);
        }
    }
    let source = agent_repl_input(options)?;
    agent_repl_scripted_command(options, &mut state, adapter_registrars, &source)
}

fn agent_repl_initial_state(options: &AgentReplOptions) -> Result<AgentReplState, ExitCode> {
    let (debug_store, debug_db_path) = agent_repl_debug_store(options)?;
    let trace = agent_repl_trace_resources(options)?;
    let connection = agent_repl_initial_connection(options)?;
    Ok(AgentReplState {
        connection,
        debug_store,
        debug_db_path,
        trace_path: trace.path,
        trace_records: trace.record_count,
        trace_resources: trace.resources,
        read_only: options.read_only,
        ..AgentReplState::default()
    })
}

fn agent_repl_scripted_command(
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
    source: &str,
) -> Result<(), ExitCode> {
    let mut cells = Vec::new();
    let mut ok = true;

    for (index, line) in source.lines().enumerate() {
        let input = line.trim();
        if input.is_empty() || input.starts_with('#') {
            continue;
        }
        let report = agent_repl_eval_line(index, input, options, &mut *state, adapter_registrars);
        let quit = report.quit;
        ok &= report.status != "error";
        state.history.push(AgentReplHistoryEntry {
            index: report.index,
            input: report.input.clone(),
            kind: report.kind.clone(),
            status: report.status.clone(),
        });
        if options.json {
            cells.push(report);
        } else {
            agent_repl_print_cell(&report);
        }
        if quit {
            break;
        }
    }

    agent_repl_finish_debug_session(state, ok)?;

    if options.json {
        print_json(&AgentReplRunReport {
            ok,
            cells,
            final_tick: state.report.as_ref().map(|report| report.tick),
            connection: state.connection.clone(),
            debug_db: state.debug_db_path.clone(),
            trace_path: state.trace_path.clone(),
            trace_records: state.trace_records,
            read_only: state.read_only,
            persisted_cells: state.persisted_cells,
        })?;
    }
    if ok { Ok(()) } else { Err(ExitCode::from(2)) }
}

#[cfg(feature = "agent-repl")]
fn agent_repl_uses_interactive_editor(options: &AgentReplOptions) -> bool {
    options.input.is_none() && !options.json && std::io::stdin().is_terminal()
}

#[cfg(feature = "agent-repl")]
fn agent_repl_interactive_command(
    options: &AgentReplOptions,
    mut state: AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    let mut editor = agent_repl_line_editor(&state)?;
    let prompt = reedline::DefaultPrompt::new(
        reedline::DefaultPromptSegment::Basic("arcw".to_owned()),
        reedline::DefaultPromptSegment::Basic("agent".to_owned()),
    );
    let mut ok = true;
    let mut index = 0usize;
    loop {
        match editor.read_line(&prompt) {
            Ok(reedline::Signal::Success(buffer)) => {
                let input = buffer.trim();
                if input.is_empty() || input.starts_with('#') {
                    continue;
                }
                let report =
                    agent_repl_eval_line(index, input, options, &mut state, adapter_registrars);
                let quit = report.quit;
                ok &= report.status != "error";
                state.history.push(AgentReplHistoryEntry {
                    index: report.index,
                    input: report.input.clone(),
                    kind: report.kind.clone(),
                    status: report.status.clone(),
                });
                agent_repl_print_cell(&report);
                index = index.saturating_add(1);
                editor = agent_repl_line_editor(&state)?;
                if quit {
                    break;
                }
            }
            Ok(reedline::Signal::CtrlD) => break,
            Ok(reedline::Signal::CtrlC) => {
                eprintln!("^C");
                break;
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!("error: Agent REPL editor failed: {error}");
                ok = false;
                break;
            }
        }
    }
    agent_repl_finish_debug_session(&mut state, ok)?;
    if ok { Ok(()) } else { Err(ExitCode::from(2)) }
}

#[cfg(feature = "agent-repl")]
fn agent_repl_line_editor(state: &AgentReplState) -> Result<reedline::Reedline, ExitCode> {
    let mut editor = reedline::Reedline::create()
        .with_validator(Box::new(AgentReplReedlineValidator))
        .with_completer(Box::new(AgentReplReedlineCompleter {
            context: agent_repl_completion_context(state),
        }));
    let history_path = agent_repl_history_path().map_err(|error| {
        eprintln!("error: failed to create Agent REPL history directory: {error}");
        ExitCode::FAILURE
    })?;
    if let Some(path) = history_path {
        let history = reedline::FileBackedHistory::with_file(512, path).map_err(|error| {
            eprintln!("error: failed to open Agent REPL history: {error}");
            ExitCode::FAILURE
        })?;
        editor = editor.with_history(Box::new(history));
    }
    Ok(editor)
}

#[cfg(feature = "agent-repl")]
fn agent_repl_history_path() -> std::io::Result<Option<PathBuf>> {
    let Some(home) = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(PathBuf::from))
    else {
        return Ok(None);
    };
    let dir = home.join(".arcweft");
    std::fs::create_dir_all(&dir)?;
    Ok(Some(dir.join("agent-repl-history.txt")))
}

#[cfg(feature = "agent-repl")]
struct AgentReplReedlineValidator;

#[cfg(feature = "agent-repl")]
impl reedline::Validator for AgentReplReedlineValidator {
    fn validate(&self, line: &str) -> reedline::ValidationResult {
        match agent_repl_classify_cell(line).completion.kind {
            AgentReplCellCompletionKind::Incomplete => reedline::ValidationResult::Incomplete,
            _ => reedline::ValidationResult::Complete,
        }
    }
}

#[cfg(feature = "agent-repl")]
struct AgentReplReedlineCompleter {
    context: AgentReplCompletionContext,
}

#[cfg(feature = "agent-repl")]
impl reedline::Completer for AgentReplReedlineCompleter {
    fn complete(&mut self, line: &str, pos: usize) -> Vec<reedline::Suggestion> {
        let prefix = &line[..pos.min(line.len())];
        let replacement_start = agent_repl_completion_replacement_start(prefix);
        agent_repl_completions(prefix, &self.context)
            .into_iter()
            .map(|item| reedline::Suggestion {
                value: item.insert_text.unwrap_or(item.label.clone()),
                display_override: Some(item.label),
                description: item.detail,
                span: reedline::Span::new(replacement_start, pos.min(line.len())),
                ..reedline::Suggestion::default()
            })
            .collect()
    }
}

#[cfg(feature = "agent-repl")]
fn agent_repl_completion_replacement_start(source_before_cursor: &str) -> usize {
    source_before_cursor
        .char_indices()
        .rev()
        .find_map(|(index, character)| {
            (!(character.is_alphanumeric()
                || matches!(character, '_' | '.' | ':' | '@' | '-' | '/')))
            .then_some(index + character.len_utf8())
        })
        .unwrap_or(0)
}

fn agent_repl_finish_debug_session(state: &mut AgentReplState, ok: bool) -> Result<(), ExitCode> {
    let Some(store) = &state.debug_store else {
        return Ok(());
    };
    let mut metadata = BTreeMap::new();
    metadata.insert("cells".to_owned(), serde_json::json!(state.history.len()));
    metadata.insert(
        "persisted_cells".to_owned(),
        serde_json::json!(state.persisted_cells),
    );
    metadata.insert("read_only".to_owned(), serde_json::json!(state.read_only));
    if let Some(report) = &state.report {
        metadata.insert("final_tick".to_owned(), serde_json::json!(report.tick));
    }
    if let Some(connection) = &state.connection {
        metadata.insert(
            "connection".to_owned(),
            serde_json::to_value(connection).map_err(|error| {
                eprintln!("error: failed to serialize REPL session metadata: {error}");
                ExitCode::FAILURE
            })?,
        );
    }
    agent_debug_finish_runtime_session(
        store,
        &agent_cli_session_id(),
        if ok {
            DebugSessionStatus::Finished
        } else {
            DebugSessionStatus::Failed
        },
        &metadata,
        "REPL debug database session",
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

fn agent_repl_start_debug_session(store: &DebugStore) -> Result<(), ExitCode> {
    agent_debug_start_runtime_session(
        store,
        agent_cli_session_id(),
        None,
        "repl",
        "cli",
        BTreeMap::new(),
        "REPL debug database session",
    )
    .map(|_| ())
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })
}

fn agent_repl_initial_connection(
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, ExitCode> {
    let Some(target) = options.connect.as_deref() else {
        return Ok(None);
    };
    if options.read_only {
        eprintln!("error: agent repl --connect is not available with --read-only");
        return Err(ExitCode::from(2));
    }
    agent_repl_parse_connection(target, options).map_err(|message| {
        eprintln!("error: {message}");
        ExitCode::from(2)
    })
}

fn agent_repl_debug_store(
    options: &AgentReplOptions,
) -> Result<(Option<DebugStore>, Option<String>), ExitCode> {
    let Some(path) = &options.debug_db else {
        return Ok((None, None));
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            eprintln!("error: failed to create {}: {error}", parent.display());
            ExitCode::FAILURE
        })?;
    }
    let store = DebugStore::open(path).map_err(|error| {
        eprintln!(
            "error: failed to open REPL debug database {}: {error}",
            path.display()
        );
        ExitCode::FAILURE
    })?;
    agent_repl_start_debug_session(&store)?;
    Ok((Some(store), Some(path.display().to_string())))
}

struct AgentReplTraceResources {
    path: Option<String>,
    record_count: usize,
    resources: Vec<AgentResource>,
}

fn agent_repl_trace_resources(
    options: &AgentReplOptions,
) -> Result<AgentReplTraceResources, ExitCode> {
    match (&options.trace, options.read_only) {
        (None, true) => {
            eprintln!("error: agent repl --read-only requires --trace <file.arcwx>");
            Err(ExitCode::FAILURE)
        }
        (None, false) => Ok(AgentReplTraceResources {
            path: None,
            record_count: 0,
            resources: Vec::new(),
        }),
        (Some(path), _) => {
            let records = super::read_and_validate_agent_trace_records(path).map_err(|error| {
                eprintln!("{}: {error}", path.display());
                ExitCode::FAILURE
            })?;
            let resource = agent_repl_trace_resource(&records).map_err(|error| {
                eprintln!("{}: {error}", path.display());
                ExitCode::FAILURE
            })?;
            Ok(AgentReplTraceResources {
                path: Some(path.display().to_string()),
                record_count: records.len(),
                resources: vec![resource],
            })
        }
    }
}

fn agent_repl_trace_resource(records: &[AgentTraceRecord]) -> Result<AgentResource, String> {
    trace_resource(records).map_err(|error| format!("failed to serialize trace: {error}"))
}

fn agent_repl_input(options: &AgentReplOptions) -> Result<String, ExitCode> {
    if let Some(path) = &options.input {
        return fs::read_to_string(path).map_err(|error| {
            eprintln!("error: failed to read {}: {error}", path.display());
            ExitCode::FAILURE
        });
    }
    let mut input = String::new();
    std::io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| {
            eprintln!("error: failed to read REPL stdin: {error}");
            ExitCode::FAILURE
        })?;
    Ok(input)
}

fn agent_repl_eval_line(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    if input.starts_with(':') {
        return agent_repl_eval_meta(index, input, options, state, adapter_registrars);
    }
    if state.read_only {
        return agent_repl_error(
            index,
            input,
            "cell",
            "read-only Agent REPL does not execute Agent cells".to_owned(),
        );
    }
    agent_repl_eval_compiled_cell(index, input, state)
}

fn agent_repl_eval_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    let mut parts = input.splitn(2, char::is_whitespace);
    let command = parts.next().unwrap_or_default();
    let rest = parts.next().unwrap_or_default().trim();
    if state.read_only && agent_repl_read_only_rejects(command) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("read-only Agent REPL does not execute `{command}`"),
        );
    }
    match command {
        ":help" => agent_repl_help(index, input),
        ":trace" => agent_repl_trace(index, input, state),
        ":observe" => agent_repl_observe(index, input, options, state, adapter_registrars),
        ":actions" => agent_repl_actions(index, input, state),
        ":type" | ":ast" | ":hir" | ":bytecode" | ":capture" | ":complete" | ":highlight" => {
            agent_repl_eval_inspection_meta(
                index,
                input,
                command,
                rest,
                options,
                state,
                adapter_registrars,
            )
        }
        ":query" => agent_repl_eval_query_meta(index, input, rest, state),
        ":history" => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({ "cells": &state.history }),
        ),
        ":bindings" => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "bindings": state.bindings.values().collect::<Vec<_>>(),
            }),
        ),
        ":connect" => agent_repl_connect(index, input, rest, options, state),
        ":drop" | ":load" | ":save" | ":parse" | ":classify" => {
            agent_repl_eval_binding_meta(index, input, command, rest, state)
        }
        ":reset" => {
            state.report = None;
            state.history.clear();
            state.bindings.clear();
            state.connection = None;
            state.persisted_cells = 0;
            agent_repl_ok(index, input, "meta", serde_json::json!({ "reset": true }))
        }
        ":quit" => AgentReplCellReport {
            index,
            input: input.to_owned(),
            kind: "meta".to_owned(),
            status: "ok".to_owned(),
            message: None,
            value: Some(serde_json::json!({ "quit": true })),
            quit: true,
        },
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

fn agent_repl_read_only_rejects(command: &str) -> bool {
    matches!(
        command,
        ":observe" | ":capture" | ":connect" | ":drop" | ":load" | ":save" | ":reset"
    )
}

fn agent_repl_eval_inspection_meta(
    index: usize,
    input: &str,
    command: &str,
    rest: &str,
    options: &AgentReplOptions,
    state: &AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    match command {
        ":type" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":type requires an expression".to_owned(),
        ),
        ":type" => agent_repl_type(index, input, rest),
        ":ast" if rest.is_empty() => {
            agent_repl_error(index, input, "meta", ":ast requires a fragment".to_owned())
        }
        ":ast" => agent_repl_ast(index, input, rest),
        ":hir" if rest.is_empty() => {
            agent_repl_error(index, input, "meta", ":hir requires a fragment".to_owned())
        }
        ":hir" => agent_repl_hir(index, input, rest),
        ":bytecode" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":bytecode requires a fragment".to_owned(),
        ),
        ":bytecode" => agent_repl_bytecode(index, input, rest),
        ":capture" => agent_repl_capture(index, input, rest, options, state, adapter_registrars),
        ":complete" => agent_repl_complete(index, input, rest, state),
        ":highlight" => agent_repl_highlight(index, input, rest),
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

fn agent_repl_eval_query_meta(
    index: usize,
    input: &str,
    rest: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if rest.is_empty() {
        agent_repl_error(index, input, "meta", ":query requires text".to_owned())
    } else {
        agent_repl_query(index, input, rest, state)
    }
}

fn agent_repl_eval_binding_meta(
    index: usize,
    input: &str,
    command: &str,
    rest: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    match command {
        ":drop" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":drop requires a binding name".to_owned(),
        ),
        ":drop" => agent_repl_drop(index, input, rest, state),
        ":load" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":load requires a .awfagent path".to_owned(),
        ),
        ":load" => agent_repl_load(index, input, rest, state),
        ":save" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":save requires a .awfagent path".to_owned(),
        ),
        ":save" => agent_repl_save(index, input, rest, state),
        ":parse" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":parse requires a fragment".to_owned(),
        ),
        ":classify" if rest.is_empty() => agent_repl_error(
            index,
            input,
            "meta",
            ":classify requires a fragment".to_owned(),
        ),
        ":parse" | ":classify" => agent_repl_ok(index, input, "meta", agent_repl_parse_cell(rest)),
        _ => agent_repl_error(
            index,
            input,
            "meta",
            format!("unknown Agent REPL command `{command}`"),
        ),
    }
}

fn agent_repl_help(index: usize, input: &str) -> AgentReplCellReport {
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "commands": [
                ":help",
                ":type EXPR",
                ":ast EXPR_OR_STMT",
                ":hir EXPR_OR_STMT",
                ":bytecode EXPR_OR_STMT",
                ":observe",
                ":actions",
                ":trace",
                ":capture [viewport|layer ID|object ID]",
                ":query TEXT",
                ":history",
                ":bindings",
                ":drop NAME",
                ":load FILE.awfagent",
                ":save FILE.awfagent",
                ":parse EXPR_OR_STMT",
                ":classify EXPR_OR_STMT",
                ":reset",
                ":connect current|source PATH|profile ID [--manifest PATH]",
                ":complete SOURCE_BEFORE_CURSOR",
                ":highlight SOURCE",
                ":quit"
            ],
            "startup_options": [
                "--connect current|source PATH|profile ID [--manifest PATH]"
            ]
        }),
    )
}

fn agent_repl_trace(index: usize, input: &str, state: &AgentReplState) -> AgentReplCellReport {
    let descriptors = list_resources_result(&state.trace_resources).resources;
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "loaded": !state.trace_resources.is_empty(),
            "path": state.trace_path.clone(),
            "record_count": state.trace_records,
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "read_only": state.read_only,
        }),
    )
}

fn agent_repl_observe(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    match agent_observation_report_for_options(
        &agent_repl_observe_options(options, state),
        adapter_registrars,
    ) {
        Ok(report) => {
            let value = serde_json::json!({
                "tick": report.tick,
                "frame_id": report.frame_id,
                "state_hash": report.state_hash,
                "render_hash": report.render_hash,
                "actions": report.actions.len(),
                "objects": report.objects.len(),
                "diagnostics": report.diagnostics.len(),
            });
            state.report = Some(report);
            agent_repl_ok(index, input, "meta", value)
        }
        Err(code) => agent_repl_error(
            index,
            input,
            "meta",
            format!("observe failed with exit code {code:?}"),
        ),
    }
}

fn agent_repl_actions(index: usize, input: &str, state: &AgentReplState) -> AgentReplCellReport {
    match &state.report {
        Some(report) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "tick": report.tick,
                "actions": report.actions,
            }),
        ),
        None => agent_repl_error(
            index,
            input,
            "meta",
            ":actions requires :observe first".to_owned(),
        ),
    }
}

fn agent_repl_complete(
    index: usize,
    input: &str,
    source_before_cursor: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if source_before_cursor.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":complete requires source text before the cursor".to_owned(),
        );
    }
    let context = agent_repl_completion_context(state);
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "source": source_before_cursor,
            "items": agent_repl_completions(source_before_cursor, &context),
        }),
    )
}

fn agent_repl_highlight(index: usize, input: &str, source: &str) -> AgentReplCellReport {
    if source.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":highlight requires source text".to_owned(),
        );
    }
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "source": source,
            "tokens": agent_repl_highlight_tokens(source),
        }),
    )
}

fn agent_repl_completion_context(state: &AgentReplState) -> AgentReplCompletionContext {
    let mut context = AgentReplCompletionContext {
        live_bindings: state.bindings.keys().cloned().collect(),
        ..AgentReplCompletionContext::default()
    };
    let Some(report) = &state.report else {
        return context;
    };
    context.entities = agent_repl_completion_entities(report);
    context.action_targets = report
        .actions
        .iter()
        .map(|action| action.target.clone())
        .collect();
    context.layer_ids = report.layers.iter().map(|layer| layer.id.clone()).collect();
    context.object_ids = report
        .objects
        .iter()
        .map(|object| object.id.clone())
        .collect();
    context.effect_capabilities = report
        .presentation_tree
        .nodes
        .iter()
        .flat_map(|node| node.effects.iter().map(|effect| effect.id.clone()))
        .collect();
    context
}

fn agent_repl_completion_entities(
    report: &AgentObservationReport,
) -> Vec<AgentReplCompletionEntity> {
    report
        .actions
        .iter()
        .filter_map(|action| match action.action {
            AgentActionKind::SelectChoice => Some(AgentReplCompletionEntity {
                id: action.target.clone(),
                kind: "choice_option".to_owned(),
            }),
            AgentActionKind::AdvanceText
            | AgentActionKind::Invoke
            | AgentActionKind::PointerClick => None,
        })
        .collect()
}

fn agent_repl_connect(
    index: usize,
    input: &str,
    target: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let target = target.trim();
    if target.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":connect requires a target".to_owned(),
        );
    }

    let connection = match agent_repl_parse_connection(target, options) {
        Ok(connection) => connection,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    state.connection = connection;
    state.report = None;

    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "connected": true,
            "connection": state.connection,
        }),
    )
}

fn agent_repl_parse_connection(
    target: &str,
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, String> {
    if target == "current" {
        if options.path.is_none() && options.profile.profile.is_none() {
            return Err(
                ":connect current requires the REPL to start with a source path or --profile"
                    .to_owned(),
            );
        }
        return Ok(None);
    }
    if let Some(endpoint) = target
        .strip_prefix("stdio:")
        .or_else(|| target.strip_prefix("mcp:"))
    {
        return Err(format!(
            "remote REPL endpoint `{endpoint}` is not implemented; use `source PATH` or `profile ID`"
        ));
    }
    if let Some(path) = target.strip_prefix("source ") {
        let path = path.trim();
        if path.is_empty() {
            return Err(":connect source requires a path".to_owned());
        }
        return Ok(Some(AgentReplConnection::Source {
            path: path.to_owned(),
        }));
    }
    if let Some(rest) = target.strip_prefix("profile ") {
        return agent_repl_parse_profile_connection(rest, options);
    }
    if Path::new(target)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
    {
        return Ok(Some(AgentReplConnection::Source {
            path: target.to_owned(),
        }));
    }
    Ok(Some(AgentReplConnection::Profile {
        id: target.to_owned(),
        manifest: options.profile.manifest.display().to_string(),
    }))
}

fn agent_repl_parse_profile_connection(
    rest: &str,
    options: &AgentReplOptions,
) -> Result<Option<AgentReplConnection>, String> {
    let mut parts = rest.split_whitespace();
    let id = parts
        .next()
        .ok_or_else(|| ":connect profile requires an id".to_owned())?;
    let mut manifest = options.profile.manifest.display().to_string();
    while let Some(flag) = parts.next() {
        match flag {
            "--manifest" => {
                parts
                    .next()
                    .ok_or_else(|| ":connect profile --manifest requires a path".to_owned())?
                    .clone_into(&mut manifest);
            }
            _ => return Err(format!("unsupported :connect profile option `{flag}`")),
        }
    }
    Ok(Some(AgentReplConnection::Profile {
        id: id.to_owned(),
        manifest,
    }))
}

fn agent_repl_type(index: usize, input: &str, fragment_source: &str) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let parse = agent_repl_fragment_report(&fragment);
    let compiled = match agent_repl_compile_fragment(index, fragment_source, &fragment) {
        Ok(compiled) => compiled,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let Some(ty) = agent_repl_display_type(&compiled.typecheck_report) else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "type-check succeeded, but no displayable expression type judgment was produced"
                .to_owned(),
        );
    };
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "type": ty,
            "parse": parse,
        }),
    )
}

fn agent_repl_ast(index: usize, input: &str, fragment_source: &str) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "parse": agent_repl_fragment_report(&fragment),
            "ast": fragment.kind().map(|kind| format!("{kind:#?}")),
        }),
    )
}

fn agent_repl_hir(index: usize, input: &str, fragment_source: &str) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let Some(source) = agent_repl_inspection_source(index, fragment_source, &fragment) else {
        return agent_repl_error(
            index,
            input,
            "meta",
            "fragment is not complete enough to lower to HIR".to_owned(),
        );
    };
    match arcweft_compiler::compile_agent_source(source) {
        Ok(compiled) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "parse": agent_repl_fragment_report(&fragment),
                "hir": format!("{:#?}", compiled.hir),
            }),
        ),
        Err(error) => agent_repl_error(index, input, "meta", error.to_string()),
    }
}

fn agent_repl_bytecode(index: usize, input: &str, fragment_source: &str) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(fragment_source);
    let compiled = match agent_repl_compile_fragment(index, fragment_source, &fragment) {
        Ok(compiled) => compiled,
        Err(message) => return agent_repl_error(index, input, "meta", message),
    };
    let stats = compiled.bundle.bytecode.program.stats();
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "parse": agent_repl_fragment_report(&fragment),
            "agent_id": compiled.manifest.agent_id.as_str(),
            "entry_flow": compiled.bundle.bytecode.program.entry_flow.as_ref().map(|flow| flow.0.as_str()),
            "stats": {
                "flows": stats.flows,
                "instructions": stats.instructions,
                "line_task_groups": stats.line_task_groups,
                "stream_plans": stats.stream_plans,
                "source_plans": stats.source_plans,
            },
            "program": format!("{:#?}", compiled.bundle.bytecode.program),
        }),
    )
}

fn agent_repl_capture(
    index: usize,
    input: &str,
    target: &str,
    options: &AgentReplOptions,
    state: &AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    let mut observe_options = agent_repl_observe_options(options, state);
    observe_options.image = Some(AgentObserveImageKind::Png);
    observe_options.capture = Some(AgentObserveCaptureKind::Color);
    if let Err(message) = apply_agent_repl_capture_target(target, &mut observe_options) {
        return agent_repl_error(index, input, "meta", message);
    }
    match agent_observation_for_options(&observe_options, adapter_registrars).and_then(
        |mut observed| {
            let image_output = agent_observe_image_output(
                &mut observed.report,
                &observe_options,
                Some(&mut observed.native_session),
                &observed.image_frames,
            )?;
            let resource = agent_observe_image_resource(&observed.report, image_output.as_ref());
            Ok((observed.report, resource))
        },
    ) {
        Ok((report, resource)) => {
            let resource_summary = resource.as_ref().map(|resource| {
                serde_json::json!({
                    "uri": resource.uri,
                    "kind": resource.kind,
                    "mime_type": resource.mime_type,
                    "hash": resource.hash,
                })
            });
            agent_repl_ok(
                index,
                input,
                "meta",
                serde_json::json!({
                    "tick": report.tick,
                    "frame_id": report.frame_id,
                    "images": report.images,
                    "resource": resource_summary,
                }),
            )
        }
        Err(code) => agent_repl_error(
            index,
            input,
            "meta",
            format!("capture failed with exit code {code:?}"),
        ),
    }
}

fn agent_repl_query(
    index: usize,
    input: &str,
    query: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    if state.report.is_none() && state.trace_resources.is_empty() {
        return agent_repl_error(
            index,
            input,
            "meta",
            ":query requires :observe or --trace first".to_owned(),
        );
    }
    let mcp_state = AgentMcpState {
        report: state.report.clone(),
        image_output: None,
        image_frames: AgentImageFrameStore::default(),
        capture_resources: Vec::new(),
        trace_resources: state.trace_resources.clone(),
        rag_context_packs: Vec::new(),
        native_capture_session: None,
        runtime: None,
        observe_options: None,
    };
    match agent_mcp_rag_context_pack(
        &mcp_state,
        query,
        Vec::new(),
        1,
        8,
        32 * 1024,
        PrivacyClass::Project,
    ) {
        Ok(pack) => {
            let value = serde_json::to_value(pack)
                .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }));
            agent_repl_ok(index, input, "meta", value)
        }
        Err(error) => agent_repl_error(index, input, "meta", error),
    }
}

fn agent_repl_drop(
    index: usize,
    input: &str,
    name: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    match state.bindings.remove(name) {
        Some(binding) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "dropped": binding.name,
                "binding_kind": binding.binding_kind,
            }),
        ),
        None => agent_repl_error(
            index,
            input,
            "meta",
            format!("REPL binding `{name}` does not exist"),
        ),
    }
}

fn agent_repl_load(
    index: usize,
    input: &str,
    raw_path: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let path = PathBuf::from(raw_path);
    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            return agent_repl_error(
                index,
                input,
                "meta",
                format!("failed to read {}: {error}", path.display()),
            );
        }
    };
    if let Err(error) = arcweft_compiler::compile_agent_source(source.clone()) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to load {}: {error}", path.display()),
        );
    }

    let name = agent_repl_loaded_binding_name(&path, index);
    state.bindings.insert(
        name.clone(),
        AgentReplBinding {
            name: name.clone(),
            binding_kind: "loaded_agent".to_owned(),
            source,
            status: "ok".to_owned(),
            final_status: None,
            host_calls: 0,
            responses: 0,
            serializable: false,
            serialized_source: None,
            snapshot_kind: None,
            non_serializable_reason: Some("loaded Agent source is not a REPL value".to_owned()),
        },
    );
    agent_repl_ok(
        index,
        input,
        "meta",
        serde_json::json!({
            "loaded": path.display().to_string(),
            "binding": name,
        }),
    )
}

fn agent_repl_save(
    index: usize,
    input: &str,
    raw_path: &str,
    state: &AgentReplState,
) -> AgentReplCellReport {
    let path = PathBuf::from(raw_path);
    let source = agent_repl_saved_source(state);
    if let Err(error) = arcweft_compiler::compile_agent_source(source.clone()) {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("refusing to save invalid Agent source: {error}"),
        );
    }
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && let Err(error) = fs::create_dir_all(parent)
    {
        return agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to create {}: {error}", parent.display()),
        );
    }
    match fs::write(&path, source.as_bytes()) {
        Ok(()) => agent_repl_ok(
            index,
            input,
            "meta",
            serde_json::json!({
                "saved": path.display().to_string(),
                "bytes": source.len(),
            }),
        ),
        Err(error) => agent_repl_error(
            index,
            input,
            "meta",
            format!("failed to write {}: {error}", path.display()),
        ),
    }
}

fn agent_repl_loaded_binding_name(path: &Path, fallback_index: usize) -> String {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("agent");
    let sanitized = stem
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '_' || character == '-' {
                character
            } else {
                '_'
            }
        })
        .collect::<String>();
    format!("loaded.{fallback_index}.{sanitized}")
}

fn agent_repl_saved_source(state: &AgentReplState) -> String {
    let mut body = state
        .bindings
        .values()
        .filter(|binding| binding.status == "ok")
        .filter(|binding| binding.binding_kind == "cell")
        .map(|binding| indent_agent_repl_body(&binding.source))
        .collect::<Vec<_>>()
        .join("\n");
    if body.trim().is_empty() {
        "    return \"empty\"".clone_into(&mut body);
    } else if !body.contains("\n    return ") && !body.starts_with("    return ") {
        body.push_str("\n    return \"saved\"");
    }
    format!(
        "#[agent(version = 1)]\nagent @agent.repl.saved repl_saved()\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n"
    )
}

fn agent_repl_observe_options(
    options: &AgentReplOptions,
    state: &AgentReplState,
) -> AgentObserveOptions {
    let mut observe_options = AgentObserveOptions {
        path: options.path.clone(),
        profile: options.profile.clone(),
        entry: options.entry.clone(),
        flow: options.flow.clone(),
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        steps: options.steps,
        capture_step: options.capture_step,
        mode: options.mode,
        max_ops: options.max_ops,
        values: options.values.clone(),
        viewport_width: options.viewport_width,
        viewport_height: options.viewport_height,
        textbox_height: options.textbox_height,
        image: None,
        capture: None,
        layer: None,
        object: None,
        page: None,
        capture_time_seconds: options.capture_time_seconds,
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        out: None,
        json: true,
    };
    match &state.connection {
        Some(AgentReplConnection::Source { path }) => {
            observe_options.path = Some(PathBuf::from(path));
            observe_options.profile.profile = None;
        }
        Some(AgentReplConnection::Profile { id, manifest }) => {
            observe_options.path = None;
            observe_options.profile.profile = Some(id.clone());
            observe_options.profile.manifest = PathBuf::from(manifest);
        }
        None => {}
    }
    observe_options
}

fn agent_repl_parse_cell(input: &str) -> serde_json::Value {
    serde_json::to_value(agent_repl_classify_cell(input))
        .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }))
}

fn agent_repl_fragment_report(fragment: &ParsedFragment) -> serde_json::Value {
    serde_json::to_value(agent_repl_classification_from_fragment(fragment))
        .unwrap_or_else(|error| serde_json::json!({ "error": error.to_string() }))
}

fn agent_repl_compile_fragment(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
) -> Result<arcweft_compiler::CompiledAgentBundle, String> {
    let source = agent_repl_inspection_source(index, input, fragment)
        .ok_or_else(|| "fragment is not complete enough to compile".to_owned())?;
    let project = agent_script_project_index(&[])?;
    arcweft_compiler::compile_agent_bundle_with_project(source, &project)
        .map_err(|error| error.to_string())
}

fn agent_repl_inspection_source(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
) -> Option<String> {
    (matches!(fragment.completion(), ParseCompletion::Complete) && fragment.errors().is_empty())
        .then(|| agent_repl_cell_source(index, input, fragment, ""))
}

fn agent_repl_display_type(report: &TypeCheckReport) -> Option<String> {
    report
        .judgments
        .iter()
        .rev()
        .find(|judgment| judgment.rule == TypeJudgmentRule::Return)
        .or_else(|| report.judgments.iter().next_back())
        .map(|judgment| format!("{:?}", judgment.ty))
}

fn apply_agent_repl_capture_target(
    target: &str,
    options: &mut AgentObserveOptions,
) -> Result<(), String> {
    let target = target.trim();
    if target.is_empty() || target == "viewport" {
        return Ok(());
    }
    let mut parts = target.split_whitespace();
    let kind = parts.next().unwrap_or_default();
    let id = parts
        .next()
        .ok_or_else(|| format!(":capture {kind} requires an id"))?;
    if parts.next().is_some() {
        return Err(":capture accepts only viewport, layer ID, or object ID".to_owned());
    }
    match kind {
        "layer" => {
            options.layer = Some(id.to_owned());
            Ok(())
        }
        "object" => {
            options.object = Some(id.to_owned());
            Ok(())
        }
        _ => Err(":capture accepts only viewport, layer ID, or object ID".to_owned()),
    }
}

fn agent_repl_eval_compiled_cell(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
) -> AgentReplCellReport {
    let fragment = agent_repl_parse_fragment(input);
    let parse_report = agent_repl_fragment_report(&fragment);
    if !matches!(fragment.completion(), ParseCompletion::Complete) || !fragment.errors().is_empty()
    {
        return agent_repl_invalid_fragment_report(index, input, state, &fragment, parse_report);
    }

    let source = agent_repl_cell_source(
        index,
        input,
        &fragment,
        &agent_repl_live_binding_prelude(state),
    );
    let binding_names = agent_repl_fragment_binding_names(&fragment);
    let serialized_bindings = agent_repl_serialized_bindings(&fragment);
    let project = match agent_script_project_index(&[]) {
        Ok(project) => project,
        Err(error) => return agent_repl_error(index, input, "cell", error),
    };
    let compiled = match arcweft_compiler::compile_agent_bundle_with_project(source, &project) {
        Ok(compiled) => compiled,
        Err(error) => {
            let message = error.to_string();
            return agent_repl_compile_error_report(index, input, state, parse_report, &message);
        }
    };
    let project_entities = match arcweft_compiler::agent_required_entities_from_project(&project) {
        Ok(entities) => entities,
        Err(error) => return agent_repl_error(index, input, "cell", error.to_string()),
    };
    let project_graph = match arcweft_compiler::agent_project_graph_from_project(&project) {
        Ok(graph) => graph,
        Err(error) => return agent_repl_error(index, input, "cell", error.to_string()),
    };
    let mut runner = AgentRunner::new(
        CliAgentSession::new(
            Vec::new(),
            Vec::new(),
            project.program_hash().as_str().to_owned(),
            project_entities,
            project_graph,
        ),
        CollectingDebugSink::default(),
        NoopRagService,
        agent_script_runtime_policy_for_bundle(&compiled.bundle),
        AgentRunnerConfig::new(agent_cli_session_id()),
    );
    let run_result = runner.run_controller_bundle(
        &compiled.bundle,
        AgentControllerRunConfig {
            max_steps: 16,
            max_ops_per_step: 128,
        },
    );
    match run_result {
        Ok(report) => agent_repl_run_success_report(
            index,
            input,
            state,
            &parse_report,
            &report,
            &binding_names,
            &serialized_bindings,
        ),
        Err(error) => {
            let message = error.to_string();
            let effect_summary = agent_repl_run_effect_summary(&runner.debug_mut().events);
            agent_repl_run_error_report(
                index,
                input,
                state,
                &parse_report,
                &message,
                effect_summary,
            )
        }
    }
}

fn agent_repl_invalid_fragment_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    fragment: &ParsedFragment,
    parse_report: serde_json::Value,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some("REPL cell is not a complete Agent fragment".to_owned()),
        value: Some(parse_report),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        serde_json::json!({
            "parse": fragment
                .errors()
                .iter()
                .map(|error| error.message().to_owned())
                .collect::<Vec<_>>()
        }),
        false,
    ))
}

fn agent_repl_compile_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: serde_json::Value,
    message: &str,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(parse_report),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        serde_json::json!({ "compile_error": message }),
        false,
    ))
}

fn agent_repl_run_success_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    report: &AgentControllerRunReport,
    binding_names: &[String],
    serialized_bindings: &BTreeMap<String, AgentReplSerializedBinding>,
) -> AgentReplCellReport {
    let binding_name = format!("cell.{index}");
    let final_status = report
        .final_status
        .as_ref()
        .map(|status| format!("{status:?}"));
    let responses = report.responses.len();
    let value = serde_json::json!({
        "binding": binding_name,
        "compiled": true,
        "steps": report.steps,
        "host_calls": report.host_calls,
        "responses": report.responses,
        "events_emitted": report.events_emitted,
        "final_status": final_status,
        "bindings": binding_names,
        "parse": parse_report,
    });
    if let Some(error) = agent_repl_unsupported_snapshot_error(binding_names, serialized_bindings) {
        return agent_repl_snapshot_escape_error_report(
            index,
            input,
            state,
            parse_report,
            &error,
            report.host_calls,
            report.events_emitted,
        );
    }
    let persisted = agent_repl_persist_cell(
        state,
        index,
        input,
        "ok",
        value.clone(),
        report.host_calls > 0,
    );
    state.bindings.insert(
        binding_name.clone(),
        AgentReplBinding {
            name: binding_name.clone(),
            binding_kind: "cell".to_owned(),
            source: input.to_owned(),
            status: "ok".to_owned(),
            final_status,
            host_calls: report.host_calls,
            responses,
            serializable: false,
            serialized_source: None,
            snapshot_kind: None,
            non_serializable_reason: Some("cell artifact is not a REPL value".to_owned()),
        },
    );
    for local_name in binding_names {
        let serialized = serialized_bindings.get(local_name);
        state.bindings.insert(
            local_name.clone(),
            AgentReplBinding {
                name: local_name.clone(),
                binding_kind: "local".to_owned(),
                source: input.to_owned(),
                status: "ok".to_owned(),
                final_status: state
                    .bindings
                    .get(&binding_name)
                    .and_then(|binding| binding.final_status.clone()),
                host_calls: report.host_calls,
                responses,
                serializable: serialized.is_some(),
                serialized_source: serialized.map(|binding| binding.source.clone()),
                snapshot_kind: serialized.map(|binding| binding.snapshot_kind.clone()),
                non_serializable_reason: serialized
                    .is_none()
                    .then(|| "binding value is not a supported REPL snapshot".to_owned()),
            },
        );
    }
    agent_repl_ok(index, input, "cell", value).with_persistence(persisted)
}

fn agent_repl_unsupported_snapshot_error(
    binding_names: &[String],
    serialized_bindings: &BTreeMap<String, AgentReplSerializedBinding>,
) -> Option<String> {
    let unsupported = binding_names
        .iter()
        .filter(|name| !serialized_bindings.contains_key(*name))
        .cloned()
        .collect::<Vec<_>>();
    (!unsupported.is_empty()).then(|| {
        format!(
            "REPL local binding(s) cannot cross cells without a supported snapshot: {}",
            unsupported.join(", ")
        )
    })
}

fn agent_repl_snapshot_escape_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    message: &str,
    host_calls: usize,
    events_emitted: u64,
) -> AgentReplCellReport {
    let value = serde_json::json!({
        "parse": parse_report,
        "snapshot_error": message,
        "host_calls": host_calls,
        "events_emitted": events_emitted,
        "partially_effectful": host_calls > 0,
    });
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(value.clone()),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        value,
        host_calls > 0,
    ))
}

fn agent_repl_run_error_report(
    index: usize,
    input: &str,
    state: &mut AgentReplState,
    parse_report: &serde_json::Value,
    message: &str,
    effect_summary: AgentReplRunEffectSummary,
) -> AgentReplCellReport {
    let value = serde_json::json!({
        "parse": parse_report,
        "run_error": message,
        "host_calls": effect_summary.host_calls,
        "events_emitted": effect_summary.events_emitted,
        "partially_effectful": effect_summary.partially_effectful,
    });
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "cell".to_owned(),
        status: "error".to_owned(),
        message: Some(message.to_owned()),
        value: Some(value.clone()),
        quit: false,
    }
    .with_persistence(agent_repl_persist_cell(
        state,
        index,
        input,
        "error",
        value,
        effect_summary.partially_effectful,
    ))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AgentReplRunEffectSummary {
    host_calls: usize,
    events_emitted: u64,
    partially_effectful: bool,
}

fn agent_repl_run_effect_summary(events: &[DebugEvent]) -> AgentReplRunEffectSummary {
    let host_calls = events
        .iter()
        .filter(|event| agent_repl_effectful_debug_event(event.kind))
        .count();
    AgentReplRunEffectSummary {
        host_calls,
        events_emitted: events.last().map_or(0, |event| event.sequence),
        partially_effectful: host_calls > 0,
    }
}

fn agent_repl_effectful_debug_event(kind: DebugEventKind) -> bool {
    matches!(
        kind,
        DebugEventKind::Observation
            | DebugEventKind::Action
            | DebugEventKind::Capture
            | DebugEventKind::Assertion
            | DebugEventKind::RagQuery
    )
}

fn agent_repl_persist_cell(
    state: &mut AgentReplState,
    index: usize,
    input: &str,
    status: &str,
    display: serde_json::Value,
    partially_effectful: bool,
) -> Result<bool, String> {
    let Some(store) = &state.debug_store else {
        return Ok(false);
    };
    let ordinal = i64::try_from(index).map_err(|_| "REPL cell index is too large".to_owned())?;
    let source_hash = StableHash::new(format!(
        "blake3:{}",
        blake3::hash(input.as_bytes()).to_hex()
    ))
    .expect("generated REPL cell hash is nonempty");
    store
        .upsert_repl_cell(&DebugReplCell {
            cell_id: format!("repl:session.cli:{ordinal}"),
            session_id: agent_cli_session_id(),
            run_id: None,
            ordinal,
            source: input.to_owned(),
            source_hash,
            status: status.to_owned(),
            inferred_type: None,
            display: Some(display),
            partially_effectful,
            diagnostic_ids: Vec::new(),
            created_unix_ms: 0,
        })
        .map_err(|error| format!("failed to persist REPL cell {index}: {error}"))?;
    state.persisted_cells = state.persisted_cells.saturating_add(1);
    Ok(true)
}

fn agent_repl_cell_source(
    index: usize,
    input: &str,
    fragment: &ParsedFragment,
    live_binding_prelude: &str,
) -> String {
    if matches!(fragment.kind(), Some(ParsedFragmentKind::Items(_))) {
        return input.to_owned();
    }
    let cell_body = if matches!(fragment.kind(), Some(ParsedFragmentKind::Expression(_))) {
        format!("    return {input}")
    } else if input.starts_with("return ") || input.contains("\nreturn ") {
        indent_agent_repl_body(input)
    } else {
        format!("{}\n    return \"ok\"", indent_agent_repl_body(input))
    };
    let body = if live_binding_prelude.trim().is_empty() {
        cell_body
    } else {
        format!(
            "{}\n{}",
            indent_agent_repl_body(live_binding_prelude),
            cell_body
        )
    };
    format!(
        "#[agent(version = 1)]\nagent @agent.repl.cell_{index} repl_cell_{index}()\neffects {{ agent.observe, agent.act.semantic, agent.act.physical, agent.wait, agent.capture, agent.resource.read, debug.read, debug.record, rag.query }}\n{{\n{body}\n}}\n"
    )
}

fn agent_repl_live_binding_prelude(state: &AgentReplState) -> String {
    state
        .bindings
        .values()
        .filter(|binding| binding.status == "ok")
        .filter(|binding| binding.binding_kind == "local")
        .filter_map(|binding| {
            binding
                .serialized_source
                .as_ref()
                .map(|source| format!("let {} = {source}", binding.name))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_repl_serialized_bindings(
    fragment: &ParsedFragment,
) -> BTreeMap<String, AgentReplSerializedBinding> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return BTreeMap::new();
    };
    statements
        .iter()
        .filter_map(agent_repl_serialized_stmt_binding)
        .collect()
}

fn agent_repl_serialized_stmt_binding(
    statement: &Stmt,
) -> Option<(String, AgentReplSerializedBinding)> {
    let Stmt::Let {
        pattern,
        expr,
        expr_source,
        ..
    } = statement
    else {
        return None;
    };
    let name = agent_repl_single_binding_name(pattern)?;
    let binding = agent_repl_serialized_expr_binding(expr, expr_source.as_deref())?;
    Some((name, binding))
}

fn agent_repl_single_binding_name(pattern: &Pattern) -> Option<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn agent_repl_serialized_expr_source(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Literal(literal) => agent_repl_serialized_literal_source(literal),
        Expr::EntityRef(entity) => agent_repl_serialized_entity_ref_source(entity),
        Expr::BracketSeq(items) => items
            .iter()
            .map(agent_repl_serialized_expr_source)
            .collect::<Option<Vec<_>>>()
            .map(|items| format!("[{}]", items.join(", "))),
        _ => None,
    }
}

fn agent_repl_serialized_expr_binding(
    expr: &Expr,
    expr_source: Option<&str>,
) -> Option<AgentReplSerializedBinding> {
    agent_repl_serialized_expr_source(expr)
        .map(|source| AgentReplSerializedBinding {
            source,
            snapshot_kind: "literal".to_owned(),
        })
        .or_else(|| {
            let snapshot_kind = agent_repl_snapshot_expr_kind(expr)?;
            let source = expr_source?.trim();
            (!source.is_empty()).then(|| AgentReplSerializedBinding {
                source: source.to_owned(),
                snapshot_kind: snapshot_kind.to_owned(),
            })
        })
}

fn agent_repl_snapshot_expr_kind(expr: &Expr) -> Option<&'static str> {
    match expr {
        Expr::Try { expr } | Expr::Await { expr, .. } => agent_repl_snapshot_expr_kind(expr),
        Expr::Call { callee, args } if agent_repl_snapshot_call_args_are_self_contained(args) => {
            agent_repl_call_snapshot_kind(callee.as_ref())
        }
        Expr::MethodCall {
            receiver,
            method,
            args,
        } if agent_repl_snapshot_call_args_are_self_contained(args) => {
            agent_repl_method_snapshot_kind(receiver.as_ref(), method)
        }
        _ => None,
    }
}

fn agent_repl_call_snapshot_kind(callee: &Expr) -> Option<&'static str> {
    match callee {
        Expr::Path(name) if name == "observe" => Some("observation"),
        Expr::Path(name) if name == "read_resource" => Some("resource"),
        Expr::Field { target, field } if field == "query" => {
            agent_repl_method_snapshot_kind(target.as_ref(), field)
        }
        _ => None,
    }
}

fn agent_repl_method_snapshot_kind(receiver: &Expr, method: &str) -> Option<&'static str> {
    match (receiver, method) {
        (Expr::Path(namespace), "query") if namespace == "rag" => Some("rag_context"),
        _ => None,
    }
}

fn agent_repl_snapshot_call_args_are_self_contained(args: &[CallArg]) -> bool {
    args.iter().all(|arg| match arg {
        CallArg::Positional(expr) => agent_repl_serialized_expr_source(expr).is_some(),
        CallArg::Named { value, .. } | CallArg::Spread { value } => {
            agent_repl_serialized_expr_source(value).is_some()
        }
    })
}

fn agent_repl_serialized_literal_source(literal: &Literal) -> Option<String> {
    match literal {
        Literal::String(value) => serde_json::to_string(value).ok(),
        Literal::Char { raw, .. } => Some(raw.clone()),
        Literal::Int { raw, .. } | Literal::Float { raw, .. } | Literal::UnitNumber { raw, .. } => {
            Some(raw.clone())
        }
        Literal::Bool(value) => Some(value.to_string()),
        Literal::Duration { .. } => None,
    }
}

fn agent_repl_serialized_entity_ref_source(entity: &EntityRefSyntax) -> Option<String> {
    entity
        .as_absolute()
        .map(|entity| format!("@{}", entity.body()))
}

fn agent_repl_fragment_binding_names(fragment: &ParsedFragment) -> Vec<String> {
    let Some(ParsedFragmentKind::Statements(statements)) = fragment.kind() else {
        return Vec::new();
    };
    let mut names = statements
        .iter()
        .flat_map(agent_repl_stmt_binding_names)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    names
}

fn agent_repl_stmt_binding_names(statement: &Stmt) -> Vec<String> {
    match statement {
        Stmt::Let { pattern, .. }
        | Stmt::LetElse { pattern, .. }
        | Stmt::LetChoice { pattern, .. }
        | Stmt::LetScope { pattern, .. }
        | Stmt::LetLoop { pattern, .. }
        | Stmt::LetAwait { pattern, .. } => agent_repl_pattern_binding_names(pattern),
        Stmt::WhileLet { pattern, .. } | Stmt::For { pattern, .. } => {
            agent_repl_pattern_binding_names(pattern)
        }
        Stmt::Return(_)
        | Stmt::Out { .. }
        | Stmt::Goto(_)
        | Stmt::Thread(_)
        | Stmt::DeferBlock { .. }
        | Stmt::Defer { .. }
        | Stmt::Yield(_)
        | Stmt::Signal { .. }
        | Stmt::LifetimeSet { .. }
        | Stmt::Expr(_)
        | Stmt::Wait(_)
        | Stmt::On { .. }
        | Stmt::UnsafeLifetime { .. }
        | Stmt::If { .. }
        | Stmt::Match { .. }
        | Stmt::Loop { .. }
        | Stmt::While { .. }
        | Stmt::Close(_)
        | Stmt::Select(_)
        | Stmt::Break { .. }
        | Stmt::Continue { .. }
        | Stmt::Raw(_) => Vec::new(),
    }
}

fn agent_repl_pattern_binding_names(pattern: &Pattern) -> Vec<String> {
    match pattern {
        Pattern::Ident(name) | Pattern::MutIdent(name) | Pattern::Typed { name, .. } => {
            vec![name.clone()]
        }
        Pattern::Whole { name, pattern } => {
            let mut names = vec![name.clone()];
            names.extend(agent_repl_pattern_binding_names(pattern));
            names
        }
        Pattern::Tuple(items) => items
            .iter()
            .flat_map(agent_repl_pattern_binding_names)
            .collect(),
        Pattern::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| agent_repl_pattern_binding_names(field.pattern()))
            .collect(),
        Pattern::BracketSeq { items, rest } => {
            let mut names = items
                .iter()
                .flat_map(agent_repl_pattern_binding_names)
                .collect::<Vec<_>>();
            names.extend(rest.clone());
            names
        }
        Pattern::Variant { payload, .. } => payload
            .as_ref()
            .map(agent_repl_variant_payload_binding_names)
            .unwrap_or_default(),
        Pattern::Literal(_) | Pattern::Entity(_) | Pattern::Discard | Pattern::Raw(_) => Vec::new(),
    }
}

fn agent_repl_variant_payload_binding_names(payload: &VariantPatternPayload) -> Vec<String> {
    match payload {
        VariantPatternPayload::Tuple(items) => items
            .iter()
            .flat_map(agent_repl_pattern_binding_names)
            .collect(),
        VariantPatternPayload::Record { fields, .. } => fields
            .iter()
            .flat_map(|field| agent_repl_pattern_binding_names(field.pattern()))
            .collect(),
    }
}

fn indent_agent_repl_body(input: &str) -> String {
    input
        .lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn agent_repl_ok(
    index: usize,
    input: &str,
    kind: &str,
    value: serde_json::Value,
) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: kind.to_owned(),
        status: "ok".to_owned(),
        message: None,
        value: Some(value),
        quit: false,
    }
}

fn agent_repl_error(index: usize, input: &str, kind: &str, message: String) -> AgentReplCellReport {
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: kind.to_owned(),
        status: "error".to_owned(),
        message: Some(message),
        value: None,
        quit: false,
    }
}

fn agent_repl_print_cell(report: &AgentReplCellReport) {
    match report.status.as_str() {
        "ok" | "parsed" => {
            if let Some(value) = &report.value {
                println!("{}: {}", report.status, value);
            } else {
                println!("{}", report.status);
            }
        }
        _ => {
            eprintln!(
                "error: {}",
                report.message.as_deref().unwrap_or("REPL cell failed")
            );
        }
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
    image_frames: AgentImageFrameStore,
    capture_resources: Vec<AgentResource>,
    trace_resources: Vec<AgentResource>,
    rag_context_packs: Vec<RagContextPack>,
    native_capture_session: Option<arcweft_render_native::NativeOffscreenCaptureSession>,
    runtime: Option<NativeAgentRuntimeState>,
    observe_options: Option<AgentObserveOptions>,
}

struct AgentObservationState {
    report: AgentObservationReport,
    image_frames: AgentImageFrameStore,
    native_session: arcweft_render_native::NativeOffscreenCaptureSession,
}

struct AgentObservationRunOutput {
    report: AgentObservationReport,
    image_frames: AgentImageFrameStore,
}

struct AgentObservationRunContext<'a> {
    host_config: NativeRunHost<'a>,
    options: &'a AgentObserveOptions,
    source_path: &'a Path,
    native_session: Option<&'a mut arcweft_render_native::NativeOffscreenCaptureSession>,
    tick_offset: usize,
    input_events: Vec<RuntimeInputEvent>,
    task_events: &'a mut Vec<TaskEvent>,
}

struct NativeAgentObservedSnapshot {
    report: AgentObservationReport,
    image_frames: AgentImageFrameStore,
}

struct NativeAgentRuntimeState {
    executor: RuntimeExecutorInstance,
    catalog: LineDisplayCatalog,
    source_path: PathBuf,
    host_policy: HostCallPolicy,
    native_session: arcweft_render_native::NativeOffscreenCaptureSession,
    task_events: Vec<TaskEvent>,
    next_tick: usize,
}

struct AgentMcpObservation {
    report: AgentObservationReport,
    image_output: Option<AgentImageOutput>,
    image_frames: AgentImageFrameStore,
    runtime: NativeAgentRuntimeState,
    options: AgentObserveOptions,
}

struct AgentMcpFrame {
    report: AgentObservationReport,
    image_output: Option<AgentImageOutput>,
    image_frames: AgentImageFrameStore,
    resources: Vec<AgentResource>,
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
    let resources = agent_mcp_current_resources(state)
        .map_err(|_| "failed to build Agent resource list".to_owned())?;
    serde_json::to_value(list_resources_result(&resources))
        .map_err(|error| format!("failed to serialize MCP resource list: {error}"))
}

fn agent_mcp_resource_read(
    params: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<serde_json::Value, String> {
    let max_privacy = agent_mcp_max_privacy_argument(params, "resources/read")?;
    let uri = params
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "resources/read requires params.uri".to_owned())?;
    if let Some(resource) = agent_mcp_session_context_resource_for_uri(state, uri)
        .map_err(|error| format!("failed to build Agent session context: {error}"))?
    {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
        {
            return Err(agent_mcp_resource_read_privacy_message(&error));
        }
        let read = read_resource_result(&resource)
            .map_err(|error| format!("failed to serialize MCP session context: {error}"))?;
        return serde_json::to_value(read)
            .map_err(|error| format!("failed to serialize MCP read: {error}"));
    }
    if let Some(resource) = agent_mcp_cached_trace_resource(state, uri) {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
        {
            return Err(agent_mcp_resource_read_privacy_message(&error));
        }
        let read = read_resource_result(&resource)
            .map_err(|error| format!("failed to serialize MCP resource: {error}"))?;
        return serde_json::to_value(read)
            .map_err(|error| format!("failed to serialize MCP read: {error}"));
    }
    let Some(report) = state.report.clone() else {
        return Err(
            "resources/read requires a prior arcweft.observe call or arcweft.trace.read call"
                .to_owned(),
        );
    };
    let image_output = state.image_output.clone();
    let resource = if let Some(resource) = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(&report, image_output.as_ref(), uri))
    {
        resource
    } else {
        agent_mcp_uncached_resource_by_uri(&report, uri, state)
            .map_err(|_| format!("failed to read Agent resource `{uri}`"))?
    };
    if let Some(error) =
        agent_mcp_resource_read_privacy_error(&resource, max_privacy, "resources/read")
    {
        return Err(agent_mcp_resource_read_privacy_message(&error));
    }
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
        "arcweft.action" => {
            let tool = agent_mcp_call_action(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP action result: {error}"))
        }
        "arcweft.wait" => {
            let tool = agent_mcp_call_wait(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP wait result: {error}"))
        }
        "arcweft.script.run" => {
            let tool = agent_mcp_call_script_run(&arguments, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP script run result: {error}"))
        }
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
        "arcweft.hit_test" => {
            let tool = agent_mcp_call_hit_test(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP hit-test result: {error}"))
        }
        "arcweft.get_state" => {
            let tool = agent_mcp_call_get_state(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP state result: {error}"))
        }
        "arcweft.signal_get" => {
            let tool = agent_mcp_call_signal_get(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP signal result: {error}"))
        }
        "arcweft.log_query" => {
            let tool = agent_mcp_call_log_query(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP log result: {error}"))
        }
        tool if tool.starts_with("arcweft.debug.") => agent_mcp_debug_tool_call(tool, &arguments),
        "arcweft.rag.query" => {
            let tool = agent_mcp_call_rag_query(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG result: {error}"))
        }
        "arcweft.rag.explain" => {
            let tool = agent_mcp_call_rag_explain(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG explanation: {error}"))
        }
        "arcweft.rag.context.read" => {
            let tool = agent_mcp_call_rag_context_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP RAG context item: {error}"))
        }
        "arcweft.trace.read" => {
            let tool = agent_mcp_call_trace_read(&arguments, state)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP trace result: {error}"))
        }
        tool => Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    }
}

fn agent_mcp_debug_tool_call(
    name: &str,
    arguments: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let (tool, label) = match name {
        "arcweft.debug.search" => (
            agent_mcp_call_debug_search(arguments)?,
            "MCP debug search result",
        ),
        "arcweft.debug.script.runs" => (
            agent_mcp_call_debug_script_runs(arguments)?,
            "MCP debug script runs",
        ),
        "arcweft.debug.sessions.close_stale" => (
            agent_mcp_call_debug_close_stale_sessions(arguments)?,
            "MCP debug close stale sessions",
        ),
        "arcweft.debug.session.timeline" => (
            agent_mcp_call_debug_session_timeline(arguments)?,
            "MCP debug timeline",
        ),
        "arcweft.debug.repl.cells" => (
            agent_mcp_call_debug_repl_cells(arguments)?,
            "MCP debug REPL cells",
        ),
        "arcweft.debug.source.files" => (
            agent_mcp_call_debug_source_files(arguments)?,
            "MCP debug source files",
        ),
        "arcweft.debug.graph.inventory" => (
            agent_mcp_call_debug_graph_inventory(arguments)?,
            "MCP debug graph inventory",
        ),
        tool => return Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    };
    serde_json::to_value(tool).map_err(|error| format!("failed to serialize {label}: {error}"))
}

fn agent_mcp_call_observe(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
    agent_mcp_store_observation(state, observed);
    let resources = agent_mcp_current_resources(state)
        .map_err(|_| "failed to build Agent session resource list".to_owned())?;
    let tool = tool_result_for_resources(&resources);
    serde_json::to_value(tool)
        .map_err(|error| format!("failed to serialize MCP tool result: {error}"))
}

fn agent_mcp_store_observation(state: &mut AgentMcpState, observed: AgentMcpObservation) {
    state.report = Some(observed.report);
    state.image_output = observed.image_output;
    state.image_frames = observed.image_frames;
    state.runtime = Some(observed.runtime);
    state.observe_options = Some(observed.options);
    state.native_capture_session = None;
    state.capture_resources.clear();
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
            "native_capture_session_active": state.runtime.is_some() || state.native_capture_session.is_some(),
            "latest_capture": latest_capture.and_then(|resource| resource.image.as_ref()),
            "latest_capture_uri": latest_capture.map(|resource| resource.uri.as_str()),
            "latest_capture_resource": latest_capture_descriptor,
            "trace_resource_count": state.trace_resources.len(),
        })
    } else {
        let descriptors = list_resources_result(&state.trace_resources).resources;
        serde_json::json!({
            "observed": false,
            "resource_count": descriptors.len(),
            "resources": descriptors,
            "resource_templates": list_resource_templates_result().resource_templates,
            "images": [],
            "layers": [],
            "objects": [],
            "capture_resource_count": 0,
            "trace_resource_count": state.trace_resources.len(),
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

fn agent_mcp_call_hit_test(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    let x = agent_mcp_u32_argument(arguments, "x", "arcweft.hit_test")?
        .ok_or_else(|| "arcweft.hit_test requires arguments.x".to_owned())?;
    let y = agent_mcp_u32_argument(arguments, "y", "arcweft.hit_test")?
        .ok_or_else(|| "arcweft.hit_test requires arguments.y".to_owned())?;
    if agent_mcp_arguments_request_observe(arguments) {
        let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
        agent_mcp_store_observation(state, observed);
    }
    let Some(report) = &state.report else {
        return Err(
            "arcweft.hit_test requires a prior arcweft.observe call, arguments.source, or arguments.profile"
                .to_owned(),
        );
    };
    let hit_test = agent_hit_test_report(report, x, y);
    let text = serde_json::to_string_pretty(&hit_test)
        .map_err(|error| format!("failed to serialize hit-test result: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

fn agent_mcp_call_action(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let before = state.report.clone().ok_or_else(|| {
        "arcweft.action requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let action = agent_mcp_action_argument(arguments, &before)?;
    let input_events = native_agent_action_input_events(&before, action.clone())
        .map_err(|error| error.to_string())?;
    let options = state
        .observe_options
        .clone()
        .ok_or_else(|| "arcweft.action requires an active native observation session".to_owned())?;
    let frame = {
        let runtime = state
            .runtime
            .as_mut()
            .ok_or_else(|| "arcweft.action requires an active native runtime session".to_owned())?;
        agent_mcp_observe_runtime(runtime, &options, input_events, adapter_registrars)?
    };
    let result = NativeAgentScriptSession::action_result(&before, &frame.report);
    let value = serde_json::json!({
        "accepted": result.accepted,
        "before_tick": result.before_tick,
        "after_tick": result.after_tick,
        "before_state_hash": result.before_state_hash,
        "after_state_hash": result.after_state_hash,
        "action": action,
        "after": {
            "tick": frame.report.tick,
            "frame_id": frame.report.frame_id,
            "state_hash": frame.report.state_hash,
            "render_hash": frame.report.render_hash,
            "final_status": frame.report.final_status,
            "actions": frame.report.actions,
        },
        "resource_count": frame.resources.len(),
    });
    agent_mcp_store_frame(state, frame);
    agent_mcp_json_tool_result(&value, "action")
}

fn agent_mcp_call_wait(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let predicate = arguments
        .get("predicate")
        .ok_or_else(|| "arcweft.wait requires arguments.predicate".to_owned())
        .and_then(|value| {
            serde_json::from_value::<Predicate>(value.clone())
                .map_err(|error| format!("invalid arcweft.wait predicate: {error}"))
        })?;
    let timeout_millis = agent_mcp_u64_argument(arguments, "timeout_millis", "arcweft.wait")?
        .ok_or_else(|| "arcweft.wait requires arguments.timeout_millis".to_owned())?;
    let stable_frames =
        agent_mcp_u32_argument(arguments, "stable_frames", "arcweft.wait")?.unwrap_or(1);
    let poll_frames =
        agent_mcp_u32_argument(arguments, "poll_frames", "arcweft.wait")?.unwrap_or(1);
    let max_polls = (timeout_millis / u64::from(poll_frames.max(1))).max(1);
    let mut stable_seen = 0u32;
    let mut last_value = None;

    for poll_index in 0..max_polls {
        let options = state
            .observe_options
            .clone()
            .ok_or_else(|| "arcweft.wait requires an active native observation session".to_owned())
            .map(|mut options| {
                options.steps = usize::try_from(poll_frames.max(1)).unwrap_or(usize::MAX);
                options.capture_step = None;
                options
            })?;
        let frame = {
            let runtime = state.runtime.as_mut().ok_or_else(|| {
                "arcweft.wait requires an active native runtime session".to_owned()
            })?;
            agent_mcp_observe_runtime(runtime, &options, Vec::new(), adapter_registrars)?
        };
        let matched = agent_mcp_predicate_matches(&predicate, &frame.report);
        let summary =
            agent_mcp_wait_report_value(&frame.report, matched, stable_seen, poll_index + 1);
        stable_seen = if matched {
            stable_seen.saturating_add(1)
        } else {
            0
        };
        let done = stable_seen >= stable_frames.max(1);
        let resources_len = frame.resources.len();
        agent_mcp_store_frame(state, frame);
        last_value = Some(serde_json::json!({
            "matched": matched,
            "stable_seen": stable_seen,
            "stable_required": stable_frames.max(1),
            "polls": poll_index + 1,
            "timeout_millis": timeout_millis,
            "poll_frames": poll_frames.max(1),
            "resource_count": resources_len,
            "observation": summary,
        }));
        if done {
            return agent_mcp_json_tool_result(
                last_value.as_ref().expect("wait result exists"),
                "wait result",
            );
        }
    }

    let value = last_value.unwrap_or_else(|| {
        serde_json::json!({
            "matched": false,
            "stable_seen": 0,
            "stable_required": stable_frames.max(1),
            "polls": 0,
            "timeout_millis": timeout_millis,
            "poll_frames": poll_frames.max(1),
        })
    });
    agent_mcp_json_tool_error(&value, "wait timeout")
}

fn agent_mcp_call_script_run(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    let options = agent_mcp_script_run_options(arguments)?;
    let report = match agent_script_run_input(&options) {
        Ok(input) => agent_script_run_bundle(&options, &input, adapter_registrars)
            .map_err(|code| format!("arcweft.script.run failed with exit code {code:?}"))?,
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
    let value = serde_json::to_value(&report)
        .map_err(|error| format!("failed to serialize Agent Script run report: {error}"))?;
    if report.ok {
        agent_mcp_json_tool_result(&value, "script run")
    } else {
        agent_mcp_json_tool_error(&value, "script run")
    }
}

fn agent_mcp_script_run_options(
    arguments: &serde_json::Value,
) -> Result<AgentScriptRunOptions, String> {
    let path = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from)
        .ok_or_else(|| "arcweft.script.run requires arguments.path".to_owned())?;
    let native_source = arguments
        .get("native_source")
        .and_then(serde_json::Value::as_str)
        .map(PathBuf::from);
    let profile = arguments
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if native_source.is_some() && profile.is_some() {
        return Err(
            "arcweft.script.run arguments.native_source and arguments.profile are mutually exclusive"
                .to_owned(),
        );
    }
    Ok(AgentScriptRunOptions {
        path,
        json: true,
        native_source,
        native_profile: ProfileOptions {
            profile,
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
        executor: agent_mcp_value_enum_argument(arguments, "executor", "arcweft.script.run")?
            .unwrap_or(CliRuntimeExecutorTier::BytecodeVm),
        pure_backend: agent_mcp_value_enum_argument(
            arguments,
            "pure_backend",
            "arcweft.script.run",
        )?,
        pure_workers: agent_mcp_pure_workers_argument(arguments, "arcweft.script.run")?,
        pure_batch_min_len: agent_mcp_usize_argument(arguments, "pure_batch_min_len"),
        pure_object_artifacts: agent_mcp_bool_argument(
            arguments,
            "pure_object_artifacts",
            "arcweft.script.run",
        )?
        .unwrap_or(false),
        math_backend: agent_mcp_value_enum_argument(
            arguments,
            "math_backend",
            "arcweft.script.run",
        )?,
        math_wgpu_min_elements: agent_mcp_usize_argument(arguments, "math_wgpu_min_elements"),
        native_steps: agent_mcp_usize_argument(arguments, "native_steps").unwrap_or(8),
        native_mode: agent_mcp_value_enum_argument(arguments, "native_mode", "arcweft.script.run")?
            .unwrap_or(CliRuntimeStepMode::Drain),
        native_max_ops: agent_mcp_usize_argument(arguments, "native_max_ops").unwrap_or(64),
        values: agent_mcp_runtime_bindings(arguments)?,
        viewport_width: agent_mcp_u32_argument(arguments, "viewport_width", "arcweft.script.run")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
        viewport_height: agent_mcp_u32_argument(
            arguments,
            "viewport_height",
            "arcweft.script.run",
        )?
        .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        textbox_height: agent_mcp_u32_argument(arguments, "textbox_height", "arcweft.script.run")?,
        capture_time_seconds: agent_mcp_capture_time_argument(arguments, "arcweft.script.run")?,
        max_steps: agent_mcp_usize_argument(arguments, "max_steps").unwrap_or(256),
        max_ops: agent_mcp_usize_argument(arguments, "max_ops").unwrap_or(1024),
        signals: agent_mcp_script_signal_args(arguments)?,
        states: agent_mcp_script_state_args(arguments)?,
        trace_out: arguments
            .get("trace_out")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        blob_dir: arguments
            .get("blob_dir")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        debug_db: arguments
            .get("debug_db")
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        run_id: arguments
            .get("run_id")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("run.cli")
            .to_owned(),
    })
}

fn agent_mcp_script_signal_args(
    arguments: &serde_json::Value,
) -> Result<Vec<super::AgentScriptSignalArg>, String> {
    agent_mcp_script_key_value_args(arguments, "signals", "arcweft.script.run signals").and_then(
        |values| {
            values
                .iter()
                .map(|value| parse_agent_script_signal_arg(value))
                .collect()
        },
    )
}

fn agent_mcp_script_state_args(
    arguments: &serde_json::Value,
) -> Result<Vec<super::AgentScriptStateArg>, String> {
    agent_mcp_script_key_value_args(arguments, "state", "arcweft.script.run state").and_then(
        |values| {
            values
                .iter()
                .map(|value| parse_agent_script_state_arg(value))
                .collect()
        },
    )
}

fn agent_mcp_script_key_value_args(
    arguments: &serde_json::Value,
    name: &str,
    context: &str,
) -> Result<Vec<String>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| format!("{context} must be a JSON object"))?;
    object
        .iter()
        .map(|(key, value)| {
            Ok(format!(
                "{key}={}",
                agent_mcp_script_scalar_arg(value, context)?
            ))
        })
        .collect()
}

fn agent_mcp_script_scalar_arg(value: &serde_json::Value, context: &str) -> Result<String, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => serde_json::to_string(value)
            .map_err(|error| format!("failed to serialize {context} string: {error}")),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(format!("{context} values must be bool, string, or number"))
        }
    }
}

fn agent_mcp_runtime_bindings(
    arguments: &serde_json::Value,
) -> Result<Vec<arcweft_core::value::RuntimeBinding>, String> {
    let Some(value) = arguments.get("values") else {
        return Ok(Vec::new());
    };
    let object = value
        .as_object()
        .ok_or_else(|| "arcweft.script.run values must be a JSON object".to_owned())?;
    object
        .iter()
        .map(|(key, value)| {
            parse_runtime_binding_arg(&format!(
                "{key}={}",
                agent_mcp_runtime_value_arg(value, "arcweft.script.run values")?
            ))
        })
        .collect()
}

fn agent_mcp_runtime_value_arg(value: &serde_json::Value, context: &str) -> Result<String, String> {
    match value {
        serde_json::Value::Bool(value) => Ok(value.to_string()),
        serde_json::Value::Number(value) => Ok(value.to_string()),
        serde_json::Value::String(value) => Ok(value.clone()),
        serde_json::Value::Null | serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
            Err(format!("{context} values must be bool, string, or number"))
        }
    }
}

fn agent_mcp_value_enum_argument<T>(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<T>, String>
where
    T: ValueEnum,
{
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{tool} argument {name} must be a string"))?;
    T::from_str(value, true)
        .map(Some)
        .map_err(|error| format!("{tool} argument {name} is invalid: {error}"))
}

fn agent_mcp_pure_workers_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<Option<CliRuntimePureWorkers>, String> {
    let Some(value) = arguments.get("pure_workers") else {
        return Ok(None);
    };
    let raw = match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Number(value) if value.is_u64() => value.to_string(),
        _ => {
            return Err(format!(
                "{tool} argument pure_workers must be `auto` or a positive integer"
            ));
        }
    };
    parse_runtime_pure_workers(&raw).map(Some)
}

fn agent_mcp_bool_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<bool>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    value
        .as_bool()
        .ok_or_else(|| format!("{tool} argument {name} must be a boolean"))
        .map(Some)
}

fn agent_mcp_store_frame(state: &mut AgentMcpState, frame: AgentMcpFrame) {
    state.report = Some(frame.report);
    state.image_output = frame.image_output;
    state.image_frames = frame.image_frames;
    state.capture_resources.clear();
}

fn agent_mcp_action_argument(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
) -> Result<AgentAction, String> {
    if let Some(action_id) = arguments
        .get("action_id")
        .and_then(serde_json::Value::as_str)
    {
        let target = report
            .actions
            .iter()
            .find(|candidate| candidate.id == action_id)
            .ok_or_else(|| format!("arcweft.action action_id `{action_id}` is not observed"))?;
        return agent_mcp_action_from_target(arguments, target);
    }
    let kind = arguments
        .get("kind")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            "arcweft.action requires arguments.action_id or arguments.kind".to_owned()
        })?;
    match kind {
        "advance_text" => Ok(AgentAction::AdvanceText),
        "select_choice" => {
            let target = agent_mcp_public_id_argument(arguments, "target", "arcweft.action")?;
            Ok(AgentAction::SelectChoice { choice: target })
        }
        "invoke" => {
            let target = agent_mcp_public_id_argument(arguments, "target", "arcweft.action")?;
            let action = arguments
                .get("action")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "arcweft.action kind invoke requires arguments.action".to_owned())?
                .to_owned();
            let args = agent_mcp_action_args(arguments)?;
            Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
                target,
                action,
                args: Box::new(args),
            })))
        }
        _ => Err(format!(
            "arcweft.action kind must be one of advance_text, select_choice, or invoke: `{kind}`"
        )),
    }
}

fn agent_mcp_action_args(
    arguments: &serde_json::Value,
) -> Result<BTreeMap<String, AgentValue>, String> {
    let Some(args) = arguments.get("args") else {
        return Ok(BTreeMap::new());
    };
    let values = args
        .as_object()
        .ok_or_else(|| "arcweft.action arguments.args must be an object".to_owned())?;
    values
        .iter()
        .map(|(key, value)| Ok((key.clone(), agent_mcp_agent_value(value)?)))
        .collect()
}

fn agent_mcp_agent_value(value: &serde_json::Value) -> Result<AgentValue, String> {
    match value {
        serde_json::Value::Null => Ok(AgentValue::Null),
        serde_json::Value::Bool(value) => Ok(AgentValue::Bool(*value)),
        serde_json::Value::Number(value) => value
            .as_i64()
            .map(AgentValue::I64)
            .or_else(|| value.as_u64().map(AgentValue::U64))
            .or_else(|| value.as_f64().map(AgentValue::F64))
            .ok_or_else(|| format!("arcweft.action arguments.args number is not finite: {value}")),
        serde_json::Value::String(value) => Ok(AgentValue::String(value.clone())),
        serde_json::Value::Array(values) => values
            .iter()
            .map(agent_mcp_agent_value)
            .collect::<Result<Vec<_>, _>>()
            .map(AgentValue::List),
        serde_json::Value::Object(values) => values
            .iter()
            .map(|(key, value)| Ok((key.clone(), agent_mcp_agent_value(value)?)))
            .collect::<Result<BTreeMap<_, _>, _>>()
            .map(AgentValue::Map),
    }
}

fn agent_mcp_action_from_target(
    arguments: &serde_json::Value,
    target: &AgentActionTarget,
) -> Result<AgentAction, String> {
    match target.action {
        AgentActionKind::AdvanceText => {
            agent_mcp_reject_action_args_for_non_invoke(arguments)?;
            Ok(AgentAction::AdvanceText)
        }
        AgentActionKind::SelectChoice => {
            agent_mcp_reject_action_args_for_non_invoke(arguments)?;
            Ok(AgentAction::SelectChoice {
                choice: agent_mcp_public_id_from_str(&target.target)?,
            })
        }
        AgentActionKind::Invoke => Ok(AgentAction::Invoke(Box::new(AgentInvokeAction {
            target: agent_mcp_public_id_from_str(&target.target)?,
            action: target.id.clone(),
            args: Box::new(agent_mcp_action_args(arguments)?),
        }))),
        AgentActionKind::PointerClick => {
            Err("arcweft.action does not synthesize physical pointer_click actions".to_owned())
        }
    }
}

fn agent_mcp_reject_action_args_for_non_invoke(
    arguments: &serde_json::Value,
) -> Result<(), String> {
    if arguments.get("args").is_some() {
        return Err("arcweft.action arguments.args is only valid for invoke actions".to_owned());
    }
    Ok(())
}

fn agent_mcp_public_id_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<PublicId, String> {
    let value = arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| format!("{tool} requires arguments.{name}"))?;
    agent_mcp_public_id_from_str(value)
}

fn agent_mcp_public_id_from_str(value: &str) -> Result<PublicId, String> {
    PublicId::new(value.trim_start_matches('@').to_owned()).map_err(|error| error.to_string())
}

fn agent_mcp_call_get_state(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.get_state")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("state", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "state privacy");
    }
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.get_state requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let summary = agent_mcp_observation_state_summary(report);
    let value = if let Some(path) = arguments.get("path").and_then(serde_json::Value::as_str) {
        let selected = agent_json_path(&summary, path).cloned();
        serde_json::json!({
            "path": path,
            "found": selected.is_some(),
            "value": selected,
            "tick": report.tick,
            "state_hash": report.state_hash,
        })
    } else {
        summary
    };
    agent_mcp_json_tool_result(&value, "state")
}

fn agent_mcp_call_signal_get(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.signal_get")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("signal", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "signal privacy");
    }
    let name = arguments
        .get("name")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.signal_get requires arguments.name".to_owned())?;
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.signal_get requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let signal = report.signals.iter().find(|signal| signal.name == name);
    let value = serde_json::json!({
        "name": name,
        "found": signal.is_some(),
        "value": signal.map(|signal| signal.value.as_str()),
        "tick": report.tick,
        "state_hash": report.state_hash,
    });
    agent_mcp_json_tool_result(&value, "signal")
}

fn agent_mcp_call_log_query(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.log_query")?;
    if let Some(error) =
        agent_mcp_observation_debug_read_privacy_error("logs", PrivacyClass::Project, max_privacy)
    {
        return agent_mcp_json_tool_error(&error, "logs privacy");
    }
    let report = state.report.as_ref().ok_or_else(|| {
        "arcweft.log_query requires a prior arcweft.observe call, arguments.source, or arguments.profile"
            .to_owned()
    })?;
    let level = arguments.get("level").and_then(serde_json::Value::as_str);
    let contains = arguments
        .get("contains")
        .and_then(serde_json::Value::as_str);
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    let logs = report
        .logs
        .iter()
        .filter(|log| level.is_none_or(|level| log.level == level))
        .filter(|log| contains.is_none_or(|needle| log.message.contains(needle)))
        .take(limit)
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "tick": report.tick,
        "state_hash": report.state_hash,
        "level": level,
        "contains": contains,
        "limit": limit,
        "count": logs.len(),
        "logs": logs,
    });
    agent_mcp_json_tool_result(&value, "logs")
}

fn agent_mcp_call_debug_search(arguments: &serde_json::Value) -> Result<McpCallToolResult, String> {
    let query = agent_mcp_non_empty_string_argument(arguments, "query");
    let query_vector = agent_mcp_query_vector_argument(arguments, "query_vector")?;
    let graph_query = agent_mcp_non_empty_string_argument(arguments, "graph_query");
    let history_query = agent_mcp_non_empty_string_argument(arguments, "history_query");
    let diagnostic_query = agent_mcp_non_empty_string_argument(arguments, "diagnostic_query");
    let test_query = agent_mcp_non_empty_string_argument(arguments, "test_query");
    let selector_count = usize::from(query.is_some())
        + usize::from(query_vector.is_some())
        + usize::from(graph_query.is_some())
        + usize::from(history_query.is_some())
        + usize::from(diagnostic_query.is_some())
        + usize::from(test_query.is_some());
    if selector_count == 0 {
        return Err(
            "arcweft.debug.search requires one of query, query_vector, graph_query, history_query, diagnostic_query, or test_query"
                .to_owned(),
        );
    }
    if selector_count > 1 {
        return Err(
            "arcweft.debug.search accepts only one of query, query_vector, graph_query, history_query, diagnostic_query, or test_query"
                .to_owned(),
        );
    }
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(10);
    if limit == 0 {
        return Err("arcweft.debug.search argument limit must be at least 1".to_owned());
    }
    let graph_depth =
        agent_mcp_u32_argument(arguments, "graph_depth", "arcweft.debug.search")?.unwrap_or(1);
    let max_privacy = agent_mcp_privacy_class_argument(arguments, "max_privacy")?
        .unwrap_or(PrivacyClass::Project);
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.search failed to open `{path}`: {error}"))?;
    let request = AgentMcpDebugSearchRequest {
        query,
        query_vector: query_vector.as_deref(),
        graph_query,
        history_query,
        diagnostic_query,
        test_query,
        graph_depth,
        limit,
        max_privacy,
    };
    let hits = agent_mcp_debug_search_hits(&store, &request, arguments)
        .map_err(|error| format!("arcweft.debug.search failed to search `{path}`: {error}"))?
        .iter()
        .map(agent_mcp_debug_search_hit_json)
        .collect::<Vec<_>>();
    let value = serde_json::json!({
        "path": path,
        "query": query,
        "query_vector_dimensions": query_vector.as_ref().map(Vec::len),
        "graph_query": graph_query,
        "graph_depth": graph_query.map(|_| graph_depth),
        "history_query": history_query,
        "diagnostic_query": diagnostic_query,
        "test_query": test_query,
        "limit": limit,
        "max_privacy": max_privacy.as_str(),
        "hits": hits,
    });
    agent_mcp_json_tool_result(&value, "debug search")
}

struct AgentMcpDebugSearchRequest<'a> {
    query: Option<&'a str>,
    query_vector: Option<&'a [f32]>,
    graph_query: Option<&'a str>,
    history_query: Option<&'a str>,
    diagnostic_query: Option<&'a str>,
    test_query: Option<&'a str>,
    graph_depth: u32,
    limit: usize,
    max_privacy: PrivacyClass,
}

fn agent_mcp_debug_search_hits(
    store: &DebugStore,
    request: &AgentMcpDebugSearchRequest<'_>,
    arguments: &serde_json::Value,
) -> Result<Vec<ChunkSearchResult>, String> {
    if let Some(query) = request.query {
        return store
            .lexical_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.graph_query {
        return store
            .graph_search_with_depth_and_max_privacy(
                query,
                request.graph_depth,
                request.limit,
                request.max_privacy,
            )
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.history_query {
        return store
            .history_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.diagnostic_query {
        return store
            .diagnostic_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    if let Some(query) = request.test_query {
        return store
            .test_result_search_with_max_privacy(query, request.limit, request.max_privacy)
            .map_err(|error| error.to_string());
    }
    let vector = request
        .query_vector
        .expect("debug search selector validation requires a vector");
    let model = agent_mcp_debug_search_model(arguments, vector.len())?;
    store
        .vector_search_with_max_privacy(&model, vector, request.limit, request.max_privacy)
        .map_err(|error| error.to_string())
}

fn agent_mcp_debug_search_hit_json(result: &ChunkSearchResult) -> serde_json::Value {
    serde_json::json!({
        "chunk_id": result.hit.chunk_id.as_str(),
        "title": result.title,
        "body": result.body,
        "source_kind": result.source_kind,
        "source_key": result.source_key,
        "privacy": result.privacy.as_str(),
        "channel": agent_mcp_search_channel_label(result.hit.channel),
        "rank": result.hit.rank,
        "score": result.hit.score,
    })
}

fn agent_mcp_call_debug_script_runs(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(20);
    if limit == 0 {
        return Err("arcweft.debug.script.runs argument limit must be at least 1".to_owned());
    }
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id")
        .map(SessionId::new)
        .transpose()
        .map_err(|error| format!("arcweft.debug.script.runs invalid session_id: {error}"))?;
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.script.runs failed to open `{path}`: {error}"))?;
    let runs = store
        .script_runs(session_id.as_ref(), limit)
        .map_err(|error| format!("arcweft.debug.script.runs failed to read `{path}`: {error}"))?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id.as_ref().map(SessionId::as_str),
        "limit": limit,
        "runs": runs.iter().map(agent_mcp_debug_script_run_json).collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug script runs")
}

fn agent_mcp_debug_script_run_json(run: &DebugScriptRun) -> serde_json::Value {
    serde_json::json!({
        "run_id": run.run_id.as_str(),
        "session_id": run.session_id.as_str(),
        "agent_id": run.agent_id.as_ref().map(PublicId::as_str),
        "artifact_hash": run.artifact_hash.as_ref().map(StableHash::as_str),
        "source_hash": run.source_hash.as_ref().map(StableHash::as_str),
        "project_binding_mode": &run.project_binding_mode,
        "started_sequence": run.started_sequence,
        "finished_sequence": run.finished_sequence,
        "outcome": run.outcome.as_str(),
        "partially_effectful": run.partially_effectful,
        "trace_uri": &run.trace_uri,
        "error": &run.error,
        "metadata": &run.metadata,
    })
}

fn agent_mcp_call_debug_close_stale_sessions(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let stale_after_millis = agent_mcp_u64_argument(
        arguments,
        "stale_after_millis",
        "arcweft.debug.sessions.close_stale",
    )?
    .ok_or_else(|| {
        "arcweft.debug.sessions.close_stale requires arguments.stale_after_millis".to_owned()
    })?;
    if stale_after_millis == 0 {
        return Err(
            "arcweft.debug.sessions.close_stale argument stale_after_millis must be at least 1"
                .to_owned(),
        );
    }
    let stale_after_millis = i64::try_from(stale_after_millis).map_err(|_| {
        "arcweft.debug.sessions.close_stale argument stale_after_millis is too large".to_owned()
    })?;
    let reason =
        agent_mcp_non_empty_string_argument(arguments, "reason").unwrap_or("stale_running_session");
    let dry_run = arguments
        .get("dry_run")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let now_unix_ms = agent_mcp_current_unix_millis();
    let cutoff_unix_ms = now_unix_ms.saturating_sub(stale_after_millis);
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.sessions.close_stale failed to open `{path}`: {error}")
    })?;
    let matched_sessions = store
        .stale_running_sessions(cutoff_unix_ms)
        .map_err(|error| {
            format!("arcweft.debug.sessions.close_stale failed to read `{path}`: {error}")
        })?;
    let closed_sessions = if dry_run {
        Vec::new()
    } else {
        store
            .abandon_stale_running_sessions(cutoff_unix_ms, now_unix_ms, reason)
            .map_err(|error| {
                format!("arcweft.debug.sessions.close_stale failed to update `{path}`: {error}")
            })?
    };
    let value = serde_json::json!({
        "path": path,
        "stale_after_millis": stale_after_millis,
        "cutoff_unix_ms": cutoff_unix_ms,
        "closed_unix_ms": now_unix_ms,
        "reason": reason,
        "dry_run": dry_run,
        "matched_sessions": matched_sessions
            .iter()
            .map(agent_mcp_debug_session_json)
            .collect::<Vec<_>>(),
        "closed_sessions": closed_sessions
            .iter()
            .map(agent_mcp_debug_session_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug close stale sessions")
}

fn agent_mcp_debug_session_json(session: &DebugSession) -> serde_json::Value {
    serde_json::json!({
        "session_id": session.session_id.as_str(),
        "program_hash": session.program_hash.as_ref().map(StableHash::as_str),
        "profile": &session.profile,
        "transport": &session.transport,
        "started_unix_ms": session.started_unix_ms,
        "ended_unix_ms": session.ended_unix_ms,
        "status": session.status.as_str(),
        "metadata": &session.metadata,
    })
}

fn agent_mcp_current_unix_millis() -> i64 {
    let Ok(duration) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return 0;
    };
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

fn agent_mcp_call_debug_session_timeline(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    if limit == 0 {
        return Err("arcweft.debug.session.timeline argument limit must be at least 1".to_owned());
    }
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.session.timeline")?;
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id");
    let run_id = agent_mcp_non_empty_string_argument(arguments, "run_id");
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.session.timeline failed to open `{path}`: {error}")
    })?;
    let events = store
        .session_timeline_with_max_privacy(session_id, run_id, limit, max_privacy)
        .map_err(|error| {
            format!("arcweft.debug.session.timeline failed to read `{path}`: {error}")
        })?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id,
        "run_id": run_id,
        "limit": limit,
        "max_privacy": max_privacy.as_str(),
        "events": events.iter().map(agent_mcp_debug_timeline_event_json).collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug session timeline")
}

fn agent_mcp_debug_timeline_event_json(event: &DebugTimelineEvent) -> serde_json::Value {
    serde_json::json!({
        "session_id": &event.session_id,
        "run_id": &event.run_id,
        "sequence": event.sequence,
        "tick": event.tick,
        "kind": &event.event_kind,
        "privacy": event.privacy.as_str(),
        "payload": &event.payload,
        "created_unix_ms": event.created_unix_ms,
    })
}

fn agent_mcp_call_debug_repl_cells(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(50);
    if limit == 0 {
        return Err("arcweft.debug.repl.cells argument limit must be at least 1".to_owned());
    }
    let session_id = agent_mcp_non_empty_string_argument(arguments, "session_id")
        .ok_or_else(|| "arcweft.debug.repl.cells requires arguments.session_id".to_owned())
        .and_then(|value| {
            SessionId::new(value)
                .map_err(|error| format!("arcweft.debug.repl.cells invalid session_id: {error}"))
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.repl.cells failed to open `{path}`: {error}"))?;
    let cells = store
        .repl_cells_for_session(&session_id)
        .map_err(|error| format!("arcweft.debug.repl.cells failed to read `{path}`: {error}"))?;
    let value = serde_json::json!({
        "path": path,
        "session_id": session_id.as_str(),
        "limit": limit,
        "cells": cells
            .iter()
            .take(limit)
            .map(agent_mcp_debug_repl_cell_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug REPL cells")
}

fn agent_mcp_debug_repl_cell_json(cell: &DebugReplCell) -> serde_json::Value {
    serde_json::json!({
        "cell_id": &cell.cell_id,
        "session_id": cell.session_id.as_str(),
        "run_id": cell.run_id.as_ref().map(AgentRunId::as_str),
        "ordinal": cell.ordinal,
        "source": &cell.source,
        "source_hash": cell.source_hash.as_str(),
        "status": &cell.status,
        "inferred_type": &cell.inferred_type,
        "display": &cell.display,
        "partially_effectful": cell.partially_effectful,
        "diagnostic_ids": &cell.diagnostic_ids,
        "created_unix_ms": cell.created_unix_ms,
    })
}

fn agent_mcp_call_debug_source_files(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let program_hash = agent_mcp_non_empty_string_argument(arguments, "program_hash")
        .ok_or_else(|| "arcweft.debug.source.files requires arguments.program_hash".to_owned())
        .and_then(|value| {
            StableHash::new(value).map_err(|error| {
                format!("arcweft.debug.source.files invalid program_hash: {error}")
            })
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.source.files")?;
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.debug.source.files failed to open `{path}`: {error}"))?;
    let sources = if PrivacyClass::Project.is_allowed_by(max_privacy) {
        store
            .source_files_for_program(&program_hash)
            .map_err(|error| {
                format!("arcweft.debug.source.files failed to read `{path}`: {error}")
            })?
    } else {
        Vec::new()
    };
    let value = serde_json::json!({
        "path": path,
        "program_hash": program_hash.as_str(),
        "max_privacy": max_privacy.as_str(),
        "sources": sources
            .iter()
            .map(agent_mcp_debug_source_file_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug source files")
}

fn agent_mcp_debug_source_file_json(source: &DebugSourceFile) -> serde_json::Value {
    serde_json::json!({
        "program_hash": source.program_hash.as_str(),
        "path": &source.path,
        "language": &source.language,
        "content_hash": source.content_hash.as_str(),
        "byte_len": source.byte_len,
        "metadata": &source.metadata,
    })
}

fn agent_mcp_call_debug_graph_inventory(
    arguments: &serde_json::Value,
) -> Result<McpCallToolResult, String> {
    let program_hash = agent_mcp_non_empty_string_argument(arguments, "program_hash")
        .ok_or_else(|| "arcweft.debug.graph.inventory requires arguments.program_hash".to_owned())
        .and_then(|value| {
            StableHash::new(value).map_err(|error| {
                format!("arcweft.debug.graph.inventory invalid program_hash: {error}")
            })
        })?;
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.debug.graph.inventory")?;
    let store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.debug.graph.inventory failed to open `{path}`: {error}")
    })?;
    let (symbols, edges) =
        if PrivacyClass::Project.is_allowed_by(max_privacy) {
            let symbols = store.graph_symbols_for_program(&program_hash).map_err(|error| {
            format!("arcweft.debug.graph.inventory failed to read symbols from `{path}`: {error}")
        })?;
            let edges = store
                .graph_edges_for_program(&program_hash)
                .map_err(|error| {
                    format!(
                        "arcweft.debug.graph.inventory failed to read edges from `{path}`: {error}"
                    )
                })?;
            (symbols, edges)
        } else {
            (Vec::new(), Vec::new())
        };
    let value = serde_json::json!({
        "path": path,
        "program_hash": program_hash.as_str(),
        "max_privacy": max_privacy.as_str(),
        "symbol_count": symbols.len(),
        "edge_count": edges.len(),
        "symbols": symbols
            .iter()
            .map(agent_mcp_debug_graph_symbol_json)
            .collect::<Vec<_>>(),
        "edges": edges
            .iter()
            .map(agent_mcp_debug_graph_edge_json)
            .collect::<Vec<_>>(),
    });
    agent_mcp_json_tool_result(&value, "debug graph inventory")
}

fn agent_mcp_debug_graph_symbol_json(symbol: &DebugGraphSymbol) -> serde_json::Value {
    serde_json::json!({
        "program_hash": symbol.program_hash.as_str(),
        "symbol_id": &symbol.symbol_id,
        "public_id": symbol.public_id.as_ref().map(PublicId::as_str),
        "qualified_name": &symbol.qualified_name,
        "kind": &symbol.kind,
        "type_json": &symbol.type_json,
        "source_path": &symbol.source_path,
        "source_content_hash": symbol.source_content_hash.as_ref().map(StableHash::as_str),
        "start_byte": symbol.start_byte,
        "end_byte": symbol.end_byte,
        "semantic_hash": symbol.semantic_hash.as_ref().map(StableHash::as_str),
        "summary": &symbol.summary,
        "metadata": &symbol.metadata,
    })
}

fn agent_mcp_debug_graph_edge_json(edge: &DebugGraphEdge) -> serde_json::Value {
    serde_json::json!({
        "program_hash": edge.program_hash.as_str(),
        "from_symbol_id": &edge.from_symbol_id,
        "to_symbol_id": &edge.to_symbol_id,
        "edge_kind": &edge.edge_kind,
        "weight": edge.weight,
        "metadata": &edge.metadata,
    })
}

fn agent_mcp_debug_store_path(arguments: &serde_json::Value) -> &str {
    arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .unwrap_or(".arcweft/cache/agent-debug.sqlite3")
}

fn agent_mcp_debug_search_model(
    arguments: &serde_json::Value,
    dimensions: usize,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = agent_mcp_non_empty_string_argument(arguments, "model_id")
        .ok_or_else(|| "arcweft.debug.search query_vector requires model_id".to_owned())?;
    let model_revision = agent_mcp_non_empty_string_argument(arguments, "model_revision")
        .ok_or_else(|| "arcweft.debug.search query_vector requires model_revision".to_owned())?;
    let dimensions = u32::try_from(dimensions)
        .map_err(|_| "arcweft.debug.search query_vector has too many dimensions".to_owned())?;
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions,
    })
}

fn agent_mcp_non_empty_string_argument<'a>(
    arguments: &'a serde_json::Value,
    name: &str,
) -> Option<&'a str> {
    arguments
        .get(name)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn agent_mcp_query_vector_argument(
    arguments: &serde_json::Value,
    name: &str,
) -> Result<Option<Vec<f32>>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .map(|item| parse_agent_mcp_query_vector_json_number(item, name))
            .collect::<Result<Vec<_>, _>>()
            .and_then(non_empty_agent_mcp_query_vector)
            .map(Some),
        serde_json::Value::String(text) => parse_agent_mcp_query_vector_string(text)
            .and_then(non_empty_agent_mcp_query_vector)
            .map(Some),
        _ => Err(format!(
            "arcweft.debug.search argument {name} must be an array of numbers or a comma-separated string"
        )),
    }
}

fn parse_agent_mcp_query_vector_json_number(
    value: &serde_json::Value,
    name: &str,
) -> Result<f32, String> {
    if !value.is_number() {
        return Err(format!(
            "arcweft.debug.search argument {name} must contain finite numbers"
        ));
    }
    let parsed = value
        .to_string()
        .parse::<f32>()
        .map_err(|_| format!("arcweft.debug.search argument {name} must contain finite numbers"))?;
    if parsed.is_finite() {
        Ok(parsed)
    } else {
        Err(format!(
            "arcweft.debug.search argument {name} must contain finite numbers"
        ))
    }
}

fn parse_agent_mcp_query_vector_string(value: &str) -> Result<Vec<f32>, String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            part.parse::<f32>().map_err(|error| {
                format!("invalid arcweft.debug.search query_vector component `{part}`: {error}")
            })
        })
        .collect()
}

fn non_empty_agent_mcp_query_vector(values: Vec<f32>) -> Result<Vec<f32>, String> {
    if values.is_empty() {
        return Err("arcweft.debug.search query_vector must not be empty".to_owned());
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(
            "arcweft.debug.search query_vector must contain only finite numbers".to_owned(),
        );
    }
    Ok(values)
}

const fn agent_mcp_search_channel_label(channel: SearchChannel) -> &'static str {
    match channel {
        SearchChannel::ExactEntity => "exact_entity",
        SearchChannel::Lexical => "lexical",
        SearchChannel::Vector => "vector",
        SearchChannel::Graph => "graph",
        SearchChannel::History => "history",
        SearchChannel::Diagnostics => "diagnostics",
        SearchChannel::Trace => "trace",
        SearchChannel::Summary => "summary",
    }
}

fn agent_mcp_call_rag_query(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<McpCallToolResult, String> {
    agent_mcp_observe_if_requested(arguments, state, adapter_registrars)?;
    let query_text = arguments
        .get("query")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|query| !query.is_empty())
        .ok_or_else(|| "arcweft.rag.query requires non-empty arguments.query".to_owned())?;
    let limit = agent_mcp_usize_argument(arguments, "limit").unwrap_or(8);
    if limit == 0 {
        return Err("arcweft.rag.query argument limit must be at least 1".to_owned());
    }
    let max_context_bytes =
        agent_mcp_usize_argument(arguments, "max_context_bytes").unwrap_or(32 * 1024);
    if max_context_bytes == 0 {
        return Err("arcweft.rag.query argument max_context_bytes must be at least 1".to_owned());
    }
    let graph_depth =
        agent_mcp_u32_argument(arguments, "graph_depth", "arcweft.rag.query")?.unwrap_or(1);
    let roots = agent_mcp_rag_roots(arguments)?;
    let max_privacy = agent_mcp_privacy_class_argument(arguments, "max_privacy")?
        .unwrap_or(PrivacyClass::Project);
    let local_embedding =
        agent_mcp_bool_argument(arguments, "local_embedding", "arcweft.rag.query")?
            .unwrap_or(false);
    if local_embedding && agent_mcp_optional_debug_store_path(arguments).is_none() {
        return Err("arcweft.rag.query local_embedding requires arguments.path".to_owned());
    }
    let mut source_context = agent_mcp_rag_source_context(arguments)?;
    let config = AgentMcpRagQueryConfig {
        roots,
        graph_depth,
        limit,
        max_context_bytes,
        max_privacy,
        local_embedding,
        local_embedding_model: agent_mcp_rag_local_embedding_model(arguments)?,
    };
    if let Some(path) = agent_mcp_optional_debug_store_path(arguments) {
        source_context
            .candidates
            .extend(agent_mcp_rag_debug_store_candidates(
                path, query_text, &config,
            )?);
    }
    let result = agent_mcp_rag_query_result(state, source_context, query_text, config)?;
    if let Some(path) = agent_mcp_optional_debug_store_path(arguments) {
        persist_agent_mcp_rag_query_result(path, &result)?;
    }
    let pack = result.pack;
    let value = serde_json::to_value(&pack)
        .map_err(|error| format!("failed to serialize Agent RAG context pack: {error}"))?;
    state.rag_context_packs.push(pack);
    agent_mcp_json_tool_result(&value, "RAG context pack")
}

fn agent_mcp_call_rag_explain(
    arguments: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let value = match agent_mcp_cached_rag_context_pack(arguments, state, "arcweft.rag.explain") {
        Ok(pack) => agent_mcp_rag_context_explanation_json(pack),
        Err(cache_error) => {
            let query_id =
                agent_mcp_non_empty_string_argument(arguments, "query_id").ok_or(cache_error)?;
            let audit =
                agent_mcp_rag_query_audit_from_store(arguments, query_id, "arcweft.rag.explain")?;
            agent_mcp_rag_query_audit_explanation_json(&audit)
        }
    };
    agent_mcp_json_tool_result(&value, "RAG explanation")
}

fn agent_mcp_call_rag_context_read(
    arguments: &serde_json::Value,
    state: &AgentMcpState,
) -> Result<McpCallToolResult, String> {
    let stored_audit;
    let pack = match agent_mcp_cached_rag_context_pack(arguments, state, "arcweft.rag.context.read")
    {
        Ok(pack) => pack,
        Err(cache_error) => {
            let query_id =
                agent_mcp_non_empty_string_argument(arguments, "query_id").ok_or(cache_error)?;
            stored_audit = agent_mcp_rag_query_audit_from_store(
                arguments,
                query_id,
                "arcweft.rag.context.read",
            )?;
            &stored_audit.pack
        }
    };
    let chunk_id = agent_mcp_non_empty_string_argument(arguments, "chunk_id")
        .ok_or_else(|| "arcweft.rag.context.read requires arguments.chunk_id".to_owned())?;
    let max_bytes = agent_mcp_usize_argument(arguments, "max_bytes").unwrap_or(8192);
    if max_bytes == 0 {
        return Err("arcweft.rag.context.read argument max_bytes must be at least 1".to_owned());
    }
    let item = pack
        .items
        .iter()
        .find(|item| item.chunk_id.as_str() == chunk_id)
        .ok_or_else(|| {
            format!(
                "arcweft.rag.context.read could not find chunk_id `{chunk_id}` in cached query {}",
                pack.query.query_id
            )
        })?;
    let (body, body_truncated) = agent_mcp_truncate_utf8(&item.body, max_bytes);
    let value = serde_json::json!({
        "query_id": &pack.query.query_id,
        "chunk_id": item.chunk_id.as_str(),
        "kind": &item.kind,
        "title": &item.title,
        "body": body,
        "truncated": body_truncated,
        "body_bytes": item.body.len(),
        "returned_bytes": body.len(),
        "fused_score": item.fused_score,
        "channels": &item.channels,
        "entity_ids": &item.entity_ids,
        "source_anchor": &item.source_anchor,
    });
    agent_mcp_json_tool_result(&value, "RAG context item")
}

fn agent_mcp_rag_query_audit_from_store(
    arguments: &serde_json::Value,
    query_id: &str,
    tool: &str,
) -> Result<DebugRagQueryAudit, String> {
    let path = agent_mcp_debug_store_path(arguments);
    let max_privacy = agent_mcp_max_privacy_argument(arguments, tool)?;
    let store = DebugStore::open(path)
        .map_err(|error| format!("{tool} failed to open `{path}`: {error}"))?;
    store
        .rag_query_audit_with_max_privacy(query_id, max_privacy)
        .map_err(|error| {
            format!("{tool} failed to read RAG query `{query_id}` from `{path}`: {error}")
        })
}

fn agent_mcp_rag_context_explanation_json(pack: &RagContextPack) -> serde_json::Value {
    serde_json::json!({
        "schema_version": pack.schema_version,
        "query": &pack.query,
        "item_count": pack.items.len(),
        "truncated": pack.truncated,
        "items": pack
            .items
            .iter()
            .map(agent_mcp_rag_context_item_explanation)
            .collect::<Vec<_>>(),
    })
}

fn agent_mcp_rag_query_audit_explanation_json(audit: &DebugRagQueryAudit) -> serde_json::Value {
    let mut value = agent_mcp_rag_context_explanation_json(&audit.pack);
    if let serde_json::Value::Object(fields) = &mut value {
        fields.insert(
            "session_id".to_owned(),
            audit
                .session_id
                .as_ref()
                .map_or(serde_json::Value::Null, |id| {
                    serde_json::Value::String(id.as_str().to_owned())
                }),
        );
        fields.insert(
            "run_id".to_owned(),
            audit.run_id.as_ref().map_or(serde_json::Value::Null, |id| {
                serde_json::Value::String(id.as_str().to_owned())
            }),
        );
        fields.insert(
            "status".to_owned(),
            serde_json::Value::String(audit.status.clone()),
        );
        fields.insert(
            "created_unix_ms".to_owned(),
            serde_json::Value::from(audit.created_unix_ms),
        );
        fields.insert(
            "source".to_owned(),
            serde_json::Value::String("debug_store".to_owned()),
        );
    }
    value
}

fn agent_mcp_cached_rag_context_pack<'a>(
    arguments: &serde_json::Value,
    state: &'a AgentMcpState,
    tool: &str,
) -> Result<&'a RagContextPack, String> {
    let query_id = agent_mcp_non_empty_string_argument(arguments, "query_id");
    if let Some(query_id) = query_id {
        return state
            .rag_context_packs
            .iter()
            .rev()
            .find(|pack| pack.query.query_id == query_id)
            .ok_or_else(|| format!("{tool} could not find cached query_id `{query_id}`"));
    }
    state
        .rag_context_packs
        .last()
        .ok_or_else(|| format!("{tool} requires a prior arcweft.rag.query call"))
}

fn agent_mcp_rag_context_item_explanation(item: &RagContextItem) -> serde_json::Value {
    serde_json::json!({
        "chunk_id": item.chunk_id.as_str(),
        "kind": &item.kind,
        "title": &item.title,
        "body_bytes": item.body.len(),
        "fused_score": item.fused_score,
        "channels": &item.channels,
        "entity_ids": &item.entity_ids,
        "source_anchor": &item.source_anchor,
    })
}

#[derive(Clone)]
struct AgentMcpRagCandidate {
    chunk: DebugChunk,
    preferred_channel: SearchChannel,
}

struct AgentMcpRagQueryResult {
    pack: RagContextPack,
    candidates: Vec<AgentMcpRagCandidate>,
    source_indexes: Vec<AgentSourceRagIndex>,
}

struct AgentMcpRagSourceContext {
    candidates: Vec<AgentMcpRagCandidate>,
    source_indexes: Vec<AgentSourceRagIndex>,
}

struct AgentMcpRagQueryConfig {
    roots: Vec<PublicId>,
    graph_depth: u32,
    limit: usize,
    max_context_bytes: usize,
    max_privacy: PrivacyClass,
    local_embedding: bool,
    local_embedding_model: EmbeddingModelDescriptor,
}

fn agent_mcp_rag_context_pack(
    state: &AgentMcpState,
    query_text: &str,
    roots: Vec<PublicId>,
    graph_depth: u32,
    limit: usize,
    max_context_bytes: usize,
    max_privacy: PrivacyClass,
) -> Result<RagContextPack, String> {
    let config = AgentMcpRagQueryConfig {
        roots,
        graph_depth,
        limit,
        max_context_bytes,
        max_privacy,
        local_embedding: false,
        local_embedding_model: EmbeddingModelDescriptor {
            model_id: DEFAULT_LOCAL_EMBEDDING_MODEL_ID.to_owned(),
            model_revision: DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION.to_owned(),
            dimensions: DEFAULT_LOCAL_EMBEDDING_DIMENSIONS,
        },
    };
    agent_mcp_rag_query_result(
        state,
        AgentMcpRagSourceContext {
            candidates: Vec::new(),
            source_indexes: Vec::new(),
        },
        query_text,
        config,
    )
    .map(|result| result.pack)
}

fn agent_mcp_rag_query_result(
    state: &AgentMcpState,
    source_context: AgentMcpRagSourceContext,
    query_text: &str,
    config: AgentMcpRagQueryConfig,
) -> Result<AgentMcpRagQueryResult, String> {
    let mut candidates = agent_mcp_rag_candidates(state)?;
    candidates.extend(source_context.candidates);
    let candidates = agent_mcp_rag_deduplicate_candidates(candidates);
    let query_candidates = candidates
        .iter()
        .filter(|candidate| candidate.chunk.privacy.is_allowed_by(config.max_privacy))
        .cloned()
        .collect::<Vec<_>>();
    if query_candidates.is_empty() {
        return Err(
            "arcweft.rag.query found no context allowed by max_privacy; observe a source/profile, read a trace, or raise arguments.max_privacy"
                .to_owned(),
        );
    }
    let program_hash = agent_mcp_rag_program_hash(state, &query_candidates)?;
    let query_id_seed = format!(
        "{}:{}:{graph_depth}:{limit}:{max_context_bytes}:{}",
        query_text,
        program_hash.as_str(),
        config.max_privacy.as_str(),
        graph_depth = config.graph_depth,
        limit = config.limit,
        max_context_bytes = config.max_context_bytes,
    );
    let query = RagQuery {
        query_id: agent_mcp_content_hash(query_id_seed),
        text: query_text.to_owned(),
        program_hash,
        roots: config.roots,
        graph_depth: config.graph_depth,
        limit: config.limit,
        max_context_bytes: config.max_context_bytes,
    };
    let pack = agent_mcp_rag_context_pack_from_candidates(
        query,
        &query_candidates,
        config.limit,
        config.max_context_bytes,
    );
    Ok(AgentMcpRagQueryResult {
        pack,
        candidates,
        source_indexes: source_context.source_indexes,
    })
}

fn agent_mcp_rag_debug_store_candidates(
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.rag.query failed to open `{path}`: {error}"))?;
    let search_limit = config.limit.saturating_mul(8).max(32);
    let terms = std::iter::once(query_text.trim().to_owned())
        .chain(config.roots.iter().map(|root| root.as_str().to_owned()))
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    let mut seen = BTreeSet::new();
    if config.local_embedding {
        let vector_results = agent_mcp_rag_debug_store_vector_results(
            &store,
            path,
            query_text,
            config,
            search_limit,
        )?;
        if vector_results.is_empty() {
            agent_mcp_rag_record_local_embedding_fallback(&store, path, query_text, config)?;
        }
        for result in vector_results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentMcpRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    for term in terms {
        let mut results = store
            .lexical_chunk_search_with_max_privacy(&term, search_limit, config.max_privacy)
            .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?;
        results.extend(
            store
                .graph_search_with_depth_and_max_privacy(
                    &term,
                    config.graph_depth,
                    search_limit,
                    config.max_privacy,
                )
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "graph"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .history_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "history"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .diagnostic_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "diagnostic"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        results.extend(
            store
                .test_result_search_with_max_privacy(&term, search_limit, config.max_privacy)
                .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
                .into_iter()
                .map(|result| agent_mcp_rag_candidate_from_search_result(result, "test"))
                .collect::<Result<Vec<_>, _>>()?,
        );
        for result in results {
            if seen.insert(result.chunk.id.clone()) {
                candidates.push(AgentMcpRagCandidate {
                    chunk: result.chunk,
                    preferred_channel: result.hit.channel,
                });
            }
        }
    }
    Ok(candidates)
}

fn agent_mcp_rag_record_local_embedding_fallback(
    store: &DebugStore,
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
) -> Result<(), String> {
    let model = &config.local_embedding_model;
    let query = query_text.trim();
    let diagnostic_id = format!(
        "agent-mcp-rag-local-embedding-fallback:{}",
        agent_mcp_content_hash(format!(
            "{}:{}:{}:{}",
            path, query, model.model_id, model.model_revision
        ))
    );
    store
        .upsert_diagnostic(&DebugDiagnostic {
            diagnostic_id,
            program_hash: None,
            session_id: None,
            run_id: None,
            sequence: None,
            code: Some("AGENT_RAG_EMBEDDING_FALLBACK".to_owned()),
            severity: "warning".to_owned(),
            phase: "agent_rag".to_owned(),
            message: format!(
                "local embedding channel produced no hits for model {}@{}:{}; using lexical fallback channels",
                model.model_id, model.model_revision, model.dimensions
            ),
            source_path: Some(path.to_owned()),
            start_byte: None,
            end_byte: None,
            related_ids: Vec::new(),
            payload: serde_json::json!({
                "provider": "local_hash",
                "model": {
                    "model_id": model.model_id,
                    "model_revision": model.model_revision,
                    "dimensions": model.dimensions,
                },
                "query": query,
                "fallback_channels": ["lexical", "graph", "history", "diagnostics", "test_result"],
                "reason": "no_vector_hits",
            }),
            created_unix_ms: 0,
        })
        .map_err(|error| {
            format!(
                "arcweft.rag.query failed to record local embedding fallback diagnostic in `{path}`: {error}"
            )
        })
}

fn agent_mcp_rag_debug_store_vector_results(
    store: &DebugStore,
    path: &str,
    query_text: &str,
    config: &AgentMcpRagQueryConfig,
    search_limit: usize,
) -> Result<Vec<DebugChunkSearchResult>, String> {
    let query_vector =
        local_hash_query_embedding(query_text.trim(), config.local_embedding_model.dimensions);
    store
        .vector_search_with_max_privacy(
            &config.local_embedding_model,
            &query_vector,
            search_limit,
            config.max_privacy,
        )
        .map_err(|error| agent_mcp_rag_debug_store_search_error(path, &error))?
        .into_iter()
        .map(|result| agent_mcp_rag_candidate_from_search_result(result, "vector"))
        .collect()
}

fn agent_mcp_rag_local_embedding_model(
    arguments: &serde_json::Value,
) -> Result<EmbeddingModelDescriptor, String> {
    let model_id = agent_mcp_non_empty_string_argument(arguments, "local_embedding_model_id")
        .unwrap_or(DEFAULT_LOCAL_EMBEDDING_MODEL_ID);
    let model_revision =
        agent_mcp_non_empty_string_argument(arguments, "local_embedding_model_revision")
            .unwrap_or(DEFAULT_LOCAL_EMBEDDING_MODEL_REVISION);
    let dimensions =
        agent_mcp_u32_argument(arguments, "local_embedding_dimensions", "arcweft.rag.query")?
            .unwrap_or(DEFAULT_LOCAL_EMBEDDING_DIMENSIONS);
    if dimensions == 0 {
        return Err(
            "arcweft.rag.query argument local_embedding_dimensions must be at least 1".to_owned(),
        );
    }
    if dimensions > MAX_LOCAL_EMBEDDING_DIMENSIONS {
        return Err(format!(
            "arcweft.rag.query argument local_embedding_dimensions must be at most {MAX_LOCAL_EMBEDDING_DIMENSIONS}"
        ));
    }
    Ok(EmbeddingModelDescriptor {
        model_id: model_id.to_owned(),
        model_revision: model_revision.to_owned(),
        dimensions,
    })
}

fn agent_mcp_rag_debug_store_search_error(
    path: &str,
    error: &arcweft_debug_sqlite::store::DebugStoreError,
) -> String {
    format!("arcweft.rag.query failed to search debug store `{path}`: {error}")
}

fn agent_mcp_rag_candidate_from_search_result(
    result: ChunkSearchResult,
    source_prefix: &str,
) -> Result<DebugChunkSearchResult, String> {
    let source_kind = match result.source_kind.as_str() {
        "source" => ChunkSourceKind::Source,
        "symbol" => ChunkSourceKind::Symbol,
        "graph_summary" | "graph_edge" | "graph_symbol" => ChunkSourceKind::GraphSummary,
        "diagnostic" => ChunkSourceKind::Diagnostic,
        "test_result" => ChunkSourceKind::TestResult,
        "agent_trace" => ChunkSourceKind::AgentTrace,
        "history" => ChunkSourceKind::History,
        "documentation" => ChunkSourceKind::Documentation,
        other => {
            return Err(format!(
                "arcweft.rag.query debug store result has unsupported source_kind `{other}`"
            ));
        }
    };
    let content_hash = agent_mcp_content_hash(&result.body);
    let mut metadata = BTreeMap::new();
    metadata.insert(
        "search_channel".to_owned(),
        serde_json::json!(agent_mcp_search_channel_label(result.hit.channel)),
    );
    metadata.insert(
        "search_score".to_owned(),
        serde_json::to_value(result.hit.score).map_err(|error| {
            format!("arcweft.rag.query failed to serialize debug store search score: {error}")
        })?,
    );
    Ok(DebugChunkSearchResult {
        hit: result.hit.clone(),
        chunk: DebugChunk {
            id: result.hit.chunk_id,
            program_hash: None,
            source_kind,
            source_key: format!("{source_prefix}:{}", result.source_key),
            title: result.title,
            body: result.body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids: Vec::new(),
            privacy: result.privacy,
            metadata,
            created_unix_ms: 0,
        },
    })
}

fn agent_mcp_rag_deduplicate_candidates(
    candidates: Vec<AgentMcpRagCandidate>,
) -> Vec<AgentMcpRagCandidate> {
    let mut seen = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|candidate| seen.insert(candidate.chunk.id.clone()))
        .collect()
}

fn agent_mcp_rag_context_pack_from_candidates(
    query: RagQuery,
    candidates: &[AgentMcpRagCandidate],
    limit: usize,
    max_context_bytes: usize,
) -> RagContextPack {
    let fused = reciprocal_rank_fusion(
        &agent_mcp_rag_ranked_lists(candidates, &query),
        &FusionConfig::default(),
        candidates.len(),
    );
    let by_id = candidates
        .iter()
        .map(|candidate| (candidate.chunk.id.clone(), &candidate.chunk))
        .collect::<BTreeMap<_, _>>();
    let mut used_bytes = 0usize;
    let mut truncated = false;
    let mut items = Vec::new();
    let mut selected_semantic_hashes = BTreeSet::new();
    let mut selected_source_anchors = Vec::new();
    for hit in fused {
        if items.len() >= limit {
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
        let remaining = max_context_bytes.saturating_sub(used_bytes);
        if remaining == 0 {
            truncated = true;
            break;
        }
        let (body, body_truncated) = agent_mcp_truncate_utf8(&chunk.body, remaining);
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

fn persist_agent_mcp_rag_query_result(
    path: &str,
    result: &AgentMcpRagQueryResult,
) -> Result<(), String> {
    let store = DebugStore::open(path)
        .map_err(|error| format!("arcweft.rag.query failed to open `{path}`: {error}"))?;
    store
        .upsert_program(&result.pack.query.program_hash, None, None, 0)
        .map_err(|error| {
            format!("arcweft.rag.query failed to index program in `{path}`: {error}")
        })?;
    let session_id = agent_mcp_rag_query_session_id(&result.pack.query.query_id)?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: Some(result.pack.query.program_hash.clone()),
            profile: "rag".to_owned(),
            transport: "mcp".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: Some(0),
            status: DebugSessionStatus::Finished,
            metadata: agent_mcp_rag_query_session_metadata(result),
        })
        .map_err(|error| {
            format!("arcweft.rag.query failed to record RAG session in `{path}`: {error}")
        })?;
    for candidate in &result.candidates {
        let mut chunk = candidate.chunk.clone();
        chunk.program_hash = Some(result.pack.query.program_hash.clone());
        store.upsert_chunk(&chunk).map_err(|error| {
            format!("arcweft.rag.query failed to index context chunk in `{path}`: {error}")
        })?;
    }
    if !result.source_indexes.is_empty() {
        for source_index in &result.source_indexes {
            let mut source_file = source_index.source_file.clone();
            source_file.program_hash = result.pack.query.program_hash.clone();
            store.upsert_source_file(&source_file).map_err(|error| {
                format!("arcweft.rag.query failed to index source file in `{path}`: {error}")
            })?;
            agent_rag_index_graph(
                &store,
                &result.pack.query.program_hash,
                source_index,
                result.source_indexes.len() > 1,
            )?;
        }
        let source_index_refs = result.source_indexes.iter().collect::<Vec<_>>();
        agent_rag_index_program_graph(&store, &result.pack.query.program_hash, &source_index_refs)?;
    }
    store
        .record_rag_context_pack(&result.pack, Some(&session_id), None, None, "selected", 0)
        .map_err(|error| {
            format!("arcweft.rag.query failed to record RAG audit in `{path}`: {error}")
        })
}

fn agent_mcp_rag_query_session_id(query_id: &str) -> Result<SessionId, String> {
    let suffix = agent_mcp_content_hash(format!("mcp:{query_id}")).replace(':', ".");
    SessionId::new(format!("session.rag.mcp.{suffix}"))
        .map_err(|error| format!("failed to build MCP RAG session id: {error}"))
}

fn agent_mcp_rag_query_session_metadata(
    result: &AgentMcpRagQueryResult,
) -> BTreeMap<String, serde_json::Value> {
    BTreeMap::from([
        (
            "query_id".to_owned(),
            serde_json::json!(result.pack.query.query_id),
        ),
        (
            "query_text".to_owned(),
            serde_json::json!(result.pack.query.text),
        ),
        (
            "item_count".to_owned(),
            serde_json::json!(result.pack.items.len()),
        ),
        (
            "truncated".to_owned(),
            serde_json::json!(result.pack.truncated),
        ),
        (
            "roots".to_owned(),
            serde_json::json!(
                result
                    .pack
                    .query
                    .roots
                    .iter()
                    .map(PublicId::as_str)
                    .collect::<Vec<_>>()
            ),
        ),
        (
            "graph_depth".to_owned(),
            serde_json::json!(result.pack.query.graph_depth),
        ),
        (
            "max_context_bytes".to_owned(),
            serde_json::json!(result.pack.query.max_context_bytes),
        ),
    ])
}

fn agent_mcp_optional_debug_store_path(arguments: &serde_json::Value) -> Option<&str> {
    arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
}

fn agent_mcp_rag_source_context(
    arguments: &serde_json::Value,
) -> Result<AgentMcpRagSourceContext, String> {
    let inputs = agent_mcp_rag_source_inputs(arguments)?;
    let paths = agent_rag_source_paths(&inputs)?;
    let source_indexes = paths
        .iter()
        .map(|path| agent_source_rag_index(path))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates = source_indexes
        .iter()
        .flat_map(|index| index.candidates.iter().cloned())
        .map(agent_mcp_rag_candidate_from_cli_source)
        .collect::<Vec<_>>();
    if !source_indexes.is_empty() {
        let seed_parts = source_indexes
            .iter()
            .map(|index| index.seed.clone())
            .collect::<Vec<_>>();
        let program_hash = agent_rag_program_hash(&seed_parts)?;
        let source_index_refs = source_indexes.iter().collect::<Vec<_>>();
        candidates.push(agent_mcp_rag_candidate_from_cli_source(
            agent_program_summary_rag_candidate(&program_hash, &source_index_refs)?,
        ));
    }
    Ok(AgentMcpRagSourceContext {
        candidates,
        source_indexes,
    })
}

fn agent_mcp_rag_candidate_from_cli_source(
    candidate: super::AgentRagCandidate,
) -> AgentMcpRagCandidate {
    let mut chunk = candidate.chunk;
    chunk.id = ChunkId::new(format!(
        "mcp:{}:{}",
        chunk.source_key,
        chunk.content_hash.as_str()
    ));
    AgentMcpRagCandidate {
        chunk,
        preferred_channel: candidate.preferred_channel,
    }
}

fn agent_mcp_rag_source_inputs(arguments: &serde_json::Value) -> Result<Vec<PathBuf>, String> {
    let mut inputs = Vec::new();
    if let Some(source) = agent_mcp_non_empty_string_argument(arguments, "source") {
        inputs.push(PathBuf::from(source));
    }
    let Some(sources) = arguments.get("sources") else {
        return Ok(inputs);
    };
    match sources {
        serde_json::Value::String(source) => {
            let source = source.trim();
            if !source.is_empty() {
                inputs.push(PathBuf::from(source));
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                let source = item
                    .as_str()
                    .map(str::trim)
                    .filter(|source| !source.is_empty())
                    .ok_or_else(|| {
                        "arcweft.rag.query argument sources must contain non-empty strings"
                            .to_owned()
                    })?;
                inputs.push(PathBuf::from(source));
            }
        }
        _ => {
            return Err(
                "arcweft.rag.query argument sources must be a string or an array of strings"
                    .to_owned(),
            );
        }
    }
    Ok(inputs)
}

fn agent_mcp_rag_candidates(state: &AgentMcpState) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let mut candidates = Vec::new();
    if let Some(report) = &state.report {
        let summary = agent_mcp_observation_state_summary(report);
        candidates.push(agent_mcp_rag_json_candidate(
            "observation.summary",
            "Observation state summary",
            ChunkSourceKind::GraphSummary,
            SearchChannel::Summary,
            &summary,
            Vec::new(),
            PrivacyClass::Project,
        )?);
        candidates.push(agent_mcp_rag_json_candidate(
            "observation.actions",
            "Agent action targets",
            ChunkSourceKind::GraphSummary,
            SearchChannel::Graph,
            &serde_json::to_value(&report.actions)
                .map_err(|error| format!("failed to serialize Agent actions: {error}"))?,
            Vec::new(),
            PrivacyClass::Project,
        )?);
        for object in &report.objects {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("object.{}", object.id),
                &format!("Observed object {} role {}", object.id, object.role),
                ChunkSourceKind::GraphSummary,
                SearchChannel::Graph,
                &serde_json::to_value(object)
                    .map_err(|error| format!("failed to serialize observed object: {error}"))?,
                agent_mcp_object_entity_ids(object),
                PrivacyClass::Project,
            )?);
        }
        for signal in &report.signals {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("signal.{}", signal.name),
                &format!("Signal {}", signal.name),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(signal)
                    .map_err(|error| format!("failed to serialize Agent signal: {error}"))?,
                agent_mcp_public_ids([signal.name.as_str()]),
                PrivacyClass::Project,
            )?);
        }
        for metric in &report.metrics {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("metric.{}", metric.name),
                &format!("Metric {}", metric.name),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(metric)
                    .map_err(|error| format!("failed to serialize Agent metric: {error}"))?,
                agent_mcp_public_ids([metric.name.as_str()]),
                PrivacyClass::Project,
            )?);
        }
        for (index, log) in report.logs.iter().enumerate() {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("log.{index}"),
                &format!("Runtime log {} {}", log.level, index),
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                &serde_json::to_value(log)
                    .map_err(|error| format!("failed to serialize runtime log: {error}"))?,
                Vec::new(),
                PrivacyClass::Project,
            )?);
        }
        for diagnostic in &report.diagnostics {
            candidates.push(agent_mcp_rag_json_candidate(
                &format!("diagnostic.{}.{}", diagnostic.step, candidates.len()),
                &format!(
                    "Diagnostic {:?} step {}",
                    diagnostic.severity, diagnostic.step
                ),
                ChunkSourceKind::Diagnostic,
                SearchChannel::Diagnostics,
                &serde_json::to_value(diagnostic)
                    .map_err(|error| format!("failed to serialize Agent diagnostic: {error}"))?,
                agent_mcp_public_ids(
                    [
                        diagnostic.source.as_deref(),
                        diagnostic.effect_id.as_deref(),
                    ]
                    .into_iter()
                    .flatten(),
                ),
                PrivacyClass::Project,
            )?);
        }
    }
    for resource in &state.trace_resources {
        candidates.extend(agent_mcp_trace_resource_rag_candidates(resource)?);
    }
    Ok(candidates)
}

fn agent_mcp_rag_json_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    value: &serde_json::Value,
    entity_ids: Vec<PublicId>,
    privacy: PrivacyClass,
) -> Result<AgentMcpRagCandidate, String> {
    let body = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize RAG candidate body: {error}"))?;
    Ok(agent_mcp_rag_text_candidate(
        source_key,
        title,
        source_kind,
        preferred_channel,
        body,
        entity_ids,
        privacy,
    ))
}

fn agent_mcp_rag_text_candidate(
    source_key: &str,
    title: &str,
    source_kind: ChunkSourceKind,
    preferred_channel: SearchChannel,
    body: String,
    entity_ids: Vec<PublicId>,
    privacy: PrivacyClass,
) -> AgentMcpRagCandidate {
    let content_hash = agent_mcp_content_hash(&body);
    AgentMcpRagCandidate {
        chunk: DebugChunk {
            id: ChunkId::new(format!("mcp:{source_key}:{content_hash}")),
            program_hash: None,
            source_kind,
            source_key: source_key.to_owned(),
            title: title.to_owned(),
            body,
            content_hash: StableHash::new(content_hash)
                .expect("generated content hash is non-empty"),
            semantic_hash: None,
            source_anchor: None,
            entity_ids,
            privacy,
            metadata: BTreeMap::new(),
            created_unix_ms: 0,
        },
        preferred_channel,
    }
}

fn agent_mcp_trace_resource_rag_candidates(
    resource: &AgentResource,
) -> Result<Vec<AgentMcpRagCandidate>, String> {
    let AgentResourceBody::Json(value) = &resource.body else {
        return Ok(vec![agent_mcp_rag_text_candidate(
            &resource.uri,
            &format!("Trace resource {}", resource.uri),
            ChunkSourceKind::AgentTrace,
            SearchChannel::Trace,
            agent_mcp_resource_body_text(resource)?,
            Vec::new(),
            PrivacyClass::Project,
        )]);
    };
    let Some(records) = value.as_array() else {
        return Ok(vec![agent_mcp_rag_json_candidate(
            &resource.uri,
            &format!("Trace resource {}", resource.uri),
            ChunkSourceKind::AgentTrace,
            SearchChannel::Trace,
            value,
            Vec::new(),
            PrivacyClass::Project,
        )?]);
    };
    records
        .iter()
        .enumerate()
        .map(|(index, record)| {
            let title = record
                .get("kind")
                .and_then(serde_json::Value::as_str)
                .map_or_else(
                    || format!("Trace record {index}"),
                    |kind| format!("Trace record {index} {kind}"),
                );
            agent_mcp_rag_json_candidate(
                &format!("{}.record.{index}", resource.uri),
                &title,
                ChunkSourceKind::AgentTrace,
                SearchChannel::Trace,
                record,
                Vec::new(),
                agent_mcp_json_privacy(record),
            )
        })
        .collect()
}

fn agent_mcp_json_privacy(value: &serde_json::Value) -> PrivacyClass {
    value
        .get("privacy_class")
        .or_else(|| value.get("privacy"))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("privacy_class"))
        })
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("privacy"))
        })
        .and_then(serde_json::Value::as_str)
        .and_then(PrivacyClass::parse)
        .unwrap_or(PrivacyClass::Project)
}

fn agent_mcp_resource_body_text(resource: &AgentResource) -> Result<String, String> {
    match &resource.body {
        AgentResourceBody::Json(value) => serde_json::to_string_pretty(value)
            .map_err(|error| format!("failed to serialize Agent resource body: {error}")),
        AgentResourceBody::Text(text) => Ok(text.clone()),
        AgentResourceBody::BytesBase64(_) => Ok(format!(
            "binary resource {} mime_type={} hash={}",
            resource.uri, resource.mime_type, resource.hash
        )),
    }
}

fn agent_mcp_rag_ranked_lists(
    candidates: &[AgentMcpRagCandidate],
    query: &RagQuery,
) -> Vec<Vec<SearchHit>> {
    [
        SearchChannel::ExactEntity,
        SearchChannel::Lexical,
        SearchChannel::Vector,
        SearchChannel::Graph,
        SearchChannel::History,
        SearchChannel::Diagnostics,
        SearchChannel::Trace,
        SearchChannel::Summary,
    ]
    .into_iter()
    .filter_map(|channel| {
        let mut scored = candidates
            .iter()
            .filter_map(|candidate| {
                agent_mcp_rag_score(candidate, query, channel).map(|score| (candidate, score))
            })
            .collect::<Vec<_>>();
        scored.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.chunk.id.cmp(&right.0.chunk.id))
        });
        if scored.is_empty() {
            return None;
        }
        Some(
            scored
                .into_iter()
                .enumerate()
                .map(|(index, (candidate, score))| SearchHit {
                    chunk_id: candidate.chunk.id.clone(),
                    channel,
                    rank: index + 1,
                    score: Some(score),
                })
                .collect(),
        )
    })
    .collect()
}

fn agent_mcp_rag_score(
    candidate: &AgentMcpRagCandidate,
    query: &RagQuery,
    channel: SearchChannel,
) -> Option<f64> {
    let haystack = agent_mcp_rag_haystack(candidate);
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
            let token_score = agent_mcp_rag_tokens(&query.text)
                .into_iter()
                .filter(|token| haystack.contains(token))
                .count();
            let token_score = agent_mcp_count_as_f64(token_score);
            (phrase + token_score > 0.0).then_some(phrase.mul_add(4.0, token_score))
        }
        SearchChannel::Graph => {
            let root_score = if query.graph_depth > 0 {
                let count = query
                    .roots
                    .iter()
                    .filter(|root| haystack.contains(&root.as_str().to_lowercase()))
                    .count();
                agent_mcp_count_as_f64(count)
            } else {
                0.0
            };
            let channel_score = f64::from(u8::from(
                candidate.preferred_channel == SearchChannel::Graph,
            ));
            (root_score + channel_score > 0.0).then_some(root_score + channel_score)
        }
        SearchChannel::History
        | SearchChannel::Diagnostics
        | SearchChannel::Trace
        | SearchChannel::Summary => {
            if candidate.preferred_channel != channel {
                return None;
            }
            let token_score = agent_mcp_rag_tokens(&query.text)
                .into_iter()
                .filter(|token| haystack.contains(token))
                .count();
            let token_score = agent_mcp_count_as_f64(token_score);
            (token_score > 0.0).then_some(token_score)
        }
        SearchChannel::Vector => {
            if candidate.preferred_channel != SearchChannel::Vector {
                return None;
            }
            candidate
                .chunk
                .metadata
                .get("search_score")
                .and_then(serde_json::Value::as_f64)
                .filter(|score| score.is_finite())
        }
    }
}

fn agent_mcp_count_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).unwrap_or(u32::MAX))
}

fn agent_mcp_rag_haystack(candidate: &AgentMcpRagCandidate) -> String {
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

fn agent_mcp_rag_tokens(text: &str) -> BTreeSet<String> {
    text.split(|character: char| {
        !(character.is_alphanumeric() || character == '.' || character == '_' || character == '-')
    })
    .map(str::trim)
    .filter(|token| !token.is_empty())
    .map(str::to_lowercase)
    .collect()
}

fn agent_mcp_rag_roots(arguments: &serde_json::Value) -> Result<Vec<PublicId>, String> {
    let Some(roots) = arguments.get("roots") else {
        return Ok(Vec::new());
    };
    let roots = roots
        .as_array()
        .ok_or_else(|| "arcweft.rag.query argument roots must be an array".to_owned())?;
    roots
        .iter()
        .map(|root| {
            let value = root
                .as_str()
                .ok_or_else(|| "arcweft.rag.query roots must contain only strings".to_owned())?;
            PublicId::new(value.to_owned())
                .map_err(|_| "arcweft.rag.query roots must not be empty".to_owned())
        })
        .collect()
}

fn agent_mcp_privacy_class_argument(
    arguments: &serde_json::Value,
    name: &str,
) -> Result<Option<PrivacyClass>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("arcweft.rag.query argument {name} must be a string"))?;
    PrivacyClass::parse(value).map(Some).ok_or_else(|| {
        format!(
            "arcweft.rag.query argument {name} must be one of public, project, sensitive, or secret"
        )
    })
}

fn agent_mcp_max_privacy_argument(
    arguments: &serde_json::Value,
    tool: &str,
) -> Result<PrivacyClass, String> {
    let Some(value) = arguments.get("max_privacy") else {
        return Ok(PrivacyClass::Project);
    };
    let value = value
        .as_str()
        .ok_or_else(|| format!("{tool} argument max_privacy must be a string"))?;
    PrivacyClass::parse(value).ok_or_else(|| {
        format!("{tool} argument max_privacy must be one of public, project, sensitive, or secret")
    })
}

fn agent_mcp_observation_debug_read_privacy_error(
    resource: &str,
    privacy: PrivacyClass,
    max_privacy: PrivacyClass,
) -> Option<serde_json::Value> {
    (!privacy.is_allowed_by(max_privacy)).then(|| {
        serde_json::json!({
            "status": "blocked",
            "error": format!(
                "arcweft.{resource} is {privacy} and exceeds max_privacy {max_privacy}",
                privacy = privacy.as_str(),
                max_privacy = max_privacy.as_str(),
            ),
            "resource": resource,
            "privacy": privacy.as_str(),
            "max_privacy": max_privacy.as_str(),
        })
    })
}

fn agent_mcp_object_entity_ids(object: &AgentObservedObject) -> Vec<PublicId> {
    agent_mcp_public_ids(
        [Some(object.id.as_str()), object.entity.as_deref()]
            .into_iter()
            .flatten(),
    )
}

fn agent_mcp_public_ids<'a>(values: impl IntoIterator<Item = &'a str>) -> Vec<PublicId> {
    values
        .into_iter()
        .filter_map(|value| PublicId::new(value.to_owned()).ok())
        .collect()
}

fn agent_mcp_rag_program_hash(
    state: &AgentMcpState,
    candidates: &[AgentMcpRagCandidate],
) -> Result<StableHash, String> {
    let candidate_hashes = candidates
        .iter()
        .map(|candidate| candidate.chunk.content_hash.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let seed = state.report.as_ref().map_or_else(
        || candidate_hashes.clone(),
        |report| {
            format!(
                "{}:{}:{}:{}\n{}",
                report.source, report.state_hash, report.render_hash, report.tick, candidate_hashes
            )
        },
    );
    StableHash::new(agent_mcp_content_hash(seed))
        .map_err(|_| "failed to build Agent RAG program hash".to_owned())
}

fn agent_mcp_content_hash(bytes: impl AsRef<[u8]>) -> String {
    format!("blake3:{}", blake3::hash(bytes.as_ref()).to_hex())
}

fn agent_mcp_truncate_utf8(text: &str, max_bytes: usize) -> (String, bool) {
    if text.len() <= max_bytes {
        return (text.to_owned(), false);
    }
    let mut end = max_bytes;
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    (text[..end].to_owned(), true)
}

fn agent_mcp_observe_if_requested(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), String> {
    if !agent_mcp_arguments_request_observe(arguments) {
        return Ok(());
    }
    let observed = agent_mcp_run_observation(arguments, adapter_registrars)?;
    agent_mcp_store_observation(state, observed);
    Ok(())
}

fn agent_mcp_json_tool_result(
    value: &serde_json::Value,
    context: &str,
) -> Result<McpCallToolResult, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize Agent {context}: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: false,
    })
}

fn agent_mcp_json_tool_error(
    value: &serde_json::Value,
    context: &str,
) -> Result<McpCallToolResult, String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("failed to serialize Agent {context}: {error}"))?;
    Ok(McpCallToolResult {
        content: vec![McpContentBlock::Text { text }],
        is_error: true,
    })
}

fn agent_mcp_observation_state_summary(report: &AgentObservationReport) -> serde_json::Value {
    serde_json::json!({
        "status": report.status,
        "final_status": report.final_status,
        "tick": report.tick,
        "frame_id": report.frame_id,
        "state_hash": report.state_hash,
        "render_hash": report.render_hash,
        "source": report.source,
        "steps": report.steps,
        "task_requests": report.task_requests,
        "capture_time_millis": report.capture_time_millis,
    })
}

fn agent_mcp_wait_report_value(
    report: &AgentObservationReport,
    matched: bool,
    stable_seen_before: u32,
    polls: u64,
) -> serde_json::Value {
    serde_json::json!({
        "matched": matched,
        "stable_seen_before": stable_seen_before,
        "polls": polls,
        "tick": report.tick,
        "frame_id": report.frame_id,
        "state_hash": report.state_hash,
        "render_hash": report.render_hash,
        "final_status": report.final_status,
        "signals": report.signals,
        "metrics": report.metrics,
    })
}

fn agent_mcp_predicate_matches(predicate: &Predicate, report: &AgentObservationReport) -> bool {
    match predicate {
        Predicate::Compare { probe, op, value } => agent_mcp_probe_value(probe, report)
            .is_some_and(|actual| agent_mcp_compare_values(&actual, *op, value)),
        Predicate::Exists { probe } => agent_mcp_probe_value(probe, report).is_some(),
        Predicate::ActionEnabled { target } => report
            .actions
            .iter()
            .any(|action| action.enabled && action.target == target.as_str()),
        Predicate::DiagnosticsHasError => report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == AgentDiagnosticSeverity::Error),
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| agent_mcp_predicate_matches(predicate, report)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| agent_mcp_predicate_matches(predicate, report)),
        Predicate::Not { predicate } => !agent_mcp_predicate_matches(predicate, report),
    }
}

fn agent_mcp_probe_value(probe: &Probe, report: &AgentObservationReport) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } => agent_mcp_assignment_value(&report.signals, target.as_str()),
        Probe::Metric { target } => agent_mcp_assignment_value(&report.metrics, target.as_str()),
        Probe::StatePath { path } => serde_json::to_value(report)
            .ok()
            .and_then(|value| agent_json_path(&value, path.as_str()).cloned())
            .and_then(|value| agent_mcp_agent_value(&value).ok()),
        Probe::ObservationField { path } if path.as_str() == "tick" => {
            u64::try_from(report.tick).ok().map(AgentValue::U64)
        }
        Probe::ObservationField { path } if path.as_str() == "frame_id" => {
            Some(AgentValue::String(report.frame_id.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "state_hash" => {
            Some(AgentValue::String(report.state_hash.clone()))
        }
        Probe::ObservationField { path } if path.as_str() == "render_hash" => {
            Some(AgentValue::String(report.render_hash.clone()))
        }
        Probe::ObservationField { path } => path
            .as_str()
            .strip_prefix("signals.")
            .and_then(|signal| agent_mcp_assignment_value(&report.signals, signal))
            .or_else(|| {
                path.as_str()
                    .strip_prefix("metrics.")
                    .and_then(|metric| agent_mcp_assignment_value(&report.metrics, metric))
            })
            .or_else(|| {
                serde_json::to_value(report)
                    .ok()
                    .and_then(|value| agent_json_path(&value, path.as_str()).cloned())
                    .and_then(|value| agent_mcp_agent_value(&value).ok())
            }),
    }
}

fn agent_mcp_assignment_value(assignments: &[AgentAssignment], name: &str) -> Option<AgentValue> {
    assignments
        .iter()
        .find(|assignment| assignment.name.trim_start_matches('@') == name.trim_start_matches('@'))
        .map(agent_assignment_value)
}

fn agent_mcp_compare_values(actual: &AgentValue, op: CompareOp, expected: &AgentValue) -> bool {
    match op {
        CompareOp::Eq => agent_mcp_values_equal(actual, expected),
        CompareOp::NotEq => !agent_mcp_values_equal(actual, expected),
        CompareOp::Greater => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(i32::is_positive)
        }
        CompareOp::GreaterOrEqual => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(|order| order >= 0)
        }
        CompareOp::Less => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(i32::is_negative)
        }
        CompareOp::LessOrEqual => {
            agent_mcp_compare_numeric_values(actual, expected).is_some_and(|order| order <= 0)
        }
    }
}

fn agent_mcp_values_equal(left: &AgentValue, right: &AgentValue) -> bool {
    match (left, right) {
        (AgentValue::Entity(left), AgentValue::String(right))
        | (AgentValue::String(right), AgentValue::Entity(left)) => left.as_str() == right,
        _ => left == right,
    }
}

fn agent_mcp_compare_numeric_values(left: &AgentValue, right: &AgentValue) -> Option<i32> {
    Some(match (left, right) {
        (AgentValue::I64(left), AgentValue::I64(right)) => agent_mcp_compare_order(left.cmp(right)),
        (AgentValue::U64(left), AgentValue::U64(right)) => agent_mcp_compare_order(left.cmp(right)),
        (AgentValue::I64(left), AgentValue::U64(right)) => {
            if *left < 0 {
                -1
            } else {
                agent_mcp_compare_order(u64::try_from(*left).ok()?.cmp(right))
            }
        }
        (AgentValue::U64(left), AgentValue::I64(right)) => {
            if *right < 0 {
                1
            } else {
                agent_mcp_compare_order(left.cmp(&u64::try_from(*right).ok()?))
            }
        }
        (AgentValue::F64(left), AgentValue::F64(right)) => {
            agent_mcp_compare_order(left.partial_cmp(right)?)
        }
        _ => return None,
    })
}

fn agent_mcp_compare_order(order: std::cmp::Ordering) -> i32 {
    match order {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

fn agent_json_path<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    if path.is_empty() {
        return Some(value);
    }
    path.split('.').try_fold(value, |current, segment| {
        current.as_object().and_then(|object| object.get(segment))
    })
}

fn agent_mcp_call_trace_read(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    let path = arguments
        .get("path")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.trace.read requires arguments.path".to_owned())?;
    let records = super::read_and_validate_agent_trace_records(Path::new(path))?;
    let resource =
        trace_resource(&records).map_err(|error| format!("failed to serialize trace: {error}"))?;
    state
        .trace_resources
        .retain(|cached| cached.uri != resource.uri);
    state.trace_resources.push(resource.clone());
    tool_result_for_resource(&resource)
        .map_err(|error| format!("failed to serialize MCP trace resource: {error}"))
}

fn agent_mcp_arguments_request_observe(arguments: &serde_json::Value) -> bool {
    arguments.get("source").is_some() || arguments.get("profile").is_some()
}

fn agent_mcp_run_observation(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentMcpObservation, String> {
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
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let lowered = lower_source_runtime_plan_with_stats_and_options(&checked.hir, &runtime_options)
        .map_err(|_| "failed to lower runtime plan".to_owned())?;
    let native_session = agent_native_capture_session_for_hir(&checked.hir)
        .map_err(|_| "failed to create native capture session".to_owned())?;
    let mut plan = lowered.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())
        .map_err(|_| "failed to select runtime entry".to_owned())?;
    let mut runtime = NativeAgentRuntimeState {
        executor: RuntimeExecutorInstance::new(plan, options.executor, pure_config),
        catalog: lowered.line_display_catalog,
        source_path: selection.path().to_owned(),
        host_policy,
        native_session,
        task_events: Vec::new(),
        next_tick: 0,
    };
    let mut observed = run_agent_observation(
        &mut runtime.executor,
        &runtime.catalog,
        AgentObservationRunContext {
            host_config: NativeRunHost {
                source_path: Some(&runtime.source_path),
                policy: &runtime.host_policy,
                adapter_registrars,
            },
            options: &options,
            source_path: &runtime.source_path,
            native_session: Some(&mut runtime.native_session),
            tick_offset: 0,
            input_events: Vec::new(),
            task_events: &mut runtime.task_events,
        },
    )
    .map_err(|error| error.to_string())?;
    extend_agent_observation_with_runtime_images(
        &mut observed,
        &runtime.source_path,
        &runtime.executor,
        &options,
    );
    let image_output = agent_observe_image_output(
        &mut observed.report,
        &options,
        Some(&mut runtime.native_session),
        &observed.image_frames,
    )
    .map_err(|_| "failed to build MCP observe image output".to_owned())?;
    runtime.next_tick = observed.report.tick.saturating_add(1);
    Ok(AgentMcpObservation {
        report: observed.report,
        image_output,
        image_frames: observed.image_frames,
        runtime,
        options,
    })
}

fn agent_mcp_observe_runtime(
    runtime: &mut NativeAgentRuntimeState,
    options: &AgentObserveOptions,
    input_events: Vec<RuntimeInputEvent>,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentMcpFrame, String> {
    let tick_offset = runtime.next_tick;
    let mut observed = run_agent_observation(
        &mut runtime.executor,
        &runtime.catalog,
        AgentObservationRunContext {
            host_config: NativeRunHost {
                source_path: Some(&runtime.source_path),
                policy: &runtime.host_policy,
                adapter_registrars,
            },
            options,
            source_path: &runtime.source_path,
            native_session: Some(&mut runtime.native_session),
            tick_offset,
            input_events,
            task_events: &mut runtime.task_events,
        },
    )
    .map_err(|error| error.to_string())?;
    extend_agent_observation_with_runtime_images(
        &mut observed,
        &runtime.source_path,
        &runtime.executor,
        options,
    );
    let image_output = agent_observe_image_output(
        &mut observed.report,
        options,
        Some(&mut runtime.native_session),
        &observed.image_frames,
    )
    .map_err(|_| "failed to build MCP action image output".to_owned())?;
    let resources = agent_observe_list_resources(&observed.report, image_output.as_ref())
        .map_err(|_| "failed to build MCP action resources".to_owned())?;
    runtime.next_tick = observed.report.tick.saturating_add(1);
    Ok(AgentMcpFrame {
        report: observed.report,
        image_output,
        image_frames: observed.image_frames,
        resources,
    })
}

fn extend_agent_observation_with_runtime_images(
    observed: &mut AgentObservationRunOutput,
    source_path: &Path,
    executor: &RuntimeExecutorInstance,
    options: &AgentObserveOptions,
) {
    let (image_observation, image_diagnostics) = agent_runtime_presentation_image_observation(
        source_path,
        observed.report.tick,
        &observed.report.viewport,
        &executor.fiber().observations.calls,
        agent_observe_capture_time_seconds(options),
    );
    observed.report.diagnostics.extend(image_diagnostics);
    if !image_observation.objects.is_empty() {
        observed.report.objects.extend(image_observation.objects);
        observed.image_frames.extend(image_observation.image_frames);
        agent_refresh_observation_object_indexes(&mut observed.report);
    }
}

fn agent_mcp_call_resource_read(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    let max_privacy = agent_mcp_max_privacy_argument(arguments, "arcweft.resource.read")?;
    let uri = arguments
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.resource.read requires arguments.uri".to_owned())?;
    let audit_path = agent_mcp_optional_debug_store_path(arguments);
    if let Some(resource) = agent_mcp_session_context_resource_for_uri(state, uri)
        .map_err(|error| format!("failed to build Agent session context: {error}"))?
    {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
        {
            agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
            return agent_mcp_json_tool_error(&error, "resource privacy");
        }
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
        return tool_result_for_resource(&resource)
            .map_err(|error| format!("failed to serialize MCP session context: {error}"));
    }
    if let Some(resource) = agent_mcp_cached_trace_resource(state, uri) {
        if let Some(error) =
            agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
        {
            agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
            return agent_mcp_json_tool_error(&error, "resource privacy");
        }
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
        return tool_result_for_resource(&resource)
            .map_err(|error| format!("failed to serialize MCP trace resource: {error}"));
    }
    let Some(report) = state.report.clone() else {
        return Err("arcweft.resource.read requires a prior arcweft.observe call or arcweft.trace.read call".to_owned());
    };
    let image_output = state.image_output.clone();
    let resource = if let Some(resource) = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(&report, image_output.as_ref(), uri))
    {
        resource
    } else {
        agent_mcp_uncached_resource_by_uri(&report, uri, state)
            .map_err(|_| format!("failed to read Agent resource `{uri}`"))?
    };
    if let Some(error) =
        agent_mcp_resource_read_privacy_error(&resource, max_privacy, "arcweft.resource.read")
    {
        agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "blocked")?;
        return agent_mcp_json_tool_error(&error, "resource privacy");
    }
    agent_mcp_audit_resource_read(audit_path, uri, &resource, max_privacy, "allowed")?;
    tool_result_for_resource(&resource)
        .map_err(|error| format!("failed to serialize MCP tool resource: {error}"))
}

fn agent_mcp_audit_resource_read(
    path: Option<&str>,
    requested_uri: &str,
    resource: &AgentResource,
    max_privacy: PrivacyClass,
    outcome: &str,
) -> Result<(), String> {
    let Some(path) = path else {
        return Ok(());
    };
    let mut store = DebugStore::open(path).map_err(|error| {
        format!("arcweft.resource.read failed to open audit store `{path}`: {error}")
    })?;
    let session_id = SessionId::new("session.mcp.resource_read")
        .map_err(|error| format!("invalid MCP resource audit session id: {error}"))?;
    store
        .upsert_session(&DebugSession {
            session_id: session_id.clone(),
            program_hash: None,
            profile: "mcp".to_owned(),
            transport: "stdio".to_owned(),
            started_unix_ms: 0,
            ended_unix_ms: None,
            status: DebugSessionStatus::Running,
            metadata: BTreeMap::from([(
                "surface".to_owned(),
                serde_json::Value::String("arcweft.resource.read".to_owned()),
            )]),
        })
        .map_err(|error| {
            format!("arcweft.resource.read failed to upsert audit session in `{path}`: {error}")
        })?;
    let privacy = agent_mcp_resource_privacy(resource);
    let sequence = store.next_event_sequence(&session_id).map_err(|error| {
        format!("arcweft.resource.read failed to allocate audit sequence in `{path}`: {error}")
    })?;
    store
        .append(&DebugEvent {
            schema_version: 1,
            session_id,
            run_id: None,
            sequence,
            tick: None,
            kind: DebugEventKind::ResourceRead,
            payload: serde_json::json!({
                "surface": "arcweft.resource.read",
                "requested_uri": requested_uri,
                "resolved_uri": resource.uri,
                "kind": resource.kind,
                "mime_type": resource.mime_type,
                "hash": resource.hash,
                "privacy": privacy.as_str(),
                "max_privacy": max_privacy.as_str(),
                "outcome": outcome,
            }),
            created_unix_ms: 0,
        })
        .map_err(|error| {
            format!("arcweft.resource.read failed to write audit event to `{path}`: {error}")
        })?;
    Ok(())
}

fn agent_mcp_resource_read_privacy_error(
    resource: &AgentResource,
    max_privacy: PrivacyClass,
    surface: &str,
) -> Option<serde_json::Value> {
    let privacy = agent_mcp_resource_privacy(resource);
    (!privacy.is_allowed_by(max_privacy)).then(|| {
        serde_json::json!({
            "status": "blocked",
            "error": format!(
                "{} resource {} is {} and exceeds max_privacy {}",
                surface,
                resource.uri,
                privacy.as_str(),
                max_privacy.as_str(),
            ),
            "resource": resource.uri,
            "privacy": privacy.as_str(),
            "max_privacy": max_privacy.as_str(),
        })
    })
}

fn agent_mcp_resource_read_privacy_message(value: &serde_json::Value) -> String {
    value
        .get("error")
        .and_then(serde_json::Value::as_str)
        .map_or_else(|| value.to_string(), str::to_owned)
}

fn agent_mcp_resource_privacy(resource: &AgentResource) -> PrivacyClass {
    if resource.kind == AgentResourceKind::Image
        || resource.image.is_some()
        || matches!(resource.body, AgentResourceBody::BytesBase64(_))
    {
        return PrivacyClass::Sensitive;
    }
    match &resource.body {
        AgentResourceBody::Json(value) => agent_mcp_resource_json_privacy(value),
        AgentResourceBody::Text(_) => PrivacyClass::Project,
        AgentResourceBody::BytesBase64(_) => PrivacyClass::Sensitive,
    }
}

fn agent_mcp_resource_json_privacy(value: &serde_json::Value) -> PrivacyClass {
    value.as_array().map_or_else(
        || agent_mcp_json_privacy(value),
        |items| {
            items
                .iter()
                .map(agent_mcp_json_privacy)
                .max()
                .unwrap_or(PrivacyClass::Project)
        },
    )
}

fn agent_mcp_current_resources(state: &AgentMcpState) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = if let Some(report) = &state.report {
        agent_observe_list_resources(report, state.image_output.as_ref())?
    } else {
        Vec::new()
    };
    if let Some(context) = agent_mcp_session_context_resource(state).map_err(|error| {
        eprintln!("error: failed to build Agent session context: {error}");
        ExitCode::FAILURE
    })? {
        resources.retain(|candidate| candidate.uri != context.uri);
        resources.insert(0, context);
    }
    for resource in state
        .capture_resources
        .iter()
        .chain(state.trace_resources.iter())
    {
        resources.retain(|candidate| candidate.uri != resource.uri);
        resources.push(resource.clone());
    }
    Ok(resources)
}

fn agent_mcp_session_context_resource_for_uri(
    state: &AgentMcpState,
    uri: &str,
) -> Result<Option<AgentResource>, serde_json::Error> {
    agent_mcp_session_context_resource(state).map(|resource| {
        resource.filter(|resource| {
            resource.uri == uri
                || agent_uri_without_query(&resource.uri).is_some_and(|base| base == uri)
        })
    })
}

fn agent_mcp_session_context_resource(
    state: &AgentMcpState,
) -> Result<Option<AgentResource>, serde_json::Error> {
    if state.report.is_none()
        && state.capture_resources.is_empty()
        && state.trace_resources.is_empty()
        && state.rag_context_packs.is_empty()
        && state.runtime.is_none()
        && state.native_capture_session.is_none()
    {
        return Ok(None);
    }
    let session_id = state
        .report
        .as_ref()
        .map_or("mcp", |report| report.session_id.as_str());
    let latest_observation = state.report.as_ref().map(|report| {
        serde_json::json!({
            "tick": report.tick,
            "frame_id": report.frame_id,
            "state_hash": report.state_hash,
            "render_hash": report.render_hash,
            "final_status": report.final_status,
            "capture_time_millis": report.capture_time_millis,
            "viewport": report.viewport,
            "counts": {
                "images": report.images.len(),
                "layers": report.layers.len(),
                "objects": report.objects.len(),
                "actions": report.actions.len(),
                "logs": report.logs.len(),
                "signals": report.signals.len(),
                "metrics": report.metrics.len(),
                "events": report.events.len(),
                "diagnostics": report.diagnostics.len(),
                "task_requests": report.task_requests,
            },
            "source_present": !report.source.is_empty(),
        })
    });
    let trace_resources = state
        .trace_resources
        .iter()
        .map(|resource| {
            serde_json::json!({
                "uri": resource.uri,
                "mime_type": resource.mime_type,
                "hash": resource.hash,
            })
        })
        .collect::<Vec<_>>();
    let rag_queries = state
        .rag_context_packs
        .iter()
        .map(|pack| {
            serde_json::json!({
                "query_id": pack.query.query_id,
                "text": pack.query.text,
                "item_count": pack.items.len(),
                "truncated": pack.truncated,
            })
        })
        .collect::<Vec<_>>();
    let body = serde_json::json!({
        "schema_version": 1,
        "kind": "session_context",
        "privacy_class": "project",
        "session_id": session_id,
        "observed": state.report.is_some(),
        "latest_observation": latest_observation,
        "resources": {
            "capture_resource_count": state.capture_resources.len(),
            "trace_resource_count": state.trace_resources.len(),
            "rag_query_count": state.rag_context_packs.len(),
            "cached_capture_uris": state.capture_resources.iter().map(|resource| &resource.uri).collect::<Vec<_>>(),
            "trace_resources": trace_resources,
            "rag_queries": rag_queries,
        },
        "runtime": {
            "native_runtime_active": state.runtime.is_some(),
            "native_capture_session_active": state.runtime.is_some() || state.native_capture_session.is_some(),
        },
    });
    let bytes = serde_json::to_vec(&body)?;
    Ok(Some(AgentResource {
        uri: format!("arcweft://session/{session_id}/context.json"),
        kind: AgentResourceKind::SessionContext,
        mime_type: "application/json".to_owned(),
        hash: agent_mcp_content_hash(bytes),
        image: None,
        body: AgentResourceBody::Json(body),
    }))
}

fn agent_mcp_cached_trace_resource(state: &AgentMcpState, uri: &str) -> Option<AgentResource> {
    state
        .trace_resources
        .iter()
        .rev()
        .find(|resource| resource.uri == uri)
        .cloned()
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

fn agent_mcp_uncached_resource_by_uri(
    report: &AgentObservationReport,
    uri: &str,
    state: &mut AgentMcpState,
) -> Result<AgentResource, ExitCode> {
    let image_frames = state.image_frames.clone();
    let native_session = agent_mcp_native_capture_session_mut(state)?;
    agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
        report,
        uri,
        None,
        agent_report_capture_time_seconds(report),
        Some(native_session),
        &image_frames,
    )
}

fn agent_uri_without_query(uri: &str) -> Option<&str> {
    uri.split_once('?').map(|(base, _)| base)
}

fn agent_mcp_call_capture(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    if arguments.get("source").is_some() || arguments.get("profile").is_some() {
        let observed = agent_mcp_run_observation(
            &agent_mcp_capture_observe_arguments(arguments),
            adapter_registrars,
        )?;
        agent_mcp_store_observation(state, observed);
    }
    let Some(report) = state.report.clone() else {
        return Err(
            "arcweft.capture requires a prior arcweft.observe call, arguments.source, or arguments.profile".to_owned(),
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
    let image_frames = state.image_frames.clone();
    let native_session = agent_mcp_native_capture_session_mut(state)?;
    agent_native_capture_resource_with_session_and_frame_store(
        report,
        request,
        native_session,
        &image_frames,
    )
}

fn agent_mcp_native_capture_session_mut(
    state: &mut AgentMcpState,
) -> Result<&mut arcweft_render_native::NativeOffscreenCaptureSession, ExitCode> {
    if state.runtime.is_none() {
        agent_mcp_ensure_native_capture_session(state)?;
        return state
            .native_capture_session
            .as_mut()
            .ok_or(ExitCode::FAILURE);
    }
    state
        .runtime
        .as_mut()
        .map(|runtime| &mut runtime.native_session)
        .ok_or(ExitCode::FAILURE)
}

fn agent_mcp_ensure_native_capture_session(state: &mut AgentMcpState) -> Result<(), ExitCode> {
    if state.native_capture_session.is_none() {
        state.native_capture_session = Some(
            arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
                eprintln!("error: native capture failed: {error}");
                ExitCode::FAILURE
            })?,
        );
    }
    Ok(())
}

fn agent_native_capture_session_for_hir(
    hir: &arcweft_lang_hir::model::HirModule,
) -> Result<arcweft_render_native::NativeOffscreenCaptureSession, ExitCode> {
    let text_helpers =
        arcweft_compiler::lower_source_text_pure_helper_candidates(hir).map_err(|errors| {
            for error in errors {
                eprintln!("error: failed to lower Arcweft text renderer function: {error}");
            }
            ExitCode::FAILURE
        })?;
    let mut native_session =
        arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    arcweft_render_native::register_arcweft_pure_text_motions(
        native_session.motion_registry_mut(),
        &text_helpers.motions,
    )
    .map_err(|error| {
        eprintln!("error: failed to register Arcweft text motion functions: {error}");
        ExitCode::FAILURE
    })?;
    arcweft_render_native::register_arcweft_pure_text_effects(
        native_session.effect_registry_mut(),
        &text_helpers.effects,
    )
    .map_err(|error| {
        eprintln!("error: failed to register Arcweft text effect functions: {error}");
        ExitCode::FAILURE
    })?;
    arcweft_render_native::register_arcweft_pure_text_shaders(
        native_session.shader_registry_mut(),
        &text_helpers.shaders,
    )
    .map_err(|error| {
        eprintln!("error: failed to register Arcweft text shader functions: {error}");
        ExitCode::FAILURE
    })?;
    Ok(native_session)
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
            agent_mcp_capture_time_argument(arguments, "arcweft.capture")?
                .unwrap_or(request.capture_time_seconds);
        return Ok(request);
    }
    let page = agent_mcp_capture_page(arguments)?;
    let capture_time_seconds =
        agent_mcp_capture_time_seconds(arguments, report, "arcweft.capture")?;
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
        capture_step: report.steps,
        capture_time_seconds,
    })
}

fn agent_mcp_capture_page(arguments: &serde_json::Value) -> Result<usize, String> {
    agent_mcp_page_argument(arguments, "arcweft.capture")
}

fn agent_mcp_capture_time_seconds(
    arguments: &serde_json::Value,
    report: &AgentObservationReport,
    tool: &str,
) -> Result<f32, String> {
    Ok(
        agent_mcp_capture_time_argument(arguments, tool)?.unwrap_or_else(|| {
            agent_mcp_usize_argument(arguments, "capture_step").map_or_else(
                || agent_report_capture_time_seconds(report),
                agent_capture_time_seconds_from_step,
            )
        }),
    )
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
    let source = arguments.get("source").and_then(serde_json::Value::as_str);
    let profile = arguments
        .get("profile")
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned);
    if source.is_some() && profile.is_some() {
        return Err(
            "arcweft.observe arguments.source and arguments.profile are mutually exclusive"
                .to_owned(),
        );
    }
    if source.is_none() && profile.is_none() {
        return Err("arcweft.observe requires arguments.source or arguments.profile".to_owned());
    }
    if arguments.get("renderer").is_some() {
        return Err("arcweft.observe no longer accepts arguments.renderer".to_owned());
    }
    Ok(AgentObserveOptions {
        path: source.map(PathBuf::from),
        profile: ProfileOptions {
            profile,
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
        capture_step: agent_mcp_usize_argument(arguments, "capture_step"),
        mode: CliRuntimeStepMode::Drain,
        max_ops: agent_mcp_usize_argument(arguments, "max_ops").unwrap_or(64),
        values: Vec::new(),
        viewport_width: agent_mcp_u32_argument(arguments, "viewport_width", "arcweft.observe")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH),
        viewport_height: agent_mcp_u32_argument(arguments, "viewport_height", "arcweft.observe")?
            .unwrap_or(AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT),
        textbox_height: agent_mcp_u32_argument(arguments, "textbox_height", "arcweft.observe")?,
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
        capture_time_seconds: agent_mcp_capture_time_argument(arguments, "arcweft.observe")?,
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

fn agent_mcp_u64_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<u64>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument {name} must be a positive integer"))
        .map(Some)
}

fn agent_mcp_u32_argument(
    arguments: &serde_json::Value,
    name: &str,
    tool: &str,
) -> Result<Option<u32>, String> {
    let Some(value) = arguments.get(name) else {
        return Ok(None);
    };
    let value = value
        .as_u64()
        .ok_or_else(|| format!("{tool} argument {name} must be a positive integer"))?;
    u32::try_from(value)
        .map(Some)
        .map_err(|_| format!("{tool} argument {name} is too large"))
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
    let mut observed = agent_observation_for_options(options, adapter_registrars)?;
    let image_output = agent_observe_image_output(
        &mut observed.report,
        options,
        Some(&mut observed.native_session),
        &observed.image_frames,
    )?;
    if let Some(uri) = &options.read_uri {
        let resource = agent_observe_cached_image_resource(
            &observed.report,
            image_output.as_ref(),
            uri,
        )
        .map_or_else(
            || {
                agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
                    &observed.report,
                    uri,
                    options.page,
                    agent_observe_capture_time_seconds(options),
                    Some(&mut observed.native_session),
                    &observed.image_frames,
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
        let resource = agent_observe_resource(&observed.report, image_output.as_ref(), resource)?;
        if options.mcp {
            let resource = agent_observe_mcp_resource_output(resource, options.mcp_format)?;
            print_json(&resource)
        } else {
            print_json(&resource)
        }
    } else if options.json {
        print_json(&observed.report)
    } else {
        println!(
            "ok: {} ({} object(s), {} diagnostic(s), render_hash={})",
            observed.report.source,
            observed.report.objects.len(),
            observed.report.diagnostics.len(),
            observed.report.render_hash
        );
        Ok(())
    }
}

fn agent_observation_report_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationReport, ExitCode> {
    agent_observation_for_options(options, adapter_registrars).map(|observed| observed.report)
}

fn native_agent_runtime_state_for_options(
    options: &AgentObserveOptions,
) -> Result<NativeAgentRuntimeState, ExitCode> {
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
    let native_session = agent_native_capture_session_for_hir(&checked.hir)?;
    let host_policy = native_host_policy_for_selection(&selection)?;
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let lowered = lower_source_runtime_plan_with_stats_and_options(&checked.hir, &runtime_options)
        .map_err(|errors| {
            for error in errors {
                eprintln!("error: {}", error.message());
            }
            ExitCode::FAILURE
        })?;
    let mut plan = lowered.plan;
    let entry = options.entry.as_deref().or(selection.entry());
    apply_runtime_entry_selection(&mut plan, entry, options.flow.as_deref())?;
    Ok(NativeAgentRuntimeState {
        executor: RuntimeExecutorInstance::new(plan, options.executor, pure_config),
        catalog: lowered.line_display_catalog,
        source_path: selection.path().to_owned(),
        host_policy,
        native_session,
        task_events: Vec::new(),
        next_tick: 0,
    })
}

fn agent_observation_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationState, ExitCode> {
    let mut runtime = native_agent_runtime_state_for_options(options)?;
    let mut observed = run_agent_observation(
        &mut runtime.executor,
        &runtime.catalog,
        AgentObservationRunContext {
            host_config: NativeRunHost {
                source_path: Some(&runtime.source_path),
                policy: &runtime.host_policy,
                adapter_registrars,
            },
            options,
            source_path: &runtime.source_path,
            native_session: Some(&mut runtime.native_session),
            tick_offset: 0,
            input_events: Vec::new(),
            task_events: &mut runtime.task_events,
        },
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    extend_agent_observation_with_runtime_images(
        &mut observed,
        &runtime.source_path,
        &runtime.executor,
        options,
    );
    Ok(AgentObservationState {
        report: observed.report,
        image_frames: observed.image_frames,
        native_session: runtime.native_session,
    })
}

pub(in crate::app::agent) fn agent_script_run_native_bundle(
    options: &AgentScriptRunOptions,
    input: &AgentScriptRunInput,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentScriptRunReport, ExitCode> {
    let session = NativeAgentScriptSession::new(
        options,
        adapter_registrars,
        input.program_hash.clone(),
        input.project_entities.clone(),
        input.project_graph.clone(),
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
    let blob_result = super::write_agent_capture_blobs(
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

#[derive(Debug, Error)]
enum NativeAgentScriptSessionError {
    #[error("native Agent Script observation failed")]
    Observe,
    #[error("native Agent Script capture failed")]
    Capture,
    #[error("native Agent Script resource read failed")]
    ResourceRead,
    #[error("native Agent Script action is not currently selectable")]
    ActionUnavailable,
    #[error("native Agent Script action kind is not supported by the native semantic dispatcher")]
    UnsupportedAction,
}

struct NativeAgentScriptSession<'a> {
    options: AgentObserveOptions,
    adapter_registrars: &'a [NativeAdapterRegistrar],
    program_hash: String,
    project_entities: Vec<RequiredEntity>,
    project_graph: arcweft_agent_protocol::protocol::AgentProjectGraph,
    runtime: Option<NativeAgentRuntimeState>,
    observed: Option<NativeAgentObservedSnapshot>,
    capture_blobs: Vec<AgentCaptureBlob>,
}

impl<'a> NativeAgentScriptSession<'a> {
    fn new(
        options: &AgentScriptRunOptions,
        adapter_registrars: &'a [NativeAdapterRegistrar],
        program_hash: String,
        project_entities: Vec<RequiredEntity>,
        project_graph: arcweft_agent_protocol::protocol::AgentProjectGraph,
    ) -> Self {
        Self {
            options: AgentObserveOptions {
                path: options.native_source.clone(),
                profile: options.native_profile.clone(),
                entry: options.entry.clone(),
                flow: options.flow.clone(),
                executor: options.executor,
                pure_backend: options.pure_backend,
                pure_workers: options.pure_workers,
                pure_batch_min_len: options.pure_batch_min_len,
                pure_object_artifacts: options.pure_object_artifacts,
                math_backend: options.math_backend,
                math_wgpu_min_elements: options.math_wgpu_min_elements,
                steps: options.native_steps,
                capture_step: None,
                mode: options.native_mode,
                max_ops: options.native_max_ops,
                values: options.values.clone(),
                viewport_width: options.viewport_width,
                viewport_height: options.viewport_height,
                textbox_height: options.textbox_height,
                image: None,
                capture: None,
                layer: None,
                object: None,
                page: None,
                capture_time_seconds: options.capture_time_seconds,
                resource: None,
                read_uri: None,
                mcp: false,
                mcp_format: AgentObserveMcpFormat::Read,
                out: None,
                json: true,
            },
            adapter_registrars,
            program_hash,
            project_entities,
            project_graph,
            runtime: None,
            observed: None,
            capture_blobs: Vec::new(),
        }
    }

    fn capture_blobs(&self) -> &[AgentCaptureBlob] {
        &self.capture_blobs
    }

    fn observe_report(&mut self) -> Result<&AgentObservationReport, NativeAgentScriptSessionError> {
        if self.observed.is_none() {
            self.refresh_observation(Vec::new())?;
        }
        self.observed
            .as_ref()
            .map(|observed| &observed.report)
            .ok_or(NativeAgentScriptSessionError::Observe)
    }

    fn runtime_state(
        &mut self,
    ) -> Result<&mut NativeAgentRuntimeState, NativeAgentScriptSessionError> {
        if self.runtime.is_none() {
            let runtime = native_agent_runtime_state_for_options(&self.options)
                .map_err(|_| NativeAgentScriptSessionError::Observe)?;
            self.runtime = Some(runtime);
        }
        self.runtime
            .as_mut()
            .ok_or(NativeAgentScriptSessionError::Observe)
    }

    fn refresh_observation(
        &mut self,
        input_events: Vec<RuntimeInputEvent>,
    ) -> Result<&AgentObservationReport, NativeAgentScriptSessionError> {
        let options = self.options.clone();
        let adapter_registrars = self.adapter_registrars;
        let runtime = self.runtime_state()?;
        let tick_offset = runtime.next_tick;
        let mut observed = run_agent_observation(
            &mut runtime.executor,
            &runtime.catalog,
            AgentObservationRunContext {
                host_config: NativeRunHost {
                    source_path: Some(&runtime.source_path),
                    policy: &runtime.host_policy,
                    adapter_registrars,
                },
                options: &options,
                source_path: &runtime.source_path,
                native_session: Some(&mut runtime.native_session),
                tick_offset,
                input_events,
                task_events: &mut runtime.task_events,
            },
        )
        .map_err(|_| NativeAgentScriptSessionError::Observe)?;
        extend_agent_observation_with_runtime_images(
            &mut observed,
            &runtime.source_path,
            &runtime.executor,
            &options,
        );
        runtime.next_tick = observed.report.tick.saturating_add(1);
        self.observed = Some(NativeAgentObservedSnapshot {
            report: observed.report,
            image_frames: observed.image_frames,
        });
        self.observe_report()
    }

    fn resource_for_uri(
        &mut self,
        uri: &str,
    ) -> Result<AgentResource, NativeAgentScriptSessionError> {
        if self.observed.is_none() {
            self.refresh_observation(Vec::new())?;
        }
        let Some(mut runtime) = self.runtime.take() else {
            return Err(NativeAgentScriptSessionError::ResourceRead);
        };
        let result = {
            let Some(observed) = self.observed.as_ref() else {
                self.runtime = Some(runtime);
                return Err(NativeAgentScriptSessionError::ResourceRead);
            };
            agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
                &observed.report,
                uri,
                None,
                agent_report_capture_time_seconds(&observed.report),
                Some(&mut runtime.native_session),
                &observed.image_frames,
            )
            .map_err(|_| NativeAgentScriptSessionError::ResourceRead)
        };
        self.runtime = Some(runtime);
        result
    }

    fn action_input_events(
        &mut self,
        action: AgentAction,
    ) -> Result<Vec<RuntimeInputEvent>, NativeAgentScriptSessionError> {
        let report = self.observe_report()?;
        native_agent_action_input_events(report, action)
    }

    fn action_result(
        before: &AgentObservationReport,
        after: &AgentObservationReport,
    ) -> ActionResult {
        ActionResult {
            accepted: before.state_hash != after.state_hash || before.tick != after.tick,
            before_tick: u64::try_from(before.tick).unwrap_or(u64::MAX),
            after_tick: u64::try_from(after.tick).unwrap_or(u64::MAX),
            before_state_hash: before.state_hash.clone(),
            after_state_hash: after.state_hash.clone(),
        }
    }
}

fn native_agent_report_has_action(
    report: &AgentObservationReport,
    kind: AgentActionKind,
    target: &str,
) -> bool {
    report.actions.iter().any(|candidate| {
        candidate.enabled
            && candidate.kind == AgentActionDispatch::Semantic
            && candidate.action == kind
            && candidate.target == target
    })
}

fn native_agent_action_input_events(
    report: &AgentObservationReport,
    action: AgentAction,
) -> Result<Vec<RuntimeInputEvent>, NativeAgentScriptSessionError> {
    match action {
        AgentAction::SelectChoice { choice } => {
            native_agent_select_choice_input_events(report, &choice)
        }
        AgentAction::AdvanceText => native_agent_advance_text_input_events(report),
        AgentAction::Invoke(invoke) => {
            native_agent_invoke_input_events(report, &invoke.target, &invoke.action)
        }
        AgentAction::PointerClick { .. } => Err(NativeAgentScriptSessionError::UnsupportedAction),
    }
}

fn native_agent_select_choice_input_events(
    report: &AgentObservationReport,
    choice: &PublicId,
) -> Result<Vec<RuntimeInputEvent>, NativeAgentScriptSessionError> {
    let choice_id = choice.as_str();
    native_agent_report_has_action(report, AgentActionKind::SelectChoice, choice_id)
        .then(|| {
            vec![RuntimeInputEvent {
                kind: "choice".to_owned(),
                payload: Some(choice_id.to_owned()),
            }]
        })
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

fn native_agent_advance_text_input_events(
    report: &AgentObservationReport,
) -> Result<Vec<RuntimeInputEvent>, NativeAgentScriptSessionError> {
    report
        .actions
        .iter()
        .any(|candidate| {
            candidate.enabled
                && candidate.kind == AgentActionDispatch::Semantic
                && candidate.action == AgentActionKind::AdvanceText
        })
        .then(|| {
            vec![RuntimeInputEvent {
                kind: "advance".to_owned(),
                payload: None,
            }]
        })
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

fn native_agent_invoke_input_events(
    report: &AgentObservationReport,
    target: &PublicId,
    action: &str,
) -> Result<Vec<RuntimeInputEvent>, NativeAgentScriptSessionError> {
    let target = target.as_str();
    report
        .actions
        .iter()
        .any(|candidate| {
            candidate.enabled
                && candidate.kind == AgentActionDispatch::Semantic
                && candidate.action == AgentActionKind::Invoke
                && candidate.target == target
                && candidate.id == action
        })
        .then(|| {
            vec![RuntimeInputEvent {
                kind: "invoke".to_owned(),
                payload: Some(action.to_owned()),
            }]
        })
        .ok_or(NativeAgentScriptSessionError::ActionUnavailable)
}

impl AgentSession for NativeAgentScriptSession<'_> {
    type Error = NativeAgentScriptSessionError;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(AgentSessionInfo {
            session_id: "session.native".to_owned(),
            program_hash: self.program_hash.clone(),
            project_entities: self.project_entities.clone(),
            project_graph: self.project_graph.clone(),
            profile: self.options.profile.profile.clone(),
            capabilities: vec![
                "agent.observe".to_owned(),
                "agent.wait".to_owned(),
                "agent.capture".to_owned(),
                "agent.act.semantic".to_owned(),
                "agent.resource.read".to_owned(),
                "debug.read".to_owned(),
                "debug.record".to_owned(),
            ],
        })
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        if self.observed.is_some()
            && self.runtime.as_ref().is_some_and(|runtime| {
                matches!(
                    runtime.executor.fiber().status,
                    FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
                )
            })
        {
            return self.observe_report().map(native_agent_observation_envelope);
        }
        let report = self.refresh_observation(Vec::new())?;
        Ok(native_agent_observation_envelope(report))
    }

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error> {
        let before = self.observe_report()?.clone();
        let input_events = self.action_input_events(action)?;
        let after = self.refresh_observation(input_events)?.clone();
        Ok(Self::action_result(&before, &after))
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        let report = self.observe_report()?;
        let uri = native_agent_capture_uri(report, &request)?;
        let resource = self.resource_for_uri(&uri)?;
        let blob = agent_capture_blob_from_resource(&resource)
            .map_err(|_| NativeAgentScriptSessionError::Capture)?;
        let byte_len = u64::try_from(blob.bytes.len()).unwrap_or(u64::MAX);
        let result = CaptureResult {
            uri: AgentResourceUri::new(resource.uri)
                .map_err(|_| NativeAgentScriptSessionError::Capture)?,
            content_hash: blob.content_hash.clone(),
            media_type: resource.mime_type,
            byte_len,
        };
        self.capture_blobs.push(blob);
        Ok(result)
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        self.resource_for_uri(uri)
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        let additional = usize::try_from(count.max(1)).unwrap_or(usize::MAX);
        self.options.steps = self.options.steps.saturating_add(additional);
        let report = self.refresh_observation(Vec::new())?;
        Ok(native_agent_observation_envelope(report))
    }
}

fn native_agent_observation_envelope(report: &AgentObservationReport) -> ObservationEnvelope {
    ObservationEnvelope {
        tick: u64::try_from(report.tick).unwrap_or(u64::MAX),
        frame_id: report.frame_id.clone(),
        state_hash: report.state_hash.clone(),
        render_hash: report.render_hash.clone(),
        actions: report.actions.clone(),
        signals: report
            .signals
            .iter()
            .map(|signal| {
                (
                    signal.name.trim_start_matches('@').to_owned(),
                    agent_assignment_value(signal),
                )
            })
            .collect(),
        payload: serde_json::to_value(report).unwrap_or(serde_json::Value::Null),
    }
}

fn native_agent_capture_uri(
    report: &AgentObservationReport,
    request: &CaptureRequest,
) -> Result<String, NativeAgentScriptSessionError> {
    let image_kind = native_agent_capture_image_kind(request.format);
    let capture_kind = native_agent_capture_kind(&request.capture_kind);
    let extension = match image_kind {
        AgentObserveImageKind::Png => "png",
        AgentObserveImageKind::RawRgba => "rgba",
        AgentObserveImageKind::Overlay => return Err(NativeAgentScriptSessionError::Capture),
    };
    let name = match &request.target {
        CaptureTarget::Viewport => capture_kind.resource_name().to_owned(),
        CaptureTarget::Layer { id } => {
            agent_scoped_capture_name("layer", id.as_str(), capture_kind.resource_name())
        }
        CaptureTarget::Object { id } => {
            agent_scoped_capture_name("object", id, capture_kind.resource_name())
        }
    };
    Ok(agent_frame_capture_uri_for_page(
        &report.session_id,
        report.tick,
        &name,
        extension,
        0,
    ))
}

fn native_agent_capture_image_kind(format: CaptureFormat) -> AgentObserveImageKind {
    match format {
        CaptureFormat::Png => AgentObserveImageKind::Png,
        CaptureFormat::RawRgba => AgentObserveImageKind::RawRgba,
    }
}

fn native_agent_capture_kind(value: &str) -> AgentObserveCaptureKind {
    match value {
        "object-id" | "object_id" => AgentObserveCaptureKind::ObjectId,
        "mask" => AgentObserveCaptureKind::Mask,
        _ => AgentObserveCaptureKind::Color,
    }
}

fn agent_assignment_value(signal: &AgentAssignment) -> arcweft_agent_protocol::value::AgentValue {
    match signal.value.as_str() {
        "true" => arcweft_agent_protocol::value::AgentValue::Bool(true),
        "false" => arcweft_agent_protocol::value::AgentValue::Bool(false),
        value if value.starts_with('@') => {
            arcweft_agent_protocol::ids::PublicId::new(value.trim_start_matches('@')).map_or_else(
                |_| arcweft_agent_protocol::value::AgentValue::String(value.to_owned()),
                arcweft_agent_protocol::value::AgentValue::Entity,
            )
        }
        value => value.parse::<i64>().map_or_else(
            |_| arcweft_agent_protocol::value::AgentValue::String(value.to_owned()),
            arcweft_agent_protocol::value::AgentValue::I64,
        ),
    }
}

fn agent_capture_blob_from_resource(
    resource: &AgentResource,
) -> Result<AgentCaptureBlob, NativeAgentScriptSessionError> {
    let bytes = match &resource.body {
        AgentResourceBody::BytesBase64(body) => match body.encoding {
            arcweft_agent_protocol::AgentBinaryEncoding::Base64 => STANDARD
                .decode(&body.data)
                .map_err(|_| NativeAgentScriptSessionError::Capture)?,
        },
        AgentResourceBody::Text(text) if resource.mime_type == "image/svg+xml" => {
            text.as_bytes().to_vec()
        }
        AgentResourceBody::Json(_) | AgentResourceBody::Text(_) => {
            return Err(NativeAgentScriptSessionError::Capture);
        }
    };
    Ok(AgentCaptureBlob {
        content_hash: format!("blake3:{}", blake3::hash(&bytes).to_hex()),
        bytes,
    })
}

fn agent_hit_test_command(
    options: &AgentHitTestOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    validate_agent_hit_test_options(options)?;
    let observe_options = agent_hit_test_observe_options(options);
    let report = agent_observation_report_for_options(&observe_options, adapter_registrars)?;
    let hit_test = agent_hit_test_report(&report, options.x, options.y);
    if options.json {
        print_json(&hit_test)
    } else if let Some(top) = &hit_test.top_object_id {
        println!(
            "ok: hit {} at {},{} ({} candidate(s))",
            top,
            options.x,
            options.y,
            hit_test.hits.len()
        );
        Ok(())
    } else {
        println!("ok: no hit at {},{}", options.x, options.y);
        Ok(())
    }
}

fn validate_agent_hit_test_options(options: &AgentHitTestOptions) -> Result<(), ExitCode> {
    let observe_options = agent_hit_test_observe_options(options);
    validate_agent_observe_options(&observe_options)
}

fn agent_hit_test_observe_options(options: &AgentHitTestOptions) -> AgentObserveOptions {
    AgentObserveOptions {
        path: options.path.clone(),
        profile: options.profile.clone(),
        entry: options.entry.clone(),
        flow: options.flow.clone(),
        executor: options.executor,
        pure_backend: options.pure_backend,
        pure_workers: options.pure_workers,
        pure_batch_min_len: options.pure_batch_min_len,
        pure_object_artifacts: options.pure_object_artifacts,
        math_backend: options.math_backend,
        math_wgpu_min_elements: options.math_wgpu_min_elements,
        steps: options.steps,
        capture_step: options.capture_step,
        mode: options.mode,
        max_ops: options.max_ops,
        values: options.values.clone(),
        viewport_width: options.viewport_width,
        viewport_height: options.viewport_height,
        textbox_height: options.textbox_height,
        image: None,
        capture: None,
        layer: None,
        object: None,
        page: None,
        capture_time_seconds: options.capture_time_seconds,
        resource: None,
        read_uri: None,
        mcp: false,
        mcp_format: AgentObserveMcpFormat::Read,
        out: None,
        json: true,
    }
}

fn agent_hit_test_report(report: &AgentObservationReport, x: u32, y: u32) -> AgentHitTestReport {
    let mut hits = report
        .objects
        .iter()
        .filter(|object| object.visible)
        .flat_map(|object| agent_hit_test_object_hits(object, x, y))
        .collect::<Vec<_>>();
    hits.sort_by(agent_hit_test_hit_order);
    for (rank, hit) in hits.iter_mut().enumerate() {
        hit.rank = rank;
    }
    AgentHitTestReport {
        status: "ok".to_owned(),
        session_id: report.session_id.clone(),
        frame_id: report.frame_id.clone(),
        source: report.source.clone(),
        viewport: report.viewport,
        x,
        y,
        top_object_id: hits.first().map(|hit| hit.object_id.clone()),
        hits,
    }
}

fn agent_hit_test_object_hits(
    object: &AgentObservedObject,
    x: u32,
    y: u32,
) -> Vec<AgentHitTestHit> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return agent_image_or_generic_object_hits(object, x, y);
    };
    if !rich_text_ref.hit_test {
        return Vec::new();
    }
    rich_text_ref
        .hit_regions
        .iter()
        .filter(|region| agent_bbox_contains(&region.bbox, x, y))
        .map(|region| AgentHitTestHit {
            rank: 0,
            object_id: object.id.clone(),
            object: AgentImageObjectRef::from_observed(object),
            layer: agent_hit_test_layer(object, rich_text_ref, region),
            role: object.role.clone(),
            text: object.text.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            region: region.clone(),
            rich_text_ref: Some(rich_text_ref.clone()),
            depth: region
                .depth
                .or(object.resolved_object_depth())
                .or(rich_text_ref.object_depth),
        })
        .collect()
}

fn agent_image_or_generic_object_hits(
    object: &AgentObservedObject,
    x: u32,
    y: u32,
) -> Vec<AgentHitTestHit> {
    if !agent_object_contains_point(object, x, y) {
        return Vec::new();
    }
    agent_image_or_generic_hit_regions(object)
        .into_iter()
        .map(|region| AgentHitTestHit {
            rank: 0,
            object_id: object.id.clone(),
            object: AgentImageObjectRef::from_observed(object),
            layer: region
                .proxy_layer
                .clone()
                .or_else(|| object.resolved_object_layer())
                .unwrap_or_else(|| object.layer.clone()),
            role: object.role.clone(),
            text: object.text.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            depth: region.depth.or(object.resolved_object_depth()),
            region,
            rich_text_ref: None,
        })
        .collect()
}

fn agent_image_or_generic_hit_regions(object: &AgentObservedObject) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_generic_object_hit_region(object)];
    if let AgentObservedObjectContent::Image(content) = &object.content {
        regions.extend(
            content
                .proxies
                .iter()
                .filter(|proxy| proxy.hit_test)
                .map(|proxy| agent_image_proxy_hit_region(object, proxy)),
        );
    }
    regions
}

fn agent_generic_object_hit_region(object: &AgentObservedObject) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::Object,
        bbox: object.bbox.clone(),
        range: RichTextRange::new(0, 0),
        proxy_id: None,
        proxy_type: None,
        proxy_declaration: None,
        proxy_role: None,
        proxy_layer: object.resolved_object_layer(),
        depth: object.resolved_object_depth(),
        proxy_params: BTreeMap::new(),
    }
}

fn agent_image_proxy_hit_region(
    object: &AgentObservedObject,
    proxy: &AgentPresentationObjectProxyRef,
) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::ObjectProxy,
        bbox: object.bbox.clone(),
        range: RichTextRange::new(0, 0),
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy
            .layer
            .clone()
            .or_else(|| object.resolved_object_layer()),
        depth: proxy.depth.or_else(|| object.resolved_object_depth()),
        proxy_params: proxy.params.clone(),
    }
}

fn agent_object_contains_point(object: &AgentObservedObject, x: u32, y: u32) -> bool {
    if !agent_bbox_contains(&object.bbox, x, y) {
        return false;
    }
    if object.polygon.len() >= 3 {
        return agent_polygon_contains(&object.polygon, x, y);
    }
    true
}

fn agent_hit_test_layer(
    object: &AgentObservedObject,
    rich_text_ref: &AgentRichTextElementRef,
    region: &AgentHitRegion,
) -> String {
    region
        .proxy_layer
        .clone()
        .or_else(|| object.resolved_object_layer())
        .or_else(|| rich_text_ref.object_layer.clone())
        .unwrap_or_else(|| object.layer.clone())
}

fn agent_hit_test_hit_order(left: &AgentHitTestHit, right: &AgentHitTestHit) -> std::cmp::Ordering {
    right
        .depth
        .unwrap_or(0)
        .cmp(&left.depth.unwrap_or(0))
        .then_with(|| {
            agent_hit_test_region_priority(left.region.kind)
                .cmp(&agent_hit_test_region_priority(right.region.kind))
        })
        .then_with(|| {
            agent_hit_test_role_priority(&left.role).cmp(&agent_hit_test_role_priority(&right.role))
        })
        .then_with(|| agent_bbox_area(&left.region.bbox).cmp(&agent_bbox_area(&right.region.bbox)))
        .then_with(|| left.object_id.cmp(&right.object_id))
}

const fn agent_hit_test_region_priority(kind: AgentHitRegionKind) -> u8 {
    match kind {
        AgentHitRegionKind::TextObjectProxy => 0,
        AgentHitRegionKind::ObjectProxy => 5,
        AgentHitRegionKind::TextGlyph => 10,
        AgentHitRegionKind::GlyphCluster => 20,
        AgentHitRegionKind::RubyAnnotation => 30,
        AgentHitRegionKind::RubyBase => 40,
        AgentHitRegionKind::RubyObject => 50,
        AgentHitRegionKind::TextRun => 60,
        AgentHitRegionKind::TextLine => 70,
        AgentHitRegionKind::TextPage => 80,
        AgentHitRegionKind::Object => 90,
    }
}

fn agent_hit_test_role_priority(role: &str) -> u8 {
    match role {
        "rich_text_proxy" => 0,
        "rich_text_glyph" => 10,
        "rich_text_cluster" => 20,
        "rich_text_ruby" => 30,
        "rich_text_run" => 40,
        "rich_text_line" => 50,
        "rich_text_page" => 60,
        _ => 100,
    }
}

fn agent_bbox_contains(bbox: &AgentBBox, x: u32, y: u32) -> bool {
    x >= bbox.x
        && y >= bbox.y
        && x < bbox.x.saturating_add(bbox.width)
        && y < bbox.y.saturating_add(bbox.height)
}

fn agent_bbox_area(bbox: &AgentBBox) -> u64 {
    u64::from(bbox.width) * u64::from(bbox.height)
}

fn agent_polygon_contains(polygon: &[AgentPoint], x: u32, y: u32) -> bool {
    let x = f64::from(x);
    let y = f64::from(y);
    let mut inside = false;
    for index in 0..polygon.len() {
        let current = &polygon[index];
        let previous = &polygon[(index + polygon.len() - 1) % polygon.len()];
        let yi = f64::from(current.y);
        let yj = f64::from(previous.y);
        if (yi > y) == (yj > y) {
            continue;
        }
        let xi = f64::from(current.x);
        let xj = f64::from(previous.x);
        let intersection_x = (xj - xi) * (y - yi) / (yj - yi) + xi;
        if x < intersection_x {
            inside = !inside;
        }
    }
    inside
}

fn validate_agent_observe_options(options: &AgentObserveOptions) -> Result<(), ExitCode> {
    if options.capture_step == Some(0) {
        eprintln!("error: --capture-step must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if agent_observe_effective_steps(options) == 0 {
        eprintln!("error: --steps must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.viewport_width == 0 || options.viewport_height == 0 {
        eprintln!("error: --viewport-width and --viewport-height must be greater than zero");
        return Err(ExitCode::from(2));
    }
    if options.textbox_height == Some(0) {
        eprintln!("error: --textbox-height must be greater than zero");
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
    if options
        .capture_time_seconds
        .is_some_and(|value| !value.is_finite() || value < 0.0)
    {
        eprintln!("error: --capture-time must be a finite non-negative number of seconds");
        return Err(ExitCode::from(2));
    }
    Ok(())
}

fn agent_observe_effective_steps(options: &AgentObserveOptions) -> usize {
    options.capture_step.unwrap_or(options.steps)
}

fn agent_observe_capture_time_seconds(options: &AgentObserveOptions) -> f32 {
    options
        .capture_time_seconds
        .unwrap_or_else(|| match options.capture_step {
            Some(step) => agent_capture_time_seconds_from_step(step),
            None => 60.0,
        })
}

fn agent_observe_report_capture_time_millis(options: &AgentObserveOptions) -> Option<u32> {
    (options.capture_time_seconds.is_some() || options.capture_step.is_some())
        .then(|| agent_capture_time_millis(agent_observe_capture_time_seconds(options)))
}

fn agent_report_capture_time_seconds(report: &AgentObservationReport) -> f32 {
    report.capture_time_millis.map_or(60.0, |millis| {
        (f64::from(millis) / 1000.0)
            .to_string()
            .parse()
            .unwrap_or(f32::MAX)
    })
}

fn agent_capture_time_seconds_from_step(step: usize) -> f32 {
    f32::from(u16::try_from(step).unwrap_or(u16::MAX))
}

fn agent_capture_time_millis(time_seconds: f32) -> u32 {
    if !time_seconds.is_finite() || time_seconds <= 0.0 {
        return 0;
    }
    let millis = f64::from(time_seconds) * 1000.0;
    if millis >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        millis.round().to_string().parse().unwrap_or(u32::MAX)
    }
}

fn agent_observe_resource_by_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time(
        report,
        uri,
        None,
        agent_report_capture_time_seconds(report),
    )
}

fn agent_observe_resource_by_uri_with_page_and_time(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time_and_session(
        report,
        uri,
        page_override,
        capture_time_seconds,
        None,
    )
}

fn agent_observe_resource_by_uri_with_page_and_time_and_session(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentResource, ExitCode> {
    agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
        report,
        uri,
        page_override,
        capture_time_seconds,
        native_session,
        &AgentImageFrameStore::default(),
    )
}

fn agent_observe_resource_by_uri_with_page_and_time_and_session_and_frame_store(
    report: &AgentObservationReport,
    uri: &str,
    page_override: Option<usize>,
    capture_time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
    image_frames: &AgentImageFrameStore,
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
    if let Some(resource) = agent_presentation_tree_resource_from_uri(report, uri) {
        return resource;
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
    match native_session {
        Some(native_session) => agent_native_capture_resource_with_session_and_frame_store(
            report,
            &request,
            native_session,
            image_frames,
        ),
        None => agent_observe_capture_resource(report, &request),
    }
}

fn agent_presentation_tree_resource_from_uri(
    report: &AgentObservationReport,
    uri: &str,
) -> Option<Result<AgentResource, ExitCode>> {
    let (base_uri, query_string) = uri.split_once('?').unwrap_or((uri, ""));
    if base_uri
        != format!(
            "arcweft://session/{}/frame/{}/presentation-tree.json",
            report.session_id, report.tick
        )
    {
        return None;
    }

    if query_string.is_empty() {
        return Some(
            report
                .presentation_tree_resource()
                .map_err(|error| agent_json_error(&error)),
        );
    }

    Some(
        agent_presentation_tree_query_from_uri(query_string).and_then(|query| {
            report
                .filtered_presentation_tree_resource(uri.to_owned(), &query)
                .map_err(|error| agent_json_error(&error))
        }),
    )
}

fn agent_presentation_tree_query_from_uri(
    query_string: &str,
) -> Result<AgentPresentationTreeQuery, ExitCode> {
    let mut query = AgentPresentationTreeQuery::default();
    for part in query_string.split('&') {
        let Some((key, value)) = part.split_once('=') else {
            eprintln!("error: invalid presentation-tree query segment: {part}");
            return Err(ExitCode::from(2));
        };
        if value.is_empty() {
            eprintln!("error: presentation-tree query value for `{key}` must not be empty");
            return Err(ExitCode::from(2));
        }
        match key {
            "role" => query.role = Some(value.to_owned()),
            "rich_text_kind" => {
                query.rich_text_kind = Some(agent_rich_text_kind_from_query_value(value)?);
            }
            "object_layer" => query.object_layer = Some(value.to_owned()),
            "effect" | "effect_id" => query.effect_id = Some(value.to_owned()),
            "shader" | "shader_id" => query.shader_id = Some(value.to_owned()),
            "motion" | "motion_function" | "motion_function_id" => {
                query.motion_function_id = Some(value.to_owned());
            }
            "proxy" | "object_proxy" | "object_proxy_id" => {
                query.object_proxy_id = Some(value.to_owned());
            }
            "proxy_type" | "object_proxy_type" | "type" => {
                query.object_proxy_type = Some(value.to_owned());
            }
            "proxy_role" | "object_proxy_role" => {
                query.object_proxy_role = Some(value.to_owned());
            }
            "proxy_struct" | "object_proxy_struct" | "struct" => {
                query.object_proxy_struct = Some(value.to_owned());
            }
            "proxy_param" | "object_proxy_param" => {
                query.object_proxy_param = Some(agent_proxy_param_query_value(value));
            }
            "has_transform" => query.has_transform = Some(agent_bool_query_value(value)?),
            _ if key.starts_with("proxy_param.") => {
                query.object_proxy_param = Some(agent_proxy_param_query_key_value(
                    key.trim_start_matches("proxy_param."),
                    value,
                )?);
            }
            _ if key.starts_with("object_proxy_param.") => {
                query.object_proxy_param = Some(agent_proxy_param_query_key_value(
                    key.trim_start_matches("object_proxy_param."),
                    value,
                )?);
            }
            _ => {
                eprintln!("error: unsupported presentation-tree query key: {key}");
                return Err(ExitCode::from(2));
            }
        }
    }
    Ok(query)
}

fn agent_proxy_param_query_value(value: &str) -> AgentPresentationObjectProxyParamQuery {
    value.split_once('=').map_or_else(
        || AgentPresentationObjectProxyParamQuery {
            key: value.to_owned(),
            value: None,
        },
        |(key, value)| AgentPresentationObjectProxyParamQuery {
            key: key.to_owned(),
            value: Some(value.to_owned()),
        },
    )
}

fn agent_proxy_param_query_key_value(
    key: &str,
    value: &str,
) -> Result<AgentPresentationObjectProxyParamQuery, ExitCode> {
    if key.is_empty() {
        eprintln!("error: proxy parameter query key must not be empty");
        return Err(ExitCode::from(2));
    }
    Ok(AgentPresentationObjectProxyParamQuery {
        key: key.to_owned(),
        value: Some(value.to_owned()),
    })
}

fn agent_rich_text_kind_from_query_value(
    value: &str,
) -> Result<AgentRichTextElementKind, ExitCode> {
    match value {
        "text_page" => Ok(AgentRichTextElementKind::TextPage),
        "text_line" => Ok(AgentRichTextElementKind::TextLine),
        "text_run" => Ok(AgentRichTextElementKind::TextRun),
        "text_glyph" => Ok(AgentRichTextElementKind::TextGlyph),
        "ruby" => Ok(AgentRichTextElementKind::Ruby),
        "glyph_cluster" => Ok(AgentRichTextElementKind::GlyphCluster),
        "text_object_proxy" => Ok(AgentRichTextElementKind::TextObjectProxy),
        _ => {
            eprintln!("error: unsupported rich_text_kind query value: {value}");
            Err(ExitCode::from(2))
        }
    }
}

fn agent_bool_query_value(value: &str) -> Result<bool, ExitCode> {
    match value {
        "true" | "1" => Ok(true),
        "false" | "0" => Ok(false),
        _ => {
            eprintln!("error: expected boolean query value, got: {value}");
            Err(ExitCode::from(2))
        }
    }
}

#[derive(Clone, Debug)]
struct AgentCaptureReadRequest {
    uri: String,
    image_kind: AgentObserveImageKind,
    capture_kind: AgentObserveCaptureKind,
    scope: AgentCaptureScope,
    page: usize,
    capture_step: usize,
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
        capture_step: report.steps,
        capture_time_seconds: agent_report_capture_time_seconds(report),
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
    let result = agent_native_capture_image(report, request)?;
    Ok(report.image_resource(&result.image, &result.bytes))
}

fn agent_native_capture_resource_with_session_and_frame_store(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<AgentResource, ExitCode> {
    let result =
        agent_native_capture_image_with_frame_store(report, request, native_session, image_frames)?;
    Ok(report.image_resource(&result.image, &result.bytes))
}

struct AgentNativeCaptureImageResult {
    image: AgentImageResource,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AgentImageFrameStore {
    frames_by_object: BTreeMap<String, AgentStoredImageFrame>,
}

#[derive(Clone, Debug, Default, PartialEq)]
struct AgentUiImageObservation {
    objects: Vec<AgentObservedObject>,
    image_frames: AgentImageFrameStore,
}

#[derive(Clone, Debug, PartialEq)]
struct AgentStoredImageFrame {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
    placement: Option<AgentStoredImagePlacement>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct AgentStoredImagePlacement {
    dst: arcweft_render_native::NativeImageRect,
    transform: arcweft_render_native::NativeImageTransform,
    opacity_milli: u16,
}

impl AgentImageFrameStore {
    #[cfg(test)]
    fn insert(&mut self, object_id: impl Into<String>, width: u32, height: u32, rgba: Vec<u8>) {
        self.insert_with_placement(object_id, width, height, rgba, None);
    }

    #[cfg(test)]
    fn insert_with_placement(
        &mut self,
        object_id: impl Into<String>,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
        placement: Option<AgentStoredImagePlacement>,
    ) {
        self.frames_by_object.insert(
            object_id.into(),
            AgentStoredImageFrame {
                width,
                height,
                rgba,
                placement,
            },
        );
    }

    fn get(&self, object_id: &str) -> Option<&AgentStoredImageFrame> {
        self.frames_by_object.get(object_id)
    }

    fn extend(&mut self, other: AgentImageFrameStore) {
        self.frames_by_object.extend(other.frames_by_object);
    }
}

fn agent_native_capture_image(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    let mut native_session =
        arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    agent_native_capture_image_with_session(report, request, &mut native_session)
}

fn agent_native_capture_image_with_session(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    agent_native_capture_image_with_frame_store(
        report,
        request,
        native_session,
        &AgentImageFrameStore::default(),
    )
}

fn agent_native_capture_image_with_frame_store(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    if let Some(result) =
        agent_native_image_layer_frame_capture(report, request, native_session, image_frames)?
    {
        return Ok(result);
    }
    if let Some(result) =
        agent_native_image_object_frame_capture(report, request, native_session, image_frames)?
    {
        return Ok(result);
    }
    if let Some(result) = agent_native_image_object_geometry_capture(report, request)? {
        return Ok(result);
    }
    let Some(textbox) = agent_native_textbox_for_capture(report, &request.scope) else {
        eprintln!("error: native renderer requires an observed textbox frame");
        return Err(ExitCode::from(2));
    };
    let (left, top) = agent_native_text_origin(textbox);
    let capture = native_session
        .capture_frame_rgba_in(
            agent_observed_rich_text(textbox),
            arcweft_render_native::NativeCaptureViewport::new(
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
            frame: agent_observed_rich_text(textbox),
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
    agent_native_capture_result_from_raster(report, request, &capture)
}

fn agent_native_capture_result_from_raster(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    capture: &AgentRasterCapture,
) -> Result<AgentNativeCaptureImageResult, ExitCode> {
    let (mime_type, bytes) = match request.image_kind {
        AgentObserveImageKind::Png => ("image/png", agent_encode_png(capture)?),
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
        capture_step: request.capture_step,
        capture_time_millis: agent_capture_time_millis(request.capture_time_seconds),
        uri: request.uri.clone(),
        mime_type: mime_type.to_owned(),
        width: capture.width,
        height: capture.height,
        hash: hash_hex(&bytes),
        crop_origin: capture.crop_origin,
        content_bbox: stats.bbox,
        content_viewport_bbox,
        content_pixels: Some(stats.content_pixels),
        object: agent_image_object_for_capture_scope(report, &request.scope),
        diagnostics: agent_native_visual_diagnostics(request.capture_step, &capture.diagnostics),
        written: None,
    };
    Ok(AgentNativeCaptureImageResult { image, bytes })
}

#[allow(clippy::cast_precision_loss)]
fn agent_native_image_layer_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Layer(layer) = &request.scope else {
        return Ok(None);
    };
    let image_items = report
        .objects
        .iter()
        .filter(|object| {
            object.visible
                && agent_object_matches_layer(object, layer)
                && matches!(object.content, AgentObservedObjectContent::Image(_))
        })
        .filter_map(|object| image_frames.get(&object.id).map(|frame| (object, frame)))
        .collect::<Vec<_>>();
    if image_items.is_empty() {
        return Ok(None);
    }

    let viewport_width = report.viewport.width.max(1);
    let viewport_height = report.viewport.height.max(1);
    let crop = image_items
        .iter()
        .map(|(object, _)| {
            agent_clamped_bbox_rect(
                viewport_width,
                viewport_height,
                object.bbox.x,
                object.bbox.y,
                object.bbox.width,
                object.bbox.height,
            )
        })
        .reduce(|left, right| agent_union_rect(left, right, viewport_width, viewport_height))
        .expect("non-empty image layer capture has crop rect");
    let capture = match request.capture_kind {
        AgentObserveCaptureKind::Color => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| agent_native_image_quad(object, frame))
                .collect::<Vec<_>>();
            native_session.capture_image_quads_rgba(&quads, viewport_width, viewport_height)
        }
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            let quads = image_items
                .iter()
                .map(|(object, frame)| {
                    let color = match request.capture_kind {
                        AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
                        AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
                        AgentObserveCaptureKind::Color => unreachable!("handled above"),
                    };
                    arcweft_render_native::NativeImageDebugQuad {
                        quad: agent_native_image_quad(object, frame),
                        color,
                    }
                })
                .collect::<Vec<_>>();
            native_session.capture_image_debug_quads_rgba(&quads, viewport_width, viewport_height)
        }
    }
    .map_err(|error| {
        eprintln!("error: native image layer capture failed: {error}");
        ExitCode::FAILURE
    })?;
    let raster = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: match request.capture_kind {
            AgentObserveCaptureKind::Color => AgentImageComposition::Framebuffer,
            AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
            AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        },
        background: [0, 0, 0, 0],
        rgba: capture.rgba,
        diagnostics: capture.diagnostics,
    };
    let capture = agent_crop_raster_capture(&raster, crop.0, crop.1, crop.2, crop.3);
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

fn agent_native_image_object_frame_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
    image_frames: &AgentImageFrameStore,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Object(object_id) = &request.scope else {
        return Ok(None);
    };
    let Some(object) = report.objects.iter().find(|object| object.id == *object_id) else {
        return Ok(None);
    };
    if !matches!(object.content, AgentObservedObjectContent::Image(_)) {
        return Ok(None);
    }
    let Some(frame) = image_frames.get(&object.id) else {
        return Ok(None);
    };
    let capture = agent_native_image_frame_capture(
        report.viewport.width,
        report.viewport.height,
        object,
        frame,
        request.capture_kind,
        native_session,
    )?;
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

#[allow(clippy::cast_precision_loss)]
fn agent_native_image_frame_capture(
    viewport_width: u32,
    viewport_height: u32,
    object: &AgentObservedObject,
    frame: &AgentStoredImageFrame,
    capture_kind: AgentObserveCaptureKind,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
) -> Result<AgentRasterCapture, ExitCode> {
    let (x, y, width, height) = agent_clamped_bbox_rect(
        viewport_width,
        viewport_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    );
    let quad = agent_native_image_quad(object, frame);
    let capture = match capture_kind {
        AgentObserveCaptureKind::Color => native_session.capture_image_quads_rgba(
            &[quad],
            viewport_width.max(1),
            viewport_height.max(1),
        ),
        AgentObserveCaptureKind::ObjectId | AgentObserveCaptureKind::Mask => {
            let color = match capture_kind {
                AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
                AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
                AgentObserveCaptureKind::Color => unreachable!("handled above"),
            };
            native_session.capture_image_debug_quads_rgba(
                &[arcweft_render_native::NativeImageDebugQuad { quad, color }],
                viewport_width.max(1),
                viewport_height.max(1),
            )
        }
    }
    .map_err(|error| {
        eprintln!("error: native image object capture failed: {error}");
        ExitCode::FAILURE
    })?;
    let raster = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: match capture_kind {
            AgentObserveCaptureKind::Color => AgentImageComposition::Framebuffer,
            AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
            AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
        },
        background: [0, 0, 0, 0],
        rgba: capture.rgba,
        diagnostics: capture.diagnostics,
    };
    Ok(agent_crop_raster_capture(&raster, x, y, width, height))
}

#[allow(clippy::cast_precision_loss)]
fn agent_native_image_quad<'a>(
    object: &AgentObservedObject,
    frame: &'a AgentStoredImageFrame,
) -> arcweft_render_native::NativeImageQuad<'a> {
    if let Some(placement) = frame.placement {
        return arcweft_render_native::NativeImageQuad {
            width: frame.width,
            height: frame.height,
            rgba: &frame.rgba,
            opacity_milli: placement.opacity_milli,
            dst: placement.dst,
            transform: placement.transform,
        };
    }
    agent_native_image_quad_for_rect(
        frame,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
        agent_image_object_opacity_milli(object),
    )
}

#[allow(clippy::cast_precision_loss)]
fn agent_native_image_quad_for_rect(
    frame: &AgentStoredImageFrame,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    opacity_milli: u16,
) -> arcweft_render_native::NativeImageQuad<'_> {
    arcweft_render_native::NativeImageQuad {
        width: frame.width,
        height: frame.height,
        rgba: &frame.rgba,
        opacity_milli,
        dst: arcweft_render_native::NativeImageRect {
            x: x as f32,
            y: y as f32,
            width: width as f32,
            height: height as f32,
        },
        transform: arcweft_render_native::NativeImageTransform::identity(),
    }
}

fn agent_image_object_opacity_milli(object: &AgentObservedObject) -> u16 {
    match &object.content {
        AgentObservedObjectContent::Image(content) => content.opacity_milli.unwrap_or(1_000),
        AgentObservedObjectContent::RichText { .. } | AgentObservedObjectContent::Custom { .. } => {
            1_000
        }
    }
}

fn agent_native_image_object_geometry_capture(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
) -> Result<Option<AgentNativeCaptureImageResult>, ExitCode> {
    let AgentCaptureScope::Object(object_id) = &request.scope else {
        return Ok(None);
    };
    let Some(object) = report.objects.iter().find(|object| object.id == *object_id) else {
        return Ok(None);
    };
    if !matches!(object.content, AgentObservedObjectContent::Image(_)) {
        return Ok(None);
    }
    if request.capture_kind == AgentObserveCaptureKind::Color {
        eprintln!(
            "error: native image object color capture requires decoded image pixels in the observation frame"
        );
        return Err(ExitCode::from(2));
    }
    let capture = agent_observed_object_geometry_capture(
        report.viewport.width,
        report.viewport.height,
        object,
        request.capture_kind,
    );
    agent_native_capture_result_from_raster(report, request, &capture).map(Some)
}

fn agent_observed_object_geometry_capture(
    viewport_width: u32,
    viewport_height: u32,
    object: &AgentObservedObject,
    capture_kind: AgentObserveCaptureKind,
) -> AgentRasterCapture {
    let (x, y, width, height) = agent_clamped_bbox_rect(
        viewport_width,
        viewport_height,
        object.bbox.x,
        object.bbox.y,
        object.bbox.width,
        object.bbox.height,
    );
    let color = match capture_kind {
        AgentObserveCaptureKind::Color => [0, 0, 0, 0],
        AgentObserveCaptureKind::ObjectId => agent_object_id_color(&object.id),
        AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
    };
    let composition = match capture_kind {
        AgentObserveCaptureKind::Color => AgentImageComposition::FramebufferCrop,
        AgentObserveCaptureKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentObserveCaptureKind::Mask => AgentImageComposition::MaskAttachment,
    };
    let mut full = AgentRasterCapture::new(
        viewport_width.max(1),
        viewport_height.max(1),
        [0, 0, 0, 0],
        composition,
    );
    agent_fill_raster_rect(&mut full, x, y, width, height, color);
    agent_crop_raster_capture(&full, x, y, width, height)
}

fn agent_image_object_for_capture_scope(
    report: &AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<AgentImageObjectRef> {
    let AgentCaptureScope::Object(object_id) = scope else {
        return None;
    };
    report
        .objects
        .iter()
        .find(|object| object.id == *object_id)
        .map(AgentImageObjectRef::from_observed)
}

fn agent_native_textbox_for_capture<'a>(
    report: &'a AgentObservationReport,
    scope: &AgentCaptureScope,
) -> Option<&'a AgentObservedObject> {
    if let AgentCaptureScope::Object(object_id) = scope {
        if let Some(object) = report.objects.iter().find(|object| object.id == *object_id) {
            if agent_is_dialogue_textbox(object) {
                return Some(object);
            }
            if let Some(parent_id) = agent_rich_text_child_parent_object_id(&object.id) {
                return report.objects.iter().find(|candidate| {
                    agent_is_dialogue_textbox(candidate) && candidate.id == parent_id
                });
            }
        }
        if let Some(parent_id) = agent_rich_text_child_parent_object_id(object_id) {
            return report.objects.iter().find(|candidate| {
                agent_is_dialogue_textbox(candidate) && candidate.id == parent_id
            });
        }
    }
    report
        .objects
        .iter()
        .find(|object| agent_is_dialogue_textbox(object))
}

fn agent_rich_text_child_parent_object_id(object_id: &str) -> Option<&str> {
    object_id
        .split_once(".page.")
        .or_else(|| object_id.split_once(".line."))
        .or_else(|| object_id.split_once(".run."))
        .or_else(|| object_id.split_once(".ruby."))
        .or_else(|| object_id.split_once(".cluster."))
        .or_else(|| object_id.split_once(".proxy."))
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
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    scope: &AgentCaptureScope,
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentRasterCapture, ExitCode> {
    let mut native_session = native_session;
    let full = AgentRasterCapture {
        width: capture.width,
        height: capture.height,
        crop_origin: None,
        composition: AgentImageComposition::Framebuffer,
        background: [0, 0, 0, 255],
        rgba: capture.rgba.clone(),
        diagnostics: capture.diagnostics.clone(),
    };
    let selected = agent_native_capture_targets_for_scope(context, scope)?;
    let selected = agent_native_capture_targets_for_page(
        capture.width,
        capture.height,
        context,
        scope,
        selected,
        native_session.as_deref_mut(),
    )?;
    if capture_kind == AgentObserveCaptureKind::Color {
        let AgentCaptureScope::Viewport = scope else {
            if matches!(scope, AgentCaptureScope::Layer(_))
                && selected
                    .iter()
                    .any(|target| !target.role().starts_with("rich_text_"))
            {
                let (x, y, width, height) = agent_native_scope_rect(
                    capture.width,
                    capture.height,
                    context,
                    &selected,
                    native_session.as_deref_mut(),
                )?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            if let Some(isolated) = agent_native_color_capture(
                capture,
                context,
                &selected,
                native_session.as_deref_mut(),
            )? {
                let mut rgba = isolated.rgba;
                make_nontransparent_pixels_opaque(&mut rgba);
                let full = AgentRasterCapture {
                    width: isolated.width,
                    height: isolated.height,
                    crop_origin: None,
                    composition: AgentImageComposition::IsolatedRegions,
                    background: [0, 0, 0, 0],
                    rgba,
                    diagnostics: isolated.diagnostics,
                };
                let (x, y, width, height) = agent_native_scope_rect(
                    capture.width,
                    capture.height,
                    context,
                    &selected,
                    native_session.as_deref_mut(),
                )?;
                return Ok(agent_crop_raster_capture(&full, x, y, width, height));
            }
            return agent_native_masked_framebuffer_capture(
                capture,
                context,
                &selected,
                native_session.as_deref_mut(),
            );
        };
        return Ok(full);
    }

    let debug = agent_native_debug_capture(
        capture,
        context,
        &selected,
        capture_kind,
        native_session.as_deref_mut(),
    )?;
    let full = AgentRasterCapture {
        width: debug.capture.width,
        height: debug.capture.height,
        crop_origin: None,
        composition: debug.composition,
        background: [0, 0, 0, 0],
        rgba: debug.capture.rgba,
        diagnostics: debug.capture.diagnostics,
    };
    if !matches!(scope, AgentCaptureScope::Viewport) {
        let (x, y, width, height) = agent_native_scope_rect(
            capture.width,
            capture.height,
            context,
            &selected,
            native_session,
        )?;
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

#[derive(Clone)]
enum AgentNativeCaptureTarget<'a> {
    Observed(&'a AgentObservedObject),
    RichTextElement {
        id: String,
        role: &'static str,
        parent: &'a AgentObservedObject,
        element: arcweft_render_native::NativeFrameElement,
    },
}

impl AgentNativeCaptureTarget<'_> {
    fn id(&self) -> &str {
        match self {
            AgentNativeCaptureTarget::Observed(object) => &object.id,
            AgentNativeCaptureTarget::RichTextElement { id, .. } => id,
        }
    }

    fn role(&self) -> &str {
        match self {
            AgentNativeCaptureTarget::Observed(object) => &object.role,
            AgentNativeCaptureTarget::RichTextElement { role, .. } => role,
        }
    }

    fn observed(&self) -> Option<&AgentObservedObject> {
        match self {
            AgentNativeCaptureTarget::Observed(object) => Some(object),
            AgentNativeCaptureTarget::RichTextElement { .. } => None,
        }
    }
}

fn agent_native_capture_targets_for_page<'a>(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'a>,
    scope: &AgentCaptureScope,
    selected: Vec<AgentNativeCaptureTarget<'a>>,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<AgentNativeCaptureTarget<'a>>, ExitCode> {
    if !matches!(scope, AgentCaptureScope::Layer(_)) {
        return Ok(selected);
    }
    let mut native_session = native_session;
    selected
        .into_iter()
        .filter_map(|target| {
            let Some(object) = target.observed() else {
                return Some(Ok(target));
            };
            match agent_native_object_is_visible_on_page(
                capture_width,
                capture_height,
                context,
                object,
                native_session.as_deref_mut(),
            ) {
                Ok(true) => Some(Ok(target)),
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
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<bool, ExitCode> {
    if !object.role.starts_with("rich_text_") {
        return Ok(true);
    }
    agent_native_rich_text_child_rect(
        capture_width,
        capture_height,
        context,
        object,
        native_session,
    )
    .map(|rect| rect.is_some())
}

struct AgentNativeDebugCapture {
    capture: arcweft_render_native::NativeFrameCapture,
    composition: AgentImageComposition,
}

fn agent_native_color_capture(
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<arcweft_render_native::NativeFrameCapture>, ExitCode> {
    let mut native_session = native_session;
    let mut regions = Vec::new();
    for target in selected {
        let object_regions = agent_native_regions_for_target(
            capture.width,
            capture.height,
            context,
            target,
            [0, 0, 0, 0],
            native_session.as_deref_mut(),
        )?;
        if object_regions.iter().any(|region| region.element.is_none()) {
            return Ok(None);
        }
        regions.extend(object_regions);
    }
    let capture_result = if let Some(native_session) = native_session {
        native_session.capture_frame_color_regions_in(
            context.frame,
            arcweft_render_native::NativeCaptureViewport::new(
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
        arcweft_render_native::capture_frame_color_regions_at_page(
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
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    capture_kind: AgentObserveCaptureKind,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentNativeDebugCapture, ExitCode> {
    let mut native_session = native_session;
    let mut regions = Vec::new();
    for target in selected {
        let color = match capture_kind {
            AgentObserveCaptureKind::Color => {
                unreachable!("native geometry capture is debug-only")
            }
            AgentObserveCaptureKind::ObjectId => agent_object_id_color(target.id()),
            AgentObserveCaptureKind::Mask => [255, 255, 255, 255],
        };
        regions.extend(agent_native_regions_for_target(
            capture.width,
            capture.height,
            context,
            target,
            color,
            native_session.as_deref_mut(),
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
            arcweft_render_native::NativeCaptureViewport::new(
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
        arcweft_render_native::capture_frame_debug_regions_at_page(
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
    capture: &arcweft_render_native::NativeFrameCapture,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentRasterCapture, ExitCode> {
    let mut native_session = native_session;
    let mut masked = AgentRasterCapture::new(
        capture.width,
        capture.height,
        [0, 0, 0, 0],
        AgentImageComposition::MaskedFramebufferCrop,
    );
    masked.diagnostics.clone_from(&capture.diagnostics);
    for target in selected {
        let (x, y, width, height) = agent_native_target_rect(
            capture.width,
            capture.height,
            context,
            target,
            native_session.as_deref_mut(),
        )?;
        agent_copy_native_framebuffer_rect(&mut masked, capture, x, y, width, height);
    }
    let (x, y, width, height) = agent_native_scope_rect(
        capture.width,
        capture.height,
        context,
        selected,
        native_session,
    )?;
    Ok(agent_crop_raster_capture(&masked, x, y, width, height))
}

fn agent_copy_native_framebuffer_rect(
    target: &mut AgentRasterCapture,
    source: &arcweft_render_native::NativeFrameCapture,
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

fn agent_fill_raster_rect(
    target: &mut AgentRasterCapture,
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    color: [u8; 4],
) {
    let target_width = usize::try_from(target.width).unwrap_or(0);
    for row in y..y.saturating_add(height).min(target.height) {
        for col in x..x.saturating_add(width).min(target.width) {
            let start = usize::try_from(row)
                .unwrap_or(0)
                .saturating_mul(target_width)
                .saturating_add(usize::try_from(col).unwrap_or(0))
                .saturating_mul(4);
            let Some(pixel) = target.rgba.get_mut(start..start.saturating_add(4)) else {
                continue;
            };
            pixel.copy_from_slice(&color);
        }
    }
}

fn agent_native_regions_for_target(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
    color: [u8; 4],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<arcweft_render_native::NativeFrameDebugRegion>, ExitCode> {
    let (x, y, width, height) = agent_native_target_rect(
        capture_width,
        capture_height,
        context,
        target,
        native_session,
    )?;
    let fallback_bbox = arcweft_render_native::NativeFrameContentBBox {
        x,
        y,
        width,
        height,
    };
    let elements = agent_native_elements_for_target(context, target);
    if elements.is_empty() {
        return Ok(vec![arcweft_render_native::NativeFrameDebugRegion {
            element: None,
            fallback_bbox,
            color,
        }]);
    }
    Ok(elements
        .into_iter()
        .map(|element| arcweft_render_native::NativeFrameDebugRegion {
            element: Some(element),
            fallback_bbox,
            color,
        })
        .collect())
}

fn agent_native_elements_for_target(
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    match target {
        AgentNativeCaptureTarget::Observed(object) => {
            agent_native_elements_for_object(context, object)
        }
        AgentNativeCaptureTarget::RichTextElement { element, .. } => vec![*element],
    }
}

fn agent_native_elements_for_object(
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    if agent_is_dialogue_textbox(object) {
        let frame = agent_observed_rich_text(object);
        return frame
            .display_map
            .text_runs
            .iter()
            .enumerate()
            .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
            .chain(
                frame
                    .display_map
                    .ruby_annotations
                    .iter()
                    .enumerate()
                    .map(|(index, _)| arcweft_render_native::NativeFrameElement::Ruby { index }),
            )
            .collect();
    }
    if object.rich_text_ref.as_ref().is_some_and(|rich_text_ref| {
        matches!(
            rich_text_ref.kind,
            AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
        )
    }) {
        return agent_native_text_range_elements(context, object);
    }
    agent_native_element_for_object(object)
        .into_iter()
        .collect()
}

fn agent_native_text_range_elements(
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
) -> Vec<arcweft_render_native::NativeFrameElement> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return Vec::new();
    };
    let Some(textbox) = agent_native_textbox_for_rich_text_child(context.objects, object) else {
        return Vec::new();
    };
    let range = rich_text_ref.range;
    let frame = agent_observed_rich_text(textbox);
    frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter(move |(_, run)| agent_rich_text_ranges_overlap(run.range, range))
        .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
        .chain(
            frame
                .display_map
                .ruby_annotations
                .iter()
                .enumerate()
                .filter(move |(_, ruby)| agent_rich_text_ranges_overlap(ruby.base_range, range))
                .map(|(index, _)| arcweft_render_native::NativeFrameElement::Ruby { index }),
        )
        .collect()
}

fn agent_native_scope_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    selected: &[AgentNativeCaptureTarget<'_>],
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    let mut native_session = native_session;
    let mut min_x = capture_width;
    let mut min_y = capture_height;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for target in selected {
        let (x, y, width, height) = agent_native_target_rect(
            capture_width,
            capture_height,
            context,
            target,
            native_session.as_deref_mut(),
        )?;
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

fn agent_native_target_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    target: &AgentNativeCaptureTarget<'_>,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    match target {
        AgentNativeCaptureTarget::Observed(object) => agent_native_object_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
        ),
        AgentNativeCaptureTarget::RichTextElement {
            parent, element, ..
        } => agent_native_rich_text_element_rect(
            capture_width,
            capture_height,
            context,
            parent,
            *element,
            native_session,
        )?
        .ok_or_else(|| {
            eprintln!(
                "error: no native layout bounds match resource object {}",
                target.id()
            );
            ExitCode::from(2)
        }),
    }
}

fn agent_native_object_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    if agent_is_dialogue_textbox(object) {
        return agent_native_textbox_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
        );
    }
    if object.role.starts_with("rich_text_")
        && let Some(rect) = agent_native_rich_text_child_rect(
            capture_width,
            capture_height,
            context,
            object,
            native_session,
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

fn agent_native_textbox_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    textbox: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<(u32, u32, u32, u32), ExitCode> {
    let mut rect = agent_clamped_bbox_rect(
        capture_width,
        capture_height,
        textbox.bbox.x,
        textbox.bbox.y,
        textbox.bbox.width,
        textbox.bbox.height,
    );
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = match agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            capture_width,
            capture_height,
            left,
            top,
            context.page_index,
        )
        .with_time_seconds(context.capture_time_seconds),
        native_session,
    ) {
        Ok(bounds) => bounds,
        Err(arcweft_render_native::NativeWindowError::EmptyPages) => return Ok(rect),
        Err(error) => {
            eprintln!("error: native text layout measurement failed: {error}");
            return Err(ExitCode::FAILURE);
        }
    };
    for bounds in bounds {
        let child_rect = agent_clamped_bbox_rect(
            capture_width,
            capture_height,
            bounds.bbox.x,
            bounds.bbox.y,
            bounds.bbox.width,
            bounds.bbox.height,
        );
        rect = agent_union_rect(rect, child_rect, capture_width, capture_height);
    }
    Ok(rect)
}

fn agent_native_rich_text_child_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    object: &AgentObservedObject,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<(u32, u32, u32, u32)>, ExitCode> {
    if object.rich_text_ref.as_ref().is_some_and(|rich_text_ref| {
        matches!(
            rich_text_ref.kind,
            AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine
        ) && rich_text_ref.page == context.page_index
    }) {
        return Ok(Some(agent_clamped_bbox_rect(
            capture_width,
            capture_height,
            object.bbox.x,
            object.bbox.y,
            object.bbox.width,
            object.bbox.height,
        )));
    }
    let Some(element) = agent_native_element_for_object(object) else {
        return Ok(None);
    };
    let Some(textbox) = agent_native_textbox_for_rich_text_child(context.objects, object) else {
        return Ok(None);
    };
    agent_native_rich_text_element_rect(
        capture_width,
        capture_height,
        context,
        textbox,
        element,
        native_session,
    )
}

fn agent_native_rich_text_element_rect(
    capture_width: u32,
    capture_height: u32,
    context: AgentNativeCaptureContext<'_>,
    textbox: &AgentObservedObject,
    element: arcweft_render_native::NativeFrameElement,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Option<(u32, u32, u32, u32)>, ExitCode> {
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            capture_width,
            capture_height,
            left,
            top,
            context.page_index,
        )
        .with_time_seconds(context.capture_time_seconds),
        native_session,
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
        .find(|candidate| agent_is_dialogue_textbox(candidate) && candidate.id == parent_id)
}

fn agent_native_element_for_object(
    object: &AgentObservedObject,
) -> Option<arcweft_render_native::NativeFrameElement> {
    let Some(rich_text_ref) = &object.rich_text_ref else {
        return agent_native_element_for_object_id(&object.id);
    };
    match rich_text_ref.kind {
        AgentRichTextElementKind::TextPage | AgentRichTextElementKind::TextLine => None,
        AgentRichTextElementKind::TextRun
        | AgentRichTextElementKind::Ruby
        | AgentRichTextElementKind::TextObjectProxy => {
            agent_native_element_for_object_id(&object.id)
        }
        AgentRichTextElementKind::TextGlyph | AgentRichTextElementKind::GlyphCluster => {
            Some(arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: rich_text_ref.index,
                range_start: rich_text_ref.range.start,
                range_end: rich_text_ref.range.end,
            })
        }
    }
}

fn agent_native_element_for_object_id(
    object_id: &str,
) -> Option<arcweft_render_native::NativeFrameElement> {
    agent_native_element_and_role_for_object_id(object_id).map(|(element, _)| element)
}

fn agent_native_element_and_role_for_object_id(
    object_id: &str,
) -> Option<(arcweft_render_native::NativeFrameElement, &'static str)> {
    if let Some((_, index)) = object_id.rsplit_once(".run.") {
        return index.parse().ok().map(|index| {
            (
                arcweft_render_native::NativeFrameElement::TextRun { index },
                "rich_text_run",
            )
        });
    }
    if let Some((_, index)) = object_id.rsplit_once(".ruby.") {
        return index.parse().ok().map(|index| {
            (
                arcweft_render_native::NativeFrameElement::Ruby { index },
                "rich_text_ruby",
            )
        });
    }
    if let Some((_, suffix)) = object_id.split_once(".proxy.") {
        let mut parts = suffix.split('.');
        let run_index = parts.next()?.parse().ok()?;
        let proxy_index = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::TextObjectProxy {
                run_index,
                proxy_index,
            },
            "rich_text_proxy",
        ));
    }
    if let Some((_, suffix)) = object_id.split_once(".cluster.") {
        let mut parts = suffix.split('.');
        let index = parts.next()?.parse().ok()?;
        let range_start = parts.next()?.parse().ok()?;
        let range_end = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::GlyphCluster {
                index,
                range_start,
                range_end,
            },
            "rich_text_cluster",
        ));
    }
    if let Some((_, suffix)) = object_id.split_once(".glyph.") {
        let mut parts = suffix.split('.');
        let index = parts.next()?.parse().ok()?;
        let range_start = parts.next()?.parse().ok()?;
        let range_end = parts.next()?.parse().ok()?;
        if parts.next().is_some() {
            return None;
        }
        return Some((
            arcweft_render_native::NativeFrameElement::GlyphCluster {
                index,
                range_start,
                range_end,
            },
            "rich_text_glyph",
        ));
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

fn agent_union_rect(
    left: (u32, u32, u32, u32),
    right: (u32, u32, u32, u32),
    capture_width: u32,
    capture_height: u32,
) -> (u32, u32, u32, u32) {
    let min_x = left.0.min(right.0);
    let min_y = left.1.min(right.1);
    let max_x = left
        .0
        .saturating_add(left.2)
        .max(right.0.saturating_add(right.2));
    let max_y = left
        .1
        .saturating_add(left.3)
        .max(right.1.saturating_add(right.3));
    let width = max_x
        .saturating_sub(min_x)
        .min(capture_width.saturating_sub(min_x))
        .max(1);
    let height = max_y
        .saturating_sub(min_y)
        .min(capture_height.saturating_sub(min_y))
        .max(1);
    (min_x, min_y, width, height)
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
    crop.diagnostics.clone_from(&source.diagnostics);
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

fn agent_native_capture_targets_for_scope<'a>(
    context: AgentNativeCaptureContext<'a>,
    scope: &AgentCaptureScope,
) -> Result<Vec<AgentNativeCaptureTarget<'a>>, ExitCode> {
    match scope {
        AgentCaptureScope::Viewport => Ok(context
            .objects
            .iter()
            .map(AgentNativeCaptureTarget::Observed)
            .collect()),
        AgentCaptureScope::Layer(layer) => {
            let selected = context
                .objects
                .iter()
                .filter(|object| agent_object_matches_layer(object, layer))
                .map(AgentNativeCaptureTarget::Observed)
                .collect::<Vec<_>>();
            if selected.is_empty() {
                eprintln!("error: no observed object matches resource layer {layer}");
                return Err(ExitCode::from(2));
            }
            Ok(selected)
        }
        AgentCaptureScope::Object(object_id) => {
            if let Some(object) = context
                .objects
                .iter()
                .find(|object| object.id == *object_id)
            {
                return Ok(vec![AgentNativeCaptureTarget::Observed(object)]);
            }
            if let Some(target) = agent_native_rich_text_target_for_object_id(context, object_id) {
                return Ok(vec![target]);
            }
            eprintln!("error: no observed object matches resource object {object_id}");
            Err(ExitCode::from(2))
        }
    }
}

fn agent_native_rich_text_target_for_object_id<'a>(
    context: AgentNativeCaptureContext<'a>,
    object_id: &str,
) -> Option<AgentNativeCaptureTarget<'a>> {
    let parent_id = agent_rich_text_child_parent_object_id(object_id)?;
    let parent = context
        .objects
        .iter()
        .find(|candidate| agent_is_dialogue_textbox(candidate) && candidate.id == parent_id)?;
    let (element, role) = agent_native_element_and_role_for_object_id(object_id)?;
    Some(AgentNativeCaptureTarget::RichTextElement {
        id: object_id.to_owned(),
        role,
        parent,
        element,
    })
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
        AgentObserveResourceKind::PresentationTree => AgentObserveResourceOutput::One(Box::new(
            report
                .presentation_tree_resource()
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
    let mut resources = agent_observe_base_resources(report, image_output)?;
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

fn agent_observe_list_resources(
    report: &AgentObservationReport,
    image_output: Option<&AgentImageOutput>,
) -> Result<Vec<AgentResource>, ExitCode> {
    let mut resources = agent_observe_base_resources(report, image_output)?;
    let mut known = resources
        .iter()
        .map(|resource| resource.uri.clone())
        .collect::<BTreeSet<_>>();
    for layer in &report.layers {
        for capture in &layer.capture_refs.captures {
            if known.insert(capture.uri.clone()) {
                resources.push(agent_layer_capture_ref_resource(report, layer, capture));
            }
        }
    }
    for object in &report.objects {
        for capture in &object.capture_refs.captures {
            if known.insert(capture.uri.clone()) {
                resources.push(agent_object_capture_ref_resource(report, object, capture));
            }
        }
    }
    Ok(resources)
}

fn agent_observe_base_resources(
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
            .presentation_tree_resource()
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
    Ok(resources)
}

fn agent_layer_capture_ref_resource(
    report: &AgentObservationReport,
    layer: &AgentObservedLayer,
    capture: &AgentLayerCaptureRef,
) -> AgentResource {
    agent_capture_ref_resource(
        report,
        AgentCaptureRefResourceSpec {
            uri: &capture.uri,
            mime_type: &capture.mime_type,
            kind: capture.kind,
            scope: AgentImageScope::Layer {
                id: layer.id.clone(),
            },
            page: capture.page,
            width: capture.width,
            height: capture.height,
            object: None,
        },
    )
}

fn agent_object_capture_ref_resource(
    report: &AgentObservationReport,
    object: &AgentObservedObject,
    capture: &AgentObjectCaptureRef,
) -> AgentResource {
    agent_capture_ref_resource(
        report,
        AgentCaptureRefResourceSpec {
            uri: &capture.uri,
            mime_type: &capture.mime_type,
            kind: capture.kind,
            scope: AgentImageScope::Object {
                id: object.id.clone(),
            },
            page: capture.page,
            width: capture.width,
            height: capture.height,
            object: Some(AgentImageObjectRef::from_observed(object)),
        },
    )
}

struct AgentCaptureRefResourceSpec<'a> {
    uri: &'a str,
    mime_type: &'a str,
    kind: AgentImageKind,
    scope: AgentImageScope,
    page: usize,
    width: u32,
    height: u32,
    object: Option<AgentImageObjectRef>,
}

fn agent_capture_ref_resource(
    report: &AgentObservationReport,
    spec: AgentCaptureRefResourceSpec<'_>,
) -> AgentResource {
    AgentResource {
        uri: spec.uri.to_owned(),
        kind: arcweft_agent_protocol::AgentResourceKind::Image,
        mime_type: spec.mime_type.to_owned(),
        hash: report.render_hash.clone(),
        image: Some(AgentImageMetadata {
            kind: spec.kind,
            renderer: AgentImageRenderer::Native,
            scope: spec.scope,
            composition: agent_capture_ref_composition(spec.kind),
            page: spec.page,
            capture_step: 0,
            capture_time_millis: report.capture_time_millis.unwrap_or_default(),
            width: spec.width,
            height: spec.height,
            crop_origin: None,
            pixel_format: (spec.mime_type == "application/octet-stream")
                .then(|| "rgba8_unorm".to_owned()),
            row_stride_bytes: (spec.mime_type == "application/octet-stream")
                .then(|| spec.width.saturating_mul(4)),
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: spec.object,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::Text(String::new()),
    }
}

const fn agent_capture_ref_composition(kind: AgentImageKind) -> AgentImageComposition {
    match kind {
        AgentImageKind::Color => AgentImageComposition::FramebufferCrop,
        AgentImageKind::ObjectId => AgentImageComposition::ObjectIdAttachment,
        AgentImageKind::Mask => AgentImageComposition::MaskAttachment,
        AgentImageKind::Overlay | AgentImageKind::OverlaySvg => {
            AgentImageComposition::OverlayVector
        }
    }
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
    mut context: AgentObservationRunContext<'_>,
) -> Result<AgentObservationRunOutput, arcweft_host_adapter::HostAdapterError> {
    let viewport = AgentViewport {
        width: context.options.viewport_width,
        height: context.options.viewport_height,
        scale: 1.0,
    };
    let mut host = context
        .host_config
        .source_path
        .map(|path| {
            NativeTaskBridge::try_new(
                path,
                context.host_config.policy.clone(),
                context.host_config.adapter_registrars,
            )
        })
        .transpose()?;
    let mut objects: Vec<AgentObservedObject> = Vec::new();
    let mut diagnostics = Vec::new();
    let mut task_request_count = 0usize;
    let mut tick = 0usize;
    let effective_steps = agent_observe_effective_steps(context.options);
    let force_capture_step = context.options.capture_step.is_some();
    let mut native_session = context.native_session;
    for step_index in 0..effective_steps {
        tick = context.tick_offset.saturating_add(step_index);
        let result = executor.step_with_root_bindings(
            RuntimeStepInput {
                input_events: std::mem::take(&mut context.input_events),
                task_events: std::mem::take(&mut *context.task_events),
                ..RuntimeStepInput::default()
            },
            &context.options.values,
            step_options(context.options.mode, context.options.max_ops),
        );
        let RuntimeStepResult { mut output, .. } = result;
        diagnostics.extend(output.diagnostics.iter().map(|diagnostic| AgentDiagnostic {
            step: step_index,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("runtime".to_owned()),
            code: None,
            effect_id: None,
            message: diagnostic.message.clone(),
        }));
        for event in &output.flow_events {
            let textbox_index = objects
                .iter()
                .filter(|object| agent_is_dialogue_textbox(object))
                .count();
            match agent_observed_objects_for_flow_event(
                step_index,
                textbox_index,
                catalog,
                event,
                &viewport,
                context.options,
                native_session.as_deref_mut(),
            ) {
                Ok(event_objects) => objects.extend(event_objects),
                Err(diagnostic) => diagnostics.push(diagnostic),
            }
        }
        let task_requests = std::mem::take(&mut output.requests.tasks);
        task_request_count += task_requests.len();
        let done = matches!(
            executor.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        );
        if done && !force_capture_step {
            break;
        }
        if let Some(host) = host.as_mut() {
            *context.task_events = host.complete_tasks(task_requests);
        }
    }
    Ok(AgentObservationRunOutput {
        report: finish_agent_observation_report(
            executor,
            context.source_path,
            AgentObservationTrace {
                viewport,
                objects,
                diagnostics,
                task_request_count,
                tick,
            },
            context.options,
        ),
        image_frames: AgentImageFrameStore::default(),
    })
}

fn agent_observed_objects_for_flow_event(
    step: usize,
    textbox_index: usize,
    catalog: &LineDisplayCatalog,
    event: &FlowEvent,
    viewport: &AgentViewport,
    options: &AgentObserveOptions,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<Vec<AgentObservedObject>, AgentDiagnostic> {
    let capture_time_seconds = agent_observe_capture_time_seconds(options);
    let FlowEvent::DialogueLine { line, bindings } = event else {
        return Ok(Vec::new());
    };
    let Some(spec) = catalog.find(line) else {
        return Err(AgentDiagnostic {
            step,
            severity: AgentDiagnosticSeverity::Warning,
            source: Some("runtime_plan".to_owned()),
            code: Some("missing_display_catalog_entry".to_owned()),
            effect_id: None,
            message: format!("missing display catalog entry for line {}", line.0),
        });
    };
    let frame = spec
        .resolve_frame(&RuntimeLineContext::new(bindings.clone()))
        .map_err(|error| AgentDiagnostic {
            step,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("render_text".to_owned()),
            code: Some("line_display_resolve_failed".to_owned()),
            effect_id: None,
            message: error.to_string(),
        })?;
    let mut textbox = agent_textbox_object(step, textbox_index, frame, viewport, options);
    let mut native_session = native_session;
    if let Some(capture_bbox) = agent_native_textbox_capture_bbox_for_page(
        &textbox,
        viewport,
        0,
        capture_time_seconds,
        native_session.as_deref_mut(),
    ) {
        textbox.capture_refs =
            agent_object_capture_refs_for_page("cli", step, &textbox.id, &capture_bbox, 0);
    }
    let native_bounds = agent_native_rich_text_element_bboxes(
        &textbox,
        viewport,
        capture_time_seconds,
        native_session.as_deref_mut(),
    );
    let children = agent_rich_text_child_objects(
        step,
        textbox_index,
        &textbox,
        viewport,
        capture_time_seconds,
        &native_bounds,
        native_session,
    );
    let mut objects = Vec::with_capacity(1 + children.len());
    objects.push(textbox);
    objects.extend(children);
    Ok(objects)
}

fn finish_agent_observation_report(
    executor: &RuntimeExecutorInstance,
    source_path: &Path,
    trace: AgentObservationTrace,
    options: &AgentObserveOptions,
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
    let mut actions = agent_action_targets(&objects);
    actions.extend(agent_action_targets_for_runtime_status(
        &executor.fiber().status,
    ));
    let layers = agent_observed_layers("cli", tick, &objects);
    let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
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
        presentation_tree,
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
        capture_time_millis: agent_observe_report_capture_time_millis(options),
        task_requests: task_request_count,
        final_status: status,
        overlay_svg: None,
    }
}

fn agent_action_targets_for_runtime_status(status: &FlowFiberStatus) -> Vec<AgentActionTarget> {
    let FlowFiberStatus::Choice(state) = status else {
        return Vec::new();
    };
    state
        .options
        .iter()
        .map(|option| {
            let target = option.id.as_deref().unwrap_or(option.label.as_str());
            AgentActionTarget {
                id: format!("action.select_choice.{target}"),
                target: target.to_owned(),
                action: AgentActionKind::SelectChoice,
                kind: AgentActionDispatch::Semantic,
                enabled: true,
            }
        })
        .collect()
}

fn agent_runtime_presentation_image_observation(
    source_path: &Path,
    step: usize,
    viewport: &AgentViewport,
    calls: &[RuntimeCall],
    capture_time_seconds: f32,
) -> (AgentUiImageObservation, Vec<AgentDiagnostic>) {
    let mut inputs = Vec::new();
    let mut background_input = None;
    let mut diagnostics = Vec::new();
    let mut image_decode_cache = AgentSourceImageDecodeCache::default();
    let image_declarations =
        agent_load_declared_image_objects_or_diagnostic(source_path, step, &mut diagnostics);
    for (call_index, call) in calls.iter().enumerate() {
        let image_call =
            match agent_runtime_image_call(call, call_index, viewport, &image_declarations) {
                Ok(Some(image_call)) => image_call,
                Ok(None) => continue,
                Err(message) => {
                    diagnostics.push(AgentDiagnostic {
                        step,
                        severity: AgentDiagnosticSeverity::Warning,
                        source: Some("agent.presentation_image".to_owned()),
                        code: Some("image_call_invalid".to_owned()),
                        effect_id: Some(call.callee.clone()),
                        message,
                    });
                    continue;
                }
            };
        match image_decode_cache.decode_source_image_asset(source_path, image_call.asset.as_str()) {
            Ok(image) => {
                let input = agent_image_presentation_input(&image_call, image);
                if image_call.background_slot {
                    background_input = Some(input);
                } else {
                    inputs.push(input);
                }
            }
            Err(message) => diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_asset_unavailable".to_owned()),
                effect_id: Some(call.callee.clone()),
                message,
            }),
        }
    }
    if let Some(background_input) = background_input {
        inputs.insert(0, background_input);
    }
    if inputs.is_empty() {
        return (AgentUiImageObservation::default(), diagnostics);
    }
    let frame = match arcweft_ui::UiImagePresentationFrame::from_inputs(inputs) {
        Ok(frame) => frame,
        Err(error) => {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_ui_lower_failed".to_owned()),
                effect_id: None,
                message: error.to_string(),
            });
            return (AgentUiImageObservation::default(), diagnostics);
        }
    };
    let (outputs, image_sources) = frame.into_parts();
    let layer_tree = agent_layer_tree_for_ui_outputs(&outputs);
    let mut builder = arcweft_runtime_host::UiFrameCommitBuilder::new(&layer_tree);
    for (layer, output) in outputs {
        if let Err(error) = builder.push_layer(layer, output) {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_ui_commit_failed".to_owned()),
                effect_id: None,
                message: error.to_string(),
            });
        }
    }
    let visual_time_millis = u64::from(agent_capture_time_millis(capture_time_seconds));
    (
        agent_image_observation_from_ui_frame(
            "cli",
            step,
            viewport,
            &builder.finish(),
            &image_sources,
            visual_time_millis,
        ),
        diagnostics,
    )
}

fn agent_load_declared_image_objects_or_diagnostic(
    source_path: &Path,
    step: usize,
    diagnostics: &mut Vec<AgentDiagnostic>,
) -> BTreeMap<String, DeclaredImageObject> {
    match load_declared_image_objects(source_path) {
        Ok(declarations) => declarations,
        Err(message) => {
            diagnostics.push(AgentDiagnostic {
                step,
                severity: AgentDiagnosticSeverity::Warning,
                source: Some("agent.presentation_image".to_owned()),
                code: Some("image_declaration_unavailable".to_owned()),
                effect_id: None,
                message,
            });
            BTreeMap::new()
        }
    }
}

#[derive(Clone, Debug)]
struct AgentRuntimeImageCall {
    asset: arcweft_id::PublicId,
    object: arcweft_id::PublicId,
    target: arcweft_id::PublicId,
    layer: arcweft_id::PublicId,
    bounds: arcweft_presentation::hit::HitRect,
    fit: arcweft_presentation::image::ImageObjectFit,
    alignment: ImageObjectAlignment,
    opacity_milli: u16,
    playback: ImageObjectPlayback,
    transform: ImageObjectTransform,
    depth_milli: i32,
    actions: Vec<arcweft_id::PublicId>,
    params: BTreeMap<arcweft_id::PublicId, ImageObjectParam>,
    proxies: Vec<ImageObjectProxy>,
    background_slot: bool,
    enabled: bool,
    visible: bool,
}

#[derive(Debug, Default)]
struct AgentSourceImageDecodeCache {
    images: BTreeMap<String, arcweft_image::DecodedImage>,
    hits: usize,
    misses: usize,
}

impl AgentSourceImageDecodeCache {
    fn decode_source_image_asset(
        &mut self,
        source_path: &Path,
        asset_id: &str,
    ) -> Result<arcweft_image::DecodedImage, String> {
        if let Some(image) = self.images.get(asset_id) {
            self.hits = self.hits.saturating_add(1);
            return Ok(image.clone());
        }
        let image = agent_decode_source_image_asset(source_path, asset_id)?;
        self.misses = self.misses.saturating_add(1);
        self.images.insert(asset_id.to_owned(), image.clone());
        Ok(image)
    }

    #[cfg(test)]
    fn hits(&self) -> usize {
        self.hits
    }

    #[cfg(test)]
    fn misses(&self) -> usize {
        self.misses
    }
}

fn agent_runtime_image_call(
    call: &RuntimeCall,
    call_index: usize,
    viewport: &AgentViewport,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Result<Option<AgentRuntimeImageCall>, String> {
    match call.callee.as_str() {
        "bg" => agent_background_runtime_image_call(call, viewport).map(Some),
        "image" | "image.show" => {
            agent_object_runtime_image_call(call, call_index, image_declarations).map(Some)
        }
        _ => Ok(None),
    }
}

fn agent_background_runtime_image_call(
    call: &RuntimeCall,
    viewport: &AgentViewport,
) -> Result<AgentRuntimeImageCall, String> {
    let asset = agent_image_call_asset(call)
        .ok_or_else(|| "`bg(...)` requires an `asset` argument".to_owned())?;
    Ok(AgentRuntimeImageCall {
        asset,
        object: arcweft_id::PublicId::try_new("image.background.default")
            .expect("static image object id is valid"),
        target: arcweft_id::PublicId::try_new("target.background.default")
            .expect("static target id is valid"),
        layer: arcweft_id::PublicId::try_new("layer.background").expect("static layer id is valid"),
        bounds: arcweft_presentation::hit::HitRect::new(
            0.0,
            0.0,
            viewport.width.to_string().parse().unwrap_or(0.0),
            viewport.height.to_string().parse().unwrap_or(0.0),
        ),
        fit: agent_call_named_value(call, "fit")
            .and_then(agent_image_fit_from_call_arg)
            .unwrap_or(arcweft_presentation::image::ImageObjectFit::Cover),
        alignment: agent_image_call_alignment(call),
        opacity_milli: agent_call_named_value(call, "opacity")
            .and_then(agent_image_call_opacity_milli)
            .unwrap_or(1_000),
        playback: agent_image_call_playback(call),
        transform: ImageObjectTransform::identity(),
        depth_milli: 0,
        actions: Vec::new(),
        params: BTreeMap::new(),
        proxies: Vec::new(),
        background_slot: true,
        enabled: true,
        visible: true,
    })
}

fn agent_object_runtime_image_call(
    call: &RuntimeCall,
    call_index: usize,
    image_declarations: &BTreeMap<String, DeclaredImageObject>,
) -> Result<AgentRuntimeImageCall, String> {
    if let Some(declaration_id) = agent_declared_image_object_id(call) {
        let declaration = image_declarations
            .get(declaration_id.as_str())
            .ok_or_else(|| {
                format!(
                    "declared image object `{}` was not found",
                    declaration_id.as_str()
                )
            })?;
        let expanded_call = RuntimeCall {
            callee: call.callee.clone(),
            args: merge_declared_image_args(declaration, agent_image_call_override_args(call)),
        };
        return agent_object_runtime_image_call(&expanded_call, call_index, image_declarations);
    }
    let asset = agent_image_call_asset(call)
        .ok_or_else(|| "`image(...)` requires an `asset` argument".to_owned())?;
    let x = agent_required_image_call_length(call, "x", 1)?;
    let y = agent_required_image_call_length(call, "y", 2)?;
    let width = agent_required_image_call_length(call, "width", 3)?;
    let height = agent_required_image_call_length(call, "height", 4)?;
    let object = agent_call_named_value(call, "id")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            let stem = asset
                .as_str()
                .strip_prefix("asset.")
                .unwrap_or(asset.as_str());
            arcweft_id::PublicId::try_new(format!("image.{stem}.{call_index}"))
                .expect("generated image object id is valid")
        });
    let target = agent_call_named_value(call, "target")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            arcweft_id::PublicId::try_new(format!("target.{}", object.as_str()))
                .expect("generated image target id is valid")
        });
    let layer = agent_call_named_value(call, "layer")
        .and_then(agent_public_id_from_call_arg)
        .unwrap_or_else(|| {
            arcweft_id::PublicId::try_new("layer.game_ui").expect("static layer id is valid")
        });
    let fit = agent_call_named_value(call, "fit")
        .and_then(agent_image_fit_from_call_arg)
        .unwrap_or_default();
    let alignment = agent_image_call_alignment(call);
    let depth_milli = agent_call_named_value(call, "depth")
        .and_then(agent_image_call_milli)
        .unwrap_or_default();
    let opacity_milli = agent_call_named_value(call, "opacity")
        .and_then(agent_image_call_opacity_milli)
        .unwrap_or(1_000);
    let playback = agent_image_call_playback(call);
    let transform = agent_image_call_transform(call);
    let enabled = agent_call_named_value(call, "enabled")
        .and_then(agent_image_call_bool)
        .unwrap_or(true);
    let visible = agent_call_named_value(call, "visible")
        .and_then(agent_image_call_bool)
        .unwrap_or(true);
    Ok(AgentRuntimeImageCall {
        asset,
        object,
        target,
        layer,
        bounds: arcweft_presentation::hit::HitRect::new(x, y, width, height),
        fit,
        alignment,
        opacity_milli,
        playback,
        transform,
        depth_milli,
        actions: agent_image_call_actions(call),
        params: agent_image_call_params(call),
        proxies: agent_image_call_proxies(call),
        background_slot: false,
        enabled,
        visible,
    })
}

fn agent_declared_image_object_id(call: &RuntimeCall) -> Option<arcweft_id::PublicId> {
    let id = agent_call_positional_value(call, 0).and_then(public_image_ref_arg)?;
    arcweft_id::PublicId::try_new(id).ok()
}

fn agent_image_call_override_args(call: &RuntimeCall) -> Vec<String> {
    let mut skipped_decl_ref = false;
    call.args
        .iter()
        .filter_map(|arg| {
            if runtime_arg_name(arg).is_none()
                && !skipped_decl_ref
                && public_image_ref_arg(arg).is_some()
            {
                skipped_decl_ref = true;
                None
            } else {
                Some(arg.clone())
            }
        })
        .collect()
}

fn agent_image_call_asset(call: &RuntimeCall) -> Option<arcweft_id::PublicId> {
    agent_call_named_value(call, "asset")
        .or_else(|| agent_call_positional_value(call, 0))
        .and_then(agent_asset_id_from_call_arg)
}

fn agent_asset_id_from_call_arg(arg: &str) -> Option<arcweft_id::PublicId> {
    let id = agent_public_id_from_call_arg(arg)?;
    id.as_str().starts_with("asset.").then_some(id)
}

fn agent_public_id_from_call_arg(arg: &str) -> Option<arcweft_id::PublicId> {
    let value = arg.trim().trim_matches('"').trim_matches('\'');
    let value = value.strip_prefix('@').unwrap_or(value);
    arcweft_id::PublicId::try_new(value).ok()
}

fn agent_call_named_value<'a>(call: &'a RuntimeCall, name: &str) -> Option<&'a str> {
    call.args.iter().find_map(|arg| {
        let (arg_name, value) = arg.split_once(" = ")?;
        (arg_name.trim() == name).then_some(value.trim())
    })
}

fn agent_call_positional_value(call: &RuntimeCall, index: usize) -> Option<&str> {
    call.args
        .iter()
        .filter(|arg| !arg.contains(" = "))
        .nth(index)
        .map(String::as_str)
}

fn agent_required_image_call_length(
    call: &RuntimeCall,
    name: &str,
    positional_index: usize,
) -> Result<f32, String> {
    let value = agent_call_named_value(call, name)
        .or_else(|| agent_call_positional_value(call, positional_index))
        .ok_or_else(|| format!("`image(...)` requires `{name}`"))?;
    agent_image_call_length(value)
        .ok_or_else(|| format!("`image(...)` argument `{name}` must be a finite px length"))
}

fn agent_image_call_length(value: &str) -> Option<f32> {
    let value = value
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .strip_suffix("px")
        .unwrap_or_else(|| value.trim().trim_matches('"').trim_matches('\''))
        .trim();
    let parsed = value.parse::<f32>().ok()?;
    parsed.is_finite().then_some(parsed)
}

fn agent_image_fit_from_call_arg(
    value: &str,
) -> Option<arcweft_presentation::image::ImageObjectFit> {
    match value.trim().trim_matches('"').trim_matches('\'') {
        "contain" => Some(arcweft_presentation::image::ImageObjectFit::Contain),
        "cover" => Some(arcweft_presentation::image::ImageObjectFit::Cover),
        "stretch" => Some(arcweft_presentation::image::ImageObjectFit::Stretch),
        "intrinsic" => Some(arcweft_presentation::image::ImageObjectFit::Intrinsic),
        _ => None,
    }
}

fn agent_image_call_alignment(call: &RuntimeCall) -> ImageObjectAlignment {
    ImageObjectAlignment::new(
        agent_call_named_value(call, "alignment.x")
            .or_else(|| agent_call_named_value(call, "align.x"))
            .and_then(|value| agent_image_alignment_component_milli(value, "x"))
            .unwrap_or_else(|| ImageObjectAlignment::default().x_milli()),
        agent_call_named_value(call, "alignment.y")
            .or_else(|| agent_call_named_value(call, "align.y"))
            .and_then(|value| agent_image_alignment_component_milli(value, "y"))
            .unwrap_or_else(|| ImageObjectAlignment::default().y_milli()),
    )
}

fn agent_image_alignment_component_milli(value: &str, axis: &str) -> Option<i32> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    match (axis, value) {
        ("x", "left" | "start") | ("y", "top" | "start") => return Some(0),
        ("x" | "y", "center" | "middle") => return Some(500),
        ("x", "right" | "end") | ("y", "bottom" | "end") => return Some(1_000),
        _ => {}
    }
    if let Ok(integer) = value.parse::<i32>() {
        return Some(if (0..=1).contains(&integer) {
            integer.saturating_mul(1_000)
        } else {
            integer.clamp(0, 1_000)
        });
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    format!("{:.0}", (decimal * 1_000.0).round().clamp(0.0, 1_000.0))
        .parse()
        .ok()
}

fn agent_image_call_actions(call: &RuntimeCall) -> Vec<arcweft_id::PublicId> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            matches!(name.trim(), "action" | "actions").then_some(value)
        })
        .flat_map(|value| value.split(','))
        .filter_map(agent_public_id_from_call_arg)
        .collect()
}

fn agent_image_call_params(call: &RuntimeCall) -> BTreeMap<arcweft_id::PublicId, ImageObjectParam> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            let key = name.trim().strip_prefix("param.")?;
            let key = arcweft_id::PublicId::try_new(format!("param.{key}")).ok()?;
            Some((key, agent_image_call_param(value.trim())))
        })
        .collect()
}

fn agent_image_call_proxies(call: &RuntimeCall) -> Vec<ImageObjectProxy> {
    let Some(id) = agent_call_named_value(call, "proxy.id").and_then(agent_public_id_from_call_arg)
    else {
        return Vec::new();
    };
    let mut proxy = ImageObjectProxy::new(id);
    if let Some(value) = agent_call_named_value(call, "proxy.type") {
        proxy = proxy.with_type_name(value.trim().trim_matches('"').trim_matches('\''));
    }
    if let Some(value) = agent_call_named_value(call, "proxy.role") {
        proxy = proxy.with_role(value.trim().trim_matches('"').trim_matches('\''));
    }
    if let Some(layer) =
        agent_call_named_value(call, "proxy.layer").and_then(agent_public_id_from_call_arg)
    {
        proxy = proxy.with_layer(layer);
    }
    if let Some(depth) =
        agent_call_named_value(call, "proxy.depth").and_then(agent_image_call_milli)
    {
        proxy = proxy.with_depth_milli(depth);
    }
    if let Some(hit_test) =
        agent_call_named_value(call, "proxy.hit_test").and_then(agent_image_call_bool)
    {
        proxy = proxy.with_hit_test(hit_test);
    }
    for (key, value) in agent_image_call_proxy_params(call) {
        proxy = proxy.with_param(key, value);
    }
    vec![proxy]
}

fn agent_image_call_proxy_params(
    call: &RuntimeCall,
) -> BTreeMap<arcweft_id::PublicId, ImageObjectParam> {
    call.args
        .iter()
        .filter_map(|arg| {
            let (name, value) = arg.split_once(" = ")?;
            let key = name.trim().strip_prefix("proxy.param.")?;
            let key = arcweft_id::PublicId::try_new(format!("param.{key}")).ok()?;
            Some((key, agent_image_call_param(value.trim())))
        })
        .collect()
}

fn agent_image_call_param(value: &str) -> ImageObjectParam {
    let trimmed = value.trim();
    if let Some(id) = agent_public_id_from_call_arg(trimmed).filter(|_| trimmed.starts_with('@')) {
        return ImageObjectParam::Id(id);
    }
    let unquoted = trimmed.trim_matches('"').trim_matches('\'');
    match unquoted {
        "true" => ImageObjectParam::Bool(true),
        "false" => ImageObjectParam::Bool(false),
        _ => agent_image_call_milli(unquoted).map_or_else(
            || ImageObjectParam::Text(unquoted.to_owned()),
            ImageObjectParam::Milli,
        ),
    }
}

fn agent_image_call_milli(value: &str) -> Option<i32> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(integer) = value.parse::<i32>() {
        return Some(integer);
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    (decimal * 1_000.0)
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_string()
        .parse()
        .ok()
}

fn agent_image_call_opacity_milli(value: &str) -> Option<u16> {
    let value = value.trim().trim_matches('"').trim_matches('\'');
    if let Ok(integer) = value.parse::<u16>() {
        if integer <= 1 {
            return Some(integer.saturating_mul(1_000));
        }
        return Some(integer.min(1_000));
    }
    let decimal = value.parse::<f64>().ok()?;
    if !decimal.is_finite() {
        return None;
    }
    (decimal * 1_000.0)
        .round()
        .clamp(0.0, 1_000.0)
        .to_string()
        .parse()
        .ok()
}

fn agent_image_call_playback(call: &RuntimeCall) -> ImageObjectPlayback {
    let start_time_millis = agent_call_named_value(call, "playback.start")
        .or_else(|| agent_call_named_value(call, "playback.start_time"))
        .and_then(agent_image_call_time_millis)
        .unwrap_or_default();
    let mut playback = ImageObjectPlayback::new(start_time_millis);
    if let Some(rate_milli) =
        agent_call_named_value(call, "playback.rate").and_then(agent_image_call_rate_milli)
    {
        playback = playback.with_rate_milli(rate_milli);
    }
    if let Some(paused_at_millis) = agent_call_named_value(call, "playback.paused_at")
        .or_else(|| agent_call_named_value(call, "playback.pause_at"))
        .and_then(agent_image_call_time_millis)
    {
        playback = playback.paused_at(paused_at_millis);
    }
    if let Some(local_time_millis) = agent_call_named_value(call, "playback.local_time")
        .or_else(|| agent_call_named_value(call, "playback.pinned_local_time"))
        .and_then(agent_image_call_time_millis)
    {
        playback = playback.pinned_local_time(local_time_millis);
    }
    playback
}

fn agent_image_call_time_millis(value: &str) -> Option<u64> {
    let value = value.trim().trim_matches('"').trim_matches('\'').trim();
    let (number, multiplier) = if let Some(number) = value.strip_suffix("ms") {
        (number.trim(), 1.0)
    } else if let Some(number) = value.strip_suffix('s') {
        (number.trim(), 1_000.0)
    } else {
        (value, 1_000.0)
    };
    let millis = number.parse::<f64>().ok()? * multiplier;
    if !millis.is_finite() || millis < 0.0 {
        return None;
    }
    format!("{:.0}", millis.round()).parse().ok()
}

fn agent_image_call_rate_milli(value: &str) -> Option<u32> {
    let value = value.trim().trim_matches('"').trim_matches('\'').trim();
    let parsed = value
        .strip_suffix('x')
        .unwrap_or(value)
        .parse::<f64>()
        .ok()?;
    let milli = if (0.0..=1.0).contains(&parsed) {
        parsed * 1_000.0
    } else {
        parsed
    };
    if !milli.is_finite() || milli < 0.0 {
        return None;
    }
    format!("{:.0}", milli.round()).parse().ok()
}

fn agent_image_call_bool(value: &str) -> Option<bool> {
    match value.trim().trim_matches('"').trim_matches('\'') {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn agent_image_call_transform(call: &RuntimeCall) -> ImageObjectTransform {
    let mut transform = ImageObjectTransform::identity();
    if let Some(value) = agent_call_named_value(call, "transform.m11")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m11_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m12")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m12_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m21")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m21_milli = value;
    }
    if let Some(value) = agent_call_named_value(call, "transform.m22")
        .and_then(agent_image_call_transform_component_milli)
    {
        transform.m22_milli = value;
    }
    if let Some(value) =
        agent_call_named_value(call, "transform.tx").and_then(agent_image_call_length_milli)
    {
        transform.tx_milli = value;
    }
    if let Some(value) =
        agent_call_named_value(call, "transform.ty").and_then(agent_image_call_length_milli)
    {
        transform.ty_milli = value;
    }
    transform
}

fn agent_image_call_transform_component_milli(value: &str) -> Option<i32> {
    agent_image_call_milli(value)
}

fn agent_image_call_length_milli(value: &str) -> Option<i32> {
    let pixels = agent_image_call_length(value)?;
    let milli = f64::from(pixels) * 1_000.0;
    milli
        .round()
        .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
        .to_string()
        .parse()
        .ok()
}

fn agent_decode_source_image_asset(
    source_path: &Path,
    asset_id: &str,
) -> Result<arcweft_image::DecodedImage, String> {
    let Some(asset_stem) = asset_id.strip_prefix("asset.") else {
        return Err(format!(
            "image asset id `{asset_id}` must start with `asset.`"
        ));
    };
    let asset_relative = asset_stem.replace('.', "/");
    let asset_root = source_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(".arcweft")
        .join("asset");
    for (extension, format) in [
        ("png", arcweft_image::ImageFormat::Png),
        ("jpg", arcweft_image::ImageFormat::Jpeg),
        ("jpeg", arcweft_image::ImageFormat::Jpeg),
        ("gif", arcweft_image::ImageFormat::Gif),
        ("webp", arcweft_image::ImageFormat::WebP),
    ] {
        let path = asset_root.join(format!("{asset_relative}.{extension}"));
        if !path.exists() {
            continue;
        }
        let bytes = fs::read(&path)
            .map_err(|error| format!("failed to read image asset {}: {error}", path.display()))?;
        return arcweft_image::decode_image_bytes(
            format,
            &bytes,
            arcweft_image::ImageDecodeOptions::default(),
        )
        .map_err(|error| format!("failed to decode image asset {}: {error}", path.display()));
    }
    Err(format!(
        "image asset `{asset_id}` was not found under {}",
        asset_root.display()
    ))
}

fn agent_image_presentation_input(
    call: &AgentRuntimeImageCall,
    image: arcweft_image::DecodedImage,
) -> arcweft_ui::UiImagePresentationInput {
    let object = arcweft_presentation::image::ImagePresentationObject::new(
        arcweft_presentation::image::ImageObjectId::new(call.object.clone()),
        arcweft_presentation::image::ImageAssetRef::new(call.asset.clone()),
        arcweft_presentation::layer::LayerId::new(call.layer.clone()),
        arcweft_presentation::input::InteractionTarget::new(call.target.clone()),
        call.bounds,
    )
    .with_fit(call.fit)
    .with_alignment(call.alignment)
    .with_opacity_milli(call.opacity_milli)
    .with_playback(call.playback)
    .with_transform(call.transform)
    .with_depth_milli(call.depth_milli)
    .with_enabled(call.enabled)
    .with_visible(call.visible);
    let object = call.actions.iter().cloned().fold(
        object,
        arcweft_presentation::image::ImagePresentationObject::with_action,
    );
    let object = call.proxies.iter().cloned().fold(
        object,
        arcweft_presentation::image::ImagePresentationObject::with_proxy,
    );
    let object = call
        .params
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .fold(object, |object, (key, value)| object.with_param(key, value));
    arcweft_ui::UiImagePresentationInput::new(object, image)
}

fn agent_layer_tree_for_ui_outputs(
    outputs: &[(
        arcweft_presentation::layer::LayerId,
        arcweft_ui::UiLayerOutput,
    )],
) -> arcweft_presentation::layer::LayerTree {
    use arcweft_presentation::layer::{LayerKind, LayerNode, LayerOrder, LayerTree, RenderPhase};
    let root = arcweft_presentation::layer::LayerId::new(
        arcweft_id::PublicId::try_new("layer.root").expect("static root layer id is valid"),
    );
    let mut tree = LayerTree::new(LayerNode::new(
        root.clone(),
        LayerKind::Root,
        LayerOrder {
            phase: RenderPhase::Background,
            z: 0,
            stable_index: 0,
        },
    ));
    for (index, (layer, _)) in outputs.iter().enumerate() {
        let kind = if layer.public_id().as_str() == "layer.background" {
            LayerKind::Background
        } else {
            LayerKind::GameUi
        };
        let phase = match kind {
            LayerKind::Background => RenderPhase::Background,
            _ => RenderPhase::GameUi,
        };
        tree.insert(
            LayerNode::new(
                layer.clone(),
                kind,
                LayerOrder {
                    phase,
                    z: 0,
                    stable_index: u32::try_from(index).unwrap_or(u32::MAX),
                },
            )
            .with_parent(root.clone()),
        )
        .expect("generated image presentation layer tree is valid");
    }
    tree
}

fn agent_refresh_observation_object_indexes(report: &mut AgentObservationReport) {
    let object_refs = report.objects.iter().collect::<Vec<_>>();
    let overlay_svg = agent_overlay_svg(&report.viewport, &object_refs);
    report.render_hash = hash_hex(overlay_svg.as_bytes());
    report.layers = agent_observed_layers("cli", report.tick, &report.objects);
    report.presentation_tree =
        AgentPresentationTree::from_layers_and_objects(&report.layers, &report.objects);
    report.actions = agent_action_targets(&report.objects);
}

fn agent_action_targets(objects: &[AgentObservedObject]) -> Vec<AgentActionTarget> {
    objects
        .iter()
        .flat_map(agent_action_targets_for_object)
        .collect()
}

fn agent_action_targets_for_object(object: &AgentObservedObject) -> Vec<AgentActionTarget> {
    match &object.content {
        AgentObservedObjectContent::RichText { .. } if agent_is_dialogue_textbox(object) => {
            vec![AgentActionTarget {
                id: format!("action.advance_text.{}", object.id),
                target: object.id.clone(),
                action: AgentActionKind::AdvanceText,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            }]
        }
        AgentObservedObjectContent::Image(content) => content
            .actions
            .iter()
            .map(|action| AgentActionTarget {
                id: action.clone(),
                target: content
                    .target
                    .clone()
                    .or_else(|| content.object.clone())
                    .unwrap_or_else(|| object.id.clone()),
                action: AgentActionKind::Invoke,
                kind: AgentActionDispatch::Semantic,
                enabled: object.visible && object.enabled,
            })
            .collect(),
        AgentObservedObjectContent::RichText { .. } | AgentObservedObjectContent::Custom { .. } => {
            Vec::new()
        }
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
    diagnostics: Vec<arcweft_render_native::NativeVisualDiagnostic>,
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
            diagnostics: Vec::new(),
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
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
    image_frames: &AgentImageFrameStore,
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
            let scope = agent_capture_scope_for_options(options);
            report.images = vec![AgentImageResource {
                kind: AgentImageKind::OverlaySvg,
                renderer: AgentImageRenderer::Native,
                scope: agent_image_scope_for_capture_scope(&scope),
                composition: AgentImageComposition::OverlayVector,
                page: 0,
                capture_step: report.steps,
                capture_time_millis: agent_capture_time_millis(agent_observe_capture_time_seconds(
                    options,
                )),
                uri: uri.clone(),
                mime_type: "image/svg+xml".to_owned(),
                width: report.viewport.width,
                height: report.viewport.height,
                hash,
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: agent_image_object_for_capture_scope(report, &scope),
                diagnostics: Vec::new(),
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
            let capture_result = match native_session {
                Some(native_session) => agent_native_capture_image_with_frame_store(
                    report,
                    &request,
                    native_session,
                    image_frames,
                )?,
                None => agent_native_capture_image(report, &request)?,
            };
            report
                .diagnostics
                .extend(capture_result.image.diagnostics.clone());
            let (mut image, bytes) = (capture_result.image, capture_result.bytes);
            image.written = options.out.as_deref().map(report_path);
            report.render_hash.clone_from(&image.hash);
            let uri = image.uri.clone();
            report.images = vec![image];
            Ok(Some(AgentImageOutput { uri, bytes }))
        }
    }
}

fn agent_native_visual_diagnostics(
    step: usize,
    diagnostics: &[arcweft_render_native::NativeVisualDiagnostic],
) -> Vec<AgentDiagnostic> {
    diagnostics
        .iter()
        .map(|diagnostic| AgentDiagnostic {
            step,
            severity: match diagnostic.severity {
                arcweft_render_native::NativeVisualDiagnosticSeverity::Error => {
                    AgentDiagnosticSeverity::Error
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Warning => {
                    AgentDiagnosticSeverity::Warning
                }
                arcweft_render_native::NativeVisualDiagnosticSeverity::Info => {
                    AgentDiagnosticSeverity::Info
                }
            },
            source: Some("native_rich_text".to_owned()),
            code: Some(diagnostic.code.clone()),
            effect_id: diagnostic.effect_id.clone(),
            message: format!(
                "native rich-text {}: {}",
                diagnostic.code, diagnostic.message
            ),
        })
        .collect()
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
        capture_step: report.steps,
        capture_time_seconds: agent_observe_capture_time_seconds(options),
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
            .filter(|object| agent_object_matches_layer(object, layer))
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
    options: &AgentObserveOptions,
) -> AgentObservedObject {
    let width = viewport.width.saturating_sub(192);
    let lines = u32::try_from(frame.text.lines().count().max(1)).unwrap_or(u32::MAX);
    let object_slot = u32::try_from(index % 4).unwrap_or(0);
    let bottom_margin = 48 + object_slot * 10;
    let default_height = (96 + lines * 28).min(220);
    let height = options
        .textbox_height
        .unwrap_or(default_height)
        .min(viewport.height.saturating_sub(bottom_margin))
        .max(1);
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
        parent_id: None,
        entity: Some(frame.callee.clone()),
        layer: "dialogue".to_owned(),
        role: AGENT_ROLE_DIALOGUE_TEXTBOX.to_owned(),
        visible: true,
        enabled: true,
        bbox: bbox.clone(),
        polygon: bbox.polygon(),
        capture_refs,
        object_layer: None,
        object_depth: None,
        text: Some(frame.text.clone()),
        rich_text_ref: None,
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(frame),
        },
    }
}

fn agent_observed_rich_text(object: &AgentObservedObject) -> &LineDisplayFrame {
    object
        .rich_text_frame()
        .expect("observed rich-text object carries rich-text content")
}

#[cfg_attr(
    not(test),
    expect(
        dead_code,
        reason = "wired into live Agent observe once runtime UI commits are exposed to the CLI adapter"
    )
)]
pub(crate) fn agent_image_objects_from_ui_frame(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    frame: &UiFrameCommit,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> Vec<AgentObservedObject> {
    agent_image_observation_from_ui_frame(
        session_id,
        step,
        viewport,
        frame,
        images,
        visual_time_millis,
    )
    .objects
}

fn agent_image_observation_from_ui_frame(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    frame: &UiFrameCommit,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> AgentUiImageObservation {
    let mut observation = AgentUiImageObservation::default();
    frame
        .image_items()
        .into_iter()
        .filter_map(|item| {
            agent_image_observation_from_ui_item(
                session_id,
                step,
                viewport,
                &item,
                images,
                visual_time_millis,
            )
        })
        .for_each(|(object, frame)| {
            observation
                .image_frames
                .frames_by_object
                .insert(object.id.clone(), frame);
            observation.objects.push(object);
        });
    observation
}

fn agent_image_observation_from_ui_item(
    session_id: &str,
    step: usize,
    viewport: &AgentViewport,
    item: &UiFrameImageItem,
    images: &UiImageSourceTable,
    visual_time_millis: u64,
) -> Option<(AgentObservedObject, AgentStoredImageFrame)> {
    let source = images.get(item.image())?;
    let local_time_millis = source.playback().local_time_millis(visual_time_millis);
    let resolved = images
        .resolve_frame(item.image(), item.layout(), visual_time_millis)
        .ok()?;
    let frame = resolved.frame();
    let native_quad =
        arcweft_render_native::native_image_quad_from_resolved_frame(resolved).ok()?;
    let geometry = agent_image_geometry_from_native_quad(native_quad, viewport);
    let bbox = geometry.bbox;
    let polygon = geometry.polygon;
    let presentation = source.presentation();
    let semantic = item.semantic();
    let object_id = format!(
        "object.image.{}.{}.{}",
        agent_uri_component(item.layer().public_id().as_str()),
        item.node().0,
        item.image().0
    );
    let source_id = format!("ui.image.{}", item.image().0);
    let metadata = agent_image_observation_metadata(item, &source_id, presentation, semantic);
    let opacity_milli = source.opacity_milli();
    let fit = source.fit();
    let alignment = source.alignment();
    let transform = source.transform();
    let dimensions = source.image().dimensions();
    let frame_dimensions = frame.dimensions();
    Some((
        AgentObservedObject {
            id: object_id.clone(),
            parent_id: None,
            entity: Some(metadata.entity),
            layer: item.layer().public_id().as_str().to_owned(),
            role: "image".to_owned(),
            visible: semantic.is_none_or(arcweft_ui::UiSemanticNode::visible),
            enabled: semantic.is_none_or(arcweft_ui::UiSemanticNode::enabled),
            bbox: bbox.clone(),
            polygon,
            capture_refs: agent_object_capture_refs(session_id, step, &object_id, &bbox),
            object_layer: Some(metadata.object_layer),
            object_depth: metadata.object_depth,
            text: None,
            rich_text_ref: None,
            content: AgentObservedObjectContent::Image(Box::new(AgentObservedImageContent {
                source: source_id,
                object: presentation.map(|presentation| presentation.object().as_str().to_owned()),
                target: metadata.target,
                asset: presentation.map(|presentation| presentation.asset().as_str().to_owned()),
                frame_index: usize::try_from(frame.index()).ok(),
                local_time_millis: Some(local_time_millis),
                opacity_milli: Some(opacity_milli),
                fit: Some(agent_image_fit(fit)),
                alignment: Some(agent_image_alignment(alignment)),
                transform: Some(agent_image_transform(transform)),
                intrinsic_width: Some(dimensions.width()),
                intrinsic_height: Some(dimensions.height()),
                actions: metadata.actions,
                params: metadata.params,
                proxies: metadata.proxies,
            })),
        },
        AgentStoredImageFrame {
            width: frame_dimensions.width(),
            height: frame_dimensions.height(),
            rgba: frame.rgba().to_vec(),
            placement: Some(AgentStoredImagePlacement {
                dst: native_quad.dst,
                transform: native_quad.transform,
                opacity_milli: native_quad.opacity_milli,
            }),
        },
    ))
}

struct AgentImageObservationMetadata {
    entity: String,
    object_layer: String,
    object_depth: Option<i32>,
    target: Option<String>,
    actions: Vec<String>,
    params: BTreeMap<String, AgentImageObjectParam>,
    proxies: Vec<AgentPresentationObjectProxyRef>,
}

fn agent_image_observation_metadata(
    item: &UiFrameImageItem,
    source_id: &str,
    presentation: Option<&arcweft_ui::UiImagePresentationMetadata>,
    semantic: Option<&arcweft_ui::UiSemanticNode>,
) -> AgentImageObservationMetadata {
    AgentImageObservationMetadata {
        entity: presentation.map_or_else(
            || source_id.to_owned(),
            |presentation| presentation.object().as_str().to_owned(),
        ),
        object_layer: presentation.map_or_else(
            || item.layer().public_id().as_str().to_owned(),
            |presentation| presentation.layer().as_str().to_owned(),
        ),
        object_depth: presentation.map(arcweft_ui::UiImagePresentationMetadata::depth_milli),
        target: presentation
            .map(|presentation| presentation.target().as_str().to_owned())
            .or_else(|| semantic.map(|semantic| semantic.target().id().as_str().to_owned())),
        actions: agent_image_observation_actions(presentation, semantic),
        params: presentation
            .map(|presentation| {
                presentation
                    .params()
                    .iter()
                    .map(|(key, value)| {
                        (
                            key.as_str().to_owned(),
                            agent_image_object_param(value.clone()),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
        proxies: presentation
            .map(|presentation| {
                presentation
                    .proxies()
                    .iter()
                    .map(agent_image_object_proxy_ref)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

fn agent_image_observation_actions(
    presentation: Option<&arcweft_ui::UiImagePresentationMetadata>,
    semantic: Option<&arcweft_ui::UiSemanticNode>,
) -> Vec<String> {
    presentation.map_or_else(
        || {
            semantic
                .map(|semantic| {
                    semantic
                        .actions()
                        .iter()
                        .map(|action| action.as_str().to_owned())
                        .collect()
                })
                .unwrap_or_default()
        },
        |presentation| {
            presentation
                .actions()
                .iter()
                .map(|action| action.as_str().to_owned())
                .collect()
        },
    )
}

fn agent_image_object_param(value: ImageObjectParam) -> AgentImageObjectParam {
    match value {
        ImageObjectParam::Bool(value) => AgentImageObjectParam::Bool { value },
        ImageObjectParam::Integer(value) => AgentImageObjectParam::Integer { value },
        ImageObjectParam::Milli(value) => AgentImageObjectParam::Milli { value },
        ImageObjectParam::Text(value) => AgentImageObjectParam::Text { value },
        ImageObjectParam::Id(value) => AgentImageObjectParam::Id {
            value: value.as_str().to_owned(),
        },
    }
}

fn agent_image_object_proxy_ref(proxy: &ImageObjectProxy) -> AgentPresentationObjectProxyRef {
    AgentPresentationObjectProxyRef {
        id: proxy.id().as_str().to_owned(),
        type_name: proxy.type_name().map(str::to_owned),
        role: proxy.role().map(str::to_owned),
        layer: proxy.layer().map(|layer| layer.as_str().to_owned()),
        depth: proxy.depth_milli(),
        declaration: None,
        hit_test: proxy.hit_test(),
        params: proxy
            .params()
            .iter()
            .map(|(key, value)| {
                (
                    key.as_str().to_owned(),
                    agent_image_object_proxy_param(value.clone()),
                )
            })
            .collect(),
    }
}

fn agent_image_object_proxy_param(value: ImageObjectParam) -> RichTextParam {
    match value {
        ImageObjectParam::Bool(value) => RichTextParam::Bool { value },
        ImageObjectParam::Integer(value) => RichTextParam::Int { value },
        ImageObjectParam::Milli(value) => RichTextParam::Milli {
            value: Milli(value),
        },
        ImageObjectParam::Text(value) => RichTextParam::Text { value },
        ImageObjectParam::Id(value) => RichTextParam::Selector {
            value: value.as_str().to_owned(),
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct AgentImageGeometry {
    bbox: AgentBBox,
    polygon: Vec<AgentPoint>,
}

fn agent_image_geometry_from_native_quad(
    quad: arcweft_render_native::NativeImageQuad<'_>,
    viewport: &AgentViewport,
) -> AgentImageGeometry {
    let corners = [
        agent_transform_image_point(quad.transform, quad.dst.x, quad.dst.y),
        agent_transform_image_point(quad.transform, quad.dst.x + quad.dst.width, quad.dst.y),
        agent_transform_image_point(
            quad.transform,
            quad.dst.x + quad.dst.width,
            quad.dst.y + quad.dst.height,
        ),
        agent_transform_image_point(quad.transform, quad.dst.x, quad.dst.y + quad.dst.height),
    ];
    let polygon = corners
        .into_iter()
        .map(|(x, y)| agent_point_from_viewport_f32(x, y, viewport))
        .collect::<Vec<_>>();
    let min_x = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::INFINITY, f32::min);
    let min_y = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::INFINITY, f32::min);
    let max_x = corners
        .iter()
        .map(|(x, _)| *x)
        .fold(f32::NEG_INFINITY, f32::max);
    let max_y = corners
        .iter()
        .map(|(_, y)| *y)
        .fold(f32::NEG_INFINITY, f32::max);
    let x = agent_floor_viewport_f32(min_x, viewport.width);
    let y = agent_floor_viewport_f32(min_y, viewport.height);
    let right = agent_ceil_viewport_f32(max_x, viewport.width);
    let bottom = agent_ceil_viewport_f32(max_y, viewport.height);
    AgentImageGeometry {
        bbox: AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x,
            y,
            width: right.saturating_sub(x).max(1),
            height: bottom.saturating_sub(y).max(1),
        },
        polygon,
    }
}

fn agent_transform_image_point(
    transform: arcweft_render_native::NativeImageTransform,
    x: f32,
    y: f32,
) -> (f32, f32) {
    (
        transform
            .m11
            .mul_add(x, transform.m12.mul_add(y, transform.tx)),
        transform
            .m21
            .mul_add(x, transform.m22.mul_add(y, transform.ty)),
    )
}

fn agent_point_from_viewport_f32(x: f32, y: f32, viewport: &AgentViewport) -> AgentPoint {
    AgentPoint {
        x: agent_round_viewport_f32(x, viewport.width),
        y: agent_round_viewport_f32(y, viewport.height),
    }
}

fn agent_round_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.round(), viewport_extent)
}

fn agent_floor_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.floor(), viewport_extent)
}

fn agent_ceil_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    agent_clamp_viewport_f32(value.ceil(), viewport_extent)
}

fn agent_clamp_viewport_f32(value: f32, viewport_extent: u32) -> u32 {
    if !value.is_finite() {
        return 0;
    }
    let max = viewport_extent
        .max(1)
        .to_string()
        .parse::<f32>()
        .unwrap_or(f32::MAX);
    value.clamp(0.0, max).to_string().parse().unwrap_or(0)
}

fn agent_image_fit(fit: arcweft_ui::ImageFit) -> AgentImageFit {
    match fit {
        arcweft_ui::ImageFit::Contain => AgentImageFit::Contain,
        arcweft_ui::ImageFit::Cover => AgentImageFit::Cover,
        arcweft_ui::ImageFit::Stretch => AgentImageFit::Stretch,
        arcweft_ui::ImageFit::Intrinsic => AgentImageFit::Intrinsic,
    }
}

fn agent_image_alignment(alignment: arcweft_ui::ImageAlignment) -> AgentImageAlignment {
    AgentImageAlignment {
        x_milli: alignment.x_milli(),
        y_milli: alignment.y_milli(),
    }
}

fn agent_image_transform(
    transform: arcweft_presentation::image::ImageObjectTransform,
) -> AgentImageTransform {
    AgentImageTransform {
        m11_milli: transform.m11_milli,
        m12_milli: transform.m12_milli,
        m21_milli: transform.m21_milli,
        m22_milli: transform.m22_milli,
        tx_milli: transform.tx_milli,
        ty_milli: transform.ty_milli,
    }
}

fn agent_rich_text_child_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut children = Vec::new();
    let mut native_session = native_session;
    children.extend(agent_rich_text_page_objects(
        step,
        index,
        textbox,
        viewport,
        time_seconds,
        native_bounds,
        native_session.as_deref_mut(),
    ));
    children.extend(agent_rich_text_line_objects(
        step,
        index,
        textbox,
        viewport,
        time_seconds,
        native_bounds,
        native_session,
    ));
    let frame = agent_observed_rich_text(textbox);
    for (run_index, run) in frame.display_map.text_runs.iter().enumerate() {
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
        children.extend(agent_rich_text_proxy_objects(
            step,
            index,
            run_index,
            textbox,
            run,
            native_bounds,
        ));
    }
    for (ruby_index, ruby) in frame.display_map.ruby_annotations.iter().enumerate() {
        if let Some(object) =
            agent_rich_text_ruby_object(step, index, ruby_index, textbox, ruby, native_bounds)
        {
            children.push(object);
        }
    }
    children.extend(agent_rich_text_glyph_objects(
        step,
        index,
        textbox,
        native_bounds,
    ));
    children.extend(agent_rich_text_cluster_objects(
        step,
        index,
        textbox,
        native_bounds,
    ));
    agent_repair_rich_text_child_parent_ids(textbox, &mut children);
    children
}

fn agent_repair_rich_text_child_parent_ids(
    textbox: &AgentObservedObject,
    children: &mut [AgentObservedObject],
) {
    let mut valid_ids = children
        .iter()
        .map(|child| child.id.clone())
        .collect::<BTreeSet<_>>();
    valid_ids.insert(textbox.id.clone());
    for child in children {
        let is_valid = child
            .parent_id
            .as_ref()
            .is_some_and(|parent_id| valid_ids.contains(parent_id));
        if !is_valid {
            child.parent_id = Some(textbox.id.clone());
        }
    }
}

fn agent_rich_text_page_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut native_session = native_session;
    let frame = agent_observed_rich_text(textbox);
    agent_rich_text_page_ranges(frame)
        .into_iter()
        .enumerate()
        .filter_map(|(page_index, page_range)| {
            if page_range.is_empty() {
                return None;
            }
            let page_text = frame.text.get(page_range.clone())?;
            if page_text.trim().is_empty() {
                return None;
            }
            let bbox = agent_native_textbox_capture_bbox_for_page(
                textbox,
                viewport,
                page_index,
                time_seconds,
                native_session.as_deref_mut(),
            )?;
            let range = RichTextRange::new(page_range.start, page_range.end);
            let presentation = agent_rich_text_range_presentation(frame, range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextPage, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                frame,
                range,
                native_bounds,
            ));
            let object_id = agent_rich_text_page_object_id(step, index, page_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(textbox.id.clone()),
                    role: "rich_text_page",
                    text: page_text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextPage,
                        index: page_index,
                        page: page_index,
                        range,
                        node_index: agent_rich_text_page_node_index(frame, range),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(frame, range),
                        object_depth: agent_rich_text_page_object_depth(frame, range),
                        hit_test: true,
                        hit_regions,
                    },
                    page: page_index,
                },
            ))
        })
        .collect()
}

fn agent_rich_text_line_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Vec<AgentObservedObject> {
    let mut native_session = native_session;
    agent_rich_text_line_ranges(agent_observed_rich_text(textbox))
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, line_range)| {
            if line_range.is_empty() {
                return None;
            }
            let line_text = agent_observed_rich_text(textbox)
                .text
                .get(line_range.clone())?;
            if line_text.trim().is_empty() {
                return None;
            }
            let range = RichTextRange::new(line_range.start, line_range.end);
            let page = agent_rich_text_page_for_range(agent_observed_rich_text(textbox), range);
            let bbox = agent_native_text_range_capture_bbox_for_page(
                textbox,
                viewport,
                page,
                range,
                time_seconds,
                native_session.as_deref_mut(),
            )?;
            let presentation =
                agent_rich_text_range_presentation(agent_observed_rich_text(textbox), range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextLine, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                agent_observed_rich_text(textbox),
                range,
                native_bounds,
            ));
            let object_id = agent_rich_text_line_object_id(step, index, line_index);
            let parent_id = agent_rich_text_page_object_id(step, index, page);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
                    role: "rich_text_line",
                    text: line_text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextLine,
                        index: line_index,
                        page,
                        range,
                        node_index: agent_rich_text_page_node_index(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        object_depth: agent_rich_text_page_object_depth(
                            agent_observed_rich_text(textbox),
                            range,
                        ),
                        hit_test: true,
                        hit_regions,
                    },
                    page,
                },
            ))
        })
        .collect()
}

fn agent_measure_frame_elements_with_session(
    frame: &LineDisplayFrame,
    viewport: arcweft_render_native::NativeCaptureViewport,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<
    Vec<arcweft_render_native::NativeFrameElementBounds>,
    arcweft_render_native::NativeWindowError,
> {
    if let Some(native_session) = native_session {
        return native_session.measure_frame_elements_in(frame, viewport);
    }
    arcweft_render_native::measure_frame_elements_at_page_with_time(
        frame,
        viewport.width,
        viewport.height,
        viewport.left,
        viewport.top,
        viewport.page_index,
        viewport.time_seconds,
    )
}

fn agent_native_rich_text_element_bboxes(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> BTreeMap<arcweft_render_native::NativeFrameElement, AgentNativeRichTextElementBounds> {
    let (left, top) = agent_native_text_origin(textbox);
    let mut bboxes = BTreeMap::new();
    let mut native_session = native_session;
    for page_index in 0.. {
        let bounds = match agent_measure_frame_elements_with_session(
            agent_observed_rich_text(textbox),
            arcweft_render_native::NativeCaptureViewport::new(
                viewport.width,
                viewport.height,
                left,
                top,
                page_index,
            )
            .with_time_seconds(time_seconds),
            native_session.as_deref_mut(),
        ) {
            Ok(bounds) => bounds,
            Err(arcweft_render_native::NativeWindowError::EmptyPages) => break,
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

fn agent_native_textbox_capture_bbox_for_page(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    page_index: usize,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Option<AgentBBox> {
    let (left, top) = agent_native_text_origin(textbox);
    let Ok(bounds) = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            viewport.width,
            viewport.height,
            left,
            top,
            page_index,
        )
        .with_time_seconds(time_seconds),
        native_session,
    ) else {
        return None;
    };
    Some(
        bounds
            .into_iter()
            .fold(textbox.bbox.clone(), |bbox, bounds| {
                agent_union_bbox(&bbox, &agent_bbox_from_native(bounds.bbox))
            }),
    )
}

fn agent_native_text_range_capture_bbox_for_page(
    textbox: &AgentObservedObject,
    viewport: &AgentViewport,
    page_index: usize,
    range: RichTextRange,
    time_seconds: f32,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Option<AgentBBox> {
    let (left, top) = agent_native_text_origin(textbox);
    let bounds = agent_measure_frame_elements_with_session(
        agent_observed_rich_text(textbox),
        arcweft_render_native::NativeCaptureViewport::new(
            viewport.width,
            viewport.height,
            left,
            top,
            page_index,
        )
        .with_time_seconds(time_seconds),
        native_session,
    )
    .ok()?;
    bounds
        .into_iter()
        .filter(|bounds| {
            agent_native_element_overlaps_range(
                agent_observed_rich_text(textbox),
                bounds.element,
                range,
            )
        })
        .map(|bounds| agent_bbox_from_native(bounds.bbox))
        .reduce(|bbox, child| agent_union_bbox(&bbox, &child))
}

#[derive(Clone, Debug)]
struct AgentNativeRichTextElementBounds {
    bbox: AgentBBox,
    glyph: Option<arcweft_render_native::NativeGlyphClusterMetadata>,
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
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let text = frame
        .text
        .get(valid_rich_text_range(run.range, &frame.text)?)?;
    if text.trim().is_empty() {
        return None;
    }
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::TextRun { index: run_index })
        .map(|bounds| bounds.bbox.clone())?;
    let object_id = agent_rich_text_run_object_id(step, index, run_index);
    let page = agent_rich_text_page_for_range(frame, run.range);
    let parent_id = agent_rich_text_line_for_range(frame, run.range).map_or_else(
        || agent_rich_text_page_object_id(step, index, page),
        |line| agent_rich_text_line_object_id(step, index, line),
    );
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            parent_id: Some(parent_id),
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
                presentation: Some(run.presentation.clone()),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: None,
                ruby_annotation_bbox: None,
                object_layer: agent_object_layer(&run.presentation),
                object_depth: agent_object_depth(&run.presentation),
                hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                hit_regions: agent_text_hit_regions(
                    AgentHitRegionKind::TextRun,
                    &bbox,
                    run.range,
                    &run.presentation,
                ),
            },
            page,
        },
    ))
}

fn agent_rich_text_proxy_objects(
    step: usize,
    index: usize,
    run_index: usize,
    textbox: &AgentObservedObject,
    run: &RichTextTextRun,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let Some(range) = valid_rich_text_range(run.range, &frame.text) else {
        return Vec::new();
    };
    let Some(text) = frame.text.get(range) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let page = agent_rich_text_page_for_range(frame, run.range);
    run.presentation
        .object_proxies
        .iter()
        .enumerate()
        .filter_map(|(proxy_index, proxy)| {
            let object_id =
                format!("object.dialogue.{step}.{index}.proxy.{run_index}.{proxy_index}");
            let presentation = agent_proxy_presentation(&run.presentation, proxy);
            let bbox = native_bounds
                .get(
                    &arcweft_render_native::NativeFrameElement::TextObjectProxy {
                        run_index,
                        proxy_index,
                    },
                )
                .map(|bounds| bounds.bbox.clone())?;
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(agent_rich_text_run_object_id(step, index, run_index)),
                    role: "rich_text_proxy",
                    text: text.to_owned(),
                    bbox: &bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextObjectProxy,
                        index: proxy_index,
                        page,
                        range: run.range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        presentation: Some(presentation),
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: proxy
                            .layer
                            .clone()
                            .or_else(|| run.presentation.layer.clone()),
                        object_depth: proxy.depth.map(|depth| depth.0).or_else(|| {
                            (run.presentation.z_index != 0)
                                .then_some(i32::from(run.presentation.z_index) * 1000)
                        }),
                        hit_test: proxy.hit_test,
                        hit_regions: agent_proxy_hit_regions(
                            &bbox,
                            run.range,
                            &run.presentation,
                            proxy,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

fn agent_rich_text_ruby_object(
    step: usize,
    index: usize,
    ruby_index: usize,
    textbox: &AgentObservedObject,
    ruby: &RichTextRubyAnnotation,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Option<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    let base_range = valid_rich_text_range(ruby.base_range, &frame.text)?;
    let base_text = frame.text.get(base_range)?;
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::Ruby { index: ruby_index })
        .cloned()?;
    let object_id = format!("object.dialogue.{step}.{index}.ruby.{ruby_index}");
    let page = agent_rich_text_page_for_range(frame, ruby.base_range);
    let parent_id = agent_rich_text_line_for_range(frame, ruby.base_range).map_or_else(
        || agent_rich_text_page_object_id(step, index, page),
        |line| agent_rich_text_line_object_id(step, index, line),
    );
    let hit_regions = agent_ruby_hit_regions(&bbox, ruby.base_range);
    Some(agent_rich_text_child_object(
        step,
        textbox,
        AgentRichTextChildObjectSpec {
            object_id: &object_id,
            parent_id: Some(parent_id),
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
                presentation: Some(ruby.presentation.clone()),
                orientation: None,
                vertical_form: None,
                ruby_base_bbox: bbox.ruby.as_ref().map(|ruby| ruby.base_bbox.clone()),
                ruby_annotation_bbox: bbox.ruby.as_ref().map(|ruby| ruby.annotation_bbox.clone()),
                object_layer: agent_object_layer(&ruby.presentation),
                object_depth: agent_object_depth(&ruby.presentation),
                hit_test: agent_presentation_has_hit_test_proxy(&ruby.presentation),
                hit_regions,
            },
            page,
        },
    ))
}

fn agent_rich_text_glyph_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    native_bounds
        .iter()
        .filter_map(|(element, bounds)| {
            let arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: glyph_index,
                range_start,
                range_end,
            } = *element
            else {
                return None;
            };
            let range = RichTextRange::new(range_start, range_end);
            let text = frame.text.get(valid_rich_text_range(range, &frame.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = frame
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.glyph.{glyph_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(frame, range);
            let parent_id = agent_rich_text_run_object_id(step, index, run_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
                    role: "rich_text_glyph",
                    text: text.to_owned(),
                    bbox: &bounds.bbox,
                    rich_text_ref: AgentRichTextElementRef {
                        kind: AgentRichTextElementKind::TextGlyph,
                        index: glyph_index,
                        page,
                        range,
                        node_index: run.node_index,
                        source: Some(run.source),
                        ruby: None,
                        presentation: Some(run.presentation.clone()),
                        orientation: bounds
                            .glyph
                            .map(|glyph| agent_glyph_orientation_from_native(glyph.orientation)),
                        vertical_form: bounds.glyph.map(|glyph| {
                            agent_glyph_vertical_form_from_native(glyph.vertical_form)
                        }),
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_object_layer(&run.presentation),
                        object_depth: agent_object_depth(&run.presentation),
                        hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                        hit_regions: agent_text_hit_regions(
                            AgentHitRegionKind::TextGlyph,
                            &bounds.bbox,
                            range,
                            &run.presentation,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

fn agent_rich_text_cluster_objects(
    step: usize,
    index: usize,
    textbox: &AgentObservedObject,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentObservedObject> {
    let frame = agent_observed_rich_text(textbox);
    native_bounds
        .iter()
        .filter_map(|(element, bounds)| {
            let arcweft_render_native::NativeFrameElement::GlyphCluster {
                index: cluster_index,
                range_start,
                range_end,
            } = *element
            else {
                return None;
            };
            let range = RichTextRange::new(range_start, range_end);
            let text = frame.text.get(valid_rich_text_range(range, &frame.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = frame
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.cluster.{cluster_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(frame, range);
            let parent_id = agent_rich_text_run_object_id(step, index, run_index);
            Some(agent_rich_text_child_object(
                step,
                textbox,
                AgentRichTextChildObjectSpec {
                    object_id: &object_id,
                    parent_id: Some(parent_id),
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
                        presentation: Some(run.presentation.clone()),
                        orientation: bounds
                            .glyph
                            .map(|glyph| agent_glyph_orientation_from_native(glyph.orientation)),
                        vertical_form: bounds.glyph.map(|glyph| {
                            agent_glyph_vertical_form_from_native(glyph.vertical_form)
                        }),
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_object_layer(&run.presentation),
                        object_depth: agent_object_depth(&run.presentation),
                        hit_test: agent_presentation_has_hit_test_proxy(&run.presentation),
                        hit_regions: agent_text_hit_regions(
                            AgentHitRegionKind::GlyphCluster,
                            &bounds.bbox,
                            range,
                            &run.presentation,
                        ),
                    },
                    page,
                },
            ))
        })
        .collect()
}

fn agent_bbox_from_native(bbox: arcweft_render_native::NativeFrameContentBBox) -> AgentBBox {
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
        proxy_id: None,
        proxy_type: None,
        proxy_declaration: None,
        proxy_role: None,
        proxy_layer: None,
        depth: None,
        proxy_params: BTreeMap::new(),
    }
}

fn agent_text_hit_regions(
    base_kind: AgentHitRegionKind,
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
) -> Vec<AgentHitRegion> {
    let mut regions = vec![agent_hit_region(base_kind, bbox, range)];
    regions.extend(
        presentation
            .object_proxies
            .iter()
            .filter(|proxy| proxy.hit_test)
            .map(|proxy| agent_proxy_hit_region(bbox, range, presentation, proxy)),
    );
    regions
}

fn agent_proxy_presentation(
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> RichTextPresentation {
    let mut proxy_presentation = presentation.clone();
    proxy_presentation.object_proxies = vec![proxy.clone()];
    proxy_presentation
}

fn agent_proxy_hit_regions(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> Vec<AgentHitRegion> {
    proxy
        .hit_test
        .then(|| agent_proxy_hit_region(bbox, range, presentation, proxy))
        .into_iter()
        .collect()
}

fn agent_proxy_hit_region(
    bbox: &AgentBBox,
    range: RichTextRange,
    presentation: &RichTextPresentation,
    proxy: &RichTextObjectProxy,
) -> AgentHitRegion {
    AgentHitRegion {
        kind: AgentHitRegionKind::TextObjectProxy,
        bbox: bbox.clone(),
        range,
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy.layer.clone().or_else(|| presentation.layer.clone()),
        depth: proxy.depth.map(|depth| depth.0),
        proxy_params: proxy.params.clone(),
    }
}

fn agent_object_layer(presentation: &RichTextPresentation) -> Option<String> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| {
            proxy
                .layer
                .as_ref()
                .map(|layer| (proxy.depth.map_or(0, |depth| depth.0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer.clone())
        .or_else(|| presentation.layer.clone())
}

fn agent_object_depth(presentation: &RichTextPresentation) -> Option<i32> {
    presentation
        .object_proxies
        .iter()
        .filter_map(|proxy| proxy.depth.map(|depth| depth.0))
        .max()
        .or_else(|| (presentation.z_index != 0).then_some(i32::from(presentation.z_index) * 1000))
}

fn agent_presentation_has_hit_test_proxy(presentation: &RichTextPresentation) -> bool {
    presentation
        .object_proxies
        .iter()
        .any(|proxy| proxy.hit_test)
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
    value: arcweft_render_native::NativeRubyElementGeometry,
) -> AgentRubyElementGeometry {
    AgentRubyElementGeometry {
        base_bbox: agent_bbox_from_native(value.base_bbox),
        annotation_bbox: agent_bbox_from_native(value.annotation_bbox),
    }
}

const fn agent_glyph_orientation_from_native(
    value: arcweft_render_native::NativeGlyphOrientation,
) -> AgentGlyphOrientation {
    match value {
        arcweft_render_native::NativeGlyphOrientation::Upright => AgentGlyphOrientation::Upright,
        arcweft_render_native::NativeGlyphOrientation::SidewaysCw => {
            AgentGlyphOrientation::SidewaysCw
        }
        arcweft_render_native::NativeGlyphOrientation::TextCombineUpright => {
            AgentGlyphOrientation::TextCombineUpright
        }
    }
}

const fn agent_glyph_vertical_form_from_native(
    value: arcweft_render_native::NativeGlyphVerticalForm,
) -> AgentGlyphVerticalForm {
    match value {
        arcweft_render_native::NativeGlyphVerticalForm::None => AgentGlyphVerticalForm::None,
        arcweft_render_native::NativeGlyphVerticalForm::UprightAlternate => {
            AgentGlyphVerticalForm::UprightAlternate
        }
        arcweft_render_native::NativeGlyphVerticalForm::RotatedAlternate => {
            AgentGlyphVerticalForm::RotatedAlternate
        }
    }
}

struct AgentRichTextChildObjectSpec<'a> {
    object_id: &'a str,
    parent_id: Option<String>,
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
        parent_id: spec.parent_id.or_else(|| Some(textbox.id.clone())),
        entity: textbox.entity.clone(),
        layer: "dialogue.rich_text".to_owned(),
        role: spec.role.to_owned(),
        visible: textbox.visible,
        enabled: textbox.enabled,
        bbox: spec.bbox.clone(),
        polygon: spec.bbox.polygon(),
        capture_refs: agent_object_capture_refs_for_page(
            "cli",
            step,
            spec.object_id,
            spec.bbox,
            spec.page,
        ),
        object_layer: spec.rich_text_ref.object_layer.clone(),
        object_depth: spec.rich_text_ref.object_depth,
        text: Some(spec.text.clone()),
        rich_text_ref: Some(spec.rich_text_ref),
        content: AgentObservedObjectContent::RichText {
            frame: Box::new(agent_child_line_display_frame(
                agent_observed_rich_text(textbox),
                spec.text,
            )),
        },
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

fn agent_rich_text_line_for_range(frame: &LineDisplayFrame, range: RichTextRange) -> Option<usize> {
    let valid_range = valid_rich_text_range(range, &frame.text)?;
    agent_rich_text_line_ranges(frame)
        .into_iter()
        .filter(|line_range| !line_range.is_empty())
        .position(|line_range| {
            valid_range.start >= line_range.start && valid_range.end <= line_range.end
        })
}

fn agent_rich_text_page_object_id(step: usize, index: usize, page: usize) -> String {
    format!("object.dialogue.{step}.{index}.page.{page}")
}

fn agent_rich_text_line_object_id(step: usize, index: usize, line: usize) -> String {
    format!("object.dialogue.{step}.{index}.line.{line}")
}

fn agent_rich_text_run_object_id(step: usize, index: usize, run: usize) -> String {
    format!("object.dialogue.{step}.{index}.run.{run}")
}

fn agent_rich_text_page_node_index(frame: &LineDisplayFrame, range: RichTextRange) -> usize {
    frame
        .display_map
        .text_runs
        .iter()
        .find(|run| agent_rich_text_ranges_overlap(run.range, range))
        .map_or(0, |run| run.node_index)
}

fn agent_rich_text_range_presentation(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<RichTextPresentation> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .map(|run| run.presentation.clone())
        .reduce(|mut accumulated, presentation| {
            accumulated.merge(presentation);
            accumulated
        })
}

fn agent_rich_text_page_object_depth(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<i32> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .filter_map(|run| agent_object_depth(&run.presentation))
        .max()
}

fn agent_rich_text_range_object_layer(
    frame: &LineDisplayFrame,
    range: RichTextRange,
) -> Option<String> {
    frame
        .display_map
        .text_runs
        .iter()
        .filter(|run| agent_rich_text_ranges_overlap(run.range, range))
        .filter_map(|run| {
            agent_object_layer(&run.presentation)
                .map(|layer| (agent_object_depth(&run.presentation).unwrap_or(0), layer))
        })
        .max_by_key(|(depth, _)| *depth)
        .map(|(_, layer)| layer)
}

fn agent_rich_text_range_proxy_hit_regions(
    frame: &LineDisplayFrame,
    range: RichTextRange,
    native_bounds: &BTreeMap<
        arcweft_render_native::NativeFrameElement,
        AgentNativeRichTextElementBounds,
    >,
) -> Vec<AgentHitRegion> {
    frame
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter(|(_, run)| agent_rich_text_ranges_overlap(run.range, range))
        .flat_map(|(run_index, run)| {
            let hit_range = RichTextRange::new(
                run.range.start.max(range.start),
                run.range.end.min(range.end),
            );
            run.presentation
                .object_proxies
                .iter()
                .enumerate()
                .filter(|(_, proxy)| proxy.hit_test)
                .filter_map(move |(proxy_index, proxy)| {
                    native_bounds
                        .get(
                            &arcweft_render_native::NativeFrameElement::TextObjectProxy {
                                run_index,
                                proxy_index,
                            },
                        )
                        .map(|bounds| {
                            agent_proxy_hit_region(
                                &bounds.bbox,
                                hit_range,
                                &run.presentation,
                                proxy,
                            )
                        })
                })
        })
        .collect()
}

fn agent_rich_text_ranges_overlap(left: RichTextRange, right: RichTextRange) -> bool {
    left.start < right.end && right.start < left.end
}

fn agent_native_element_overlaps_range(
    frame: &LineDisplayFrame,
    element: arcweft_render_native::NativeFrameElement,
    range: RichTextRange,
) -> bool {
    match element {
        arcweft_render_native::NativeFrameElement::TextRun { index } => frame
            .display_map
            .text_runs
            .get(index)
            .is_some_and(|run| agent_rich_text_ranges_overlap(run.range, range)),
        arcweft_render_native::NativeFrameElement::Ruby { index } => frame
            .display_map
            .ruby_annotations
            .get(index)
            .is_some_and(|ruby| agent_rich_text_ranges_overlap(ruby.base_range, range)),
        arcweft_render_native::NativeFrameElement::TextObjectProxy { run_index, .. } => frame
            .display_map
            .text_runs
            .get(run_index)
            .is_some_and(|run| agent_rich_text_ranges_overlap(run.range, range)),
        arcweft_render_native::NativeFrameElement::GlyphCluster {
            range_start,
            range_end,
            ..
        } => agent_rich_text_ranges_overlap(RichTextRange::new(range_start, range_end), range),
    }
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
        .map(|offset| agent_display_map_offset_after_atomic_ruby_base(frame, offset))
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

fn agent_rich_text_line_ranges(frame: &LineDisplayFrame) -> Vec<std::ops::Range<usize>> {
    let mut break_offsets = frame
        .display_map
        .controls
        .iter()
        .filter(|marker| {
            matches!(
                marker.control,
                RichTextControl::HardBreak
                    | RichTextControl::Page
                    | RichTextControl::LineWait
                    | RichTextControl::Clear
            )
        })
        .map(|marker| agent_display_map_line_break_offset(frame, marker))
        .map(|offset| agent_display_map_offset_after_atomic_ruby_base(frame, offset))
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

fn agent_display_map_line_break_offset(
    frame: &LineDisplayFrame,
    marker: &arcweft_render_text::RichTextControlMarker,
) -> usize {
    match marker.control {
        RichTextControl::HardBreak => marker.range.map_or_else(
            || agent_display_map_offset_before_node(frame, marker.node_index),
            |range| range.end,
        ),
        _ => agent_display_map_offset_before_node(frame, marker.node_index),
    }
}

fn agent_display_map_offset_after_atomic_ruby_base(
    frame: &LineDisplayFrame,
    offset: usize,
) -> usize {
    let mut adjusted = offset;
    loop {
        let Some(range) = frame
            .display_map
            .ruby_annotations
            .iter()
            .filter_map(|annotation| valid_rich_text_range(annotation.base_range, &frame.text))
            .find(|range| range.start < adjusted && adjusted < range.end)
        else {
            return adjusted;
        };
        adjusted = range.end;
    }
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
        style_contributions: parent.style_contributions.clone(),
        nodes: vec![RichTextNode::Text { text }],
        display_map: arcweft_render_text::RichTextDisplayMap::default(),
        host_events: Vec::new(),
        inline_failures: Vec::new(),
        unresolved: Vec::new(),
    }
}

fn agent_object_layers(object: &AgentObservedObject) -> Vec<String> {
    let mut layers = vec![object.layer.clone()];
    if let Some(object_layer) = object
        .resolved_object_layer()
        .as_ref()
        .filter(|object_layer| *object_layer != &object.layer)
    {
        layers.push(object_layer.clone());
    }
    layers
}

fn agent_object_matches_layer(object: &AgentObservedObject, layer: &str) -> bool {
    object.layer == layer
        || object
            .resolved_object_layer()
            .as_ref()
            .is_some_and(|object_layer| object_layer == layer)
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
        for object_layer in agent_object_layers(object) {
            layers
                .entry(object_layer)
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
