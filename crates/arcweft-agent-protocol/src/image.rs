use crate::geometry::{AgentBBox, AgentCoordinateSpace, AgentPoint};
use crate::proxy::AgentPresentationObjectProxyRef;
use crate::rich_text::AgentRichTextElementRef;
use crate::serde_helpers::{is_zero, is_zero_u32};
use arcweft_layout::{
    CaptureComposition, CaptureCropBounds, CaptureMaskMetadata, CaptureMetadata,
    CaptureRendererKind, CaptureScope, FitTransformMetadata, LayoutCoordinateSpace, LayoutRect,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Capture resources addressable for one observed object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObjectCaptureRefs {
    pub object_id_color: crate::geometry::AgentRgbaColor,
    pub captures: Vec<AgentObjectCaptureRef>,
}

/// One image capture resource that can be requested for an object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentObjectCaptureRef {
    pub kind: AgentImageKind,
    pub uri: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
}

/// Capture resources addressable for one observed layer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentLayerCaptureRefs {
    pub captures: Vec<AgentLayerCaptureRef>,
}

/// One image capture resource that can be requested for a layer.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentLayerCaptureRef {
    pub kind: AgentImageKind,
    pub uri: String,
    pub mime_type: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
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

/// Renderer path that produced an image capture.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImageRenderer {
    #[default]
    Native,
}

impl AgentImageRenderer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
        }
    }
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

impl AgentImageComposition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Framebuffer => "framebuffer",
            Self::OverlayVector => "overlay_vector",
            Self::FramebufferCrop => "framebuffer_crop",
            Self::ObjectIdAttachment => "object_id_attachment",
            Self::MaskAttachment => "mask_attachment",
            Self::MaskedFramebufferCrop => "masked_framebuffer_crop",
            Self::IsolatedRegions => "isolated_regions",
            Self::DebugGeometry => "debug_geometry",
        }
    }

    /// Composition recorded after a content-policy mask has been applied.
    #[must_use]
    pub const fn after_policy_mask(self) -> Self {
        match self {
            Self::OverlayVector => Self::IsolatedRegions,
            Self::Framebuffer
            | Self::FramebufferCrop
            | Self::ObjectIdAttachment
            | Self::MaskAttachment
            | Self::MaskedFramebufferCrop
            | Self::IsolatedRegions
            | Self::DebugGeometry => Self::MaskedFramebufferCrop,
        }
    }
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

impl AgentImageScope {
    pub fn description(&self) -> String {
        match self {
            Self::Viewport => "viewport".to_owned(),
            Self::Layer { id } => format!("layer:{id}"),
            Self::Object { id } => format!("object:{id}"),
        }
    }

    /// Replaces internal layer/object identifiers before an external publication.
    #[must_use]
    pub fn with_opaque_id(&self, opaque_id: &str) -> Self {
        match self {
            Self::Viewport => Self::Viewport,
            Self::Layer { .. } => Self::Layer {
                id: format!("layer.{opaque_id}"),
            },
            Self::Object { .. } => Self::Object {
                id: format!("object.{opaque_id}"),
            },
        }
    }
}

/// Renderer family recorded by selected capture metadata.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSelectedCaptureRenderer {
    #[default]
    NativeRichTextObserver,
    SharedWebGpuScene,
    NativeWgpuAdapter,
}

impl From<CaptureRendererKind> for AgentSelectedCaptureRenderer {
    fn from(value: CaptureRendererKind) -> Self {
        match value {
            CaptureRendererKind::NativeRichTextObserver => Self::NativeRichTextObserver,
            CaptureRendererKind::SharedWebGpuScene => Self::SharedWebGpuScene,
            CaptureRendererKind::NativeWgpuAdapter => Self::NativeWgpuAdapter,
        }
    }
}

/// Selected capture scope mirrored into the Agent protocol contract.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentSelectedCaptureScope {
    #[default]
    Viewport,
    Layer {
        id: String,
    },
    Object {
        id: String,
    },
}

