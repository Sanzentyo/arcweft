use super::{ViewGeometryRect, ViewStyleNodeKey};
use crate::ViewElementKind;
use crate::style::{
    ViewDisplay, ViewPhysicalAxis, ViewPhysicalEdges, ViewPhysicalFlow, ViewPropertyKind,
};
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
    node: &ViewStyleNodeKey,
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
        node: ViewStyleNodeKey,
        field: ViewGeometryField,
        value_milli: i32,
    },
    #[error("node {node:?} {axis:?} min {min_milli} exceeds max {max_milli}")]
    ConflictingConstraints {
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
        min_milli: u32,
        max_milli: u32,
    },
    #[error("node {node:?} {axis:?} edges {edges_milli} exceed used size {used_milli}")]
    EdgesExceedUsedBorderBox {
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
        used_milli: u32,
        edges_milli: u64,
    },
    #[error("node {node:?} arithmetic overflow in {operation:?} on {axis:?}")]
    ArithmeticOverflow {
        node: ViewStyleNodeKey,
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
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
        border_extent_milli: u32,
        margin_start_milli: i32,
        margin_end_milli: i32,
    },
    #[error("node {node:?} margins {margin:?} invert border box {border_box:?}")]
    InvertedMarginBox {
        node: ViewStyleNodeKey,
        border_box: ViewGeometryRect,
        margin: ViewPhysicalEdges<i32>,
    },
    #[error("node {node:?} supplies an inset on static {axis:?}")]
    InsetOnStatic {
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} relative {axis:?} has both physical insets")]
    OverConstrainedRelativeAxis {
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} positioned {axis:?} has definite size and both insets")]
    OverConstrainedPositionedAxis {
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
    },
    #[error("node {node:?} stretched {axis:?} size violates edge or min/max constraints")]
    PositionedStretchConstraintViolation {
        node: ViewStyleNodeKey,
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
        node: ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
        current_milli: i32,
        min_milli: i32,
        max_milli: i32,
    },
    #[error(
        "node {node:?} property {property:?} is represented-only for {consumer:?}: {feature:?}"
    )]
    UnsupportedConsumer {
        node: ViewStyleNodeKey,
        consumer: ViewGeometryConsumer,
        property: ViewPropertyKind,
        feature: ViewRepresentedGeometryFeature,
    },
    #[error("node {node:?} leaf element {element:?} cannot use container display {display:?}")]
    DisplayRequiresContainer {
        node: ViewStyleNodeKey,
        element: ViewElementKind,
        display: ViewDisplay,
    },
    #[error("node {node:?} leaf element {element:?} cannot use container property {property:?}")]
    ContainerStyleOnLeaf {
        node: ViewStyleNodeKey,
        element: ViewElementKind,
        property: ViewPropertyKind,
    },
    #[error("node {node:?} {flow:?} cross-axis gap {property:?}={value_milli} requires wrap")]
    CrossAxisGapRequiresWrap {
        node: ViewStyleNodeKey,
        flow: ViewPhysicalFlow,
        property: ViewPropertyKind,
        value_milli: i32,
    },
    #[error("node {node:?} non-linear flow {flow:?} cannot use gap {property:?}={value_milli}")]
    GapRequiresLinearFlow {
        node: ViewStyleNodeKey,
        flow: ViewPhysicalFlow,
        property: ViewPropertyKind,
        value_milli: i32,
    },
    #[error("node {node:?} has missing or cyclic geometry parentage")]
    InvalidTree { node: ViewStyleNodeKey },
    #[error("node {node:?} has no intrinsic content measure")]
    MissingIntrinsicMeasure { node: ViewStyleNodeKey },
    #[error("logical pointer coordinate bits {value_bits:#018x} are invalid: {kind:?}")]
    InvalidPointerCoordinate {
        value_bits: u64,
        kind: ViewPointerCoordinateErrorKind,
    },
}
