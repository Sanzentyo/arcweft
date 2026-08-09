use super::table::PublicIdRef;
use crate::container::{BundleDigest, SectionId, SectionKindCode};
use arcweft_source::ProductSourceRef;
use serde::{Deserialize, Serialize};
use thiserror::Error;

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

/// Opaque index into one resource's canonical product-source table.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProductSourceRefIndex(u32);

/// Source range reference. Byte offsets are UTF-8 byte positions in the
/// referenced normalized source, not character or line-column pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceRangeRef {
    source: ProductSourceRefIndex,
    start_byte: u32,
    end_byte: u32,
}

/// Failure to construct a typed product source table or range.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewProductBuildError {
    #[error("product source table contains more entries than a u32 index can address")]
    TooManySourceRefs,
    #[error("source range references a source absent from its product source table")]
    UnknownSource,
    #[error("source reference index {index} is out of bounds for {count} sources")]
    InvalidSourceIndex { index: u32, count: usize },
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

impl ProductSourceRefIndex {
    pub(crate) fn try_from_index(index: usize) -> Result<Self, ViewProductBuildError> {
        u32::try_from(index)
            .map(Self)
            .map_err(|_| ViewProductBuildError::TooManySourceRefs)
    }

    pub const fn value(self) -> u32 {
        self.0
    }

    pub(crate) const fn index(self) -> usize {
        self.0 as usize
    }
}

impl SourceRangeRef {
    pub fn try_for_source(
        source_refs: &[ProductSourceRef],
        source: &ProductSourceRef,
        start_byte: u32,
        end_byte: u32,
    ) -> Result<Self, ViewProductBuildError> {
        let index = source_refs
            .iter()
            .position(|candidate| candidate == source)
            .ok_or(ViewProductBuildError::UnknownSource)?;
        Ok(Self::new(
            ProductSourceRefIndex::try_from_index(index)?,
            start_byte,
            end_byte,
        ))
    }

    pub(crate) const fn new(source: ProductSourceRefIndex, start_byte: u32, end_byte: u32) -> Self {
        Self {
            source,
            start_byte,
            end_byte,
        }
    }

    pub const fn source(&self) -> ProductSourceRefIndex {
        self.source
    }

    pub const fn start_byte(&self) -> u32 {
        self.start_byte
    }

    pub const fn end_byte(&self) -> u32 {
        self.end_byte
    }

    pub(crate) fn set_source(&mut self, source: ProductSourceRefIndex) {
        self.source = source;
    }
}
