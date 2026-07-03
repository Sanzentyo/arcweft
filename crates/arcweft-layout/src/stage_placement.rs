//! Deterministic stage placement shared by image and character presentation objects.
//!
//! The authored placement is renderer-independent.  Player frame planning resolves
//! it into output logical coordinates before native, web, offscreen capture, or
//! Agent observation consume the frame.

use crate::{
    ContentRect, FitTransformMetadata, LayoutCoordinateSpace, LayoutError, LayoutPoint, LayoutRect,
    LayoutSize, SafeAreaInsets, ScalePolicy,
};
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};
use thiserror::Error;

const DEFAULT_EPSILON: f32 = 0.001;

/// Authored placement for an image-like or character-stage object.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum StagePlacement {
    /// Explicit output-logical absolute pixels. This intentionally does not
    /// scale from the design viewport.
    Absolute { rect: StageRect },
    /// Responsive anchor placement authored in design viewport space.
    Anchor {
        anchor: StageAnchor,
        #[serde(default)]
        object_anchor: StageAnchor,
        #[serde(default)]
        margins: StageInsets,
        size: StageSize,
        #[serde(default)]
        scale: StageScalePolicy,
        #[serde(default)]
        safe_area: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        baseline: Option<StageBaseline>,
    },
}

/// Rectangle in fixed-point logical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageRect {
    pub x_milli: i32,
    pub y_milli: i32,
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Size in fixed-point logical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageSize {
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Margins/insets in fixed-point logical pixels.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct StageInsets {
    pub top_milli: i32,
    pub right_milli: i32,
    pub bottom_milli: i32,
    pub left_milli: i32,
}

/// Anchor point on the viewport/safe-area rectangle or object box.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StageAnchor {
    #[default]
    TopLeft,
    TopCenter,
    TopRight,
    CenterLeft,
    Center,
    CenterRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    Custom {
        x_milli: i32,
        y_milli: i32,
    },
}

/// How responsive placement sizes and margins scale.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageScalePolicy {
    /// Authored size/margins are design viewport pixels and are mapped through
    /// the viewport content transform. This is the intended stage path.
    #[default]
    Design,
    /// Anchor to the output viewport but keep the authored size/margins in
    /// output logical pixels. This is deterministic but should be rare.
    FixedOutput,
}

/// Optional baseline semantics for character-stage objects.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StageBaseline {
    Bottom,
    Ground,
}

/// Context available when player frame planning resolves placement.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct StagePlacementContext {
    pub design_viewport: LayoutSize,
    pub output_viewport: LayoutSize,
    pub physical_viewport: LayoutSize,
    pub scale_factor: f32,
    pub safe_area: SafeAreaInsets,
    pub viewport_policy: ScalePolicy,
}

/// Resolved placement used by rendering, hit-test, capture, and observe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ResolvedStagePlacement {
    pub authored: StagePlacement,
    pub authored_space: LayoutCoordinateSpace,
    pub design_bbox: LayoutRect,
    pub output_bbox: LayoutRect,
    pub physical_bbox: LayoutRect,
    pub fit_transform: FitTransformMetadata,
    pub diagnostics: Vec<StagePlacementDiagnostic>,
}

/// One deterministic placement diagnostic.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct StagePlacementDiagnostic {
    pub code: StagePlacementDiagnosticCode,
    pub severity: StagePlacementDiagnosticSeverity,
    pub message: String,
}

/// Stable placement diagnostic code.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagePlacementDiagnosticCode {
    MixedAbsoluteAndAnchor,
    MissingSize,
    ConflictingFitAndScale,
    IndependentAxisScaleRejected,
    ObjectExceedsViewport,
    ObjectExceedsSafeArea,
    EmptyViewport,
    NonFiniteGeometry,
}

/// Placement diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StagePlacementDiagnosticSeverity {
    Error,
    Warning,
}

/// Fatal placement resolution error.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum StagePlacementError {
    #[error("stage placement viewport must have finite positive dimensions")]
    EmptyViewport,
    #[error("stage placement size must be non-zero")]
    MissingSize,
    #[error("stage placement geometry is not finite")]
    NonFiniteGeometry,
    #[error(transparent)]
    Layout(#[from] LayoutError),
}

