use super::super::runtime::{
    NativeRunHost, RuntimeExecutorInstance, apply_runtime_entry_selection, report_path,
};
use super::{
    AGENT_OBSERVE_DEFAULT_VIEWPORT_HEIGHT, AGENT_OBSERVE_DEFAULT_VIEWPORT_WIDTH, AgentCommand,
    AgentHitTestOptions, AgentMcpOptions, AgentObserveCaptureKind, AgentObserveImageKind,
    AgentObserveMcpFormat, AgentObserveOptions, AgentObserveResourceKind, CliRuntimeExecutorTier,
    CliRuntimeStepMode, ExitCode, FlowFiberStatus, LineDisplayCatalog, NativeAdapterRegistrar,
    NativeTaskBridge, Path, PathBuf, ProfileOptions, RuntimeStepInput, RuntimeStepResult,
    flow_status_label, fs, load_and_check_selection,
    lower_source_runtime_plan_with_stats_and_options, native_host_policy_for_selection, print_json,
    resolve_source_selection, runtime_plan_options_for_selection,
    runtime_pure_config_for_selection, step_options,
};
use arcweft_agent_mcp::{
    McpCallToolResult, McpContentBlock, agent_tool_descriptors, list_resource_templates_result,
    list_resources_result, read_resource_result, resource_descriptor, tool_result_for_resource,
    tool_result_for_resources,
};
use arcweft_agent_protocol::{
    AgentActionDispatch, AgentActionKind, AgentActionTarget, AgentAssignment, AgentAudioState,
    AgentBBox, AgentCoordinateSpace, AgentDiagnostic, AgentDiagnosticSeverity,
    AgentGlyphOrientation, AgentGlyphVerticalForm, AgentHitRegion, AgentHitRegionKind,
    AgentHitTestHit, AgentHitTestReport, AgentImageComposition, AgentImageContentBBox,
    AgentImageCropOrigin, AgentImageKind, AgentImageMetadata, AgentImageObjectRef,
    AgentImageRenderer, AgentImageResource, AgentImageScope, AgentLayerCaptureRef,
    AgentLayerCaptureRefs, AgentObjectCaptureRef, AgentObjectCaptureRefs, AgentObservationReport,
    AgentObservedLayer, AgentObservedObject, AgentPresentationObjectProxyParamQuery,
    AgentPresentationTree, AgentPresentationTreeQuery, AgentResource, AgentResourceBody,
    AgentRgbaColor, AgentRichTextElementKind, AgentRichTextElementRef, AgentUiTree, AgentViewport,
};
use arcweft_core::plan::FlowEvent;
use arcweft_render_text::{
    LineDisplayFrame, RichTextControl, RichTextNode, RichTextObjectProxy, RichTextPresentation,
    RichTextRange, RichTextRubyAnnotation, RichTextTextRun, RichTextTextSource, RuntimeLineContext,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::io::{BufRead as _, Write as _};

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

pub(super) fn agent_command(
    command: AgentCommand,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<(), ExitCode> {
    match command {
        AgentCommand::Observe(options) => agent_observe_command(&options, adapter_registrars),
        AgentCommand::HitTest(options) => agent_hit_test_command(&options, adapter_registrars),
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
    native_capture_session: Option<arcweft_render_native::NativeOffscreenCaptureSession>,
}

struct AgentObservationState {
    report: AgentObservationReport,
    native_session: arcweft_render_native::NativeOffscreenCaptureSession,
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
    state: &mut AgentMcpState,
) -> Result<serde_json::Value, String> {
    let uri = params
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "resources/read requires params.uri".to_owned())?;
    let Some(report) = state.report.clone() else {
        return Err("resources/read requires a prior arcweft.observe call".to_owned());
    };
    let image_output = state.image_output.clone();
    let resource = if let Some(resource) = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(&report, image_output.as_ref(), uri))
    {
        resource
    } else {
        let native_session = agent_mcp_native_capture_session(state)
            .map_err(|_| format!("failed to read Agent resource `{uri}`"))?;
        agent_observe_resource_by_uri_with_page_and_time_and_session(
            &report,
            uri,
            None,
            agent_report_capture_time_seconds(&report),
            Some(native_session),
        )
        .map_err(|_| format!("failed to read Agent resource `{uri}`"))?
    };
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
        "arcweft.hit_test" => {
            let tool = agent_mcp_call_hit_test(&arguments, state, adapter_registrars)?;
            serde_json::to_value(tool)
                .map_err(|error| format!("failed to serialize MCP hit-test result: {error}"))
        }
        tool => Err(format!("unsupported Arcweft MCP tool `{tool}`")),
    }
}

