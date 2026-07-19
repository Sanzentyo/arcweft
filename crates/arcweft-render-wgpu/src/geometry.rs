use crate::convert::saturating_usize_as_f32;
use crate::view_mask::ViewMaskChannel;
use crate::view_scene::{ViewMaskImage, ViewScene};
use arcweft_glyphon::{
    GlyphonTextEngine, GlyphonTextEngineError, PreparedTextBatch, PreparedTextError,
    PreparedTextId, PreparedTextItem,
};
use arcweft_id::PublicId;
use arcweft_layout::ContentRect;
use arcweft_presentation::fx::{FiniteF32Error, FxTarget, Transform2DError};
use arcweft_presentation::hit::{HitRect, HitTree};
use arcweft_presentation::input::{InteractionTarget, ViewportPoint};
use arcweft_presentation::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    RenderPhase,
};
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_presentation::text_input::{
    TextGeometryTransform, TextInputClientSnapshot, TextInputGeometrySnapshot,
};
use arcweft_render_text::{
    ResolvedTextDocument, ResolvedTextRunSource, ResolvedTextStyle, RichTextRange, TextFontFamily,
    TextResolveError, TextSlant, TextWeight,
};
use arcweft_text_layout::{LayoutPoint, LayoutRect, LayoutSize, TextLayoutError};
use num_traits::ToPrimitive;
use thiserror::Error;

mod action_buttons;
mod control_style;
mod dialogue_prepared;
mod dialogue_timeline;
mod dialogue_transform;
mod focus_navigation;
mod images;
mod prepared_text;
mod scroll;
mod text_controls;
pub mod view_final;
pub use action_buttons::{PreparedActionButton, RenderActionButton, RenderActionButtonAction};
pub use control_style::{
    PreparedControlBackdrop, PreparedControlFilter, PreparedControlPaint, PreparedControlShadow,
    RenderControlBorderStyle, RenderControlCornerFrameStyle, RenderControlFilter,
    RenderControlFilterList, RenderControlFocusRingStyle, RenderControlShadow,
    RenderControlShadowKind, RenderControlVisualStyle, RuntimeControlBackdropSamplePolicy,
};
pub use focus_navigation::{
    FocusNavigationDebug, FocusNavigationDebugCandidate, PreparedFocusGraph, PreparedFocusGroup,
    PreparedFocusNavigationTarget, RenderFocusGroup, RenderFocusGroupPolicy,
    RenderFocusInitialPolicy, RenderFocusNavigation, RenderFocusNavigationEdge,
    RenderFocusSkipPolicy, RenderFocusTargetResolution, RenderFocusWrapPolicy,
};
pub use images::{RenderImage, RenderImageFrame, RenderImageQuad, RenderImageTransformMatrix};
pub use scroll::{
    PreparedScrollIndicator, RenderFocusAutoScrollPolicy, RenderScrollAxis,
    RenderScrollIndicatorsPolicy, RenderScrollOverflow, RenderScrollOverscrollPolicy,
    RenderScrollRegion,
};
pub use text_controls::RenderTextInputControl;

/// Logical viewport shared by visual planning and hit-testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderViewport {
    pub logical_width: f32,
    pub logical_height: f32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor: f64,
}

impl RenderViewport {
    /// Returns the host-validated physical scale factor for renderer APIs.
    ///
    /// Native, Web, and headless adapters validate this value before frame
    /// preparation. This conversion deliberately has no renderer fallback.
    #[must_use]
    pub fn physical_scale_factor_f32(self) -> f32 {
        debug_assert!(self.scale_factor.is_finite() && self.scale_factor > 0.0);
        debug_assert!(self.scale_factor <= f64::from(f32::MAX));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "host adapters validate the finite positive f32 range before preparation"
        )]
        let scale_factor = self.scale_factor as f32;
        scale_factor
    }
}

/// User-facing presentation preferences independent of platform APIs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderPreferences {
    pub text_scale_milli: u16,
    pub high_contrast: bool,
    pub reduce_motion: bool,
}

/// Choice list scroll state in logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChoiceScroll {
    pub offset_y: f32,
}

/// Frame-crossing interaction visuals rendered into the canvas.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InteractionVisualState {
    pub focused: Option<InteractionTarget>,
    pub hovered: Option<InteractionTarget>,
    pub pressed: Option<InteractionTarget>,
}

/// Logical direction for keyboard, controller, or accessibility focus movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusNavigationDirection {
    Up,
    Down,
    Left,
    Right,
    Next,
    Previous,
}

/// Renderer input assembled by the player from portable runtime state.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderScene {
    /// Layout regions that auto-positioned content, such as choices, must not overlap.
    pub content_avoidance_regions: Vec<HitRect>,
    pub choices: Vec<RenderChoiceItem>,
    pub text_inputs: Vec<RenderTextInputControl>,
    pub action_buttons: Vec<RenderActionButton>,
    pub focus_groups: Vec<RenderFocusGroup>,
    pub focus_navigation: Vec<RenderFocusNavigation>,
    pub images: Vec<RenderImage>,
    pub viewport: RenderViewport,
    pub visual_time_millis: u64,
    pub preferences: RenderPreferences,
    pub interaction: InteractionVisualState,
    pub choice_scroll: ChoiceScroll,
    pub scroll_regions: Vec<RenderScrollRegion>,
}

/// One retained View scene attached to the normal renderer frame.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedViewScene {
    pub scene: ViewScene,
    pub resources: PreparedViewSceneResources,
}

/// Backend-neutral resource payloads required by one prepared `ViewScene`.
///
/// The player/runtime adapter resolves URLs and bundle assets before the frame
/// reaches `arcweft-render-wgpu`; text is referenced directly from the frame's
/// canonical prepared batch, and this resource type contains no I/O.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedViewSceneResources {
    images: Vec<PreparedViewImageResource>,
    masks: Vec<PreparedViewMaskResource>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedViewImageResource {
    pub resource_index: u32,
    pub frame: RenderImageFrame,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PreparedViewMaskResource {
    pub image: ViewMaskImage,
    pub frame: RenderImageFrame,
    pub channel: ViewMaskChannel,
}

/// Portable choice data supplied by a player/runtime adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderChoiceItem {
    pub id: String,
    pub label: String,
}

