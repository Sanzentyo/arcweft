//! Shared Agent Debug Bus data types.
//!
//! This crate is Sans I/O. CLI commands, MCP servers, tests, and future player
//! adapters should exchange these typed observation records instead of defining
//! transport-local JSON shapes.

use arcweft_core::effect::{RuntimeEvent, RuntimeLog};
use arcweft_render_text::{
    LineDisplayFrame, RichTextObjectProxyDeclaration, RichTextParam, RichTextPresentation,
    RichTextRange, RichTextTextSource,
};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One Agent Debug Bus observation frame.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservationReport {
    pub status: String,
    pub session_id: String,
    pub tick: usize,
    pub frame_id: String,
    pub state_hash: String,
    pub render_hash: String,
    pub source: String,
    pub viewport: AgentViewport,
    pub images: Vec<AgentImageResource>,
    pub layers: Vec<AgentObservedLayer>,
    pub objects: Vec<AgentObservedObject>,
    pub presentation_tree: AgentPresentationTree,
    pub actions: Vec<AgentActionTarget>,
    pub ui_tree: AgentUiTree,
    pub scene_graph: Vec<serde_json::Value>,
    pub audio_state: AgentAudioState,
    pub logs: Vec<RuntimeLog>,
    pub signals: Vec<AgentAssignment>,
    pub metrics: Vec<AgentAssignment>,
    pub events: Vec<RuntimeEvent>,
    pub diagnostics: Vec<AgentDiagnostic>,
    pub steps: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capture_time_millis: Option<u32>,
    pub task_requests: usize,
    pub final_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overlay_svg: Option<String>,
}

impl AgentObservationReport {
    /// Builds the MCP-style latest observation JSON resource.
    pub fn observation_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!(
                "arcweft://session/{}/observation/latest.json",
                self.session_id
            ),
            kind: AgentResourceKind::ObservationLatest,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(self)?),
        })
    }

    /// Builds the MCP-style observed objects JSON resource.
    pub fn objects_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!(
                "arcweft://session/{}/frame/{}/objects.json",
                self.session_id, self.tick
            ),
            kind: AgentResourceKind::Objects,
            mime_type: "application/json".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.objects)?),
        })
    }

    /// Builds the MCP-style overlay SVG resource when the observation embeds one.
    pub fn overlay_svg_resource(&self) -> Option<AgentResource> {
        self.overlay_svg.as_ref().map(|overlay| AgentResource {
            uri: format!(
                "arcweft://session/{}/frame/{}/overlay.svg",
                self.session_id, self.tick
            ),
            kind: AgentResourceKind::OverlaySvg,
            mime_type: "image/svg+xml".to_owned(),
            hash: self.render_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(overlay.clone()),
        })
    }

    /// Builds an MCP-style image resource body for an image listed in this observation.
    pub fn image_resource(&self, image: &AgentImageResource, bytes: &[u8]) -> AgentResource {
        AgentResource {
            uri: image.uri.clone(),
            kind: AgentResourceKind::Image,
            mime_type: image.mime_type.clone(),
            hash: image.hash.clone(),
            image: Some(AgentImageMetadata::from_image_resource(image)),
            body: AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                data: STANDARD.encode(bytes),
            }),
        }
    }

    /// Builds the MCP-style signals JSON resource.
    pub fn signals_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!("arcweft://session/{}/signals.json", self.session_id),
            kind: AgentResourceKind::Signals,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.signals)?),
        })
    }

    /// Builds the MCP-style audio JSON resource.
    pub fn audio_resource(&self) -> Result<AgentResource, serde_json::Error> {
        Ok(AgentResource {
            uri: format!("arcweft://session/{}/audio.json", self.session_id),
            kind: AgentResourceKind::Audio,
            mime_type: "application/json".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Json(serde_json::to_value(&self.audio_state)?),
        })
    }

    /// Builds the MCP-style log stream resource as newline-delimited JSON.
    pub fn logs_resource(&self) -> Result<AgentResource, serde_json::Error> {
        let mut lines = self
            .logs
            .iter()
            .map(serde_json::to_string)
            .collect::<Result<Vec<_>, _>>()?
            .join("\n");
        if !lines.is_empty() {
            lines.push('\n');
        }
        Ok(AgentResource {
            uri: format!("arcweft://session/{}/logs.ndjson", self.session_id),
            kind: AgentResourceKind::Logs,
            mime_type: "application/x-ndjson".to_owned(),
            hash: self.state_hash.clone(),
            image: None,
            body: AgentResourceBody::Text(lines),
        })
    }
}

/// MCP-addressable Agent Debug Bus resource.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentResource {
    pub uri: String,
    pub kind: AgentResourceKind,
    pub mime_type: String,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<AgentImageMetadata>,
    pub body: AgentResourceBody,
}

/// Machine-readable image metadata attached to image resources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageMetadata {
    pub kind: AgentImageKind,
    pub renderer: AgentImageRenderer,
    pub scope: AgentImageScope,
    pub composition: AgentImageComposition,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_origin: Option<AgentImageCropOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub row_stride_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_viewport_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_pixels: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<AgentImageObjectRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<AgentDiagnostic>,
}

/// Observed object metadata preserved on an image resource that captures an object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageObjectRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    pub layer: String,
    pub role: String,
    pub bbox: AgentBBox,
    pub polygon: Vec<AgentPoint>,
    pub capture_refs: AgentObjectCaptureRefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
}