impl StagePlacement {
    pub const fn absolute(rect: StageRect) -> Self {
        Self::Absolute { rect }
    }

    pub const fn anchor(anchor: StageAnchor, object_anchor: StageAnchor, size: StageSize) -> Self {
        Self::Anchor {
            anchor,
            object_anchor,
            margins: StageInsets::zero(),
            size,
            scale: StageScalePolicy::Design,
            safe_area: false,
            baseline: None,
        }
    }

    #[must_use]
    pub fn with_margins(mut self, margins: StageInsets) -> Self {
        if let Self::Anchor {
            margins: target, ..
        } = &mut self
        {
            *target = margins;
        }
        self
    }

    #[must_use]
    pub fn with_scale_policy(mut self, scale: StageScalePolicy) -> Self {
        if let Self::Anchor { scale: target, .. } = &mut self {
            *target = scale;
        }
        self
    }

    #[must_use]
    pub fn with_safe_area(mut self, safe_area: bool) -> Self {
        if let Self::Anchor {
            safe_area: target, ..
        } = &mut self
        {
            *target = safe_area;
        }
        self
    }

    #[must_use]
    pub fn with_baseline(mut self, baseline: StageBaseline) -> Self {
        if let Self::Anchor {
            baseline: target, ..
        } = &mut self
        {
            *target = Some(baseline);
        }
        self
    }

    /// Resolve authored placement into design, output logical, and physical bboxes.
    pub fn resolve(
        &self,
        context: StagePlacementContext,
    ) -> Result<ResolvedStagePlacement, StagePlacementError> {
        context.validate()?;
        match self {
            Self::Absolute { rect } => resolve_absolute(*rect, *self, context),
            Self::Anchor { .. } => resolve_anchor(*self, context),
        }
    }
}

impl StageRect {
    pub const fn new(x_milli: i32, y_milli: i32, width_milli: u32, height_milli: u32) -> Self {
        Self {
            x_milli,
            y_milli,
            width_milli,
            height_milli,
        }
    }

    pub fn to_layout_rect(self) -> LayoutRect {
        LayoutRect::from_xywh(
            milli_i32_to_f32(self.x_milli),
            milli_i32_to_f32(self.y_milli),
            milli_u32_to_f32(self.width_milli),
            milli_u32_to_f32(self.height_milli),
        )
    }

    pub const fn is_empty(self) -> bool {
        self.width_milli == 0 || self.height_milli == 0
    }
}

impl StageSize {
    pub const fn new(width_milli: u32, height_milli: u32) -> Self {
        Self {
            width_milli,
            height_milli,
        }
    }

    pub fn to_layout_size(self) -> LayoutSize {
        LayoutSize::new(
            milli_u32_to_f32(self.width_milli),
            milli_u32_to_f32(self.height_milli),
        )
    }

    pub const fn is_empty(self) -> bool {
        self.width_milli == 0 || self.height_milli == 0
    }
}

impl StageInsets {
    pub const fn zero() -> Self {
        Self {
            top_milli: 0,
            right_milli: 0,
            bottom_milli: 0,
            left_milli: 0,
        }
    }

    pub const fn new(top_milli: i32, right_milli: i32, bottom_milli: i32, left_milli: i32) -> Self {
        Self {
            top_milli,
            right_milli,
            bottom_milli,
            left_milli,
        }
    }

    fn left(self) -> f32 {
        milli_i32_to_f32(self.left_milli)
    }

    fn right(self) -> f32 {
        milli_i32_to_f32(self.right_milli)
    }

    fn top(self) -> f32 {
        milli_i32_to_f32(self.top_milli)
    }

    fn bottom(self) -> f32 {
        milli_i32_to_f32(self.bottom_milli)
    }
}

