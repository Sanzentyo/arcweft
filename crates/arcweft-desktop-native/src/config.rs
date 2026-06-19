use arcweft_desktop_contract::KnownDirectory;
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum GlobalPointerPolicy {
    #[default]
    Disabled,
    Observe,
    Control,
}

impl GlobalPointerPolicy {
    pub const fn allows_observe(self) -> bool {
        matches!(self, Self::Observe | Self::Control)
    }

    pub const fn allows_control(self) -> bool {
        matches!(self, Self::Control)
    }
}

/// Explicit host policy. High-authority features default to disabled.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NativeDesktopOptions {
    pub allowed_known_directories: BTreeSet<KnownDirectory>,
    pub global_pointer: GlobalPointerPolicy,
    pub external_window_observe: bool,
}

impl NativeDesktopOptions {
    #[must_use]
    pub fn allow_known_directory(mut self, directory: KnownDirectory) -> Self {
        self.allowed_known_directories.insert(directory);
        self
    }

    #[must_use]
    pub const fn with_global_pointer(mut self, policy: GlobalPointerPolicy) -> Self {
        self.global_pointer = policy;
        self
    }

    #[must_use]
    pub const fn with_external_window_observe(mut self, enabled: bool) -> Self {
        self.external_window_observe = enabled;
        self
    }
}