impl AgentSelectedCaptureScope {
    /// Replaces internal layer/object identifiers before an external publication.
    #[must_use]
    pub fn with_opaque_id(&self, opaque_id: &str) -> Self {
        match self {
            Self::Viewport => Self::Viewport,
            Self::Layer { .. } => Self::Layer {
                id: format!("layer.{opaque_id}"),
            },
            Self::Object { .. } => Self::Object {
                id: format!("object.{opaque_id}"),
            },
        }
    }
}

impl From<CaptureScope> for AgentSelectedCaptureScope {
    fn from(value: CaptureScope) -> Self {
        match value {
            CaptureScope::Viewport => Self::Viewport,
            CaptureScope::Layer { id } => Self::Layer { id },
            CaptureScope::Object { id } => Self::Object { id },
        }
    }
}

impl From<&AgentImageScope> for AgentSelectedCaptureScope {
    fn from(value: &AgentImageScope) -> Self {
        match value {
            AgentImageScope::Viewport => Self::Viewport,
            AgentImageScope::Layer { id } => Self::Layer { id: id.clone() },
            AgentImageScope::Object { id } => Self::Object { id: id.clone() },
        }
    }
}

/// Selected capture composition mirrored from layout/protocol image composition.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentSelectedCaptureComposition {
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

impl From<CaptureComposition> for AgentSelectedCaptureComposition {
    fn from(value: CaptureComposition) -> Self {
        match value {
            CaptureComposition::Framebuffer => Self::Framebuffer,
            CaptureComposition::OverlayVector => Self::OverlayVector,
            CaptureComposition::FramebufferCrop => Self::FramebufferCrop,
            CaptureComposition::ObjectIdAttachment => Self::ObjectIdAttachment,
            CaptureComposition::MaskAttachment => Self::MaskAttachment,
            CaptureComposition::MaskedFramebufferCrop => Self::MaskedFramebufferCrop,
            CaptureComposition::IsolatedRegions => Self::IsolatedRegions,
            CaptureComposition::DebugGeometry => Self::DebugGeometry,
        }
    }
}

impl From<AgentImageComposition> for AgentSelectedCaptureComposition {
    fn from(value: AgentImageComposition) -> Self {
        match value {
            AgentImageComposition::Framebuffer => Self::Framebuffer,
            AgentImageComposition::OverlayVector => Self::OverlayVector,
            AgentImageComposition::FramebufferCrop => Self::FramebufferCrop,
            AgentImageComposition::ObjectIdAttachment => Self::ObjectIdAttachment,
            AgentImageComposition::MaskAttachment => Self::MaskAttachment,
            AgentImageComposition::MaskedFramebufferCrop => Self::MaskedFramebufferCrop,
            AgentImageComposition::IsolatedRegions => Self::IsolatedRegions,
            AgentImageComposition::DebugGeometry => Self::DebugGeometry,
        }
    }
}

/// Crop bounds for a selected capture.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentSelectedCaptureCrop {
    pub basis: LayoutCoordinateSpace,
    pub unclipped: LayoutRect,
    pub clipped: LayoutRect,
}

impl From<CaptureCropBounds> for AgentSelectedCaptureCrop {
    fn from(value: CaptureCropBounds) -> Self {
        Self {
            basis: value.basis,
            unclipped: value.unclipped,
            clipped: value.clipped,
        }
    }
}

/// Whether a selected capture mask is available for a renderer path.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentCaptureMaskAvailability {
    #[default]
    Available,
    Unavailable,
    Unsupported,
}

/// Mask and object-id metadata for a selected capture.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentSelectedCaptureMask {
    pub availability: AgentCaptureMaskAvailability,
    pub basis: LayoutCoordinateSpace,
    pub bounds: LayoutRect,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layer_ids: Vec<String>,
    pub has_object_id_attachment: bool,
    pub has_alpha_mask: bool,
}

