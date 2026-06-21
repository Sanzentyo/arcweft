//! MCP resource descriptors and MCP result mapping for Arcweft Agent Debug Bus.
//!
//! This module owns the resource/list/read conversions and the tool-result
//! projection that wraps resources for MCP clients.

use arcweft_agent_protocol::{
    image::{AgentImageComposition, AgentImageKind, AgentImageRenderer, AgentImageScope},
    resource::{AgentResource, AgentResourceBody, AgentResourceKind},
    trace::AgentTraceRecord,
};

use crate::model::{
    AGENT_TRACE_MIME_TYPE, McpBlobResourceContents, McpCallToolResult, McpContentBlock,
    McpListResourceTemplatesResult, McpListResourcesResult, McpReadResourceResult,
    McpResourceContents, McpResourceDescriptor, McpResourceTemplateDescriptor,
    McpTextResourceContents,
};

pub fn agent_resource_templates() -> Vec<McpResourceTemplateDescriptor> {
    vec![
        resource_template(
            "arcweft://session/{session_id}/context.json",
            "session-context",
            "Session context",
            "Path-redacted Agent session context for observing the latest frame, cached captures, trace resources, and RAG query state.",
            Some("application/json"),
        ),
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
            "arcweft://session/{session_id}/frame/{tick}/presentation-tree.json",
            "presentation-tree",
            "Presentation tree",
            "Typed presentation object tree for the frame, including layer/object hierarchy and lightweight rich-text visual feature indexes.",
            Some("application/json"),
        ),
        resource_template(
            "arcweft://session/{session_id}/frame/{tick}/presentation-tree.json?{filter_key}={filter_value}",
            "presentation-tree-filter",
            "Filtered presentation tree",
            "Typed presentation object tree filtered by role, rich_text_kind, object_layer, effect, shader, motion, proxy id/type/role/struct/params, or has_transform while preserving ancestors.",
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
        resource_template(
            "arcweft://run/{run_id}/trace.arcwx",
            "agent-trace",
            "Agent trace",
            "Validated Agent execution trace records for read-only replay and regression comparison.",
            Some(AGENT_TRACE_MIME_TYPE),
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

/// Builds an MCP-addressable Agent trace resource from typed trace records.
///
/// The trace remains JSON at this boundary; portable binary/archive packaging
/// for large blobs is handled by higher-level tooling.
pub fn trace_resource(records: &[AgentTraceRecord]) -> Result<AgentResource, serde_json::Error> {
    Ok(AgentResource {
        uri: trace_resource_uri(records),
        kind: AgentResourceKind::Trace,
        mime_type: AGENT_TRACE_MIME_TYPE.to_owned(),
        hash: trace_resource_hash(records),
        image: None,
        body: AgentResourceBody::Json(serde_json::to_value(records)?),
    })
}

fn trace_resource_uri(records: &[AgentTraceRecord]) -> String {
    records.first().map_or_else(
        || "arcweft://run/unknown/trace.arcwx".to_owned(),
        |record| format!("arcweft://run/{}/trace.arcwx", record.run_id.as_str()),
    )
}

fn trace_resource_hash(records: &[AgentTraceRecord]) -> String {
    records.last().map_or_else(
        || "trace:empty".to_owned(),
        |record| {
            format!(
                "trace:{}:{}:{}",
                record.run_id.as_str(),
                records.len(),
                record.payload_hash.as_str()
            )
        },
    )
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
        AgentResourceKind::SessionContext => "Session context",
        AgentResourceKind::ObservationLatest => "Latest observation",
        AgentResourceKind::Objects => "Observed objects",
        AgentResourceKind::PresentationTree => "Presentation tree",
        AgentResourceKind::OverlaySvg => "Overlay SVG",
        AgentResourceKind::Image => "Captured image",
        AgentResourceKind::Logs => "Runtime logs",
        AgentResourceKind::Signals => "Runtime signals",
        AgentResourceKind::Audio => "Audio state",
        AgentResourceKind::Trace => "Agent trace",
    }
    .to_owned()
}

fn resource_description(resource: &AgentResource) -> String {
    if resource.kind == AgentResourceKind::SessionContext {
        return format!(
            "Path-redacted Agent session context resource ({})",
            resource.mime_type
        );
    }
    if resource.kind == AgentResourceKind::Trace {
        return format!(
            "Agent execution trace resource for read-only replay ({})",
            resource.mime_type
        );
    }
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

pub(crate) fn image_composition_description(composition: AgentImageComposition) -> &'static str {
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
