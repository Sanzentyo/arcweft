use crate::{
    model::{
        AGENT_TRACE_MIME_TYPE, McpBlobResourceContents, McpContentBlock, McpResourceContents,
        McpTextResourceContents, McpToolDescriptor,
    },
    resources::{
        image_composition_description, list_resource_templates_result, list_resources_result,
        read_resource_result, resource_descriptor, resource_link, tool_result_for_resource,
        tool_result_for_resources, trace_resource,
    },
    tools::agent_tool_descriptors,
};

use arcweft_agent_protocol::{
    geometry::AgentCoordinateSpace,
    ids::{AgentRunId, SessionId, StableHash},
    image::{
        AgentImageComposition, AgentImageContentBBox, AgentImageCropOrigin, AgentImageKind,
        AgentImageMetadata, AgentImageRenderer, AgentImageScope,
    },
    resource::{
        AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource, AgentResourceBody,
        AgentResourceKind,
    },
    trace::{AgentTraceKind, AgentTraceRecord},
};

#[test]
fn tool_descriptors_include_wait_control_surface() {
    let tools = agent_tool_descriptors();
    let wait = tools
        .iter()
        .find(|tool| tool.name == "arcweft.wait")
        .expect("wait tool is listed");

    assert_eq!(wait.title.as_deref(), Some("Wait For Arcweft Predicate"));
    assert_eq!(
        wait.input_schema["required"],
        serde_json::json!(["predicate", "timeout_millis"])
    );
    assert_eq!(
        wait.input_schema["properties"]["predicate"]["type"],
        "object"
    );
}

#[test]
fn tool_descriptors_include_script_run_surface() {
    let tools = agent_tool_descriptors();
    let script_run = tools
        .iter()
        .find(|tool| tool.name == "arcweft.script.run")
        .expect("script run tool is listed");

    assert_eq!(
        script_run.title.as_deref(),
        Some("Run Arcweft Agent Script")
    );
    assert_eq!(
        script_run.input_schema["required"],
        serde_json::json!(["path"])
    );
    assert_eq!(
        script_run.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        script_run.input_schema["properties"]["executor"]["enum"],
        serde_json::json!(["bytecode-vm", "aot"])
    );
    assert_eq!(
        script_run.input_schema["properties"]["native_mode"]["enum"],
        serde_json::json!(["one-op", "drain", "game", "server"])
    );
    assert_eq!(
        script_run.input_schema["properties"]["values"]["type"],
        "object"
    );
    assert_eq!(
        script_run.input_schema["properties"]["signals"]["type"],
        "object"
    );
    assert_eq!(
        script_run.input_schema["properties"]["state"]["type"],
        "object"
    );
}

#[test]
fn image_agent_resource_maps_to_mcp_blob_and_image_tool_content() {
    let resource = AgentResource {
        uri: "arcweft://session/cli/frame/0/layer.dialogue.png".to_owned(),
        kind: AgentResourceKind::Image,
        mime_type: "image/png".to_owned(),
        hash: "hash".to_owned(),
        image: Some(AgentImageMetadata {
            kind: AgentImageKind::Color,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Layer {
                id: "dialogue".to_owned(),
            },
            composition: AgentImageComposition::MaskedFramebufferCrop,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            width: 320,
            height: 180,
            crop_origin: Some(AgentImageCropOrigin {
                space: AgentCoordinateSpace::Viewport,
                x: 96,
                y: 548,
            }),
            pixel_format: None,
            row_stride_bytes: None,
            content_bbox: Some(AgentImageContentBBox {
                x: 10,
                y: 12,
                width: 32,
                height: 24,
            }),
            content_viewport_bbox: Some(AgentImageContentBBox {
                x: 106,
                y: 560,
                width: 32,
                height: 24,
            }),
            content_pixels: Some(512),
            object: None,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: "iVBORw0KGgo=".to_owned(),
        }),
    };

    let descriptor = resource_descriptor(&resource);
    let read = read_resource_result(&resource).expect("resource read serializes");
    let tool = tool_result_for_resource(&resource).expect("tool result serializes");

    assert_eq!(descriptor.name, "layer.dialogue.png");
    assert_eq!(descriptor.mime_type.as_deref(), Some("image/png"));
    assert_eq!(descriptor.size, Some(8));
    let description = descriptor.description.as_deref().unwrap();
    assert!(description.contains("kind=color"));
    assert!(description.contains("renderer=native"));
    assert!(description.contains("scope=layer:dialogue"));
    assert!(description.contains("composition=masked_framebuffer_crop"));
    assert_eq!(
        image_composition_description(AgentImageComposition::ObjectIdAttachment),
        "object_id_attachment"
    );
    assert_eq!(
        image_composition_description(AgentImageComposition::MaskAttachment),
        "mask_attachment"
    );
    assert!(description.contains("width=320"));
    assert!(description.contains("height=180"));
    assert!(matches!(
        read.contents.as_slice(),
        [McpResourceContents::Blob(McpBlobResourceContents { blob, .. })] if blob == "iVBORw0KGgo="
    ));
    assert!(matches!(
        tool.content.as_slice(),
        [
            McpContentBlock::Text { text },
            McpContentBlock::Image { data, mime_type },
        ] if text.contains("\"width\":320")
            && text.contains("\"renderer\":\"native\"")
            && text.contains("\"scope\"")
            && text.contains("\"kind\":\"layer\"")
            && text.contains("\"id\":\"dialogue\"")
            && text.contains("\"composition\":\"masked_framebuffer_crop\"")
            && text.contains("\"crop_origin\"")
            && text.contains("\"content_viewport_bbox\"")
            && text.contains("\"content_pixels\":512")
            && data == "iVBORw0KGgo="
            && mime_type == "image/png"
    ));
}

