use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKind},
    fingerprint::BuildDigest,
    incremental::CACHE_SCHEMA_VERSION,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// On-disk cache record mapping one artifact key to immutable object bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct CacheRecord {
    schema_version: u32,
    key: ArtifactKey,
    artifact_kind: ArtifactKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    logical_item: Option<String>,
    object_digest: BuildDigest,
    object_len: u64,
}

/// Cache record decode or validation failure.
#[derive(Debug, Error)]
pub enum CacheRecordError {
    #[error("failed to encode Arcweft cache record: {0}")]
    Encode(#[from] serde_json::Error),
    #[error("cache record schema version {actual} is not supported; expected {expected}")]
    UnsupportedSchema { actual: u32, expected: u32 },
    #[error("cache record key does not match requested artifact key")]
    KeyMismatch,
}

impl CacheRecord {
    /// Creates a cache record for immutable object bytes.
    pub const fn new(
        key: ArtifactKey,
        artifact_kind: ArtifactKind,
        object_digest: BuildDigest,
        object_len: u64,
    ) -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            key,
            artifact_kind,
            logical_item: None,
            object_digest,
            object_len,
        }
    }

    pub fn with_logical_item(
        key: ArtifactKey,
        artifact_kind: ArtifactKind,
        logical_item: impl Into<String>,
        object_digest: BuildDigest,
        object_len: u64,
    ) -> Self {
        Self {
            schema_version: CACHE_SCHEMA_VERSION,
            key,
            artifact_kind,
            logical_item: Some(logical_item.into()),
            object_digest,
            object_len,
        }
    }

    pub const fn schema_version(&self) -> u32 {
        self.schema_version
    }

    pub const fn key(&self) -> ArtifactKey {
        self.key
    }

    pub const fn artifact_kind(&self) -> ArtifactKind {
        self.artifact_kind
    }

    pub fn logical_item(&self) -> Option<&str> {
        self.logical_item.as_deref()
    }

    pub const fn object_digest(&self) -> BuildDigest {
        self.object_digest
    }

    pub const fn object_len(&self) -> u64 {
        self.object_len
    }

    /// Encodes as deterministic pretty JSON for local inspection.
    pub fn to_bytes(&self) -> Result<Vec<u8>, CacheRecordError> {
        serde_json::to_vec_pretty(self).map_err(CacheRecordError::Encode)
    }

    /// Decodes and validates a record for the requested artifact key.
    pub fn from_slice_for_key(key: ArtifactKey, bytes: &[u8]) -> Result<Self, CacheRecordError> {
        let record = Self::from_slice(bytes)?;
        if record.key != key {
            return Err(CacheRecordError::KeyMismatch);
        }
        Ok(record)
    }

    /// Decodes and validates a record without requiring the caller to already
    /// know the artifact key.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, CacheRecordError> {
        let record: Self = serde_json::from_slice(bytes).map_err(CacheRecordError::Encode)?;
        if record.schema_version != CACHE_SCHEMA_VERSION {
            return Err(CacheRecordError::UnsupportedSchema {
                actual: record.schema_version,
                expected: CACHE_SCHEMA_VERSION,
            });
        }
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::CacheRecord;
    use arcweft_project::{
        artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
        fingerprint::BuildDigest,
        incremental::QueryKind,
    };

    fn key() -> ArtifactKey {
        ArtifactKey::derive(&ArtifactKeyInput {
            compiler_build_id: "compiler".to_owned(),
            query: QueryKind::Parse,
            artifact_kind: ArtifactKind::ParsedSyntax,
            target_triple: "native".to_owned(),
            target_features: Vec::new(),
            profile: "dev".to_owned(),
            package: "pkg".to_owned(),
            logical_item: "crate".to_owned(),
            source_digest: BuildDigest::of(b"source"),
            dependency_interface_digests: Vec::new(),
            dependency_body_digests: Vec::new(),
            adapter_environment_digest: BuildDigest::ZERO,
            launch_profile_digest: BuildDigest::ZERO,
            declared_environment_digest: BuildDigest::ZERO,
            format_options_digest: BuildDigest::ZERO,
        })
    }

    #[test]
    fn cache_record_round_trips_for_key() {
        let key = key();
        let record = CacheRecord::new(
            key,
            ArtifactKind::ParsedSyntax,
            BuildDigest::of(b"object"),
            6,
        );
        let bytes = record.to_bytes().expect("record encodes");

        assert_eq!(
            CacheRecord::from_slice_for_key(key, &bytes).expect("record decodes"),
            record
        );
    }
}
