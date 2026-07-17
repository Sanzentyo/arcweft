use super::{
    ViewGeometryError, ViewGeometryOperation, ViewPointerCoordinateErrorKind, ViewStyleNodeKey,
};
use crate::style::{ViewOverflow, ViewPhysicalAxis, ViewPhysicalEdges, ViewScalarMilli};

const MILLI_PER_LOGICAL_PIXEL: i32 = 1_000;

/// One physical point in integer milli-pixels.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryPoint {
    pub x_milli: i32,
    pub y_milli: i32,
}

impl ViewGeometryPoint {
    pub const fn new(x_milli: i32, y_milli: i32) -> Self {
        Self { x_milli, y_milli }
    }
}

/// One non-negative physical extent in integer milli-pixels.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometrySize {
    pub width_milli: u32,
    pub height_milli: u32,
}

impl ViewGeometrySize {
    pub const fn new(width_milli: u32, height_milli: u32) -> Self {
        Self {
            width_milli,
            height_milli,
        }
    }

    pub const fn axis(self, axis: ViewPhysicalAxis) -> u32 {
        match axis {
            ViewPhysicalAxis::X => self.width_milli,
            ViewPhysicalAxis::Y => self.height_milli,
        }
    }
}

/// One valid half-open physical span.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometrySpan {
    pub start_milli: i32,
    pub end_milli: i32,
}

impl ViewGeometrySpan {
    pub fn new(start_milli: i32, end_milli: i32) -> Result<Self, ViewGeometryError> {
        if start_milli > end_milli {
            return Err(ViewGeometryError::InvertedSpan {
                start_milli,
                end_milli,
            });
        }
        Ok(Self {
            start_milli,
            end_milli,
        })
    }

    pub fn from_start_extent(
        node: &ViewStyleNodeKey,
        axis: ViewPhysicalAxis,
        start_milli: i32,
        extent_milli: u32,
    ) -> Result<Self, ViewGeometryError> {
        let end_milli = checked_i32(
            node,
            Some(axis),
            ViewGeometryOperation::Add,
            i64::from(start_milli) + i64::from(extent_milli),
        )?;
        Self::new(start_milli, end_milli)
    }

    pub fn extent_milli(self) -> u32 {
        self.end_milli.abs_diff(self.start_milli)
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let start_milli = self.start_milli.max(other.start_milli);
        let end_milli = self.end_milli.min(other.end_milli);
        (start_milli < end_milli).then_some(Self {
            start_milli,
            end_milli,
        })
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            start_milli: if self.start_milli < other.start_milli {
                self.start_milli
            } else {
                other.start_milli
            },
            end_milli: if self.end_milli > other.end_milli {
                self.end_milli
            } else {
                other.end_milli
            },
        }
    }
}

/// One valid half-open physical rectangle.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryRect {
    pub left_milli: i32,
    pub top_milli: i32,
    pub right_milli: i32,
    pub bottom_milli: i32,
}

impl ViewGeometryRect {
    pub fn new(
        left_milli: i32,
        top_milli: i32,
        right_milli: i32,
        bottom_milli: i32,
    ) -> Result<Self, ViewGeometryError> {
        if left_milli > right_milli || top_milli > bottom_milli {
            return Err(ViewGeometryError::InvertedRect {
                left_milli,
                top_milli,
                right_milli,
                bottom_milli,
            });
        }
        Ok(Self {
            left_milli,
            top_milli,
            right_milli,
            bottom_milli,
        })
    }

    pub fn from_origin_size(
        node: &ViewStyleNodeKey,
        origin: ViewGeometryPoint,
        size: ViewGeometrySize,
    ) -> Result<Self, ViewGeometryError> {
        let x = ViewGeometrySpan::from_start_extent(
            node,
            ViewPhysicalAxis::X,
            origin.x_milli,
            size.width_milli,
        )?;
        let y = ViewGeometrySpan::from_start_extent(
            node,
            ViewPhysicalAxis::Y,
            origin.y_milli,
            size.height_milli,
        )?;
        Self::new(x.start_milli, y.start_milli, x.end_milli, y.end_milli)
    }

    pub const fn x(self) -> ViewGeometrySpan {
        ViewGeometrySpan {
            start_milli: self.left_milli,
            end_milli: self.right_milli,
        }
    }

    pub const fn y(self) -> ViewGeometrySpan {
        ViewGeometrySpan {
            start_milli: self.top_milli,
            end_milli: self.bottom_milli,
        }
    }

