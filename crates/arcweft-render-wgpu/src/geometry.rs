use crate::convert::saturating_usize_as_f32;
use crate::view_mask::ViewMaskChannel;
use crate::view_scene::{ViewMaskImage, ViewScene};
use arcweft_id::PublicId;
use arcweft_layout::ContentRect;
use arcweft_presentation::hit::{HitRect, HitTree};
use arcweft_presentation::input::{InteractionTarget, ViewportPoint};
use arcweft_presentation::layer::{
    LayerContent, LayerId, LayerInputPolicy, LayerKind, LayerNode, LayerOrder, LayerTree,
    RenderPhase,
};
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_presentation::text_input::{
    TextByteOffset, TextCharacterBounds, TextGeometryTransform, TextInputClientSnapshot,
    TextInputGeometrySnapshot, TextRange,
};
use arcweft_render_text::{
    LineDisplayFrame, RichTextColor, RichTextEffectDescriptor, RichTextEffectPhase,
    RichTextFontFamily, RichTextParam, RichTextRange, RichTextStyle, RichTextTextRun,
    RichTextTextSource, presentation_from_styles,
};
use num_traits::ToPrimitive;
use thiserror::Error;

mod action_buttons;
mod control_style;
mod focus_navigation;
mod images;
mod text_controls;
pub use action_buttons::{PreparedActionButton, RenderActionButton, RenderActionButtonAction};
pub use control_style::{
    PreparedControlBackdrop, PreparedControlFilter, PreparedControlPaint, PreparedControlShadow,
    RenderControlBorderStyle, RenderControlCornerFrameStyle, RenderControlFilter,
    RenderControlFilterList, RenderControlFocusRingStyle, RenderControlShadow,
    RenderControlShadowKind, RenderControlStyle, RenderControlVisualState,
    RenderControlVisualStyle, RuntimeControlBackdropSamplePolicy,
};
pub use focus_navigation::{
    FocusNavigationDebug, FocusNavigationDebugCandidate, PreparedFocusGraph, PreparedFocusGroup,
    PreparedFocusNavigationTarget, RenderFocusGroup, RenderFocusGroupPolicy,
    RenderFocusInitialPolicy, RenderFocusNavigation, RenderFocusNavigationEdge,
    RenderFocusSkipPolicy, RenderFocusTargetResolution, RenderFocusWrapPolicy,
};
pub use images::{RenderImage, RenderImageFrame, RenderImageQuad, RenderImageTransformMatrix};
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
    /// Returns the finite physical scale factor used by renderer subsystems
    /// whose APIs accept `f32` coordinates.
    #[must_use]
    pub fn physical_scale_factor_f32(self) -> f32 {
        let Some(scale_factor) = self.scale_factor.to_f32() else {
            return 1.0;
        };
        if scale_factor.is_finite() {
            scale_factor.max(f32::EPSILON)
        } else {
            1.0
        }
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

/// One scrollable retained View region in logical viewport coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderScrollRegion {
    pub id: String,
    pub bounds: HitRect,
    pub content_width: f32,
    pub content_height: f32,
    pub offset_x: f32,
    pub offset_y: f32,
    pub axis: RenderScrollAxis,
    pub overflow: RenderScrollOverflow,
    pub auto_scroll_focus: RenderFocusAutoScrollPolicy,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollAxis {
    #[default]
    Vertical,
    Horizontal,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderScrollOverflow {
    #[default]
    Auto,
    Scroll,
    Hidden,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderFocusAutoScrollPolicy {
    #[default]
    Nearest,
    Start,
    End,
    Disabled,
}

impl RenderScrollRegion {
    #[must_use]
    pub fn max_offset_x(&self) -> f32 {
        if self.axis != RenderScrollAxis::Horizontal || !self.overflow.scroll_enabled() {
            return 0.0;
        }
        if self.content_width.is_finite() && self.bounds.width.is_finite() {
            (self.content_width - self.bounds.width).max(0.0)
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn max_offset_y(&self) -> f32 {
        if self.axis != RenderScrollAxis::Vertical || !self.overflow.scroll_enabled() {
            return 0.0;
        }
        if self.content_height.is_finite() && self.bounds.height.is_finite() {
            (self.content_height - self.bounds.height).max(0.0)
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn clamped_offset_x(&self, offset_x: f32) -> f32 {
        if offset_x.is_finite() {
            offset_x.clamp(0.0, self.max_offset_x())
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn clamped_offset_y(&self, offset_y: f32) -> f32 {
        if offset_y.is_finite() {
            offset_y.clamp(0.0, self.max_offset_y())
        } else {
            0.0
        }
    }

    #[must_use]
    pub fn contains(&self, point: arcweft_presentation::input::ViewportPoint) -> bool {
        point.x >= self.bounds.x
            && point.x <= self.bounds.x + self.bounds.width
            && point.y >= self.bounds.y
            && point.y <= self.bounds.y + self.bounds.height
    }
}

impl RenderScrollOverflow {
    pub const fn scroll_enabled(self) -> bool {
        matches!(self, Self::Auto | Self::Scroll)
    }
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
    pub dialogue: Option<RenderDialogue>,
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
/// The player/runtime adapter resolves URLs, bundle assets, and text handoffs
/// before the frame reaches `arcweft-render-wgpu`; this type contains no I/O.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreparedViewSceneResources {
    images: Vec<PreparedViewImageResource>,
    masks: Vec<PreparedViewMaskResource>,
    glyph_handoffs: Vec<PreparedViewGlyphRunHandoff>,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedViewGlyphRunHandoff {
    pub run_index: u32,
    pub prepared_text_index: u32,
}

/// Minimal dialogue data consumed by the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderDialogue {
    pub speaker: String,
    pub text: String,
    pub base_styles: Vec<RichTextStyle>,
    pub text_runs: Vec<RichTextTextRun>,
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

/// One text block prepared for glyphon.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextBlock {
    pub target: Option<InteractionTarget>,
    pub text: String,
    pub bounds: HitRect,
    pub clip_bounds: Option<HitRect>,
    pub buffer_width: Option<f32>,
    pub buffer_height: Option<f32>,
    pub font_size: f32,
    pub line_height: f32,
    pub font_family: RenderFontFamily,
    pub weight: RenderTextWeight,
    pub slant: RenderTextSlant,
    pub rgba: [u8; 4],
    pub selection_policy: RenderTextSelectionPolicy,
    pub selection: Option<TextRange<TextByteOffset>>,
    pub selection_rgba: [f32; 4],
}

/// Static text selection policy for player-rendered text blocks.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextSelectionPolicy {
    #[default]
    Disabled,
    Enabled,
}

impl RenderTextSelectionPolicy {
    pub const fn enabled(self) -> bool {
        matches!(self, Self::Enabled)
    }
}

/// Prepared hit-test geometry for a selectable static text block.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedSelectableTextBlock {
    pub target: InteractionTarget,
    pub text: String,
    pub bounds: HitRect,
    pub clip_bounds: Option<HitRect>,
    pub character_bounds: Vec<TextCharacterBounds>,
}

/// One dialogue body laid out as a single rich-text paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderStyledParagraph {
    pub text: String,
    pub bounds: HitRect,
    pub default_style: RenderTextStyle,
    pub spans: Vec<RenderStyledTextSpan>,
    pub reveal: RenderTextReveal,
    pub glyph_transforms: Vec<RenderGlyphTransformSpan>,
    pub visual_time_millis: u64,
}

impl RenderStyledParagraph {
    #[must_use]
    pub fn reveal_complete(&self) -> bool {
        self.reveal.visible_end >= self.text.len()
    }
}

/// One typed style span inside a renderer-owned paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderStyledTextSpan {
    pub range: RichTextRange,
    pub style: RenderTextStyle,
    pub node_index: usize,
}

/// Source-range reveal mask for effects such as typewriter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderTextReveal {
    pub visible_end: usize,
}

/// One post-layout glyph transform span inside a renderer-owned paragraph.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderGlyphTransformSpan {
    pub range: RichTextRange,
    pub motion: RenderGlyphMotion,
    pub node_index: usize,
}

/// Supported deterministic glyph-position transform kinds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RenderGlyphTransformKind {
    Wave,
    Shake,
    Jitter,
}

impl RenderGlyphTransformKind {
    fn from_effect_id(id: &str) -> Option<Self> {
        match id {
            "wave" => Some(Self::Wave),
            "shake" => Some(Self::Shake),
            "jitter" => Some(Self::Jitter),
            _ => None,
        }
    }
}

/// Deterministic glyph-position transform parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RenderGlyphMotion {
    pub kind: RenderGlyphTransformKind,
    pub amplitude: f32,
    pub frequency: f32,
}

impl RenderGlyphMotion {
    pub fn offset_y(self, seconds: f32, source_byte: usize) -> f32 {
        let source_phase = source_byte.to_f32().unwrap_or(f32::MAX) * 0.58;
        let phase = seconds.mul_add(self.frequency, source_phase);
        match self.kind {
            RenderGlyphTransformKind::Wave => phase.sin() * self.amplitude,
            RenderGlyphTransformKind::Shake => {
                ((phase * 1.7).sin() * 0.6 + (phase * 2.3).cos() * 0.4) * self.amplitude
            }
            RenderGlyphTransformKind::Jitter => (phase.sin() * 12_989.0).sin() * self.amplitude,
        }
    }
}

/// Font family requested by a prepared text block.
#[derive(Clone, Debug, Default, Eq, Hash, PartialEq)]
pub enum RenderFontFamily {
    Serif,
    #[default]
    SansSerif,
    Monospace,
    Cursive,
    Fantasy,
    Named(String),
    Stack(Vec<String>),
}

impl RenderFontFamily {
    pub fn from_css_stack(stack: &str) -> Self {
        let families = stack
            .split(',')
            .map(|family| {
                family
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim()
                    .to_owned()
            })
            .filter(|family| !family.is_empty())
            .collect::<Vec<_>>();
        match families.as_slice() {
            [] => Self::SansSerif,
            [family] => Self::Named(family.clone()),
            _ => Self::Stack(families),
        }
    }

    fn from_rich_text(family: &RichTextFontFamily) -> Self {
        match family {
            RichTextFontFamily::Serif => Self::Serif,
            RichTextFontFamily::SansSerif => Self::SansSerif,
            RichTextFontFamily::Monospace => Self::Monospace,
            RichTextFontFamily::Cursive => Self::Cursive,
            RichTextFontFamily::Fantasy => Self::Fantasy,
            RichTextFontFamily::Named { name } => Self::Named(name.clone()),
        }
    }
}

/// Text weight requested by a prepared text block.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextWeight {
    #[default]
    Regular,
    Bold,
}

