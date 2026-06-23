use super::{
    lock::{CacheLock, CacheLockError},
    record::{CacheRecord, CacheRecordError},
};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKind},
    fingerprint::BuildDigest,
    incremental::QueryKind,
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Filesystem object and record cache rooted under `target/arcweft/cache/v1`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilesystemCacheStore {
    root: PathBuf,
}

/// Filesystem cache store failure.
#[derive(Debug, Error)]
pub enum CacheStoreError {
    #[error("failed to create cache directory `{path}`: {source}")]
    CreateDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write cache temp file `{path}`: {source}")]
    WriteTemp {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to publish cache file `{from}` to `{to}`: {source}")]
    Publish {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read cache file `{path}`: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("cache object `{path}` digest mismatch: expected {expected}, actual {actual}")]
    ObjectDigestMismatch {
        path: PathBuf,
        expected: BuildDigest,
        actual: BuildDigest,
    },
    #[error("cache object `{path}` length mismatch: expected {expected}, actual {actual}")]
    ObjectLengthMismatch {
        path: PathBuf,
        expected: u64,
        actual: u64,
    },
    #[error("cache artifact is too large to record its byte length")]
    ArtifactTooLarge,
    #[error(transparent)]
    Record(#[from] CacheRecordError),
    #[error(transparent)]
    Lock(#[from] CacheLockError),
}

impl FilesystemCacheStore {
    /// Creates a cache store rooted at `root`.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Acquires a package-level lock file under `locks`.
    pub fn lock_package(&self, package: &str) -> Result<CacheLock, CacheStoreError> {
        CacheLock::acquire(
            self.root.join("locks").join(format!("{package}.lock")),
            package,
        )
        .map_err(CacheStoreError::from)
    }

    /// Stores immutable object bytes and returns their digest.
    pub fn put_object(&self, bytes: &[u8]) -> Result<BuildDigest, CacheStoreError> {
        let digest = BuildDigest::of(bytes);
        let path = self.object_path(digest);
        write_immutable(&path, bytes)?;
        Ok(digest)
    }

    /// Reads and verifies immutable object bytes.
    pub fn read_object(&self, digest: BuildDigest) -> Result<Vec<u8>, CacheStoreError> {
        let path = self.object_path(digest);
        let bytes = read_file(&path)?;
        let actual = BuildDigest::of(&bytes);
        if actual != digest {
            return Err(CacheStoreError::ObjectDigestMismatch {
                path,
                expected: digest,
                actual,
            });
        }
        Ok(bytes)
    }

    /// Stores object bytes and an immutable record for one artifact key.
    pub fn store_artifact(
        &self,
        query: QueryKind,
        key: ArtifactKey,
        artifact_kind: ArtifactKind,
        bytes: &[u8],
    ) -> Result<CacheRecord, CacheStoreError> {
        self.store_artifact_with_logical_item(query, key, artifact_kind, None, bytes)
    }

    /// Stores object bytes and an immutable record with a logical lookup label.
    pub fn store_artifact_with_logical_item(
        &self,
        query: QueryKind,
        key: ArtifactKey,
        artifact_kind: ArtifactKind,
        logical_item: Option<&str>,
        bytes: &[u8],
    ) -> Result<CacheRecord, CacheStoreError> {
        let object_digest = self.put_object(bytes)?;
        let object_len =
            u64::try_from(bytes.len()).map_err(|_| CacheStoreError::ArtifactTooLarge)?;
        let record = logical_item.map_or_else(
            || CacheRecord::new(key, artifact_kind, object_digest, object_len),
            |logical_item| {
                CacheRecord::with_logical_item(
                    key,
                    artifact_kind,
                    logical_item,
                    object_digest,
                    object_len,
                )
            },
        );
        let record_bytes = record.to_bytes()?;
        write_immutable(&self.record_path(query, key), &record_bytes)?;
        Ok(record)
    }

    /// Reads a record for an artifact key.
    pub fn read_record(
        &self,
        query: QueryKind,
        key: ArtifactKey,
    ) -> Result<CacheRecord, CacheStoreError> {
        let bytes = read_file(&self.record_path(query, key))?;
        CacheRecord::from_slice_for_key(key, &bytes).map_err(CacheStoreError::from)
    }

    /// Reads record and object bytes, verifying digest and length.
    pub fn read_artifact(
        &self,
        query: QueryKind,
        key: ArtifactKey,
    ) -> Result<Option<Vec<u8>>, CacheStoreError> {
        let record_path = self.record_path(query, key);
        if !record_path.is_file() {
            return Ok(None);
        }
        let record = self.read_record(query, key)?;
        let bytes = self.read_object(record.object_digest())?;
        let actual_len =
            u64::try_from(bytes.len()).map_err(|_| CacheStoreError::ArtifactTooLarge)?;
        if actual_len != record.object_len() {
            return Err(CacheStoreError::ObjectLengthMismatch {
                path: self.object_path(record.object_digest()),
                expected: record.object_len(),
                actual: actual_len,
            });
        }
        Ok(Some(bytes))
    }

    fn object_path(&self, digest: BuildDigest) -> PathBuf {
        let hex = digest.to_hex();
        self.root
            .join("objects")
            .join("blake3")
            .join(&hex[..2])
            .join(&hex[2..])
    }

    fn record_path(&self, query: QueryKind, key: ArtifactKey) -> PathBuf {
        let hex = key.digest().to_hex();
        self.root()
            .join("records")
            .join(query.cache_namespace())
            .join(&hex[..2])
            .join(format!("{}.awci", &hex[2..]))
    }
}

fn read_file(path: &Path) -> Result<Vec<u8>, CacheStoreError> {
    let mut file =
        OpenOptions::new()
            .read(true)
            .open(path)
            .map_err(|source| CacheStoreError::Read {
                path: path.to_path_buf(),
                source,
            })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|source| CacheStoreError::Read {
            path: path.to_path_buf(),
            source,
        })?;
    Ok(bytes)
}

fn write_immutable(path: &Path, bytes: &[u8]) -> Result<(), CacheStoreError> {
    if path.is_file() {
        return Ok(());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| CacheStoreError::CreateDir {
        path: parent.to_path_buf(),
        source,
    })?;
    let tmp = temp_path(parent);
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)
            .map_err(|source| CacheStoreError::WriteTemp {
                path: tmp.clone(),
                source,
            })?;
        file.write_all(bytes)
            .and_then(|()| file.sync_all())
            .map_err(|source| CacheStoreError::WriteTemp {
                path: tmp.clone(),
                source,
            })?;
    }
    match fs::rename(&tmp, path) {
        Ok(()) => Ok(()),
        Err(_source) if path.is_file() => {
            let _ = fs::remove_file(&tmp);
            Ok(())
        }
        Err(source) => Err(CacheStoreError::Publish {
            from: tmp,
            to: path.to_path_buf(),
            source,
        }),
    }
}

fn temp_path(parent: &Path) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    parent.join(format!(".tmp-{}-{nanos}.awci", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::FilesystemCacheStore;
    use arcweft_project::{
        artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
        fingerprint::BuildDigest,
        incremental::QueryKind,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(label: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-cache-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

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
    fn store_artifact_round_trips_verified_bytes() {
        let store = FilesystemCacheStore::new(temp_root("round-trip"));
        let key = key();
        store
            .store_artifact(
                QueryKind::Parse,
                key,
                ArtifactKind::ParsedSyntax,
                b"artifact",
            )
            .expect("artifact stored");

        assert_eq!(
            store
                .read_artifact(QueryKind::Parse, key)
                .expect("artifact read"),
            Some(b"artifact".to_vec())
        );
    }

    #[test]
    fn missing_record_is_cache_miss() {
        let store = FilesystemCacheStore::new(temp_root("miss"));
        assert_eq!(
            store
                .read_artifact(QueryKind::Parse, key())
                .expect("cache miss"),
            None
        );
    }
}
