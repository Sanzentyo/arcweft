use crate::{
    model::{
        McpBlobResourceContents, McpContentBlock, McpResourceContents, McpTextResourceContents,
        McpToolDescriptor,
    },
    resources::{
        list_resource_templates_result, list_resources_result, read_resource_result,
        resource_descriptor, resource_link, tool_result_for_resource, tool_result_for_resources,
    },
    tools::agent_tool_descriptors,
};

use arcweft_agent_policy::{
    AgentContentPolicyGate, AgentPublicationPolicy, PublishedAgentResource,
};
use arcweft_agent_protocol::{
    diagnostic::{AgentDiagnostic, AgentDiagnosticSeverity},
    geometry::{AgentCoordinateSpace, AgentViewport},
    ids::{AgentResourceUri, AgentRunId, SessionId, StableHash},
    image::{
        AgentImageComposition, AgentImageContentBBox, AgentImageCropOrigin, AgentImageKind,
        AgentImageMetadata, AgentImageRenderer, AgentImageScope,
    },
    observation::AgentObservationReport,
    presentation::AgentPresentationTree,
    resource::{
        AGENT_TRACE_MIME_TYPE, AgentBinaryEncoding, AgentBinaryResourceBody, AgentResource,
        AgentResourceBody, AgentResourceKind, TraceResourceError, trace_resource,
    },
    session::AgentAudioState,
    trace::{AgentTraceKind, AgentTraceRecord},
    view::AgentViewTree,
};
use arcweft_content_policy::{
    ClassificationReport, ClassifierIdentity, ClassifierRun, ContentClassifier,
    ContentPolicyEngine, PolicyInputRef, PolicyProfile,
};

fn resource_uri(value: &str) -> AgentResourceUri {
    AgentResourceUri::new(value).expect("test resource URI is nonempty")
}

#[derive(Clone, Debug)]
struct AllowAllClassifier;

impl ContentClassifier for AllowAllClassifier {
    fn identity(&self) -> ClassifierIdentity {
        ClassifierIdentity::new("arcweft.test.allow", "2026-06-22")
    }

    fn classify(
        &self,
        _input: PolicyInputRef<'_>,
    ) -> Result<ClassificationReport, arcweft_content_policy::PolicyError> {
        Ok(ClassificationReport {
            findings: Vec::new(),
            runs: vec![ClassifierRun::complete(self.identity())],
        })
    }
}

fn publish(resource: AgentResource) -> PublishedAgentResource {
    publish_with_auxiliary_policy(resource, true)
}

fn publish_with_auxiliary_policy(
    resource: AgentResource,
    publish_auxiliary_images: bool,
) -> PublishedAgentResource {
    let gate = AgentContentPolicyGate::with_publication_policy(
        ContentPolicyEngine::new(AllowAllClassifier, PolicyProfile::strict_default()),
        AgentPublicationPolicy {
            publish_auxiliary_images,
        },
    );
    gate.publish(resource).expect("test resource publishes")
}

fn has_extension(name: &str, extension: &str) -> bool {
    std::path::Path::new(name)
        .extension()
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(extension))
}

const PNG_1X1_BASE64: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGNgYGD4DwABBAEAX+XDSwAAAABJRU5ErkJggg==";
const MCP_PROXY_TEXT: &str = "abcdefghijproxy";

/// Deterministic 6x8 shaping metrics for the transport/policy fixture.
struct McpTextFixtureShaper;

impl arcweft_text_layout::TextShaper for McpTextFixtureShaper {
    type Error = std::convert::Infallible;

    fn font_inventory_hash(&self) -> arcweft_text_layout::FontInventoryHash {
        let face = arcweft_text_layout::FontFaceId::derive(b"mcp-fixture-glyphs", 0, &[]);
        arcweft_text_layout::FontInventoryHash::derive([face], std::iter::empty())
    }

