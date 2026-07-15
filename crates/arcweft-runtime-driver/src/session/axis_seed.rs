//! Public session facade for root View logical-axis host seeds.

use super::BundleSession;
use crate::presentation_handles::PresentationHandleId;
use crate::view_runtime::{
    BundleViewAxisSeedError, BundleViewAxisSeedUpdate, BundleViewAxisSeedUpdateOutcome,
};
use arcweft_view::{ViewBoxAxisHostSeed, ViewInheritedBoxAxes};

impl BundleSession {
    /// Reserves the typed seed consumed by this handle's next root View mount.
    pub fn configure_next_view_axis_seed(
        &mut self,
        handle: PresentationHandleId,
        seed: ViewBoxAxisHostSeed,
    ) -> Result<(), BundleViewAxisSeedError> {
        self.view_runtime.configure_next_axis_seed(
            handle,
            seed,
            &self.presentation.presentation_handles,
        )
    }

    /// Cancels a pending next-root-mount reservation, if one exists.
    pub fn cancel_next_view_axis_seed(
        &mut self,
        handle: &PresentationHandleId,
    ) -> Option<ViewBoxAxisHostSeed> {
        self.view_runtime.cancel_next_axis_seed(handle)
    }

    /// Applies one revision-checked update to a live top-level View mount.
    pub fn update_view_axis_seed(
        &mut self,
        update: BundleViewAxisSeedUpdate,
    ) -> Result<BundleViewAxisSeedUpdateOutcome, BundleViewAxisSeedError> {
        let outcome = self.view_runtime.update_axis_seed(update)?;
        if let BundleViewAxisSeedUpdateOutcome::Updated { current, .. } = outcome {
            synchronize_visible_root_seed(&mut self.presentation, update.mount, current);
        }
        Ok(outcome)
    }
}

fn synchronize_visible_root_seed(
    presentation: &mut crate::display::BundlePresentationSnapshot,
    mount: arcweft_view::ViewMountId,
    current: ViewInheritedBoxAxes,
) {
    let Some(output) = presentation
        .view
        .mounts
        .iter_mut()
        .find(|output| output.mount == mount && output.path.segments().is_empty())
    else {
        return;
    };
    if output.host_axis_seed != Some(current) {
        output.host_axis_seed = Some(current);
        presentation.revision = presentation.revision.saturating_add(1);
    }
}
