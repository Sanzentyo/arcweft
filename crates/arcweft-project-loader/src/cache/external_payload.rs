use super::store::{CacheStoreError, FilesystemCacheStore};
use arcweft_bundle::{
    container::{BundleDigest, SectionId},
    release::{
        ReleaseMirror,
        archive::{
            AwfrArchiveError, AwfrArchiveManifest, ExternalPayloadCarrier,
            ExternalPayloadDescriptorKey,
        },
    },
};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
    fingerprint::BuildDigest,
    incremental::QueryKind,
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Result of fetching one AWFR external payload into the local cache.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExternalPayloadCacheFetchReport {
    pub archive: String,
    pub cache_root: String,
    pub bundle_content_root: String,
    pub descriptor_id: String,
    pub compressed_digest: String,
    pub decoded_digest: String,
    pub decoded_size: u64,
    pub compressed_size: u64,
    pub status: ExternalPayloadCacheFetchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_key: Option<String>,
    pub attempts: Vec<ExternalPayloadCacheFetchAttempt>,
}

/// Fetched external payload bytes plus cache report.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExternalPayloadCacheFetchBytes {
    pub report: ExternalPayloadCacheFetchReport,
    pub compressed_bytes: Vec<u8>,
    pub decoded_bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPayloadCacheFetchStatus {
    CacheHit,
    Fetched,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExternalPayloadCacheFetchAttempt {
    pub uri: String,
    pub status: ExternalPayloadCacheFetchAttemptStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExternalPayloadCacheFetchAttemptStatus {
    CacheMiss,
    Fetched,
    Hit,
    SkippedUnsupportedScheme,
    Failed,
}

#[derive(Debug, Error)]
pub enum ExternalPayloadCacheFetchError {
    #[error("failed to read AWFR archive `{path}`: {source}")]
    ReadArchive {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read external payload mirror `{path}`: {source}")]
    ReadMirror {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Archive(#[from] AwfrArchiveError),
    #[error(transparent)]
    Cache(#[from] CacheStoreError),
    #[error("AWFR archive has no external payload carrier for {0:?}")]
    MissingCarrier(ExternalPayloadDescriptorKey),
    #[error("external payload carrier has no usable local or cache mirror for {0:?}")]
    NoUsableMirror(ExternalPayloadDescriptorKey),
}

/// Fetches an AWFR external payload through local/cache mirrors and stores it in
/// the filesystem cache. Network mirrors are intentionally left to a follow-up
/// adapter so this module can remain a small filesystem/cache boundary.
pub fn fetch_external_payload_to_cache(
    archive_path: &Path,
    bundle_content_root: BundleDigest,
    descriptor_id: SectionId,
    cache_root: &Path,
) -> Result<ExternalPayloadCacheFetchReport, ExternalPayloadCacheFetchError> {
    fetch_external_payload_bytes_to_cache(
        archive_path,
        bundle_content_root,
        descriptor_id,
        cache_root,
    )
    .map(|fetched| fetched.report)
}

/// Fetches an AWFR external payload and returns both compressed and decoded bytes.
pub fn fetch_external_payload_bytes_to_cache(
    archive_path: &Path,
    bundle_content_root: BundleDigest,
    descriptor_id: SectionId,
    cache_root: &Path,
) -> Result<ExternalPayloadCacheFetchBytes, ExternalPayloadCacheFetchError> {
    let archive_bytes =
        fs::read(archive_path).map_err(|source| ExternalPayloadCacheFetchError::ReadArchive {
            path: archive_path.to_path_buf(),
            source,
        })?;
    let archive = AwfrArchiveManifest::from_json_slice(&archive_bytes)?;
    let key = ExternalPayloadDescriptorKey::new(bundle_content_root, descriptor_id);
    let carrier = archive
        .external_payload(key)
        .ok_or(ExternalPayloadCacheFetchError::MissingCarrier(key))?;
    let archive_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let store = FilesystemCacheStore::new(cache_root);
    let mut attempts = Vec::new();

    for mirror in &carrier.mirrors {
        match mirror_scheme(mirror) {
            Some("arcweft-cache") => {
                if let Some(report) = try_cache_mirror(
                    &store,
                    archive_path,
                    cache_root,
                    carrier,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            Some("file") => {
                if let Some(report) = fetch_file_mirror(
                    &store,
                    archive_path,
                    archive_dir,
                    cache_root,
                    carrier,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            Some("http" | "https") => attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::SkippedUnsupportedScheme,
                message: Some(
                    "external payload network fetching is owned by the network fetch adapter follow-up"
                        .to_owned(),
                ),
            }),
            _ => attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::SkippedUnsupportedScheme,
                message: Some("unsupported mirror scheme".to_owned()),
            }),
        }
    }

    Err(ExternalPayloadCacheFetchError::NoUsableMirror(key))
}

fn try_cache_mirror(
    store: &FilesystemCacheStore,
    archive_path: &Path,
    cache_root: &Path,
    carrier: &ExternalPayloadCarrier,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ExternalPayloadCacheFetchAttempt>,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    match store.read_object(build_digest(carrier.compressed_digest)) {
        Ok(bytes) => {
            let decoded = carrier.verify_stored_bytes(&bytes)?;
            let key = store_external_payload_record(store, carrier, &bytes)?;
            attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Hit,
                message: None,
            });
            Ok(Some(ExternalPayloadCacheFetchBytes {
                report: report(
                    archive_path,
                    cache_root,
                    carrier,
                    ExternalPayloadCacheFetchStatus::CacheHit,
                    Some(mirror.uri.clone()),
                    Some(key),
                    attempts.clone(),
                ),
                compressed_bytes: bytes,
                decoded_bytes: decoded,
            }))
        }
        Err(error) => {
            attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::CacheMiss,
                message: Some(error.to_string()),
            });
            Ok(None)
        }
    }
}

fn fetch_file_mirror(
    store: &FilesystemCacheStore,
    archive_path: &Path,
    archive_dir: &Path,
    cache_root: &Path,
    carrier: &ExternalPayloadCarrier,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ExternalPayloadCacheFetchAttempt>,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    let path = file_mirror_path(archive_dir, &mirror.uri);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) => {
            attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                message: Some(source.to_string()),
            });
            return Ok(None);
        }
    };
    let decoded = match carrier.verify_stored_bytes(&bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                message: Some(error.to_string()),
            });
            return Ok(None);
        }
    };
    let key = store_external_payload_record(store, carrier, &bytes)?;
    attempts.push(ExternalPayloadCacheFetchAttempt {
        uri: mirror.uri.clone(),
        status: ExternalPayloadCacheFetchAttemptStatus::Fetched,
        message: None,
    });
    Ok(Some(ExternalPayloadCacheFetchBytes {
        report: report(
            archive_path,
            cache_root,
            carrier,
            ExternalPayloadCacheFetchStatus::Fetched,
            Some(mirror.uri.clone()),
            Some(key),
            attempts.clone(),
        ),
        compressed_bytes: bytes,
        decoded_bytes: decoded,
    }))
}

