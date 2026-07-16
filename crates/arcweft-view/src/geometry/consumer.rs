use super::primitives::checked_i32;
use super::{
    ViewGeometryClip, ViewGeometryError, ViewGeometryNodeId, ViewGeometryOperation,
    ViewGeometryRect, ViewGeometrySpan,
};
use crate::style::{ViewOverflow, ViewPhysicalAxis};

/// Shared final bounds for every geometry consumer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewConsumerGeometry {
    pub visible_border_box: Option<ViewGeometryRect>,
    pub hit_bounds: Option<ViewGeometryRect>,
    pub focus_target_bounds: Option<ViewGeometryRect>,
    pub avoidance_bounds: Option<ViewGeometryRect>,
    pub scroll_target_bounds: Option<ViewGeometryRect>,
    pub paint_bounds: Option<ViewGeometryRect>,
}

pub fn consumer_geometry(
    world_border_box: ViewGeometryRect,
    world_paint_box: ViewGeometryRect,
    ancestor_clip: ViewGeometryClip,
) -> ViewConsumerGeometry {
    let visible = ancestor_clip.clip_rect(world_border_box);
    ViewConsumerGeometry {
        visible_border_box: visible,
        hit_bounds: visible,
        focus_target_bounds: visible,
        avoidance_bounds: visible,
        scroll_target_bounds: visible,
        paint_bounds: ancestor_clip.clip_rect(world_paint_box),
    }
}

/// Physical scrolling supported by one overflow axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ViewScrollCapability {
    None,
    Programmatic,
    UserAndProgrammatic,
}

/// Signed scroll range and current offset for one physical axis.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewScrollAxisGeometry {
    pub capability: ViewScrollCapability,
    pub viewport: ViewGeometrySpan,
    pub content: ViewGeometrySpan,
    pub min_offset_milli: i32,
    pub max_offset_milli: i32,
    pub current_offset_milli: i32,
}

/// Signed scroll geometry for both physical axes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewScrollGeometry {
    pub x: ViewScrollAxisGeometry,
    pub y: ViewScrollAxisGeometry,
}

pub fn scroll_axis_geometry(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    overflow: ViewOverflow,
    viewport: ViewGeometrySpan,
    content: ViewGeometrySpan,
    current_offset_milli: i32,
) -> Result<ViewScrollAxisGeometry, ViewGeometryError> {
    let before = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::ScrollRange,
        i64::from(content.start_milli) - i64::from(viewport.start_milli),
    )?;
    let after = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::ScrollRange,
        i64::from(content.end_milli) - i64::from(viewport.end_milli),
    )?;
    let (min_offset_milli, max_offset_milli) =
        if matches!(overflow, ViewOverflow::Visible | ViewOverflow::Clip) {
            (0, 0)
        } else {
            (before.min(0), after.max(0))
        };
    if current_offset_milli < min_offset_milli || current_offset_milli > max_offset_milli {
        return Err(ViewGeometryError::ScrollOffsetOutOfRange {
            node: node.clone(),
            axis,
            current_milli: current_offset_milli,
            min_milli: min_offset_milli,
            max_milli: max_offset_milli,
        });
    }
    let has_range = min_offset_milli != 0 || max_offset_milli != 0;
    Ok(ViewScrollAxisGeometry {
        capability: overflow.scroll_capability(has_range),
        viewport,
        content,
        min_offset_milli,
        max_offset_milli,
        current_offset_milli,
    })
}

pub fn scroll_into_view_nearest(
    node: &ViewGeometryNodeId,
    axis: ViewPhysicalAxis,
    geometry: ViewScrollAxisGeometry,
    target_unscrolled: ViewGeometrySpan,
) -> Result<i32, ViewGeometryError> {
    let visible_start = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::Subtract,
        i64::from(target_unscrolled.start_milli) - i64::from(geometry.current_offset_milli),
    )?;
    let visible_end = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::Subtract,
        i64::from(target_unscrolled.end_milli) - i64::from(geometry.current_offset_milli),
    )?;
    let delta = if visible_start < geometry.viewport.start_milli {
        i64::from(visible_start) - i64::from(geometry.viewport.start_milli)
    } else if visible_end > geometry.viewport.end_milli {
        i64::from(visible_end) - i64::from(geometry.viewport.end_milli)
    } else {
        0
    };
    let requested = checked_i32(
        node,
        Some(axis),
        ViewGeometryOperation::Add,
        i64::from(geometry.current_offset_milli) + delta,
    )?;
    Ok(requested.clamp(geometry.min_offset_milli, geometry.max_offset_milli))
}