impl AgentSelectedCaptureMask {
    #[must_use]
    pub fn unavailable(bounds: LayoutRect, basis: LayoutCoordinateSpace) -> Self {
        Self {
            availability: AgentCaptureMaskAvailability::Unavailable,
            basis,
            bounds,
            object_ids: Vec::new(),
            layer_ids: Vec::new(),
            has_object_id_attachment: false,
            has_alpha_mask: false,
        }
    }

    #[must_use]
    pub fn unsupported(bounds: LayoutRect, basis: LayoutCoordinateSpace) -> Self {
        Self {
            availability: AgentCaptureMaskAvailability::Unsupported,
            ..Self::unavailable(bounds, basis)
        }
    }

    fn scrub_for_external_publication(&mut self, opaque_scope_id: &str) {
        for (index, object_id) in self.object_ids.iter_mut().enumerate() {
            *object_id = format!("object.{opaque_scope_id}.{index}");
        }
        for (index, layer_id) in self.layer_ids.iter_mut().enumerate() {
            *layer_id = format!("layer.{opaque_scope_id}.{index}");
        }
    }
}

impl From<CaptureMaskMetadata> for AgentSelectedCaptureMask {
    fn from(value: CaptureMaskMetadata) -> Self {
        Self {
            availability: AgentCaptureMaskAvailability::Available,
            basis: value.basis,
            bounds: value.bounds,
            object_ids: value.object_ids,
            layer_ids: value.layer_ids,
            has_object_id_attachment: value.has_object_id_attachment,
            has_alpha_mask: value.has_alpha_mask,
        }
    }
}

/// Rich-text source identity preserved for selected rich-text child captures.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentCaptureRichTextIdentity {
    pub kind: crate::rich_text::AgentRichTextElementKind,
    pub index: usize,
    pub page: usize,
    pub range_start: usize,
    pub range_end: usize,
    pub node_index: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_layer: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object_depth: Option<i32>,
}

impl From<&AgentRichTextElementRef> for AgentCaptureRichTextIdentity {
    fn from(value: &AgentRichTextElementRef) -> Self {
        Self {
            kind: value.kind,
            index: value.index,
            page: value.page,
            range_start: value.range.start,
            range_end: value.range.end,
            node_index: value.node_index,
            object_layer: value.object_layer.clone(),
            object_depth: value.object_depth,
        }
    }
}

/// Stable source identity for a selected capture.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentCaptureSourceIdentity {
    Viewport {
        width: u32,
        height: u32,
    },
    Layer {
        id: String,
        object_count: usize,
    },
    Object {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        entity: Option<String>,
        layer: String,
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        object_layer: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        object_depth: Option<i32>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        rich_text: Option<AgentCaptureRichTextIdentity>,
    },
}

impl AgentCaptureSourceIdentity {
    #[must_use]
    pub const fn viewport(width: u32, height: u32) -> Self {
        Self::Viewport { width, height }
    }

    #[must_use]
    pub fn from_layer(layer: &crate::object::AgentObservedLayer) -> Self {
        Self::Layer {
            id: layer.id.clone(),
            object_count: layer.object_count,
        }
    }

    #[must_use]
    pub fn from_object(object: &crate::object::AgentObservedObject) -> Self {
        Self::Object {
            id: object.id.clone(),
            parent_id: object.parent_id.clone(),
            entity: object.entity.clone(),
            layer: object.layer.clone(),
            role: object.role.clone(),
            object_layer: object.resolved_object_layer(),
            object_depth: object.resolved_object_depth(),
            rich_text: object
                .rich_text_ref
                .as_ref()
                .map(AgentCaptureRichTextIdentity::from),
        }
    }

    fn scrub_for_external_publication(&mut self, opaque_scope_id: &str) {
        match self {
            Self::Viewport { .. } => {}
            Self::Layer { id, .. } => {
                *id = format!("layer.{opaque_scope_id}");
            }
            Self::Object {
                id,
                parent_id,
                entity,
                layer,
                object_layer,
                rich_text,
                ..
            } => {
                *id = format!("object.{opaque_scope_id}");
                *parent_id = None;
                *entity = None;
                *layer = format!("layer.{opaque_scope_id}");
                *object_layer = None;
                *rich_text = None;
            }
        }
    }
}

