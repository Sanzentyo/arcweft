//! MCP-facing adapters for Arcweft Agent Debug Bus resources.
//!
//! This crate is Sans I/O. It does not own stdio, HTTP, sessions, or renderer
//! readback. It maps `arcweft-agent-protocol` records into MCP-compatible JSON
//! shapes so CLI, tests, and a future MCP transport share one contract.

use arcweft_agent_protocol::{
    AgentImageComposition, AgentImageKind, AgentImageRenderer, AgentImageScope, AgentResource,
    AgentResourceBody, AgentResourceKind,
};
use serde::{Deserialize, Serialize};

/// Resource descriptor returned by MCP `resources/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpResourceDescriptor {
    pub uri: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// Result body returned by MCP `resources/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpListResourcesResult {
    pub resources: Vec<McpResourceDescriptor>,
}

/// Resource template descriptor returned by MCP `resources/templates/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpResourceTemplateDescriptor {
    #[serde(rename = "uriTemplate")]
    pub uri_template: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
}

/// Result body returned by MCP `resources/templates/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpListResourceTemplatesResult {
    #[serde(rename = "resourceTemplates")]
    pub resource_templates: Vec<McpResourceTemplateDescriptor>,
}

/// Result body returned by MCP `resources/read`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpReadResourceResult {
    pub contents: Vec<McpResourceContents>,
}

/// MCP resource content. Text resources carry `text`; binary resources carry
/// base64 `blob`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(untagged)]
pub enum McpResourceContents {
    Text(McpTextResourceContents),
    Blob(McpBlobResourceContents),
}

/// Text resource content for MCP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpTextResourceContents {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub text: String,
}

/// Binary resource content for MCP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpBlobResourceContents {
    pub uri: String,
    #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    pub blob: String,
}

/// Tool descriptor returned by MCP `tools/list`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpToolDescriptor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
}

/// Result returned by MCP `tools/call`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpCallToolResult {
    pub content: Vec<McpContentBlock>,
    #[serde(rename = "isError")]
    pub is_error: bool,
}

/// MCP content blocks relevant to Agent observation tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum McpContentBlock {
    Text {
        text: String,
    },
    Image {
        data: String,
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
    Resource {
        resource: McpResourceContents,
    },
    ResourceLink {
        uri: String,
        name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        #[serde(rename = "mimeType", skip_serializing_if = "Option::is_none")]
        mime_type: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        size: Option<u64>,
    },
}

/// Returns the current Agent Debug Bus tool descriptors.
pub fn agent_tool_descriptors() -> Vec<McpToolDescriptor> {
    vec![
        McpToolDescriptor {
            name: "arcweft.observe".to_owned(),
            title: Some("Observe Arcweft".to_owned()),
            description: "Runs a bounded Agent observation and returns resource links for the frame, objects, and optional image capture.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string" },
                    "image": { "type": "string", "enum": ["overlay", "png", "raw-rgba"] },
                    "capture": { "type": "string", "enum": ["color", "object-id", "mask"], "default": "color" },
                    "layer": { "type": "string" },
                    "object": { "type": "string" },
                    "page": { "type": "integer", "minimum": 0, "description": "0-based rendered page index for native rich-text captures." },
                    "capture_time": { "type": "number", "minimum": 0, "description": "Native capture time in seconds for visibility-only glyph effects such as typewriter." },
                    "steps": { "type": "integer", "minimum": 1 },
                    "max_ops": { "type": "integer", "minimum": 1 }
                },
                "required": ["source"]
            }),
        },
        McpToolDescriptor {
            name: "arcweft.resource.read".to_owned(),
            title: Some("Read Arcweft Resource".to_owned()),
            description: "Reads an arcweft:// Agent Debug Bus resource, including PNG/raw image blobs.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "uri": { "type": "string" }
                },
                "required": ["uri"]
            }),
        },
        McpToolDescriptor {
            name: "arcweft.capture".to_owned(),
            title: Some("Capture Arcweft Image".to_owned()),
            description: "Captures the latest observed viewport, layer, or object as PNG or raw RGBA image content; with source, observes first and then captures.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Optional .arcw source to observe before capturing." },
                    "entry": { "type": "string" },
                    "flow": { "type": "string" },
                    "steps": { "type": "integer", "minimum": 1 },
                    "max_ops": { "type": "integer", "minimum": 1 },
                    "uri": { "type": "string", "description": "Optional arcweft:// image resource URI from resources/list or resources/templates/list. When supplied, it selects format, capture kind, and viewport/layer/object scope." },
                    "format": { "type": "string", "enum": ["png", "raw-rgba"], "default": "png" },
                    "capture": { "type": "string", "enum": ["color", "object-id", "mask"], "default": "color" },
                    "layer": { "type": "string" },
                    "object": { "type": "string" },
                    "page": { "type": "integer", "minimum": 0, "description": "0-based rendered page index for native rich-text captures." },
                    "capture_time": { "type": "number", "minimum": 0, "description": "Native capture time in seconds for visibility-only glyph effects such as typewriter." }
                }
            }),
        },
        McpToolDescriptor {
            name: "arcweft.session.info".to_owned(),
            title: Some("Inspect Arcweft Session".to_owned()),
            description: "Returns the latest Agent Debug Bus session/frame state, available resources, and current image metadata.".to_owned(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        },
    ]
}

