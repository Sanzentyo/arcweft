use super::primitives::{checked_i32, checked_u32, checked_u32_sum};
use super::revision::measured_revision;
use super::{
    ViewGeometryError, ViewGeometryField, ViewGeometryNodeId, ViewGeometryOperation,
    ViewGeometryPoint, ViewGeometryRect, ViewGeometrySize, ViewGeometrySpan,
    ViewIntrinsicMeasureRevision, ViewMeasuredGeometryRevision,
};
use crate::style::{
    ViewLengthMilli, ViewPhysicalAxis, ViewPhysicalBoxStyle, ViewPhysicalContainerStyle,
    ViewPhysicalEdges, ViewPhysicalFlow, ViewPosition,
};

/// Host- or child-derived intrinsic content-box measurement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewIntrinsicMeasure {
    pub content_size: ViewGeometrySize,
    pub revision: ViewIntrinsicMeasureRevision,
}

/// Validated measurement for one physical border-box axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMeasuredAxis {
    pub natural_border_extent_milli: u32,
    pub used_border_extent_milli: u32,
    pub edge_extent_milli: u32,
    pub min_milli: Option<u32>,
    pub max_milli: Option<u32>,
    pub auto: bool,
}

/// Validated content, edge, and border-box measurement for one node.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMeasuredBox {
    pub x: ViewMeasuredAxis,
    pub y: ViewMeasuredAxis,
    pub content_size: ViewGeometrySize,
    pub padding: ViewPhysicalEdges<u32>,
    pub border: ViewPhysicalEdges<u32>,
    pub margin: ViewPhysicalEdges<i32>,
    pub revision: ViewMeasuredGeometryRevision,
}

impl ViewMeasuredBox {
    pub const fn border_size(self) -> ViewGeometrySize {
        ViewGeometrySize::new(
            self.x.used_border_extent_milli,
            self.y.used_border_extent_milli,
        )
    }
}

/// Signed-margin outer size after proving that neither axis is inverted.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewOuterSize {
    pub width_milli: u32,
    pub height_milli: u32,
}

/// Placed border and margin spans for one physical axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPlacedAxis {
    pub border: ViewGeometrySpan,
    pub margin: ViewGeometrySpan,
    pub used_border_extent_milli: u32,
}

/// Local content, padding, border, and margin rectangles after placement.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewBoxPlacement {
    pub content_box: ViewGeometryRect,
    pub padding_box: ViewGeometryRect,
    pub border_box: ViewGeometryRect,
    pub margin_box: ViewGeometryRect,
}

pub fn measure_box(
    node: &ViewGeometryNodeId,
    style: &ViewPhysicalBoxStyle,
    intrinsic: ViewIntrinsicMeasure,
) -> Result<ViewMeasuredBox, ViewGeometryError> {
    let padding = validate_non_negative_edges(node, style.padding, EdgeKind::Padding)?;
    let border = validate_non_negative_edges(node, style.border, EdgeKind::Border)?;
    let horizontal_edges = checked_u32_sum(
        node,
        Some(ViewPhysicalAxis::X),
        ViewGeometryOperation::Add,
        [padding.left, padding.right, border.left, border.right],
    )?;
    let vertical_edges = checked_u32_sum(
        node,
        Some(ViewPhysicalAxis::Y),
        ViewGeometryOperation::Add,
        [padding.top, padding.bottom, border.top, border.bottom],
    )?;
    let natural_width = checked_u32_sum(
        node,
        Some(ViewPhysicalAxis::X),
        ViewGeometryOperation::Add,
        [intrinsic.content_size.width_milli, horizontal_edges],
    )?;
    let natural_height = checked_u32_sum(
        node,
        Some(ViewPhysicalAxis::Y),
        ViewGeometryOperation::Add,
        [intrinsic.content_size.height_milli, vertical_edges],
    )?;
    let x = measure_axis(
        node,
        ViewPhysicalAxis::X,
        style.width,
        style.min_width,
        style.max_width,
        natural_width,
        horizontal_edges,
    )?;
    let y = measure_axis(
        node,
        ViewPhysicalAxis::Y,
        style.height,
        style.min_height,
        style.max_height,
        natural_height,
        vertical_edges,
    )?;
    let content_size = ViewGeometrySize::new(
        x.used_border_extent_milli - x.edge_extent_milli,
        y.used_border_extent_milli - y.edge_extent_milli,
    );
    let margin = map_edges(style.margin, ViewLengthMilli::value);
    Ok(ViewMeasuredBox {
        x,
        y,
        content_size,
        padding,
        border,
        margin,
        revision: measured_revision(
            node,
            style,
            intrinsic,
            x.used_border_extent_milli,
            y.used_border_extent_milli,
        ),
    })
}