    pub fn size(self) -> ViewGeometrySize {
        ViewGeometrySize::new(self.x().extent_milli(), self.y().extent_milli())
    }

    pub const fn is_empty(self) -> bool {
        self.left_milli == self.right_milli || self.top_milli == self.bottom_milli
    }

    pub const fn union(self, other: Self) -> Self {
        Self {
            left_milli: if self.left_milli < other.left_milli {
                self.left_milli
            } else {
                other.left_milli
            },
            top_milli: if self.top_milli < other.top_milli {
                self.top_milli
            } else {
                other.top_milli
            },
            right_milli: if self.right_milli > other.right_milli {
                self.right_milli
            } else {
                other.right_milli
            },
            bottom_milli: if self.bottom_milli > other.bottom_milli {
                self.bottom_milli
            } else {
                other.bottom_milli
            },
        }
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        let left_milli = self.left_milli.max(other.left_milli);
        let top_milli = self.top_milli.max(other.top_milli);
        let right_milli = self.right_milli.min(other.right_milli);
        let bottom_milli = self.bottom_milli.min(other.bottom_milli);
        (left_milli < right_milli && top_milli < bottom_milli).then_some(Self {
            left_milli,
            top_milli,
            right_milli,
            bottom_milli,
        })
    }

    pub fn translated(
        self,
        node: &ViewStyleNodeKey,
        delta: ViewGeometryPoint,
    ) -> Result<Self, ViewGeometryError> {
        let left_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::X),
            ViewGeometryOperation::Translate,
            i64::from(self.left_milli) + i64::from(delta.x_milli),
        )?;
        let right_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::X),
            ViewGeometryOperation::Translate,
            i64::from(self.right_milli) + i64::from(delta.x_milli),
        )?;
        let top_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::Y),
            ViewGeometryOperation::Translate,
            i64::from(self.top_milli) + i64::from(delta.y_milli),
        )?;
        let bottom_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::Y),
            ViewGeometryOperation::Translate,
            i64::from(self.bottom_milli) + i64::from(delta.y_milli),
        )?;
        Self::new(left_milli, top_milli, right_milli, bottom_milli)
    }

    pub fn outset_signed(
        self,
        node: &ViewStyleNodeKey,
        edges: ViewPhysicalEdges<i32>,
    ) -> Result<Self, ViewGeometryError> {
        let left_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::X),
            ViewGeometryOperation::Outset,
            i64::from(self.left_milli) - i64::from(edges.left),
        )?;
        let right_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::X),
            ViewGeometryOperation::Outset,
            i64::from(self.right_milli) + i64::from(edges.right),
        )?;
        let top_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::Y),
            ViewGeometryOperation::Outset,
            i64::from(self.top_milli) - i64::from(edges.top),
        )?;
        let bottom_milli = checked_i32(
            node,
            Some(ViewPhysicalAxis::Y),
            ViewGeometryOperation::Outset,
            i64::from(self.bottom_milli) + i64::from(edges.bottom),
        )?;
        Self::new(left_milli, top_milli, right_milli, bottom_milli).map_err(|_| {
            ViewGeometryError::InvertedMarginBox {
                node: node.clone(),
                border_box: self,
                margin: edges,
            }
        })
    }

    pub fn outset_non_negative(
        self,
        node: &ViewStyleNodeKey,
        edges: ViewPhysicalEdges<u32>,
    ) -> Result<Self, ViewGeometryError> {
        Self::new(
            checked_i32(
                node,
                Some(ViewPhysicalAxis::X),
                ViewGeometryOperation::Outset,
                i64::from(self.left_milli) - i64::from(edges.left),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::Y),
                ViewGeometryOperation::Outset,
                i64::from(self.top_milli) - i64::from(edges.top),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::X),
                ViewGeometryOperation::Outset,
                i64::from(self.right_milli) + i64::from(edges.right),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::Y),
                ViewGeometryOperation::Outset,
                i64::from(self.bottom_milli) + i64::from(edges.bottom),
            )?,
        )
    }

    pub fn inset_non_negative(
        self,
        node: &ViewStyleNodeKey,
        edges: ViewPhysicalEdges<u32>,
    ) -> Result<Self, ViewGeometryError> {
        let horizontal_edges = u64::from(edges.left) + u64::from(edges.right);
        let vertical_edges = u64::from(edges.top) + u64::from(edges.bottom);
        if horizontal_edges > u64::from(self.x().extent_milli()) {
            return Err(ViewGeometryError::EdgesExceedUsedBorderBox {
                node: node.clone(),
                axis: ViewPhysicalAxis::X,
                used_milli: self.x().extent_milli(),
                edges_milli: horizontal_edges,
            });
        }
        if vertical_edges > u64::from(self.y().extent_milli()) {
            return Err(ViewGeometryError::EdgesExceedUsedBorderBox {
                node: node.clone(),
                axis: ViewPhysicalAxis::Y,
                used_milli: self.y().extent_milli(),
                edges_milli: vertical_edges,
            });
        }
        Self::new(
            checked_i32(
                node,
                Some(ViewPhysicalAxis::X),
                ViewGeometryOperation::Inset,
                i64::from(self.left_milli) + i64::from(edges.left),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::Y),
                ViewGeometryOperation::Inset,
                i64::from(self.top_milli) + i64::from(edges.top),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::X),
                ViewGeometryOperation::Inset,
                i64::from(self.right_milli) - i64::from(edges.right),
            )?,
            checked_i32(
                node,
                Some(ViewPhysicalAxis::Y),
                ViewGeometryOperation::Inset,
                i64::from(self.bottom_milli) - i64::from(edges.bottom),
            )?,
        )
    }

    /// Converts milli endpoints to an outward integer-pixel capture rectangle.
    pub fn outward_raster_rect(self) -> ViewGeometryRasterRect {
        ViewGeometryRasterRect {
            left_px: self.left_milli.div_euclid(MILLI_PER_LOGICAL_PIXEL),
            top_px: self.top_milli.div_euclid(MILLI_PER_LOGICAL_PIXEL),
            right_px: ceil_div_i32(self.right_milli, MILLI_PER_LOGICAL_PIXEL),
            bottom_px: ceil_div_i32(self.bottom_milli, MILLI_PER_LOGICAL_PIXEL),
        }
    }
}