    fn shape_run(
        &mut self,
        request: arcweft_text_layout::TextShapeRequest<'_>,
    ) -> Result<arcweft_text_layout::ShapedTextRun, Self::Error> {
        use arcweft_text_layout::{
            FontFaceId, LayoutPoint, LayoutRect, LayoutSize, ShapedGlyphKey, ShapedTextGlyph,
            ShapedTextRun,
        };
        let face = FontFaceId::derive(b"mcp-fixture-glyphs", 0, &[]);
        let glyphs = request
            .text
            .char_indices()
            .enumerate()
            .map(|(index, (offset, ch))| ShapedTextGlyph {
                key: ShapedGlyphKey {
                    face,
                    glyph_id: u32::from(ch),
                    font_size_bits: 8.0_f32.to_bits(),
                    font_weight: 400,
                    flags: 0,
                },
                source_range: arcweft_text_model::RichTextRange::new(
                    request.source_range.start + offset,
                    request.source_range.start + offset + ch.len_utf8(),
                ),
                line_index: 0,
                cluster_index: u32::try_from(index).expect("short fixture ordinal fits u32"),
                offset: LayoutPoint::default(),
                advance: LayoutSize::new(6.0, 0.0),
                ink_bounds: LayoutRect::new(0.0, 0.0, 6.0, 8.0),
            })
            .collect::<Vec<_>>();
        let width = glyphs.iter().map(|glyph| glyph.advance.width).sum();
        Ok(ShapedTextRun::new(
            glyphs,
            LayoutSize::new(width, 0.0),
            Some(LayoutRect::new(0.0, 0.0, width, 8.0)),
        ))
    }
}

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
        script_run.input_schema["properties"]["view_values"]["type"],
        "object"
    );
    assert!(
        script_run.input_schema["properties"]
            .get("values")
            .is_none()
    );
    assert_eq!(script_run.input_schema["additionalProperties"], false);
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
        uri: resource_uri("arcweft://session/cli/frame/0/layer.dialogue.png"),
        kind: AgentResourceKind::Image,
        mime_type: "image/png".to_owned(),
        hash: "hash".to_owned(),
        image: Some(AgentImageMetadata {
            text_contexts: None,
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
            view: None,
            selected_capture: None,
            diagnostics: Vec::new(),
        }),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: PNG_1X1_BASE64.to_owned(),
        }),
    };
    let resource = publish(resource);

    let descriptor = resource_descriptor(&resource);
    let read = read_resource_result(&resource).expect("resource read serializes");
    let tool = tool_result_for_resource(&resource).expect("tool result serializes");

    assert!(has_extension(&descriptor.name, "png"));
    assert_eq!(descriptor.mime_type.as_deref(), Some("image/png"));
    assert!(descriptor.size.is_some_and(|size| size > 8));
    let description = descriptor.description.as_deref().unwrap();
    assert!(description.contains("kind=color"));
    assert!(description.contains("renderer=native"));
    assert!(description.contains("scope=layer:layer."));
    assert!(description.contains("composition=masked_framebuffer_crop"));
    assert_eq!(
        AgentImageComposition::ObjectIdAttachment.as_str(),
        "object_id_attachment"
    );
    assert_eq!(
        AgentImageComposition::MaskAttachment.as_str(),
        "mask_attachment"
    );
    assert!(description.contains("width=320"));
    assert!(description.contains("height=180"));
    assert!(matches!(
        read.contents.as_slice(),
        [McpResourceContents::Blob(McpBlobResourceContents { blob, .. })] if blob == PNG_1X1_BASE64
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
            && text.contains("\"id\":\"layer.")
            && text.contains("\"composition\":\"masked_framebuffer_crop\"")
            && text.contains("\"crop_origin\"")
            && text.contains("\"content_viewport_bbox\"")
            && text.contains("\"content_pixels\":512")
            && text.contains("\"content_policy\"")
            && data == PNG_1X1_BASE64
            && mime_type == "image/png"
    ));
}

