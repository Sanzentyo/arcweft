use crate::geometry::{AgentBBox, AgentCoordinateSpace, AgentPoint};
use crate::rich_text::AgentRichTextElementRef;
use crate::serde_helpers::{is_zero, is_zero_u32};
use arcweft_layout::{
    CaptureComposition, CaptureCropBounds, CaptureMaskMetadata, CaptureMetadata,
    CaptureRendererKind, CaptureScope, FitTransformMetadata, LayoutCoordinateSpace, LayoutRect,
};
use arcweft_text_model::RichTextNodeIndex;
use serde::{Deserialize, Serialize};

/// Closed raw pixel representation carried by the actual capture producer.
/// Encoded PNG/SVG resources carry no pixel format or row stride.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentImagePixelFormat {
    Rgba8Unorm,
}

#[derive(Clone, Debug, thiserror::Error, Eq, PartialEq)]
pub enum AgentImageFormatError {
    #[error("image extent {width}x{height} is empty or cannot describe its pixel buffer")]
    InvalidExtent { width: u32, height: u32 },
    #[error("image MIME type {mime_type} disagrees with the declared raw pixel format")]
    MimeType { mime_type: String },
    #[error("image row stride does not match its declared pixel format and extent")]
    RowStride,
}

impl AgentImagePixelFormat {
    pub fn row_stride_bytes(self, width: u32, height: u32) -> Result<u32, AgentImageFormatError> {
        let stride = width.checked_mul(4).filter(|_| width != 0 && height != 0);
        let bytes = usize::try_from(width)
            .ok()
            .zip(usize::try_from(height).ok())
            .and_then(|(width, height)| width.checked_mul(height)?.checked_mul(4));
        match (stride, bytes) {
            (Some(stride), Some(_)) => Ok(stride),
            _ => Err(AgentImageFormatError::InvalidExtent { width, height }),
        }
    }
}

pub fn validate_agent_image_format(
    pixel_format: Option<AgentImagePixelFormat>,
    mime_type: &str,
    width: u32,
    height: u32,
) -> Result<Option<u32>, AgentImageFormatError> {
    if width == 0 || height == 0 {
        return Err(AgentImageFormatError::InvalidExtent { width, height });
    }
    if pixel_format.is_some() != (mime_type == "application/octet-stream") {
        return Err(AgentImageFormatError::MimeType {
            mime_type: mime_type.to_owned(),
        });
    }
    pixel_format
        .map(|format| format.row_stride_bytes(width, height))
        .transpose()
}

/// Capture resources addressable for one observed view boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentViewCaptureRefs {
    pub captures: Vec<AgentViewCaptureRef>,
}

/// One image capture resource that can be requested for a view.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentViewCaptureRef {
    pub kind: AgentImageKind,
    pub uri: String,
    pub mime_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<AgentImagePixelFormat>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub page: usize,
    pub width: u32,
    pub height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<AgentImagePixelFormat>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<AgentImagePixelFormat>,
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
    View {
        id: String,
    },
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
            Self::View { id } => format!("view:{id}"),
            Self::Layer { id } => format!("layer:{id}"),
            Self::Object { id } => format!("object:{id}"),
        }
    }

    /// Replaces internal layer/object identifiers before an external publication.
    #[must_use]
    pub fn with_opaque_id(&self, opaque_id: &str) -> Self {
        match self {
            Self::Viewport => Self::Viewport,
            Self::View { .. } => Self::View {
                id: format!("view.{opaque_id}"),
            },
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
    SharedWgpuPreparedFrame,
}

impl From<CaptureRendererKind> for AgentSelectedCaptureRenderer {
    fn from(value: CaptureRendererKind) -> Self {
        match value {
            CaptureRendererKind::SharedWgpuPreparedFrame => Self::SharedWgpuPreparedFrame,
        }
    }
}

/// Selected capture scope mirrored into the Agent protocol contract.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentSelectedCaptureScope {
    #[default]
    Viewport,
    View {
        id: String,
    },
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
            Self::View { .. } => Self::View {
                id: format!("view.{opaque_id}"),
            },
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
            CaptureScope::View { id } => Self::View { id },
            CaptureScope::Layer { id } => Self::Layer { id },
            CaptureScope::Object { id } => Self::Object { id },
        }
    }
}