impl StageAnchor {
    pub fn from_keyword(value: &str) -> Option<Self> {
        match value {
            "top_left" | "left_top" => Some(Self::TopLeft),
            "top_center" | "center_top" => Some(Self::TopCenter),
            "top_right" | "right_top" => Some(Self::TopRight),
            "center_left" | "left_center" => Some(Self::CenterLeft),
            "center" | "middle" => Some(Self::Center),
            "center_right" | "right_center" => Some(Self::CenterRight),
            "bottom_left" | "left_bottom" => Some(Self::BottomLeft),
            "bottom_center" | "center_bottom" => Some(Self::BottomCenter),
            "bottom_right" | "right_bottom" => Some(Self::BottomRight),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TopLeft => "top_left",
            Self::TopCenter => "top_center",
            Self::TopRight => "top_right",
            Self::CenterLeft => "center_left",
            Self::Center => "center",
            Self::CenterRight => "center_right",
            Self::BottomLeft => "bottom_left",
            Self::BottomCenter => "bottom_center",
            Self::BottomRight => "bottom_right",
            Self::Custom { .. } => "custom",
        }
    }

    pub fn factor_x(self) -> f32 {
        match self {
            Self::TopLeft | Self::CenterLeft | Self::BottomLeft => 0.0,
            Self::TopCenter | Self::Center | Self::BottomCenter => 0.5,
            Self::TopRight | Self::CenterRight | Self::BottomRight => 1.0,
            Self::Custom { x_milli, .. } => milli_i32_to_f32(x_milli).clamp(0.0, 1.0),
        }
    }

    pub fn factor_y(self) -> f32 {
        match self {
            Self::TopLeft | Self::TopCenter | Self::TopRight => 0.0,
            Self::CenterLeft | Self::Center | Self::CenterRight => 0.5,
            Self::BottomLeft | Self::BottomCenter | Self::BottomRight => 1.0,
            Self::Custom { y_milli, .. } => milli_i32_to_f32(y_milli).clamp(0.0, 1.0),
        }
    }
}

impl StageScalePolicy {
    pub fn from_keyword(value: &str) -> Option<Self> {
        match value {
            "design" | "design_uniform" => Some(Self::Design),
            "fixed_output" | "output" | "none" => Some(Self::FixedOutput),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Design => "design",
            Self::FixedOutput => "fixed_output",
        }
    }
}

impl StagePlacementContext {
    pub const fn new(design_viewport: LayoutSize, output_viewport: LayoutSize) -> Self {
        Self {
            design_viewport,
            output_viewport,
            physical_viewport: output_viewport,
            scale_factor: 1.0,
            safe_area: SafeAreaInsets {
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
                left: 0.0,
            },
            viewport_policy: ScalePolicy::Contain,
        }
    }

    #[must_use]
    pub const fn with_physical_viewport(mut self, physical_viewport: LayoutSize) -> Self {
        self.physical_viewport = physical_viewport;
        self
    }

    #[must_use]
    pub const fn with_scale_factor(mut self, scale_factor: f32) -> Self {
        self.scale_factor = scale_factor;
        self
    }

    #[must_use]
    pub const fn with_safe_area(mut self, safe_area: SafeAreaInsets) -> Self {
        self.safe_area = safe_area;
        self
    }

    #[must_use]
    pub const fn with_viewport_policy(mut self, viewport_policy: ScalePolicy) -> Self {
        self.viewport_policy = viewport_policy;
        self
    }

    fn validate(self) -> Result<(), StagePlacementError> {
        if !self.design_viewport.is_positive()
            || !self.output_viewport.is_positive()
            || !self.physical_viewport.is_positive()
        {
            return Err(StagePlacementError::EmptyViewport);
        }
        Ok(())
    }

    fn physical_scale_x(self) -> f32 {
        self.physical_viewport.width / self.output_viewport.width
    }

    fn physical_scale_y(self) -> f32 {
        self.physical_viewport.height / self.output_viewport.height
    }
}

impl ResolvedStagePlacement {
    pub const fn resolved_output_bbox(&self) -> LayoutRect {
        self.output_bbox
    }
}

impl StagePlacementDiagnostic {
    pub fn warning(code: StagePlacementDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: StagePlacementDiagnosticSeverity::Warning,
            message: message.into(),
        }
    }

    pub fn error(code: StagePlacementDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: StagePlacementDiagnosticSeverity::Error,
            message: message.into(),
        }
    }
}