/// Text slant requested by a prepared text block.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextSlant {
    #[default]
    Upright,
    Italic,
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
    pub layers: LayerTree,
    pub semantics: SemanticTree,
    pub hits: HitTree,
    pub rectangles: Vec<PaintRect>,
    pub images: Vec<RenderImage>,
    pub text: Vec<RenderTextBlock>,
    pub selectable_text_blocks: Vec<PreparedSelectableTextBlock>,
    pub styled_paragraphs: Vec<RenderStyledParagraph>,
    pub choices: Vec<RenderChoice>,
    pub action_buttons: Vec<PreparedActionButton>,
    pub control_backdrops: Vec<PreparedControlBackdrop>,
    pub control_shadows: Vec<PreparedControlShadow>,
    pub control_filters: Vec<PreparedControlFilter>,
    pub control_paints: Vec<PreparedControlPaint>,
    pub scroll_regions: Vec<RenderScrollRegion>,
    pub focus_graph: PreparedFocusGraph,
    view_scenes: Vec<PreparedViewScene>,
    dialogue_present: bool,
    dialogue_reveal_complete: bool,
    interaction: InteractionVisualState,
    focused_text_input: Option<PreparedTextInputTarget>,
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

/// Pure geometry planner shared by native and browser hosts.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SharedFramePlanner;