pub fn outer_size(
    node: &ViewGeometryNodeId,
    measured: ViewMeasuredBox,
) -> Result<ViewOuterSize, ViewGeometryError> {
    Ok(ViewOuterSize {
        width_milli: checked_outer_extent(
            node,
            ViewPhysicalAxis::X,
            measured.x.used_border_extent_milli,
            measured.margin.left,
            measured.margin.right,
        )?,
        height_milli: checked_outer_extent(
            node,
            ViewPhysicalAxis::Y,
            measured.y.used_border_extent_milli,
            measured.margin.top,
            measured.margin.bottom,
        )?,
    })
}

pub fn flow_intrinsic_size(
    node: &ViewGeometryNodeId,
    container: ViewPhysicalContainerStyle,
    children: &[ViewOuterSize],
) -> Result<ViewGeometrySize, ViewGeometryError> {
    let row_gap = non_negative(node, ViewGeometryField::RowGap, container.row_gap.value())?;
    let column_gap = non_negative(
        node,
        ViewGeometryField::ColumnGap,
        container.column_gap.value(),
    )?;
    let gap_count = u32::try_from(if children.is_empty() {
        0
    } else {
        children.len() - 1
    })
    .map_err(|_| ViewGeometryError::ArithmeticOverflow {
        node: node.clone(),
        axis: container.flow.main_axis(),
        operation: ViewGeometryOperation::Multiply,
    })?;
    match container.flow {
        ViewPhysicalFlow::Overlay => Ok(ViewGeometrySize::new(
            children
                .iter()
                .map(|child| child.width_milli)
                .max()
                .unwrap_or(0),
            children
                .iter()
                .map(|child| child.height_milli)
                .max()
                .unwrap_or(0),
        )),
        ViewPhysicalFlow::Row | ViewPhysicalFlow::RowReverse => Ok(ViewGeometrySize::new(
            checked_u32_sum(
                node,
                Some(ViewPhysicalAxis::X),
                ViewGeometryOperation::Add,
                children
                    .iter()
                    .map(|child| child.width_milli)
                    .chain([checked_product(
                        node,
                        ViewPhysicalAxis::X,
                        column_gap,
                        gap_count,
                    )?]),
            )?,
            children
                .iter()
                .map(|child| child.height_milli)
                .max()
                .unwrap_or(0),
        )),
        ViewPhysicalFlow::Column | ViewPhysicalFlow::ColumnReverse => Ok(ViewGeometrySize::new(
            children
                .iter()
                .map(|child| child.width_milli)
                .max()
                .unwrap_or(0),
            checked_u32_sum(
                node,
                Some(ViewPhysicalAxis::Y),
                ViewGeometryOperation::Add,
                children
                    .iter()
                    .map(|child| child.height_milli)
                    .chain([checked_product(
                        node,
                        ViewPhysicalAxis::Y,
                        row_gap,
                        gap_count,
                    )?]),
            )?,
        )),
    }
}

pub fn place_box(
    node: &ViewGeometryNodeId,
    style: &ViewPhysicalBoxStyle,
    measured: ViewMeasuredBox,
    containing_block: ViewGeometryRect,
    static_border_origin: ViewGeometryPoint,
) -> Result<ViewBoxPlacement, ViewGeometryError> {
    let x = place_axis(
        node,
        ViewPhysicalAxis::X,
        style.position,
        containing_block.x(),
        static_border_origin.x_milli,
        measured.x,
        measured.margin.left,
        measured.margin.right,
        style.inset.left.map(ViewLengthMilli::value),
        style.inset.right.map(ViewLengthMilli::value),
    )?;
    let y = place_axis(
        node,
        ViewPhysicalAxis::Y,
        style.position,
        containing_block.y(),
        static_border_origin.y_milli,
        measured.y,
        measured.margin.top,
        measured.margin.bottom,
        style.inset.top.map(ViewLengthMilli::value),
        style.inset.bottom.map(ViewLengthMilli::value),
    )?;
    let border_box = ViewGeometryRect::new(
        x.border.start_milli,
        y.border.start_milli,
        x.border.end_milli,
        y.border.end_milli,
    )?;
    let margin_box = ViewGeometryRect::new(
        x.margin.start_milli,
        y.margin.start_milli,
        x.margin.end_milli,
        y.margin.end_milli,
    )
    .map_err(|_| ViewGeometryError::InvertedMarginBox {
        node: node.clone(),
        border_box,
        margin: measured.margin,
    })?;
    let padding_box = border_box.inset_non_negative(node, measured.border)?;
    let content_box = padding_box.inset_non_negative(node, measured.padding)?;
    Ok(ViewBoxPlacement {
        content_box,
        padding_box,
        border_box,
        margin_box,
    })
}