/// Typed selected object/layer capture metadata carried by image resources and refs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentSelectedCaptureMetadata {
    pub renderer: AgentSelectedCaptureRenderer,
    pub scope: AgentSelectedCaptureScope,
    pub composition: AgentSelectedCaptureComposition,
    pub coordinate_basis: LayoutCoordinateSpace,
    pub crop: AgentSelectedCaptureCrop,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<AgentSelectedCaptureMask>,
    pub fit_transform: FitTransformMetadata,
    pub source: AgentCaptureSourceIdentity,
}

impl AgentSelectedCaptureMetadata {
    /// Converts layout-owned renderer-independent capture metadata into the protocol shape.
    #[must_use]
    pub fn from_layout(metadata: CaptureMetadata, source: AgentCaptureSourceIdentity) -> Self {
        Self {
            renderer: metadata.renderer.into(),
            scope: metadata.scope.into(),
            composition: metadata.composition.into(),
            coordinate_basis: metadata.coordinate_basis,
            crop: metadata.crop.into(),
            mask: metadata.mask.map(AgentSelectedCaptureMask::from),
            fit_transform: metadata.fit_transform,
            source,
        }
    }

    #[must_use]
    pub fn with_mask(mut self, mask: Option<AgentSelectedCaptureMask>) -> Self {
        self.mask = mask;
        self
    }

    pub fn scrub_for_external_publication(&mut self, opaque_scope_id: &str) {
        self.scope = self.scope.with_opaque_id(opaque_scope_id);
        self.source.scrub_for_external_publication(opaque_scope_id);
        if let Some(mask) = &mut self.mask {
            mask.scrub_for_external_publication(opaque_scope_id);
        }
    }
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

impl AgentImageKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Color => "color",
            Self::Overlay => "overlay",
            Self::OverlaySvg => "overlay_svg",
            Self::ObjectId => "object_id",
            Self::Mask => "mask",
        }
    }

    /// Object-id and mask images are policy auxiliaries, not ordinary visual output.
    pub const fn is_policy_auxiliary(self) -> bool {
        matches!(self, Self::ObjectId | Self::Mask)
    }

    /// Canonical composition for a capture reference of this kind.
    pub const fn default_capture_composition(self) -> AgentImageComposition {
        match self {
            Self::Color => AgentImageComposition::FramebufferCrop,
            Self::ObjectId => AgentImageComposition::ObjectIdAttachment,
            Self::Mask => AgentImageComposition::MaskAttachment,
            Self::Overlay | Self::OverlaySvg => AgentImageComposition::OverlayVector,
        }
    }
}

/// Image fitting policy attached to an observed image object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImageFit {
    Contain,
    Cover,
    Stretch,
    Intrinsic,
}

/// Fixed-point image alignment attached to an observed image object.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageAlignment {
    pub x_milli: i32,
    pub y_milli: i32,
}

/// Fixed-point affine transform attached to an observed image object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageTransform {
    pub m11_milli: i32,
    pub m12_milli: i32,
    pub m21_milli: i32,
    pub m22_milli: i32,
    pub tx_milli: i32,
    pub ty_milli: i32,
}

/// Typed custom parameter attached to an observed image presentation object.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentImageObjectParam {
    Bool { value: bool },
    Integer { value: i64 },
    Milli { value: i32 },
    Text { value: String },
    Id { value: String },
}

/// Rendered or render-adjacent frame resource addressable by Agent tools.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<crate::diagnostic::AgentDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written: Option<String>,
}

/// Machine-readable image metadata attached to image resources.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentImageMetadata {
    pub kind: AgentImageKind,
    pub renderer: AgentImageRenderer,
    pub scope: AgentImageScope,
    pub composition: AgentImageComposition,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub capture_step: usize,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub capture_time_millis: u32,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<crate::diagnostic::AgentDiagnostic>,
}