/// Stateful geometry planner context for hosts that prepare many frames.
///
/// This owns the same kind of font system used by rendering, so custom
/// project-provided font bytes can be registered once and then reused for
/// renderer-exact text-control caret, selection, and IME geometry planning.
#[derive(Debug, Default)]
pub struct SharedFramePlanContext {
    text_control_font_context: text_controls::TextControlFontContext,
}

/// Counters exposed by the stateful frame planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SharedFramePlanStats {
    pub registered_font_bytes: usize,
    pub text_control_layout_cache_hits: u64,
    pub text_control_layout_cache_misses: u64,
    pub text_control_layout_cache_entries: usize,
}

/// Invalid frame inputs rejected before GPU work.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum FramePlanError {
    #[error("viewport must have non-zero logical and physical dimensions")]
    EmptyViewport,
    #[error("font bytes must not be empty")]
    EmptyFont,
    #[error("failed to construct stable presentation id `{value}`")]
    InvalidId { value: String },
    #[error("semantic role {role:?} is not a text-input control")]
    InvalidTextInputRole { role: SemanticRole },
    #[error(transparent)]
    TextEditor(#[from] TextEditorError),
}

impl PreparedFrame {
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

    /// Returns the renderer-backed focused text target for platform IME sync.
    ///
    /// This is the single native/web focus-target source. It is populated only
    /// by real `RenderTextInputControl` input lowered from runtime/player state.
    #[must_use]
    pub fn focused_text_input_target(&self) -> Option<PreparedTextInputTarget> {
        self.focused_text_input.clone()
    }

    #[must_use]
    pub fn selectable_text_block_at(
        &self,
        point: arcweft_presentation::input::ViewportPoint,
    ) -> Option<&PreparedSelectableTextBlock> {
        self.selectable_text_blocks.iter().rev().find(|block| {
            let visible = block.clip_bounds.unwrap_or(block.bounds);
            visible.contains(f64::from(point.x), f64::from(point.y))
        })
    }

    #[must_use]
    pub fn selectable_text_block_for_target(
        &self,
        target: &InteractionTarget,
    ) -> Option<&PreparedSelectableTextBlock> {
        self.selectable_text_blocks
            .iter()
            .rev()
            .find(|block| &block.target == target)
    }

    #[must_use]
    pub const fn has_dialogue(&self) -> bool {
        self.dialogue_present
    }