#[test]
fn image_tool_content_preserves_object_rich_text_ref_metadata() {
    let metadata = proxy_object_image_metadata_fixture();
    let metadata = AgentImageMetadata::decode_json(
        &serde_json::to_vec(&metadata).expect("issued proxy metadata serializes"),
    )
    .expect("issued proxy metadata roundtrips through its checked owner");
    let object = metadata
        .object
        .as_ref()
        .expect("issued proxy object is present");
    let element = object
        .rich_text_ref
        .as_ref()
        .expect("proxy child is admitted");
    assert_eq!(element.range.start, 10);
    assert_eq!(element.range.end, 15);
    let proxy = &element.presentation.as_ref().unwrap().object_proxies[0];
    assert_eq!(proxy.type_name.as_deref(), Some("KeywordHit"));
    assert_eq!(proxy.depth.map(|depth| depth.0), Some(4000));
    assert_eq!(proxy.fields[0].name, "channel");
    assert_eq!(
        proxy.fields[0].value,
        arcweft_text_model::RichTextTextProxyScalar::Text {
            value: "choice".to_owned(),
        }
    );
    let capture_uri = object.capture_refs.captures[0].uri.clone();
    let resource = AgentResource {
        uri: resource_uri(&capture_uri),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "hash".to_owned(),
        image: Some(metadata),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: "AAAAAA==".to_owned(),
        }),
    };
    let resource = publish(resource);

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
    assert!(json["image"]["object"].is_null());
    assert!(json["image"]["text_contexts"].is_null());
    assert!(
        json["image"]["scope"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("object."))
    );
    assert!(json["content_policy"].is_object());
}

#[test]
fn image_tool_content_preserves_image_object_frame_metadata() {
    let metadata: AgentImageMetadata =
        serde_json::from_value(image_object_frame_metadata_fixture())
            .expect("image object frame metadata deserializes");
    let resource = AgentResource {
        uri: resource_uri(
            "arcweft://session/cli/frame/0/object.object.image.layer.foreground.0.1.rgba",
        ),
        kind: AgentResourceKind::Image,
        mime_type: "application/octet-stream".to_owned(),
        hash: "hash".to_owned(),
        image: Some(metadata),
        body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
            encoding: AgentBinaryEncoding::Base64,
            data: "AAAAAA==".to_owned(),
        }),
    };
    let resource = publish(resource);

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
    assert!(json["image"]["object"].is_null());
    assert!(
        json["image"]["scope"]["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("object."))
    );
    assert!(json["content_policy"].is_object());
}

fn proxy_document_layout_fixture() -> arcweft_text_layout::TextLayout {
    use arcweft_render_text::{
        ResolvedTextDocument, ResolvedTextRun, ResolvedTextRunSource, ResolvedTextStyle,
        TextDocumentRevision, TextFontFamily,
    };
    use arcweft_text_model::{
        Milli, RichTextObjectProxy, RichTextObjectProxyDeclaration, RichTextPresentation,
        RichTextRange, RichTextTextProxyField, RichTextTextProxyFieldKind,
        RichTextTextProxyFieldSchema, RichTextTextProxyScalar, RichTextTextProxySchema,
    };
    let declaration = RichTextObjectProxyDeclaration {
        struct_name: "KeywordHit".to_owned(),
        attribute: "text_proxy".to_owned(),
    };
    let value = RichTextTextProxyScalar::Text {
        value: "choice".to_owned(),
    };
    let presentation = RichTextPresentation {
        object_proxies: vec![RichTextObjectProxy {
            id: "hotspot".to_owned(),
            schema: Some(RichTextTextProxySchema {
                id: "KeywordHit".to_owned(),
                declaration: declaration.clone(),
                fields: vec![RichTextTextProxyFieldSchema {
                    id: 0,
                    name: "channel".to_owned(),
                    kind: RichTextTextProxyFieldKind::Text,
                    optional: false,
                    default: Some(value.clone()),
                }],
            }),
            declaration: Some(declaration),
            type_name: Some("KeywordHit".to_owned()),
            role: Some("keyword".to_owned()),
            layer: None,
            depth: Some(Milli(4000)),
            hit_test: true,
            fields: vec![RichTextTextProxyField {
                id: 0,
                name: "channel".to_owned(),
                value,
            }],
        }],
        ..RichTextPresentation::default()
    };
    let style =
        ResolvedTextStyle::new(vec![TextFontFamily::SansSerif], 8000, 8000).expect("fixture style");
    let runs = [
        (RichTextRange::new(0, 10), RichTextPresentation::default()),
        (RichTextRange::new(10, 15), presentation),
    ]
    .into_iter()
    .map(|(range, presentation)| {
        ResolvedTextRun::new(
            range,
            range,
            style.clone(),
            presentation,
            ResolvedTextRunSource::Plain,
        )
        .expect("checked fixture run")
    })
    .collect();
    let document = ResolvedTextDocument::new(
        MCP_PROXY_TEXT,
        0,
        runs,
        Vec::new(),
        TextDocumentRevision::new(1),
    )
    .expect("checked complete fixture document");
    arcweft_text_layout::layout_document(
        &document,
        arcweft_text_layout::TextLayoutRequest {
            origin: arcweft_text_layout::LayoutPoint::new(60.0, 520.0),
            ..arcweft_text_layout::TextLayoutRequest::default()
        },
        &mut McpTextFixtureShaper,
    )
    .expect("fixture passes the owning layout compiler")
}

