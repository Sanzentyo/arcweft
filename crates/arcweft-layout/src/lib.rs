//! Sans I/O layout contracts shared by renderers, capture adapters, and Agent observation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Two-dimensional layout size in design-space logical pixels unless paired with an explicit basis.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutSize {
    pub width: f32,
    pub height: f32,
}

/// Two-dimensional layout point in the selected layout space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutPoint {
    pub x: f32,
    pub y: f32,
}

/// Axis-aligned rectangle in the selected layout space.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutRect {
    pub origin: LayoutPoint,
    pub size: LayoutSize,
}

/// Public coordinate spaces that may cross renderer, capture, and Agent observation boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutCoordinateSpace {
    /// Project-authored design logical pixels before fit transforms.
    #[default]
    Design,
    /// The fitted design viewport rectangle inside output space.
    Content,
    /// Output logical pixels after fit transforms and before device-pixel scaling.
    Output,
    /// Device pixels after host scale factor / device-pixel-ratio conversion.
    Physical,
    /// Host UI logical pixels, such as windowing-system logical coordinates.
    Logical,
    /// Object-local coordinates whose parent object id is carried by nearby metadata.
    ObjectLocal,
    /// Layer-local coordinates whose parent layer id is carried by nearby metadata.
    LayerLocal,
}

/// Insets or overflows in output logical pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LayoutInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Policy used when a design viewport is mapped into an output viewport.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScalePolicy {
    /// Preserve raw design coordinates with no implicit scale or letterbox.
    #[default]
    Raw,
    /// Preserve aspect ratio and fit the complete design viewport.
    Contain,
    /// Preserve aspect ratio and fill the output viewport, cropping overflow.
    Cover,
    /// Scale width and height independently to the output viewport.
    Stretch,
}

/// Computed content rectangle and scale for one design/output viewport pair.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct ContentRect {
    pub design_size: LayoutSize,
    pub output_size: LayoutSize,
    pub rect: LayoutRect,
    pub scale_x: f32,
    pub scale_y: f32,
    pub policy: ScalePolicy,
}

/// Serializable fit-transform metadata shared by renderers, captures, and Agent observation.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct FitTransformMetadata {
    pub policy: ScalePolicy,
    pub design_space: LayoutCoordinateSpace,
    pub content_space: LayoutCoordinateSpace,
    pub output_space: LayoutCoordinateSpace,
    pub serialized_geometry_space: LayoutCoordinateSpace,
    pub hit_test_input_space: LayoutCoordinateSpace,
    pub design_viewport: LayoutSize,
    pub output_viewport: LayoutSize,
    pub content_rect: LayoutRect,
    pub visible_output_rect: LayoutRect,
    pub visible_design_rect: LayoutRect,
    pub scale_x: f32,
    pub scale_y: f32,
    pub bars: LayoutInsets,
    pub crop: LayoutInsets,
    pub raw_pixel_mode: bool,
}

/// Coordinate basis accepted by deterministic hit-test conversion.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitTestInputSpace {
    /// Input points are already in design space.
    Design,
    /// Input points are in output logical pixels and must be inverse-mapped.
    #[default]
    Output,
}

/// Result of converting one hit-test point through a fit transform.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitTestMapping {
    pub input_space: HitTestInputSpace,
    pub design_point: LayoutPoint,
    pub output_point: LayoutPoint,
    pub inside_design_viewport: bool,
    pub inside_content_rect: bool,
    pub inside_output_viewport: bool,
}

/// Unit accepted by shared layout length expressions.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutUnit {
    Px,
    Sp,
    Percent,
    Vw,
    Vh,
    Cw,
    Ch,
    Em,
    GlyphCh,
    SafeAreaTop,
    SafeAreaRight,
    SafeAreaBottom,
    SafeAreaLeft,
}

/// Boundary where a layout unit can first be resolved without guessing missing context.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LayoutUnitResolutionPhase {
    Hir,
    Sema,
    RuntimePlan,
    UiLayout,
    Renderer,
    AgentObserve,
}

