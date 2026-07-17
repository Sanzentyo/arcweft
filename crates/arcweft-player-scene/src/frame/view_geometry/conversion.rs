//! Checked host and renderer geometry conversion.

use arcweft_view::geometry::ViewGeometryConsumer;
use arcweft_view::geometry::{
    ViewFinalGeometry, ViewGeometryRect, ViewViewportGeometryInput, ViewViewportGeometryRevision,
};
use arcweft_view::style::ViewStyleNodeKey;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryPlatform {
    Native,
    Web,
    Headless,
    Wgpu,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryConversionField {
    ViewportLeft,
    ViewportTop,
    ViewportWidth,
    ViewportHeight,
    Left,
    Top,
    Right,
    Bottom,
    Width,
    Height,
    Clip,
    Scale,
    Raster,
    IndexRange,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewGeometryConversionError {
    #[error("non-finite {field:?} for {consumer:?} on {platform:?}")]
    NonFiniteInput {
        node: Option<ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value_bits: u64,
    },
    #[error(
        "{field:?} is outside {min_milli}..={max_milli} milli for {consumer:?} on {platform:?}"
    )]
    OutsideMilliRange {
        node: Option<ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value_bits: u64,
        min_milli: i64,
        max_milli: i64,
    },
    #[error("negative extent {value_milli} for {field:?} and {consumer:?} on {platform:?}")]
    NegativeExtent {
        node: Option<ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value_milli: i64,
    },
    #[error(
        "milli value {value_milli} is not exactly representable for {consumer:?} on {platform:?}"
    )]
    InexactF32 {
        node: Option<ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value_milli: i64,
        round_trip_milli: i64,
    },
    #[error("value {value} exceeds target index maximum {max} for {consumer:?} on {platform:?}")]
    IndexRange {
        node: Option<ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value: u64,
        max: u64,
    },
}

impl ViewGeometryConversionError {
    pub fn scale_factor(platform: ViewGeometryPlatform, value: f64) -> Result<f32, Self> {
        if !value.is_finite() {
            return Err(Self::NonFiniteInput {
                node: None,
                platform,
                consumer: ViewGeometryConsumer::Layout,
                field: ViewGeometryConversionField::Scale,
                value_bits: value.to_bits(),
            });
        }
        if value <= 0.0 || value > f64::from(f32::MAX) {
            return Err(Self::OutsideMilliRange {
                node: None,
                platform,
                consumer: ViewGeometryConsumer::Layout,
                field: ViewGeometryConversionField::Scale,
                value_bits: value.to_bits(),
                min_milli: 1,
                max_milli: i64::from(i32::MAX),
            });
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the finite positive f32 range is checked before host scale conversion"
        )]
        Ok(value as f32)
    }

    pub fn logical_pointer(
        platform: ViewGeometryPlatform,
        field: ViewGeometryConversionField,
        value: f64,
    ) -> Result<f32, Self> {
        if !value.is_finite() {
            return Err(Self::NonFiniteInput {
                node: None,
                platform,
                consumer: ViewGeometryConsumer::HitTest,
                field,
                value_bits: value.to_bits(),
            });
        }
        let scaled = value * 1_000.0;
        if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
            return Err(Self::OutsideMilliRange {
                node: None,
                platform,
                consumer: ViewGeometryConsumer::HitTest,
                field,
                value_bits: value.to_bits(),
                min_milli: i64::from(i32::MIN),
                max_milli: i64::from(i32::MAX),
            });
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the complete finite i32 milli range is checked before flooring"
        )]
        let value_milli = scaled.floor() as i64;
        Self::exact_f32(
            None,
            platform,
            ViewGeometryConsumer::HitTest,
            field,
            value_milli,
        )
    }

    pub fn viewport_input(
        platform: ViewGeometryPlatform,
        width: f64,
        height: f64,
    ) -> Result<ViewViewportGeometryInput, Self> {
        let right = viewport_edge(platform, width, ViewGeometryConversionField::ViewportWidth)?;
        let bottom = viewport_edge(
            platform,
            height,
            ViewGeometryConversionField::ViewportHeight,
        )?;
        if right < 0 {
            return Err(negative_viewport(
                platform,
                ViewGeometryConversionField::ViewportWidth,
                i64::from(right),
            ));
        }
        if bottom < 0 {
            return Err(negative_viewport(
                platform,
                ViewGeometryConversionField::ViewportHeight,
                i64::from(bottom),
            ));
        }
        let rect = ViewGeometryRect {
            left_milli: 0,
            top_milli: 0,
            right_milli: right,
            bottom_milli: bottom,
        };
        let mut revision = 0xcbf2_9ce4_8422_2325_u64;
        for byte in right.to_le_bytes().into_iter().chain(bottom.to_le_bytes()) {
            revision ^= u64::from(byte);
            revision = revision.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Ok(ViewViewportGeometryInput {
            rect,
            revision: ViewViewportGeometryRevision::new(revision),
        })
    }

    pub fn exact_f32(
        node: Option<&ViewStyleNodeKey>,
        platform: ViewGeometryPlatform,
        consumer: ViewGeometryConsumer,
        field: ViewGeometryConversionField,
        value_milli: i64,
    ) -> Result<f32, Self> {
        #[expect(
            clippy::cast_precision_loss,
            reason = "the authoritative milli domain is bounded to i32 endpoints or checked u32 design extents, and the later round-trip rejects inexact f32 values"
        )]
        let value = value_milli as f64 / 1_000.0;
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the round-trip check below rejects every inexact authoritative conversion"
        )]
        let converted = value as f32;
        if !converted.is_finite() {
            return Err(Self::OutsideMilliRange {
                node: node.cloned(),
                platform,
                consumer,
                field,
                value_bits: value.to_bits(),
                min_milli: i64::from(i32::MIN),
                max_milli: i64::from(i32::MAX),
            });
        }
        #[expect(
            clippy::cast_possible_truncation,
            reason = "finite milli coordinates are rounded before the checked equality comparison"
        )]
        let round_trip_milli = (f64::from(converted) * 1_000.0).round() as i64;
        if round_trip_milli != value_milli {
            return Err(Self::InexactF32 {
                node: node.cloned(),
                platform,
                consumer,
                field,
                value_milli,
                round_trip_milli,
            });
        }
        Ok(converted)
    }
}