/// Bounds of non-transparent pixels in image-local coordinates.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageContentBBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Viewport-space origin for a cropped image.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageCropOrigin {
    pub space: AgentCoordinateSpace,
    pub x: u32,
    pub y: u32,
}

impl AgentImageMetadata {
    fn from_image_resource(image: &AgentImageResource) -> Self {
        let is_raw_rgba = image.mime_type == "application/octet-stream"
            && image.uri.rsplit('.').next() == Some("rgba");
        Self {
            kind: image.kind,
            renderer: image.renderer,
            scope: image.scope.clone(),
            composition: image.composition,
            page: image.page,
            width: image.width,
            height: image.height,
            crop_origin: image.crop_origin,
            pixel_format: is_raw_rgba.then(|| "rgba8_unorm".to_owned()),
            row_stride_bytes: is_raw_rgba.then(|| image.width.saturating_mul(4)),
            content_bbox: image.content_bbox,
            content_viewport_bbox: image.content_viewport_bbox,
            content_pixels: image.content_pixels,
            object: image.object.clone(),
            diagnostics: image.diagnostics.clone(),
        }
    }
}

/// Resource body payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "body_kind", content = "body", rename_all = "snake_case")]
pub enum AgentResourceBody {
    Json(serde_json::Value),
    Text(String),
    BytesBase64(AgentBinaryResourceBody),
}

/// Binary resource payload encoded for JSON/MCP transports.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBinaryResourceBody {
    pub encoding: AgentBinaryEncoding,
    pub data: String,
}

/// Binary resource encoding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentBinaryEncoding {
    Base64,
}

/// Agent resource kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentResourceKind {
    ObservationLatest,
    Objects,
    OverlaySvg,
    Image,
    Logs,
    Signals,
    Audio,
}

/// Viewport that coordinates object bounds and image resources.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentViewport {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
}

/// Rendered or render-adjacent frame resource addressable by Agent tools.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageResource {
    pub kind: AgentImageKind,
    #[serde(default)]
    pub renderer: AgentImageRenderer,
    #[serde(default)]
    pub scope: AgentImageScope,
    #[serde(default)]
    pub composition: AgentImageComposition,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub capture_step: usize,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub capture_time_millis: u32,
    pub uri: String,
    pub mime_type: String,
    pub width: u32,
    pub height: u32,
    pub hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop_origin: Option<AgentImageCropOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_viewport_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_pixels: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<AgentImageObjectRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<AgentDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written: Option<String>,
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

/// Renderer path that produced an image capture.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImageRenderer {
    #[default]
    Native,
}

/// How an image capture was composed.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImageComposition {
    #[default]
    Framebuffer,
    OverlayVector,
    FramebufferCrop,
    ObjectIdAttachment,
    MaskAttachment,
    MaskedFramebufferCrop,
    IsolatedRegions,
    DebugGeometry,
}

/// Image capture scope.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentImageScope {
    #[default]
    Viewport,
    Layer {
        id: String,
    },
    Object {
        id: String,
    },
}

/// Visible rendering layer known to the Agent observer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentObservedLayer {
    pub id: String,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub object_count: usize,
    pub capture_refs: AgentLayerCaptureRefs,
}

/// Capture resources addressable for one observed layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentLayerCaptureRefs {
    pub captures: Vec<AgentLayerCaptureRef>,
}

/// One image capture resource that can be requested for a layer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentLayerCaptureRef {
    pub kind: AgentImageKind,
    pub uri: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
}

/// Image resource kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImageKind {
    Color,
    Overlay,
    OverlaySvg,
    ObjectId,
    Mask,
}

/// Visible object or UI element known to the Agent observer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObservedObject {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entity: Option<String>,
    pub layer: String,
    pub role: String,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub polygon: Vec<AgentPoint>,
    pub capture_refs: AgentObjectCaptureRefs,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    pub rich_text: LineDisplayFrame,
}

impl AgentImageObjectRef {
    /// Copies the stable object identity and rich-text link into image metadata.
    pub fn from_observed(object: &AgentObservedObject) -> Self {
        Self {
            id: object.id.clone(),
            parent_id: object.parent_id.clone(),
            entity: object.entity.clone(),
            layer: object.layer.clone(),
            role: object.role.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            object_layer: object
                .rich_text_ref
                .as_ref()
                .and_then(|rich_text_ref| rich_text_ref.object_layer.clone()),
            object_depth: object
                .rich_text_ref
                .as_ref()
                .and_then(|rich_text_ref| rich_text_ref.object_depth),
            text: object.text.clone(),
            rich_text_ref: object.rich_text_ref.clone(),
        }
    }
}

/// Structured reference from an observed child object back into its parent rich-text display map.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentRichTextElementRef {
    pub kind: AgentRichTextElementKind,
    pub index: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub range: RichTextRange,
    pub node_index: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<RichTextTextSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentation: Option<RichTextPresentation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orientation: Option<AgentGlyphOrientation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_form: Option<AgentGlyphVerticalForm>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby_base_bbox: Option<AgentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ruby_annotation_bbox: Option<AgentBBox>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
    #[serde(default)]
    pub hit_test: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hit_regions: Vec<AgentHitRegion>,
}