#[test]
fn image_tool_content_preserves_object_rich_text_ref_metadata() {
    let metadata: AgentImageMetadata =
        serde_json::from_value(proxy_object_image_metadata_fixture())
            .expect("object image metadata deserializes");
    let resource = AgentResource {
        uri: "arcweft://session/cli/frame/0/object.object.dialogue.0.0.proxy.0.0.mask.rgba"
            .to_owned(),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "hash".to_owned(),
        image: Some(metadata),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: "AAAA".to_owned(),
        }),
    };

    let tool = tool_result_for_resource(&resource).expect("tool result serializes");

    let [
        McpContentBlock::Text { text },
        McpContentBlock::Resource { .. },
    ] = tool.content.as_slice()
    else {
        panic!(
            "raw image tool result should expose metadata text plus resource blob: {:?}",
            tool.content
        );
    };
    let json: serde_json::Value = serde_json::from_str(text).expect("metadata text is JSON object");
    assert_eq!(
        json["image"]["object"]["id"],
        "object.dialogue.0.0.proxy.0.0"
    );
    assert_eq!(
        json["image"]["object"]["rich_text_ref"]["kind"],
        "text_object_proxy"
    );
    assert_eq!(
        json["image"]["object"]["rich_text_ref"]["presentation"]["object_proxies"][0]["params"]["channel"]
            ["value"],
        "choice"
    );
    assert_eq!(
        json["image"]["object"]["bbox"]["space"],
        serde_json::json!("viewport")
    );
    assert_eq!(
        json["image"]["object"]["capture_refs"]["object_id_color"]["alpha"],
        255
    );
}

#[test]
fn image_tool_content_preserves_image_object_frame_metadata() {
    let metadata: AgentImageMetadata =
        serde_json::from_value(image_object_frame_metadata_fixture())
            .expect("image object frame metadata deserializes");
    let resource = AgentResource {
        uri: "arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba"
            .to_owned(),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "hash".to_owned(),
        image: Some(metadata),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: "AAAA".to_owned(),
        }),
    };

    let tool = tool_result_for_resource(&resource).expect("tool result serializes");

    let [
        McpContentBlock::Text { text },
        McpContentBlock::Resource { .. },
    ] = tool.content.as_slice()
    else {
        panic!(
            "raw image tool result should expose metadata text plus resource blob: {:?}",
            tool.content
        );
    };
    let json: serde_json::Value = serde_json::from_str(text).expect("metadata text is JSON object");
    assert_eq!(
        json["image"]["object"]["image_ref"]["asset"],
        "asset.bg.pulse"
    );
    assert_eq!(json["image"]["object"]["image_ref"]["frame_index"], 1);
    assert_eq!(
        json["image"]["object"]["image_ref"]["local_time_millis"],
        150
    );
    assert_eq!(
        json["image"]["object"]["image_ref"]["proxies"][0]["id"],
        "proxy.pulse_sprite.hotspot"
    );
    assert_eq!(
        json["image"]["object"]["image_ref"]["params"]["param.role"]["value"],
        "animated-hotspot"
    );
}

