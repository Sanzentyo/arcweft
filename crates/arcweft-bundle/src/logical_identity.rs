//! Complete identity for the typed bundle model before container encoding.

use crate::container::BundleDigest;
use crate::{ArcweftBundle, BundleCodecError};

/// Identity of the complete typed bundle model, independent of its source
/// serialization format.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(transparent)]
pub struct LogicalBundleIdentity(BundleDigest);

impl LogicalBundleIdentity {
    #[must_use]
    pub const fn digest(self) -> BundleDigest {
        self.0
    }
}

impl ArcweftBundle {
    /// Hashes the complete validated typed bundle through its deterministic
    /// JSON codec. Unlike an AWFB content root, this includes manifest and
    /// source metadata as well as executable and presentation resources.
    pub fn logical_identity(&self) -> Result<LogicalBundleIdentity, BundleCodecError> {
        const DOMAIN: &[u8] = b"arcweft.logical-bundle.v1\0";
        let bytes = self.to_json_bytes()?;
        let mut transcript = Vec::with_capacity(DOMAIN.len() + bytes.len());
        transcript.extend_from_slice(DOMAIN);
        transcript.extend_from_slice(&bytes);
        Ok(LogicalBundleIdentity(BundleDigest::of(&transcript)))
    }
}
