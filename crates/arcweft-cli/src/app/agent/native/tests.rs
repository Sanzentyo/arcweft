use super::capture::{
    AgentNativeCaptureContext, AgentNativeCaptureTarget, agent_native_debug_capture,
    agent_native_image_object_geometry_capture, agent_native_masked_framebuffer_capture,
};
use super::image_mapping::{
    agent_image_objects_from_ui_frame, agent_image_observation_from_ui_frame,
};
use super::mcp_protocol::{
    agent_mcp_call_get_state, agent_mcp_call_log_query, agent_mcp_call_session_info,
    agent_mcp_call_signal_get, agent_mcp_resource_list, agent_mcp_resource_read,
    agent_mcp_script_run_options,
};
use super::mcp_rag::{AgentMcpRagCandidate, agent_mcp_rag_context_pack_from_candidates};
use super::mcp_resources::agent_mcp_capture_time_seconds;
use super::observe::{
    NativeAgentScriptSessionError, native_agent_advance_text_input_events,
    native_agent_invoke_input_events, native_runtime_input_event,
    validate_agent_observe_output_extension,
};
use super::repl::{
    AgentReplBinding, AgentReplConnection, AgentReplState, agent_repl_apply_connection,
    agent_repl_parse_connection,
};
#[cfg(feature = "agent-repl")]
use super::repl::{AgentReplReedlineCompleter, AgentReplReedlineValidator};
use super::repl_project_binding::agent_repl_reconcile_project_bound_bindings;
use super::repl_snapshot::agent_repl_serialized_bindings;
use super::runtime_observation::agent_observe_layout_scene_graph;
use super::*;
use arcweft_agent_protocol::protocol::{AgentProjectGraph, AgentSessionInfo};
use arcweft_agent_protocol::{
    presentation::AgentPresentationTree, session::AgentAudioState, ui::AgentUiTree,
};
use arcweft_debug_model::{
    diagnostic::DebugDiagnostic,
    graph::{DebugGraphEdge, DebugGraphSymbol},
    history::DebugHistoryEntry,
    script::DebugScriptRunOutcome,
    test_result::DebugTestResult,
};
use arcweft_render_text::{RichTextNode, RuntimeLineContext};
use serde::Serialize;

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
        Some(crate::app::runtime::options::CliRuntimePureBackend::Jit)
    );
    assert!(matches!(
        options.pure_workers,
        Some(CliRuntimePureWorkers::Fixed(2))
    ));
    assert_eq!(options.pure_batch_min_len, Some(4));
    assert!(options.pure_object_artifacts);
    assert_eq!(
        options.math_backend,
        Some(crate::app::runtime::options::CliRuntimeMathBackend::Ndarray)
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

#[test]
fn agent_observe_output_extension_must_match_image_mime() {
    assert!(
        validate_agent_observe_output_extension(
            std::path::Path::new("frame.svg"),
            AgentObserveImageKind::Overlay,
        )
        .is_ok()
    );
    assert!(
        validate_agent_observe_output_extension(
            std::path::Path::new("frame.PNG"),
            AgentObserveImageKind::Png,
        )
        .is_ok()
    );
    assert!(
        validate_agent_observe_output_extension(
            std::path::Path::new("frame.rgba"),
            AgentObserveImageKind::RawRgba,
        )
        .is_ok()
    );
    assert!(
        validate_agent_observe_output_extension(
            std::path::Path::new("frame.png"),
            AgentObserveImageKind::Overlay,
        )
        .is_err()
    );
}

#[test]
fn agent_observe_layout_scene_graph_records_raw_content_rect() {
    let metadata = agent_observe_layout_scene_graph(&AgentViewport {
        width: 1000,
        height: 800,
        scale: 1.0,
    });

    assert_eq!(metadata["kind"], serde_json::json!("layout.viewport_scale"));
    assert_eq!(
        metadata["renderer_kind"],
        serde_json::json!("native_rich_text_observer")
    );
    assert_eq!(metadata["scale_policy"], serde_json::json!("raw"));
    assert_eq!(metadata["raw_pixel_mode"], serde_json::json!(true));
    assert_eq!(
        metadata["output_viewport"]["width"],
        serde_json::json!(1000)
    );
    assert_eq!(
        metadata["output_viewport"]["height"],
        serde_json::json!(800)
    );
    assert_eq!(
        metadata["design_viewport"]["width"],
        serde_json::json!(1280)
    );
    assert_eq!(
        metadata["design_viewport"]["height"],
        serde_json::json!(720)
    );
    assert_eq!(metadata["content_rect"]["x"], serde_json::json!(0.0));
    assert_eq!(metadata["content_rect"]["y"], serde_json::json!(0.0));
    assert_eq!(metadata["content_rect"]["width"], serde_json::json!(1280.0));
    assert_eq!(metadata["content_rect"]["height"], serde_json::json!(720.0));
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
    assert_eq!(events, vec![native_runtime_input_event("advance", None)]);
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
        vec![native_runtime_input_event(
            "invoke",
            Some("action.inspect.pulse"),
        )]
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
                arcweft_agent_protocol::resource::AgentBinaryResourceBody {
                    encoding: AgentBinaryEncoding::Base64,
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

    assert_listed_moderated_uri_reads(&mut state);

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
    assert_missing_image_metadata_tool_result(&allowed_result);
    assert_resource_read_audit(&db_path);

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
    let moderated_uri = direct_allowed["contents"][0]["uri"]
        .as_str()
        .expect("moderated resource uri");
    assert!(moderated_uri.starts_with("arcweft://moderated/"));
    assert_missing_image_metadata_read(&direct_allowed);
    let moderated_read = agent_mcp_resource_read(
        &serde_json::json!({
            "uri": moderated_uri,
            "max_privacy": "sensitive"
        }),
        &mut state,
    )
    .expect("raw read caches moderated URI for follow-up read");
    assert_eq!(
        moderated_read["contents"][0]["uri"],
        serde_json::json!(moderated_uri)
    );
    let _ = std::fs::remove_file(&db_path);
}

#[test]
fn agent_mcp_strict_mode_withholds_color_capture_without_visual_classifier() {
    let mut state = AgentMcpState {
        report: Some(test_agent_observation_report(None)),
        capture_resources: vec![test_agent_raw_rgba_capture_resource(AgentImageKind::Color)],
        ..AgentMcpState::default()
    };

    let result = agent_mcp_call_resource_read(
        &serde_json::json!({
            "uri": "arcweft://session/cli/frame/0/object.customer.secret.color.rgba",
            "max_privacy": "sensitive"
        }),
        &mut state,
    )
    .expect("strict mode returns a policy placeholder");

    assert!(!result.is_error);
    let metadata = mcp_tool_metadata_json(&result);
    assert_eq!(
        metadata["content_policy"]["disposition"],
        serde_json::json!("review")
    );
    assert!(
        metadata["content_policy"]["reason_codes"]
            .as_array()
            .expect("reason codes")
            .contains(&serde_json::json!("classifier_no_applicable_run"))
    );
    let body = mcp_tool_resource_text_json(&result);
    assert_eq!(
        body["content_policy"]["disposition"],
        serde_json::json!("review")
    );
}

#[test]
fn agent_mcp_local_dev_mode_allows_color_capture_and_scrubs_metadata() {
    let mut state = AgentMcpState {
        content_policy_mode: AgentContentPolicyMode::LocalDev,
        report: Some(test_agent_observation_report(None)),
        capture_resources: vec![test_agent_raw_rgba_capture_resource(AgentImageKind::Color)],
        ..AgentMcpState::default()
    };

    let result = agent_mcp_call_resource_read(
        &serde_json::json!({
            "uri": "arcweft://session/cli/frame/0/object.customer.secret.color.rgba",
            "max_privacy": "sensitive"
        }),
        &mut state,
    )
    .expect("local-dev mode publishes color capture");

    assert!(!result.is_error);
    let metadata = mcp_tool_metadata_json(&result);
    assert_eq!(
        metadata["content_policy"]["disposition"],
        serde_json::json!("allow")
    );
    assert_eq!(metadata["image"]["kind"], serde_json::json!("color"));
    assert_eq!(
        metadata["image"]["scope"]["kind"],
        serde_json::json!("object")
    );
    assert!(
        metadata["image"]["scope"]["id"]
            .as_str()
            .expect("opaque object scope id")
            .starts_with("object.")
    );
    assert!(
        !serde_json::to_string(&metadata)
            .expect("metadata serializes")
            .contains("customer.secret")
    );
    let resource = mcp_tool_resource_value(&result);
    assert!(
        resource["resource"]["uri"]
            .as_str()
            .expect("published URI")
            .starts_with("arcweft://moderated/")
    );
    assert_eq!(
        resource["resource"]["mimeType"],
        serde_json::json!("application/octet-stream")
    );
    assert_eq!(resource["resource"]["blob"], serde_json::json!("IECA/w=="));
}

#[test]
fn agent_mcp_local_dev_mode_still_withholds_auxiliary_captures() {
    let mut state = AgentMcpState {
        content_policy_mode: AgentContentPolicyMode::LocalDev,
        report: Some(test_agent_observation_report(None)),
        capture_resources: vec![test_agent_raw_rgba_capture_resource(
            AgentImageKind::ObjectId,
        )],
        ..AgentMcpState::default()
    };

    let result = agent_mcp_call_resource_read(
        &serde_json::json!({
            "uri": "arcweft://session/cli/frame/0/object.customer.secret.object_id.rgba",
            "max_privacy": "sensitive"
        }),
        &mut state,
    )
    .expect("local-dev mode returns a policy placeholder for auxiliary captures");

    assert!(!result.is_error);
    let metadata = mcp_tool_metadata_json(&result);
    assert!(
        metadata["content_policy"]["reason_codes"]
            .as_array()
            .expect("reason codes")
            .contains(&serde_json::json!("auxiliary_capture_not_publishable"))
    );
    let body = mcp_tool_resource_text_json(&result);
    assert_eq!(
        body["content_policy"]["code"],
        serde_json::json!("auxiliary_capture_not_publishable")
    );
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
        project_context: Some(test_agent_mcp_project_context()),
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
    assert_eq!(
        body["project"]["project_graph"]["summary_symbol_id"],
        serde_json::json!("project:summary")
    );
    assert_eq!(
        body["project"]["project_graph"]["project_summary"]["entity_count"],
        serde_json::json!(2)
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

    let session_info = agent_mcp_call_session_info(&mut state).expect("session info reads");
    let info = mcp_text_json(&session_info);
    assert_eq!(
        info["project"]["project_graph"]["project_summary"]["agent_action_count"],
        serde_json::json!(1)
    );
}

fn test_agent_mcp_project_context() -> AgentMcpProjectContext {
    AgentMcpProjectContext {
        project_entities: serde_json::json!({
            "count": 2,
            "kind_counts": {
                "flow": 1,
                "choice_option": 1,
            },
        }),
        project_graph: serde_json::json!({
            "symbol_count": 3,
            "edge_count": 2,
            "summary_symbol_id": "project:summary",
            "has_project_summary": true,
            "project_summary": {
                "entity_count": 2,
                "agent_action_count": 1,
                "project_callable_count": 0,
                "relation_count": 1,
                "dependency_edge_count": 0,
                "dynamic_control_flow_count": 0,
                "debug_query_count": 0,
            },
            "symbol_kind_counts": {
                "project_summary": 1,
                "flow": 1,
                "choice_option": 1,
            },
            "edge_kind_counts": {
                "contains_entity": 2,
            },
        }),
    }
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
fn agent_repl_parse_stdio_connection_preserves_program_and_args() {
    let options = test_repl_options();
    let connection =
        agent_repl_parse_connection("stdio:arcw agent mcp", &options).expect("connection");

    assert!(matches!(
        connection,
        Some(AgentReplConnection::StdioMcp { program, args })
            if program == "arcw" && args == ["agent", "mcp"]
    ));
}

#[test]
fn agent_repl_parse_stdio_connection_rejects_shell_syntax() {
    let options = test_repl_options();
    let error = agent_repl_parse_connection("stdio:arcw agent mcp | tee out", &options)
        .expect_err("shell metacharacters are rejected");

    assert!(error.contains("shell metacharacter"));
}

#[test]
fn agent_repl_project_hash_change_preserves_only_literal_bindings() {
    let mut state = AgentReplState::default();
    state.bindings.insert(
        "count".to_owned(),
        test_repl_binding("count", "local", true, Some("literal")),
    );
    state.bindings.insert(
        "hero".to_owned(),
        test_repl_binding("hero", "local", true, Some("project_ref")),
    );
    state.bindings.insert(
        "screen".to_owned(),
        test_repl_binding("screen", "local", true, Some("observation")),
    );
    state.bindings.insert(
        "doc".to_owned(),
        test_repl_binding("doc", "local", true, Some("resource")),
    );
    state.bindings.insert(
        "ctx".to_owned(),
        test_repl_binding("ctx", "local", true, Some("rag_context")),
    );
    state.bindings.insert(
        "tmp".to_owned(),
        test_repl_binding("tmp", "cell", false, None),
    );
    state.bindings.insert(
        "loaded".to_owned(),
        test_repl_binding("loaded", "loaded_agent", false, None),
    );

    let decisions = agent_repl_reconcile_project_bound_bindings(
        &mut state,
        Some("program.old"),
        Some("program.new"),
    );

    assert!(state.bindings.contains_key("count"));
    assert!(!state.bindings.contains_key("hero"));
    assert!(!state.bindings.contains_key("screen"));
    assert!(!state.bindings.contains_key("doc"));
    assert!(!state.bindings.contains_key("ctx"));
    assert!(!state.bindings.contains_key("tmp"));
    assert!(!state.bindings.contains_key("loaded"));
    assert_eq!(decisions.len(), 7);
    assert!(decisions.iter().any(|decision| {
        decision.name == "count"
            && decision.decision == "preserved"
            && decision.old_program_hash == "program.old"
            && decision.new_program_hash == "program.new"
    }));
    assert!(decisions.iter().any(|decision| {
        decision.name == "screen"
            && decision.decision == "dropped"
            && decision.reason == "local binding snapshot is project-bound or session-derived"
    }));
    assert!(decisions.iter().any(|decision| {
        decision.name == "tmp"
            && decision.decision == "dropped"
            && decision.reason == "cell artifact belongs to the previous program hash"
    }));
}

#[test]
fn agent_repl_project_hash_unchanged_keeps_bindings_without_decisions() {
    let mut state = AgentReplState::default();
    state.bindings.insert(
        "screen".to_owned(),
        test_repl_binding("screen", "local", true, Some("observation")),
    );

    let decisions = agent_repl_reconcile_project_bound_bindings(
        &mut state,
        Some("program.same"),
        Some("program.same"),
    );

    assert!(decisions.is_empty());
    assert!(state.bindings.contains_key("screen"));
}

#[test]
fn agent_repl_stdio_connect_reports_project_hash_binding_policy() {
    let mut state = AgentReplState::default();
    let first = agent_repl_apply_connection(
        1,
        ":connect first",
        Some(fake_repl_mcp_connection("program.old")),
        &mut state,
    );
    assert_eq!(first.status, "ok");
    assert_eq!(state.remote_program_hash.as_deref(), Some("program.old"));

    state.bindings.insert(
        "answer".to_owned(),
        test_repl_binding("answer", "local", true, Some("literal")),
    );
    state.bindings.insert(
        "frame".to_owned(),
        test_repl_binding("frame", "local", true, Some("observation")),
    );

    let second = agent_repl_apply_connection(
        2,
        ":connect second",
        Some(fake_repl_mcp_connection("program.new")),
        &mut state,
    );

    assert_eq!(second.status, "ok");
    assert_eq!(state.remote_program_hash.as_deref(), Some("program.new"));
    assert!(state.bindings.contains_key("answer"));
    assert!(!state.bindings.contains_key("frame"));
    let value = second.value.expect("connect report value");
    assert_eq!(
        value["binding_policy"]["program_hash_changed"],
        serde_json::json!(true)
    );
    assert_eq!(
        value["binding_policy"]["old_program_hash"],
        serde_json::json!("program.old")
    );
    assert_eq!(
        value["binding_policy"]["new_program_hash"],
        serde_json::json!("program.new")
    );
    let decisions = value["binding_policy"]["decisions"]
        .as_array()
        .expect("binding decisions are reported");
    assert!(
        decisions.iter().any(|decision| {
            decision["name"] == "answer" && decision["decision"] == "preserved"
        })
    );
    assert!(
        decisions
            .iter()
            .any(|decision| { decision["name"] == "frame" && decision["decision"] == "dropped" })
    );
}

#[test]
fn agent_repl_serialized_bindings_separate_literals_from_project_refs() {
    let count = agent_repl_serialized_bindings(&agent_repl_parse_fragment("let count = [1, 2, 3]"));
    let hero =
        agent_repl_serialized_bindings(&agent_repl_parse_fragment("let hero = @character.alice"));
    let party = agent_repl_serialized_bindings(&agent_repl_parse_fragment(
        "let party = [@character.alice, \"bob\"]",
    ));

    assert_eq!(
        count
            .get("count")
            .map(|binding| binding.snapshot_kind.as_str()),
        Some("literal")
    );
    assert_eq!(
        hero.get("hero")
            .map(|binding| binding.snapshot_kind.as_str()),
        Some("project_ref")
    );
    assert_eq!(
        party
            .get("party")
            .map(|binding| binding.snapshot_kind.as_str()),
        Some("project_ref")
    );
}

fn test_repl_binding(
    name: &str,
    binding_kind: &str,
    serializable: bool,
    snapshot_kind: Option<&str>,
) -> AgentReplBinding {
    AgentReplBinding {
        name: name.to_owned(),
        binding_kind: binding_kind.to_owned(),
        source: "let value = 1".to_owned(),
        status: "ok".to_owned(),
        final_status: None,
        host_calls: 0,
        responses: 0,
        serializable,
        serialized_source: serializable.then(|| "1".to_owned()),
        snapshot_kind: snapshot_kind.map(str::to_owned),
        non_serializable_reason: (!serializable)
            .then(|| "test binding is intentionally not serializable".to_owned()),
    }
}

fn fake_repl_mcp_connection(program_hash: &str) -> AgentReplConnection {
    let info = AgentSessionInfo {
        session_id: format!("session.{program_hash}"),
        program_hash: program_hash.to_owned(),
        project_entities: Vec::new(),
        project_graph: AgentProjectGraph::default(),
        profile: Some("fake".to_owned()),
        capabilities: Vec::new(),
    };
    fake_repl_mcp_connection_with_responses(vec![
        rpc_result(
            1,
            &serde_json::json!({
                "protocolVersion": "2024-11-05",
                "serverInfo": { "name": "fake-repl-child" }
            }),
        ),
        rpc_result(2, &serde_json::json!({ "tools": required_mcp_tools() })),
        rpc_result(3, &tool_result(&info)),
    ])
}

fn required_mcp_tools() -> Vec<serde_json::Value> {
    [
        "arcweft.session.info",
        "arcweft.observe",
        "arcweft.action",
        "arcweft.capture",
        "arcweft.resource.read",
        "arcweft.session.step_frames",
    ]
    .into_iter()
    .map(|name| {
        serde_json::json!({
            "name": name,
            "title": null,
            "description": "",
            "inputSchema": { "type": "object" }
        })
    })
    .collect()
}

fn tool_result(value: &impl Serialize) -> serde_json::Value {
    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string(value).expect("serializes")
        }],
        "isError": false
    })
}