/// One colored rectangle in logical viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintRect {
    pub bounds: HitRect,
    pub rgba: [f32; 4],
    pub radii: PaintRectRadii,
    pub stroke_width_px: f32,
    pub clip: Option<PaintRectClip>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PaintRectClip {
    pub bounds: HitRect,
    pub radii: PaintRectRadii,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintRectRadii {
    pub top_left: PaintRectCornerRadius,
    pub top_right: PaintRectCornerRadius,
    pub bottom_right: PaintRectCornerRadius,
    pub bottom_left: PaintRectCornerRadius,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PaintRectCornerRadius {
    pub x_px: f32,
    pub y_px: f32,
}

impl PaintRect {
    #[must_use]
    pub const fn new(bounds: HitRect, rgba: [f32; 4]) -> Self {
        Self {
            bounds,
            rgba,
            radii: PaintRectRadii::ZERO,
            stroke_width_px: 0.0,
            clip: None,
        }
    }

    #[must_use]
    pub const fn rounded(bounds: HitRect, rgba: [f32; 4], radius_px: f32) -> Self {
        Self::with_radii(bounds, rgba, PaintRectRadii::uniform(radius_px))
    }

    #[must_use]
    pub const fn with_radii(bounds: HitRect, rgba: [f32; 4], radii: PaintRectRadii) -> Self {
        Self {
            bounds,
            rgba,
            radii,
            stroke_width_px: 0.0,
            clip: None,
        }
    }

    #[must_use]
    pub const fn stroke(
        bounds: HitRect,
        rgba: [f32; 4],
        radii: PaintRectRadii,
        stroke_width_px: f32,
    ) -> Self {
        Self {
            bounds,
            rgba,
            radii,
            stroke_width_px,
            clip: None,
        }
    }

    #[must_use]
    pub const fn clipped_to(mut self, bounds: HitRect, radius_px: f32) -> Self {
        self.clip = Some(PaintRectClip {
            bounds,
            radii: PaintRectRadii::uniform(radius_px),
        });
        self
    }

    #[must_use]
    pub const fn clipped_to_radii(mut self, bounds: HitRect, radii: PaintRectRadii) -> Self {
        self.clip = Some(PaintRectClip { bounds, radii });
        self
    }
}

impl PaintRectRadii {
    pub const ZERO: Self = Self {
        top_left: PaintRectCornerRadius::ZERO,
        top_right: PaintRectCornerRadius::ZERO,
        bottom_right: PaintRectCornerRadius::ZERO,
        bottom_left: PaintRectCornerRadius::ZERO,
    };

    #[must_use]
    pub const fn uniform(radius_px: f32) -> Self {
        let radius = PaintRectCornerRadius::circular(radius_px);
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    #[must_use]
    pub const fn new(
        top_left: PaintRectCornerRadius,
        top_right: PaintRectCornerRadius,
        bottom_right: PaintRectCornerRadius,
        bottom_left: PaintRectCornerRadius,
    ) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    #[must_use]
    pub fn outset(self, amount_px: f32) -> Self {
        Self {
            top_left: self.top_left.outset(amount_px),
            top_right: self.top_right.outset(amount_px),
            bottom_right: self.bottom_right.outset(amount_px),
            bottom_left: self.bottom_left.outset(amount_px),
        }
    }

    #[must_use]
    pub fn scaled(self, scale_x: f32, scale_y: f32) -> Self {
        Self {
            top_left: self.top_left.scaled(scale_x, scale_y),
            top_right: self.top_right.scaled(scale_x, scale_y),
            bottom_right: self.bottom_right.scaled(scale_x, scale_y),
            bottom_left: self.bottom_left.scaled(scale_x, scale_y),
        }
    }

    #[must_use]
    pub fn normalized_for(self, width: f32, height: f32) -> Self {
        let radii = Self {
            top_left: self.top_left.non_negative(),
            top_right: self.top_right.non_negative(),
            bottom_right: self.bottom_right.non_negative(),
            bottom_left: self.bottom_left.non_negative(),
        };
        let mut scale: f32 = 1.0;
        scale = corner_scale_limit(scale, width, radii.top_left.x_px + radii.top_right.x_px);
        scale = corner_scale_limit(
            scale,
            width,
            radii.bottom_left.x_px + radii.bottom_right.x_px,
        );
        scale = corner_scale_limit(scale, height, radii.top_left.y_px + radii.bottom_left.y_px);
        scale = corner_scale_limit(
            scale,
            height,
            radii.top_right.y_px + radii.bottom_right.y_px,
        );
        radii.scaled(scale, scale)
    }
}

impl PaintRectCornerRadius {
    pub const ZERO: Self = Self {
        x_px: 0.0,
        y_px: 0.0,
    };

    #[must_use]
    pub const fn new(x_px: f32, y_px: f32) -> Self {
        Self { x_px, y_px }
    }

    #[must_use]
    pub const fn circular(radius_px: f32) -> Self {
        Self {
            x_px: radius_px,
            y_px: radius_px,
        }
    }

    #[must_use]
    pub fn outset(self, amount_px: f32) -> Self {
        Self {
            x_px: (self.x_px + amount_px).max(0.0),
            y_px: (self.y_px + amount_px).max(0.0),
        }
    }

    #[must_use]
    pub fn scaled(self, scale_x: f32, scale_y: f32) -> Self {
        Self {
            x_px: self.x_px * scale_x,
            y_px: self.y_px * scale_y,
        }
    }

    #[must_use]
    pub fn non_negative(self) -> Self {
        Self {
            x_px: self.x_px.max(0.0),
            y_px: self.y_px.max(0.0),
        }
    }
}

fn corner_scale_limit(current: f32, basis_px: f32, sum_px: f32) -> f32 {
    if sum_px <= f32::EPSILON {
        current
    } else {
        current.min((basis_px / sum_px).clamp(0.0, 1.0))
    }
}

/// Layout, interaction, and clipping contract for one canonical resolved document.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextDocumentRequest {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
    pub container_bounds: LayoutRect,
    pub clip: Option<LayoutRect>,
    pub target: Option<InteractionTarget>,
    pub selection_enabled: bool,
    pub selection: Option<RichTextRange>,
    pub selection_rgba: [f32; 4],
}

/// Layout and reveal inputs for one resolved `RichText` display stage.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedRichTextStageRequest {
    pub bounds: HitRect,
    pub default_style: ResolvedTextStyle,
    pub visual_time_millis: u64,
    pub reveal_complete: bool,
}

/// Canonical prepared item and reveal result for one `RichText` display stage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedRichTextStageResult {
    pub text: PreparedTextId,
    pub reveal_complete: bool,
    pub source_origin: usize,
}

impl PreparedTextDocumentRequest {
    #[must_use]
    pub const fn new(origin: LayoutPoint, size: LayoutSize) -> Self {
        Self {
            origin,
            size,
            container_bounds: LayoutRect::new(origin.x, origin.y, size.width, size.height),
            clip: None,
            target: None,
            selection_enabled: false,
            selection: None,
            selection_rgba: [0.25, 0.5, 1.0, 0.35],
        }
    }

    fn mapped(mut self, mapping: PreparedFrameViewportMapping) -> Self {
        self.origin = LayoutPoint::new(
            mapping.translate_x + self.origin.x * mapping.scale_x,
            mapping.translate_y + self.origin.y * mapping.scale_y,
        );
        self.size = LayoutSize::new(
            self.size.width * mapping.scale_x.abs(),
            self.size.height * mapping.scale_y.abs(),
        );
        self.container_bounds = mapping.layout_rect(self.container_bounds);
        self.clip = self.clip.map(|clip| mapping.layout_rect(clip));
        self
    }
}

struct PlannedPlainText {
    text: String,
    style: ResolvedTextStyle,
    source: ResolvedTextRunSource,
    request: PreparedTextDocumentRequest,
    owner: PlannedTextOwner,
}

struct PlannedTextOwner {
    semantic_id: PublicId,
    object_bounds: HitRect,
}

enum PlannedFrameText {
    Plain(Box<PlannedPlainText>),
    TextInput(Box<text_controls::PlannedTextInput>),
}

impl PlannedFrameText {
    fn mapped(self, mapping: PreparedFrameViewportMapping) -> Result<Self, FramePlanError> {
        match self {
            Self::Plain(mut text) => {
                text.style = mapped_text_style(text.style, mapping.text_scale)?;
                text.request = text.request.mapped(mapping);
                text.owner.object_bounds = mapping.rect(text.owner.object_bounds);
                Ok(Self::Plain(text))
            }
            Self::TextInput(text) => Ok(Self::TextInput(Box::new((*text).mapped(mapping)?))),
        }
    }
}

struct PlannedFrame {
    frame: PreparedFrame,
    text: Vec<PlannedFrameText>,
}

/// Choice geometry and stable semantic target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderChoice {
    pub option_id: String,
    pub label: String,
    pub target: InteractionTarget,
}

/// Pure frame plan consumed by the shared GPU renderer and input router.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedFrame {
    pub viewport: RenderViewport,
    pub visual_time_millis: u64,
    pub preferences: RenderPreferences,
    pub layers: LayerTree,
    pub semantics: SemanticTree,
    pub hits: HitTree,
    pub rectangles: Vec<PaintRect>,
    pub images: Vec<RenderImage>,
    /// The only text renderer input: canonical pre-shaped items in painter order.
    pub text: PreparedTextBatch,
    text_owners: Vec<PreparedTextOwner>,
    /// Typed shared-Fx evaluation/capability diagnostics for this exact frame.
    pub fx_diagnostics: Vec<arcweft_presentation::fx::FxDiagnostic>,
    pub choices: Vec<RenderChoice>,
    pub action_buttons: Vec<PreparedActionButton>,
    pub control_backdrops: Vec<PreparedControlBackdrop>,
    pub control_shadows: Vec<PreparedControlShadow>,
    pub control_filters: Vec<PreparedControlFilter>,
    pub control_paints: Vec<PreparedControlPaint>,
    pub scroll_regions: Vec<RenderScrollRegion>,
    pub scroll_indicators: Vec<PreparedScrollIndicator>,
    pub focus_graph: PreparedFocusGraph,
    view_scenes: Vec<PreparedViewScene>,
    dialogue_views: Vec<PreparedDialogueViewState>,
    interaction: InteractionVisualState,
    focused_text_input: Option<PreparedTextInputTarget>,
}

