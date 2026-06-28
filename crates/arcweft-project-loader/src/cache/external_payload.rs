use super::{
    network::{network_policy_rejection, read_http_mirror, read_https_mirror},
    store::{CacheStoreError, FilesystemCacheStore},
};
use arcweft_bundle::{
    container::{BundleDigest, SectionId},
    release::{
        ReleaseFetchPolicy, ReleaseMirror,
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
    #[error("external payload carrier has no usable local, cache, or network mirror for {0:?}")]
    NoUsableMirror(ExternalPayloadDescriptorKey),
}

/// Fetches an AWFR external payload through cache, local, HTTP, or HTTPS mirrors
/// and stores validated compressed bytes in the filesystem cache.
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
    let mut context = ExternalPayloadFetchContext {
        store: &store,
        archive_path,
        cache_root,
        carrier,
        fetch_policy: &archive.release_manifest.fetch_policy,
        attempts: &mut attempts,
    };

    for mirror in &context.carrier.mirrors {
        match mirror_scheme(mirror) {
            Some("arcweft-cache") => {
                if let Some(report) = try_cache_mirror(&mut context, mirror)? {
                    return Ok(report);
                }
            }
            Some("file") => {
                if let Some(report) = fetch_file_mirror(&mut context, archive_dir, mirror)? {
                    return Ok(report);
                }
            }
            Some("http") => {
                if let Some(report) = fetch_http_mirror(&mut context, mirror)? {
                    return Ok(report);
                }
            }
            Some("https") => {
                if let Some(report) = fetch_https_mirror(&mut context, mirror)? {
                    return Ok(report);
                }
            }
            _ => context.attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::SkippedUnsupportedScheme,
                message: Some("unsupported mirror scheme".to_owned()),
            }),
        }
    }

    Err(ExternalPayloadCacheFetchError::NoUsableMirror(key))
}

struct ExternalPayloadFetchContext<'a> {
    store: &'a FilesystemCacheStore,
    archive_path: &'a Path,
    cache_root: &'a Path,
    carrier: &'a ExternalPayloadCarrier,
    fetch_policy: &'a ReleaseFetchPolicy,
    attempts: &'a mut Vec<ExternalPayloadCacheFetchAttempt>,
}

fn try_cache_mirror(
    context: &mut ExternalPayloadFetchContext<'_>,
    mirror: &ReleaseMirror,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    match context
        .store
        .read_object(build_digest(context.carrier.compressed_digest))
    {
        Ok(bytes) => {
            let decoded = context.carrier.verify_stored_bytes(&bytes)?;
            let key = store_external_payload_record(context.store, context.carrier, &bytes)?;
            context.attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Hit,
                message: None,
            });
            Ok(Some(ExternalPayloadCacheFetchBytes {
                report: report(
                    context.archive_path,
                    context.cache_root,
                    context.carrier,
                    ExternalPayloadCacheFetchStatus::CacheHit,
                    Some(mirror.uri.clone()),
                    Some(key),
                    context.attempts.clone(),
                ),
                compressed_bytes: bytes,
                decoded_bytes: decoded,
            }))
        }
        Err(error) => {
            context.attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::CacheMiss,
                message: Some(error.to_string()),
            });
            Ok(None)
        }
    }
}

fn fetch_file_mirror(
    context: &mut ExternalPayloadFetchContext<'_>,
    archive_dir: &Path,
    mirror: &ReleaseMirror,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    let path = file_mirror_path(archive_dir, &mirror.uri);
    for _ in 1..=context.fetch_policy.max_attempts_per_mirror {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) => {
                context.attempts.push(ExternalPayloadCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                    message: Some(source.to_string()),
                });
                continue;
            }
        };
        if metadata.len() > context.fetch_policy.candidate_byte_budget {
            context.attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                message: Some(format!(
                    "candidate byte budget exceeded: {} byte(s) > {} byte(s)",
                    metadata.len(),
                    context.fetch_policy.candidate_byte_budget
                )),
            });
            return Ok(None);
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) => {
                context.attempts.push(ExternalPayloadCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                    message: Some(source.to_string()),
                });
                continue;
            }
        };
        if let Some(fetched) = publish_verified_payload(
            context,
            mirror,
            bytes,
            ExternalPayloadCacheFetchStatus::Fetched,
        )? {
            return Ok(Some(fetched));
        }
    }
    Ok(None)
}

fn fetch_http_mirror(
    context: &mut ExternalPayloadFetchContext<'_>,
    mirror: &ReleaseMirror,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    if let Some(message) = network_policy_rejection(context.fetch_policy, "http") {
        context.attempts.push(ExternalPayloadCacheFetchAttempt {
            uri: mirror.uri.clone(),
            status: ExternalPayloadCacheFetchAttemptStatus::Failed,
            message: Some(message),
        });
        return Ok(None);
    }
    for _ in 1..=context.fetch_policy.max_attempts_per_mirror {
        let bytes = match read_http_mirror(&mirror.uri, context.fetch_policy) {
            Ok(bytes) => bytes,
            Err(message) => {
                context.attempts.push(ExternalPayloadCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                    message: Some(message),
                });
                continue;
            }
        };
        if let Some(fetched) = publish_verified_payload(
            context,
            mirror,
            bytes,
            ExternalPayloadCacheFetchStatus::Fetched,
        )? {
            return Ok(Some(fetched));
        }
    }
    Ok(None)
}