/// Integer capture bounds produced by outward milli-pixel rounding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryRasterRect {
    pub left_px: i32,
    pub top_px: i32,
    pub right_px: i32,
    pub bottom_px: i32,
}

/// Translation followed by uniform scale about the translated border-box center.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryTransform {
    pub border_box: ViewGeometryRect,
    pub translate: ViewGeometryPoint,
    pub scale: ViewScalarMilli,
}

/// One independently bounded or unbounded physical clip axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryClipAxis {
    Unbounded,
    Bounded(ViewGeometrySpan),
}

/// A validated non-empty pair of physical clip axes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryClipAxes {
    x: ViewGeometryClipAxis,
    y: ViewGeometryClipAxis,
}

/// Closed physical clip state. Empty is distinct from two unbounded axes.
#[must_use]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewGeometryClip {
    Empty,
    NonEmpty(ViewGeometryClipAxes),
}

impl ViewGeometryClipAxis {
    pub const fn unbounded() -> Self {
        Self::Unbounded
    }

    pub const fn bounded(span: ViewGeometrySpan) -> Self {
        Self::Bounded(span)
    }

    fn intersect(self, other: Self) -> Option<Self> {
        match (self, other) {
            (Self::Unbounded, axis) | (axis, Self::Unbounded) => Some(axis),
            (Self::Bounded(left), Self::Bounded(right)) => {
                left.intersection(right).map(Self::Bounded)
            }
        }
    }
}

impl ViewGeometryClipAxes {
    pub const fn x(self) -> ViewGeometryClipAxis {
        self.x
    }

    pub const fn y(self) -> ViewGeometryClipAxis {
        self.y
    }
}

impl ViewGeometryClip {
    pub const fn unbounded() -> Self {
        Self::NonEmpty(ViewGeometryClipAxes {
            x: ViewGeometryClipAxis::Unbounded,
            y: ViewGeometryClipAxis::Unbounded,
        })
    }

    pub fn from_rect(rect: ViewGeometryRect) -> Self {
        Self::from_axes(
            ViewGeometryClipAxis::bounded(rect.x()),
            ViewGeometryClipAxis::bounded(rect.y()),
        )
    }

    pub fn from_axes(x: ViewGeometryClipAxis, y: ViewGeometryClipAxis) -> Self {
        if matches!(x, ViewGeometryClipAxis::Bounded(span) if span.extent_milli() == 0)
            || matches!(y, ViewGeometryClipAxis::Bounded(span) if span.extent_milli() == 0)
        {
            return Self::Empty;
        }
        Self::NonEmpty(ViewGeometryClipAxes { x, y })
    }

    pub const fn axes(self) -> Option<ViewGeometryClipAxes> {
        match self {
            Self::Empty => None,
            Self::NonEmpty(axes) => Some(axes),
        }
    }