#[expect(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the physical positioning contract exposes both edge inputs explicitly"
)]
pub fn place_axis(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    position: ViewPosition,
    containing: ViewGeometrySpan,
    static_border_start_milli: i32,
    measured: ViewMeasuredAxis,
    margin_start_milli: i32,
    margin_end_milli: i32,
    start_inset_milli: Option<i32>,
    end_inset_milli: Option<i32>,
) -> Result<ViewPlacedAxis, ViewGeometryError> {
    let mut used_border_extent_milli = measured.used_border_extent_milli;
    let border_start_milli = match position {
        ViewPosition::Static => {
            if start_inset_milli.is_some() || end_inset_milli.is_some() {
                return Err(ViewGeometryError::InsetOnStatic {
                    node: node.clone(),
                    axis,
                });
            }
            static_border_start_milli
        }
        ViewPosition::Relative => match (start_inset_milli, end_inset_milli) {
            (Some(_), Some(_)) => {
                return Err(ViewGeometryError::OverConstrainedRelativeAxis {
                    node: node.clone(),
                    axis,
                });
            }
            (Some(value), None) => checked_i32(
                node,
                Some(axis),
                ViewGeometryOperation::Add,
                i64::from(static_border_start_milli) + i64::from(value),
            )?,
            (None, Some(value)) => checked_i32(
                node,
                Some(axis),
                ViewGeometryOperation::Subtract,
                i64::from(static_border_start_milli) - i64::from(value),
            )?,
            (None, None) => static_border_start_milli,
        },
        ViewPosition::Absolute | ViewPosition::Fixed => {
            match (start_inset_milli, end_inset_milli) {
                (Some(_), Some(_)) if !measured.auto => {
                    return Err(ViewGeometryError::OverConstrainedPositionedAxis {
                        node: node.clone(),
                        axis,
                    });
                }
                (Some(start), Some(end)) => {
                    let candidate_milli = i64::from(containing.end_milli)
                        - i64::from(containing.start_milli)
                        - i64::from(start)
                        - i64::from(end)
                        - i64::from(margin_start_milli)
                        - i64::from(margin_end_milli);
                    if candidate_milli < 0 {
                        return Err(stretch_error(node, axis, measured, candidate_milli));
                    }
                    used_border_extent_milli = checked_u32(
                        node,
                        Some(axis),
                        ViewGeometryOperation::Stretch,
                        candidate_milli,
                    )?;
                    validate_stretched_axis(node, axis, measured, used_border_extent_milli)?;
                    checked_i32(
                        node,
                        Some(axis),
                        ViewGeometryOperation::Add,
                        i64::from(containing.start_milli)
                            + i64::from(start)
                            + i64::from(margin_start_milli),
                    )?
                }
                (Some(start), None) => checked_i32(
                    node,
                    Some(axis),
                    ViewGeometryOperation::Add,
                    i64::from(containing.start_milli)
                        + i64::from(start)
                        + i64::from(margin_start_milli),
                )?,
                (None, Some(end)) => checked_i32(
                    node,
                    Some(axis),
                    ViewGeometryOperation::Subtract,
                    i64::from(containing.end_milli)
                        - i64::from(end)
                        - i64::from(margin_end_milli)
                        - i64::from(used_border_extent_milli),
                )?,
                (None, None) => static_border_start_milli,
            }
        }
    };
    let border = ViewGeometrySpan::from_start_extent(
        node,
        axis,
        border_start_milli,
        used_border_extent_milli,
    )?;
    let margin_start_coordinate = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::Outset,
        i64::from(border.start_milli) - i64::from(margin_start_milli),
    )?;
    let margin_end_coordinate = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::Outset,
        i64::from(border.end_milli) + i64::from(margin_end_milli),
    )?;
    let margin =
        ViewGeometrySpan::new(margin_start_coordinate, margin_end_coordinate).map_err(|_| {
            ViewGeometryError::InvertedMarginSpan {
                node: node.clone(),
                axis,
                border_extent_milli: border.extent_milli(),
                margin_start_milli,
                margin_end_milli,
            }
        })?;
    Ok(ViewPlacedAxis {
        border,
        margin,
        used_border_extent_milli,
    })
}

