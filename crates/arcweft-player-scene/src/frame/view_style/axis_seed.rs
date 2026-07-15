//! Runtime-output adaptation for the canonical retained axis seed chain.

use super::{PlayerFrameError, ResolvedNode};
use arcweft_runtime_driver::view_runtime::{BundleViewMountOutput, BundleViewStyleNode};
use arcweft_view::style::ViewInheritedBoxAxes;

pub(super) fn validate_mount_seed_shape(
    mount: &BundleViewMountOutput,
) -> Result<(), PlayerFrameError> {
    if !mount.path.segments().is_empty() && mount.host_axis_seed.is_some() {
        return Err(PlayerFrameError::UnexpectedHostAxisSeed { mount: mount.mount });
    }
    Ok(())
}

pub(super) fn inherited_axes(
    mount: &BundleViewMountOutput,
    node: &BundleViewStyleNode,
    parent: Option<&ResolvedNode>,
) -> Result<ViewInheritedBoxAxes, PlayerFrameError> {
    parent.map_or_else(
        || {
            mount
                .host_axis_seed
                .ok_or(PlayerFrameError::MissingHostAxisSeed {
                    mount: mount.mount,
                    instruction: node.instruction,
                })
        },
        |parent| Ok(parent.computed.axes().inherited_snapshot()),
    )
}
