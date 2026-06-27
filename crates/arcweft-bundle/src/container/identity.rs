use super::{BundleDigest, BundleKind, CONTAINER_VERSION};

/// Logical AWFB artifact identity including manifest bytes and content root.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct ArtifactIdentity {
    pub container_version: u32,
    pub kind: BundleKind,
    pub content_root: BundleDigest,
    pub manifest_digest: BundleDigest,
}

impl ArtifactIdentity {
    #[must_use]
    pub const fn new(
        container_version: u32,
        kind: BundleKind,
        content_root: BundleDigest,
        manifest_digest: BundleDigest,
    ) -> Self {
        Self {
            container_version,
            kind,
            content_root,
            manifest_digest,
        }
    }

    #[must_use]
    pub fn for_current_container(
        kind: BundleKind,
        content_root: BundleDigest,
        manifest_digest: BundleDigest,
    ) -> Self {
        Self::new(CONTAINER_VERSION, kind, content_root, manifest_digest)
    }

    #[must_use]
    pub fn digest(self) -> BundleDigest {
        let mut transcript = Vec::with_capacity(8 + 4 + 4 + 32 + 32);
        transcript.extend_from_slice(b"arcweft.artifact-identity.v1\0");
        transcript.extend_from_slice(&self.container_version.to_le_bytes());
        transcript.extend_from_slice(&self.kind.encoded().to_le_bytes());
        transcript.extend_from_slice(&self.content_root.as_bytes());
        transcript.extend_from_slice(&self.manifest_digest.as_bytes());
        BundleDigest::of(&transcript)
    }
}