fn proxy_text_context_fixture() -> (
    arcweft_agent_protocol::text_context::AgentTextContextCatalog,
    arcweft_agent_protocol::rich_text::AgentRichTextElementRef,
    arcweft_agent_protocol::geometry::AgentBBox,
) {
    use arcweft_agent_protocol::{
        geometry::AgentBBox,
        rich_text::{
            AgentHitRegion, AgentHitRegionKind, AgentRichTextElementKind, AgentRichTextElementRef,
        },
        text_context::{
            AgentTextContextCatalog, AgentTextContextContent, AgentTextContextEntry,
            AgentTextContextOwner, AgentTextContextScope,
        },
    };
    use arcweft_text_model::{
        PreparedTextDocumentContext, PreparedTextOwnerKind, RichTextNodeIndex, RichTextRange,
    };
    let layout = std::sync::Arc::new(proxy_document_layout_fixture());
    assert_eq!(
        layout.runs[1].bounds,
        arcweft_text_layout::LayoutRect::new(120.0, 520.0, 30.0, 8.0)
    );
    let presentation = layout.runs[1].presentation.clone();
    let proxy = &presentation.object_proxies[0];
    let bbox = AgentBBox {
        space: AgentCoordinateSpace::Viewport,
        x: 120,
        y: 520,
        width: 30,
        height: 8,
    };
    let range = RichTextRange::new(10, 15);
    let region = AgentHitRegion {
        kind: AgentHitRegionKind::TextObjectProxy,
        bbox: bbox.clone(),
        range,
        proxy_id: Some(proxy.id.clone()),
        proxy_type: proxy.type_name.clone(),
        proxy_declaration: proxy.declaration.clone(),
        proxy_schema: proxy.schema.clone(),
        proxy_fields: proxy.fields.clone(),
        proxy_role: proxy.role.clone(),
        proxy_layer: proxy.layer.clone(),
        depth: proxy.depth.map(|depth| depth.0),
        proxy_params: std::collections::BTreeMap::new(),
    };
    let element = AgentRichTextElementRef {
        kind: AgentRichTextElementKind::TextObjectProxy,
        index: 0,
        page: 0,
        range,
        node_index: RichTextNodeIndex::new(0),
        source: None,
        ruby: None,
        presentation: Some(presentation),
        orientation: None,
        vertical_form: None,
        ruby_base_bbox: None,
        ruby_annotation_bbox: None,
        object_layer: None,
        object_depth: Some(4000),
        hit_test: true,
        hit_regions: vec![region],
    };
    let catalog = AgentTextContextCatalog::try_new(
        AgentTextContextScope {
            session_id: "cli".to_owned(),
            frame_id: "frame.0".to_owned(),
            tick: 0,
            capture_time_millis: Some(0),
        },
        [AgentTextContextEntry {
            owner_ordinal: 0,
            owner: AgentTextContextOwner {
                root_id: "object.dialogue.0.0".to_owned(),
                parent_id: None,
                semantic_id: "view.mcp.proxy-fixture".to_owned(),
                prepared_text: 0,
                kind: PreparedTextOwnerKind::View { mount: 0 },
            },
            content: AgentTextContextContent::PreparedDocument {
                context: PreparedTextDocumentContext::try_new(0, MCP_PROXY_TEXT.len())
                    .expect("checked prepared document extent"),
                text: MCP_PROXY_TEXT.to_owned(),
                layout,
            },
        }],
    )
    .expect("checked context/layout catalog");
    (catalog, element, bbox)
}