/// Runtime identity, visual progress, and actionability for one authored dialogue View.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedDialogueViewState {
    pub dialogue: u64,
    pub entry: u64,
    pub mount: u64,
    pub revision: u64,
    pub instance: u64,
    pub stage: u32,
    pub bounds: HitRect,
    pub reveal_complete: bool,
    pub advance_available: bool,
    pub primary_action: Option<arcweft_view::DialogueAdvanceTarget>,
}

/// Semantic ownership for one canonical prepared-text item.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedTextOwnerKind {
    /// Dialogue content rendered inside one persistent authored View mount.
    DialogueView {
        dialogue: u64,
        entry: u64,
        mount: u64,
    },
    /// Authored or Rust-backed View text.
    View { mount: u64 },
    /// Shared player control text associated with an interaction target.
    Control,
}

/// Renderer-neutral identity and geometry associated with one prepared item.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextOwner {
    pub text: PreparedTextId,
    pub semantic_id: PublicId,
    pub parent_id: Option<PublicId>,
    pub kind: PreparedTextOwnerKind,
    pub source_origin: usize,
    pub object_bounds: HitRect,
}

impl PreparedTextOwner {
    #[must_use]
    pub const fn new(
        text: PreparedTextId,
        semantic_id: PublicId,
        kind: PreparedTextOwnerKind,
        source_origin: usize,
        object_bounds: HitRect,
    ) -> Self {
        Self {
            text,
            semantic_id,
            parent_id: None,
            kind,
            source_origin,
            object_bounds,
        }
    }

    #[must_use]
    pub fn with_parent(mut self, parent_id: PublicId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }
}

/// Renderer-backed text input target prepared for platform IME adapters.
///
/// This intentionally contains Arcweft-owned text-input snapshots rather than
/// native handles. Platform adapters consume it through the native player
/// bridge.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedTextInputTarget {
    pub snapshot: TextInputClientSnapshot,
    pub geometry: TextInputGeometrySnapshot,
}

#[derive(Clone, Debug, PartialEq)]
struct KeyboardFocusCandidate {
    target: InteractionTarget,
    bounds: HitRect,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct DirectionalFocusScore {
    outside_beam: bool,
    primary_distance: f32,
    secondary_distance: f32,
}

impl DirectionalFocusScore {
    fn new(
        direction: FocusNavigationDirection,
        origin: HitRect,
        candidate: HitRect,
    ) -> Option<Self> {
        let origin_center = rect_center(origin);
        let candidate_center = rect_center(candidate);
        let (primary_distance, secondary_distance, outside_beam) = match direction {
            FocusNavigationDirection::Up => (
                origin_center.1 - candidate_center.1,
                (origin_center.0 - candidate_center.0).abs(),
                !ranges_overlap(
                    origin.x,
                    origin.x + origin.width,
                    candidate.x,
                    candidate.x + candidate.width,
                ),
            ),
            FocusNavigationDirection::Down => (
                candidate_center.1 - origin_center.1,
                (origin_center.0 - candidate_center.0).abs(),
                !ranges_overlap(
                    origin.x,
                    origin.x + origin.width,
                    candidate.x,
                    candidate.x + candidate.width,
                ),
            ),
            FocusNavigationDirection::Left => (
                origin_center.0 - candidate_center.0,
                (origin_center.1 - candidate_center.1).abs(),
                !ranges_overlap(
                    origin.y,
                    origin.y + origin.height,
                    candidate.y,
                    candidate.y + candidate.height,
                ),
            ),
            FocusNavigationDirection::Right => (
                candidate_center.0 - origin_center.0,
                (origin_center.1 - candidate_center.1).abs(),
                !ranges_overlap(
                    origin.y,
                    origin.y + origin.height,
                    candidate.y,
                    candidate.y + candidate.height,
                ),
            ),
            FocusNavigationDirection::Next | FocusNavigationDirection::Previous => return None,
        };
        (primary_distance > f32::EPSILON).then_some(Self {
            outside_beam,
            primary_distance,
            secondary_distance,
        })
    }
}

impl Eq for DirectionalFocusScore {}

impl Ord for DirectionalFocusScore {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.outside_beam
            .cmp(&other.outside_beam)
            .then_with(|| self.primary_distance.total_cmp(&other.primary_distance))
            .then_with(|| self.secondary_distance.total_cmp(&other.secondary_distance))
    }
}