fn proxy_object_image_metadata_fixture() -> serde_json::Value {
    serde_json::json!({
        "kind": "mask",
        "renderer": "native",
        "scope": { "kind": "object", "id": "object.dialogue.0.0.proxy.0.0" },
        "composition": "mask_attachment",
        "width": 12,
        "height": 8,
        "pixel_format": "rgba8_unorm",
        "row_stride_bytes": 48,
        "content_pixels": 24,
        "object": {
            "id": "object.dialogue.0.0.proxy.0.0",
            "layer": "dialogue.rich_text",
            "role": "rich_text_proxy",
            "bbox": { "space": "viewport", "x": 120, "y": 520, "width": 12, "height": 8 },
            "polygon": [
                { "x": 120, "y": 520 },
                { "x": 132, "y": 520 },
                { "x": 132, "y": 528 },
                { "x": 120, "y": 528 }
            ],
            "capture_refs": {
                "object_id_color": {
                    "red": 10,
                    "green": 20,
                    "blue": 30,
                    "alpha": 255
                },
                "captures": [{
                    "kind": "mask",
                    "uri": "arcweft://session/cli/frame/0/object.object.dialogue.0.0.proxy.0.0.mask.rgba",
                    "mime_type": "application/octet-stream",
                    "width": 12,
                    "height": 8
                }]
            },
            "text": "proxy",
            "rich_text_ref": {
                "kind": "text_object_proxy",
                "index": 0,
                "range": { "start": 10, "end": 15 },
                "node_index": 3,
                "presentation": {
                    "object_proxies": [{
                        "id": "hotspot",
                        "type_name": "KeywordHit",
                        "role": "keyword",
                        "depth": 4000,
                        "hit_test": true,
                        "params": {
                            "channel": { "kind": "selector", "value": "choice" }
                        }
                    }]
                },
                "object_depth": 4000,
                "hit_test": true,
                "hit_regions": []
            }
        }
    })
}

fn image_object_frame_metadata_fixture() -> serde_json::Value {
    serde_json::from_str(
            r#"{
                "kind": "color",
                "renderer": "native",
                "scope": { "kind": "object", "id": "object.image.layer.foreground.0.1" },
                "composition": "framebuffer_crop",
                "width": 360,
                "height": 180,
                "pixel_format": "rgba8_unorm",
                "row_stride_bytes": 1440,
                "content_pixels": 64800,
                "object": {
                    "id": "object.image.layer.foreground.0.1",
                    "entity": "image.sample.pulse_sprite",
                    "layer": "layer.foreground",
                    "role": "image",
                    "bbox": { "space": "viewport", "x": 120, "y": 84, "width": 360, "height": 180 },
                    "polygon": [
                        { "x": 120, "y": 84 },
                        { "x": 480, "y": 84 },
                        { "x": 480, "y": 264 },
                        { "x": 120, "y": 264 }
                    ],
                    "capture_refs": {
                        "object_id_color": {
                            "red": 10,
                            "green": 20,
                            "blue": 30,
                            "alpha": 255
                        },
                        "captures": [{
                            "kind": "color",
                            "uri": "arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba",
                            "mime_type": "application/octet-stream",
                            "width": 360,
                            "height": 180
                        }]
                    },
                    "object_layer": "layer.foreground",
                    "object_depth": 2500,
                    "image_ref": {
                        "source": "ui.image.1",
                        "object": "image.sample.pulse_sprite",
                        "target": "target.sample.pulse_sprite",
                        "asset": "asset.bg.pulse",
                        "frame_index": 1,
                        "local_time_millis": 150,
                        "opacity_milli": 500,
                        "intrinsic_width": 2,
                        "intrinsic_height": 1,
                        "actions": ["action.inspect.pulse_sprite"],
                        "params": {
                            "param.role": { "kind": "text", "value": "animated-hotspot" }
                        },
                        "proxies": [{
                            "id": "proxy.pulse_sprite.hotspot",
                            "type_name": "PulseSpriteHotspot",
                            "role": "inspect",
                            "layer": "layer.hit",
                            "depth": 2600,
                            "hit_test": true,
                            "params": {
                                "param.channel": { "kind": "text", "value": "preview" }
                            }
                        }]
                    }
                }
            }"#,
        )
        .expect("image object frame metadata fixture is valid JSON")
}