fn proxy_object_image_metadata_fixture() -> AgentImageMetadata {
    use arcweft_agent_protocol::{
        geometry::AgentRgbaColor,
        image::{
            AgentImageObjectRef, AgentImagePixelFormat, AgentObjectCaptureRef,
            AgentObjectCaptureRefs,
        },
        object::{AgentObservedObjectContent, AgentTextObjectContent},
    };
    let (catalog, element, bbox) = proxy_text_context_fixture();
    let reference = catalog.reference(0).expect("issued fixture owner");
    let entry = catalog.resolve(&reference).unwrap();
    let id = entry.element_id(&element).expect("issued proxy descriptor");
    let parent_id = entry
        .element_parent_id(Some(&element))
        .expect("issued proxy parent");
    let uri = format!(
        "arcweft://session/{}/frame/{}/object.{id}.mask.rgba",
        catalog.scope().session_id,
        catalog.scope().tick
    );
    let object = AgentImageObjectRef {
        id: id.clone(),
        parent_id,
        entity: None,
        layer: "dialogue.rich_text".to_owned(),
        role: "rich_text_proxy".to_owned(),
        polygon: bbox.polygon(),
        bbox: bbox.clone(),
        capture_refs: AgentObjectCaptureRefs {
            object_id_color: AgentRgbaColor {
                red: 10,
                green: 20,
                blue: 30,
                alpha: 255,
            },
            captures: vec![AgentObjectCaptureRef {
                kind: AgentImageKind::Mask,
                uri,
                mime_type: "application/octet-stream".to_owned(),
                pixel_format: Some(AgentImagePixelFormat::Rgba8Unorm),
                page: 0,
                width: bbox.width,
                height: bbox.height,
                selected_capture: None,
            }],
        },
        object_layer: None,
        object_depth: Some(4000),
        text: Some("proxy".to_owned()),
        text_context: Some(reference),
        rich_text_ref: Some(element),
        content: AgentObservedObjectContent::ViewText(AgentTextObjectContent::default()),
    };
    object
        .validate_text_context(&catalog)
        .expect("proxy matches its actual context and layout");
    AgentImageMetadata {
        kind: AgentImageKind::Mask,
        renderer: AgentImageRenderer::Native,
        scope: AgentImageScope::Object { id },
        composition: AgentImageComposition::MaskAttachment,
        page: 0,
        capture_step: 1,
        capture_time_millis: 0,
        width: 1,
        height: 1,
        crop_origin: None,
        pixel_format: Some(AgentImagePixelFormat::Rgba8Unorm),
        row_stride_bytes: Some(4),
        content_bbox: None,
        content_viewport_bbox: None,
        content_pixels: Some(1),
        object: Some(object),
        view: None,
        selected_capture: None,
        diagnostics: Vec::new(),
        text_contexts: Some(catalog),
    }
}

