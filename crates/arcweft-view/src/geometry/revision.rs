use super::{
    ViewBoxPlacement, ViewGeometryClip, ViewGeometryClipAxis, ViewGeometryPoint, ViewGeometryRect,
    ViewGeometryTransform, ViewIntrinsicMeasure, ViewMeasuredBox, ViewOuterSize, ViewPaintOutsets,
    ViewStyleNodeKey,
};
use crate::style::{
    ViewDisplay, ViewLengthMilli, ViewPhysicalBoxStyle, ViewPhysicalContainerStyle,
};

const REVISION_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
const REVISION_PRIME: u64 = 0x0000_0100_0000_01b3;
const MEASURE_DOMAIN: &[u8] = b"arcweft.view-geometry.measure.v1";
const OUTER_MEASURE_DOMAIN: &[u8] = b"arcweft.view-geometry.outer-measure.v1";
const PLACED_DOMAIN: &[u8] = b"arcweft.view-geometry.placed.v1";
const FINAL_DOMAIN: &[u8] = b"arcweft.view-geometry.final.v1";

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryMeasureStyleRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewIntrinsicMeasureRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMeasuredGeometryRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewOuterMeasureRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewGeometryPlaceStyleRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPlacedGeometryRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFinalGeometryRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPaintOutsetsRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewViewportGeometryRevision(u64);

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewScrollStateRevision(u64);