    pub fn intersect(self, other: Self) -> Self {
        let (Self::NonEmpty(left), Self::NonEmpty(right)) = (self, other) else {
            return Self::Empty;
        };
        let Some(x) = left.x.intersect(right.x) else {
            return Self::Empty;
        };
        let Some(y) = left.y.intersect(right.y) else {
            return Self::Empty;
        };
        Self::from_axes(x, y)
    }

    pub fn with_overflow(
        self,
        padding_box: ViewGeometryRect,
        overflow_x: ViewOverflow,
        overflow_y: ViewOverflow,
    ) -> Self {
        let x = if overflow_x.clips_descendants() {
            ViewGeometryClipAxis::bounded(padding_box.x())
        } else {
            ViewGeometryClipAxis::unbounded()
        };
        let y = if overflow_y.clips_descendants() {
            ViewGeometryClipAxis::bounded(padding_box.y())
        } else {
            ViewGeometryClipAxis::unbounded()
        };
        self.intersect(Self::from_axes(x, y))
    }

    pub fn clip_rect(self, rect: ViewGeometryRect) -> Option<ViewGeometryRect> {
        if rect.is_empty() {
            return None;
        }
        let Self::NonEmpty(axes) = self else {
            return None;
        };
        let x = match axes.x {
            ViewGeometryClipAxis::Unbounded => rect.x(),
            ViewGeometryClipAxis::Bounded(clip) => rect.x().intersection(clip)?,
        };
        let y = match axes.y {
            ViewGeometryClipAxis::Unbounded => rect.y(),
            ViewGeometryClipAxis::Bounded(clip) => rect.y().intersection(clip)?,
        };
        Some(ViewGeometryRect {
            left_milli: x.start_milli,
            top_milli: y.start_milli,
            right_milli: x.end_milli,
            bottom_milli: y.end_milli,
        })
    }
}

pub fn transform_rect(
    node: &ViewStyleNodeKey,
    rect: ViewGeometryRect,
    transform: ViewGeometryTransform,
) -> Result<ViewGeometryRect, ViewGeometryError> {
    let translated_rect = rect.translated(node, transform.translate)?;
    let translated_center = transform.border_box.translated(node, transform.translate)?;
    if transform.scale == ViewScalarMilli::ONE {
        return Ok(translated_rect);
    }
    if transform.scale == ViewScalarMilli::ZERO {
        let center_x = floor_div(
            i128::from(translated_center.left_milli) + i128::from(translated_center.right_milli),
            2,
        );
        let center_y = floor_div(
            i128::from(translated_center.top_milli) + i128::from(translated_center.bottom_milli),
            2,
        );
        let x = checked_i32_i128(
            node,
            Some(ViewPhysicalAxis::X),
            ViewGeometryOperation::Scale,
            center_x,
        )?;
        let y = checked_i32_i128(
            node,
            Some(ViewPhysicalAxis::Y),
            ViewGeometryOperation::Scale,
            center_y,
        )?;
        return ViewGeometryRect::new(x, y, x, y);
    }
    let doubled_center_x =
        i128::from(translated_center.left_milli) + i128::from(translated_center.right_milli);
    let doubled_center_y =
        i128::from(translated_center.top_milli) + i128::from(translated_center.bottom_milli);
    ViewGeometryRect::new(
        scale_edge(
            node,
            ViewPhysicalAxis::X,
            translated_rect.left_milli,
            doubled_center_x,
            transform.scale,
            RoundDirection::Down,
        )?,
        scale_edge(
            node,
            ViewPhysicalAxis::Y,
            translated_rect.top_milli,
            doubled_center_y,
            transform.scale,
            RoundDirection::Down,
        )?,
        scale_edge(
            node,
            ViewPhysicalAxis::X,
            translated_rect.right_milli,
            doubled_center_x,
            transform.scale,
            RoundDirection::Up,
        )?,
        scale_edge(
            node,
            ViewPhysicalAxis::Y,
            translated_rect.bottom_milli,
            doubled_center_y,
            transform.scale,
            RoundDirection::Up,
        )?,
    )
}

pub fn transform_chain(
    node: &ViewStyleNodeKey,
    rect: ViewGeometryRect,
    transforms_inner_to_outer: &[ViewGeometryTransform],
) -> Result<ViewGeometryRect, ViewGeometryError> {
    transforms_inner_to_outer
        .iter()
        .try_fold(rect, |current, transform| {
            transform_rect(node, current, *transform)
        })
}