pub fn first_flow_border_start(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    content_start_milli: i32,
    margin_start_milli: i32,
) -> Result<i32, ViewGeometryError> {
    checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::FlowAdvance,
        i64::from(content_start_milli) + i64::from(margin_start_milli),
    )
}

pub fn next_flow_border_start(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    previous_border_end_milli: i32,
    previous_margin_end_milli: i32,
    gap_milli: i32,
    next_margin_start_milli: i32,
) -> Result<i32, ViewGeometryError> {
    validate_axis_gap(node, axis, gap_milli)?;
    checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::FlowAdvance,
        i64::from(previous_border_end_milli)
            + i64::from(previous_margin_end_milli)
            + i64::from(gap_milli)
            + i64::from(next_margin_start_milli),
    )
}

pub fn first_reverse_flow_border_start(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    content_end_milli: i32,
    margin_end_milli: i32,
    border_extent_milli: u32,
) -> Result<i32, ViewGeometryError> {
    checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::FlowAdvance,
        i64::from(content_end_milli) - i64::from(margin_end_milli) - i64::from(border_extent_milli),
    )
}

pub fn next_reverse_flow_border_start(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    previous_border_start_milli: i32,
    previous_margin_start_milli: i32,
    gap_milli: i32,
    next_margin_end_milli: i32,
    next_border_extent_milli: u32,
) -> Result<i32, ViewGeometryError> {
    validate_axis_gap(node, axis, gap_milli)?;
    checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::FlowAdvance,
        i64::from(previous_border_start_milli)
            - i64::from(previous_margin_start_milli)
            - i64::from(gap_milli)
            - i64::from(next_margin_end_milli)
            - i64::from(next_border_extent_milli),
    )
}

fn measure_axis(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    explicit: Option<ViewLengthMilli>,
    min: Option<ViewLengthMilli>,
    max: Option<ViewLengthMilli>,
    natural_milli: u32,
    edges_milli: u32,
) -> Result<ViewMeasuredAxis, ViewGeometryError> {
    let explicit = optional_non_negative(node, axis_field(axis, AxisField::Size), explicit)?;
    let min = optional_non_negative(node, axis_field(axis, AxisField::Min), min)?;
    let max = optional_non_negative(node, axis_field(axis, AxisField::Max), max)?;
    if let (Some(min_milli), Some(max_milli)) = (min, max)
        && min_milli > max_milli
    {
        return Err(ViewGeometryError::ConflictingConstraints {
            node: node.clone(),
            axis,
            min_milli,
            max_milli,
        });
    }
    let mut used_milli = explicit.unwrap_or(natural_milli);
    if let Some(min_milli) = min {
        used_milli = used_milli.max(min_milli);
    }
    if let Some(max_milli) = max {
        used_milli = used_milli.min(max_milli);
    }
    if used_milli < edges_milli {
        return Err(ViewGeometryError::EdgesExceedUsedBorderBox {
            node: node.clone(),
            axis,
            used_milli,
            edges_milli: u64::from(edges_milli),
        });
    }
    Ok(ViewMeasuredAxis {
        natural_border_extent_milli: natural_milli,
        used_border_extent_milli: used_milli,
        edge_extent_milli: edges_milli,
        min_milli: min,
        max_milli: max,
        auto: explicit.is_none(),
    })
}

fn validate_stretched_axis(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    measured: ViewMeasuredAxis,
    candidate_milli: u32,
) -> Result<(), ViewGeometryError> {
    let within_min = measured.min_milli.is_none_or(|min| candidate_milli >= min);
    let within_max = measured.max_milli.is_none_or(|max| candidate_milli <= max);
    if candidate_milli < measured.edge_extent_milli || !within_min || !within_max {
        return Err(stretch_error(
            node,
            axis,
            measured,
            i64::from(candidate_milli),
        ));
    }
    Ok(())
}

fn stretch_error(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    measured: ViewMeasuredAxis,
    candidate_milli: i64,
) -> ViewGeometryError {
    ViewGeometryError::PositionedStretchConstraintViolation {
        node: node.clone(),
        axis,
        candidate_milli,
        edge_extent_milli: measured.edge_extent_milli,
        min_milli: measured.min_milli,
        max_milli: measured.max_milli,
    }
}