fn resolve_absolute(
    rect: StageRect,
    authored: StagePlacement,
    context: StagePlacementContext,
) -> Result<ResolvedStagePlacement, StagePlacementError> {
    if rect.is_empty() {
        return Err(StagePlacementError::MissingSize);
    }
    let output_bbox = rect.to_layout_rect();
    ensure_finite(output_bbox)?;
    let raw = ContentRect::calculate(
        context.output_viewport,
        context.output_viewport,
        ScalePolicy::Raw,
    )?;
    let design_bbox = output_bbox;
    let physical_bbox = output_to_physical(output_bbox, context);
    let mut diagnostics = Vec::new();
    push_exceeds_viewport_diagnostic(&mut diagnostics, output_bbox, context.output_viewport);
    Ok(ResolvedStagePlacement {
        authored,
        authored_space: LayoutCoordinateSpace::Output,
        design_bbox,
        output_bbox,
        physical_bbox,
        fit_transform: raw
            .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output),
        diagnostics,
    })
}

fn resolve_anchor(
    authored: StagePlacement,
    placement_context: StagePlacementContext,
) -> Result<ResolvedStagePlacement, StagePlacementError> {
    let StagePlacement::Anchor {
        anchor,
        object_anchor,
        margins,
        size,
        scale,
        safe_area,
        baseline: _,
    } = authored
    else {
        unreachable!("resolve_anchor is only called for authored anchor placement");
    };
    if size.is_empty() {
        return Err(StagePlacementError::MissingSize);
    }
    let viewport_fit = ContentRect::calculate(
        placement_context.design_viewport,
        placement_context.output_viewport,
        placement_context.viewport_policy,
    )?;
    if placement_context.viewport_policy == ScalePolicy::Stretch
        && scale == StageScalePolicy::Design
    {
        return Ok(resolved_with_error(
            authored,
            placement_context,
            viewport_fit,
            StagePlacementDiagnostic::error(
                StagePlacementDiagnosticCode::ConflictingFitAndScale,
                "responsive stage placement rejects non-uniform viewport stretch",
            ),
        ));
    }

    let (design_bbox, output_bbox, authored_space, safe_output_rect) = match scale {
        StageScalePolicy::Design => {
            let available = design_available_rect(placement_context, viewport_fit, safe_area);
            let inner = inset_rect(available, margins);
            let design_bbox = anchored_rect(inner, anchor, object_anchor, size.to_layout_size());
            (
                design_bbox,
                viewport_fit.map_rect(design_bbox),
                LayoutCoordinateSpace::Design,
                None,
            )
        }
        StageScalePolicy::FixedOutput => {
            let available = output_available_rect(placement_context, safe_area);
            let inner = inset_rect(available, margins);
            let output_bbox = anchored_rect(inner, anchor, object_anchor, size.to_layout_size());
            (
                viewport_fit.unmap_rect(output_bbox),
                output_bbox,
                LayoutCoordinateSpace::Output,
                safe_area.then_some(available),
            )
        }
    };
    ensure_finite(design_bbox)?;
    ensure_finite(output_bbox)?;

    let physical_bbox = output_to_physical(output_bbox, placement_context);
    let mut diagnostics = Vec::new();
    push_exceeds_viewport_diagnostic(
        &mut diagnostics,
        output_bbox,
        placement_context.output_viewport,
    );
    if safe_area {
        let safe_rect = safe_output_rect.unwrap_or_else(|| {
            viewport_fit.map_rect(design_available_rect(placement_context, viewport_fit, true))
        });
        push_exceeds_safe_area_diagnostic(&mut diagnostics, output_bbox, safe_rect);
    }

    Ok(ResolvedStagePlacement {
        authored,
        authored_space,
        design_bbox,
        output_bbox,
        physical_bbox,
        fit_transform: viewport_fit
            .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output),
        diagnostics,
    })
}

fn resolved_with_error(
    authored: StagePlacement,
    placement_context: StagePlacementContext,
    viewport_fit: ContentRect,
    diagnostic: StagePlacementDiagnostic,
) -> ResolvedStagePlacement {
    let zero = LayoutRect::from_xywh(0.0, 0.0, 0.0, 0.0);
    ResolvedStagePlacement {
        authored,
        authored_space: LayoutCoordinateSpace::Design,
        design_bbox: zero,
        output_bbox: zero,
        physical_bbox: zero,
        fit_transform: viewport_fit
            .fit_transform_metadata(LayoutCoordinateSpace::Output, LayoutCoordinateSpace::Output),
        diagnostics: vec![
            diagnostic,
            StagePlacementDiagnostic::error(
                StagePlacementDiagnosticCode::EmptyViewport,
                format!(
                    "output viewport was {}x{} while rejecting placement",
                    placement_context.output_viewport.width,
                    placement_context.output_viewport.height
                ),
            ),
        ],
    }
}