impl ViewGeometryMeasureStyleRevision {
    pub fn for_style(
        physical_box: &ViewPhysicalBoxStyle,
        container: Option<ViewPhysicalContainerStyle>,
    ) -> Self {
        let mut transcript = RevisionTranscript::new(MEASURE_DOMAIN);
        transcript.option_u8(physical_box.display.map(ViewDisplay::canonical_tag));
        transcript.option_i32(physical_box.width.map(ViewLengthMilli::value));
        transcript.option_i32(physical_box.height.map(ViewLengthMilli::value));
        transcript.option_i32(physical_box.min_width.map(ViewLengthMilli::value));
        transcript.option_i32(physical_box.min_height.map(ViewLengthMilli::value));
        transcript.option_i32(physical_box.max_width.map(ViewLengthMilli::value));
        transcript.option_i32(physical_box.max_height.map(ViewLengthMilli::value));
        for value in [
            physical_box.padding.top.value(),
            physical_box.padding.right.value(),
            physical_box.padding.bottom.value(),
            physical_box.padding.left.value(),
            physical_box.border.top.value(),
            physical_box.border.right.value(),
            physical_box.border.bottom.value(),
            physical_box.border.left.value(),
            physical_box.margin.top.value(),
            physical_box.margin.right.value(),
            physical_box.margin.bottom.value(),
            physical_box.margin.left.value(),
        ] {
            transcript.i32(value);
        }
        match container {
            Some(container) => {
                transcript.u8(1);
                transcript.u8(container.flow.canonical_tag());
                transcript.i32(container.row_gap.value());
                transcript.i32(container.column_gap.value());
            }
            None => transcript.u8(0),
        }
        Self(transcript.finish())
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewIntrinsicMeasureRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewMeasuredGeometryRevision {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewOuterMeasureRevision {
    pub fn for_measured(
        measured: ViewMeasuredGeometryRevision,
        width_milli: u32,
        height_milli: u32,
    ) -> Self {
        let mut transcript = RevisionTranscript::new(OUTER_MEASURE_DOMAIN);
        transcript.u64(measured.value());
        transcript.u32(width_milli);
        transcript.u32(height_milli);
        Self(transcript.finish())
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewGeometryPlaceStyleRevision {
    pub fn for_style(physical: &ViewPhysicalBoxStyle) -> Self {
        let mut transcript = RevisionTranscript::new(PLACED_DOMAIN);
        transcript.u8(physical.position.canonical_tag());
        for value in [
            physical.inset.top.map(ViewLengthMilli::value),
            physical.inset.right.map(ViewLengthMilli::value),
            physical.inset.bottom.map(ViewLengthMilli::value),
            physical.inset.left.map(ViewLengthMilli::value),
        ] {
            transcript.option_i32(value);
        }
        for value in [
            physical.margin.top.value(),
            physical.margin.right.value(),
            physical.margin.bottom.value(),
            physical.margin.left.value(),
            physical.translate_x.value(),
            physical.translate_y.value(),
        ] {
            transcript.i32(value);
        }
        transcript.u32(physical.scale.value());
        transcript.u8(physical.overflow_x.canonical_tag());
        transcript.u8(physical.overflow_y.canonical_tag());
        Self(transcript.finish())
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewPlacedGeometryRevision {
    /// Deterministic containing-block revision for the root mount viewport.
    pub fn for_root_viewport(viewport: ViewViewportGeometryRevision) -> Self {
        let mut transcript = RevisionTranscript::new(PLACED_DOMAIN);
        transcript.u8(0);
        transcript.u64(viewport.value());
        Self(transcript.finish())
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewFinalGeometryRevision {
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewPaintOutsetsRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewViewportGeometryRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

impl ViewScrollStateRevision {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewAvailableGeometrySize {
    pub width_milli: Option<u32>,
    pub height_milli: Option<u32>,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewChildOuterDependency {
    pub node: ViewStyleNodeKey,
    pub outer_size: ViewOuterSize,
    pub revision: ViewOuterMeasureRevision,
}

/// Exact measured-cache identity. Revision equality never substitutes for this value.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMeasuredGeometryKey {
    pub node: ViewStyleNodeKey,
    pub box_style: ViewPhysicalBoxStyle,
    pub container_style: Option<ViewPhysicalContainerStyle>,
    pub intrinsic: ViewIntrinsicMeasure,
    pub available: ViewAvailableGeometrySize,
    pub ordered_children: Vec<ViewChildOuterDependency>,
}

impl ViewMeasuredGeometryKey {
    pub fn revision(&self) -> ViewMeasuredGeometryRevision {
        let mut transcript = RevisionTranscript::new(MEASURE_DOMAIN);
        transcript.node(&self.node);
        transcript.physical_box(&self.box_style);
        transcript.physical_container(self.container_style);
        transcript.intrinsic(self.intrinsic);
        transcript.option_u32(self.available.width_milli);
        transcript.option_u32(self.available.height_milli);
        transcript.u64(self.ordered_children.len() as u64);
        for child in &self.ordered_children {
            transcript.node(&child.node);
            transcript.outer_size(child.outer_size);
            transcript.u64(child.revision.value());
        }
        ViewMeasuredGeometryRevision(transcript.finish())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPlacedDependency {
    pub node: ViewStyleNodeKey,
    pub placement: ViewBoxPlacement,
    pub revision: ViewPlacedGeometryRevision,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewContainingBlockDependency {
    pub node: Option<ViewStyleNodeKey>,
    pub rect: ViewGeometryRect,
    pub revision: ViewPlacedGeometryRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewViewportGeometryInput {
    pub rect: ViewGeometryRect,
    pub revision: ViewViewportGeometryRevision,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewScrollStateInput {
    pub x_milli: i32,
    pub y_milli: i32,
    pub revision: ViewScrollStateRevision,
}

/// Exact placement-cache identity without a parent/child final-revision cycle.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPlacedGeometryKey {
    pub node: ViewStyleNodeKey,
    pub measured: ViewMeasuredBox,
    pub box_style: ViewPhysicalBoxStyle,
    pub containing_block: ViewContainingBlockDependency,
    pub static_border_origin: ViewGeometryPoint,
    pub parent: Option<ViewPlacedDependency>,
    pub previous_flow_sibling: Option<ViewPlacedDependency>,
    pub viewport: ViewViewportGeometryInput,
    pub scroll: ViewScrollStateInput,
}

impl ViewPlacedGeometryKey {
    pub fn revision(&self) -> ViewPlacedGeometryRevision {
        let mut transcript = RevisionTranscript::new(PLACED_DOMAIN);
        transcript.node(&self.node);
        transcript.measured(self.measured);
        transcript.physical_box(&self.box_style);
        transcript.option_node(self.containing_block.node.as_ref());
        transcript.rect(self.containing_block.rect);
        transcript.u64(self.containing_block.revision.value());
        transcript.point(self.static_border_origin);
        transcript.placed_dependency(self.parent.as_ref());
        transcript.placed_dependency(self.previous_flow_sibling.as_ref());
        transcript.rect(self.viewport.rect);
        transcript.u64(self.viewport.revision.value());
        transcript.i32(self.scroll.x_milli);
        transcript.i32(self.scroll.y_milli);
        transcript.u64(self.scroll.revision.value());
        ViewPlacedGeometryRevision(transcript.finish())
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewTransformDependency {
    pub node: ViewStyleNodeKey,
    pub transform: ViewGeometryTransform,
    pub placed_revision: ViewPlacedGeometryRevision,
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewChildFinalDependency {
    pub node: ViewStyleNodeKey,
    pub world_border_box: ViewGeometryRect,
    pub layout_subtree_bounds: ViewGeometryRect,
    pub paint_subtree_bounds: Option<ViewGeometryRect>,
    pub descendant_clip: ViewGeometryClip,
    pub revision: ViewFinalGeometryRevision,
}

/// Exact final-cache identity after placement, child aggregation, and paint outsets.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFinalGeometryKey {
    pub node: ViewStyleNodeKey,
    pub placement: ViewBoxPlacement,
    pub box_style: ViewPhysicalBoxStyle,
    pub transform_chain: Vec<ViewTransformDependency>,
    pub inherited_clip: ViewGeometryClip,
    pub paint_outsets: ViewPaintOutsets,
    pub scroll: ViewScrollStateInput,
    pub ordered_children: Vec<ViewChildFinalDependency>,
}

impl ViewFinalGeometryKey {
    pub fn revision(&self) -> ViewFinalGeometryRevision {
        let mut transcript = RevisionTranscript::new(FINAL_DOMAIN);
        transcript.node(&self.node);
        transcript.placement(self.placement);
        transcript.physical_box(&self.box_style);
        transcript.u64(self.transform_chain.len() as u64);
        for dependency in &self.transform_chain {
            transcript.node(&dependency.node);
            transcript.transform(dependency.transform);
            transcript.u64(dependency.placed_revision.value());
        }
        transcript.clip(self.inherited_clip);
        for edge in [
            self.paint_outsets.edges.top,
            self.paint_outsets.edges.right,
            self.paint_outsets.edges.bottom,
            self.paint_outsets.edges.left,
        ] {
            transcript.u32(edge);
        }
        transcript.u64(self.paint_outsets.revision.value());
        transcript.i32(self.scroll.x_milli);
        transcript.i32(self.scroll.y_milli);
        transcript.u64(self.scroll.revision.value());
        transcript.u64(self.ordered_children.len() as u64);
        for child in &self.ordered_children {
            transcript.node(&child.node);
            transcript.rect(child.world_border_box);
            transcript.rect(child.layout_subtree_bounds);
            transcript.option_rect(child.paint_subtree_bounds);
            transcript.clip(child.descendant_clip);
            transcript.u64(child.revision.value());
        }
        ViewFinalGeometryRevision(transcript.finish())
    }
}

pub(super) fn measured_revision(
    node: &ViewStyleNodeKey,
    style: &ViewPhysicalBoxStyle,
    intrinsic: ViewIntrinsicMeasure,
    used_width_milli: u32,
    used_height_milli: u32,
) -> ViewMeasuredGeometryRevision {
    let mut transcript = RevisionTranscript::new(MEASURE_DOMAIN);
    transcript.node(node);
    transcript.u64(intrinsic.revision.value());
    transcript.u32(intrinsic.content_size.width_milli);
    transcript.u32(intrinsic.content_size.height_milli);
    transcript.u64(ViewGeometryMeasureStyleRevision::for_style(style, None).value());
    transcript.u32(used_width_milli);
    transcript.u32(used_height_milli);
    ViewMeasuredGeometryRevision(transcript.finish())
}

struct RevisionTranscript {
    value: u64,
}

impl RevisionTranscript {
    fn new(domain: &[u8]) -> Self {
        let mut transcript = Self {
            value: REVISION_OFFSET_BASIS,
        };
        transcript.length_prefixed(domain);
        transcript
    }

    fn node(&mut self, node: &ViewStyleNodeKey) {
        self.u64(node.mount().get());
        self.u64(node.path().len() as u64);
        for segment in node.path() {
            self.u64(*segment);
        }
        self.u32(node.instruction());
    }

    fn option_node(&mut self, node: Option<&ViewStyleNodeKey>) {
        match node {
            Some(node) => {
                self.u8(1);
                self.node(node);
            }
            None => self.u8(0),
        }
    }

    fn point(&mut self, point: ViewGeometryPoint) {
        self.i32(point.x_milli);
        self.i32(point.y_milli);
    }

    fn rect(&mut self, rect: ViewGeometryRect) {
        self.i32(rect.left_milli);
        self.i32(rect.top_milli);
        self.i32(rect.right_milli);
        self.i32(rect.bottom_milli);
    }

    fn option_rect(&mut self, rect: Option<ViewGeometryRect>) {
        match rect {
            Some(rect) => {
                self.u8(1);
                self.rect(rect);
            }
            None => self.u8(0),
        }
    }

    fn outer_size(&mut self, outer: ViewOuterSize) {
        self.u32(outer.width_milli);
        self.u32(outer.height_milli);
    }

    fn intrinsic(&mut self, intrinsic: ViewIntrinsicMeasure) {
        self.u32(intrinsic.content_size.width_milli);
        self.u32(intrinsic.content_size.height_milli);
        self.u64(intrinsic.revision.value());
    }

    fn measured(&mut self, measured: ViewMeasuredBox) {
        for axis in [measured.x, measured.y] {
            self.u32(axis.natural_border_extent_milli);
            self.u32(axis.used_border_extent_milli);
            self.u32(axis.edge_extent_milli);
            self.option_u32(axis.min_milli);
            self.option_u32(axis.max_milli);
            self.u8(u8::from(axis.auto));
        }
        self.u32(measured.content_size.width_milli);
        self.u32(measured.content_size.height_milli);
        for edge in [
            measured.padding.top,
            measured.padding.right,
            measured.padding.bottom,
            measured.padding.left,
            measured.border.top,
            measured.border.right,
            measured.border.bottom,
            measured.border.left,
        ] {
            self.u32(edge);
        }
        for edge in [
            measured.margin.top,
            measured.margin.right,
            measured.margin.bottom,
            measured.margin.left,
        ] {
            self.i32(edge);
        }
        self.u64(measured.revision.value());
    }

    fn placement(&mut self, placement: ViewBoxPlacement) {
        self.rect(placement.content_box);
        self.rect(placement.padding_box);
        self.rect(placement.border_box);
        self.rect(placement.margin_box);
    }

    fn placed_dependency(&mut self, dependency: Option<&ViewPlacedDependency>) {
        match dependency {
            Some(dependency) => {
                self.u8(1);
                self.node(&dependency.node);
                self.placement(dependency.placement);
                self.u64(dependency.revision.value());
            }
            None => self.u8(0),
        }
    }

    fn transform(&mut self, transform: ViewGeometryTransform) {
        self.rect(transform.border_box);
        self.point(transform.translate);
        self.u32(transform.scale.value());
    }

    fn clip(&mut self, clip: ViewGeometryClip) {
        let Some(axes) = clip.axes() else {
            self.u8(0);
            return;
        };
        self.u8(1);
        self.clip_axis(axes.x());
        self.clip_axis(axes.y());
    }

    fn clip_axis(&mut self, axis: ViewGeometryClipAxis) {
        match axis {
            ViewGeometryClipAxis::Unbounded => self.u8(0),
            ViewGeometryClipAxis::Bounded(span) => {
                self.u8(1);
                self.i32(span.start_milli);
                self.i32(span.end_milli);
            }
        }
    }

    fn physical_box(&mut self, physical: &ViewPhysicalBoxStyle) {
        self.u8(physical.axes.canonical_tag());
        self.option_u8(physical.display.map(ViewDisplay::canonical_tag));
        self.u8(physical.position.canonical_tag());
        for value in [
            physical.width,
            physical.height,
            physical.min_width,
            physical.min_height,
            physical.max_width,
            physical.max_height,
        ] {
            self.option_i32(value.map(ViewLengthMilli::value));
        }
        for value in [
            physical.padding.top,
            physical.padding.right,
            physical.padding.bottom,
            physical.padding.left,
            physical.border.top,
            physical.border.right,
            physical.border.bottom,
            physical.border.left,
            physical.margin.top,
            physical.margin.right,
            physical.margin.bottom,
            physical.margin.left,
        ] {
            self.i32(value.value());
        }
        for value in [
            physical.inset.top,
            physical.inset.right,
            physical.inset.bottom,
            physical.inset.left,
        ] {
            self.option_i32(value.map(ViewLengthMilli::value));
        }
        self.i32(physical.translate_x.value());
        self.i32(physical.translate_y.value());
        self.u32(physical.scale.value());
        self.u8(physical.overflow_x.canonical_tag());
        self.u8(physical.overflow_y.canonical_tag());
    }

    fn physical_container(&mut self, container: Option<ViewPhysicalContainerStyle>) {
        match container {
            Some(container) => {
                self.u8(1);
                self.u8(container.flow.canonical_tag());
                self.i32(container.row_gap.value());
                self.i32(container.column_gap.value());
            }
            None => self.u8(0),
        }
    }

    fn bytes(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.value ^= u64::from(*byte);
            self.value = self.value.wrapping_mul(REVISION_PRIME);
        }
    }

    fn length_prefixed(&mut self, bytes: &[u8]) {
        self.u64(bytes.len() as u64);
        self.bytes(bytes);
    }

    fn u8(&mut self, value: u8) {
        self.bytes(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes(&value.to_le_bytes());
    }

    fn i32(&mut self, value: i32) {
        self.bytes(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes(&value.to_le_bytes());
    }

    fn option_u8(&mut self, value: Option<u8>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u8(value);
            }
            None => self.u8(0),
        }
    }

    fn option_i32(&mut self, value: Option<i32>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.i32(value);
            }
            None => self.u8(0),
        }
    }

    fn option_u32(&mut self, value: Option<u32>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u32(value);
            }
            None => self.u8(0),
        }
    }

    const fn finish(self) -> u64 {
        self.value
    }
}