fn store_external_payload_record(
    store: &FilesystemCacheStore,
    carrier: &ExternalPayloadCarrier,
    bytes: &[u8],
) -> Result<ArtifactKey, ExternalPayloadCacheFetchError> {
    let key = external_payload_artifact_key(carrier);
    let logical_item = carrier.cache_key.logical_item();
    store.store_artifact_with_logical_item(
        QueryKind::AssetPayload,
        key,
        ArtifactKind::AssetPayload,
        Some(&logical_item),
        bytes,
    )?;
    Ok(key)
}

fn external_payload_artifact_key(carrier: &ExternalPayloadCarrier) -> ArtifactKey {
    ArtifactKey::derive(&ArtifactKeyInput {
        compiler_build_id: format!("awfr-external-payload-v{}", carrier.cache_key.epoch),
        query: QueryKind::AssetPayload,
        artifact_kind: ArtifactKind::AssetPayload,
        target_triple: "external-release".to_owned(),
        target_features: Vec::new(),
        profile: "release-cache".to_owned(),
        package: "external-payload".to_owned(),
        logical_item: carrier.cache_key.logical_item(),
        source_digest: build_digest(carrier.compressed_digest),
        dependency_interface_digests: Vec::new(),
        dependency_body_digests: Vec::new(),
        adapter_environment_digest: build_digest(carrier.bundle_content_root),
        launch_profile_digest: build_digest(carrier.cache_key.digest()),
        declared_environment_digest: build_digest(carrier.decoded_digest),
        format_options_digest: BuildDigest::of(carrier.media_type.as_str().as_bytes()),
    })
}