    #[must_use]
    pub const fn dialogue_reveal_complete(&self) -> bool {
        self.dialogue_reveal_complete
    }

    #[must_use]
    pub const fn has_revealing_dialogue(&self) -> bool {
        self.dialogue_present && !self.dialogue_reveal_complete
    }

    #[must_use]
    pub fn mapped_to_viewport(mut self, viewport: RenderViewport, content: ContentRect) -> Self {
        let mapping = PreparedFrameViewportMapping::new(viewport, content);
        self.viewport = viewport;
        self.map_surface_geometry(mapping);
        self.map_runtime_control_geometry(mapping);
        self.map_focused_text_input(mapping);
        self.map_scroll_regions(mapping);
        self
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
        self.text = self
            .text
            .drain(..)
            .map(|block| map_text_block(block, mapping))
            .collect();
        self.selectable_text_blocks = self
            .selectable_text_blocks
            .drain(..)
            .map(|block| map_selectable_text_block(block, mapping))
            .collect();
        self.styled_paragraphs = self
            .styled_paragraphs
            .drain(..)
            .map(|paragraph| map_styled_paragraph(paragraph, mapping))
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
            region.offset_x *= mapping.scale_x;
            region.offset_y *= mapping.scale_y;
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

fn map_text_block(
    mut block: RenderTextBlock,
    mapping: PreparedFrameViewportMapping,
) -> RenderTextBlock {
    block.bounds = mapping.rect(block.bounds);
    block.clip_bounds = block.clip_bounds.map(|bounds| mapping.rect(bounds));
    block.buffer_width = block
        .buffer_width
        .map(|width| width * mapping.scale_x.abs());
    block.buffer_height = block
        .buffer_height
        .map(|height| height * mapping.scale_y.abs());
    block.font_size *= mapping.text_scale;
    block.line_height *= mapping.text_scale;
    block
}

fn map_selectable_text_block(
    mut block: PreparedSelectableTextBlock,
    mapping: PreparedFrameViewportMapping,
) -> PreparedSelectableTextBlock {
    block.bounds = mapping.rect(block.bounds);
    block.clip_bounds = block.clip_bounds.map(|bounds| mapping.rect(bounds));
    block.character_bounds = block
        .character_bounds
        .into_iter()
        .map(|bounds| TextCharacterBounds::new(bounds.range, mapping.rect(bounds.bounds)))
        .collect();
    block
}

fn map_styled_paragraph(
    mut paragraph: RenderStyledParagraph,
    mapping: PreparedFrameViewportMapping,
) -> RenderStyledParagraph {
    paragraph.bounds = mapping.rect(paragraph.bounds);
    paragraph.default_style = map_text_style(paragraph.default_style, mapping.text_scale);
    paragraph.spans = paragraph
        .spans
        .into_iter()
        .map(|mut span| {
            span.style = map_text_style(span.style, mapping.text_scale);
            span
        })
        .collect();
    paragraph.glyph_transforms = paragraph
        .glyph_transforms
        .into_iter()
        .map(|mut transform| {
            transform.motion.amplitude *= mapping.text_scale;
            transform
        })
        .collect();
    paragraph
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

fn map_text_style(mut style: RenderTextStyle, text_scale: f32) -> RenderTextStyle {
    style.font_size *= text_scale;
    style.line_height *= text_scale;
    style
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

    pub fn push_glyph_handoff(&mut self, handoff: PreparedViewGlyphRunHandoff) {
        self.glyph_handoffs.push(handoff);
    }

    pub fn images(&self) -> &[PreparedViewImageResource] {
        &self.images
    }

    pub fn masks(&self) -> &[PreparedViewMaskResource] {
        &self.masks
    }

    pub fn glyph_handoffs(&self) -> &[PreparedViewGlyphRunHandoff] {
        &self.glyph_handoffs
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

impl SharedFramePlanner {
    /// # Panics
    ///
    /// Panics if internal layer parent ids are inconsistent. That indicates a
    /// planner bug rather than invalid caller input.
    pub fn prepare(scene: &RenderScene) -> Result<PreparedFrame, FramePlanError> {
        SharedFramePlanContext::new().prepare(scene)
    }
}

impl SharedFramePlanContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), FramePlanError> {
        self.text_control_font_context.register_font_bytes(bytes)
    }

    #[must_use]
    pub fn stats(&self) -> SharedFramePlanStats {
        self.text_control_font_context.stats()
    }

    /// # Panics
    ///
    /// Panics if internal layer parent ids are inconsistent. That indicates a
    /// planner bug rather than invalid caller input.
    pub fn prepare(&mut self, scene: &RenderScene) -> Result<PreparedFrame, FramePlanError> {
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
        let mut styled_paragraphs = Vec::new();
        let dialogue_paragraph_start = styled_paragraphs.len();
        push_dialogue_panel(
            scene,
            &mut rectangles,
            &mut text,
            &mut styled_paragraphs,
            &palette,
        );
        let dialogue_present = scene.dialogue.is_some();
        let dialogue_reveal_complete = !dialogue_present
            || styled_paragraphs[dialogue_paragraph_start..]
                .iter()
                .all(RenderStyledParagraph::reveal_complete);

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
            &mut self.text_control_font_context,
            &mut control_backdrops,
            &mut control_shadows,
            &mut control_filters,
        )?;
        let hits = semantics.to_hit_tree();

        Ok(PreparedFrame {
            viewport: scene.viewport,
            layers,
            semantics,
            hits,
            rectangles,
            images: build_retained_images(scene),
            text,
            selectable_text_blocks: Vec::new(),
            styled_paragraphs,
            choices,
            action_buttons: runtime_controls.action_buttons,
            control_backdrops,
            control_shadows,
            control_filters,
            control_paints: runtime_controls.control_paints,
            scroll_regions: scene.scroll_regions.clone(),
            focus_graph: PreparedFocusGraph::new(
                scene.focus_groups.clone(),
                scene.focus_navigation.clone(),
            ),
            view_scenes: Vec::new(),
            dialogue_present,
            dialogue_reveal_complete,
            interaction: scene.interaction.clone(),
            focused_text_input: runtime_controls.focused_text_input,
        })
    }

    pub fn prepare_selectable_text_blocks(&mut self, frame: &mut PreparedFrame) {
        let mut selectable_text_blocks = Vec::new();
        for block in &frame.text {
            if let Some((prepared, selection_rects)) = text_controls::build_selectable_text_block(
                block,
                &mut self.text_control_font_context,
            ) {
                frame.rectangles.extend(selection_rects);
                selectable_text_blocks.push(prepared);
            }
        }
        frame.selectable_text_blocks = selectable_text_blocks;
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
    focused_text_input: Option<PreparedTextInputTarget>,
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
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
    text_control_font_context: &mut text_controls::TextControlFontContext,
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
                depth_milli: text_controls::text_input_depth_milli(scene, control),
            }),
    );
    items.extend(
        scene
            .action_buttons
            .iter()
            .enumerate()
            .map(|(index, button)| RuntimeControlPlanItem::ActionButton {
                index,
                depth_milli: action_buttons::action_button_depth_milli(scene, button),
            }),
    );
    items.sort_by_key(|item| (item.depth_milli(), item.kind_order(), item.source_order()));

    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 20.0 * scale;
    let line_height = 28.0 * scale;
    let mut focused_text_input = None;
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
                let (target, paint) = text_controls::build_text_input(
                    scene,
                    &ids.text_input,
                    &control,
                    semantics,
                    rectangles,
                    text,
                    palette,
                    text_control_font_context,
                    control_backdrops,
                    control_shadows,
                    control_filters,
                )?;
                control_paints.push(paint);
                if let Some(target) = target {
                    focused_text_input = Some(target);
                }
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
                    scene,
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
                );
                prepared_buttons.push(button);
                control_paints.push(paint);
            }
        }
    }

    Ok(RuntimeControlsBuildOutput {
        focused_text_input,
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
        bounds.x - region.clamped_offset_x(region.offset_x),
        bounds.y - region.clamped_offset_y(region.offset_y),
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
                ids.dialogue.clone(),
                LayerKind::TextBox,
                order(RenderPhase::Dialogue, 0),
            )
            .with_parent(ids.root.clone())
            .with_content(LayerContent::TextBox(ids.dialogue_content.clone()))
            .with_input_policy(LayerInputPolicy::Ignore),
        )
        .expect("dialogue layer parent is present");
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