/// Returns the Agent Debug Bus resource templates understood by the current
/// one-shot CLI/MCP session model.
pub fn agent_resource_templates() -> Vec<McpResourceTemplateDescriptor> {
    vec![
        resource_template(
            "arcweft://session/{session_id}/observation/latest.json",
            "latest-observation",
            "Latest observation",
            "Latest Agent observation JSON, including viewport, layers, objects, actions, logs, signals, diagnostics, and image resource refs.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/objects.json",
            "observed-objects",
            "Observed objects",
            "Observed object JSON for the frame, including textbox and rich-text child bboxes plus object-local capture refs.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/{capture}.{extension}",
            "viewport-capture",
            "Viewport capture",
            "Full-frame image capture. capture is color, object-id, or mask; extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.{extension}",
            "layer-color-capture",
            "Layer color capture",
            "Selected layer color capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.object-id.{extension}",
            "layer-object-id-capture",
            "Layer object-id capture",
            "Selected layer object-id capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/layer.{layer_id}.mask.{extension}",
            "layer-mask-capture",
            "Layer mask capture",
            "Selected layer mask capture. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.{extension}",
            "object-color-capture",
            "Object color capture",
            "Selected object color capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.object-id.{extension}",
            "object-object-id-capture",
            "Object object-id capture",
            "Selected object object-id capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/object.{object_id}.mask.{extension}",
            "object-mask-capture",
            "Object mask capture",
            "Selected object mask capture, including rich-text child objects. extension is png or rgba.",
            None,
        ),
    ]
}

/// Converts the static Agent Debug Bus templates into an MCP
/// `resources/templates/list` result.
pub fn list_resource_templates_result() -> McpListResourceTemplatesResult {
    McpListResourceTemplatesResult {
        resource_templates: agent_resource_templates(),
    }
}

/// Converts an Agent resource into an MCP resource descriptor.
pub fn resource_descriptor(resource: &AgentResource) -> McpResourceDescriptor {
    let size = match &resource.body {
        AgentResourceBody::Json(value) => serde_json::to_vec(value)
            .ok()
            .and_then(|bytes| u64::try_from(bytes.len()).ok()),
        AgentResourceBody::Text(text) => u64::try_from(text.len()).ok(),
        AgentResourceBody::BytesBase64(body) => decoded_base64_len(&body.data),
    };
    McpResourceDescriptor {
        uri: resource.uri.clone(),
        name: resource_name(resource),
        title: Some(resource_title(resource)),
        description: Some(resource_description(resource)),
        mime_type: Some(resource.mime_type.clone()),
        size,
    }
}

fn resource_template(
    uri_template: &str,
    name: &str,
    title: &str,
    description: &str,
    mime_type: Option<&str>,
) -> McpResourceTemplateDescriptor {
    McpResourceTemplateDescriptor {
        uri_template: uri_template.to_owned(),
        name: name.to_owned(),
        title: Some(title.to_owned()),
        description: Some(description.to_owned()),
        mime_type: mime_type.map(ToOwned::to_owned),
    }
}

/// Converts Agent resources into an MCP `resources/list` result.
pub fn list_resources_result(resources: &[AgentResource]) -> McpListResourcesResult {
    McpListResourcesResult {
        resources: resources.iter().map(resource_descriptor).collect(),
    }
}

