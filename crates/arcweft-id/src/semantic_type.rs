//! Stable identity shared by checked semantic types and runtime artifacts.

use serde::{Deserialize, Serialize};

/// Stable semantic identity for a checked type after alias and projection
/// normalization.
///
/// This lower-layer value is the one identity carried by semantic analysis,
/// runtime plans, AWBC, bundles, and domain schemas. Consumers compare it
/// directly and never recreate it from display names or executable shapes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeSemanticTypeId([u8; 32]);

impl RuntimeSemanticTypeId {
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn from_semantic_digest(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