/// Rich-text display-map element kind observed as a debuggable object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentRichTextElementKind {
    TextPage,
    TextLine,
    TextRun,
    TextGlyph,
    Ruby,
    GlyphCluster,
    TextObjectProxy,
}

/// Hit-test region for one observed rich-text element.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentHitRegion {
    pub kind: AgentHitRegionKind,
    pub bbox: AgentBBox,
    pub range: RichTextRange,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_declaration: Option<RichTextObjectProxyDeclaration>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub proxy_params: BTreeMap<String, RichTextParam>,
}

/// Result of hit-testing observed Agent objects at a viewport coordinate.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentHitTestReport {
    pub status: String,
    pub session_id: String,
    pub frame_id: String,
    pub source: String,
    pub viewport: AgentViewport,
    pub x: u32,
    pub y: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_object_id: Option<String>,
    pub hits: Vec<AgentHitTestHit>,
}

/// One observed object/region hit by an Agent hit-test query.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentHitTestHit {
    pub rank: usize,
    pub object_id: String,
    pub object: AgentImageObjectRef,
    pub layer: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    pub bbox: AgentBBox,
    pub polygon: Vec<AgentPoint>,
    pub capture_refs: AgentObjectCaptureRefs,
    pub region: AgentHitRegion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<i32>,
}

/// Semantic role for a rich-text hit-test region.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentHitRegionKind {
    TextPage,
    TextLine,
    TextRun,
    TextGlyph,
    GlyphCluster,
    TextObjectProxy,
    RubyObject,
    RubyBase,
    RubyAnnotation,
}

/// Renderer-facing orientation chosen for one observed glyph cluster.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGlyphOrientation {
    Upright,
    SidewaysCw,
    TextCombineUpright,
}

/// Vertical alternate shaping request attached to one observed glyph cluster.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGlyphVerticalForm {
    None,
    UprightAlternate,
    RotatedAlternate,
}

/// Capture resources addressable for one observed object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentObjectCaptureRefs {
    pub object_id_color: AgentRgbaColor,
    pub captures: Vec<AgentObjectCaptureRef>,
}

/// One image capture resource that can be requested for an object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentObjectCaptureRef {
    pub kind: AgentImageKind,
    pub uri: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
}

/// RGBA color used by object-id debug images.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentRgbaColor {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// Axis-aligned object bounds.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentBBox {
    pub space: AgentCoordinateSpace,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl AgentBBox {
    /// Returns the viewport-space rectangle corners in clockwise order.
    pub fn polygon(&self) -> Vec<AgentPoint> {
        vec![
            AgentPoint {
                x: self.x,
                y: self.y,
            },
            AgentPoint {
                x: self.x + self.width,
                y: self.y,
            },
            AgentPoint {
                x: self.x + self.width,
                y: self.y + self.height,
            },
            AgentPoint {
                x: self.x,
                y: self.y + self.height,
            },
        ]
    }
}

/// Coordinate space for Agent geometry.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCoordinateSpace {
    Viewport,
    World,
    Ui,
}

/// Point in an Agent coordinate space.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPoint {
    pub x: u32,
    pub y: u32,
}

/// Action that can be invoked semantically or via future input synthesis.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentActionTarget {
    pub id: String,
    pub target: String,
    pub action: AgentActionKind,
    pub kind: AgentActionDispatch,
    pub enabled: bool,
}

/// Semantic action kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionKind {
    AdvanceText,
    SelectChoice,
    Invoke,
    PointerClick,
}

/// How an action should be dispatched.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentActionDispatch {
    Semantic,
    Physical,
}

/// Minimal UI tree slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentUiTree {
    pub root: String,
    pub children: Vec<String>,
}

/// Typed presentation object tree for renderable and render-adjacent objects.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationTree {
    pub root: String,
    pub nodes: Vec<AgentPresentationTreeNode>,
}

/// One node in the typed presentation object tree.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentPresentationTreeNode {
    pub id: String,
    pub kind: AgentPresentationTreeNodeKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layer_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rich_text_kind: Option<AgentRichTextElementKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
}

/// Presentation tree node category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPresentationTreeNodeKind {
    Root,
    Layer,
    Object,
}