/// Deterministic layout length expression evaluated against a layout context.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LayoutLengthExpr {
    Literal {
        value: f32,
        unit: LayoutUnit,
    },
    Add {
        left: Box<Self>,
        right: Box<Self>,
    },
    Sub {
        left: Box<Self>,
        right: Box<Self>,
    },
    Mul {
        expr: Box<Self>,
        factor: f32,
    },
    Div {
        expr: Box<Self>,
        divisor: f32,
    },
    Min {
        items: Vec<Self>,
    },
    Max {
        items: Vec<Self>,
    },
    Clamp {
        min: Box<Self>,
        value: Box<Self>,
        max: Box<Self>,
    },
}

/// Shared text overflow policy crossing renderer and Agent observation boundaries.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextOverflowPolicy {
    #[default]
    Clip,
    Page,
    FitText,
    ExpandBox,
    Diagnostic,
}

/// Shared text fitting result produced after measurement and before capture reporting.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextFitResult {
    pub policy: TextOverflowPolicy,
    pub pages: Vec<TextPage>,
    pub fitted_font_size: Option<f32>,
    pub expanded_bounds: Option<LayoutRect>,
    pub diagnostics: Vec<TextFitDiagnostic>,
}

/// Stable summary of the renderer-independent text fitting outcome.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextFitOutcome {
    #[default]
    Fits,
    Clipped,
    Paginated,
    Scaled,
    Expanded,
    Failed,
}

/// Text fitting report shape suitable for diagnostics, capture metadata, and Agent observe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TextFitReport {
    pub outcome: TextFitOutcome,
    pub flags: TextFitReportFlags,
    pub page_count: usize,
    pub fitted_font_size: Option<f32>,
    pub expanded_bounds: Option<LayoutRect>,
    pub diagnostics: Vec<TextFitDiagnostic>,
}

/// Compact flags describing which text-fitting behaviors occurred.
#[must_use]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextFitReportFlags {
    bits: u8,
}

/// One page of shaped text expressed in stable shaped-cluster indices.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextPage {
    pub cluster_start: usize,
    pub cluster_end: usize,
}

/// Diagnostic emitted while applying text fitting policy.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct TextFitDiagnostic {
    pub code: TextFitDiagnosticCode,
    pub message: String,
}

/// Stable text fitting diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TextFitDiagnosticCode {
    OverflowClipped,
    TextTruncated,
    FitTextReachedMinimum,
    FitTextFailed,
    PaginationLimitReached,
    ExpandBoxConstrained,
    GlyphMetricFallback,
}

/// Capture scope used by selected object/layer capture metadata.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureScope {
    Viewport,
    Layer { id: String },
    Object { id: String },
}

/// Capture composition policy that is shared; resource I/O remains adapter-specific.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureComposition {
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

/// Renderer label for capture metadata. This is identity only, not a backend dependency.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureRendererKind {
    #[default]
    NativeRichTextObserver,
    SharedWebGpuScene,
    NativeWgpuAdapter,
}

/// Bounds carried by selected object/layer captures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureCropBounds {
    pub basis: LayoutCoordinateSpace,
    pub unclipped: LayoutRect,
    pub clipped: LayoutRect,
}

/// Mask and object-id metadata for selected object/layer captures.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureMaskMetadata {
    pub basis: LayoutCoordinateSpace,
    pub bounds: LayoutRect,
    pub object_ids: Vec<String>,
    pub layer_ids: Vec<String>,
    pub has_object_id_attachment: bool,
    pub has_alpha_mask: bool,
}

/// Shared capture metadata that capture adapters attach to object/layer image resources.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaptureMetadata {
    pub renderer: CaptureRendererKind,
    pub scope: CaptureScope,
    pub composition: CaptureComposition,
    pub coordinate_basis: LayoutCoordinateSpace,
    pub crop: CaptureCropBounds,
    pub mask: Option<CaptureMaskMetadata>,
    pub fit_transform: FitTransformMetadata,
}

/// Complete context required to evaluate a layout length expression.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutEvaluationContext {
    pub design_viewport: LayoutSize,
    pub output_viewport: LayoutSize,
    pub content_rect: ContentRect,
    pub containing_box: LayoutSize,
    pub font_size: f32,
    pub glyph_ch: f32,
    pub safe_area: SafeAreaInsets,
}

