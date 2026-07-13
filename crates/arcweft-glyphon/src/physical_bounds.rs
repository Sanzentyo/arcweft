//! Checked logical-clip conversion into glyph raster pixel bounds.

use arcweft_text_layout::LayoutRect;
use glyphon::TextBounds;
use thiserror::Error;

/// Logical input component rejected while constructing physical text bounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedTextBoundsComponent {
    Left,
    Top,
    Width,
    Height,
}

/// Physical clip edge rounded for the glyph raster boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedTextBoundsEdge {
    Left,
    Top,
    Right,
    Bottom,
}

/// A checked physical-pixel clip accepted by glyphon's signed scissor domain.
///
/// Construction validates the complete logical-to-physical operation: logical
/// geometry, edge addition, scale multiplication, outward rounding, and the
/// final `i32` coordinate range. Callers never need to clamp or substitute a
/// coordinate after this boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedTextPhysicalBounds {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

/// Structured failure from logical-to-physical prepared-text clip conversion.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum PreparedTextPhysicalBoundsError {
    #[error("prepared-text raster scale `{value}` must be finite and positive")]
    InvalidRasterScale { value: f32 },
    #[error("prepared-text logical clip {component:?} `{value}` must be finite")]
    NonFiniteLogicalValue {
        component: PreparedTextBoundsComponent,
        value: f32,
    },
    #[error("prepared-text logical clip {component:?} `{value}` must not be negative")]
    NegativeLogicalExtent {
        component: PreparedTextBoundsComponent,
        value: f32,
    },
    #[error(
        "prepared-text logical {edge:?} edge overflows from origin `{origin}` and extent `{extent}`"
    )]
    LogicalEdgeOverflow {
        edge: PreparedTextBoundsEdge,
        origin: f32,
        extent: f32,
    },
    #[error(
        "prepared-text logical {edge:?} edge `{logical}` overflows at raster scale `{raster_scale}`"
    )]
    PhysicalScaleOverflow {
        edge: PreparedTextBoundsEdge,
        logical: f32,
        raster_scale: f32,
    },
    #[error(
        "prepared-text physical {edge:?} edge `{physical}` is outside the signed pixel range {minimum}..={maximum}"
    )]
    PixelCoordinateOutOfRange {
        edge: PreparedTextBoundsEdge,
        physical: f32,
        minimum: i32,
        maximum: i32,
    },
}

impl PreparedTextPhysicalBounds {
    /// Converts one logical clip and raster scale into checked outward-rounded
    /// physical bounds.
    pub fn try_from_logical(
        clip: LayoutRect,
        raster_scale: f32,
    ) -> Result<Self, PreparedTextPhysicalBoundsError> {
        validate_logical_component(PreparedTextBoundsComponent::Left, clip.x)?;
        validate_logical_component(PreparedTextBoundsComponent::Top, clip.y)?;
        validate_logical_component(PreparedTextBoundsComponent::Width, clip.width)?;
        validate_logical_component(PreparedTextBoundsComponent::Height, clip.height)?;
        if clip.width < 0.0 {
            return Err(PreparedTextPhysicalBoundsError::NegativeLogicalExtent {
                component: PreparedTextBoundsComponent::Width,
                value: clip.width,
            });
        }
        if clip.height < 0.0 {
            return Err(PreparedTextPhysicalBoundsError::NegativeLogicalExtent {
                component: PreparedTextBoundsComponent::Height,
                value: clip.height,
            });
        }
        if !raster_scale.is_finite() || raster_scale <= 0.0 {
            return Err(PreparedTextPhysicalBoundsError::InvalidRasterScale {
                value: raster_scale,
            });
        }

        let right = logical_far_edge(PreparedTextBoundsEdge::Right, clip.x, clip.width)?;
        let bottom = logical_far_edge(PreparedTextBoundsEdge::Bottom, clip.y, clip.height)?;
        Ok(Self {
            left: checked_pixel_edge(PreparedTextBoundsEdge::Left, clip.x, raster_scale)?,
            top: checked_pixel_edge(PreparedTextBoundsEdge::Top, clip.y, raster_scale)?,
            right: checked_pixel_edge(PreparedTextBoundsEdge::Right, right, raster_scale)?,
            bottom: checked_pixel_edge(PreparedTextBoundsEdge::Bottom, bottom, raster_scale)?,
        })
    }