fn fetch_https_mirror(
    context: &mut ExternalPayloadFetchContext<'_>,
    mirror: &ReleaseMirror,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    if let Some(message) = network_policy_rejection(context.fetch_policy, "https") {
        context.attempts.push(ExternalPayloadCacheFetchAttempt {
            uri: mirror.uri.clone(),
            status: ExternalPayloadCacheFetchAttemptStatus::Failed,
            message: Some(message),
        });
        return Ok(None);
    }
    for _ in 1..=context.fetch_policy.max_attempts_per_mirror {
        let bytes = match read_https_mirror(&mirror.uri, context.fetch_policy) {
            Ok(bytes) => bytes,
            Err(message) => {
                context.attempts.push(ExternalPayloadCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                    message: Some(message),
                });
                continue;
            }
        };
        if let Some(fetched) = publish_verified_payload(
            context,
            mirror,
            bytes,
            ExternalPayloadCacheFetchStatus::Fetched,
        )? {
            return Ok(Some(fetched));
        }
    }
    Ok(None)
}

fn publish_verified_payload(
    context: &mut ExternalPayloadFetchContext<'_>,
    mirror: &ReleaseMirror,
    bytes: Vec<u8>,
    status: ExternalPayloadCacheFetchStatus,
) -> Result<Option<ExternalPayloadCacheFetchBytes>, ExternalPayloadCacheFetchError> {
    let decoded = match context.carrier.verify_stored_bytes(&bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            context.attempts.push(ExternalPayloadCacheFetchAttempt {
                uri: mirror.uri.clone(),
                status: ExternalPayloadCacheFetchAttemptStatus::Failed,
                message: Some(error.to_string()),
            });
            return Err(ExternalPayloadCacheFetchError::Archive(error));
        }
    };
    let key = store_external_payload_record(context.store, context.carrier, &bytes)?;
    context.attempts.push(ExternalPayloadCacheFetchAttempt {
        uri: mirror.uri.clone(),
        status: ExternalPayloadCacheFetchAttemptStatus::Fetched,
        message: None,
    });
    Ok(Some(ExternalPayloadCacheFetchBytes {
        report: report(
            context.archive_path,
            context.cache_root,
            context.carrier,
            status,
            Some(mirror.uri.clone()),
            Some(key),
            context.attempts.clone(),
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
            ReleaseBundleRef, ReleaseFetchPolicy, ReleaseManifest, ReleaseNetworkFetchPolicy,
            archive::{ExternalPayloadMediaType, ReleaseChannel},
        },
    };
    use std::{
        io::{Read, Write},
        net::{Shutdown, TcpListener},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn fetch_external_payload_from_file_mirror_populates_cache() {
        let root = temp_root("external-payload-file");
        let cache = root.join("cache");
        let payload = b"voice-external";
        let section_id = SectionId::from_bytes([7; 16]);
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
            ReleaseFetchPolicy::default(),
        );
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("payload.bin"), payload).expect("payload writes");
        let archive_path = write_archive(&root, &fixture.archive);

        let fetched = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
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
            fixture.carrier.bundle_content_root.to_string()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_external_payload_from_cache_mirror_populates_record() {
        let root = temp_root("external-payload-cache");
        let cache = root.join("cache");
        let payload = b"voice-external-cache";
        let section_id = SectionId::from_bytes([8; 16]);
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new("arcweft-cache:payload").expect("cache mirror")],
            ReleaseFetchPolicy::default(),
        );
        fs::create_dir_all(&root).expect("root creates");
        FilesystemCacheStore::new(&cache)
            .put_object(payload)
            .expect("payload object stores");
        let archive_path = write_archive(&root, &fixture.archive);

        let fetched = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
            &cache,
        )
        .expect("payload fetches from cache");

        assert_eq!(
            fetched.report.status,
            ExternalPayloadCacheFetchStatus::CacheHit
        );
        assert_eq!(
            fetched.report.attempts[0].status,
            ExternalPayloadCacheFetchAttemptStatus::Hit
        );
        assert!(fetched.report.record_key.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_external_payload_from_http_mirror_uses_release_network_policy() {
        let root = temp_root("external-payload-http");
        let cache = root.join("cache");
        let payload = b"voice-external-http";
        let (uri, server) = spawn_http_payload_server(payload.to_vec());
        let section_id = SectionId::from_bytes([9; 16]);
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new(uri).expect("http mirror")],
            ReleaseFetchPolicy::default(),
        );
        fs::create_dir_all(&root).expect("root creates");
        let archive_path = write_archive(&root, &fixture.archive);

        let fetched = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
            &cache,
        );

        server.join().expect("server exits");
        let fetched = fetched.expect("payload fetches over http");
        assert_eq!(fetched.decoded_bytes, payload);
        assert_eq!(
            fetched.report.attempts.last().expect("attempt").status,
            ExternalPayloadCacheFetchAttemptStatus::Fetched
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn http_external_payload_rejects_plain_http_when_https_is_required() {
        let root = temp_root("external-payload-http-policy");
        let cache = root.join("cache");
        let payload = b"voice-external-http-policy";
        let section_id = SectionId::from_bytes([10; 16]);
        let fetch_policy = ReleaseFetchPolicy::new(1, u64::MAX, None)
            .expect("fetch policy")
            .with_network_policy(ReleaseNetworkFetchPolicy::require_https())
            .expect("network policy");
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new("http://127.0.0.1:9/payload.bin").expect("http mirror")],
            fetch_policy,
        );
        fs::create_dir_all(&root).expect("root creates");
        let archive_path = write_archive(&root, &fixture.archive);

        let error = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
            &cache,
        )
        .expect_err("plain HTTP is rejected before network fetch");

        assert!(matches!(
            error,
            ExternalPayloadCacheFetchError::NoUsableMirror(_)
        ));
        assert!(!cache.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn digest_mismatch_fails_before_cache_record_publication() {
        let root = temp_root("external-payload-digest-mismatch");
        let cache = root.join("cache");
        let payload = b"voice-external-good";
        let section_id = SectionId::from_bytes([11; 16]);
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
            ReleaseFetchPolicy::default(),
        );
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("payload.bin"), b"voice-external-evil").expect("payload writes");
        let archive_path = write_archive(&root, &fixture.archive);

        let error = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
            &cache,
        )
        .expect_err("digest mismatch rejects");

        assert!(matches!(
            error,
            ExternalPayloadCacheFetchError::Archive(AwfrArchiveError::DigestMismatch { .. })
        ));
        assert!(!cache.exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn size_mismatch_fails_before_cache_record_publication() {
        let root = temp_root("external-payload-size-mismatch");
        let cache = root.join("cache");
        let payload = b"voice-external-good-size";
        let section_id = SectionId::from_bytes([12; 16]);
        let fixture = external_archive_fixture(
            payload,
            section_id,
            [ReleaseMirror::new("file:payload.bin").expect("payload mirror")],
            ReleaseFetchPolicy::default(),
        );
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("payload.bin"), b"wrong-size").expect("payload writes");
        let archive_path = write_archive(&root, &fixture.archive);

        let error = fetch_external_payload_bytes_to_cache(
            &archive_path,
            fixture.carrier.bundle_content_root,
            fixture.carrier.descriptor_id,
            &cache,
        )
        .expect_err("size mismatch rejects");

        assert!(matches!(
            error,
            ExternalPayloadCacheFetchError::Archive(AwfrArchiveError::ByteLengthMismatch { .. })
        ));
        assert!(!cache.exists());
        let _ = fs::remove_dir_all(root);
    }

    struct ExternalArchiveFixture {
        archive: AwfrArchiveManifest,
        carrier: ExternalPayloadCarrier,
    }

    fn external_archive_fixture(
        payload: &[u8],
        section_id: SectionId,
        mirrors: impl IntoIterator<Item = ReleaseMirror>,
        fetch_policy: ReleaseFetchPolicy,
    ) -> ExternalArchiveFixture {
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
            mirrors,
        )
        .expect("carrier");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("bundle mirror")],
        )
        .expect("bundle ref");
        let release_manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy,
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref],
        };
        let archive = AwfrArchiveManifest::new(
            ReleaseChannel::new("dev").expect("channel"),
            release_manifest,
            [carrier.clone()],
        )
        .expect("archive");
        ExternalArchiveFixture { archive, carrier }
    }

    fn write_archive(root: &Path, archive: &AwfrArchiveManifest) -> PathBuf {
        let archive_path = root.join("game.awfr");
        fs::write(
            &archive_path,
            archive.to_json_bytes().expect("archive json"),
        )
        .expect("archive writes");
        archive_path
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

    fn spawn_http_payload_server(body: Vec<u8>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test HTTP listener binds");
        let addr = listener.local_addr().expect("test HTTP local addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test HTTP accepts request");
            let mut request = Vec::new();
            let mut buffer = [0_u8; 256];
            while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                let read = stream.read(&mut buffer).expect("test HTTP request reads");
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..read]);
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("test HTTP headers write");
            stream.write_all(&body).expect("test HTTP body writes");
            stream.flush().expect("test HTTP response flushes");
            stream
                .shutdown(Shutdown::Write)
                .expect("test HTTP response shuts down");
        });
        (format!("http://{addr}/payload.bin"), handle)
    }
}