/// Safe-area insets in output pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SafeAreaInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

/// Error produced by deterministic layout evaluation.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum LayoutError {
    #[error("layout viewport must have finite positive dimensions")]
    EmptyViewport,
    #[error("layout division by zero")]
    DivisionByZero,
    #[error("layout expression is not finite")]
    NonFinite,
}

impl LayoutSize {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub fn is_positive(self) -> bool {
        self.width.is_finite() && self.height.is_finite() && self.width > 0.0 && self.height > 0.0
    }
}

impl LayoutPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl LayoutRect {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            origin: LayoutPoint::new(x, y),
            size: LayoutSize::new(width, height),
        }
    }

    pub fn contains(self, point: LayoutPoint) -> bool {
        point.x >= self.origin.x
            && point.y >= self.origin.y
            && point.x <= self.right()
            && point.y <= self.bottom()
    }

    pub fn right(self) -> f32 {
        self.origin.x + self.size.width
    }

    pub fn bottom(self) -> f32 {
        self.origin.y + self.size.height
    }

    #[must_use]
    pub fn clipped_to(self, viewport: LayoutSize) -> Self {
        let x0 = self.origin.x.max(0.0);
        let y0 = self.origin.y.max(0.0);
        let x1 = self.right().min(viewport.width).max(x0);
        let y1 = self.bottom().min(viewport.height).max(y0);
        Self::new(x0, y0, x1 - x0, y1 - y0)
    }
}

impl LayoutCoordinateSpace {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::Content => "content",
            Self::Output => "output",
            Self::Physical => "physical",
            Self::Logical => "logical",
            Self::ObjectLocal => "object_local",
            Self::LayerLocal => "layer_local",
        }
    }
}

impl LayoutInsets {
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }
}

impl ScalePolicy {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Raw => "raw",
            Self::Contain => "contain",
            Self::Cover => "cover",
            Self::Stretch => "stretch",
        }
    }
}

impl ContentRect {
    pub fn calculate(
        design_size: LayoutSize,
        output_size: LayoutSize,
        policy: ScalePolicy,
    ) -> Result<Self, LayoutError> {
        if !design_size.is_positive() || !output_size.is_positive() {
            return Err(LayoutError::EmptyViewport);
        }
        let (width, height, scale_x, scale_y) = match policy {
            ScalePolicy::Raw => (design_size.width, design_size.height, 1.0, 1.0),
            ScalePolicy::Contain => {
                let scale = (output_size.width / design_size.width)
                    .min(output_size.height / design_size.height);
                (
                    design_size.width * scale,
                    design_size.height * scale,
                    scale,
                    scale,
                )
            }
            ScalePolicy::Cover => {
                let scale = (output_size.width / design_size.width)
                    .max(output_size.height / design_size.height);
                (
                    design_size.width * scale,
                    design_size.height * scale,
                    scale,
                    scale,
                )
            }
            ScalePolicy::Stretch => (
                output_size.width,
                output_size.height,
                output_size.width / design_size.width,
                output_size.height / design_size.height,
            ),
        };
        let (x, y) = match policy {
            ScalePolicy::Raw => (0.0, 0.0),
            ScalePolicy::Contain | ScalePolicy::Cover | ScalePolicy::Stretch => (
                (output_size.width - width) * 0.5,
                (output_size.height - height) * 0.5,
            ),
        };
        Ok(Self {
            design_size,
            output_size,
            rect: LayoutRect::new(x, y, width, height),
            scale_x,
            scale_y,
            policy,
        })
    }

    pub fn map_point(self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(
            self.rect.origin.x + point.x * self.scale_x,
            self.rect.origin.y + point.y * self.scale_y,
        )
    }

    pub fn unmap_point(self, point: LayoutPoint) -> LayoutPoint {
        LayoutPoint::new(
            (point.x - self.rect.origin.x) / self.scale_x,
            (point.y - self.rect.origin.y) / self.scale_y,
        )
    }

    pub fn map_rect(self, rect: LayoutRect) -> LayoutRect {
        let origin = self.map_point(rect.origin);
        LayoutRect::new(
            origin.x,
            origin.y,
            rect.size.width * self.scale_x,
            rect.size.height * self.scale_y,
        )
    }