pub(crate) fn viewport_input(
    width: f64,
    height: f64,
) -> Result<ViewViewportGeometryInput, ViewGeometryConversionError> {
    ViewGeometryConversionError::viewport_input(ViewGeometryPlatform::Headless, width, height)
}

fn viewport_edge(
    platform: ViewGeometryPlatform,
    value: f64,
    field: ViewGeometryConversionField,
) -> Result<i32, ViewGeometryConversionError> {
    if !value.is_finite() {
        return Err(ViewGeometryConversionError::NonFiniteInput {
            node: None,
            platform,
            consumer: ViewGeometryConsumer::Layout,
            field,
            value_bits: value.to_bits(),
        });
    }
    let scaled = (value * 1_000.0).ceil();
    if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
        return Err(ViewGeometryConversionError::OutsideMilliRange {
            node: None,
            platform,
            consumer: ViewGeometryConsumer::Layout,
            field,
            value_bits: value.to_bits(),
            min_milli: i64::from(i32::MIN),
            max_milli: i64::from(i32::MAX),
        });
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the finite complete i32 range is checked before the integral cast"
    )]
    let milli = scaled as i32;
    Ok(milli)
}

fn negative_viewport(
    platform: ViewGeometryPlatform,
    field: ViewGeometryConversionField,
    value_milli: i64,
) -> ViewGeometryConversionError {
    ViewGeometryConversionError::NegativeExtent {
        node: None,
        platform,
        consumer: ViewGeometryConsumer::Layout,
        field,
        value_milli,
    }
}

pub(crate) fn consumer_hit_rect(
    node: &ViewStyleNodeKey,
    geometry: &ViewFinalGeometry,
    consumer: ViewGeometryConsumer,
) -> Result<Option<arcweft_presentation::hit::HitRect>, ViewGeometryConversionError> {
    let rect = match consumer {
        ViewGeometryConsumer::Paint => geometry.consumers.paint_bounds,
        ViewGeometryConsumer::HitTest => geometry.consumers.hit_bounds,
        ViewGeometryConsumer::Focus => geometry.consumers.focus_target_bounds,
        ViewGeometryConsumer::Avoidance => geometry.consumers.avoidance_bounds,
        ViewGeometryConsumer::Scroll => geometry.consumers.scroll_target_bounds,
        ViewGeometryConsumer::Capture => geometry.consumers.visible_border_box,
        ViewGeometryConsumer::Measure
        | ViewGeometryConsumer::Layout
        | ViewGeometryConsumer::Clip => Some(geometry.world_border_box),
    };
    rect.map(|rect| rect_hit_rect(node, rect, consumer))
        .transpose()
}

pub(crate) fn rect_hit_rect(
    node: &ViewStyleNodeKey,
    rect: ViewGeometryRect,
    consumer: ViewGeometryConsumer,
) -> Result<arcweft_presentation::hit::HitRect, ViewGeometryConversionError> {
    let size = rect.size();
    Ok(arcweft_presentation::hit::HitRect::new(
        exact_f32(
            node,
            consumer,
            ViewGeometryConversionField::Left,
            i64::from(rect.left_milli),
        )?,
        exact_f32(
            node,
            consumer,
            ViewGeometryConversionField::Top,
            i64::from(rect.top_milli),
        )?,
        exact_f32(
            node,
            consumer,
            ViewGeometryConversionField::Width,
            i64::from(size.width_milli),
        )?,
        exact_f32(
            node,
            consumer,
            ViewGeometryConversionField::Height,
            i64::from(size.height_milli),
        )?,
    ))
}

pub(crate) fn exact_f32(
    node: &ViewStyleNodeKey,
    consumer: ViewGeometryConsumer,
    field: ViewGeometryConversionField,
    value_milli: i64,
) -> Result<f32, ViewGeometryConversionError> {
    ViewGeometryConversionError::exact_f32(
        Some(node),
        ViewGeometryPlatform::Wgpu,
        consumer,
        field,
        value_milli,
    )
}
