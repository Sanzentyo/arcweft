//! Stable identities for externally owned deterministic runtime programs.

use core::fmt;
use serde::{Deserialize, Serialize};

/// Stable identity of one deterministic pure program bound into a runtime
/// artifact by a domain owner.
///
/// The identity is opaque to runtime consumers. They compare the checked bytes
/// directly and never derive it from a display label or source spelling.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimePureProgramId([u8; 32]);

impl RuntimePureProgramId {
    #[must_use]
    pub const fn from_checked_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for RuntimePureProgramId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}