fn agent_mcp_call_observe(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<serde_json::Value, String> {
    let (report, image_output, resources, native_session) =
        agent_mcp_run_observation(arguments, adapter_registrars)?;
    state.report = Some(report);
    state.image_output = image_output;
    state.native_capture_session = Some(native_session);
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
        let (report, image_output, _, native_session) =
            agent_mcp_run_observation(arguments, adapter_registrars)?;
        state.report = Some(report);
        state.image_output = image_output;
        state.native_capture_session = Some(native_session);
        state.capture_resources.clear();
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

fn agent_mcp_arguments_request_observe(arguments: &serde_json::Value) -> bool {
    arguments.get("source").is_some() || arguments.get("profile").is_some()
}

fn agent_mcp_run_observation(
    arguments: &serde_json::Value,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<
    (
        AgentObservationReport,
        Option<AgentImageOutput>,
        Vec<AgentResource>,
        arcweft_render_native::NativeOffscreenCaptureSession,
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
    let runtime_options = runtime_plan_options_for_selection(&selection);
    let lowered = lower_source_runtime_plan_with_stats_and_options(&checked.hir, &runtime_options)
        .map_err(|_| "failed to lower runtime plan".to_owned())?;
    let mut native_session = agent_native_capture_session_for_hir(&checked.hir)
        .map_err(|_| "failed to create native capture session".to_owned())?;
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
        Some(&mut native_session),
    )
    .map_err(|error| error.to_string())?;
    let image_output = agent_observe_image_output(&mut report, &options, Some(&mut native_session))
        .map_err(|_| "failed to build MCP observe image output".to_owned())?;
    let resources = agent_observe_list_resources(&report, image_output.as_ref())
        .map_err(|_| "failed to build MCP observe resources".to_owned())?;
    Ok((report, image_output, resources, native_session))
}

fn agent_mcp_call_resource_read(
    arguments: &serde_json::Value,
    state: &mut AgentMcpState,
) -> Result<arcweft_agent_mcp::McpCallToolResult, String> {
    let uri = arguments
        .get("uri")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "arcweft.resource.read requires arguments.uri".to_owned())?;
    let Some(report) = state.report.clone() else {
        return Err("arcweft.resource.read requires a prior arcweft.observe call".to_owned());
    };
    let image_output = state.image_output.clone();
    let resource = if let Some(resource) = agent_mcp_cached_capture_resource(state, uri)
        .or_else(|| agent_observe_cached_image_resource(&report, image_output.as_ref(), uri))
    {
        resource
    } else {
        let native_session = agent_mcp_native_capture_session(state)
            .map_err(|_| format!("failed to read Agent resource `{uri}`"))?;
        agent_observe_resource_by_uri_with_page_and_time_and_session(
            &report,
            uri,
            None,
            agent_report_capture_time_seconds(&report),
            Some(native_session),
        )
        .map_err(|_| format!("failed to read Agent resource `{uri}`"))?
    };
    tool_result_for_resource(&resource)
        .map_err(|error| format!("failed to serialize MCP tool resource: {error}"))
}

fn agent_mcp_current_resources(state: &AgentMcpState) -> Result<Vec<AgentResource>, ExitCode> {
    let Some(report) = &state.report else {
        return Ok(Vec::new());
    };
    let mut resources = agent_observe_list_resources(report, state.image_output.as_ref())?;
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
    if arguments.get("source").is_some() || arguments.get("profile").is_some() {
        let (report, image_output, _, native_session) = agent_mcp_run_observation(
            &agent_mcp_capture_observe_arguments(arguments),
            adapter_registrars,
        )?;
        state.report = Some(report);
        state.image_output = image_output;
        state.native_capture_session = Some(native_session);
        state.capture_resources.clear();
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
    let native_session = agent_mcp_native_capture_session(state)?;
    agent_native_capture_resource_with_session(report, request, native_session)
}

fn agent_mcp_native_capture_session(
    state: &mut AgentMcpState,
) -> Result<&mut arcweft_render_native::NativeOffscreenCaptureSession, ExitCode> {
    if state.native_capture_session.is_none() {
        state.native_capture_session = Some(
            arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
                eprintln!("error: native capture failed: {error}");
                ExitCode::FAILURE
            })?,
        );
    }
    Ok(state
        .native_capture_session
        .as_mut()
        .expect("native capture session initialized above"))
}

