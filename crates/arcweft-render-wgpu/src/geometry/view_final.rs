//! Checked WGPU preparation from committed retained View geometry.
//!
//! This adapter deliberately accepts final geometry rather than computed
//! Style. Integer milli-pixels remain authoritative until every renderer
//! coordinate and raster range has passed an exact conversion check.

use arcweft_view::geometry::{
    ViewFinalGeometry, ViewFinalGeometryRevision, ViewGeometryClip, ViewGeometryClipAxis,
    ViewGeometryConsumer, ViewGeometryRasterRect, ViewGeometryRect,
};
use arcweft_view::style::ViewStyleNodeKey;
use thiserror::Error;

/// Physical field being lowered for WGPU consumption.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewFinalGeometryField {
    Left,
    Top,
    Right,
    Bottom,
    Clip,
    Raster,
    IndexRange,
}

/// Checked conversion failure while preparing committed View geometry.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewFinalGeometryLoweringError {
    #[error(
        "milli value {value_milli} is not exactly representable for {consumer:?} WGPU {field:?} on {node:?}"
    )]
    InexactF32 {
        node: ViewStyleNodeKey,
        consumer: ViewGeometryConsumer,
        field: ViewFinalGeometryField,
        value_milli: i64,
        round_trip_milli: i64,
    },
    #[error(
        "value {value} exceeds WGPU target maximum {max} for {consumer:?} {field:?} on {node:?}"
    )]
    IndexRange {
        node: Option<ViewStyleNodeKey>,
        consumer: ViewGeometryConsumer,
        field: ViewFinalGeometryField,
        value: u64,
        max: u64,
    },
}

/// One exact rectangle represented in WGPU-compatible logical pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PreparedViewRect {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// One independently bounded renderer clip axis.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreparedViewClipAxis {
    Unbounded,
    Bounded { start: f32, end: f32 },
}

/// Closed renderer clip state derived from final retained geometry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PreparedViewClip {
    Empty,
    NonEmpty {
        x: PreparedViewClipAxis,
        y: PreparedViewClipAxis,
    },
}

/// Outward-rounded integer raster bounds and checked extents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedViewRasterRect {
    pub left_px: i32,
    pub top_px: i32,
    pub right_px: i32,
    pub bottom_px: i32,
    pub width_px: u32,
    pub height_px: u32,
}

/// Renderer-owned immutable lowering of one visible final View node.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedViewFinalNode {
    node: ViewStyleNodeKey,
    final_revision: ViewFinalGeometryRevision,
    world_border_box: PreparedViewRect,
    paint_bounds: PreparedViewRect,
    descendant_clip: PreparedViewClip,
    raster_bounds: PreparedViewRasterRect,
}

impl PreparedViewFinalNode {
    pub fn node(&self) -> &ViewStyleNodeKey {
        &self.node
    }

    pub const fn final_revision(&self) -> ViewFinalGeometryRevision {
        self.final_revision
    }

    pub const fn world_border_box(&self) -> PreparedViewRect {
        self.world_border_box
    }

    pub const fn paint_bounds(&self) -> PreparedViewRect {
        self.paint_bounds
    }

    pub const fn descendant_clip(&self) -> PreparedViewClip {
        self.descendant_clip
    }

    pub const fn raster_bounds(&self) -> PreparedViewRasterRect {
        self.raster_bounds
    }
}

/// Fallibly prepared WGPU work for one candidate geometry generation.
///
/// Construction performs every coordinate and index conversion. Publication
/// may therefore move this value without another fallible geometry operation.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedViewRenderCandidate {
    generation: u64,
    nodes: Vec<PreparedViewFinalNode>,
}

impl PreparedViewRenderCandidate {
    pub fn prepare<'a>(
        generation: u64,
        final_nodes: impl IntoIterator<Item = &'a ViewFinalGeometry>,
    ) -> Result<Self, ViewFinalGeometryLoweringError> {
        let mut nodes = Vec::new();
        for geometry in final_nodes {
            let Some(paint_bounds) = geometry.consumers.paint_bounds else {
                continue;
            };
            checked_index(
                Some(&geometry.node),
                ViewGeometryConsumer::Paint,
                nodes.len() as u64,
            )?;
            nodes.push(PreparedViewFinalNode {
                node: geometry.node.clone(),
                final_revision: geometry.final_revision,
                world_border_box: prepared_rect(
                    &geometry.node,
                    ViewGeometryConsumer::Paint,
                    geometry.world_border_box,
                )?,
                paint_bounds: prepared_rect(
                    &geometry.node,
                    ViewGeometryConsumer::Paint,
                    paint_bounds,
                )?,
                descendant_clip: prepared_clip(&geometry.node, geometry.descendant_clip)?,
                raster_bounds: prepared_raster_rect(&geometry.node, paint_bounds)?,
            });
        }
        Ok(Self { generation, nodes })
    }

    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub fn nodes(&self) -> &[PreparedViewFinalNode] {
        &self.nodes
    }
}