fn image_object_frame_metadata_fixture() -> serde_json::Value {
    serde_json::from_str(
            r#"{
                "kind": "color",
                "renderer": "native",
                "scope": { "kind": "object", "id": "object.image.layer.foreground.0.1" },
                "composition": "framebuffer_crop",
                "width": 1,
                "height": 1,
                "pixel_format": "rgba8_unorm",
                "row_stride_bytes": 4,
                "content_pixels": 1,
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
                    "content": {
                        "kind": "image",
                        "source": "view.image.1",
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
            uri: resource_uri("arcweft://session/cli/observation/latest.json"),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: "hash".to_owned(),
            image: None,
            body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
        },
        AgentResource {
            uri: resource_uri("arcweft://session/cli/frame/0/layer.dialogue.object-id.png"),
            kind: AgentResourceKind::Image,
            mime_type: "image/png".to_owned(),
            hash: "hash".to_owned(),
            image: Some(AgentImageMetadata {
                text_contexts: None,
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
                view: None,
                selected_capture: None,
                diagnostics: Vec::new(),
            }),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: PNG_1X1_BASE64.to_owned(),
            }),
        },
    ]
    .into_iter()
    .map(|resource| publish_with_auxiliary_policy(resource, false))
    .collect::<Vec<_>>();

    let list = list_resources_result(&resources);
    let tool = tool_result_for_resources(&resources);

    assert_eq!(list.resources.len(), 2);
    assert!(has_extension(&list.resources[1].name, "json"));
    assert_eq!(
        list.resources[1].mime_type.as_deref(),
        Some("application/json")
    );
    assert!(
        list.resources[1]
            .description
            .as_deref()
            .is_some_and(|description| description.contains("Agent Debug Bus resource"))
    );
    assert!(matches!(
        tool.content.as_slice(),
        [
            McpContentBlock::ResourceLink { name: first, .. },
            McpContentBlock::ResourceLink { name: second, mime_type: Some(mime_type), .. },
        ] if has_extension(first, "json") && has_extension(second, "json") && mime_type == "application/json"
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
fn same_run_trace_resource_maps_to_mcp_text_resource_and_link() {
    let records = trace_records_fixture();
    let resource = trace_resource(&records).expect("trace resource serializes");
    assert_eq!(resource.kind, AgentResourceKind::Trace);
    assert_eq!(resource.uri, "arcweft://run/run.cli/trace.arcwx");
    assert_eq!(resource.mime_type, AGENT_TRACE_MIME_TYPE);
    assert_eq!(resource.hash, "trace:run.cli:2:blake3:run-finished-payload");
    let resource = publish(resource);
    let list = list_resources_result(std::slice::from_ref(&resource));
    let read = read_resource_result(&resource).expect("trace resource reads");
    let tool = tool_result_for_resource(&resource).expect("trace tool result serializes");

    assert_eq!(list.resources[0].uri, "arcweft://run/run.cli/trace.arcwx");
    assert!(has_extension(&list.resources[0].name, "arcwx"));
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
        [McpContentBlock::Text { text }, McpContentBlock::Resource { resource: McpResourceContents::Text(McpTextResourceContents { uri, .. }) }]
            if uri == "arcweft://run/run.cli/trace.arcwx" && text.contains("\"content_policy\"")
    ));
}

#[test]
fn empty_trace_resource_uses_the_canonical_unknown_run_identity() {
    let resource = trace_resource(&[]).expect("empty trace resource serializes");

    assert_eq!(resource.uri, "arcweft://run/run.unknown/trace.arcwx");
    assert!(resource.has_canonical_public_uri());
    assert_eq!(resource.hash, "trace:empty");
    assert_eq!(
        resource.body,
        AgentResourceBody::Json(serde_json::json!([]))
    );
}

#[test]
fn mixed_run_trace_records_do_not_produce_a_resource() {
    let mut records = trace_records_fixture();
    records[1].run_id = AgentRunId::new("run.other").expect("test run id is canonical");

    let result = trace_resource(&records);
    let Err(TraceResourceError::MixedRun {
        expected,
        actual,
        index,
    }) = result
    else {
        panic!("mixed-run records must fail before producing a resource");
    };
    assert_eq!(expected.as_str(), "run.cli");
    assert_eq!(actual.as_str(), "run.other");
    assert_eq!(index, 1);
}

#[test]
fn json_agent_resource_maps_to_mcp_text_resource() {
    let resource = AgentResource {
        uri: resource_uri("arcweft://session/cli/observation/latest.json"),
        kind: AgentResourceKind::ObservationLatest,
        mime_type: "application/json".to_owned(),
        hash: "hash".to_owned(),
        image: None,
        body: AgentResourceBody::Json(serde_json::json!({ "status": "ok" })),
    };
    let resource = publish(resource);

    let read = read_resource_result(&resource).expect("resource read serializes");
    let link = resource_link(&resource);

    assert!(matches!(
        read.contents.as_slice(),
        [McpResourceContents::Text(McpTextResourceContents { text, .. })] if text == "{\"status\":\"ok\"}"
    ));
    assert!(matches!(
        link,
        McpContentBlock::ResourceLink { name, mime_type: Some(mime_type), .. }
            if has_extension(&name, "json") && mime_type == "application/json"
    ));
}

