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
    /// Hashes the complete validated typed bundle. Unlike an AWFB content
    /// root, this includes manifest and source metadata as well as executable
    /// and presentation resources that have a dedicated compact AWFB wire.
    pub fn logical_identity(&self) -> Result<LogicalBundleIdentity, BundleCodecError> {
        const DOMAIN: &[u8] = b"arcweft.logical-bundle.v1\0";
        let bytes = self.to_json_bytes()?;
        let mut transcript = Vec::with_capacity(DOMAIN.len() + bytes.len() + 73);
        transcript.extend_from_slice(DOMAIN);
        let byte_len = u64::try_from(bytes.len())
            .map_err(|_| BundleCodecError::LogicalIdentityLengthOverflow)?;
        transcript.extend_from_slice(&byte_len.to_le_bytes());
        transcript.extend_from_slice(&bytes);
        match self.character_presentation.as_ref() {
            None => transcript.push(0),
            Some(catalog) => {
                transcript.push(1);
                transcript.extend_from_slice(catalog.semantic_digest().as_bytes());
                transcript.extend_from_slice(catalog.locale_policy_digest().as_bytes());
            }
        }
        Ok(LogicalBundleIdentity(BundleDigest::of(&transcript)))
    }
}
