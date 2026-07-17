//! Checked physical box geometry shared by native View consumers.
//!
//! Authoritative coordinates are signed integer milli-pixels. Logical-axis
//! projection is complete before values enter this module.

mod box_model;
mod consumer;
mod error;
mod primitives;
mod revision;

pub use box_model::{
    ViewBoxPlacement, ViewIntrinsicMeasure, ViewMeasuredAxis, ViewMeasuredBox, ViewOuterSize,
    ViewPlacedAxis, first_flow_border_start, first_reverse_flow_border_start, flow_intrinsic_size,
    measure_box, next_flow_border_start, next_reverse_flow_border_start, outer_size, place_axis,
    place_box,
};
pub use consumer::{
    ViewConsumerGeometry, ViewScrollAxisGeometry, ViewScrollCapability, ViewScrollGeometry,
    consumer_geometry, scroll_axis_geometry, scroll_into_view_nearest,
};
pub use error::{
    ViewGeometryConsumer, ViewGeometryError, ViewGeometryField, ViewGeometryOperation,
    ViewGeometryPropertySupport, ViewPointerCoordinateErrorKind, ViewRepresentedGeometryFeature,
    validate_supported_properties,
};
pub use primitives::{
    ViewGeometryClip, ViewGeometryClipAxes, ViewGeometryClipAxis, ViewGeometryPoint,
    ViewGeometryRasterRect, ViewGeometryRect, ViewGeometrySize, ViewGeometrySpan,
    ViewGeometryTransform, milli_from_logical_pointer, transform_chain, transform_rect,
    union_rects,
};
pub use revision::{
    ViewAvailableGeometrySize, ViewChildFinalDependency, ViewChildOuterDependency,
    ViewContainingBlockDependency, ViewFinalGeometryKey, ViewFinalGeometryRevision,
    ViewGeometryMeasureStyleRevision, ViewGeometryPlaceStyleRevision, ViewIntrinsicMeasureRevision,
    ViewMeasuredGeometryKey, ViewMeasuredGeometryRevision, ViewOuterMeasureRevision,
    ViewPaintOutsetsRevision, ViewPlacedDependency, ViewPlacedGeometryKey,
    ViewPlacedGeometryRevision, ViewScrollStateInput, ViewScrollStateRevision,
    ViewTransformDependency, ViewViewportGeometryInput, ViewViewportGeometryRevision,
};

use crate::style::{ViewPhysicalEdges, ViewStyleNodeKey};

/// Exact non-negative visual outsets supplied by a paint owner.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPaintOutsets {
    pub edges: ViewPhysicalEdges<u32>,
    pub revision: ViewPaintOutsetsRevision,
}

/// Geometry consumed by paint, input, focus, avoidance, scrolling, and capture.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ViewFinalGeometry {
    pub node: ViewStyleNodeKey,
    pub content_box: ViewGeometryRect,
    pub padding_box: ViewGeometryRect,
    pub border_box: ViewGeometryRect,
    pub margin_box: ViewGeometryRect,
    pub world_border_box: ViewGeometryRect,
    pub descendant_clip: ViewGeometryClip,
    pub consumers: ViewConsumerGeometry,
    pub layout_subtree_bounds: ViewGeometryRect,
    pub paint_subtree_bounds: Option<ViewGeometryRect>,
    pub scroll: ViewScrollGeometry,
    pub measured_revision: ViewMeasuredGeometryRevision,
    pub placed_revision: ViewPlacedGeometryRevision,
    pub final_revision: ViewFinalGeometryRevision,
}