fn design_available_rect(
    placement_context: StagePlacementContext,
    viewport_fit: ContentRect,
    safe_area: bool,
) -> LayoutRect {
    let design = LayoutRect::from_xywh(
        0.0,
        0.0,
        placement_context.design_viewport.width,
        placement_context.design_viewport.height,
    );
    if !safe_area {
        return design;
    }
    let safe = placement_context.safe_area;
    let insets = StageInsets::new(
        f32_to_i32_milli(safe.top / viewport_fit.scale_y),
        f32_to_i32_milli(safe.right / viewport_fit.scale_x),
        f32_to_i32_milli(safe.bottom / viewport_fit.scale_y),
        f32_to_i32_milli(safe.left / viewport_fit.scale_x),
    );
    inset_rect(design, insets)
}

fn output_available_rect(context: StagePlacementContext, safe_area: bool) -> LayoutRect {
    let output = LayoutRect::from_xywh(
        0.0,
        0.0,
        context.output_viewport.width,
        context.output_viewport.height,
    );
    if !safe_area {
        return output;
    }
    let safe = context.safe_area;
    inset_rect(
        output,
        StageInsets::new(
            f32_to_i32_milli(safe.top),
            f32_to_i32_milli(safe.right),
            f32_to_i32_milli(safe.bottom),
            f32_to_i32_milli(safe.left),
        ),
    )
}

fn anchored_rect(
    available: LayoutRect,
    anchor: StageAnchor,
    object_anchor: StageAnchor,
    size: LayoutSize,
) -> LayoutRect {
    let point = LayoutPoint::new(
        available.origin.x + available.size.width * anchor.factor_x(),
        available.origin.y + available.size.height * anchor.factor_y(),
    );
    LayoutRect::from_xywh(
        point.x - size.width * object_anchor.factor_x(),
        point.y - size.height * object_anchor.factor_y(),
        size.width,
        size.height,
    )
}

fn inset_rect(rect: LayoutRect, insets: StageInsets) -> LayoutRect {
    let x = rect.origin.x + insets.left();
    let y = rect.origin.y + insets.top();
    let width = (rect.size.width - insets.left() - insets.right()).max(0.0);
    let height = (rect.size.height - insets.top() - insets.bottom()).max(0.0);
    LayoutRect::from_xywh(x, y, width, height)
}

fn output_to_physical(rect: LayoutRect, context: StagePlacementContext) -> LayoutRect {
    LayoutRect::from_xywh(
        rect.origin.x * context.physical_scale_x(),
        rect.origin.y * context.physical_scale_y(),
        rect.size.width * context.physical_scale_x(),
        rect.size.height * context.physical_scale_y(),
    )
}

fn push_exceeds_viewport_diagnostic(
    diagnostics: &mut Vec<StagePlacementDiagnostic>,
    rect: LayoutRect,
    viewport: LayoutSize,
) {
    if rect.origin.x < -DEFAULT_EPSILON
        || rect.origin.y < -DEFAULT_EPSILON
        || rect.right() > viewport.width + DEFAULT_EPSILON
        || rect.bottom() > viewport.height + DEFAULT_EPSILON
    {
        diagnostics.push(StagePlacementDiagnostic::warning(
            StagePlacementDiagnosticCode::ObjectExceedsViewport,
            "resolved stage object exceeds output viewport",
        ));
    }
}

fn push_exceeds_safe_area_diagnostic(
    diagnostics: &mut Vec<StagePlacementDiagnostic>,
    rect: LayoutRect,
    safe_rect: LayoutRect,
) {
    if rect.origin.x < safe_rect.origin.x - DEFAULT_EPSILON
        || rect.origin.y < safe_rect.origin.y - DEFAULT_EPSILON
        || rect.right() > safe_rect.right() + DEFAULT_EPSILON
        || rect.bottom() > safe_rect.bottom() + DEFAULT_EPSILON
    {
        diagnostics.push(StagePlacementDiagnostic::warning(
            StagePlacementDiagnosticCode::ObjectExceedsSafeArea,
            "resolved stage object exceeds safe area",
        ));
    }
}