fn push_dialogue_panel(
    scene: &RenderScene,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    styled_paragraphs: &mut Vec<RenderStyledParagraph>,
    palette: &Palette,
) {
    let Some(dialogue) = &scene.dialogue else {
        return;
    };
    let panel = dialogue_panel(scene.viewport);
    rectangles.push(PaintRect::new(panel, palette.dialogue_panel));
    let inset = 28.0;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let base_style = text_style_from_styles(
        &dialogue.base_styles,
        RenderTextStyle::new(
            25.0 * scale,
            34.0 * scale,
            palette.dialogue_text,
            RenderFontFamily::SansSerif,
        ),
    );
    let speaker_style = RenderTextStyle {
        font_size: (base_style.font_size * 0.8).max(16.0 * scale),
        line_height: (base_style.line_height * 0.78).max(24.0 * scale),
        color: base_style.color,
        font_family: base_style.font_family.clone(),
        weight: RenderTextWeight::Bold,
        slant: base_style.slant,
    };
    text.push(RenderTextBlock {
        target: None,
        text: dialogue.speaker.clone(),
        bounds: HitRect::new(
            panel.x + inset,
            panel.y + 20.0,
            panel.width - inset * 2.0,
            28.0 * scale,
        ),
        clip_bounds: None,
        buffer_width: None,
        buffer_height: None,
        font_size: speaker_style.font_size,
        line_height: speaker_style.line_height,
        font_family: speaker_style.font_family,
        weight: speaker_style.weight,
        slant: speaker_style.slant,
        rgba: if dialogue.base_styles.is_empty() {
            palette.speaker_text
        } else {
            speaker_style.color
        },
        selection_policy: RenderTextSelectionPolicy::Disabled,
        selection: None,
        selection_rgba: palette.choice_active,
    });
    push_dialogue_styled_paragraph(
        styled_paragraphs,
        dialogue,
        &DialogueTextLayout {
            bounds: HitRect::new(
                panel.x + inset,
                panel.y + 58.0,
                panel.width - inset * 2.0,
                panel.height - 76.0,
            ),
            style: base_style,
            visual_time_millis: scene.visual_time_millis,
            reduce_motion: scene.preferences.reduce_motion,
        },
    );
}

