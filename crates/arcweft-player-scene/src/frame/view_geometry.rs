//! Player-owned retained View geometry orchestration.

mod cache;
mod conversion;
mod error;
mod finalize;
mod intrinsic;
mod measure;
mod place;
#[cfg(test)]
mod tests;
mod tree;

pub use cache::{ViewCommittedGeometryFrame, ViewGeometryGeneration};
pub use conversion::{
    ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform,
};
pub use error::{
    ViewGeometryFailure, ViewGeometryFailureCode, ViewGeometryFailureField,
    ViewGeometryFailureGeneration, ViewGeometryFailureRange, ViewGeometryFailureRect,
    ViewGeometryRuntimeError,
};

pub(crate) use cache::{PlayerViewGeometryState, ViewGeometryPreparedFrame};
pub(crate) use conversion::{exact_f32, viewport_input};
pub(crate) use error::{ViewGeometryProductKind, ViewGeometryTargetKey};
pub(crate) use intrinsic::{PresentationIntrinsicGeometryProvider, ViewIntrinsicGeometryProvider};

use crate::frame::view_style::ResolvedViewStyleFrame;
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use arcweft_runtime_driver::view_runtime::BundleViewFrame;
use arcweft_view::geometry::{
    ViewGeometryError, ViewGeometryOperation, ViewPaintOutsets, ViewScrollStateInput,
    ViewViewportGeometryInput,
};
use arcweft_view::style::ViewPhysicalEdges;
use arcweft_view::style::ViewStyleNodeKey;
use std::collections::BTreeMap;

/// Current scroll input indexed by retained occurrence identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ViewScrollStateSnapshot {
    values: BTreeMap<ViewStyleNodeKey, ViewScrollStateInput>,
}

impl ViewScrollStateSnapshot {
    pub(crate) fn insert(&mut self, node: ViewStyleNodeKey, value: ViewScrollStateInput) {
        self.values.insert(node, value);
    }

    pub(crate) fn get(&self, node: &ViewStyleNodeKey) -> ViewScrollStateInput {
        self.values
            .get(node)
            .copied()
            .unwrap_or(ViewScrollStateInput {
                x_milli: 0,
                y_milli: 0,
                revision: arcweft_view::geometry::ViewScrollStateRevision::new(0),
            })
    }

    pub(crate) fn from_frame(
        frame: &BundleViewFrame,
        input: &crate::input::InputController,
    ) -> Result<Self, ViewGeometryRuntimeError> {
        let mut snapshot = Self::default();
        for mount in &frame.mounts {
            for node in &mount.style_nodes {
                let arcweft_runtime_driver::view_runtime::BundleViewStyleNodeKind::Element {
                    element: arcweft_view::ViewElementKind::Scroll,
                    target: Some(target),
                } = &node.kind
                else {
                    continue;
                };
                let target = mount.scoped_id(target);
                let key = node.style_node_key(mount.mount);
                let x_milli = arcweft_view::geometry::milli_from_logical_pointer(f64::from(
                    input.scroll_offset_x(&target),
                ))?;
                let y_milli = arcweft_view::geometry::milli_from_logical_pointer(f64::from(
                    input.scroll_offset_y(&target),
                ))?;
                let revision = scroll_revision(x_milli, y_milli);
                snapshot.insert(
                    key,
                    ViewScrollStateInput {
                        x_milli,
                        y_milli,
                        revision,
                    },
                );
            }
        }
        Ok(snapshot)
    }
}

/// Supported paint outsets indexed by retained occurrence identity.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ViewPaintOutsetSnapshot {
    values: BTreeMap<ViewStyleNodeKey, ViewPaintOutsets>,
}

impl ViewPaintOutsetSnapshot {
    pub(crate) fn insert(&mut self, node: ViewStyleNodeKey, value: ViewPaintOutsets) {
        self.values.insert(node, value);
    }

    pub(crate) fn get(&self, node: &ViewStyleNodeKey) -> ViewPaintOutsets {
        self.values.get(node).copied().unwrap_or_default()
    }

    pub(crate) fn from_styles(
        styles: &ResolvedViewStyleFrame,
    ) -> Result<Self, ViewGeometryRuntimeError> {
        let mut snapshot = Self::default();
        for (node, style) in styles.nodes() {
            let focus = style.visual().focus_ring.map_or(Ok(0), |ring| {
                ring.offset_milli
                    .max(0)
                    .unsigned_abs()
                    .checked_add(ring.width_milli)
                    .ok_or_else(|| ViewGeometryError::ArithmeticOverflow {
                        node: node.clone(),
                        axis: None,
                        operation: ViewGeometryOperation::Outset,
                    })
            })?;
            let corner = style.visual().corner_frame.map_or(Ok(0), |frame| {
                frame
                    .offset_milli
                    .max(0)
                    .unsigned_abs()
                    .checked_add(frame.width_milli)
                    .ok_or_else(|| ViewGeometryError::ArithmeticOverflow {
                        node: node.clone(),
                        axis: None,
                        operation: ViewGeometryOperation::Outset,
                    })
            })?;
            let outset = focus.max(corner);
            let revision = paint_revision(focus, corner);
            snapshot.insert(
                node.clone(),
                ViewPaintOutsets {
                    edges: ViewPhysicalEdges::all(outset),
                    revision,
                },
            );
        }
        Ok(snapshot)
    }
}

fn scroll_revision(x_milli: i32, y_milli: i32) -> arcweft_view::geometry::ViewScrollStateRevision {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in x_milli
        .to_le_bytes()
        .into_iter()
        .chain(y_milli.to_le_bytes())
    {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    arcweft_view::geometry::ViewScrollStateRevision::new(value)
}

fn paint_revision(focus: u32, corner: u32) -> arcweft_view::geometry::ViewPaintOutsetsRevision {
    let mut value = 0xcbf2_9ce4_8422_2325_u64;
    for byte in focus.to_le_bytes().into_iter().chain(corner.to_le_bytes()) {
        value ^= u64::from(byte);
        value = value.wrapping_mul(0x0000_0100_0000_01b3);
    }
    arcweft_view::geometry::ViewPaintOutsetsRevision::new(value)
}

/// Complete immutable input for one retained geometry candidate.
#[derive(Clone, Copy)]
pub(crate) struct ViewGeometryFrameInput<'a> {
    pub frame: &'a BundleViewFrame,
    pub styles: &'a ResolvedViewStyleFrame,
    pub presentation: &'a BundlePresentationSnapshot,
    pub viewport: ViewViewportGeometryInput,
    pub scroll: &'a ViewScrollStateSnapshot,
    pub paint_outsets: &'a ViewPaintOutsetSnapshot,
}

/// Builds a complete staged geometry frame without mutating committed state.
pub(crate) fn prepare_view_geometry(
    state: &PlayerViewGeometryState,
    input: ViewGeometryFrameInput<'_>,
    intrinsic: &mut dyn ViewIntrinsicGeometryProvider,
) -> Result<ViewGeometryPreparedFrame, ViewGeometryRuntimeError> {
    let inventory = tree::build_inventory(&input)?;
    let measured = measure::measure_inventory(state, &inventory, &input, intrinsic)?;
    let placed = place::place_inventory(state, &inventory, &input, &measured)?;
    finalize::finalize_inventory(state, inventory, &input, measured, placed)
}