fn prepared_rect(
    node: &ViewStyleNodeKey,
    consumer: ViewGeometryConsumer,
    rect: ViewGeometryRect,
) -> Result<PreparedViewRect, ViewFinalGeometryLoweringError> {
    Ok(PreparedViewRect {
        left: exact_f32(
            node,
            consumer,
            ViewFinalGeometryField::Left,
            rect.left_milli,
        )?,
        top: exact_f32(node, consumer, ViewFinalGeometryField::Top, rect.top_milli)?,
        right: exact_f32(
            node,
            consumer,
            ViewFinalGeometryField::Right,
            rect.right_milli,
        )?,
        bottom: exact_f32(
            node,
            consumer,
            ViewFinalGeometryField::Bottom,
            rect.bottom_milli,
        )?,
    })
}

fn prepared_clip(
    node: &ViewStyleNodeKey,
    clip: ViewGeometryClip,
) -> Result<PreparedViewClip, ViewFinalGeometryLoweringError> {
    let Some(axes) = clip.axes() else {
        return Ok(PreparedViewClip::Empty);
    };
    Ok(PreparedViewClip::NonEmpty {
        x: prepared_clip_axis(node, axes.x())?,
        y: prepared_clip_axis(node, axes.y())?,
    })
}

fn prepared_clip_axis(
    node: &ViewStyleNodeKey,
    axis: ViewGeometryClipAxis,
) -> Result<PreparedViewClipAxis, ViewFinalGeometryLoweringError> {
    match axis {
        ViewGeometryClipAxis::Unbounded => Ok(PreparedViewClipAxis::Unbounded),
        ViewGeometryClipAxis::Bounded(span) => Ok(PreparedViewClipAxis::Bounded {
            start: exact_f32(
                node,
                ViewGeometryConsumer::Clip,
                ViewFinalGeometryField::Clip,
                span.start_milli,
            )?,
            end: exact_f32(
                node,
                ViewGeometryConsumer::Clip,
                ViewFinalGeometryField::Clip,
                span.end_milli,
            )?,
        }),
    }
}

fn prepared_raster_rect(
    node: &ViewStyleNodeKey,
    rect: ViewGeometryRect,
) -> Result<PreparedViewRasterRect, ViewFinalGeometryLoweringError> {
    let raster = rect.outward_raster_rect();
    Ok(PreparedViewRasterRect {
        left_px: raster.left_px,
        top_px: raster.top_px,
        right_px: raster.right_px,
        bottom_px: raster.bottom_px,
        width_px: raster_extent(node, raster, true)?,
        height_px: raster_extent(node, raster, false)?,
    })
}

fn raster_extent(
    node: &ViewStyleNodeKey,
    raster: ViewGeometryRasterRect,
    horizontal: bool,
) -> Result<u32, ViewFinalGeometryLoweringError> {
    let (start, end) = if horizontal {
        (raster.left_px, raster.right_px)
    } else {
        (raster.top_px, raster.bottom_px)
    };
    let extent = i64::from(end) - i64::from(start);
    u32::try_from(extent).map_err(|_| ViewFinalGeometryLoweringError::IndexRange {
        node: Some(node.clone()),
        consumer: ViewGeometryConsumer::Paint,
        field: ViewFinalGeometryField::Raster,
        value: extent.unsigned_abs(),
        max: u64::from(u32::MAX),
    })
}

fn checked_index(
    node: Option<&ViewStyleNodeKey>,
    consumer: ViewGeometryConsumer,
    value: u64,
) -> Result<u32, ViewFinalGeometryLoweringError> {
    u32::try_from(value).map_err(|_| ViewFinalGeometryLoweringError::IndexRange {
        node: node.cloned(),
        consumer,
        field: ViewFinalGeometryField::IndexRange,
        value,
        max: u64::from(u32::MAX),
    })
}

fn exact_f32(
    node: &ViewStyleNodeKey,
    consumer: ViewGeometryConsumer,
    field: ViewFinalGeometryField,
    value_milli: i32,
) -> Result<f32, ViewFinalGeometryLoweringError> {
    let value = f64::from(value_milli) / 1_000.0;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "the round-trip check rejects every inexact authoritative conversion"
    )]
    let converted = value as f32;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "i32 milli coordinates keep the finite rounded value inside i64"
    )]
    let round_trip_milli = (f64::from(converted) * 1_000.0).round() as i64;
    if round_trip_milli != i64::from(value_milli) {
        return Err(ViewFinalGeometryLoweringError::InexactF32 {
            node: node.clone(),
            consumer,
            field,
            value_milli: i64::from(value_milli),
            round_trip_milli,
        });
    }
    Ok(converted)
}

#[cfg(test)]
mod tests {
    use super::{
        PreparedViewClip, PreparedViewFinalNode, PreparedViewRenderCandidate,
        ViewFinalGeometryField, ViewFinalGeometryLoweringError, checked_index,
    };
    use arcweft_view::geometry::{
        ViewConsumerGeometry, ViewFinalGeometry, ViewGeometryClip, ViewGeometryRect,
        ViewGeometrySpan, ViewScrollAxisGeometry, ViewScrollCapability, ViewScrollGeometry,
    };
    use arcweft_view::style::ViewStyleNodeKey;
    use arcweft_view::{ViewMountId, geometry};

