//! Typed identity and indexed validation metadata for bundled source maps.

use crate::BundleSource;
use crate::container::BundleDigest;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;
use thiserror::Error;

/// Stable product identity for one normalized source document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceMapSourceId(String);

/// Indexed metadata derived from the canonical encoded source-map section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapIndex {
    entries: BTreeMap<SourceMapSourceId, SourceMapEntry>,
}

/// Digest and normalized UTF-8 extent for one source document.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceMapEntry {
    digest: BundleDigest,
    utf8_len: usize,
}

/// Invalid product source identity.
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
        Ok(Self {
            entries: BTreeMap::from([(id, entry)]),
        })
    }

    pub fn entry(&self, id: &SourceMapSourceId) -> Option<SourceMapEntry> {
        self.entries.get(id).copied()
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
