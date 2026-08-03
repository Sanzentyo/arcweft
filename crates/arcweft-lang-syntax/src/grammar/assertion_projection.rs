//! Parser-owned semantic projection for assertion statements.

use crate::assertion::AssertionMode;

/// Canonical assertion mode selected by the parser, or typed recovery.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct PendingAssertionProjection {
    mode: Option<AssertionMode>,
}

impl PendingAssertionProjection {
    pub(crate) const fn new(mode: Option<AssertionMode>) -> Self {
        Self { mode }
    }

    pub(crate) const fn mode(self) -> Option<AssertionMode> {
        self.mode
    }

    pub(crate) const fn has_recovery(self) -> bool {
        self.mode.is_none()
    }
}