#[test]
fn resource_list_and_observe_tool_result_expose_resource_links() {
    let resources = vec![
        AgentResource {
            uri: "arcweft://session/cli/observation/latest.json".to_owned(),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "hash".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
        },
        AgentResource {
            uri: "arcweft://session/cli/frame/0/layer.dialogue.object-id.png".to_owned(),
            kind: AgentResourceKind::Image,
            mime_type: "image/png".to_owned(),
            hash: "hash".to_owned(),
            image: Some(AgentImageMetadata {
                kind: AgentImageKind::ObjectId,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Layer {
                    id: "dialogue".to_owned(),
                },
                composition: AgentImageComposition::ObjectIdAttachment,
                page: 0,
                capture_step: 0,
                capture_time_millis: 0,
                width: 320,
                height: 180,
                crop_origin: None,
                pixel_format: None,
                row_stride_bytes: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: None,
                diagnostics: Vec::new(),
            }),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: "iVBORw0KGgo=".to_owned(),
            }),
        },
    ];

    let list = list_resources_result(&resources);
    let tool = tool_result_for_resources(&resources);

    assert_eq!(list.resources.len(), 2);
    assert_eq!(list.resources[1].name, "layer.dialogue.object-id.png");
    assert_eq!(list.resources[1].mime_type.as_deref(), Some("image/png"));
    assert!(
        list.resources[1]
            .description
            .as_deref()
            .is_some_and(|description| description.contains("kind=object_id")
                && description.contains("renderer=native")
                && description.contains("scope=layer:dialogue")
                && description.contains("composition=object_id_attachment"))
    );
    assert!(matches!(
        tool.content.as_slice(),
        [
            McpContentBlock::ResourceLink { name: first, .. },
            McpContentBlock::ResourceLink { name: second, mime_type: Some(mime_type), .. },
        ] if first == "latest.json" && second == "layer.dialogue.object-id.png" && mime_type == "image/png"
    ));
}

#[test]
fn resource_templates_list_capture_uri_patterns() {
    let templates = list_resource_templates_result();

    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "session-context"
            && template.uri_template == "arcweft://session/{session_id}/context.json"
            && template.mime_type.as_deref() == Some("application/json")
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "viewport-capture"
            && template.uri_template
                == "arcweft://session/{session_id}/frame/{tick}/{capture}.{extension}"
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "layer-mask-capture"
            && template
                .uri_template
                .contains("layer.{layer_id}.mask.{extension}")
            && template
                .description
                .as_deref()
                .is_some_and(|description| description.contains("png or rgba"))
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "layer-object-id-capture"
            && template
                .uri_template
                .contains("layer.{layer_id}.object-id.{extension}")
            && template
                .description
                .as_deref()
                .is_some_and(|description| description.contains("png or rgba"))
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "object-color-capture"
            && template
                .uri_template
                .contains("object.{object_id}.{extension}")
            && template
                .description
                .as_deref()
                .is_some_and(|description| description.contains("rich-text child objects"))
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "object-object-id-capture"
            && template
                .uri_template
                .contains("object.{object_id}.object-id.{extension}")
            && template
                .description
                .as_deref()
                .is_some_and(|description| description.contains("rich-text child objects"))
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "presentation-tree-filter"
            && template
                .uri_template
                .contains("presentation-tree.json?{filter_key}={filter_value}")
            && template.description.as_deref().is_some_and(|description| {
                description.contains("proxy id/type/role/struct/params")
                    && description.contains("preserving ancestors")
            })
    }));
    assert!(templates.resource_templates.iter().any(|template| {
        template.name == "agent-trace"
            && template.uri_template == "arcweft://run/{run_id}/trace.arcwx"
            && template.mime_type.as_deref() == Some(AGENT_TRACE_MIME_TYPE)
    }));
}

#[test]
fn trace_resource_maps_to_mcp_text_resource_and_link() {
    let records = trace_records_fixture();
    let resource = trace_resource(&records).expect("trace resource serializes");
    let list = list_resources_result(std::slice::from_ref(&resource));
    let read = read_resource_result(&resource).expect("trace resource reads");
    let tool = tool_result_for_resource(&resource).expect("trace tool result serializes");

    assert_eq!(resource.kind, AgentResourceKind::Trace);
    assert_eq!(resource.uri, "arcweft://run/run.cli/trace.arcwx");
    assert_eq!(resource.mime_type, AGENT_TRACE_MIME_TYPE);
    assert_eq!(resource.hash, "trace:run.cli:2:blake3:run-finished-payload");
    assert_eq!(list.resources[0].name, "trace.arcwx");
    assert_eq!(list.resources[0].title.as_deref(), Some("Agent trace"));
    assert!(
        list.resources[0]
            .description
            .as_deref()
            .is_some_and(|description| description.contains("read-only replay"))
    );
    assert!(matches!(
        read.contents.as_slice(),
        [McpResourceContents::Text(McpTextResourceContents { mime_type: Some(mime_type), text, .. })]
            if mime_type == AGENT_TRACE_MIME_TYPE && text.contains("\"run_finished\"")
    ));
    assert!(matches!(
        tool.content.as_slice(),
        [McpContentBlock::Resource { resource: McpResourceContents::Text(McpTextResourceContents { uri, .. }) }]
            if uri == "arcweft://run/run.cli/trace.arcwx"
    ));
}

