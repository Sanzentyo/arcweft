//! Complete identity of the typed bundle or encoded AWFB artifact that owns
//! one program generation.

use crate::container::{ArtifactIdentity, BundleDigest};
use crate::logical_identity::LogicalBundleIdentity;
use arcweft_core::task::GenerationId;
use arcweft_core::value::{
    RuntimeBundleAssetArtifactDigest, RuntimeBundleAssetContext, RuntimeBundleAssetValueError,
};

/// Identity of the complete bundle artifact that owns runtime resources.
///
/// The variants are deliberately distinct: a logical in-memory bundle and an
/// encoded AWFB container have different complete identities even when they
/// decode to equivalent models.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BundleArtifactIdentity {
    LogicalBundle { identity: LogicalBundleIdentity },
    AwfbContainer { identity: ArtifactIdentity },
}

impl BundleArtifactIdentity {
    /// Returns the complete artifact identity when this is an encoded AWFB.
    #[must_use]
    pub const fn awfb_container(self) -> Option<ArtifactIdentity> {
        match self {
            Self::AwfbContainer { identity } => Some(identity),
            Self::LogicalBundle { .. } => None,
        }
    }

    /// Returns the domain-tagged digest used to bind generation-owned assets.
    #[must_use]
    pub fn binding_digest(self) -> BundleDigest {
        const DOMAIN: &[u8] = b"arcweft.bundle-artifact-binding.v1\0";
        let (tag, identity_digest) = match self {
            Self::LogicalBundle { identity } => (0_u8, identity.digest()),
            Self::AwfbContainer { identity } => (1_u8, identity.digest()),
        };
        let mut transcript = Vec::with_capacity(DOMAIN.len() + 1 + 32);
        transcript.extend_from_slice(DOMAIN);
        transcript.push(tag);
        transcript.extend_from_slice(&identity_digest.as_bytes());
        BundleDigest::of(&transcript)
    }

    /// Binds this complete artifact identity to one runtime generation for
    /// bundle-owned resource handles and task dispatches.
    pub fn bundle_asset_context(
        self,
        generation: GenerationId,
    ) -> Result<RuntimeBundleAssetContext, RuntimeBundleAssetValueError> {
        let artifact =
            RuntimeBundleAssetArtifactDigest::try_from_bytes(self.binding_digest().as_bytes())?;
        Ok(RuntimeBundleAssetContext::new(generation, artifact))
    }
}

#[cfg(test)]
mod tests {
    use super::BundleArtifactIdentity;
    use crate::container::{ArtifactIdentity, BundleDigest, BundleKind};
    use crate::logical_identity::LogicalBundleIdentity;

    #[test]
    fn awfb_identity_has_one_variant_tagged_asset_binding_digest() {
        let identity = ArtifactIdentity::for_current_container(
            BundleKind::Program,
            BundleDigest::of(b"content root"),
            BundleDigest::of(b"manifest"),
        );
        let artifact = BundleArtifactIdentity::AwfbContainer { identity };
        let mut transcript = b"arcweft.bundle-artifact-binding.v1\0".to_vec();
        transcript.push(1);
        transcript.extend_from_slice(&identity.digest().as_bytes());

        assert_eq!(artifact.binding_digest(), BundleDigest::of(&transcript));
        assert_eq!(artifact.awfb_container(), Some(identity));

        let logical_identity: LogicalBundleIdentity = serde_json::from_value(
            serde_json::to_value([7_u8; 32]).expect("logical identity digest encodes"),
        )
        .expect("logical identity digest decodes");
        assert_ne!(
            BundleArtifactIdentity::LogicalBundle {
                identity: logical_identity,
            }
            .binding_digest(),
            artifact.binding_digest()
        );
    }
}