fn validate_non_negative_edges(
    node: &ViewGeometryNodeId,
    edges: ViewPhysicalEdges<ViewLengthMilli>,
    kind: EdgeKind,
) -> Result<ViewPhysicalEdges<u32>, ViewGeometryError> {
    Ok(ViewPhysicalEdges::new(
        non_negative(node, edge_field(kind, Side::Top), edges.top.value())?,
        non_negative(node, edge_field(kind, Side::Right), edges.right.value())?,
        non_negative(node, edge_field(kind, Side::Bottom), edges.bottom.value())?,
        non_negative(node, edge_field(kind, Side::Left), edges.left.value())?,
    ))
}

fn checked_outer_extent(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    border_extent_milli: u32,
    margin_start_milli: i32,
    margin_end_milli: i32,
) -> Result<u32, ViewGeometryError> {
    let value = i64::from(border_extent_milli)
        + i64::from(margin_start_milli)
        + i64::from(margin_end_milli);
    if value < 0 {
        return Err(ViewGeometryError::InvertedMarginSpan {
            node: node.clone(),
            axis,
            border_extent_milli,
            margin_start_milli,
            margin_end_milli,
        });
    }
    checked_u32(node, Some(axis), ViewGeometryOperation::Add, value)
}

fn optional_non_negative(
    node: &ViewGeometryNodeId,
    field: ViewGeometryField,
    value: Option<ViewLengthMilli>,
) -> Result<Option<u32>, ViewGeometryError> {
    value
        .map(|value| non_negative(node, field, value.value()))
        .transpose()
}

fn non_negative(
    node: &ViewGeometryNodeId,
    field: ViewGeometryField,
    value_milli: i32,
) -> Result<u32, ViewGeometryError> {
    u32::try_from(value_milli).map_err(|_| ViewGeometryError::NegativeNonNegativeField {
        node: node.clone(),
        field,
        value_milli,
    })
}

fn validate_axis_gap(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    gap_milli: i32,
) -> Result<(), ViewGeometryError> {
    non_negative(
        node,
        match axis {
            ViewPhysicalAxis::X => ViewGeometryField::ColumnGap,
            ViewPhysicalAxis::Y => ViewGeometryField::RowGap,
        },
        gap_milli,
    )
    .map(|_| ())
}

fn checked_product(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    left: u32,
    right: u32,
) -> Result<u32, ViewGeometryError> {
    u32::try_from(u64::from(left) * u64::from(right)).map_err(|_| {
        ViewGeometryError::ArithmeticOverflow {
            node: node.clone(),
            axis: Some(axis),
            operation: ViewGeometryOperation::Multiply,
        }
    })
}

fn map_edges<T: Copy, U>(
    edges: ViewPhysicalEdges<T>,
    map: impl Fn(T) -> U,
) -> ViewPhysicalEdges<U> {
    ViewPhysicalEdges::new(
        map(edges.top),
        map(edges.right),
        map(edges.bottom),
        map(edges.left),
    )
}

#[derive(Clone, Copy)]
enum EdgeKind {
    Padding,
    Border,
}

#[derive(Clone, Copy)]
enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

const fn edge_field(kind: EdgeKind, side: Side) -> ViewGeometryField {
    match (kind, side) {
        (EdgeKind::Padding, Side::Top) => ViewGeometryField::PaddingTop,
        (EdgeKind::Padding, Side::Right) => ViewGeometryField::PaddingRight,
        (EdgeKind::Padding, Side::Bottom) => ViewGeometryField::PaddingBottom,
        (EdgeKind::Padding, Side::Left) => ViewGeometryField::PaddingLeft,
        (EdgeKind::Border, Side::Top) => ViewGeometryField::BorderTop,
        (EdgeKind::Border, Side::Right) => ViewGeometryField::BorderRight,
        (EdgeKind::Border, Side::Bottom) => ViewGeometryField::BorderBottom,
        (EdgeKind::Border, Side::Left) => ViewGeometryField::BorderLeft,
    }
}

#[derive(Clone, Copy)]
enum AxisField {
    Size,
    Min,
    Max,
}

const fn axis_field(axis: ViewPhysicalAxis, field: AxisField) -> ViewGeometryField {
    match (axis, field) {
        (ViewPhysicalAxis::X, AxisField::Size) => ViewGeometryField::Width,
        (ViewPhysicalAxis::Y, AxisField::Size) => ViewGeometryField::Height,
        (ViewPhysicalAxis::X, AxisField::Min) => ViewGeometryField::MinWidth,
        (ViewPhysicalAxis::Y, AxisField::Min) => ViewGeometryField::MinHeight,
        (ViewPhysicalAxis::X, AxisField::Max) => ViewGeometryField::MaxWidth,
        (ViewPhysicalAxis::Y, AxisField::Max) => ViewGeometryField::MaxHeight,
    }
}