fn agent_native_capture_session_for_hir(
    hir: &arcweft_lang_hir::model::HirModule,
) -> Result<arcweft_render_native::NativeOffscreenCaptureSession, ExitCode> {
    let mut native_session =
        arcweft_render_native::NativeOffscreenCaptureSession::new().map_err(|error| {
            eprintln!("error: native capture failed: {error}");
            ExitCode::FAILURE
        })?;
    arcweft_render_native::register_arcweft_pure_text_motions(
        native_session.motion_registry_mut(),
        hir,
    )
    .map_err(|error| {
        eprintln!("error: failed to register Arcweft text motion functions: {error}");
        ExitCode::FAILURE
    })?;
    arcweft_render_native::register_arcweft_pure_text_effects(
        native_session.effect_registry_mut(),
        hir,
    )
    .map_err(|error| {
        eprintln!("error: failed to register Arcweft text effect functions: {error}");
        ExitCode::FAILURE
    })?;
    arcweft_render_native::register_arcweft_pure_text_shaders(
        native_session.shader_registry_mut(),
        hir,
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
    )?;
    if let Some(uri) = &options.read_uri {
        let resource =
            agent_observe_cached_image_resource(&observed.report, image_output.as_ref(), uri)
                .map_or_else(
                    || {
                        agent_observe_resource_by_uri_with_page_and_time_and_session(
                            &observed.report,
                            uri,
                            options.page,
                            agent_observe_capture_time_seconds(options),
                            Some(&mut observed.native_session),
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

fn agent_observation_for_options(
    options: &AgentObserveOptions,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> Result<AgentObservationState, ExitCode> {
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
    let mut native_session = agent_native_capture_session_for_hir(&checked.hir)?;
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
        Some(&mut native_session),
    )
    .map_err(|error| {
        eprintln!("error: {error}");
        ExitCode::FAILURE
    })?;
    Ok(AgentObservationState {
        report,
        native_session,
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
        return Vec::new();
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
            depth: region.depth.or(rich_text_ref.object_depth),
        })
        .collect()
}

fn agent_hit_test_layer(
    object: &AgentObservedObject,
    rich_text_ref: &AgentRichTextElementRef,
    region: &AgentHitRegion,
) -> String {
    region
        .proxy_layer
        .clone()
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
        AgentHitRegionKind::TextGlyph => 10,
        AgentHitRegionKind::GlyphCluster => 20,
        AgentHitRegionKind::RubyAnnotation => 30,
        AgentHitRegionKind::RubyBase => 40,
        AgentHitRegionKind::RubyObject => 50,
        AgentHitRegionKind::TextRun => 60,
        AgentHitRegionKind::TextLine => 70,
        AgentHitRegionKind::TextPage => 80,
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
        Some(native_session) => {
            agent_native_capture_resource_with_session(report, &request, native_session)
        }
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

fn agent_native_capture_resource_with_session(
    report: &AgentObservationReport,
    request: &AgentCaptureReadRequest,
    native_session: &mut arcweft_render_native::NativeOffscreenCaptureSession,
) -> Result<AgentResource, ExitCode> {
    let result = agent_native_capture_image_with_session(report, request, native_session)?;
    Ok(report.image_resource(&result.image, &result.bytes))
}

struct AgentNativeCaptureImageResult {
    image: AgentImageResource,
    bytes: Vec<u8>,
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
    let Some(textbox) = agent_native_textbox_for_capture(report, &request.scope) else {
        eprintln!("error: native renderer requires an observed textbox frame");
        return Err(ExitCode::from(2));
    };
    let (left, top) = agent_native_text_origin(textbox);
    let capture = native_session
        .capture_frame_rgba_in(
            &textbox.rich_text,
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
        if let Some(parent_id) = agent_rich_text_child_parent_object_id(object_id) {
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
    if object.role == "textbox" {
        return object
            .rich_text
            .display_map
            .text_runs
            .iter()
            .enumerate()
            .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
            .chain(
                object
                    .rich_text
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
    textbox
        .rich_text
        .display_map
        .text_runs
        .iter()
        .enumerate()
        .filter(move |(_, run)| agent_rich_text_ranges_overlap(run.range, range))
        .map(|(index, _)| arcweft_render_native::NativeFrameElement::TextRun { index })
        .chain(
            textbox
                .rich_text
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
    if object.role == "textbox" {
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
        &textbox.rich_text,
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
        &textbox.rich_text,
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
        .find(|candidate| candidate.role == "textbox" && candidate.id == parent_id)
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
        .find(|candidate| candidate.role == "textbox" && candidate.id == parent_id)?;
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
    host_config: NativeRunHost<'_>,
    options: &AgentObserveOptions,
    source_path: &Path,
    native_session: Option<&mut arcweft_render_native::NativeOffscreenCaptureSession>,
) -> Result<AgentObservationReport, arcweft_host_adapter::HostAdapterError> {
    let viewport = AgentViewport {
        width: options.viewport_width,
        height: options.viewport_height,
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
    let effective_steps = agent_observe_effective_steps(options);
    let force_capture_step = options.capture_step.is_some();
    let mut native_session = native_session;
    for step_index in 0..effective_steps {
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
            source: Some("runtime".to_owned()),
            code: None,
            effect_id: None,
            message: diagnostic.message.clone(),
        }));
        for event in &output.flow_events {
            let textbox_index = objects
                .iter()
                .filter(|object| object.role == "textbox")
                .count();
            match agent_observed_objects_for_flow_event(
                step_index,
                textbox_index,
                catalog,
                event,
                &viewport,
                options,
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
                Some(native_session) => {
                    agent_native_capture_image_with_session(report, &request, native_session)?
                }
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
        children.extend(agent_rich_text_proxy_objects(
            step,
            index,
            run_index,
            textbox,
            run,
            native_bounds,
        ));
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
    agent_rich_text_page_ranges(&textbox.rich_text)
        .into_iter()
        .enumerate()
        .filter_map(|(page_index, page_range)| {
            if page_range.is_empty() {
                return None;
            }
            let page_text = textbox.rich_text.text.get(page_range.clone())?;
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
            let presentation = agent_rich_text_range_presentation(&textbox.rich_text, range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextPage, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                &textbox.rich_text,
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
                        node_index: agent_rich_text_page_node_index(&textbox.rich_text, range),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(&textbox.rich_text, range),
                        object_depth: agent_rich_text_page_object_depth(&textbox.rich_text, range),
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
    agent_rich_text_line_ranges(&textbox.rich_text)
        .into_iter()
        .enumerate()
        .filter_map(|(line_index, line_range)| {
            if line_range.is_empty() {
                return None;
            }
            let line_text = textbox.rich_text.text.get(line_range.clone())?;
            if line_text.trim().is_empty() {
                return None;
            }
            let range = RichTextRange::new(line_range.start, line_range.end);
            let page = agent_rich_text_page_for_range(&textbox.rich_text, range);
            let bbox = agent_native_text_range_capture_bbox_for_page(
                textbox,
                viewport,
                page,
                range,
                time_seconds,
                native_session.as_deref_mut(),
            )?;
            let presentation = agent_rich_text_range_presentation(&textbox.rich_text, range);
            let mut hit_regions =
                vec![agent_hit_region(AgentHitRegionKind::TextLine, &bbox, range)];
            hit_regions.extend(agent_rich_text_range_proxy_hit_regions(
                &textbox.rich_text,
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
                        node_index: agent_rich_text_page_node_index(&textbox.rich_text, range),
                        source: None,
                        ruby: None,
                        presentation,
                        orientation: None,
                        vertical_form: None,
                        ruby_base_bbox: None,
                        ruby_annotation_bbox: None,
                        object_layer: agent_rich_text_range_object_layer(&textbox.rich_text, range),
                        object_depth: agent_rich_text_page_object_depth(&textbox.rich_text, range),
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
            &textbox.rich_text,
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
        &textbox.rich_text,
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
        &textbox.rich_text,
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
            agent_native_element_overlaps_range(&textbox.rich_text, bounds.element, range)
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
    let text = textbox
        .rich_text
        .text
        .get(valid_rich_text_range(run.range, &textbox.rich_text.text)?)?;
    if text.trim().is_empty() {
        return None;
    }
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::TextRun { index: run_index })
        .map(|bounds| bounds.bbox.clone())?;
    let object_id = agent_rich_text_run_object_id(step, index, run_index);
    let page = agent_rich_text_page_for_range(&textbox.rich_text, run.range);
    let parent_id = agent_rich_text_line_for_range(&textbox.rich_text, run.range).map_or_else(
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
    let Some(range) = valid_rich_text_range(run.range, &textbox.rich_text.text) else {
        return Vec::new();
    };
    let Some(text) = textbox.rich_text.text.get(range) else {
        return Vec::new();
    };
    if text.trim().is_empty() {
        return Vec::new();
    }
    let page = agent_rich_text_page_for_range(&textbox.rich_text, run.range);
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
    let base_range = valid_rich_text_range(ruby.base_range, &textbox.rich_text.text)?;
    let base_text = textbox.rich_text.text.get(base_range)?;
    let bbox = native_bounds
        .get(&arcweft_render_native::NativeFrameElement::Ruby { index: ruby_index })
        .cloned()?;
    let object_id = format!("object.dialogue.{step}.{index}.ruby.{ruby_index}");
    let page = agent_rich_text_page_for_range(&textbox.rich_text, ruby.base_range);
    let parent_id = agent_rich_text_line_for_range(&textbox.rich_text, ruby.base_range)
        .map_or_else(
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
            let text = textbox
                .rich_text
                .text
                .get(valid_rich_text_range(range, &textbox.rich_text.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = textbox
                .rich_text
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.glyph.{glyph_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(&textbox.rich_text, range);
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
            let text = textbox
                .rich_text
                .text
                .get(valid_rich_text_range(range, &textbox.rich_text.text)?)?;
            if text.trim().is_empty() {
                return None;
            }
            let (run_index, run) = textbox
                .rich_text
                .display_map
                .text_runs
                .iter()
                .enumerate()
                .find(|(_, run)| range.start >= run.range.start && range.end <= run.range.end)?;
            let object_id = format!(
                "object.dialogue.{step}.{index}.cluster.{cluster_index}.{range_start}.{range_end}"
            );
            let page = agent_rich_text_page_for_range(&textbox.rich_text, range);
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
        .rich_text_ref
        .as_ref()
        .and_then(|rich_text_ref| rich_text_ref.object_layer.as_ref())
        .filter(|object_layer| *object_layer != &object.layer)
    {
        layers.push(object_layer.clone());
    }
    layers
}

fn agent_object_matches_layer(object: &AgentObservedObject, layer: &str) -> bool {
    object.layer == layer
        || object
            .rich_text_ref
            .as_ref()
            .and_then(|rich_text_ref| rich_text_ref.object_layer.as_ref())
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