#[test]
fn json_agent_resource_maps_to_mcp_text_resource() {
    let resource = AgentResource {
        uri: "arcweft://session/cli/observation/latest.json".to_owned(),
        kind: AgentResourceKind::ObservationLatest,
        mime_type: "application/json".to_owned(),
        hash: "hash".to_owned(),
        image: None,
        body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
    };

    let read = read_resource_result(&resource).expect("resource read serializes");
    let link = resource_link(&resource);

    assert!(matches!(
        read.contents.as_slice(),
        [McpResourceContents::Text(McpTextResourceContents { text, .. })] if text == "{\"status\":\"ok\"}"
    ));
    assert!(matches!(
        link,
        McpContentBlock::ResourceLink { name, mime_type: Some(mime_type), .. }
            if name == "latest.json" && mime_type == "application/json"
    ));
}

#[test]
fn agent_tools_describe_observe_and_resource_read() {
    let tools = agent_tool_descriptors();

    assert!(tools.iter().any(|tool| tool.name == "arcweft.observe"));
    let resource_read = tools
        .iter()
        .find(|tool| tool.name == "arcweft.resource.read")
        .expect("resource read tool is listed");
    assert_eq!(
        resource_read.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
    assert_eq!(
        resource_read.input_schema["properties"]["path"]["type"],
        serde_json::json!("string")
    );
    assert!(tools.iter().any(|tool| tool.name == "arcweft.capture"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.hit_test"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.session.info"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.get_state"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.signal_get"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.log_query"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.rag.query"));
    assert!(tools.iter().any(|tool| tool.name == "arcweft.rag.explain"));
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.rag.context.read")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.script.runs")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.sessions.close_stale")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.session.timeline")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.repl.cells")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.source.files")
    );
    assert!(
        tools
            .iter()
            .any(|tool| tool.name == "arcweft.debug.graph.inventory")
    );
    assert!(tools.iter().any(|tool| tool.name == "arcweft.trace.read"));
}

#[test]
fn tool_schemas_expose_image_capture_scope_and_uri() {
    let tools = agent_tool_descriptors();
    let observe = tools
        .iter()
        .find(|tool| tool.name == "arcweft.observe")
        .expect("observe tool is described");
    let properties = &observe.input_schema["properties"];

    assert_eq!(
        properties["image"]["enum"],
        serde_json::json!(["overlay", "png", "raw-rgba"])
    );
    assert_eq!(
        properties["capture"]["enum"],
        serde_json::json!(["color", "object-id", "mask"])
    );
    assert!(properties.get("renderer").is_none());
    assert_eq!(properties["source"]["type"], "string");
    assert_eq!(properties["manifest"]["type"], "string");
    assert_eq!(properties["profile"]["type"], "string");
    assert_eq!(
        observe.input_schema["anyOf"],
        serde_json::json!([
            { "required": ["source"] },
            { "required": ["profile"] }
        ])
    );
    assert_eq!(properties["layer"]["type"], "string");
    assert_eq!(properties["object"]["type"], "string");
    assert_eq!(properties["page"]["type"], "integer");
    assert_eq!(properties["page"]["minimum"], 0);
    assert_eq!(properties["capture_time"]["type"], "number");
    assert_eq!(properties["capture_time"]["minimum"], 0);
    assert_capture_time_description_mentions_animated_presentation_objects(
        &properties["capture_time"],
        true,
    );
    assert_eq!(properties["capture_step"]["type"], "integer");
    assert_eq!(properties["capture_step"]["minimum"], 1);
    assert_eq!(properties["viewport_width"]["type"], "integer");
    assert_eq!(properties["viewport_width"]["minimum"], 1);
    assert_eq!(properties["viewport_height"]["type"], "integer");
    assert_eq!(properties["viewport_height"]["minimum"], 1);
    assert_eq!(properties["textbox_height"]["type"], "integer");
    assert_eq!(properties["textbox_height"]["minimum"], 1);

    let capture = tools
        .iter()
        .find(|tool| tool.name == "arcweft.capture")
        .expect("capture tool is described");
    let properties = &capture.input_schema["properties"];
    assert_eq!(properties["uri"]["type"], "string");
    assert_eq!(properties["source"]["type"], "string");
    assert_eq!(properties["manifest"]["type"], "string");
    assert_eq!(properties["profile"]["type"], "string");
    assert!(properties.get("renderer").is_none());
    assert_eq!(
        properties["format"]["enum"],
        serde_json::json!(["png", "raw-rgba"])
    );
    assert_eq!(properties["page"]["type"], "integer");
    assert_eq!(properties["page"]["minimum"], 0);
    assert_eq!(properties["capture_time"]["type"], "number");
    assert_eq!(properties["capture_time"]["minimum"], 0);
    assert_capture_time_description_mentions_animated_presentation_objects(
        &properties["capture_time"],
        true,
    );
    assert_eq!(properties["capture_step"]["type"], "integer");
    assert_eq!(properties["capture_step"]["minimum"], 1);
    assert_eq!(properties["viewport_width"]["type"], "integer");
    assert_eq!(properties["viewport_width"]["minimum"], 1);
    assert_eq!(properties["viewport_height"]["type"], "integer");
    assert_eq!(properties["viewport_height"]["minimum"], 1);
    assert_eq!(properties["textbox_height"]["type"], "integer");
    assert_eq!(properties["textbox_height"]["minimum"], 1);
}