pub fn union_rects(rects: impl IntoIterator<Item = ViewGeometryRect>) -> Option<ViewGeometryRect> {
    rects.into_iter().reduce(ViewGeometryRect::union)
}

/// Floors a finite logical pointer coordinate into the authoritative milli grid.
#[expect(
    clippy::cast_possible_truncation,
    reason = "finite bounds are checked against the complete i32 range before flooring"
)]
pub fn milli_from_logical_pointer(value: f64) -> Result<i32, ViewGeometryError> {
    if !value.is_finite() {
        return Err(ViewGeometryError::InvalidPointerCoordinate {
            value_bits: value.to_bits(),
            kind: ViewPointerCoordinateErrorKind::NonFinite,
        });
    }
    let scaled = value * f64::from(MILLI_PER_LOGICAL_PIXEL);
    if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
        return Err(ViewGeometryError::InvalidPointerCoordinate {
            value_bits: value.to_bits(),
            kind: ViewPointerCoordinateErrorKind::OutsideMilliRange,
        });
    }
    Ok(scaled.floor() as i32)
}

pub(crate) fn checked_i32(
    node: &ViewStyleNodeKey,
    axis: Option<ViewPhysicalAxis>,
    operation: ViewGeometryOperation,
    value: i64,
) -> Result<i32, ViewGeometryError> {
    i32::try_from(value).map_err(|_| ViewGeometryError::ArithmeticOverflow {
        node: node.clone(),
        axis,
        operation,
    })
}

pub(crate) fn checked_u32(
    node: &ViewStyleNodeKey,
    axis: Option<ViewPhysicalAxis>,
    operation: ViewGeometryOperation,
    value: i64,
) -> Result<u32, ViewGeometryError> {
    u32::try_from(value).map_err(|_| ViewGeometryError::ArithmeticOverflow {
        node: node.clone(),
        axis,
        operation,
    })
}

pub(crate) fn checked_u32_sum(
    node: &ViewStyleNodeKey,
    axis: Option<ViewPhysicalAxis>,
    operation: ViewGeometryOperation,
    values: impl IntoIterator<Item = u32>,
) -> Result<u32, ViewGeometryError> {
    let total = values
        .into_iter()
        .try_fold(0_u64, |total, value| total.checked_add(u64::from(value)))
        .ok_or_else(|| ViewGeometryError::ArithmeticOverflow {
            node: node.clone(),
            axis,
            operation,
        })?;
    u32::try_from(total).map_err(|_| ViewGeometryError::ArithmeticOverflow {
        node: node.clone(),
        axis,
        operation,
    })
}

fn checked_i32_i128(
    node: &ViewStyleNodeKey,
    axis: Option<ViewPhysicalAxis>,
    operation: ViewGeometryOperation,
    value: i128,
) -> Result<i32, ViewGeometryError> {
    i32::try_from(value).map_err(|_| ViewGeometryError::ArithmeticOverflow {
        node: node.clone(),
        axis,
        operation,
    })
}

fn scale_edge(
    node: &ViewStyleNodeKey,
    axis: ViewPhysicalAxis,
    edge_milli: i32,
    center2: i128,
    scale: ViewScalarMilli,
    direction: RoundDirection,
) -> Result<i32, ViewGeometryError> {
    let relative2 = i128::from(edge_milli) * 2 - center2;
    let numerator =
        center2 * i128::from(MILLI_PER_LOGICAL_PIXEL) + relative2 * i128::from(scale.value());
    let denominator = i128::from(MILLI_PER_LOGICAL_PIXEL) * 2;
    let value = match direction {
        RoundDirection::Down => floor_div(numerator, denominator),
        RoundDirection::Up => ceil_div(numerator, denominator),
    };
    checked_i32_i128(node, Some(axis), ViewGeometryOperation::Scale, value)
}

const fn floor_div(numerator: i128, denominator: i128) -> i128 {
    numerator.div_euclid(denominator)
}

const fn ceil_div(numerator: i128, denominator: i128) -> i128 {
    let quotient = numerator.div_euclid(denominator);
    if numerator.rem_euclid(denominator) == 0 {
        quotient
    } else {
        quotient + 1
    }
}

const fn ceil_div_i32(numerator: i32, denominator: i32) -> i32 {
    let quotient = numerator.div_euclid(denominator);
    if numerator.rem_euclid(denominator) == 0 {
        quotient
    } else {
        quotient + 1
    }
}

#[derive(Clone, Copy)]
enum RoundDirection {
    Down,
    Up,
}