impl AgentPresentationTree {
    /// Builds a stable layer/object tree from observed layers and objects.
    pub fn from_layers_and_objects(
        layers: &[AgentObservedLayer],
        objects: &[AgentObservedObject],
    ) -> Self {
        let root = "presentation.root".to_owned();
        let mut layer_ids = layers
            .iter()
            .map(|layer| layer.id.clone())
            .collect::<Vec<_>>();
        for object in objects {
            if !layer_ids.iter().any(|layer_id| layer_id == &object.layer) {
                layer_ids.push(object.layer.clone());
            }
        }

        let object_ids = objects
            .iter()
            .map(|object| object.id.clone())
            .collect::<Vec<_>>();
        let mut children_by_parent = BTreeMap::<String, Vec<String>>::new();
        children_by_parent.insert(
            root.clone(),
            layer_ids
                .iter()
                .map(|layer_id| presentation_layer_node_id(layer_id))
                .collect(),
        );
        for layer_id in &layer_ids {
            children_by_parent
                .entry(presentation_layer_node_id(layer_id))
                .or_default();
        }
        for object in objects {
            let parent_id = object
                .parent_id
                .as_ref()
                .filter(|parent_id| object_ids.iter().any(|object_id| object_id == *parent_id))
                .cloned()
                .unwrap_or_else(|| presentation_layer_node_id(&object.layer));
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(object.id.clone());
        }

        let mut nodes = Vec::with_capacity(1 + layer_ids.len() + objects.len());
        nodes.push(AgentPresentationTreeNode {
            id: root.clone(),
            kind: AgentPresentationTreeNodeKind::Root,
            parent_id: None,
            children: children_by_parent.remove(&root).unwrap_or_default(),
            layer_id: None,
            object_id: None,
            role: None,
            rich_text_kind: None,
            object_layer: None,
            object_depth: None,
        });
        nodes.extend(layer_ids.iter().map(|layer_id| {
            let node_id = presentation_layer_node_id(layer_id);
            AgentPresentationTreeNode {
                id: node_id.clone(),
                kind: AgentPresentationTreeNodeKind::Layer,
                parent_id: Some(root.clone()),
                children: children_by_parent.remove(&node_id).unwrap_or_default(),
                layer_id: Some(layer_id.clone()),
                object_id: None,
                role: None,
                rich_text_kind: None,
                object_layer: None,
                object_depth: None,
            }
        }));
        nodes.extend(objects.iter().map(|object| {
            let parent_id = object
                .parent_id
                .as_ref()
                .filter(|parent_id| object_ids.iter().any(|object_id| object_id == *parent_id))
                .cloned()
                .unwrap_or_else(|| presentation_layer_node_id(&object.layer));
            let rich_text_ref = object.rich_text_ref.as_ref();
            AgentPresentationTreeNode {
                id: object.id.clone(),
                kind: AgentPresentationTreeNodeKind::Object,
                parent_id: Some(parent_id),
                children: children_by_parent.remove(&object.id).unwrap_or_default(),
                layer_id: Some(object.layer.clone()),
                object_id: Some(object.id.clone()),
                role: Some(object.role.clone()),
                rich_text_kind: rich_text_ref.map(|rich_text_ref| rich_text_ref.kind),
                object_layer: rich_text_ref
                    .and_then(|rich_text_ref| rich_text_ref.object_layer.clone()),
                object_depth: rich_text_ref.and_then(|rich_text_ref| rich_text_ref.object_depth),
            }
        }));

        Self { root, nodes }
    }
}

fn presentation_layer_node_id(layer_id: &str) -> String {
    format!("presentation.layer.{layer_id}")
}

/// Current audio observation slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentAudioState {
    pub active_voices: Vec<String>,
    pub pending_events: Vec<String>,
}

/// Named observation value.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentAssignment {
    pub name: String,
    pub value: String,
}