    #[must_use]
    pub const fn left(self) -> i32 {
        self.left
    }

    #[must_use]
    pub const fn top(self) -> i32 {
        self.top
    }

    #[must_use]
    pub const fn right(self) -> i32 {
        self.right
    }

    #[must_use]
    pub const fn bottom(self) -> i32 {
        self.bottom
    }
}

impl From<PreparedTextPhysicalBounds> for TextBounds {
    fn from(bounds: PreparedTextPhysicalBounds) -> Self {
        Self {
            left: bounds.left,
            top: bounds.top,
            right: bounds.right,
            bottom: bounds.bottom,
        }
    }
}

fn validate_logical_component(
    component: PreparedTextBoundsComponent,
    value: f32,
) -> Result<(), PreparedTextPhysicalBoundsError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(PreparedTextPhysicalBoundsError::NonFiniteLogicalValue { component, value })
    }
}

fn logical_far_edge(
    edge: PreparedTextBoundsEdge,
    origin: f32,
    extent: f32,
) -> Result<f32, PreparedTextPhysicalBoundsError> {
    let value = origin + extent;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(PreparedTextPhysicalBoundsError::LogicalEdgeOverflow {
            edge,
            origin,
            extent,
        })
    }
}

fn checked_pixel_edge(
    edge: PreparedTextBoundsEdge,
    logical: f32,
    raster_scale: f32,
) -> Result<i32, PreparedTextPhysicalBoundsError> {
    let physical = logical * raster_scale;
    if !physical.is_finite() {
        return Err(PreparedTextPhysicalBoundsError::PhysicalScaleOverflow {
            edge,
            logical,
            raster_scale,
        });
    }
    let rounded = match edge {
        PreparedTextBoundsEdge::Left | PreparedTextBoundsEdge::Top => physical.floor(),
        PreparedTextBoundsEdge::Right | PreparedTextBoundsEdge::Bottom => physical.ceil(),
    };
    let rounded_f64 = f64::from(rounded);
    if !(f64::from(i32::MIN)..=f64::from(i32::MAX)).contains(&rounded_f64) {
        return Err(PreparedTextPhysicalBoundsError::PixelCoordinateOutOfRange {
            edge,
            physical,
            minimum: i32::MIN,
            maximum: i32::MAX,
        });
    }

    #[allow(
        clippy::cast_possible_truncation,
        reason = "outward-rounded value was proven finite and inside the i32 pixel domain"
    )]
    Ok(rounded as i32)
}

#[cfg(test)]
mod tests {
    use arcweft_text_layout::LayoutRect;

    use super::{
        PreparedTextBoundsComponent, PreparedTextBoundsEdge, PreparedTextPhysicalBounds,
        PreparedTextPhysicalBoundsError,
    };

    #[test]
    fn accepts_largest_f32_pixel_below_the_signed_upper_boundary() {
        // `i32::MAX` rounds up to 2^31 as f32. The preceding f32 is the
        // largest exactly representable coordinate accepted by the i32 API.
        let maximum_representable_pixel = 2_147_483_520.0_f32;

        let bounds = PreparedTextPhysicalBounds::try_from_logical(
            LayoutRect::new(maximum_representable_pixel, -2_147_483_648.0, 0.0, 0.0),
            1.0,
        )
        .expect("largest representable signed pixel bounds");

        assert_eq!(bounds.left(), 2_147_483_520);
        assert_eq!(bounds.right(), 2_147_483_520);
        assert_eq!(bounds.top(), i32::MIN);
        assert_eq!(bounds.bottom(), i32::MIN);
    }

