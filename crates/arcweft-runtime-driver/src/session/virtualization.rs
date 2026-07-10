//! Session ownership for retained virtual-list mounts.

use super::BundleSession;
use arcweft_bundle::resource_codec::ViewRuntimeScrollRegion;
use arcweft_view::program::ViewVirtualAxis;
use arcweft_view::virtualization::{
    ViewMountId, ViewVirtualItem, ViewVirtualList, ViewVirtualScrollTarget,
    ViewVirtualizationError, ViewVirtualizationRuntime,
};
use thiserror::Error;

/// Error while connecting a retained virtual list to an authored Scroll.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum BundleVirtualListMountError {
    #[error("unknown authored Scroll target `{target}` for virtual list mount")]
    UnknownScrollTarget { target: String },
    #[error(
        "virtual list axis {list_axis:?} does not match Scroll target `{target}` axis {scroll_axis:?}"
    )]
    AxisMismatch {
        target: String,
        list_axis: ViewVirtualAxis,
        scroll_axis: ViewVirtualAxis,
    },
    #[error(transparent)]
    Virtualization(#[from] ViewVirtualizationError),
}

impl BundleSession {
    /// Current retained virtual-list state for renderer, Agent, and save adapters.
    pub const fn view_virtualization(&self) -> &ViewVirtualizationRuntime {
        &self.view_virtualization
    }

    /// Registers one independently mounted virtual list occurrence.
    pub fn mount_virtual_list(
        &mut self,
        scroll_target: ViewVirtualScrollTarget,
        axis: ViewVirtualAxis,
        viewport_extent_milli: u32,
        items: Vec<ViewVirtualItem>,
    ) -> Result<ViewMountId, BundleVirtualListMountError> {
        validate_virtual_list_scroll_owner(&self.scroll_regions, &scroll_target, axis)?;
        self.view_virtualization
            .mount(scroll_target, axis, viewport_extent_milli, items)
            .map_err(Into::into)
    }

    /// Removes one mounted virtual list and its range-planning state.
    pub fn unmount_virtual_list(&mut self, mount: ViewMountId) -> Option<ViewVirtualList> {
        self.view_virtualization.unmount(mount)
    }

    /// Mutably accesses one mount for scrolling or exact finite-source updates.
    pub fn virtual_list_mut(&mut self, mount: ViewMountId) -> Option<&mut ViewVirtualList> {
        self.view_virtualization.get_mut(mount)
    }
}

pub(super) fn validate_virtual_list_scroll_owner(
    scroll_regions: &[ViewRuntimeScrollRegion],
    scroll_target: &ViewVirtualScrollTarget,
    axis: ViewVirtualAxis,
) -> Result<(), BundleVirtualListMountError> {
    let scroll_axis = scroll_regions
        .iter()
        .find(|region| region.target == scroll_target.as_str())
        .map(|region| region.axis.virtual_axis())
        .ok_or_else(|| BundleVirtualListMountError::UnknownScrollTarget {
            target: scroll_target.as_str().to_owned(),
        })?;
    if scroll_axis != axis {
        return Err(BundleVirtualListMountError::AxisMismatch {
            target: scroll_target.as_str().to_owned(),
            list_axis: axis,
            scroll_axis,
        });
    }
    Ok(())
}