impl PartialOrd for DirectionalFocusScore {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

fn rect_center(rect: HitRect) -> (f32, f32) {
    (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

fn ranges_overlap(a_start: f32, a_end: f32, b_start: f32, b_end: f32) -> bool {
    a_start < b_end && b_start < a_end
}

/// Stateful frame planner that owns the shared project-font shaping cache.
#[derive(Debug, Default)]
pub struct SharedFramePlanContext {
    prepared_text_engine: Option<GlyphonTextEngine>,
    registered_font_bytes: usize,
}

/// Counters exposed by the stateful frame planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SharedFramePlanStats {
    pub registered_font_bytes: usize,
    pub prepared_text_shape_cache_hits: u64,
    pub prepared_text_shape_cache_misses: u64,
    pub prepared_text_shape_cache_entries: usize,
}

/// Invalid frame inputs rejected before GPU work.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FramePlanError {
    #[error("viewport must have non-zero logical and physical dimensions")]
    EmptyViewport,
    #[error("font bytes must not be empty")]
    EmptyFont,
    #[error(transparent)]
    PreparedTextFont(#[from] GlyphonTextEngineError),
    #[error(transparent)]
    PreparedText(#[from] PreparedTextError),
    #[error(transparent)]
    ResolveText(#[from] TextResolveError),
    #[error(transparent)]
    LayoutText(#[from] TextLayoutError<GlyphonTextEngineError>),
    #[error("prepared text requires registered project fonts")]
    MissingProjectFonts,
    #[error("text metric `{field}` is not finite and representable in milli-pixels")]
    InvalidTextMetric { field: &'static str },
    #[error("rich-text opacity {value} milli is outside [0, 1000]")]
    InvalidRichTextOpacity { value: i32 },
    #[error(transparent)]
    InvalidFxNumber(#[from] FiniteF32Error),
    #[error(transparent)]
    InvalidFxTransform(#[from] Transform2DError),
    #[error("authored RichText transform target {target:?} has no inline geometry owner")]
    UnsupportedRichTextTransformTarget { target: FxTarget },
    #[error("Fx logical ordinal {actual} exceeds the u32 runtime domain")]
    FxOrdinalOverflow { actual: usize },
    #[error("failed to construct stable presentation id `{value}`")]
    InvalidId { value: String },
    #[error("prepared-text owner references missing batch item {index}")]
    MissingPreparedTextOwnerItem { index: u32 },
    #[error("prepared-text item {index} already has a semantic owner")]
    DuplicatePreparedTextOwner { index: u32 },
    #[error("planned text item {expected} was inserted at batch index {actual}")]
    PreparedTextOrderMismatch { expected: usize, actual: u32 },
    #[error("semantic role {role:?} is not a text-input control")]
    InvalidTextInputRole { role: SemanticRole },
    #[error(transparent)]
    TextEditor(#[from] TextEditorError),
}

impl PreparedFrame {
    /// Keeps the canonical prepared-text painter graph while making every
    /// non-text producer transparent for one glyph-coverage render.
    pub(crate) fn retain_prepared_text_coverage_paint(&mut self, selected_text: PreparedTextId) {
        for rectangle in &mut self.rectangles {
            rectangle.rgba[3] = 0.0;
        }
        for image in &mut self.images {
            image.opacity_milli = 0;
        }
        self.control_backdrops.clear();
        self.control_shadows.clear();
        self.control_filters.clear();
        self.control_paints.clear();
        for prepared in &mut self.view_scenes {
            prepared
                .scene
                .retain_prepared_text_coverage_paint(selected_text);
        }
    }

    #[must_use]
    pub fn with_view_scenes(mut self, view_scenes: impl Into<Vec<PreparedViewScene>>) -> Self {
        self.view_scenes = view_scenes.into();
        self
    }

    pub fn push_view_scene(&mut self, scene: PreparedViewScene) {
        self.view_scenes.push(scene);
    }

    #[must_use]
    pub fn view_scenes(&self) -> &[PreparedViewScene] {
        &self.view_scenes
    }

    /// Registers one semantic owner after its prepared item is in the batch.
    pub fn push_prepared_text_owner(
        &mut self,
        owner: PreparedTextOwner,
    ) -> Result<(), FramePlanError> {
        if self.text.get(owner.text).is_none() {
            return Err(FramePlanError::MissingPreparedTextOwnerItem {
                index: owner.text.index(),
            });
        }
        if self
            .text_owners
            .iter()
            .any(|candidate| candidate.text == owner.text)
        {
            return Err(FramePlanError::DuplicatePreparedTextOwner {
                index: owner.text.index(),
            });
        }
        self.text_owners.push(owner);
        Ok(())
    }

    /// Returns semantic owners in prepared painter order.
    #[must_use]
    pub fn prepared_text_owners(&self) -> &[PreparedTextOwner] {
        &self.text_owners
    }

    pub fn push_dialogue_view(&mut self, dialogue: PreparedDialogueViewState) {
        self.dialogue_views.push(dialogue);
    }

    #[must_use]
    pub fn dialogue_views(&self) -> &[PreparedDialogueViewState] {
        &self.dialogue_views
    }

    /// Returns the renderer-backed focused text target for platform IME sync.
    ///
    /// This is the single native/web focus-target source. It is populated only
    /// by real `RenderTextInputControl` input lowered from runtime/player state.
    #[must_use]
    pub fn focused_text_input_target(&self) -> Option<PreparedTextInputTarget> {
        self.focused_text_input.clone()
    }

    #[must_use]
    pub fn selectable_text_at(
        &self,
        point: arcweft_presentation::input::ViewportPoint,
    ) -> Option<&PreparedTextItem> {
        self.text.items().iter().rev().find(|item| {
            let interaction = &item.interaction;
            if !interaction.selection_enabled || interaction.target.is_none() {
                return false;
            }
            let bounds = interaction.container_bounds.or(item.layout.bounds);
            let visible = bounds
                .into_iter()
                .chain(item.clip)
                .reduce(intersect_layout_rect);
            visible.is_some_and(|bounds| {
                bounds.width > 0.0
                    && bounds.height > 0.0
                    && point.x >= bounds.x
                    && point.x <= bounds.right()
                    && point.y >= bounds.y
                    && point.y <= bounds.bottom()
            })
        })
    }

    #[must_use]
    pub fn selectable_text_for_target(
        &self,
        target: &InteractionTarget,
    ) -> Option<&PreparedTextItem> {
        self.text.items().iter().rev().find(|item| {
            item.interaction.selection_enabled && item.interaction.target.as_ref() == Some(target)
        })
    }

    #[must_use]
    pub fn has_dialogue_views(&self) -> bool {
        !self.dialogue_views.is_empty()
    }

    #[must_use]
    pub fn latest_dialogue_view(&self) -> Option<&PreparedDialogueViewState> {
        self.dialogue_views
            .iter()
            .max_by_key(|dialogue| dialogue.instance)
    }

    #[must_use]
    pub fn has_revealing_dialogue(&self) -> bool {
        self.latest_dialogue_view()
            .is_some_and(|dialogue| !dialogue.reveal_complete)
    }

    #[must_use]
    pub fn dialogue_advance_available(&self) -> bool {
        self.dialogue_views
            .iter()
            .any(|dialogue| dialogue.advance_available)
    }

    fn map_to_viewport(&mut self, mapping: PreparedFrameViewportMapping) {
        self.viewport = mapping.viewport;
        self.map_surface_geometry(mapping);
        self.map_runtime_control_geometry(mapping);
        self.map_focused_text_input(mapping);
        self.map_scroll_regions(mapping);
    }

    fn map_surface_geometry(&mut self, mapping: PreparedFrameViewportMapping) {
        self.semantics = core::mem::take(&mut self.semantics).with_transformed_bounds(
            mapping.translate_x,
            mapping.translate_y,
            mapping.scale_x,
            mapping.scale_y,
        );
        self.hits = core::mem::take(&mut self.hits).with_transformed_bounds(
            mapping.translate_x,
            mapping.translate_y,
            mapping.scale_x,
            mapping.scale_y,
        );
        self.rectangles = self
            .rectangles
            .drain(..)
            .map(|rect| map_paint_rect(rect, mapping))
            .collect();
        self.images = self
            .images
            .drain(..)
            .map(|mut image| {
                image.bounds = mapping.rect(image.bounds);
                image.viewport_clip = image.viewport_clip.map(|clip| mapping.rect(clip));
                image
            })
            .collect();
    }

    fn map_runtime_control_geometry(&mut self, mapping: PreparedFrameViewportMapping) {
        self.control_backdrops = self
            .control_backdrops
            .drain(..)
            .map(|mut backdrop| {
                backdrop.bounds = mapping.rect(backdrop.bounds);
                backdrop
            })
            .collect();
        self.control_filters = self
            .control_filters
            .drain(..)
            .map(|mut filter| {
                filter.bounds = mapping.rect(filter.bounds);
                filter
            })
            .collect();
        self.control_shadows = self
            .control_shadows
            .drain(..)
            .map(|mut shadow| {
                shadow.plan = shadow.plan.transformed(
                    mapping.translate_x,
                    mapping.translate_y,
                    mapping.scale_x,
                    mapping.scale_y,
                );
                shadow
            })
            .collect();
        self.control_paints = self
            .control_paints
            .drain(..)
            .map(|mut paint| {
                paint.bounds = mapping.rect(paint.bounds);
                paint
            })
            .collect();
    }

    fn map_scroll_regions(&mut self, mapping: PreparedFrameViewportMapping) {
        for region in &mut self.scroll_regions {
            region.bounds = mapping.rect(region.bounds);
            region.content_width *= mapping.scale_x;
            region.content_height *= mapping.scale_y;
            region.min_offset_x *= mapping.scale_x;
            region.max_offset_x *= mapping.scale_x;
            region.min_offset_y *= mapping.scale_y;
            region.max_offset_y *= mapping.scale_y;
            region.offset_x *= mapping.scale_x;
            region.offset_y *= mapping.scale_y;
            region.overscroll_x *= mapping.scale_x;
            region.overscroll_y *= mapping.scale_y;
        }
        for indicator in &mut self.scroll_indicators {
            indicator.track_bounds = mapping.rect(indicator.track_bounds);
            indicator.thumb_bounds = mapping.rect(indicator.thumb_bounds);
        }
    }

    fn map_focused_text_input(&mut self, mapping: PreparedFrameViewportMapping) {
        self.focused_text_input =
            self.focused_text_input
                .take()
                .map(|target| PreparedTextInputTarget {
                    snapshot: target.snapshot.transformed(mapping.viewport_transform),
                    geometry: target
                        .geometry
                        .transformed_viewport(mapping.viewport_transform, mapping.screen_transform),
                });
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct PreparedFrameViewportMapping {
    viewport: RenderViewport,
    translate_x: f32,
    translate_y: f32,
    scale_x: f32,
    scale_y: f32,
    text_scale: f32,
    viewport_transform: TextGeometryTransform,
    screen_transform: TextGeometryTransform,
}

impl PreparedFrameViewportMapping {
    fn new(viewport: RenderViewport, content: ContentRect) -> Self {
        let translate_x = content.rect.origin.x;
        let translate_y = content.rect.origin.y;
        let scale_x = content.scale_x;
        let scale_y = content.scale_y;
        Self {
            viewport,
            translate_x,
            translate_y,
            scale_x,
            scale_y,
            text_scale: ((scale_x.abs() + scale_y.abs()) * 0.5).max(f32::EPSILON),
            viewport_transform: TextGeometryTransform::scale(scale_x, scale_y)
                .then(TextGeometryTransform::translation(translate_x, translate_y)),
            screen_transform: TextGeometryTransform::scale(
                viewport.physical_scale_factor_f32(),
                viewport.physical_scale_factor_f32(),
            ),
        }
    }

    fn rect(self, rect: HitRect) -> HitRect {
        rect.transformed(
            self.translate_x,
            self.translate_y,
            self.scale_x,
            self.scale_y,
        )
    }

    fn layout_rect(self, rect: LayoutRect) -> LayoutRect {
        LayoutRect::new(
            self.translate_x + rect.x * self.scale_x,
            self.translate_y + rect.y * self.scale_y,
            rect.width * self.scale_x.abs(),
            rect.height * self.scale_y.abs(),
        )
    }
}

fn map_paint_rect(mut rect: PaintRect, mapping: PreparedFrameViewportMapping) -> PaintRect {
    rect.bounds = mapping.rect(rect.bounds);
    rect.radii = rect.radii.scaled(mapping.scale_x, mapping.scale_y);
    rect.stroke_width_px *= mapping.text_scale;
    rect.clip = rect.clip.map(|clip| PaintRectClip {
        bounds: mapping.rect(clip.bounds),
        radii: clip.radii.scaled(mapping.scale_x, mapping.scale_y),
    });
    rect
}

fn mapped_text_style(
    style: ResolvedTextStyle,
    scale: f32,
) -> Result<ResolvedTextStyle, FramePlanError> {
    let font_size = style.font_size_milli().to_f32().unwrap_or(f32::MAX) / 1_000.0 * scale;
    let line_height = style.line_height_milli().to_f32().unwrap_or(f32::MAX) / 1_000.0 * scale;
    let letter_spacing = scaled_spacing_milli(style.letter_spacing_milli(), scale);
    let word_spacing = scaled_spacing_milli(style.word_spacing_milli(), scale);
    style
        .with_font_metrics(
            prepared_text::pixels_to_milli("font_size", font_size)?,
            prepared_text::pixels_to_milli("line_height", line_height)?,
        )
        .map(|style| style.with_spacing(letter_spacing, word_spacing))
        .map_err(FramePlanError::from)
}

fn scaled_spacing_milli(value: i32, scale: f32) -> i32 {
    let scaled = f64::from(value) * f64::from(scale);
    scaled.round().to_i32().unwrap_or_else(|| {
        if scaled.is_sign_negative() {
            i32::MIN
        } else {
            i32::MAX
        }
    })
}

fn intersect_layout_rect(left: LayoutRect, right: LayoutRect) -> LayoutRect {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = left.right().min(right.right());
    let bottom_edge = left.bottom().min(right.bottom());
    LayoutRect::new(x, y, (right_edge - x).max(0.0), (bottom_edge - y).max(0.0))
}

pub(super) fn intersect_hit_rect(left: HitRect, right: HitRect) -> Option<HitRect> {
    let x = left.x.max(right.x);
    let y = left.y.max(right.y);
    let right_edge = (left.x + left.width).min(right.x + right.width);
    let bottom_edge = (left.y + left.height).min(right.y + right.height);
    let width = right_edge - x;
    let height = bottom_edge - y;
    (width > 0.0 && height > 0.0).then(|| HitRect::new(x, y, width, height))
}

impl PreparedViewScene {
    pub fn new(scene: ViewScene) -> Self {
        Self {
            scene,
            resources: PreparedViewSceneResources::default(),
        }
    }

    #[must_use]
    pub fn with_resources(mut self, resources: PreparedViewSceneResources) -> Self {
        self.resources = resources;
        self
    }
}

impl PreparedViewSceneResources {
    pub fn push_image(&mut self, image: PreparedViewImageResource) {
        self.images.push(image);
    }

    pub fn push_mask(&mut self, mask: PreparedViewMaskResource) {
        self.masks.push(mask);
    }

    pub fn images(&self) -> &[PreparedViewImageResource] {
        &self.images
    }

    pub fn masks(&self) -> &[PreparedViewMaskResource] {
        &self.masks
    }
}

impl Default for RenderPreferences {
    fn default() -> Self {
        Self {
            text_scale_milli: 1_000,
            high_contrast: false,
            reduce_motion: false,
        }
    }
}

impl SharedFramePlanContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), FramePlanError> {
        if bytes.is_empty() {
            return Err(FramePlanError::EmptyFont);
        }
        let byte_len = bytes.len();
        if let Some(engine) = &mut self.prepared_text_engine {
            engine.register_project_font(bytes)?;
        } else {
            self.prepared_text_engine =
                Some(GlyphonTextEngine::from_project_fonts("und", vec![bytes])?);
        }
        self.registered_font_bytes = self.registered_font_bytes.saturating_add(byte_len);
        Ok(())
    }

    /// Creates isolated planner state for a frame publication candidate.
    ///
    /// The fork retains exact registered fonts and text-shape cache contents,
    /// while all subsequent cache mutations remain private until publication.
    pub fn fork_for_candidate(&self) -> Result<Self, FramePlanError> {
        Ok(Self {
            prepared_text_engine: self
                .prepared_text_engine
                .as_ref()
                .map(GlyphonTextEngine::fork_for_candidate)
                .transpose()?,
            registered_font_bytes: self.registered_font_bytes,
        })
    }

    #[must_use]
    pub fn stats(&self) -> SharedFramePlanStats {
        let prepared = self
            .prepared_text_engine
            .as_ref()
            .map(GlyphonTextEngine::cache_stats)
            .unwrap_or_default();
        SharedFramePlanStats {
            registered_font_bytes: self.registered_font_bytes,
            prepared_text_shape_cache_hits: prepared.hits,
            prepared_text_shape_cache_misses: prepared.misses,
            prepared_text_shape_cache_entries: prepared.entries,
        }
    }

    /// # Panics
    ///
    /// Panics if internal layer parent ids are inconsistent. That indicates a
    /// planner bug rather than invalid caller input.
    pub fn prepare(&mut self, scene: &RenderScene) -> Result<PreparedFrame, FramePlanError> {
        let planned = Self::plan(scene)?;
        self.finish(planned)
    }

    /// Plans in design coordinates, maps the exact geometry, then shapes text
    /// at its final output size and raster scale.
    pub fn prepare_mapped(
        &mut self,
        scene: &RenderScene,
        viewport: RenderViewport,
        content: ContentRect,
    ) -> Result<PreparedFrame, FramePlanError> {
        let mut planned = Self::plan(scene)?;
        let mapping = PreparedFrameViewportMapping::new(viewport, content);
        planned.frame.map_to_viewport(mapping);
        planned.text = planned
            .text
            .into_iter()
            .map(|text| text.mapped(mapping))
            .collect::<Result<Vec<_>, _>>()?;
        self.finish(planned)
    }

    fn plan(scene: &RenderScene) -> Result<PlannedFrame, FramePlanError> {
        validate_viewport(scene.viewport)?;
        let ids = FrameIds::new()?;
        let layers = build_frame_layers(&ids);

        let palette = Palette::from_preferences(scene.preferences);
        let mut rectangles = vec![PaintRect::new(
            HitRect::new(
                0.0,
                0.0,
                scene.viewport.logical_width,
                scene.viewport.logical_height,
            ),
            palette.background,
        )];
        let mut text = Vec::new();
        let mut semantics = SemanticTree::default();
        let action = RenderActionKind::ChoiceSelect.public_id()?;
        let choices = build_choices(
            scene,
            &ids.choice,
            &mut semantics,
            &mut rectangles,
            &mut text,
            &palette,
            &action,
        )?;
        let mut control_backdrops = Vec::new();
        let mut control_shadows = Vec::new();
        let mut control_filters = Vec::new();
        let runtime_controls = build_runtime_controls(
            scene,
            &ids,
            &mut semantics,
            &mut rectangles,
            &mut text,
            &palette,
            &mut control_backdrops,
            &mut control_shadows,
            &mut control_filters,
        )?;
        let scroll_indicators = scroll::build_scroll_indicators(scene, &mut rectangles, &palette);
        let hits = semantics.to_hit_tree();

        let frame = PreparedFrame {
            viewport: scene.viewport,
            visual_time_millis: scene.visual_time_millis,
            preferences: scene.preferences,
            layers,
            semantics,
            hits,
            rectangles,
            images: build_retained_images(scene),
            text: PreparedTextBatch::default(),
            text_owners: Vec::new(),
            fx_diagnostics: Vec::new(),
            choices,
            action_buttons: runtime_controls.action_buttons,
            control_backdrops,
            control_shadows,
            control_filters,
            control_paints: runtime_controls.control_paints,
            scroll_regions: scene.scroll_regions.clone(),
            scroll_indicators,
            focus_graph: PreparedFocusGraph::new(
                scene.focus_groups.clone(),
                scene.focus_navigation.clone(),
            ),
            view_scenes: Vec::new(),
            dialogue_views: Vec::new(),
            interaction: scene.interaction.clone(),
            focused_text_input: None,
        };
        Ok(PlannedFrame { frame, text })
    }

    fn finish(&mut self, planned: PlannedFrame) -> Result<PreparedFrame, FramePlanError> {
        let PlannedFrame {
            mut frame,
            text: planned_text,
        } = planned;
        if planned_text.is_empty() {
            return Ok(frame);
        }
        let engine = self
            .prepared_text_engine
            .as_mut()
            .ok_or(FramePlanError::MissingProjectFonts)?;
        for (expected, planned) in planned_text.into_iter().enumerate() {
            let (item, owner, focused_text_input) = match planned {
                PlannedFrameText::Plain(text) => {
                    let text = *text;
                    (
                        prepared_text::prepare_plain_text(
                            engine,
                            &text.text,
                            text.style,
                            text.source,
                            &text.request,
                            frame.viewport,
                        )?,
                        text.owner,
                        None,
                    )
                }
                PlannedFrameText::TextInput(text) => {
                    let prepared = (*text).prepare(engine, frame.viewport)?;
                    (prepared.item, prepared.owner, prepared.focused_target)
                }
            };
            let id = frame.text.push(item)?;
            if usize::try_from(id.index()).ok() != Some(expected) {
                return Err(FramePlanError::PreparedTextOrderMismatch {
                    expected,
                    actual: id.index(),
                });
            }
            frame.push_prepared_text_owner(PreparedTextOwner::new(
                id,
                owner.semantic_id,
                PreparedTextOwnerKind::Control,
                0,
                owner.object_bounds,
            ))?;
            if let Some(target) = focused_text_input {
                frame.focused_text_input = Some(target);
            }
        }
        Ok(frame)
    }

    /// Appends one `RichText` display stage to the canonical prepared batch.
    pub fn push_prepared_rich_text_stage(
        &mut self,
        frame: &mut PreparedFrame,
        stage: arcweft_render_text::LineDisplayStage<'_>,
        request: &PreparedRichTextStageRequest,
        fx_resolver: &dyn arcweft_presentation::fx::FxApplicationResolver,
    ) -> Result<PreparedRichTextStageResult, FramePlanError> {
        let engine = self
            .prepared_text_engine
            .as_mut()
            .ok_or(FramePlanError::MissingProjectFonts)?;
        let (item, reveal_complete, diagnostics, source_origin) = dialogue_prepared::prepare_stage(
            engine,
            stage,
            request,
            frame.viewport,
            frame.preferences.reduce_motion,
            fx_resolver,
        )?;
        let text = frame.text.push(item)?;
        frame.fx_diagnostics.extend(diagnostics);
        Ok(PreparedRichTextStageResult {
            text,
            reveal_complete,
            source_origin,
        })
    }

    /// Prepares one already-resolved document without a source-specific adapter type.
    pub fn prepare_text_document(
        &mut self,
        document: &ResolvedTextDocument<'_>,
        request: &PreparedTextDocumentRequest,
        viewport: RenderViewport,
    ) -> Result<PreparedTextItem, FramePlanError> {
        let engine = self
            .prepared_text_engine
            .as_mut()
            .ok_or(FramePlanError::MissingProjectFonts)?;
        prepared_text::prepare_text_document(engine, document, request, viewport)
    }

    /// Appends one canonical resolved document in exact frame painter order.
    pub fn push_prepared_text_document(
        &mut self,
        frame: &mut PreparedFrame,
        document: &ResolvedTextDocument<'_>,
        request: &PreparedTextDocumentRequest,
    ) -> Result<PreparedTextId, FramePlanError> {
        let item = self.prepare_text_document(document, request, frame.viewport)?;
        frame.text.push(item).map_err(FramePlanError::from)
    }
}

fn build_retained_images(scene: &RenderScene) -> Vec<RenderImage> {
    scene
        .images
        .iter()
        .filter_map(|image| {
            let mut image = image.clone();
            let (bounds, viewport_clip) = scroll_adjusted_bounds(
                scene,
                image.containing_scroll_region.as_deref(),
                image.bounds,
            )?;
            image.bounds = bounds;
            image.viewport_clip = viewport_clip;
            Some(image)
        })
        .collect()
}

struct RuntimeControlsBuildOutput {
    action_buttons: Vec<PreparedActionButton>,
    control_paints: Vec<PreparedControlPaint>,
}

enum RuntimeControlPlanItem {
    TextInput { index: usize, depth_milli: i32 },
    ActionButton { index: usize, depth_milli: i32 },
}

impl RuntimeControlPlanItem {
    const fn depth_milli(&self) -> i32 {
        match self {
            Self::TextInput { depth_milli, .. } | Self::ActionButton { depth_milli, .. } => {
                *depth_milli
            }
        }
    }

    const fn source_order(&self) -> usize {
        match self {
            Self::TextInput { index, .. } | Self::ActionButton { index, .. } => *index,
        }
    }

    const fn kind_order(&self) -> u8 {
        match self {
            Self::TextInput { .. } => 0,
            Self::ActionButton { .. } => 1,
        }
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "The frame planner explicitly owns shared render buffers at this boundary."
)]
fn build_runtime_controls(
    scene: &RenderScene,
    ids: &FrameIds,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<PlannedFrameText>,
    palette: &Palette,
    control_backdrops: &mut Vec<PreparedControlBackdrop>,
    control_shadows: &mut Vec<PreparedControlShadow>,
    control_filters: &mut Vec<PreparedControlFilter>,
) -> Result<RuntimeControlsBuildOutput, FramePlanError> {
    let mut items = Vec::with_capacity(scene.text_inputs.len() + scene.action_buttons.len());
    items.extend(
        scene
            .text_inputs
            .iter()
            .enumerate()
            .map(|(index, control)| RuntimeControlPlanItem::TextInput {
                index,
                depth_milli: text_controls::text_input_depth_milli(control),
            }),
    );
    items.extend(
        scene
            .action_buttons
            .iter()
            .enumerate()
            .map(|(index, button)| RuntimeControlPlanItem::ActionButton {
                index,
                depth_milli: action_buttons::action_button_depth_milli(button),
            }),
    );
    items.sort_by_key(|item| (item.depth_milli(), item.kind_order(), item.source_order()));

    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 20.0 * scale;
    let line_height = 28.0 * scale;
    let mut prepared_buttons = Vec::new();
    let mut control_paints = Vec::new();

    for item in items {
        match item {
            RuntimeControlPlanItem::TextInput { index, .. } => {
                let mut control = scene.text_inputs[index].clone();
                let Some((bounds, viewport_clip)) = scroll_adjusted_bounds(
                    scene,
                    control.containing_scroll_region.as_deref(),
                    control.bounds,
                ) else {
                    continue;
                };
                control.bounds = bounds;
                control.viewport_clip = viewport_clip;
                let paint = text_controls::build_text_input(
                    scene,
                    &ids.text_input,
                    &control,
                    semantics,
                    rectangles,
                    text,
                    palette,
                    control_backdrops,
                    control_shadows,
                    control_filters,
                )?;
                control_paints.push(paint);
            }
            RuntimeControlPlanItem::ActionButton { index, .. } => {
                let mut button = scene.action_buttons[index].clone();
                let Some((bounds, viewport_clip)) = scroll_adjusted_bounds(
                    scene,
                    button.containing_scroll_region.as_deref(),
                    button.bounds,
                ) else {
                    continue;
                };
                button.bounds = bounds;
                button.viewport_clip = viewport_clip;
                let (button, paint) = action_buttons::build_action_button(
                    &ids.action_button,
                    &button,
                    action_buttons::ActionButtonBuildOutput {
                        semantics,
                        rectangles,
                        text,
                        control_backdrops,
                        control_shadows,
                        control_filters,
                    },
                    palette,
                    font_size,
                    line_height,
                )?;
                prepared_buttons.push(button);
                control_paints.push(paint);
            }
        }
    }

    Ok(RuntimeControlsBuildOutput {
        action_buttons: prepared_buttons,
        control_paints,
    })
}

fn scroll_adjusted_bounds(
    scene: &RenderScene,
    containing_scroll_region: Option<&str>,
    bounds: HitRect,
) -> Option<(HitRect, Option<HitRect>)> {
    let Some(scroll_region) = containing_scroll_region else {
        return Some((bounds, None));
    };
    let region = scene
        .scroll_regions
        .iter()
        .find(|region| region.id == scroll_region)?;
    let shifted = HitRect::new(
        bounds.x - region.visual_offset_x(),
        bounds.y - region.visual_offset_y(),
        bounds.width,
        bounds.height,
    );
    intersect_hit_rect(shifted, region.bounds).map(|_| (shifted, Some(region.bounds)))
}

fn build_frame_layers(ids: &FrameIds) -> LayerTree {
    let mut layers = LayerTree::new(
        LayerNode::new(
            ids.root.clone(),
            LayerKind::Root,
            order(RenderPhase::Background, 0),
        )
        .with_input_policy(LayerInputPolicy::Ignore),
    );
    layers
        .insert(
            LayerNode::new(
                ids.choice.clone(),
                LayerKind::GameView,
                order(RenderPhase::GameView, 0),
            )
            .with_parent(ids.root.clone())
            .with_content(LayerContent::NativeView(ids.choice_content.clone()))
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .expect("choice layer parent is present");
    layers
        .insert(
            LayerNode::new(
                ids.text_input.clone(),
                LayerKind::GameView,
                order(RenderPhase::GameView, 1),
            )
            .with_parent(ids.root.clone())
            .with_content(LayerContent::NativeView(ids.text_input_content.clone()))
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .expect("text-input layer parent is present");
    layers
        .insert(
            LayerNode::new(
                ids.action_button.clone(),
                LayerKind::GameView,
                order(RenderPhase::GameView, 2),
            )
            .with_parent(ids.root.clone())
            .with_content(LayerContent::NativeView(ids.action_button_content.clone()))
            .with_input_policy(LayerInputPolicy::HitTest),
        )
        .expect("action-button layer parent is present");
    layers
}

impl PreparedFrame {
    pub fn choice_for_target(&self, target: &InteractionTarget) -> Option<&RenderChoice> {
        self.choices.iter().find(|choice| &choice.target == target)
    }

    pub fn action_button_for_target(
        &self,
        target: &InteractionTarget,
    ) -> Option<&PreparedActionButton> {
        self.action_buttons
            .iter()
            .find(|button| &button.target == target)
    }

    pub fn target_bounds(&self, target: &InteractionTarget) -> Option<HitRect> {
        self.hits
            .find_target(target)
            .map(arcweft_presentation::hit::HitRecord::bounds)
            .or_else(|| self.semantics.find(target).map(SemanticNode::bounds))
    }

    pub fn scroll_region_for_target(
        &self,
        target: &InteractionTarget,
    ) -> Option<&RenderScrollRegion> {
        let bounds = self.target_bounds(target)?;
        let center = ViewportPoint::new(
            bounds.x + bounds.width * 0.5,
            bounds.y + bounds.height * 0.5,
        );
        self.scroll_regions
            .iter()
            .rev()
            .find(|region| region.contains(center))
    }

    pub fn first_choice_target(&self) -> Option<InteractionTarget> {
        self.choices.first().map(|choice| choice.target.clone())
    }

    pub fn text_input_targets(&self) -> impl Iterator<Item = InteractionTarget> + '_ {
        self.semantics
            .as_slice()
            .iter()
            .filter(|node| node.role().is_text_input_control() && node.visible() && node.enabled())
            .map(|node| node.target().clone())
    }

    pub fn action_button_targets(&self) -> impl Iterator<Item = InteractionTarget> + '_ {
        self.action_buttons
            .iter()
            .filter(|button| button.enabled)
            .map(|button| button.target.clone())
    }

    pub fn keyboard_focus_targets(&self) -> Vec<InteractionTarget> {
        self.text_input_targets()
            .chain(self.action_button_targets())
            .chain(self.choices.iter().map(|choice| choice.target.clone()))
            .collect()
    }

    fn keyboard_focus_candidates(&self) -> Vec<KeyboardFocusCandidate> {
        self.keyboard_focus_targets()
            .into_iter()
            .filter_map(|target| {
                self.hits
                    .find_target(&target)
                    .filter(|record| record.visible() && record.enabled())
                    .map(|record| KeyboardFocusCandidate {
                        target,
                        bounds: record.bounds(),
                    })
            })
            .collect()
    }

    pub fn first_keyboard_focus_target(&self) -> Option<InteractionTarget> {
        self.keyboard_focus_targets().into_iter().next()
    }

    pub fn last_keyboard_focus_target(&self) -> Option<InteractionTarget> {
        self.keyboard_focus_targets().into_iter().last()
    }

    pub fn adjacent_keyboard_focus_target(
        &self,
        current: Option<&InteractionTarget>,
        delta: isize,
    ) -> Option<InteractionTarget> {
        self.adjacent_keyboard_focus_target_with_wrap(current, delta, true)
    }

    pub fn adjacent_keyboard_focus_target_with_wrap(
        &self,
        current: Option<&InteractionTarget>,
        delta: isize,
        wrap: bool,
    ) -> Option<InteractionTarget> {
        let targets = self.keyboard_focus_targets();
        if targets.is_empty() {
            return None;
        }
        let current = current
            .and_then(|target| targets.iter().position(|candidate| candidate == target))
            .unwrap_or(0);
        let current = isize::try_from(current).ok()?;
        let len = isize::try_from(targets.len()).ok()?;
        let raw_next = current + delta;
        let next = if wrap {
            raw_next.rem_euclid(len)
        } else if (0..len).contains(&raw_next) {
            raw_next
        } else {
            return None;
        };
        targets.get(usize::try_from(next).ok()?).cloned()
    }

    pub fn directional_keyboard_focus_target(
        &self,
        current: Option<&InteractionTarget>,
        direction: FocusNavigationDirection,
    ) -> Option<InteractionTarget> {
        self.focus_target(current, direction)
    }

    pub fn geometric_keyboard_focus_target(
        &self,
        current: &InteractionTarget,
        direction: FocusNavigationDirection,
    ) -> Option<InteractionTarget> {
        let candidates = self.keyboard_focus_candidates();
        if candidates.is_empty() {
            return None;
        }
        let Some(origin) = candidates
            .iter()
            .find(|candidate| &candidate.target == current)
        else {
            return candidates.first().map(|candidate| candidate.target.clone());
        };
        candidates
            .iter()
            .enumerate()
            .filter(|(_, candidate)| candidate.target != origin.target)
            .filter_map(|(index, candidate)| {
                DirectionalFocusScore::new(direction, origin.bounds, candidate.bounds)
                    .map(|score| (score, index, candidate.target.clone()))
            })
            .min_by(
                |(left_score, left_index, _), (right_score, right_index, _)| {
                    left_score
                        .cmp(right_score)
                        .then_with(|| left_index.cmp(right_index))
                },
            )
            .map(|(_, _, target)| target)
    }

    pub fn is_enabled_keyboard_focus_target(&self, target: &InteractionTarget) -> bool {
        self.hits
            .find_target(target)
            .is_some_and(|record| record.visible() && record.enabled())
    }

    pub const fn interaction_focused_target(&self) -> Option<&InteractionTarget> {
        self.interaction.focused.as_ref()
    }

    pub fn last_choice_target(&self) -> Option<InteractionTarget> {
        self.choices.last().map(|choice| choice.target.clone())
    }

    pub fn adjacent_choice_target(
        &self,
        current: Option<&InteractionTarget>,
        delta: isize,
    ) -> Option<InteractionTarget> {
        if self.choices.is_empty() {
            return None;
        }
        let current = current
            .and_then(|target| {
                self.choices
                    .iter()
                    .position(|choice| &choice.target == target)
            })
            .unwrap_or(0);
        let len = isize::try_from(self.choices.len()).ok()?;
        let next = (isize::try_from(current).ok()? + delta).rem_euclid(len);
        self.choices
            .get(usize::try_from(next).ok()?)
            .map(|choice| choice.target.clone())
    }
}

fn build_choices(
    scene: &RenderScene,
    layer: &LayerId,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<PlannedFrameText>,
    palette: &Palette,
    action: &PublicId,
) -> Result<Vec<RenderChoice>, FramePlanError> {
    if scene.choices.is_empty() {
        return Ok(Vec::new());
    }
    let width = (scene.viewport.logical_width * 0.52).clamp(360.0, 760.0);
    let item_height = 60.0;
    let gap = 12.0;
    let total = saturating_usize_as_f32(scene.choices.len()) * (item_height + gap) - gap;
    let left = (scene.viewport.logical_width - width) * 0.5;
    let avoidance_top = scene
        .content_avoidance_regions
        .iter()
        .filter(|region| region.x < left + width && region.x + region.width > left)
        .map(|region| region.y)
        .min_by(f32::total_cmp);
    let top = avoidance_top.map_or_else(
        || ((scene.viewport.logical_height - total) * 0.42).max(36.0),
        |avoidance_top| (avoidance_top - total - 22.0).max(36.0),
    );
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 22.0 * scale;
    let line_height = 30.0 * scale;
    let text_style = prepared_text::resolved_plain_style(
        vec![TextFontFamily::SansSerif],
        font_size,
        line_height,
        TextWeight::Bold,
        TextSlant::Upright,
        palette.choice_text,
    )?;

    scene
        .choices
        .iter()
        .enumerate()
        .map(|(index, choice)| {
            let target = InteractionTarget::new(ChoiceTargetId(index).public_id()?);
            let bounds = HitRect::new(
                left,
                top + saturating_usize_as_f32(index) * (item_height + gap),
                width,
                item_height,
            );
            let is_focused = scene.interaction.focused.as_ref() == Some(&target);
            let is_hovered = scene.interaction.hovered.as_ref() == Some(&target);
            let is_pressed = scene.interaction.pressed.as_ref() == Some(&target);
            rectangles.push(PaintRect::new(
                bounds,
                if is_pressed {
                    palette.choice_pressed
                } else if is_focused || is_hovered {
                    palette.choice_active
                } else {
                    palette.choice_idle
                },
            ));
            if is_focused {
                push_focus_ring(rectangles, bounds, palette.focus_ring);
            }
            let text_bounds = HitRect::new(
                bounds.x + 24.0,
                bounds.y + (bounds.height - line_height) * 0.5,
                bounds.width - 48.0,
                line_height,
            );
            text.push(PlannedFrameText::Plain(Box::new(PlannedPlainText {
                text: choice.label.clone(),
                style: text_style.clone(),
                source: ResolvedTextRunSource::Generated,
                request: PreparedTextDocumentRequest {
                    origin: LayoutPoint::new(text_bounds.x, text_bounds.y),
                    size: LayoutSize::new(text_bounds.width, text_bounds.height),
                    container_bounds: LayoutRect::new(
                        text_bounds.x,
                        text_bounds.y,
                        text_bounds.width,
                        text_bounds.height,
                    ),
                    clip: None,
                    target: None,
                    selection_enabled: false,
                    selection: None,
                    selection_rgba: palette.choice_active,
                },
                owner: PlannedTextOwner {
                    semantic_id: target.id().clone(),
                    object_bounds: bounds,
                },
            })));
            semantics.push(
                SemanticNode::new(layer.clone(), target.clone(), SemanticRole::Button, bounds)
                    .with_label(choice.label.clone())
                    .with_action(action.clone()),
            );
            Ok(RenderChoice {
                option_id: choice.id.clone(),
                label: choice.label.clone(),
                target,
            })
        })
        .collect()
}

fn push_focus_ring(rectangles: &mut Vec<PaintRect>, bounds: HitRect, color: [f32; 4]) {
    let thickness = 3.0;
    rectangles.extend([
        PaintRect::new(
            HitRect::new(bounds.x, bounds.y, bounds.width, thickness),
            color,
        ),
        PaintRect::new(
            HitRect::new(
                bounds.x,
                bounds.y + bounds.height - thickness,
                bounds.width,
                thickness,
            ),
            color,
        ),
        PaintRect::new(
            HitRect::new(bounds.x, bounds.y, thickness, bounds.height),
            color,
        ),
        PaintRect::new(
            HitRect::new(
                bounds.x + bounds.width - thickness,
                bounds.y,
                thickness,
                bounds.height,
            ),
            color,
        ),
    ]);
}

fn validate_viewport(viewport: RenderViewport) -> Result<(), FramePlanError> {
    (viewport.logical_width > 0.0
        && viewport.logical_height > 0.0
        && viewport.physical_width > 0
        && viewport.physical_height > 0)
        .then_some(())
        .ok_or(FramePlanError::EmptyViewport)
}

const fn order(phase: RenderPhase, z: i32) -> LayerOrder {
    LayerOrder {
        phase,
        z,
        stable_index: 0,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenderActionKind {
    ChoiceSelect,
}

impl RenderActionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::ChoiceSelect => "action.choice.select",
        }
    }

    fn public_id(self) -> Result<PublicId, FramePlanError> {
        PublicId::try_new(self.as_str()).map_err(|_| FramePlanError::InvalidId {
            value: self.as_str().to_owned(),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FrameStaticId {
    RootLayer,
    ChoiceLayer,
    TextInputLayer,
    ActionButtonLayer,
    ChoiceContent,
    TextInputContent,
    ActionButtonContent,
}

impl FrameStaticId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RootLayer => "layer.player.root",
            Self::ChoiceLayer => "layer.player.choice",
            Self::TextInputLayer => "layer.player.text_input",
            Self::ActionButtonLayer => "layer.player.action_button",
            Self::ChoiceContent => "view.player.choice",
            Self::TextInputContent => "view.player.text_input",
            Self::ActionButtonContent => "view.player.action_button",
        }
    }

    fn public_id(self) -> Result<PublicId, FramePlanError> {
        PublicId::try_new(self.as_str()).map_err(|_| FramePlanError::InvalidId {
            value: self.as_str().to_owned(),
        })
    }

    fn layer_id(self) -> Result<LayerId, FramePlanError> {
        self.public_id().map(LayerId::new)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ChoiceTargetId(usize);

impl ChoiceTargetId {
    fn public_id(self) -> Result<PublicId, FramePlanError> {
        let value = format!("target.choice.{}", self.0);
        PublicId::try_new(&value).map_err(|_| FramePlanError::InvalidId { value })
    }
}

struct FrameIds {
    root: LayerId,
    choice: LayerId,
    text_input: LayerId,
    action_button: LayerId,
    choice_content: PublicId,
    text_input_content: PublicId,
    action_button_content: PublicId,
}

impl FrameIds {
    fn new() -> Result<Self, FramePlanError> {
        Ok(Self {
            root: FrameStaticId::RootLayer.layer_id()?,
            choice: FrameStaticId::ChoiceLayer.layer_id()?,
            text_input: FrameStaticId::TextInputLayer.layer_id()?,
            action_button: FrameStaticId::ActionButtonLayer.layer_id()?,
            choice_content: FrameStaticId::ChoiceContent.public_id()?,
            text_input_content: FrameStaticId::TextInputContent.public_id()?,
            action_button_content: FrameStaticId::ActionButtonContent.public_id()?,
        })
    }
}

struct Palette {
    background: [f32; 4],
    choice_idle: [f32; 4],
    choice_active: [f32; 4],
    choice_pressed: [f32; 4],
    focus_ring: [f32; 4],
    scroll_track: [f32; 4],
    scroll_thumb: [f32; 4],
    choice_text: [u8; 4],
}

impl Palette {
    fn from_preferences(preferences: RenderPreferences) -> Self {
        if preferences.high_contrast {
            Self {
                background: [0.0, 0.0, 0.0, 1.0],
                choice_idle: [0.08, 0.08, 0.08, 1.0],
                choice_active: [0.2, 0.2, 0.2, 1.0],
                choice_pressed: [0.32, 0.32, 0.32, 1.0],
                focus_ring: [1.0, 1.0, 0.0, 1.0],
                scroll_track: [0.35, 0.35, 0.35, 0.72],
                scroll_thumb: [1.0, 1.0, 0.0, 1.0],
                choice_text: [255, 255, 255, 255],
            }
        } else {
            Self {
                background: [0.019, 0.027, 0.024, 1.0],
                choice_idle: [0.125, 0.124, 0.099, 0.98],
                choice_active: [0.119, 0.235, 0.153, 1.0],
                choice_pressed: [0.207, 0.3, 0.164, 1.0],
                focus_ring: [0.886, 0.914, 0.384, 1.0],
                scroll_track: [0.04, 0.05, 0.045, 0.55],
                scroll_thumb: [0.72, 0.78, 0.55, 0.92],
                choice_text: [255, 252, 238, 255],
            }
        }
    }
}