impl RenderDialogue {
    pub fn plain(speaker: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            speaker: speaker.into(),
            text: text.into(),
            base_styles: Vec::new(),
            text_runs: Vec::new(),
        }
    }

    pub fn from_display_frame(frame: &LineDisplayFrame) -> Self {
        Self {
            speaker: frame
                .speaker_label
                .clone()
                .unwrap_or_else(|| frame.callee.clone()),
            text: frame.text.clone(),
            base_styles: frame.base_styles.clone(),
            text_runs: frame.display_map.text_runs.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct DialogueTextLayout {
    bounds: HitRect,
    style: RenderTextStyle,
    visual_time_millis: u64,
    reduce_motion: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RenderTextStyle {
    pub font_size: f32,
    pub line_height: f32,
    pub color: [u8; 4],
    pub font_family: RenderFontFamily,
    pub weight: RenderTextWeight,
    pub slant: RenderTextSlant,
}

impl RenderTextStyle {
    const fn new(
        font_size: f32,
        line_height: f32,
        color: [u8; 4],
        font_family: RenderFontFamily,
    ) -> Self {
        Self {
            font_size,
            line_height,
            color,
            font_family,
            weight: RenderTextWeight::Regular,
            slant: RenderTextSlant::Upright,
        }
    }
}

fn push_dialogue_styled_paragraph(
    styled_paragraphs: &mut Vec<RenderStyledParagraph>,
    dialogue: &RenderDialogue,
    layout: &DialogueTextLayout,
) {
    let runs = dialogue_text_runs(dialogue);
    let visible_end = if layout.reduce_motion {
        dialogue.text.len()
    } else {
        typewriter_visible_end(&dialogue.text, &runs, layout.visual_time_millis)
    };
    let spans = runs
        .iter()
        .filter_map(|run| render_styled_text_span(dialogue, layout, run))
        .collect();
    let glyph_transforms = if layout.reduce_motion {
        Vec::new()
    } else {
        runs.iter()
            .filter_map(|run| render_glyph_transform_span(dialogue, run))
            .collect()
    };
    styled_paragraphs.push(RenderStyledParagraph {
        text: dialogue.text.clone(),
        bounds: layout.bounds,
        default_style: layout.style.clone(),
        spans,
        reveal: RenderTextReveal {
            visible_end: visible_end.min(dialogue.text.len()),
        },
        glyph_transforms,
        visual_time_millis: layout.visual_time_millis,
    });
}

fn dialogue_text_runs(dialogue: &RenderDialogue) -> Vec<RichTextTextRun> {
    if !dialogue.text_runs.is_empty() {
        return dialogue.text_runs.clone();
    }
    let styles = dialogue.base_styles.clone();
    vec![RichTextTextRun {
        range: RichTextRange::new(0, dialogue.text.len()),
        source: RichTextTextSource::Text,
        node_index: 0,
        presentation: presentation_from_styles(&styles),
        styles,
    }]
}

fn render_styled_text_span(
    dialogue: &RenderDialogue,
    layout: &DialogueTextLayout,
    run: &RichTextTextRun,
) -> Option<RenderStyledTextSpan> {
    let range = valid_text_range(&dialogue.text, run.range)?;
    Some(RenderStyledTextSpan {
        range,
        style: text_style_from_styles(&run.styles, layout.style.clone()),
        node_index: run.node_index,
    })
}

fn render_glyph_transform_span(
    dialogue: &RenderDialogue,
    run: &RichTextTextRun,
) -> Option<RenderGlyphTransformSpan> {
    let range = valid_text_range(&dialogue.text, run.range)?;
    Some(RenderGlyphTransformSpan {
        range,
        motion: glyph_motion(&run.presentation.effects)?,
        node_index: run.node_index,
    })
}

fn valid_text_range(text: &str, range: RichTextRange) -> Option<RichTextRange> {
    let start = range.start.min(text.len());
    let end = range.end.min(text.len());
    if start >= end || text.get(start..end).is_none() {
        return None;
    }
    Some(RichTextRange::new(start, end))
}

fn typewriter_visible_end(text: &str, runs: &[RichTextTextRun], visual_time_millis: u64) -> usize {
    const DEFAULT_TYPEWRITER_CPS: f32 = 28.0;
    let cps = runs
        .iter()
        .flat_map(|run| &run.presentation.effects)
        .find(|effect| effect.id == "typewriter" && effect.phase == RichTextEffectPhase::GlyphMask)
        .map_or(DEFAULT_TYPEWRITER_CPS, typewriter_cps);
    let visible_chars = ((visual_time_millis.to_f32().unwrap_or(f32::MAX) / 1_000.0) * cps)
        .floor()
        .to_usize()
        .unwrap_or(usize::MAX);
    text.char_indices()
        .map(|(index, _)| index)
        .chain(std::iter::once(text.len()))
        .nth(visible_chars)
        .unwrap_or(text.len())
}

fn typewriter_cps(effect: &RichTextEffectDescriptor) -> f32 {
    effect
        .params
        .get("cps")
        .or_else(|| effect.params.get("speed"))
        .and_then(param_f32)
        .unwrap_or(28.0)
        .clamp(1.0, 240.0)
}

fn glyph_motion(effects: &[RichTextEffectDescriptor]) -> Option<RenderGlyphMotion> {
    effects
        .iter()
        .find(|effect| {
            RenderGlyphTransformKind::from_effect_id(&effect.id).is_some()
                && effect.phase == RichTextEffectPhase::GlyphTransform
        })
        .map(|effect| RenderGlyphMotion {
            kind: RenderGlyphTransformKind::from_effect_id(&effect.id)
                .expect("effect id was matched above"),
            amplitude: effect
                .params
                .get("amp")
                .or_else(|| effect.params.get("amplitude"))
                .and_then(param_f32)
                .unwrap_or(4.0)
                .clamp(0.0, 24.0),
            frequency: effect
                .params
                .get("freq")
                .or_else(|| effect.params.get("frequency"))
                .and_then(param_f32)
                .unwrap_or(7.0)
                .clamp(0.1, 24.0),
        })
}

fn text_style_from_styles(styles: &[RichTextStyle], fallback: RenderTextStyle) -> RenderTextStyle {
    styles.iter().fold(fallback, apply_text_style)
}

fn apply_text_style(mut style: RenderTextStyle, rich_style: &RichTextStyle) -> RenderTextStyle {
    match rich_style {
        RichTextStyle::Em { .. } | RichTextStyle::Italic { .. } | RichTextStyle::Oblique { .. } => {
            style.slant = RenderTextSlant::Italic;
        }
        RichTextStyle::Strong { .. } => style.weight = RenderTextWeight::Bold,
        RichTextStyle::Color { value } => style.color = rich_text_color(value),
        RichTextStyle::Font { family } => {
            style.font_family = RenderFontFamily::from_rich_text(family);
        }
        RichTextStyle::Size {
            points: Some(points),
            ..
        } => {
            style.font_size = f32::from(*points);
            style.line_height = style.font_size * 1.35;
        }
        RichTextStyle::Size { points: None, .. }
        | RichTextStyle::Speed { .. }
        | RichTextStyle::Layout { .. }
        | RichTextStyle::Transform { .. }
        | RichTextStyle::Presentation { .. }
        | RichTextStyle::Effect { .. }
        | RichTextStyle::Shader { .. }
        | RichTextStyle::Object { .. }
        | RichTextStyle::Unknown { .. } => {}
    }
    style
}

fn rich_text_color(color: &RichTextColor) -> [u8; 4] {
    match color {
        RichTextColor::Rgb { red, green, blue } => [*red, *green, *blue, 255],
        RichTextColor::Named { name } => match name.as_str() {
            "red" => [240, 110, 110, 255],
            "green" => [120, 220, 150, 255],
            "blue" => [130, 180, 255, 255],
            "yellow" => [240, 220, 120, 255],
            "muted" | "quiet" => [170, 170, 170, 255],
            _ => [245, 245, 245, 255],
        },
    }
}

fn param_f32(param: &RichTextParam) -> Option<f32> {
    match param {
        RichTextParam::Int { value } => value.to_f32(),
        RichTextParam::Milli { value } => Some(value.as_f32()),
        RichTextParam::Text { value } | RichTextParam::Raw { value } => {
            value.trim().trim_end_matches("px").parse().ok()
        }
        RichTextParam::Bool { .. }
        | RichTextParam::Vec2 { .. }
        | RichTextParam::Selector { .. }
        | RichTextParam::Expr { .. } => None,
    }
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
    text: &mut Vec<RenderTextBlock>,
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
    let top = scene.dialogue.as_ref().map_or_else(
        || ((scene.viewport.logical_height - total) * 0.42).max(36.0),
        |_| {
            let panel = dialogue_panel(scene.viewport);
            (panel.y - total - 22.0).max(36.0)
        },
    );
    let left = (scene.viewport.logical_width - width) * 0.5;
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 22.0 * scale;
    let line_height = 30.0 * scale;

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
            text.push(RenderTextBlock {
                target: None,
                text: choice.label.clone(),
                bounds: HitRect::new(
                    bounds.x + 24.0,
                    bounds.y + (bounds.height - line_height) * 0.5,
                    bounds.width - 48.0,
                    line_height,
                ),
                clip_bounds: None,
                buffer_width: None,
                buffer_height: None,
                font_size,
                line_height,
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Bold,
                slant: RenderTextSlant::Upright,
                rgba: palette.choice_text,
                selection_policy: RenderTextSelectionPolicy::Disabled,
                selection: None,
                selection_rgba: palette.choice_active,
            });
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

fn dialogue_panel(viewport: RenderViewport) -> HitRect {
    let margin = (viewport.logical_width * 0.045).max(24.0);
    let height = (viewport.logical_height * 0.28).clamp(180.0, 320.0);
    HitRect::new(
        margin,
        viewport.logical_height - height - margin,
        viewport.logical_width - margin * 2.0,
        height,
    )
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
    DialogueLayer,
    ChoiceLayer,
    TextInputLayer,
    ActionButtonLayer,
    DialogueContent,
    ChoiceContent,
    TextInputContent,
    ActionButtonContent,
}

impl FrameStaticId {
    const fn as_str(self) -> &'static str {
        match self {
            Self::RootLayer => "layer.player.root",
            Self::DialogueLayer => "layer.player.dialogue",
            Self::ChoiceLayer => "layer.player.choice",
            Self::TextInputLayer => "layer.player.text_input",
            Self::ActionButtonLayer => "layer.player.action_button",
            Self::DialogueContent => "textbox.player.dialogue",
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
    dialogue: LayerId,
    choice: LayerId,
    text_input: LayerId,
    action_button: LayerId,
    dialogue_content: PublicId,
    choice_content: PublicId,
    text_input_content: PublicId,
    action_button_content: PublicId,
}

impl FrameIds {
    fn new() -> Result<Self, FramePlanError> {
        Ok(Self {
            root: FrameStaticId::RootLayer.layer_id()?,
            dialogue: FrameStaticId::DialogueLayer.layer_id()?,
            choice: FrameStaticId::ChoiceLayer.layer_id()?,
            text_input: FrameStaticId::TextInputLayer.layer_id()?,
            action_button: FrameStaticId::ActionButtonLayer.layer_id()?,
            dialogue_content: FrameStaticId::DialogueContent.public_id()?,
            choice_content: FrameStaticId::ChoiceContent.public_id()?,
            text_input_content: FrameStaticId::TextInputContent.public_id()?,
            action_button_content: FrameStaticId::ActionButtonContent.public_id()?,
        })
    }
}

struct Palette {
    background: [f32; 4],
    dialogue_panel: [f32; 4],
    choice_idle: [f32; 4],
    choice_active: [f32; 4],
    choice_pressed: [f32; 4],
    focus_ring: [f32; 4],
    speaker_text: [u8; 4],
    dialogue_text: [u8; 4],
    choice_text: [u8; 4],
}

impl Palette {
    fn from_preferences(preferences: RenderPreferences) -> Self {
        if preferences.high_contrast {
            Self {
                background: [0.0, 0.0, 0.0, 1.0],
                dialogue_panel: [0.02, 0.02, 0.02, 0.98],
                choice_idle: [0.08, 0.08, 0.08, 1.0],
                choice_active: [0.2, 0.2, 0.2, 1.0],
                choice_pressed: [0.32, 0.32, 0.32, 1.0],
                focus_ring: [1.0, 1.0, 0.0, 1.0],
                speaker_text: [255, 255, 0, 255],
                dialogue_text: [255, 255, 255, 255],
                choice_text: [255, 255, 255, 255],
            }
        } else {
            Self {
                background: [0.019, 0.027, 0.024, 1.0],
                dialogue_panel: [0.066, 0.071, 0.064, 0.95],
                choice_idle: [0.125, 0.124, 0.099, 0.98],
                choice_active: [0.119, 0.235, 0.153, 1.0],
                choice_pressed: [0.207, 0.3, 0.164, 1.0],
                focus_ring: [0.886, 0.914, 0.384, 1.0],
                speaker_text: [174, 226, 142, 255],
                dialogue_text: [248, 246, 234, 255],
                choice_text: [255, 252, 238, 255],
            }
        }
    }
}