    #[test]
    fn rounds_outward_only_after_checked_physical_scaling() {
        let bounds = PreparedTextPhysicalBounds::try_from_logical(
            LayoutRect::new(-1.25, 2.25, 2.5, 3.5),
            2.0,
        )
        .expect("finite physical bounds");

        assert_eq!((bounds.left(), bounds.top()), (-3, 4));
        assert_eq!((bounds.right(), bounds.bottom()), (3, 12));
    }

    #[test]
    fn rejects_positive_and_negative_scale_multiplication_overflow() {
        for (logical, edge) in [
            (f32::MAX, PreparedTextBoundsEdge::Left),
            (f32::MIN, PreparedTextBoundsEdge::Left),
        ] {
            assert!(matches!(
                PreparedTextPhysicalBounds::try_from_logical(
                    LayoutRect::new(logical, 0.0, 0.0, 0.0),
                    2.0,
                ),
                Err(PreparedTextPhysicalBoundsError::PhysicalScaleOverflow {
                    edge: actual,
                    ..
                }) if actual == edge
            ));
        }
    }

    #[test]
    fn rejects_finite_physical_values_outside_the_signed_pixel_domain() {
        assert!(matches!(
            PreparedTextPhysicalBounds::try_from_logical(
                LayoutRect::new(2_147_483_648.0, 0.0, 0.0, 0.0),
                1.0,
            ),
            Err(PreparedTextPhysicalBoundsError::PixelCoordinateOutOfRange {
                edge: PreparedTextBoundsEdge::Left,
                minimum: i32::MIN,
                maximum: i32::MAX,
                ..
            })
        ));
    }

    #[test]
    fn validates_extreme_and_invalid_scales_without_fallback() {
        let tiny = PreparedTextPhysicalBounds::try_from_logical(
            LayoutRect::new(-1.0, 1.0, 2.0, 2.0),
            f32::MIN_POSITIVE,
        )
        .expect("smallest positive normal scale remains finite");
        assert_eq!(
            (tiny.left(), tiny.top(), tiny.right(), tiny.bottom()),
            (-1, 0, 1, 1)
        );

        assert!(matches!(
            PreparedTextPhysicalBounds::try_from_logical(
                LayoutRect::new(2.0, 0.0, 0.0, 0.0),
                f32::MAX,
            ),
            Err(PreparedTextPhysicalBoundsError::PhysicalScaleOverflow {
                edge: PreparedTextBoundsEdge::Left,
                ..
            })
        ));
        for scale in [0.0, -0.0, -1.0, f32::NAN, f32::INFINITY] {
            assert!(matches!(
                PreparedTextPhysicalBounds::try_from_logical(
                    LayoutRect::new(0.0, 0.0, 1.0, 1.0),
                    scale,
                ),
                Err(PreparedTextPhysicalBoundsError::InvalidRasterScale { .. })
            ));
        }
    }

    #[test]
    fn rejects_non_finite_logical_values_and_far_edge_overflow() {
        for (clip, component) in [
            (
                LayoutRect::new(f32::NAN, 0.0, 1.0, 1.0),
                PreparedTextBoundsComponent::Left,
            ),
            (
                LayoutRect::new(0.0, f32::INFINITY, 1.0, 1.0),
                PreparedTextBoundsComponent::Top,
            ),
            (
                LayoutRect::new(0.0, 0.0, f32::NEG_INFINITY, 1.0),
                PreparedTextBoundsComponent::Width,
            ),
        ] {
            assert!(matches!(
                PreparedTextPhysicalBounds::try_from_logical(clip, 1.0),
                Err(PreparedTextPhysicalBoundsError::NonFiniteLogicalValue {
                    component: actual,
                    ..
                }) if actual == component
            ));
        }

        assert!(matches!(
            PreparedTextPhysicalBounds::try_from_logical(
                LayoutRect::new(f32::MAX, 0.0, f32::MAX, 0.0),
                1.0,
            ),
            Err(PreparedTextPhysicalBoundsError::LogicalEdgeOverflow {
                edge: PreparedTextBoundsEdge::Right,
                ..
            })
        ));
    }
}
