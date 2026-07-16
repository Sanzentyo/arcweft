use super::{ViewGeometryNodeId, ViewIntrinsicMeasure};
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

/// Exact measured-cache identity. Hash equality never substitutes for key equality.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewMeasuredGeometryKey {
    pub node: ViewGeometryNodeId,
    pub measure_style_revision: ViewGeometryMeasureStyleRevision,
    pub intrinsic_revision: ViewIntrinsicMeasureRevision,
    pub available_width_milli: Option<u32>,
    pub available_height_milli: Option<u32>,
    pub ordered_child_outer_revisions: Vec<ViewOuterMeasureRevision>,
}

impl ViewMeasuredGeometryKey {
    pub fn revision(&self) -> ViewMeasuredGeometryRevision {
        let mut transcript = RevisionTranscript::new(MEASURE_DOMAIN);
        transcript.node(&self.node);
        transcript.u64(self.measure_style_revision.value());
        transcript.u64(self.intrinsic_revision.value());
        transcript.option_u32(self.available_width_milli);
        transcript.option_u32(self.available_height_milli);
        transcript.u64(self.ordered_child_outer_revisions.len() as u64);
        for revision in &self.ordered_child_outer_revisions {
            transcript.u64(revision.value());
        }
        ViewMeasuredGeometryRevision(transcript.finish())
    }
}

/// Exact placement-cache identity without a parent/child final-revision cycle.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewPlacedGeometryKey {
    pub node: ViewGeometryNodeId,
    pub measured_revision: ViewMeasuredGeometryRevision,
    pub place_style_revision: ViewGeometryPlaceStyleRevision,
    pub parent_placed_revision: Option<ViewPlacedGeometryRevision>,
    pub containing_block_revision: ViewPlacedGeometryRevision,
    pub previous_flow_sibling_revision: Option<ViewPlacedGeometryRevision>,
    pub viewport_revision: ViewViewportGeometryRevision,
    pub scroll_state_revision: ViewScrollStateRevision,
}

impl ViewPlacedGeometryKey {
    pub fn revision(&self) -> ViewPlacedGeometryRevision {
        let mut transcript = RevisionTranscript::new(PLACED_DOMAIN);
        transcript.node(&self.node);
        transcript.u64(self.measured_revision.value());
        transcript.u64(self.place_style_revision.value());
        transcript.option_u64(
            self.parent_placed_revision
                .map(ViewPlacedGeometryRevision::value),
        );
        transcript.u64(self.containing_block_revision.value());
        transcript.option_u64(
            self.previous_flow_sibling_revision
                .map(ViewPlacedGeometryRevision::value),
        );
        transcript.u64(self.viewport_revision.value());
        transcript.u64(self.scroll_state_revision.value());
        ViewPlacedGeometryRevision(transcript.finish())
    }
}

/// Exact final-cache identity after placement, child aggregation, and paint outsets.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewFinalGeometryKey {
    pub placed_revision: ViewPlacedGeometryRevision,
    pub visual_outsets_revision: ViewPaintOutsetsRevision,
    pub ordered_child_final_revisions: Vec<ViewFinalGeometryRevision>,
}

impl ViewFinalGeometryKey {
    pub fn revision(&self) -> ViewFinalGeometryRevision {
        let mut transcript = RevisionTranscript::new(FINAL_DOMAIN);
        transcript.u64(self.placed_revision.value());
        transcript.u64(self.visual_outsets_revision.value());
        transcript.u64(self.ordered_child_final_revisions.len() as u64);
        for revision in &self.ordered_child_final_revisions {
            transcript.u64(revision.value());
        }
        ViewFinalGeometryRevision(transcript.finish())
    }
}

pub(super) fn measured_revision(
    node: &ViewGeometryNodeId,
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

    fn node(&mut self, node: &ViewGeometryNodeId) {
        self.u64(node.mount().get());
        self.u64(node.path().len() as u64);
        for segment in node.path() {
            self.u64(*segment);
        }
        self.u32(node.instruction());
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

    fn option_u64(&mut self, value: Option<u64>) {
        match value {
            Some(value) => {
                self.u8(1);
                self.u64(value);
            }
            None => self.u8(0),
        }
    }

    const fn finish(self) -> u64 {
        self.value
    }
}
