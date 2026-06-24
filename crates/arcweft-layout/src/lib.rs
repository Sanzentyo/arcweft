//! Sans I/O layout contracts shared by renderers, capture adapters, and Agent observation.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Two-dimensional layout size in design-space logical pixels.
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
    FitTextReachedMinimum,
    ExpandBoxConstrained,
    GlyphMetricFallback,
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
            && point.x <= self.origin.x + self.size.width
            && point.y <= self.origin.y + self.size.height
    }

    #[must_use]
    pub fn clipped_to(self, viewport: LayoutSize) -> Self {
        let x0 = self.origin.x.max(0.0);
        let y0 = self.origin.y.max(0.0);
        let x1 = (self.origin.x + self.size.width)
            .min(viewport.width)
            .max(x0);
        let y1 = (self.origin.y + self.size.height)
            .min(viewport.height)
            .max(y0);
        Self::new(x0, y0, x1 - x0, y1 - y0)
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
}

#[cfg(test)]
mod tests {
    use super::{
        ContentRect, LayoutEvaluationContext, LayoutLengthExpr, LayoutPoint, LayoutRect,
        LayoutSize, LayoutUnit, SafeAreaInsets, ScalePolicy,
    };

    #[test]
    fn contain_maps_1000_by_800_to_letterboxed_content_rect() {
        let rect = ContentRect::calculate(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1000.0, 800.0),
            ScalePolicy::Contain,
        )
        .expect("content rect");
        assert_eq!(rect.rect.origin, LayoutPoint::new(0.0, 118.75));
        assert!((rect.rect.size.width - 1_000.0).abs() < f32::EPSILON);
        assert!((rect.rect.size.height - 562.5).abs() < f32::EPSILON);
    }

    #[test]
    fn cover_maps_1000_by_800_to_signed_crop_rect() {
        let rect = ContentRect::calculate(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1000.0, 800.0),
            ScalePolicy::Cover,
        )
        .expect("content rect");
        assert!((rect.rect.origin.x + 211.111_15).abs() < 0.000_01);
        assert!(rect.rect.origin.y.abs() < 0.000_1);
        assert!((rect.rect.size.width - 1_422.222_3).abs() < 0.000_1);
        assert!((rect.rect.size.height - 800.0).abs() < 0.000_1);
    }

    #[test]
    fn inverse_mapping_returns_design_point() {
        let rect = ContentRect::calculate(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1000.0, 800.0),
            ScalePolicy::Contain,
        )
        .expect("content rect");
        let design = LayoutPoint::new(96.0, 48.0);
        let output = rect.map_point(design);
        assert_eq!(rect.unmap_point(output), design);
    }

    #[test]
    fn length_expr_evaluates_against_context() {
        let content_rect = ContentRect::calculate(
            LayoutSize::new(1280.0, 720.0),
            LayoutSize::new(1000.0, 800.0),
            ScalePolicy::Contain,
        )
        .expect("content rect");
        let context = LayoutEvaluationContext {
            design_viewport: LayoutSize::new(1280.0, 720.0),
            output_viewport: LayoutSize::new(1000.0, 800.0),
            content_rect,
            containing_box: LayoutSize::new(400.0, 200.0),
            font_size: 16.0,
            glyph_ch: 8.0,
            safe_area: SafeAreaInsets {
                top: 4.0,
                right: 8.0,
                bottom: 12.0,
                left: 16.0,
            },
        };
        let expr = LayoutLengthExpr::Clamp {
            min: Box::new(LayoutLengthExpr::Literal {
                value: 10.0,
                unit: LayoutUnit::Px,
            }),
            value: Box::new(LayoutLengthExpr::Add {
                left: Box::new(LayoutLengthExpr::Literal {
                    value: 50.0,
                    unit: LayoutUnit::Percent,
                }),
                right: Box::new(LayoutLengthExpr::Literal {
                    value: 2.0,
                    unit: LayoutUnit::Em,
                }),
            }),
            max: Box::new(LayoutLengthExpr::Literal {
                value: 80.0,
                unit: LayoutUnit::Px,
            }),
        };
        assert!(
            (expr.evaluate(&context, true).expect("expr evaluates") - 80.0).abs() < f32::EPSILON
        );
    }

    #[test]
    fn clipped_rect_preserves_signed_source_before_clipping() {
        let rect = LayoutRect::new(-10.0, 5.0, 30.0, 50.0);
        assert_eq!(
            rect.clipped_to(LayoutSize::new(100.0, 40.0)),
            LayoutRect::new(0.0, 5.0, 20.0, 35.0)
        );
    }
}