    fn node(instruction: u32) -> ViewStyleNodeKey {
        ViewStyleNodeKey::new(ViewMountId::from_raw(7), vec![11, 13], instruction)
    }

    fn final_geometry(
        node: ViewStyleNodeKey,
        world: ViewGeometryRect,
        paint: Option<ViewGeometryRect>,
    ) -> ViewFinalGeometry {
        let scroll_axis = ViewScrollAxisGeometry {
            capability: ViewScrollCapability::None,
            viewport: ViewGeometrySpan::new(0, 0).expect("zero span is valid"),
            content: ViewGeometrySpan::new(0, 0).expect("zero span is valid"),
            min_offset_milli: 0,
            max_offset_milli: 0,
            current_offset_milli: 0,
        };
        ViewFinalGeometry {
            node,
            content_box: world,
            padding_box: world,
            border_box: world,
            margin_box: world,
            world_border_box: world,
            descendant_clip: ViewGeometryClip::from_rect(world),
            consumers: ViewConsumerGeometry {
                visible_border_box: paint,
                hit_bounds: paint,
                focus_target_bounds: paint,
                avoidance_bounds: paint,
                scroll_target_bounds: paint,
                paint_bounds: paint,
            },
            layout_subtree_bounds: world,
            paint_subtree_bounds: paint,
            scroll: ViewScrollGeometry {
                x: scroll_axis,
                y: scroll_axis,
            },
            measured_revision: geometry::ViewMeasuredGeometryRevision::default(),
            placed_revision: geometry::ViewPlacedGeometryRevision::default(),
            final_revision: geometry::ViewFinalGeometryRevision::default(),
        }
    }

    fn only_node(candidate: &PreparedViewRenderCandidate) -> &PreparedViewFinalNode {
        candidate
            .nodes()
            .first()
            .expect("candidate must contain one visible node")
    }

    #[test]
    fn final_geometry_lowering_retains_generation_node_revision_and_outward_raster() {
        let node = node(17);
        let world = ViewGeometryRect::new(-1_501, 999, 1_001, 2_001).expect("valid rect");
        let geometry = final_geometry(node.clone(), world, Some(world));
        let candidate = PreparedViewRenderCandidate::prepare(41, [&geometry])
            .expect("exact test geometry must lower");
        let prepared = only_node(&candidate);

        assert_eq!(candidate.generation(), 41);
        assert_eq!(prepared.node(), &node);
        assert_eq!(prepared.final_revision(), geometry.final_revision);
        assert_eq!(
            prepared.world_border_box().left.to_bits(),
            (-1.501_f32).to_bits()
        );
        assert_eq!(
            prepared.world_border_box().top.to_bits(),
            0.999_f32.to_bits()
        );
        assert_eq!(prepared.raster_bounds().left_px, -2);
        assert_eq!(prepared.raster_bounds().top_px, 0);
        assert_eq!(prepared.raster_bounds().right_px, 2);
        assert_eq!(prepared.raster_bounds().bottom_px, 3);
        assert_eq!(prepared.raster_bounds().width_px, 4);
        assert_eq!(prepared.raster_bounds().height_px, 3);
        assert!(matches!(
            prepared.descendant_clip(),
            PreparedViewClip::NonEmpty { .. }
        ));
    }

    #[test]
    fn fully_clipped_node_is_absent_from_render_candidate() {
        let world = ViewGeometryRect::new(0, 0, 1_000, 1_000).expect("valid rect");
        let geometry = final_geometry(node(23), world, None);
        let candidate = PreparedViewRenderCandidate::prepare(5, [&geometry])
            .expect("clipped nodes require no conversion");
        assert!(candidate.nodes().is_empty());
    }

    #[test]
    fn inexact_f32_preserves_node_consumer_field_and_round_trip() {
        let node = node(29);
        let world = ViewGeometryRect::new(0, 0, i32::MAX, 1_000).expect("valid rect");
        let geometry = final_geometry(node.clone(), world, Some(world));
        let error = PreparedViewRenderCandidate::prepare(8, [&geometry])
            .expect_err("large milli endpoint is not exactly representable as f32");
        assert!(matches!(
            error,
            ViewFinalGeometryLoweringError::InexactF32 {
                node: actual_node,
                consumer: geometry::ViewGeometryConsumer::Paint,
                field: ViewFinalGeometryField::Right,
                value_milli,
                round_trip_milli,
            } if actual_node == node
                && value_milli == i64::from(i32::MAX)
                && round_trip_milli != value_milli
        ));
    }

    #[test]
    fn index_range_uses_checked_target_maximum() {
        let error = checked_index(None, geometry::ViewGeometryConsumer::Paint, u64::MAX)
            .expect_err("usize maximum must not fit the WGPU u32 index domain");
        assert_eq!(
            error,
            ViewFinalGeometryLoweringError::IndexRange {
                node: None,
                consumer: geometry::ViewGeometryConsumer::Paint,
                field: ViewFinalGeometryField::IndexRange,
                value: u64::MAX,
                max: u64::from(u32::MAX),
            }
        );
    }
}
