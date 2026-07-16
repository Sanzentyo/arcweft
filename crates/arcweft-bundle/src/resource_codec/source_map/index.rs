use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::BundleSource;
use crate::container::BundleDigest;

/// Provisional source key used by source-bearing View products until their
/// source-reference table is migrated to `ProductSourceId`.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceMapSourceId(String);

/// Indexed metadata used by the current source-bound View product validator.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapIndex {
    entries: BTreeMap<SourceMapSourceId, SourceMapEntry>,
    utf8_boundaries: BTreeMap<SourceMapSourceId, Box<[usize]>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMapEntry {
    digest: BundleDigest,
    utf8_len: usize,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceMapSourceIdError {
    #[error("source-map identity must not be empty")]
    Empty,
    #[error("source-map identity must not contain NUL")]
    ContainsNul,
}

impl SourceMapSourceId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SourceMapSourceIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SourceMapSourceIdError::Empty);
        }
        if value.contains('\0') {
            return Err(SourceMapSourceIdError::ContainsNul);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl SourceMapIndex {
    pub fn from_source(source: &BundleSource) -> Result<Self, SourceMapSourceIdError> {
        let id = SourceMapSourceId::try_new(source.label.clone())?;
        let entry = SourceMapEntry {
            digest: BundleDigest::of(source.text.as_bytes()),
            utf8_len: source.text.len(),
        };
        let boundaries = source
            .text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(source.text.len()))
            .collect::<Box<[_]>>();
        Ok(Self {
            entries: BTreeMap::from([(id.clone(), entry)]),
            utf8_boundaries: BTreeMap::from([(id, boundaries)]),
        })
    }

    pub fn entry(&self, id: &SourceMapSourceId) -> Option<SourceMapEntry> {
        self.entries.get(id).copied()
    }

    pub fn is_utf8_boundary(&self, id: &SourceMapSourceId, offset: usize) -> Option<bool> {
        self.utf8_boundaries
            .get(id)
            .map(|boundaries| boundaries.binary_search(&offset).is_ok())
    }
}

impl SourceMapEntry {
    pub const fn digest(self) -> BundleDigest {
        self.digest
    }

    pub const fn utf8_len(self) -> usize {
        self.utf8_len
    }
}

impl Serialize for SourceMapSourceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SourceMapSourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::try_new(value).map_err(serde::de::Error::custom)
    }
}