#[test]
fn hit_test_tool_schema_requires_viewport_coordinate() {
    let tools = agent_tool_descriptors();
    let hit_test = tools
        .iter()
        .find(|tool| tool.name == "arcweft.hit_test")
        .expect("hit-test tool is described");
    let properties = &hit_test.input_schema["properties"];

    assert_eq!(
        hit_test.input_schema["required"],
        serde_json::json!(["x", "y"])
    );
    assert_eq!(properties["x"]["type"], "integer");
    assert_eq!(properties["x"]["minimum"], 0);
    assert_eq!(properties["y"]["type"], "integer");
    assert_eq!(properties["y"]["minimum"], 0);
    assert_eq!(properties["capture_time"]["type"], "number");
    assert_capture_time_description_mentions_animated_presentation_objects(
        &properties["capture_time"],
        false,
    );
    assert_eq!(properties["capture_step"]["minimum"], 1);
}

#[test]
fn debug_read_tool_schemas_expose_state_signal_and_log_filters() {
    let tools = agent_tool_descriptors();
    let action = tools
        .iter()
        .find(|tool| tool.name == "arcweft.action")
        .expect("action tool is described");
    let action_alias = tools
        .iter()
        .find(|tool| tool.name == "arcweft.act")
        .expect("action alias tool is described");
    assert_eq!(action.input_schema, action_alias.input_schema);
    assert_eq!(
        action.input_schema["properties"]["kind"]["enum"],
        serde_json::json!(["advance_text", "select_choice", "invoke"])
    );
    assert_eq!(
        action.input_schema["properties"]["action_id"]["type"],
        "string"
    );
    assert_eq!(action.input_schema["properties"]["args"]["type"], "object");

    let step_frames = tools
        .iter()
        .find(|tool| tool.name == "arcweft.session.step_frames")
        .expect("step frames tool is described");
    assert_eq!(
        step_frames.input_schema["properties"]["count"]["minimum"],
        serde_json::json!(1)
    );
    assert_eq!(
        step_frames.input_schema["properties"]["count"]["default"],
        serde_json::json!(1)
    );

    let state = tools
        .iter()
        .find(|tool| tool.name == "arcweft.get_state")
        .expect("state tool is described");
    assert_eq!(state.input_schema["properties"]["path"]["type"], "string");
    assert_eq!(state.input_schema["properties"]["source"]["type"], "string");
    assert_eq!(
        state.input_schema["properties"]["profile"]["type"],
        "string"
    );

    let signal = tools
        .iter()
        .find(|tool| tool.name == "arcweft.signal_get")
        .expect("signal tool is described");
    assert_eq!(signal.input_schema["required"], serde_json::json!(["name"]));
    assert_eq!(signal.input_schema["properties"]["name"]["type"], "string");

    let logs = tools
        .iter()
        .find(|tool| tool.name == "arcweft.log_query")
        .expect("log query tool is described");
    assert_eq!(logs.input_schema["properties"]["level"]["type"], "string");
    assert_eq!(
        logs.input_schema["properties"]["contains"]["type"],
        "string"
    );
    assert_eq!(logs.input_schema["properties"]["limit"]["minimum"], 0);

    assert_debug_search_tool_schema(&tools);

    let rag = tools
        .iter()
        .find(|tool| tool.name == "arcweft.rag.query")
        .expect("rag query tool is described");
    assert_eq!(rag.input_schema["required"], serde_json::json!(["query"]));
    assert_eq!(rag.input_schema["properties"]["query"]["type"], "string");
    assert_eq!(rag.input_schema["properties"]["roots"]["type"], "array");
    assert_eq!(rag.input_schema["properties"]["graph_depth"]["minimum"], 0);
    assert_eq!(rag.input_schema["properties"]["limit"]["minimum"], 1);
    assert_eq!(
        rag.input_schema["properties"]["max_context_bytes"]["minimum"],
        1
    );
    assert_eq!(
        rag.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
    assert_eq!(
        rag.input_schema["properties"]["local_embedding"]["type"],
        "boolean"
    );
    assert_eq!(
        rag.input_schema["properties"]["local_embedding_model_id"]["type"],
        "string"
    );
    assert_eq!(
        rag.input_schema["properties"]["local_embedding_model_revision"]["type"],
        "string"
    );
    assert_eq!(
        rag.input_schema["properties"]["local_embedding_dimensions"]["minimum"],
        1
    );
}

fn assert_debug_search_tool_schema(tools: &[McpToolDescriptor]) {
    let debug_search = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.search")
        .expect("debug search tool is described");
    assert_eq!(
        debug_search.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["query"]["type"],
        "string"
    );
    assert!(debug_search.input_schema["properties"]["query_vector"]["oneOf"].is_array());
    assert_eq!(
        debug_search.input_schema["properties"]["graph_query"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["graph_depth"]["minimum"],
        0
    );
    assert_eq!(
        debug_search.input_schema["properties"]["history_query"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["diagnostic_query"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["test_query"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["model_id"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["model_revision"]["type"],
        "string"
    );
    assert_eq!(
        debug_search.input_schema["properties"]["limit"]["minimum"],
        1
    );
    assert_eq!(
        debug_search.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
}

#[test]
fn rag_query_tool_schema_exposes_source_project_inputs() {
    let tools = agent_tool_descriptors();
    let rag = tools
        .iter()
        .find(|tool| tool.name == "arcweft.rag.query")
        .expect("rag query tool is described");
    assert_eq!(rag.input_schema["properties"]["source"]["type"], "string");
    assert_eq!(
        rag.input_schema["properties"]["sources"]["oneOf"][0]["type"],
        "string"
    );
    assert_eq!(
        rag.input_schema["properties"]["sources"]["oneOf"][1]["items"]["type"],
        "string"
    );
}

#[test]
fn rag_context_and_timeline_tool_schemas_are_described() {
    let tools = agent_tool_descriptors();
    let rag_explain = tools
        .iter()
        .find(|tool| tool.name == "arcweft.rag.explain")
        .expect("rag explain tool is described");
    assert_eq!(
        rag_explain.input_schema["properties"]["query_id"]["type"],
        "string"
    );

    let context_read = tools
        .iter()
        .find(|tool| tool.name == "arcweft.rag.context.read")
        .expect("rag context read tool is described");
    assert_eq!(
        context_read.input_schema["required"],
        serde_json::json!(["chunk_id"])
    );
    assert_eq!(
        context_read.input_schema["properties"]["max_bytes"]["minimum"],
        1
    );

    let script_runs = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.script.runs")
        .expect("debug script runs tool is described");
    assert_eq!(
        script_runs.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        script_runs.input_schema["properties"]["session_id"]["type"],
        "string"
    );
    assert_eq!(
        script_runs.input_schema["properties"]["limit"]["minimum"],
        1
    );
    assert_eq!(
        script_runs.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
    assert_eq!(
        script_runs.input_schema["properties"]["max_privacy"]["default"],
        "project"
    );

    let close_stale = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.sessions.close_stale")
        .expect("debug close stale sessions tool is described");
    assert_eq!(
        close_stale.input_schema["required"],
        serde_json::json!(["stale_after_millis"])
    );
    assert_eq!(
        close_stale.input_schema["properties"]["stale_after_millis"]["minimum"],
        1
    );
    assert_eq!(
        close_stale.input_schema["properties"]["dry_run"]["type"],
        "boolean"
    );

    let timeline = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.session.timeline")
        .expect("debug timeline tool is described");
    assert_eq!(
        timeline.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        timeline.input_schema["properties"]["session_id"]["type"],
        "string"
    );
    assert_eq!(
        timeline.input_schema["properties"]["run_id"]["type"],
        "string"
    );
    assert_eq!(timeline.input_schema["properties"]["limit"]["minimum"], 1);
    assert_eq!(
        timeline.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );

    let repl_cells = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.repl.cells")
        .expect("debug REPL cells tool is described");
    assert_eq!(
        repl_cells.input_schema["required"],
        serde_json::json!(["session_id"])
    );
    assert_eq!(
        repl_cells.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        repl_cells.input_schema["properties"]["session_id"]["type"],
        "string"
    );
    assert_eq!(repl_cells.input_schema["properties"]["limit"]["minimum"], 1);
}

#[test]
fn debug_source_files_tool_schema_is_described() {
    let tools = agent_tool_descriptors();
    let source_files = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.source.files")
        .expect("debug source files tool is described");
    assert_eq!(
        source_files.input_schema["required"],
        serde_json::json!(["program_hash"])
    );
    assert_eq!(
        source_files.input_schema["properties"]["path"]["type"],
        "string"
    );
    assert_eq!(
        source_files.input_schema["properties"]["program_hash"]["type"],
        "string"
    );
    assert_eq!(
        source_files.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
    assert_eq!(
        source_files.input_schema["properties"]["max_privacy"]["default"],
        "project"
    );
}

#[test]
fn debug_graph_inventory_tool_schema_is_described() {
    let tools = agent_tool_descriptors();
    let graph = tools
        .iter()
        .find(|tool| tool.name == "arcweft.debug.graph.inventory")
        .expect("debug graph inventory tool is described");
    assert_eq!(
        graph.input_schema["required"],
        serde_json::json!(["program_hash"])
    );
    assert_eq!(graph.input_schema["properties"]["path"]["type"], "string");
    assert_eq!(
        graph.input_schema["properties"]["program_hash"]["type"],
        "string"
    );
    assert_eq!(
        graph.input_schema["properties"]["max_privacy"]["enum"],
        serde_json::json!(["public", "project", "sensitive", "secret"])
    );
    assert_eq!(
        graph.input_schema["properties"]["max_privacy"]["default"],
        "project"
    );
}

#[test]
fn debug_read_tool_schemas_expose_max_privacy() {
    let tools = agent_tool_descriptors();
    for name in [
        "arcweft.get_state",
        "arcweft.signal_get",
        "arcweft.log_query",
    ] {
        let tool = tools
            .iter()
            .find(|tool| tool.name == name)
            .expect("debug read tool is described");
        assert_eq!(
            tool.input_schema["properties"]["max_privacy"]["enum"],
            serde_json::json!(["public", "project", "sensitive", "secret"])
        );
    }
}

fn assert_capture_time_description_mentions_animated_presentation_objects(
    property: &serde_json::Value,
    includes_image_capture: bool,
) {
    let description = property["description"]
        .as_str()
        .expect("capture_time description");
    assert!(description.contains("animation sample time"));
    assert!(description.contains("motion functions"));
    assert!(description.contains("typewriter visibility"));
    assert!(description.contains("animated proxy bounds"));
    assert!(description.contains("animated image frame selection"));
    if includes_image_capture {
        assert!(description.contains("image capture"));
    } else {
        assert!(description.contains("before hit-testing"));
    }
}

fn trace_records_fixture() -> Vec<AgentTraceRecord> {
    vec![
        trace_record(0, AgentTraceKind::RunStarted, "blake3:run-started-payload"),
        trace_record(
            1,
            AgentTraceKind::RunFinished,
            "blake3:run-finished-payload",
        ),
    ]
}

fn trace_record(sequence: u64, kind: AgentTraceKind, payload_hash: &str) -> AgentTraceRecord {
    AgentTraceRecord {
        schema_version: 1,
        run_id: AgentRunId::new("run.cli").expect("test run id"),
        session_id: Some(SessionId::new("session.cli").expect("test session id")),
        sequence,
        tick: None,
        kind,
        payload_hash: StableHash::new(payload_hash).expect("test trace hash"),
        payload: serde_json::json!({ "sequence": sequence }),
        blob_refs: Vec::new(),
    }
}