    pub fn unmap_rect(self, rect: LayoutRect) -> LayoutRect {
        let origin = self.unmap_point(rect.origin);
        LayoutRect::new(
            origin.x,
            origin.y,
            rect.size.width / self.scale_x,
            rect.size.height / self.scale_y,
        )
    }

    pub fn visible_output_rect(self) -> LayoutRect {
        self.rect.clipped_to(self.output_size)
    }

    pub fn visible_design_rect(self) -> LayoutRect {
        self.unmap_rect(self.visible_output_rect())
            .clipped_to(self.design_size)
    }

    pub fn bars(self) -> LayoutInsets {
        LayoutInsets::new(
            self.rect.origin.y.max(0.0),
            (self.output_size.width - self.rect.right()).max(0.0),
            (self.output_size.height - self.rect.bottom()).max(0.0),
            self.rect.origin.x.max(0.0),
        )
    }

    pub fn crop(self) -> LayoutInsets {
        LayoutInsets::new(
            (-self.rect.origin.y).max(0.0),
            (self.rect.right() - self.output_size.width).max(0.0),
            (self.rect.bottom() - self.output_size.height).max(0.0),
            (-self.rect.origin.x).max(0.0),
        )
    }

    pub fn fit_transform_metadata(
        self,
        serialized_geometry_space: LayoutCoordinateSpace,
        hit_test_input_space: LayoutCoordinateSpace,
    ) -> FitTransformMetadata {
        FitTransformMetadata {
            policy: self.policy,
            design_space: LayoutCoordinateSpace::Design,
            content_space: LayoutCoordinateSpace::Content,
            output_space: LayoutCoordinateSpace::Output,
            serialized_geometry_space,
            hit_test_input_space,
            design_viewport: self.design_size,
            output_viewport: self.output_size,
            content_rect: self.rect,
            visible_output_rect: self.visible_output_rect(),
            visible_design_rect: self.visible_design_rect(),
            scale_x: self.scale_x,
            scale_y: self.scale_y,
            bars: self.bars(),
            crop: self.crop(),
            raw_pixel_mode: self.policy == ScalePolicy::Raw,
        }
    }

    pub fn hit_test_mapping(
        self,
        point: LayoutPoint,
        input_space: HitTestInputSpace,
    ) -> HitTestMapping {
        let (design_point, output_point) = match input_space {
            HitTestInputSpace::Design => (point, self.map_point(point)),
            HitTestInputSpace::Output => (self.unmap_point(point), point),
        };
        HitTestMapping {
            input_space,
            design_point,
            output_point,
            inside_design_viewport: LayoutRect::new(
                0.0,
                0.0,
                self.design_size.width,
                self.design_size.height,
            )
            .contains(design_point),
            inside_content_rect: self.rect.contains(output_point),
            inside_output_viewport: LayoutRect::new(
                0.0,
                0.0,
                self.output_size.width,
                self.output_size.height,
            )
            .contains(output_point),
        }
    }
}

impl LayoutLengthExpr {
    pub fn evaluate(
        &self,
        context: &LayoutEvaluationContext,
        horizontal_axis: bool,
    ) -> Result<f32, LayoutError> {
        let value = match self {
            Self::Literal { value, unit } => unit.evaluate(*value, context, horizontal_axis),
            Self::Add { left, right } => {
                left.evaluate(context, horizontal_axis)?
                    + right.evaluate(context, horizontal_axis)?
            }
            Self::Sub { left, right } => {
                left.evaluate(context, horizontal_axis)?
                    - right.evaluate(context, horizontal_axis)?
            }
            Self::Mul { expr, factor } => expr.evaluate(context, horizontal_axis)? * factor,
            Self::Div { divisor: 0.0, .. } => return Err(LayoutError::DivisionByZero),
            Self::Div { expr, divisor } => expr.evaluate(context, horizontal_axis)? / divisor,
            Self::Min { items } => items.iter().try_fold(f32::INFINITY, |best, item| {
                item.evaluate(context, horizontal_axis)
                    .map(|value| best.min(value))
            })?,
            Self::Max { items } => items.iter().try_fold(f32::NEG_INFINITY, |best, item| {
                item.evaluate(context, horizontal_axis)
                    .map(|value| best.max(value))
            })?,
            Self::Clamp { min, value, max } => value.evaluate(context, horizontal_axis)?.clamp(
                min.evaluate(context, horizontal_axis)?,
                max.evaluate(context, horizontal_axis)?,
            ),
        };
        value
            .is_finite()
            .then_some(value)
            .ok_or(LayoutError::NonFinite)
    }
}