impl AgentImageMetadata {
    pub(crate) fn from_image_resource(image: &AgentImageResource) -> Self {
        let is_raw_rgba = image.mime_type == "application/octet-stream"
            && image.uri.rsplit('.').next() == Some("rgba");
        Self {
            kind: image.kind,
            renderer: image.renderer,
            scope: image.scope.clone(),
            composition: image.composition,
            page: image.page,
            capture_step: image.capture_step,
            capture_time_millis: image.capture_time_millis,
            width: image.width,
            height: image.height,
            crop_origin: image.crop_origin,
            pixel_format: is_raw_rgba.then(|| "rgba8_unorm".to_owned()),
            row_stride_bytes: is_raw_rgba.then(|| image.width.saturating_mul(4)),
            content_bbox: image.content_bbox,
            content_viewport_bbox: image.content_viewport_bbox,
            content_pixels: image.content_pixels,
            object: image.object.clone(),
            selected_capture: image.selected_capture.clone(),
            diagnostics: image.diagnostics.clone(),
        }
    }

    /// Removes free-form object/diagnostic metadata and internal scope ids before
    /// this metadata is attached to an externally visible resource.
    pub fn scrub_for_external_publication(&mut self, opaque_scope_id: &str) {
        self.scope = self.scope.with_opaque_id(opaque_scope_id);
        self.object = None;
        self.diagnostics.clear();
        if let Some(selected_capture) = &mut self.selected_capture {
            selected_capture.scrub_for_external_publication(opaque_scope_id);
        }
    }

    pub fn description(&self, mime_type: &str) -> String {
        let page = if self.page == 0 {
            String::new()
        } else {
            format!(", page={}", self.page)
        };
        format!(
            "Agent Debug Bus image resource (mime_type={mime_type}, kind={}, renderer={}, scope={}, composition={}{page}, width={}, height={})",
            self.kind.as_str(),
            self.renderer.as_str(),
            self.scope.description(),
            self.composition.as_str(),
            self.width,
            self.height,
        )
    }
}

/// Observed object metadata preserved on an image resource that captures an object.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_ref: Option<AgentImageObjectContentRef>,
}

/// Image payload summary preserved on object-scoped capture resources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentImageObjectContentRef {
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub object: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame_index: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_time_millis: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity_milli: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit: Option<AgentImageFit>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alignment: Option<AgentImageAlignment>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<AgentImageTransform>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intrinsic_height: Option<u32>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub params: BTreeMap<String, AgentImageObjectParam>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proxies: Vec<AgentPresentationObjectProxyRef>,
}

impl AgentImageObjectRef {
    /// Copies stable object identity and typed object payload links into image metadata.
    pub fn from_observed(object: &crate::object::AgentObservedObject) -> Self {
        Self {
            id: object.id.clone(),
            parent_id: object.parent_id.clone(),
            entity: object.entity.clone(),
            layer: object.layer.clone(),
            role: object.role.clone(),
            bbox: object.bbox.clone(),
            polygon: object.polygon.clone(),
            capture_refs: object.capture_refs.clone(),
            object_layer: object.resolved_object_layer(),
            object_depth: object.resolved_object_depth(),
            text: object.text.clone(),
            rich_text_ref: object.rich_text_ref.clone(),
            image_ref: object.image_content_ref(),
        }
    }
}

impl From<&crate::object::AgentObservedImageContent> for AgentImageObjectContentRef {
    fn from(content: &crate::object::AgentObservedImageContent) -> Self {
        Self {
            source: content.source.clone(),
            object: content.object.clone(),
            target: content.target.clone(),
            asset: content.asset.clone(),
            frame_index: content.frame_index,
            local_time_millis: content.local_time_millis,
            opacity_milli: content.opacity_milli,
            fit: content.fit,
            alignment: content.alignment,
            transform: content.transform.clone(),
            intrinsic_width: content.intrinsic_width,
            intrinsic_height: content.intrinsic_height,
            actions: content.actions.clone(),
            params: content.params.clone(),
            proxies: content.proxies.clone(),
        }
    }
}