fn rpc_result(id: u64, result: &serde_json::Value) -> String {
    serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
    .to_string()
}

#[cfg(windows)]
fn fake_repl_mcp_connection_with_responses(responses: Vec<String>) -> AgentReplConnection {
    let cases = responses
        .into_iter()
        .enumerate()
        .map(|(index, response)| {
            format!(
                "{} {{ [Console]::Out.WriteLine('{}') }}",
                index + 1,
                response.replace('\'', "''")
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    AgentReplConnection::StdioMcp {
        program: "powershell".to_owned(),
        args: vec![
            "-NoProfile".to_owned(),
            "-Command".to_owned(),
            format!(
                "$i=0; while (($line=[Console]::In.ReadLine()) -ne $null) {{ $i++; switch ($i) {{ {cases} default {{ exit 0 }} }} [Console]::Out.Flush() }}"
            ),
        ],
    }
}

#[cfg(not(windows))]
fn fake_repl_mcp_connection_with_responses(responses: Vec<String>) -> AgentReplConnection {
    let cases = responses
        .into_iter()
        .enumerate()
        .map(|(index, response)| {
            format!("{}) printf '%s\\n' '{}' ;;", index + 1, sh_quote(&response))
        })
        .collect::<Vec<_>>()
        .join(" ");
    AgentReplConnection::StdioMcp {
        program: "/bin/sh".to_owned(),
        args: vec![
            "-c".to_owned(),
            format!(
                "i=0; while IFS= read -r line; do i=$((i+1)); case $i in {cases} *) exit 0 ;; esac; done"
            ),
        ],
    }
}

#[cfg(not(windows))]
fn sh_quote(value: &str) -> String {
    value.replace('\'', "'\"'\"'")
}

fn test_repl_options() -> AgentReplOptions {
    AgentReplOptions {
        path: None,
        profile: ProfileOptions::default(),
        entry: None,
        flow: None,
        executor: CliRuntimeExecutorTier::BytecodeVm,
        pure_backend: None,
        pure_workers: None,
        pure_batch_min_len: None,
        pure_object_artifacts: false,
        math_backend: None,
        math_wgpu_min_elements: None,
        steps: 1,
        capture_step: None,
        mode: CliRuntimeStepMode::Drain,
        max_ops: 64,
        values: Vec::new(),
        viewport_width: AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH,
        viewport_height: AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT,
        textbox_height: None,
        capture_time_seconds: None,
        debug_db: None,
        trace: None,
        read_only: false,
        connect: None,
        input: None,
        json: true,
    }
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

#[test]
fn agent_mcp_debug_script_runs_enforces_max_privacy() {
    let db_path = std::env::temp_dir().join(format!(
        "arcweft-agent-mcp-script-runs-{}.sqlite3",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&db_path);
    let store = DebugStore::open(&db_path).expect("debug store");
    let session_id = SessionId::new("session.mcp.script").expect("session id");
    store
        .start_session(&session_id, None, "script", "mcp", 0)
        .expect("session");
    let metadata = BTreeMap::from([
        (
            "project_entities".to_owned(),
            serde_json::json!({
                "count": 1,
                "kind_counts": { "flow": 1 }
            }),
        ),
        (
            "project_graph".to_owned(),
            serde_json::json!({
                "symbol_count": 2,
                "edge_count": 1,
                "summary_symbol_id": "project:summary",
                "project_summary": {
                    "agent_action_count": 1
                }
            }),
        ),
        ("steps".to_owned(), serde_json::json!(2)),
    ]);
    store
        .upsert_script_run(&DebugScriptRun {
            run_id: AgentRunId::new("run.mcp.script").expect("run id"),
            session_id: session_id.clone(),
            agent_id: Some(PublicId::new("agent.mcp").expect("agent id")),
            artifact_hash: None,
            source_hash: Some(StableHash::new("blake3:mcp-script-source").expect("hash")),
            project_binding_mode: "strict".to_owned(),
            started_sequence: 1,
            finished_sequence: Some(2),
            outcome: DebugScriptRunOutcome::Done,
            partially_effectful: false,
            trace_uri: Some("target/run.mcp.script.arcwx".to_owned()),
            error: None,
            metadata,
        })
        .expect("script run");
    drop(store);

    let result = agent_mcp_call_debug_script_runs(&serde_json::json!({
        "path": db_path.display().to_string(),
        "session_id": "session.mcp.script"
    }))
    .expect("debug script runs succeeds");
    let value = mcp_text_json(&result);
    assert_eq!(value["max_privacy"], serde_json::json!("project"));
    assert_eq!(value["runs"][0]["metadata"]["steps"], serde_json::json!(2));
    assert_eq!(
        value["runs"][0]["project"]["graph_summary_symbol_id"],
        serde_json::json!("project:summary")
    );

    let public_result = agent_mcp_call_debug_script_runs(&serde_json::json!({
        "path": db_path.display().to_string(),
        "session_id": "session.mcp.script",
        "max_privacy": "public"
    }))
    .expect("public debug script runs succeeds");
    let public_value = mcp_text_json(&public_result);
    assert_eq!(public_value["max_privacy"], serde_json::json!("public"));
    assert!(public_value["runs"][0]["project"].is_null());
    assert_eq!(
        public_value["runs"][0]["metadata"]
            .as_object()
            .map(serde_json::Map::len),
        Some(0),
        "project-private script run metadata should be omitted at public ceiling"
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
            kind: AgentResourceKind::Trace,
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

fn mcp_tool_metadata_json(result: &McpCallToolResult) -> serde_json::Value {
    let Some(McpContentBlock::Text { text }) = result.content.first() else {
        panic!("expected leading metadata text block");
    };
    serde_json::from_str(text).expect("MCP metadata text block is JSON")
}

fn mcp_tool_resource_value(result: &McpCallToolResult) -> serde_json::Value {
    let Some(resource) = result.content.get(1) else {
        panic!("expected resource content block");
    };
    serde_json::to_value(resource).expect("MCP resource content serializes")
}

fn mcp_tool_resource_text_json(result: &McpCallToolResult) -> serde_json::Value {
    let resource = mcp_tool_resource_value(result);
    let text = resource["resource"]["text"]
        .as_str()
        .expect("resource content carries text");
    serde_json::from_str(text).expect("resource text is JSON")
}

fn mcp_read_text_json(read: &serde_json::Value) -> serde_json::Value {
    let text = read["contents"][0]["text"]
        .as_str()
        .expect("read result text");
    serde_json::from_str(text).expect("read text is JSON")
}

fn assert_listed_moderated_uri_reads(state: &mut AgentMcpState) {
    let listed = agent_mcp_resource_list(state).expect("resource list serializes");
    let listed_uri = listed["resources"]
        .as_array()
        .expect("resources array")
        .iter()
        .filter_map(|resource| resource["uri"].as_str())
        .find(|uri| uri.starts_with("arcweft://moderated/"))
        .expect("resource list returns a moderated URI")
        .to_owned();
    let listed_read = agent_mcp_resource_read(
        &serde_json::json!({
            "uri": listed_uri,
            "max_privacy": "sensitive"
        }),
        state,
    )
    .expect("listed moderated URI reads through cache");
    assert_eq!(
        listed_read["contents"][0]["uri"],
        serde_json::json!(listed_uri)
    );
}

fn assert_missing_image_metadata_tool_result(result: &McpCallToolResult) {
    let metadata = mcp_tool_metadata_json(result);
    assert_eq!(
        metadata["content_policy"]["disposition"],
        serde_json::json!("review")
    );
    assert!(
        metadata["content_policy"]["reason_codes"]
            .as_array()
            .expect("reason codes")
            .contains(&serde_json::json!("missing_image_metadata"))
    );
    let body = mcp_tool_resource_text_json(result);
    assert_eq!(
        body["content_policy"]["code"],
        serde_json::json!("missing_image_metadata")
    );
}

fn assert_missing_image_metadata_read(read: &serde_json::Value) {
    let body = mcp_read_text_json(read);
    assert_eq!(
        body["content_policy"]["code"],
        serde_json::json!("missing_image_metadata")
    );
}

fn assert_resource_read_audit(db_path: &std::path::Path) {
    let store = DebugStore::open(db_path).expect("debug store opens");
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
}

fn test_agent_raw_rgba_capture_resource(kind: AgentImageKind) -> AgentResource {
    let capture_name = kind.as_str();
    AgentResource {
        uri: format!("arcweft://session/cli/frame/0/object.customer.secret.{capture_name}.rgba"),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "blake3:raw-rgba-test".to_owned(),
        image: Some(AgentImageMetadata {
            kind,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Object {
                id: "object.customer.secret".to_owned(),
            },
            composition: kind.default_capture_composition(),
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 1,
            height: 1,
            crop_origin: None,
            pixel_format: Some("rgba8_unorm".to_owned()),
            row_stride_bytes: Some(4),
            content_bbox: None,
            content_viewport_bbox: None,
            content_pixels: None,
            object: None,
            selected_capture: None,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::BytesBase64(
            arcweft_agent_protocol::resource::AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "IECA/w==".to_owned(),
            },
        ),
    }
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

fn test_observed_object(id: &str, x: u32, y: u32, width: u32, height: u32) -> AgentObservedObject {
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
    let mut native_session = arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();

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
fn agent_viewport_color_capture_uses_image_frames_without_textbox() {
    let mut object = test_observed_object("object.image.logo", 1, 1, 2, 2);
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
    report.viewport = AgentViewport {
        width: 4,
        height: 4,
        scale: 1.0,
    };
    report.objects = vec![object];
    let mut frames = AgentImageFrameStore::default();
    frames.insert(
        "object.image.logo",
        2,
        2,
        vec![255, 0, 0, 255, 0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 255],
    );
    let mut native_session = arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();

    let result = agent_native_capture_image_with_frame_store(
        &report,
        &AgentCaptureReadRequest {
            uri: "arcweft://session/cli/frame/3/color.rgba".to_owned(),
            image_kind: AgentObserveImageKind::RawRgba,
            capture_kind: AgentObserveCaptureKind::Color,
            scope: AgentCaptureScope::Viewport,
            page: 0,
            capture_step: 3,
            capture_time_seconds: 0.0,
        },
        &mut native_session,
        &frames,
    )
    .unwrap();

    assert_eq!(result.image.composition, AgentImageComposition::Framebuffer);
    assert_eq!(result.image.width, 4);
    assert_eq!(result.image.height, 4);
    assert_eq!(
        &result.bytes[((4 + 1) * 4)..((4 + 2) * 4)],
        &[255, 0, 0, 255]
    );
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
        DisplayList, FragmentKind, ImageId, ImagePlayback, LayoutBox, LayoutLength, LayoutPoint,
        LayoutResults, LayoutSize, LayoutTree, NodeKey, StyleId, UiImageSource, UiImageSourceTable,
        UiLayerOutput, UiSemanticFragment, ViewFragmentBuilder,
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
            DecodedImageFrame::new(0, dimensions, 100, vec![0, 0, 0, 255, 1, 1, 1, 255]).unwrap(),
            DecodedImageFrame::new(1, dimensions, 100, vec![2, 2, 2, 255, 3, 3, 3, 255]).unwrap(),
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
    let mut native_session = arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();
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
    assert_eq!(result.bytes, vec![0, 0, 0, 0, 2, 2, 2, 255]);
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
            DecodedImageFrame::new(0, dimensions, 100, vec![8, 8, 8, 255, 9, 9, 9, 255]).unwrap(),
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
    let mut native_session = arcweft_render_native::NativeOffscreenCaptureSession::new().unwrap();
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