impl LayoutUnit {
    pub fn evaluate(
        self,
        value: f32,
        context: &LayoutEvaluationContext,
        horizontal_axis: bool,
    ) -> f32 {
        match self {
            Self::Px => value,
            Self::Sp | Self::Em => value * context.font_size,
            Self::Percent => {
                value
                    * 0.01
                    * if horizontal_axis {
                        context.containing_box.width
                    } else {
                        context.containing_box.height
                    }
            }
            Self::Vw => value * 0.01 * context.design_viewport.width,
            Self::Vh => value * 0.01 * context.design_viewport.height,
            Self::Cw => value * 0.01 * context.content_rect.rect.size.width,
            Self::Ch => value * 0.01 * context.content_rect.rect.size.height,
            Self::GlyphCh => value * context.glyph_ch,
            Self::SafeAreaTop => value * context.safe_area.top,
            Self::SafeAreaRight => value * context.safe_area.right,
            Self::SafeAreaBottom => value * context.safe_area.bottom,
            Self::SafeAreaLeft => value * context.safe_area.left,
        }
    }

    pub const fn earliest_resolution_phase(self) -> LayoutUnitResolutionPhase {
        match self {
            Self::Px => LayoutUnitResolutionPhase::RuntimePlan,
            Self::Vw | Self::Vh => LayoutUnitResolutionPhase::UiLayout,
            Self::Percent
            | Self::Cw
            | Self::Ch
            | Self::SafeAreaTop
            | Self::SafeAreaRight
            | Self::SafeAreaBottom
            | Self::SafeAreaLeft
            | Self::Sp
            | Self::Em
            | Self::GlyphCh => LayoutUnitResolutionPhase::Renderer,
        }
    }

    pub const fn requires_font_metrics(self) -> bool {
        matches!(self, Self::Sp | Self::Em | Self::GlyphCh)
    }

    pub const fn requires_safe_area(self) -> bool {
        matches!(
            self,
            Self::SafeAreaTop | Self::SafeAreaRight | Self::SafeAreaBottom | Self::SafeAreaLeft
        )
    }

    pub const fn requires_content_rect(self) -> bool {
        matches!(self, Self::Cw | Self::Ch)
    }
}

impl TextFitResult {
    pub fn report(&self) -> TextFitReport {
        let truncated = self.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                TextFitDiagnosticCode::OverflowClipped | TextFitDiagnosticCode::TextTruncated
            )
        });
        let failed = self.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                TextFitDiagnosticCode::FitTextFailed
                    | TextFitDiagnosticCode::PaginationLimitReached
                    | TextFitDiagnosticCode::ExpandBoxConstrained
            )
        });
        let scaled = self.policy == TextOverflowPolicy::FitText && self.fitted_font_size.is_some();
        let paginated = self.policy == TextOverflowPolicy::Page && self.pages.len() > 1;
        let expanded =
            self.policy == TextOverflowPolicy::ExpandBox && self.expanded_bounds.is_some();
        let outcome = if failed {
            TextFitOutcome::Failed
        } else if truncated {
            TextFitOutcome::Clipped
        } else if paginated {
            TextFitOutcome::Paginated
        } else if scaled {
            TextFitOutcome::Scaled
        } else if expanded {
            TextFitOutcome::Expanded
        } else {
            TextFitOutcome::Fits
        };
        let mut flags = TextFitReportFlags::empty();
        if truncated {
            flags = flags.with_truncated();
        }
        if scaled {
            flags = flags.with_scaled();
        }
        if paginated {
            flags = flags.with_paginated();
        }
        if expanded {
            flags = flags.with_expanded();
        }
        if failed {
            flags = flags.with_failed();
        }

        TextFitReport {
            outcome,
            flags,
            page_count: self.pages.len(),
            fitted_font_size: self.fitted_font_size,
            expanded_bounds: self.expanded_bounds,
            diagnostics: self.diagnostics.clone(),
        }
    }
}

