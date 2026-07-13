use super::table::PublicIdRef;
use crate::container::{BundleDigest, SectionId, SectionKindCode};

/// Stable 128-bit resource identity derived from an owner-defined stable key.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
    serde::Deserialize,
    serde::Serialize,
)]
#[serde(transparent)]
pub struct StableId([u8; 16]);

/// Digest reference used by resource sections when they point at stable content.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct DigestRef {
    pub digest: BundleDigest,
}

/// Source range reference. Byte offsets are UTF-8 byte positions in the
/// referenced normalized source, not character or line-column pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRangeRef {
    pub source: PublicIdRef,
    pub start_byte: u32,
    pub end_byte: u32,
}

/// Cross-section reference. The section kind code is raw so future optional
/// section kinds can still participate in deterministic resource identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct CrossSectionRef {
    pub section_kind: SectionKindCode,
    pub section_id: SectionId,
    pub content_digest: BundleDigest,
    pub public_id: Option<PublicIdRef>,
}

impl StableId {
    pub fn for_key(key: &str) -> Self {
        let digest = BundleDigest::of(key.as_bytes()).as_bytes();
        let mut bytes = [0_u8; 16];
        bytes.copy_from_slice(&digest[..16]);
        Self(bytes)
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }
}