/// Converts an Agent resource into an MCP `resources/read` result.
pub fn read_resource_result(
    resource: &AgentResource,
) -> Result<McpReadResourceResult, serde_json::Error> {
    Ok(McpReadResourceResult {
        contents: vec![resource_contents(resource)?],
    })
}

/// Converts a set of Agent resources into an observe tool result. The result is
/// intentionally link-oriented so MCP clients can choose which image/blob to
/// fetch without embedding every frame resource in the initial tool response.
pub fn tool_result_for_resources(resources: &[AgentResource]) -> McpCallToolResult {
    McpCallToolResult {
        content: resources.iter().map(resource_link).collect(),
        is_error: false,
    }
}

/// Converts an Agent resource into a tool result. Image resources become MCP
/// image content so multimodal clients can render them directly.
pub fn tool_result_for_resource(
    resource: &AgentResource,
) -> Result<McpCallToolResult, serde_json::Error> {
    let content = match &resource.body {
        AgentResourceBody::BytesBase64(body) if resource.mime_type.starts_with("image/") => {
            let mut content = image_metadata_content(resource)?;
            content.push(McpContentBlock::Image {
                data: body.data.clone(),
                mime_type: resource.mime_type.clone(),
            });
            content
        }
        _ => {
            let mut content = image_metadata_content(resource)?;
            content.push(McpContentBlock::Resource {
                resource: resource_contents(resource)?,
            });
            content
        }
    };
    Ok(McpCallToolResult {
        content,
        is_error: false,
    })
}

fn image_metadata_content(
    resource: &AgentResource,
) -> Result<Vec<McpContentBlock>, serde_json::Error> {
    resource.image.as_ref().map_or_else(
        || Ok(Vec::new()),
        |metadata| {
            Ok(vec![McpContentBlock::Text {
                text: serde_json::to_string(&serde_json::json!({
                    "uri": resource.uri,
                    "mime_type": resource.mime_type,
                    "image": metadata,
                }))?,
            }])
        },
    )
}

/// Converts an Agent resource into an MCP content block link.
pub fn resource_link(resource: &AgentResource) -> McpContentBlock {
    let descriptor = resource_descriptor(resource);
    McpContentBlock::ResourceLink {
        uri: descriptor.uri,
        name: descriptor.name,
        title: descriptor.title,
        description: descriptor.description,
        mime_type: descriptor.mime_type,
        size: descriptor.size,
    }
}

fn resource_contents(resource: &AgentResource) -> Result<McpResourceContents, serde_json::Error> {
    match &resource.body {
        AgentResourceBody::Json(value) => Ok(McpResourceContents::Text(McpTextResourceContents {
            uri: resource.uri.clone(),
            mime_type: Some(resource.mime_type.clone()),
            text: serde_json::to_string(value)?,
        })),
        AgentResourceBody::Text(text) => Ok(McpResourceContents::Text(McpTextResourceContents {
            uri: resource.uri.clone(),
            mime_type: Some(resource.mime_type.clone()),
            text: text.clone(),
        })),
        AgentResourceBody::BytesBase64(body) => {
            Ok(McpResourceContents::Blob(McpBlobResourceContents {
                uri: resource.uri.clone(),
                mime_type: Some(resource.mime_type.clone()),
                blob: body.data.clone(),
            }))
        }
    }
}

fn resource_name(resource: &AgentResource) -> String {
    resource
        .uri
        .rsplit('/')
        .next()
        .filter(|name| !name.is_empty())
        .unwrap_or(resource.uri.as_str())
        .to_owned()
}

fn resource_title(resource: &AgentResource) -> String {
    match resource.kind {
        AgentResourceKind::ObservationLatest => "Latest observation",
        AgentResourceKind::Objects => "Observed objects",
        AgentResourceKind::OverlaySvg => "Overlay SVG",
        AgentResourceKind::Image => "Captured image",
        AgentResourceKind::Logs => "Runtime logs",
        AgentResourceKind::Signals => "Runtime signals",
        AgentResourceKind::Audio => "Audio state",
    }
    .to_owned()
}