fn ensure_finite(rect: LayoutRect) -> Result<(), StagePlacementError> {
    if rect.origin.x.is_finite()
        && rect.origin.y.is_finite()
        && rect.size.width.is_finite()
        && rect.size.height.is_finite()
    {
        Ok(())
    } else {
        Err(StagePlacementError::NonFiniteGeometry)
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or(0.0) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}

fn f32_to_i32_milli(value: f32) -> i32 {
    let milli = f64::from(value) * 1_000.0;
    let rounded = milli.round();
    if rounded.is_finite() {
        rounded
            .clamp(f64::from(i32::MIN), f64::from(i32::MAX))
            .to_i32()
            .unwrap_or(0)
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn standing_top_right() -> StagePlacement {
        StagePlacement::anchor(
            StageAnchor::TopRight,
            StageAnchor::TopRight,
            StageSize::new(250_000, 430_000),
        )
        .with_margins(StageInsets::new(20_000, 100_000, 0, 0))
    }

    fn context(width: f32, height: f32) -> StagePlacementContext {
        StagePlacementContext::new(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(width, height),
        )
    }

    fn rect_milli(rect: LayoutRect) -> (i32, i32, u32, u32) {
        (
            f32_to_i32_milli(rect.origin.x),
            f32_to_i32_milli(rect.origin.y),
            u32::try_from(f32_to_i32_milli(rect.size.width)).unwrap(),
            u32::try_from(f32_to_i32_milli(rect.size.height)).unwrap(),
        )
    }

    fn assert_rect_milli_eq(rect: LayoutRect, expected: (i32, i32, u32, u32)) {
        assert_eq!(rect_milli(rect), expected);
    }

    #[test]
    fn top_right_anchor_keeps_design_relation_across_viewports() {
        let cases = [
            (1280.0, 720.0, (930_000, 20_000, 250_000, 430_000)),
            (1920.0, 1080.0, (1_395_000, 30_000, 375_000, 645_000)),
            (2560.0, 1440.0, (1_860_000, 40_000, 500_000, 860_000)),
        ];
        for (width, height, expected) in cases {
            let resolved = standing_top_right()
                .resolve(context(width, height))
                .unwrap();
            assert_rect_milli_eq(resolved.output_bbox, expected);
            assert!(resolved.diagnostics.is_empty());
        }
    }

    #[test]
    fn absolute_mode_does_not_scale_to_larger_output() {
        let resolved = StagePlacement::absolute(StageRect::new(930_000, 20_000, 250_000, 430_000))
            .resolve(context(1920.0, 1080.0))
            .unwrap();
        assert_rect_milli_eq(resolved.output_bbox, (930_000, 20_000, 250_000, 430_000));
        assert_eq!(resolved.authored_space, LayoutCoordinateSpace::Output);
    }

    #[test]
    fn high_dpi_changes_physical_not_logical_bbox() {
        let resolved = standing_top_right()
            .resolve(
                context(1920.0, 1080.0)
                    .with_physical_viewport(LayoutSize::new(3840.0, 2160.0))
                    .with_scale_factor(2.0),
            )
            .unwrap();
        assert_rect_milli_eq(resolved.output_bbox, (1_395_000, 30_000, 375_000, 645_000));
        assert_rect_milli_eq(
            resolved.physical_bbox,
            (2_790_000, 60_000, 750_000, 1_290_000),
        );
    }

    #[test]
    fn safe_area_excess_is_reported_as_diagnostic() {
        let resolved = StagePlacement::anchor(
            StageAnchor::TopRight,
            StageAnchor::TopRight,
            StageSize::new(1_300_000, 430_000),
        )
        .with_margins(StageInsets::new(20_000, 0, 0, 0))
        .with_safe_area(true)
        .resolve(context(1280.0, 720.0).with_safe_area(SafeAreaInsets {
            top: 0.0,
            right: 80.0,
            bottom: 0.0,
            left: 0.0,
        }))
        .unwrap();
        assert!(resolved.diagnostics.iter().any(
            |diagnostic| diagnostic.code == StagePlacementDiagnosticCode::ObjectExceedsSafeArea
        ));
    }
}