/// Diagnostic attached to an observation frame.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentDiagnostic {
    pub step: usize,
    pub severity: AgentDiagnosticSeverity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_id: Option<String>,
    pub message: String,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentDiagnosticSeverity {
    Error,
    Warning,
    Info,
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_core::plan::RuntimeLineId;
    use arcweft_render_text::{
        RichTextAssignOp, RichTextCascadeLayer, RichTextEffectDescriptor, RichTextEffectPhase,
        RichTextEffectTarget, RichTextParam, RichTextPresentation, RichTextSettingSource,
        RichTextStateScope, RichTextStyleContribution,
    };
    use std::collections::BTreeMap;

    fn test_capture_refs() -> AgentObjectCaptureRefs {
        AgentObjectCaptureRefs {
            object_id_color: AgentRgbaColor {
                red: 120,
                green: 130,
                blue: 140,
                alpha: 255,
            },
            captures: vec![AgentObjectCaptureRef {
                kind: AgentImageKind::Mask,
                uri: "arcweft://session/cli/frame/1/object.object.dialogue.0.0.mask.png".to_owned(),
                mime_type: "image/png".to_owned(),
                page: 0,
                width: 3,
                height: 4,
            }],
        }
    }

    fn test_layer_capture_refs() -> AgentLayerCaptureRefs {
        AgentLayerCaptureRefs {
            captures: vec![AgentLayerCaptureRef {
                kind: AgentImageKind::Color,
                uri: "arcweft://session/cli/frame/1/layer.dialogue.png".to_owned(),
                mime_type: "image/png".to_owned(),
                page: 0,
                width: 10,
                height: 20,
            }],
        }
    }

    fn test_line_display_frame() -> LineDisplayFrame {
        LineDisplayFrame {
            line: RuntimeLineId("say.test.001".to_owned()),
            callee: "alice".to_owned(),
            text: "Hello".to_owned(),
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: vec![RichTextStyleContribution {
                path: "rich_text.ruby.size".to_owned(),
                layer: RichTextCascadeLayer::DialogueDefaults,
                source: RichTextSettingSource::EngineDefault {
                    key: "dialogue.rich_text.ruby.size".to_owned(),
                },
                op: RichTextAssignOp::Replace,
                value: "14".to_owned(),
                style_index: None,
                active: true,
                shadowed_by: None,
            }],
            nodes: Vec::new(),
            display_map: arcweft_render_text::RichTextDisplayMap::default(),
            host_events: Vec::new(),
            inline_failures: Vec::new(),
            unresolved: Vec::new(),
        }
    }

    fn test_raw_mask_image_resource() -> AgentImageResource {
        AgentImageResource {
            kind: AgentImageKind::Mask,
            renderer: AgentImageRenderer::Native,
            scope: AgentImageScope::Object {
                id: "object.dialogue.0.0".to_owned(),
            },
            composition: AgentImageComposition::MaskAttachment,
            page: 0,
            capture_step: 0,
            capture_time_millis: 0,
            uri: "arcweft://session/cli/frame/7/object.object.dialogue.0.0.mask.rgba".to_owned(),
            mime_type: "application/octet-stream".to_owned(),
            width: 3,
            height: 4,
            hash: "raw-hash".to_owned(),
            crop_origin: Some(AgentImageCropOrigin {
                space: AgentCoordinateSpace::Viewport,
                x: 96,
                y: 548,
            }),
            content_bbox: Some(AgentImageContentBBox {
                x: 0,
                y: 0,
                width: 3,
                height: 4,
            }),
            content_viewport_bbox: Some(AgentImageContentBBox {
                x: 96,
                y: 548,
                width: 3,
                height: 4,
            }),
            content_pixels: Some(12),
            object: None,
            diagnostics: Vec::new(),
            written: None,
        }
    }

    fn assert_image_metadata(resource: &AgentResource, expected: AgentImageMetadata) {
        assert_eq!(resource.image, Some(expected));
    }

    fn test_mcp_observation_report() -> AgentObservationReport {
        AgentObservationReport {
            status: "ok".to_owned(),
            session_id: "cli".to_owned(),
            tick: 7,
            frame_id: "frame.7".to_owned(),
            state_hash: "state-hash".to_owned(),
            render_hash: "render-hash".to_owned(),
            source: "game.arcw".to_owned(),
            viewport: AgentViewport {
                width: 1280,
                height: 720,
                scale: 1.0,
            },
            images: vec![AgentImageResource {
                kind: AgentImageKind::Color,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Viewport,
                composition: AgentImageComposition::Framebuffer,
                page: 0,
                capture_step: 0,
                capture_time_millis: 0,
                uri: "arcweft://session/cli/frame/7/color.png".to_owned(),
                mime_type: "image/png".to_owned(),
                width: 1280,
                height: 720,
                hash: "image-hash".to_owned(),
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: None,
                diagnostics: Vec::new(),
                written: None,
            }],
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
            signals: vec![AgentAssignment {
                name: "signal.ready".to_owned(),
                value: "true".to_owned(),
            }],
            metrics: Vec::new(),
            events: Vec::new(),
            diagnostics: Vec::new(),
            steps: 1,
            capture_time_millis: None,
            task_requests: 0,
            final_status: "done Return(\"ok\")".to_owned(),
            overlay_svg: Some("<svg/>".to_owned()),
        }
    }

    fn test_serialization_observation_report() -> AgentObservationReport {
        let bbox = AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x: 1,
            y: 2,
            width: 3,
            height: 4,
        };
        let layers = vec![AgentObservedLayer {
            id: "dialogue".to_owned(),
            visible: true,
            bbox: bbox.clone(),
            object_count: 1,
            capture_refs: test_layer_capture_refs(),
        }];
        let objects = vec![AgentObservedObject {
            id: "object.dialogue.0.0".to_owned(),
            parent_id: None,
            entity: Some("alice".to_owned()),
            layer: "dialogue".to_owned(),
            role: "textbox".to_owned(),
            visible: true,
            polygon: bbox.polygon(),
            bbox: bbox.clone(),
            capture_refs: test_capture_refs(),
            text: Some("Hello".to_owned()),
            rich_text_ref: Some(test_rich_text_ref(&bbox)),
            rich_text: test_line_display_frame(),
        }];
        let presentation_tree = AgentPresentationTree::from_layers_and_objects(&layers, &objects);
        AgentObservationReport {
            status: "ok".to_owned(),
            session_id: "cli".to_owned(),
            tick: 1,
            frame_id: "frame.1".to_owned(),
            state_hash: "state".to_owned(),
            render_hash: "render".to_owned(),
            source: "game.arcw".to_owned(),
            viewport: AgentViewport {
                width: 1280,
                height: 720,
                scale: 1.0,
            },
            images: vec![AgentImageResource {
                kind: AgentImageKind::OverlaySvg,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Viewport,
                composition: AgentImageComposition::OverlayVector,
                page: 0,
                capture_step: 0,
                capture_time_millis: 0,
                uri: "arcweft://session/cli/frame/1/overlay.svg".to_owned(),
                mime_type: "image/svg+xml".to_owned(),
                width: 1280,
                height: 720,
                hash: "render".to_owned(),
                crop_origin: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: None,
                diagnostics: Vec::new(),
                written: None,
            }],
            layers,
            objects,
            presentation_tree,
            actions: vec![AgentActionTarget {
                id: "action.advance_text.object.dialogue.0.0".to_owned(),
                target: "object.dialogue.0.0".to_owned(),
                action: AgentActionKind::AdvanceText,
                kind: AgentActionDispatch::Semantic,
                enabled: true,
            }],
            ui_tree: AgentUiTree {
                root: "ui.root".to_owned(),
                children: vec!["dialogue.layer".to_owned()],
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
                step: 0,
                severity: AgentDiagnosticSeverity::Info,
                source: None,
                code: None,
                effect_id: None,
                message: "ready".to_owned(),
            }],
            steps: 1,
            capture_time_millis: None,
            task_requests: 0,
            final_status: "done Return(\"ok\")".to_owned(),
            overlay_svg: None,
        }
    }

    fn test_rich_text_ref(bbox: &AgentBBox) -> AgentRichTextElementRef {
        AgentRichTextElementRef {
            kind: AgentRichTextElementKind::TextRun,
            index: 0,
            page: 0,
            range: RichTextRange::new(0, 5),
            node_index: 0,
            source: Some(RichTextTextSource::Text),
            ruby: None,
            presentation: Some(RichTextPresentation {
                effects: vec![RichTextEffectDescriptor {
                    id: "shake".to_owned(),
                    params: BTreeMap::default(),
                    target: RichTextEffectTarget::default(),
                    phase: RichTextEffectPhase::GlyphTransform,
                    state_scope: RichTextStateScope::default(),
                }],
                ..RichTextPresentation::default()
            }),
            orientation: None,
            vertical_form: None,
            ruby_base_bbox: None,
            ruby_annotation_bbox: None,
            object_layer: None,
            object_depth: None,
            hit_test: false,
            hit_regions: vec![AgentHitRegion {
                kind: AgentHitRegionKind::TextRun,
                bbox: bbox.clone(),
                range: RichTextRange::new(0, 5),
                proxy_id: None,
                proxy_type: None,
                proxy_declaration: None,
                proxy_role: None,
                proxy_layer: None,
                depth: None,
                proxy_params: BTreeMap::new(),
            }],
        }
    }

    #[test]
    fn observation_report_serializes_stable_snake_case_enums() {
        let report = test_serialization_observation_report();

        let json = serde_json::to_value(&report).expect("report serializes");

        assert_eq!(json["images"][0]["kind"], "overlay_svg");
        assert_eq!(json["images"][0]["renderer"], "native");
        assert_eq!(json["images"][0]["scope"]["kind"], "viewport");
        assert_eq!(json["images"][0]["composition"], "overlay_vector");
        assert_eq!(
            serde_json::to_value(AgentImageComposition::MaskedFramebufferCrop)
                .expect("composition serializes"),
            "masked_framebuffer_crop"
        );
        assert_eq!(
            serde_json::to_value(AgentImageComposition::ObjectIdAttachment)
                .expect("composition serializes"),
            "object_id_attachment"
        );
        assert_eq!(
            serde_json::to_value(AgentImageComposition::MaskAttachment)
                .expect("composition serializes"),
            "mask_attachment"
        );
        assert_eq!(
            json["layers"][0]["capture_refs"]["captures"][0]["kind"],
            "color"
        );
        assert_eq!(json["objects"][0]["bbox"]["space"], "viewport");
        assert_eq!(
            json["objects"][0]["capture_refs"]["captures"][0]["kind"],
            "mask"
        );
        assert_eq!(
            json["objects"][0]["capture_refs"]["object_id_color"]["alpha"],
            255
        );
        assert_eq!(json["objects"][0]["rich_text_ref"]["kind"], "text_run");
        assert_eq!(
            serde_json::to_value(AgentRichTextElementKind::TextPage)
                .expect("rich-text element kind serializes"),
            "text_page"
        );
        assert_eq!(
            serde_json::to_value(AgentRichTextElementKind::TextLine)
                .expect("rich-text element kind serializes"),
            "text_line"
        );
        assert_eq!(
            serde_json::to_value(AgentRichTextElementKind::TextGlyph)
                .expect("rich-text element kind serializes"),
            "text_glyph"
        );
        assert_eq!(json["objects"][0]["rich_text_ref"]["source"], "text");
        assert_eq!(
            json["objects"][0]["rich_text_ref"]["presentation"]["effects"][0]["id"],
            "shake"
        );
        assert_eq!(
            json["objects"][0]["rich_text_ref"]["presentation"]["effects"][0]["phase"],
            "glyph_transform"
        );
        assert_eq!(
            json["objects"][0]["rich_text"]["style_contributions"][0]["path"],
            "rich_text.ruby.size"
        );
        assert_eq!(
            json["objects"][0]["rich_text"]["style_contributions"][0]["layer"],
            "dialogue_defaults"
        );
        assert_eq!(
            json["objects"][0]["rich_text_ref"]["hit_regions"][0]["kind"],
            "text_run"
        );
        assert_eq!(json["presentation_tree"]["root"], "presentation.root");
        assert_eq!(json["presentation_tree"]["nodes"][0]["kind"], "root");
        assert_eq!(
            json["presentation_tree"]["nodes"][0]["children"][0],
            "presentation.layer.dialogue"
        );
        assert_eq!(json["presentation_tree"]["nodes"][1]["kind"], "layer");
        assert_eq!(
            json["presentation_tree"]["nodes"][1]["layer_id"],
            "dialogue"
        );
        assert_eq!(
            json["presentation_tree"]["nodes"][1]["children"][0],
            "object.dialogue.0.0"
        );
        assert_eq!(json["presentation_tree"]["nodes"][2]["kind"], "object");
        assert_eq!(
            json["presentation_tree"]["nodes"][2]["object_id"],
            "object.dialogue.0.0"
        );
        assert_eq!(json["presentation_tree"]["nodes"][2]["role"], "textbox");
        assert_eq!(
            json["presentation_tree"]["nodes"][2]["rich_text_kind"],
            "text_run"
        );
        assert_eq!(
            serde_json::to_value(AgentHitRegionKind::TextPage).expect("hit-region kind serializes"),
            "text_page"
        );
        assert_eq!(
            serde_json::to_value(AgentHitRegionKind::TextLine).expect("hit-region kind serializes"),
            "text_line"
        );
        assert_eq!(
            serde_json::to_value(AgentHitRegionKind::TextGlyph)
                .expect("hit-region kind serializes"),
            "text_glyph"
        );
        assert_eq!(
            serde_json::to_value(AgentHitRegionKind::RubyAnnotation)
                .expect("hit-region kind serializes"),
            "ruby_annotation"
        );
        assert_eq!(json["actions"][0]["action"], "advance_text");
        assert_eq!(json["actions"][0]["kind"], "semantic");
        assert_eq!(json["diagnostics"][0]["severity"], "info");
    }

    #[test]
    fn hit_region_serializes_proxy_params_when_present() {
        let region = AgentHitRegion {
            kind: AgentHitRegionKind::TextObjectProxy,
            bbox: AgentBBox {
                space: AgentCoordinateSpace::Viewport,
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            range: RichTextRange::new(0, 3),
            proxy_id: Some("hotspot".to_owned()),
            proxy_type: Some("KeywordHit".to_owned()),
            proxy_declaration: Some(RichTextObjectProxyDeclaration {
                struct_name: "KeywordHit".to_owned(),
                attribute: "text_proxy".to_owned(),
            }),
            proxy_role: Some("keyword".to_owned()),
            proxy_layer: None,
            depth: Some(4000),
            proxy_params: BTreeMap::from([(
                "channel".to_owned(),
                RichTextParam::Selector {
                    value: "choice".to_owned(),
                },
            )]),
        };

        let json = serde_json::to_value(&region).expect("hit region serializes");

        assert_eq!(json["kind"], "text_object_proxy");
        assert_eq!(json["proxy_declaration"]["struct_name"], "KeywordHit");
        assert_eq!(json["proxy_declaration"]["attribute"], "text_proxy");
        assert_eq!(json["proxy_params"]["channel"]["value"], "choice");
    }

    #[test]
    fn hit_test_hit_serializes_capture_refs() {
        let bbox = AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x: 10,
            y: 20,
            width: 30,
            height: 40,
        };
        let hit = AgentHitTestHit {
            rank: 0,
            object_id: "object.dialogue.0.0.proxy.0.0".to_owned(),
            object: AgentImageObjectRef {
                id: "object.dialogue.0.0.proxy.0.0".to_owned(),
                parent_id: Some("object.dialogue.0.0".to_owned()),
                entity: Some("character.alice".to_owned()),
                layer: "dialogue.rich_text".to_owned(),
                role: "rich_text_proxy".to_owned(),
                bbox: bbox.clone(),
                polygon: bbox.polygon(),
                capture_refs: test_capture_refs(),
                object_layer: Some("ui".to_owned()),
                object_depth: Some(4000),
                text: Some("Hit".to_owned()),
                rich_text_ref: None,
            },
            layer: "ui".to_owned(),
            role: "rich_text_proxy".to_owned(),
            text: Some("Hit".to_owned()),
            bbox: bbox.clone(),
            polygon: bbox.polygon(),
            capture_refs: test_capture_refs(),
            region: AgentHitRegion {
                kind: AgentHitRegionKind::TextObjectProxy,
                bbox,
                range: RichTextRange::new(0, 3),
                proxy_id: Some("hotspot".to_owned()),
                proxy_type: Some("KeywordHit".to_owned()),
                proxy_declaration: Some(RichTextObjectProxyDeclaration {
                    struct_name: "KeywordHit".to_owned(),
                    attribute: "text_proxy".to_owned(),
                }),
                proxy_role: Some("keyword".to_owned()),
                proxy_layer: Some("ui".to_owned()),
                depth: Some(4000),
                proxy_params: BTreeMap::new(),
            },
            rich_text_ref: None,
            depth: Some(4000),
        };

        let json = serde_json::to_value(&hit).expect("hit serializes");

        assert_eq!(json["object"]["layer"], "dialogue.rich_text");
        assert_eq!(json["object"]["object_layer"], "ui");
        assert_eq!(json["layer"], "ui");
        assert_eq!(json["polygon"].as_array().unwrap().len(), 4);
        assert_eq!(json["capture_refs"]["object_id_color"]["alpha"], 255);
        assert_eq!(json["capture_refs"]["captures"][0]["kind"], "mask");
    }

    #[test]
    fn image_resource_metadata_preserves_observed_object_ref() {
        let report = test_mcp_observation_report();
        let bbox = AgentBBox {
            space: AgentCoordinateSpace::Viewport,
            x: 96,
            y: 548,
            width: 3,
            height: 4,
        };
        let mut rich_text_ref = test_rich_text_ref(&bbox);
        rich_text_ref.object_layer = Some("ui".to_owned());
        rich_text_ref.object_depth = Some(7000);
        let mut image = test_raw_mask_image_resource();
        image.object = Some(AgentImageObjectRef {
            id: "object.dialogue.0.0.run.0".to_owned(),
            parent_id: Some("object.dialogue.0.0".to_owned()),
            entity: Some("character.alice".to_owned()),
            layer: "dialogue".to_owned(),
            role: "rich_text_run".to_owned(),
            bbox: bbox.clone(),
            polygon: bbox.polygon(),
            capture_refs: test_capture_refs(),
            object_layer: Some("ui".to_owned()),
            object_depth: Some(7000),
            text: Some("Hello".to_owned()),
            rich_text_ref: Some(rich_text_ref.clone()),
        });

        let resource = report.image_resource(&image, &[255; 48]);

        assert_eq!(
            resource
                .image
                .as_ref()
                .and_then(|image| image.object.as_ref()),
            image.object.as_ref()
        );
        assert_eq!(
            resource
                .image
                .as_ref()
                .and_then(|image| image.object.as_ref())
                .and_then(|object| object.rich_text_ref.as_ref()),
            Some(&rich_text_ref)
        );
        let json = serde_json::to_value(&resource).expect("resource serializes");
        assert_eq!(json["image"]["object"]["layer"], "dialogue");
        assert_eq!(json["image"]["object"]["bbox"]["x"], 96);
        assert_eq!(
            json["image"]["object"]["polygon"].as_array().unwrap().len(),
            4
        );
        assert_eq!(
            json["image"]["object"]["capture_refs"]["object_id_color"]["alpha"],
            255
        );
        assert_eq!(
            json["image"]["object"]["capture_refs"]["captures"][0]["kind"],
            "mask"
        );
        assert_eq!(json["image"]["object"]["object_layer"], "ui");
        assert_eq!(json["image"]["object"]["object_depth"], 7000);
    }

    #[test]
    fn image_resource_metadata_preserves_capture_diagnostics() {
        let report = test_mcp_observation_report();
        let mut image = test_raw_mask_image_resource();
        image.diagnostics = vec![AgentDiagnostic {
            step: 7,
            severity: AgentDiagnosticSeverity::Warning,
            source: Some("native_rich_text".to_owned()),
            code: Some("missing_shader".to_owned()),
            effect_id: Some("ghost_glow".to_owned()),
            message: "native rich-text missing_shader: ghost_glow".to_owned(),
        }];

        let resource = report.image_resource(&image, &[255; 48]);
        let metadata = resource.image.expect("image metadata is attached");

        assert_eq!(metadata.diagnostics, image.diagnostics);
    }

    #[test]
    fn observation_report_builds_mcp_style_resources() {
        let report = test_mcp_observation_report();

        let latest = report
            .observation_resource()
            .expect("latest resource serializes");
        let objects = report.objects_resource().expect("objects serialize");
        let overlay = report.overlay_svg_resource().expect("overlay exists");
        let image = report.image_resource(&report.images[0], b"\x89PNG\r\n\x1a\n");
        let raw_image = report.image_resource(&test_raw_mask_image_resource(), &[255; 48]);
        let signals = report.signals_resource().expect("signals serialize");

        assert_eq!(latest.uri, "arcweft://session/cli/observation/latest.json");
        assert_eq!(latest.kind, AgentResourceKind::ObservationLatest);
        assert_eq!(objects.uri, "arcweft://session/cli/frame/7/objects.json");
        assert_eq!(overlay.uri, "arcweft://session/cli/frame/7/overlay.svg");
        assert_eq!(overlay.mime_type, "image/svg+xml");
        assert_eq!(image.uri, "arcweft://session/cli/frame/7/color.png");
        assert_eq!(image.kind, AgentResourceKind::Image);
        assert_eq!(image.mime_type, "image/png");
        assert_image_metadata(
            &image,
            AgentImageMetadata {
                kind: AgentImageKind::Color,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Viewport,
                composition: AgentImageComposition::Framebuffer,
                page: 0,
                width: 1280,
                height: 720,
                crop_origin: None,
                pixel_format: None,
                row_stride_bytes: None,
                content_bbox: None,
                content_viewport_bbox: None,
                content_pixels: None,
                object: None,
                diagnostics: Vec::new(),
            },
        );
        assert_image_metadata(
            &raw_image,
            AgentImageMetadata {
                kind: AgentImageKind::Mask,
                renderer: AgentImageRenderer::Native,
                scope: AgentImageScope::Object {
                    id: "object.dialogue.0.0".to_owned(),
                },
                composition: AgentImageComposition::MaskAttachment,
                page: 0,
                width: 3,
                height: 4,
                crop_origin: Some(AgentImageCropOrigin {
                    space: AgentCoordinateSpace::Viewport,
                    x: 96,
                    y: 548,
                }),
                pixel_format: Some("rgba8_unorm".to_owned()),
                row_stride_bytes: Some(12),
                content_bbox: Some(AgentImageContentBBox {
                    x: 0,
                    y: 0,
                    width: 3,
                    height: 4,
                }),
                content_viewport_bbox: Some(AgentImageContentBBox {
                    x: 96,
                    y: 548,
                    width: 3,
                    height: 4,
                }),
                content_pixels: Some(12),
                object: None,
                diagnostics: Vec::new(),
            },
        );
        assert_eq!(signals.uri, "arcweft://session/cli/signals.json");
        assert!(matches!(overlay.body, AgentResourceBody::Text(_)));
        assert!(matches!(
            image.body,
            AgentResourceBody::BytesBase64(AgentBinaryResourceBody {
                encoding: AgentBinaryEncoding::Base64,
                ..
            })
        ));
    }
}