fn resource_description(resource: &AgentResource) -> String {
    if let Some(image) = &resource.image {
        let page = if image.page == 0 {
            String::new()
        } else {
            format!(", page={}", image.page)
        };
        return format!(
            "Agent Debug Bus image resource (mime_type={}, kind={}, renderer={}, scope={}, composition={}{}, width={}, height={})",
            resource.mime_type,
            image_kind_description(image.kind),
            image_renderer_description(image.renderer),
            image_scope_description(&image.scope),
            image_composition_description(image.composition),
            page,
            image.width,
            image.height
        );
    }
    format!("Agent Debug Bus resource ({})", resource.mime_type)
}

fn image_scope_description(scope: &AgentImageScope) -> String {
    match scope {
        AgentImageScope::Viewport => "viewport".to_owned(),
        AgentImageScope::Layer { id } => format!("layer:{id}"),
        AgentImageScope::Object { id } => format!("object:{id}"),
    }
}

fn image_kind_description(kind: AgentImageKind) -> &'static str {
    match kind {
        AgentImageKind::Color => "color",
        AgentImageKind::Overlay => "overlay",
        AgentImageKind::OverlaySvg => "overlay_svg",
        AgentImageKind::ObjectId => "object_id",
        AgentImageKind::Mask => "mask",
    }
}

fn image_renderer_description(renderer: AgentImageRenderer) -> &'static str {
    match renderer {
        AgentImageRenderer::Native => "native",
    }
}

fn image_composition_description(composition: AgentImageComposition) -> &'static str {
    match composition {
        AgentImageComposition::OverlayVector => "overlay_vector",
        AgentImageComposition::Framebuffer => "framebuffer",
        AgentImageComposition::FramebufferCrop => "framebuffer_crop",
        AgentImageComposition::ObjectIdAttachment => "object_id_attachment",
        AgentImageComposition::MaskAttachment => "mask_attachment",
        AgentImageComposition::MaskedFramebufferCrop => "masked_framebuffer_crop",
        AgentImageComposition::IsolatedRegions => "isolated_regions",
        AgentImageComposition::DebugGeometry => "debug_geometry",
    }
}

fn decoded_base64_len(value: &str) -> Option<u64> {
    let padding = value
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    let groups = value.len().checked_div(4)?;
    let len = groups.checked_mul(3)?.checked_sub(padding)?;
    u64::try_from(len).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::{
        AgentBinaryEncoding, AgentBinaryResourceBody, AgentCoordinateSpace, AgentImageComposition,
        AgentImageContentBBox, AgentImageCropOrigin, AgentImageKind, AgentImageMetadata,
        AgentImageRenderer, AgentImageScope, AgentResourceBody,
    };

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
                    width: 320,
                    height: 180,
                    crop_origin: None,
                    pixel_format: None,
                    row_stride_bytes: None,
                    content_bbox: None,
                    content_viewport_bbox: None,
                    content_pixels: None,
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
            template.name == "object-color-capture"
                && template
                    .uri_template
                    .contains("object.{object_id}.{extension}")
                && template
                    .description
                    .as_deref()
                    .is_some_and(|description| description.contains("rich-text child objects"))
        }));
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
        assert!(
            tools
                .iter()
                .any(|tool| tool.name == "arcweft.resource.read")
        );
        assert!(tools.iter().any(|tool| tool.name == "arcweft.capture"));
        assert!(tools.iter().any(|tool| tool.name == "arcweft.session.info"));
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
        assert_eq!(properties["layer"]["type"], "string");
        assert_eq!(properties["object"]["type"], "string");
        assert_eq!(properties["page"]["type"], "integer");
        assert_eq!(properties["page"]["minimum"], 0);
        assert_eq!(properties["capture_time"]["type"], "number");
        assert_eq!(properties["capture_time"]["minimum"], 0);

        let capture = tools
            .iter()
            .find(|tool| tool.name == "arcweft.capture")
            .expect("capture tool is described");
        let properties = &capture.input_schema["properties"];
        assert_eq!(properties["uri"]["type"], "string");
        assert!(properties.get("renderer").is_none());
        assert_eq!(
            properties["format"]["enum"],
            serde_json::json!(["png", "raw-rgba"])
        );
        assert_eq!(properties["page"]["type"], "integer");
        assert_eq!(properties["page"]["minimum"], 0);
        assert_eq!(properties["capture_time"]["type"], "number");
        assert_eq!(properties["capture_time"]["minimum"], 0);
    }
}
