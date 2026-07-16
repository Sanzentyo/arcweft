//! Aggregate producer handle for all windowed player live-update channels.

use crate::windowed_environment_ingress::WindowedEnvironmentIngress;
use crate::windowed_ingress::WindowedPatchIngress;

/// Cloneable aggregate passed to native player ingress configuration callbacks.
#[derive(Clone)]
pub struct WindowedPlayerIngress {
    patches: WindowedPatchIngress,
    environment: WindowedEnvironmentIngress,
}

impl WindowedPlayerIngress {
    pub(crate) const fn new(
        patches: WindowedPatchIngress,
        environment: WindowedEnvironmentIngress,
    ) -> Self {
        Self {
            patches,
            environment,
        }
    }

    /// Patch-specific producer API.
    pub const fn patches(&self) -> &WindowedPatchIngress {
        &self.patches
    }

    /// Presentation-environment producer API.
    pub const fn environment(&self) -> &WindowedEnvironmentIngress {
        &self.environment
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn aggregate_keeps_patch_and_environment_as_distinct_real_handles() {
        assert!(size_of::<WindowedPlayerIngress>() >= size_of::<WindowedPatchIngress>());
        assert_ne!(
            std::any::TypeId::of::<WindowedPatchIngress>(),
            std::any::TypeId::of::<WindowedEnvironmentIngress>()
        );
    }
}