fn report(
    archive_path: &Path,
    cache_root: &Path,
    carrier: &ExternalPayloadCarrier,
    status: ExternalPayloadCacheFetchStatus,
    source_uri: Option<String>,
    key: Option<ArtifactKey>,
    attempts: Vec<ExternalPayloadCacheFetchAttempt>,
) -> ExternalPayloadCacheFetchReport {
    ExternalPayloadCacheFetchReport {
        archive: archive_path.display().to_string(),
        cache_root: cache_root.display().to_string(),
        bundle_content_root: carrier.bundle_content_root.to_string(),
        descriptor_id: carrier.descriptor_id.to_string(),
        compressed_digest: carrier.compressed_digest.to_string(),
        decoded_digest: carrier.decoded_digest.to_string(),
        decoded_size: carrier.decoded_size,
        compressed_size: carrier.compressed_size,
        status,
        source_uri,
        record_key: key.map(|key| key.digest().to_string()),
        attempts,
    }
}

fn mirror_scheme(mirror: &ReleaseMirror) -> Option<&str> {
    mirror.uri.split_once(':').map(|(scheme, _)| scheme)
}

fn file_mirror_path(archive_dir: &Path, uri: &str) -> PathBuf {
    let path = uri.strip_prefix("file:").unwrap_or(uri);
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        archive_dir.join(path)
    }
}

fn build_digest(digest: BundleDigest) -> BuildDigest {
    BuildDigest::from_bytes(digest.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{
        container::{
            BundleKind, BundleSectionKind, BundleView, ContentResidency, ReadBudget, SectionInput,
            encode_bundle,
        },
        release::{
            ReleaseBundleRef, ReleaseManifest,
            archive::{ExternalPayloadMediaType, ReleaseChannel},
        },
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn fetch_external_payload_from_file_mirror_populates_cache() {
        let root = temp_root("external-payload-file");
        let cache = root.join("cache");
        let payload = b"voice-external";
        let section_id = SectionId::from_bytes([7; 16]);
        let bundle = encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::external_ref(
                section_id,
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                payload.len() as u64,
                BundleDigest::of(payload),
            )],
        )
        .expect("bundle encodes");
        let view = BundleView::parse(&bundle, ReadBudget::default()).expect("bundle parses");
        let carrier = ExternalPayloadCarrier::from_descriptor(
            &view.sections()[0],
            view.artifact_identity(),
            ExternalPayloadMediaType::default(),
            payload.len() as u64,
            BundleDigest::of(payload),
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
        )
        .expect("carrier");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("bundle mirror")],
        )
        .expect("bundle ref");
        let archive = AwfrArchiveManifest::new(
            ReleaseChannel::new("dev").expect("channel"),
            ReleaseManifest::new([bundle_ref]).expect("release manifest"),
            [carrier.clone()],
        )
        .expect("archive");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("payload.bin"), payload).expect("payload writes");
        let archive_path = root.join("game.awfr");
        fs::write(
            &archive_path,
            archive.to_json_bytes().expect("archive json"),
        )
        .expect("archive writes");

        let fetched = fetch_external_payload_bytes_to_cache(
            &archive_path,
            carrier.bundle_content_root,
            carrier.descriptor_id,
            &cache,
        )
        .expect("payload fetches");

        assert_eq!(
            fetched.report.status,
            ExternalPayloadCacheFetchStatus::Fetched
        );
        assert_eq!(fetched.decoded_bytes, payload);
        assert_eq!(
            fetched.report.bundle_content_root,
            carrier.bundle_content_root.to_string()
        );
        let _ = fs::remove_dir_all(root);
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
