use super::{ViewGeometryNodeId, ViewGeometryRect};
use crate::style::{ViewPhysicalAxis, ViewPhysicalEdges, ViewPropertyKind};
use thiserror::Error;

/// Geometry consumer requesting an exact physical result.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryConsumer {
    Measure,
    Layout,
    Clip,
    Paint,
    HitTest,
    Focus,
    Avoidance,
    Scroll,
    Capture,
}

/// Geometry behavior represented by Style but not yet executable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewRepresentedGeometryFeature {
    InlineLayout,
    FlexWrap,
    FlexDistribution,
    FlexBasis,
    Order,
    Alignment,
    Rotate,
    NonRectClip,
    Mask,
    PaintEffectBounds,
}

/// Whether a canonical Style property participates in executable geometry.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryPropertySupport {
    Supported,
    RepresentedOnly(ViewRepresentedGeometryFeature),
    NotGeometry,
}

/// Rejects any Style property whose geometry behavior is represented but not executable.
pub fn validate_supported_properties(
    node: &ViewGeometryNodeId,
    consumer: ViewGeometryConsumer,
    properties: &[ViewPropertyKind],
) -> Result<(), ViewGeometryError> {
    for property in properties {
        if let ViewGeometryPropertySupport::RepresentedOnly(feature) = property.geometry_support() {
            return Err(ViewGeometryError::UnsupportedConsumer {
                node: node.clone(),
                consumer,
                property: *property,
                feature,
            });
        }
    }
    Ok(())
}

/// Physical field participating in checked geometry validation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryField {
    Width,
    Height,
    MinWidth,
    MinHeight,
    MaxWidth,
    MaxHeight,
    PaddingTop,
    PaddingRight,
    PaddingBottom,
    PaddingLeft,
    BorderTop,
    BorderRight,
    BorderBottom,
    BorderLeft,
    RowGap,
    ColumnGap,
}

/// Domain operation whose checked arithmetic failed.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryOperation {
    Add,
    Subtract,
    Multiply,
    Translate,
    Scale,
    Outset,
    Inset,
    FlowAdvance,
    Stretch,
    ScrollRange,
    Rasterize,
}

/// Typed reason a platform pointer coordinate cannot enter the milli grid.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewPointerCoordinateErrorKind {
    NonFinite,
    OutsideMilliRange,
}

/// Deterministic physical geometry failure.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewGeometryError {
    #[error("node {node:?} has negative {field:?}: {value_milli}")]
    NegativeNonNegativeField {
        node: ViewGeometryNodeId,
        field: ViewGeometryField,
        value_milli: i32,
    },
    #[error("node {node:?} {axis:?} min {min_milli} exceeds max {max_milli}")]
    ConflictingConstraints {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
        min_milli: u32,
        max_milli: u32,
    },
    #[error("node {node:?} {axis:?} edges {edges_milli} exceed used size {used_milli}")]
    EdgesExceedUsedBorderBox {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
        used_milli: u32,
        edges_milli: u64,
    },
    #[error("node {node:?} arithmetic overflow in {operation:?} on {axis:?}")]
    ArithmeticOverflow {
        node: ViewGeometryNodeId,
        axis: Option<ViewPhysicalAxis>,
        operation: ViewGeometryOperation,
    },
    #[error("inverted geometry span [{start_milli}, {end_milli})")]
    InvertedSpan { start_milli: i32, end_milli: i32 },
    #[error(
        "inverted geometry rectangle [{left_milli}, {top_milli})..[{right_milli}, {bottom_milli})"
    )]
    InvertedRect {
        left_milli: i32,
        top_milli: i32,
        right_milli: i32,
        bottom_milli: i32,
    },
    #[error("node {node:?} margins invert the {axis:?} span")]
    InvertedMarginSpan {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
        border_extent_milli: u32,
        margin_start_milli: i32,
        margin_end_milli: i32,
    },
    #[error("node {node:?} margins {margin:?} invert border box {border_box:?}")]
    InvertedMarginBox {
        node: ViewGeometryNodeId,
        border_box: ViewGeometryRect,
        margin: ViewPhysicalEdges<i32>,
    },
    #[error("node {node:?} supplies an inset on static {axis:?}")]
    InsetOnStatic {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} relative {axis:?} has both physical insets")]
    OverConstrainedRelativeAxis {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} positioned {axis:?} has definite size and both insets")]
    OverConstrainedPositionedAxis {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} stretched {axis:?} size violates edge or min/max constraints")]
    PositionedStretchConstraintViolation {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
        candidate_milli: i64,
        edge_extent_milli: u32,
        min_milli: Option<u32>,
        max_milli: Option<u32>,
    },
    #[error(
        "node {node:?} {axis:?} scroll offset {current_milli} is outside {min_milli}..={max_milli}"
    )]
    ScrollOffsetOutOfRange {
        node: ViewGeometryNodeId,
        axis: ViewPhysicalAxis,
        current_milli: i32,
        min_milli: i32,
        max_milli: i32,
    },
    #[error(
        "node {node:?} property {property:?} is represented-only for {consumer:?}: {feature:?}"
    )]
    UnsupportedConsumer {
        node: ViewGeometryNodeId,
        consumer: ViewGeometryConsumer,
        property: ViewPropertyKind,
        feature: ViewRepresentedGeometryFeature,
    },
    #[error("node {node:?} has missing or cyclic geometry parentage")]
    InvalidTree { node: ViewGeometryNodeId },
    #[error("node {node:?} has no intrinsic content measure")]
    MissingIntrinsicMeasure { node: ViewGeometryNodeId },
    #[error("logical pointer coordinate bits {value_bits:#018x} are invalid: {kind:?}")]
    InvalidPointerCoordinate {
        value_bits: u64,
        kind: ViewPointerCoordinateErrorKind,
    },
}