#[test]
fn runtime_assertion_diagnostic_survives_observation_policy_and_mcp_projection() {
    let report = AgentObservationReport {
        status: "failed".to_owned(),
        session_id: "assertion".to_owned(),
        tick: 7,
        frame_id: "frame.7".to_owned(),
        state_hash: "state.assertion.7".to_owned(),
        render_hash: "render.assertion.7".to_owned(),
        source: "game.arcw".to_owned(),
        viewport: AgentViewport {
            width: 1280,
            height: 720,
            scale: 1.0,
        },
        images: Vec::new(),
        layers: Vec::new(),
        views: Vec::new(),
        objects: Vec::new(),
        text_contexts: arcweft_agent_protocol::text_context::AgentTextContextCatalog::empty(
            arcweft_agent_protocol::text_context::AgentTextContextScope {
                session_id: "assertion".to_owned(),
                frame_id: "frame.7".to_owned(),
                tick: 7,
                capture_time_millis: None,
            },
        )
        .unwrap(),
        presentation_tree: AgentPresentationTree::from_layers_and_objects(&[], &[]),
        actions: Vec::new(),
        scroll_regions: Vec::new(),
        virtual_lists: Vec::new(),
        view_tree: AgentViewTree {
            root: "view.root".to_owned(),
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
        diagnostics: vec![AgentDiagnostic {
            step: 7,
            severity: AgentDiagnosticSeverity::Error,
            source: Some("game.arcw:12:9".to_owned()),
            code: Some("runtime.assertion_failed".to_owned()),
            effect_id: Some("assertion.guard.00112233445566778899aabbccddeeff".to_owned()),
            // Deliberately resembles a successful condition. MCP must transport
            // the projected diagnostic verbatim, never reinterpret message text.
            message: "assertion condition `true` failed".to_owned(),
        }],
        steps: 1,
        capture_time_millis: None,
        task_requests: 0,
        final_status: "failed".to_owned(),
        overlay_svg: None,
    };
    let resource = publish(
        report
            .observation_resource()
            .expect("typed assertion observation serializes"),
    );

    let read = read_resource_result(&resource).expect("MCP resource read serializes");
    let tool = tool_result_for_resource(&resource).expect("MCP tool resource serializes");
    let [
        McpResourceContents::Text(McpTextResourceContents {
            uri,
            mime_type,
            text,
            ..
        }),
    ] = read.contents.as_slice()
    else {
        panic!("assertion observation must remain one JSON text resource");
    };
    let json: serde_json::Value =
        serde_json::from_str(text).expect("MCP assertion observation is valid JSON");
    assert!(
        uri.starts_with("arcweft://moderated/") && uri.to_ascii_lowercase().ends_with(".json"),
        "policy-owned publication URI must remain a moderated JSON resource: {uri}"
    );
    assert_eq!(mime_type.as_deref(), Some("application/json"));
    assert_eq!(
        json["diagnostics"],
        serde_json::json!([{
            "step": 7,
            "severity": "error",
            "source": "game.arcw:12:9",
            "code": "runtime.assertion_failed",
            "effect_id": "assertion.guard.00112233445566778899aabbccddeeff",
            "message": "assertion condition `true` failed"
        }])
    );
    assert!(matches!(
        tool.content.as_slice(),
        [
            McpContentBlock::Text { .. },
            McpContentBlock::Resource {
                resource: McpResourceContents::Text(McpTextResourceContents { text, .. }),
            },
        ] if serde_json::from_str::<serde_json::Value>(text)
            .is_ok_and(|value| value["diagnostics"][0]["code"] == "runtime.assertion_failed"
                && value["diagnostics"][0]["message"] == "assertion condition `true` failed")
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
}

#[test]
fn tool_schemas_do_not_advertise_unconsumed_dialogue_view_height() {
    for tool in agent_tool_descriptors() {
        assert!(
            tool.input_schema["properties"]
                .as_object()
                .is_none_or(|properties| !properties.contains_key("dialogue_view_height")),
            "{} must expose only layout inputs consumed by the player",
            tool.name
        );
    }
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
        serde_json::json!(["advance_text", "select_choice", "invoke", "scroll"])
    );
    assert_eq!(
        action.input_schema["properties"]["action_id"]["type"],
        "string"
    );
    assert_eq!(action.input_schema["properties"]["args"]["type"], "object");
    assert_eq!(
        action.input_schema["properties"]["delta_x_milli"]["minimum"],
        i32::MIN
    );
    assert_eq!(
        action.input_schema["properties"]["delta_y_milli"]["maximum"],
        i32::MAX
    );

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