impl From<&AgentImageScope> for AgentSelectedCaptureScope {
    fn from(value: &AgentImageScope) -> Self {
        match value {
            AgentImageScope::Viewport => Self::Viewport,
            AgentImageScope::View { id } => Self::View { id: id.clone() },
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
    pub node_index: RichTextNodeIndex,
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
    View {
        id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent_id: Option<String>,
        object_count: usize,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        object_refs: Vec<String>,
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
    pub fn from_view(view: &crate::object::AgentObservedView) -> Self {
        Self::View {
            id: view.id.clone(),
            parent_id: view.parent_id.clone(),
            object_count: view.object_count,
            object_refs: view.object_refs.clone(),
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
            Self::View {
                id,
                parent_id,
                object_count,
                object_refs,
            } => {
                *id = format!("view.{opaque_scope_id}");
                *parent_id = None;
                *object_refs = (0..*object_count)
                    .map(|index| format!("object.{opaque_scope_id}.{index}"))
                    .collect();
            }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pixel_format: Option<AgentImagePixelFormat>,
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
    pub view: Option<AgentImageViewRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<crate::diagnostic::AgentDiagnostic>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub written: Option<String>,
}

/// Machine-readable image metadata attached to image resources.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentImageMetadata {
    pub kind: AgentImageKind,
    pub renderer: AgentImageRenderer,
    pub scope: AgentImageScope,
    pub composition: AgentImageComposition,
    pub page: usize,
    pub capture_step: usize,
    pub capture_time_millis: u32,
    pub width: u32,
    pub height: u32,
    pub crop_origin: Option<AgentImageCropOrigin>,
    pub pixel_format: Option<AgentImagePixelFormat>,
    pub row_stride_bytes: Option<u32>,
    pub content_bbox: Option<AgentImageContentBBox>,
    pub content_viewport_bbox: Option<AgentImageContentBBox>,
    pub content_pixels: Option<u64>,
    pub object: Option<AgentImageObjectRef>,
    pub view: Option<AgentImageViewRef>,
    pub selected_capture: Option<AgentSelectedCaptureMetadata>,
    pub diagnostics: Vec<crate::diagnostic::AgentDiagnostic>,
    pub text_contexts: Option<crate::text_context::AgentTextContextCatalog>,
}
#[derive(Deserialize, Serialize)]
#[serde(remote = "AgentImageMetadata")]
struct AgentImageMetadataWire {
    kind: AgentImageKind,
    renderer: AgentImageRenderer,
    scope: AgentImageScope,
    composition: AgentImageComposition,
    #[serde(default, skip_serializing_if = "is_zero")]
    page: usize,
    #[serde(default, skip_serializing_if = "is_zero")]
    capture_step: usize,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    capture_time_millis: u32,
    width: u32,
    height: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    crop_origin: Option<AgentImageCropOrigin>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pixel_format: Option<AgentImagePixelFormat>,
    #[serde(skip_serializing_if = "Option::is_none")]
    row_stride_bytes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_viewport_bbox: Option<AgentImageContentBBox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_pixels: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    object: Option<AgentImageObjectRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    view: Option<AgentImageViewRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    selected_capture: Option<AgentSelectedCaptureMetadata>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    diagnostics: Vec<crate::diagnostic::AgentDiagnostic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    text_contexts: Option<crate::text_context::AgentTextContextCatalog>,
}

impl AgentImageMetadata {
    pub(crate) fn from_image_resource(
        image: &AgentImageResource,
        text_contexts: Option<crate::text_context::AgentTextContextCatalog>,
    ) -> Result<Self, AgentImageFormatError> {
        let row_stride_bytes = validate_agent_image_format(
            image.pixel_format,
            &image.mime_type,
            image.width,
            image.height,
        )?;
        Ok(Self {
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
            pixel_format: image.pixel_format,
            row_stride_bytes,
            content_bbox: image.content_bbox,
            content_viewport_bbox: image.content_viewport_bbox,
            content_pixels: image.content_pixels,
            object: image.object.clone(),
            view: image.view.clone(),
            selected_capture: image.selected_capture.clone(),
            diagnostics: image.diagnostics.clone(),
            text_contexts,
        })
    }

    /// Removes free-form object/diagnostic metadata and internal scope ids before
    /// this metadata is attached to an externally visible resource.
    pub fn scrub_for_external_publication(&mut self, opaque_scope_id: &str) {
        self.text_contexts = None;
        self.scope = self.scope.with_opaque_id(opaque_scope_id);
        self.object = None;
        self.view = None;
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
#[serde(deny_unknown_fields)]
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_context: Option<crate::text_context::AgentTextContextReference>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rich_text_ref: Option<AgentRichTextElementRef>,
    pub content: crate::object::AgentObservedObjectContent,
}

impl AgentImageObjectRef {
    pub fn validate_text_context(
        &self,
        catalog: &crate::text_context::AgentTextContextCatalog,
    ) -> Result<(), crate::text_context::AgentTextContextError> {
        self.content
            .validate_text_context_family(&self.id, self.text_context.as_ref(), catalog)?;
        if let Some(reference) = &self.text_context {
            if self.parent_id
                != catalog
                    .resolve(reference)?
                    .element_parent_id(self.rich_text_ref.as_ref())?
            {
                return Err(crate::text_context::AgentTextContextError::ObjectOwner {
                    object_id: self.id.clone(),
                });
            }
            catalog.validate_element(
                &self.id,
                reference,
                self.rich_text_ref.as_ref(),
                self.text.as_deref(),
            )?;
        } else if self.rich_text_ref.is_some() {
            return Err(
                crate::text_context::AgentTextContextError::MissingObjectContext {
                    object_id: self.id.clone(),
                },
            );
        }
        Ok(())
    }

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
            text_context: object.text_context.clone(),
            rich_text_ref: object.rich_text_ref.clone(),
            content: object.content.clone(),
        }
    }
}

/// Observed view metadata preserved on an image resource that captures a view.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentImageViewRef {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub visible: bool,
    pub bbox: AgentBBox,
    pub object_count: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub object_refs: Vec<String>,
    pub capture_refs: AgentViewCaptureRefs,
}

impl AgentImageViewRef {
    pub fn from_observed(view: &crate::object::AgentObservedView) -> Self {
        Self {
            id: view.id.clone(),
            parent_id: view.parent_id.clone(),
            visible: view.visible,
            bbox: view.bbox.clone(),
            object_count: view.object_count,
            object_refs: view.object_refs.clone(),
            capture_refs: view.capture_refs.clone(),
        }
    }
}

impl AgentImageMetadata {
    pub fn decode_json(bytes: &[u8]) -> Result<Self, crate::text_context::AgentTextContextError> {
        crate::text_context::decode_text_context_envelope(bytes)
    }

    pub fn validate_text_contexts(&self) -> Result<(), crate::text_context::AgentTextContextError> {
        if self.width == 0 || self.height == 0 {
            return Err(AgentImageFormatError::InvalidExtent {
                width: self.width,
                height: self.height,
            }
            .into());
        }
        if self.row_stride_bytes
            != self
                .pixel_format
                .map(|format| format.row_stride_bytes(self.width, self.height))
                .transpose()?
        {
            return Err(AgentImageFormatError::RowStride.into());
        }
        if let Some(object) = &self.object {
            if object.text_context.is_some()
                && !matches!(&self.scope, AgentImageScope::Object { id } if id == &object.id)
            {
                return Err(crate::text_context::AgentTextContextError::ObjectOwner {
                    object_id: object.id.clone(),
                });
            }
            match &self.text_contexts {
                Some(catalog) => {
                    object.validate_text_context(catalog)?;
                    if let Some(reference) = &object.text_context {
                        let entry = catalog.resolve(reference)?;
                        if catalog.scope().tick.checked_add(1) != Some(self.capture_step) {
                            return Err(crate::text_context::AgentTextContextError::ScopeMismatch);
                        }
                        if self.page != entry.content.display_stage().map_or(0, |stage| stage.frame().page_index)
                            || self.selected_capture.as_ref().is_some_and(|selected| {
                                !matches!(&selected.source, AgentCaptureSourceIdentity::Object { id, parent_id, rich_text, .. }
                                    if id == &object.id && parent_id == &object.parent_id && rich_text.as_ref() == object.rich_text_ref.as_ref().map(AgentCaptureRichTextIdentity::from).as_ref())
                            })
                        {
                            return Err(crate::text_context::AgentTextContextError::ObjectOwner { object_id: object.id.clone() });
                        }
                    }
                    if catalog
                        .scope()
                        .capture_time_millis
                        .is_some_and(|time| time != self.capture_time_millis)
                    {
                        return Err(crate::text_context::AgentTextContextError::ScopeMismatch);
                    }
                }
                None if object.content.requires_text_context()
                    || object.text_context.is_some()
                    || object.rich_text_ref.is_some() =>
                {
                    return Err(
                        crate::text_context::AgentTextContextError::MissingObjectContext {
                            object_id: object.id.clone(),
                        },
                    );
                }
                None => {}
            }
        } else if self
            .text_contexts
            .as_ref()
            .is_some_and(|catalog| catalog.entries().len() != 0)
        {
            return Err(crate::text_context::AgentTextContextError::ScopeMismatch);
        }
        crate::text_context::validate_text_context_envelope(|writer| {
            AgentImageMetadataWire::serialize(self, &mut serde_json::Serializer::new(writer))
        })
    }
}
impl Serialize for AgentImageMetadata {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.validate_text_contexts()
            .map_err(serde::ser::Error::custom)?;
        AgentImageMetadataWire::serialize(self, serializer)
    }
}
impl<'de> Deserialize<'de> for AgentImageMetadata {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let metadata = AgentImageMetadataWire::deserialize(deserializer)?;
        metadata
            .validate_text_contexts()
            .map_err(serde::de::Error::custom)?;
        Ok(metadata)
    }
}