impl TextFitReportFlags {
    const TRUNCATED: u8 = 1 << 0;
    const SCALED: u8 = 1 << 1;
    const PAGINATED: u8 = 1 << 2;
    const EXPANDED: u8 = 1 << 3;
    const FAILED: u8 = 1 << 4;

    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    pub const fn bits(self) -> u8 {
        self.bits
    }

    pub const fn truncated(self) -> bool {
        self.bits & Self::TRUNCATED != 0
    }

    pub const fn scaled(self) -> bool {
        self.bits & Self::SCALED != 0
    }

    pub const fn paginated(self) -> bool {
        self.bits & Self::PAGINATED != 0
    }

    pub const fn expanded(self) -> bool {
        self.bits & Self::EXPANDED != 0
    }

    pub const fn failed(self) -> bool {
        self.bits & Self::FAILED != 0
    }

    pub const fn with_truncated(self) -> Self {
        self.with_bit(Self::TRUNCATED)
    }

    pub const fn with_scaled(self) -> Self {
        self.with_bit(Self::SCALED)
    }

    pub const fn with_paginated(self) -> Self {
        self.with_bit(Self::PAGINATED)
    }

    pub const fn with_expanded(self) -> Self {
        self.with_bit(Self::EXPANDED)
    }

    pub const fn with_failed(self) -> Self {
        self.with_bit(Self::FAILED)
    }

    const fn with_bit(self, bit: u8) -> Self {
        Self {
            bits: self.bits | bit,
        }
    }
}

impl TextFitReport {
    pub const fn truncated(&self) -> bool {
        self.flags.truncated()
    }

    pub const fn scaled(&self) -> bool {
        self.flags.scaled()
    }

    pub const fn paginated(&self) -> bool {
        self.flags.paginated()
    }

    pub const fn expanded(&self) -> bool {
        self.flags.expanded()
    }

    pub const fn failed(&self) -> bool {
        self.flags.failed()
    }
}

impl CaptureMetadata {
    pub fn selected_object(
        renderer: CaptureRendererKind,
        object_id: impl Into<String>,
        unclipped: LayoutRect,
        clipped: LayoutRect,
        fit_transform: FitTransformMetadata,
    ) -> Self {
        let object_id = object_id.into();
        Self {
            renderer,
            scope: CaptureScope::Object {
                id: object_id.clone(),
            },
            composition: CaptureComposition::MaskedFramebufferCrop,
            coordinate_basis: LayoutCoordinateSpace::Output,
            crop: CaptureCropBounds {
                basis: LayoutCoordinateSpace::Output,
                unclipped,
                clipped,
            },
            mask: Some(CaptureMaskMetadata {
                basis: LayoutCoordinateSpace::Output,
                bounds: clipped,
                object_ids: vec![object_id],
                layer_ids: Vec::new(),
                has_object_id_attachment: true,
                has_alpha_mask: true,
            }),
            fit_transform,
        }
    }

    pub fn selected_layer(
        renderer: CaptureRendererKind,
        layer_id: impl Into<String>,
        unclipped: LayoutRect,
        clipped: LayoutRect,
        fit_transform: FitTransformMetadata,
    ) -> Self {
        let layer_id = layer_id.into();
        Self {
            renderer,
            scope: CaptureScope::Layer {
                id: layer_id.clone(),
            },
            composition: CaptureComposition::MaskedFramebufferCrop,
            coordinate_basis: LayoutCoordinateSpace::Output,
            crop: CaptureCropBounds {
                basis: LayoutCoordinateSpace::Output,
                unclipped,
                clipped,
            },
            mask: Some(CaptureMaskMetadata {
                basis: LayoutCoordinateSpace::Output,
                bounds: clipped,
                object_ids: Vec::new(),
                layer_ids: vec![layer_id],
                has_object_id_attachment: true,
                has_alpha_mask: true,
            }),
            fit_transform,
        }
    }
}
