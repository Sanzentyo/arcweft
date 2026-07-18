use super::store::{CacheStoreError, FilesystemCacheStore};
use arcweft_bundle::{
    ArcweftBundle, BundleCodecError,
    container::{BundleDigest, BundleKind, ExternalSectionPayload},
    release::{ReleaseFetchPlan, ReleaseManifest, ReleaseManifestError, ReleaseMirror},
};
use arcweft_project::{
    artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
    fingerprint::BuildDigest,
    incremental::QueryKind,
};
use serde::Serialize;
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    path::{Path, PathBuf},
    time::Duration,
};
use thiserror::Error;

/// Result of fetching one release-manifest bundle into the local cache.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseCacheFetchReport {
    pub manifest: String,
    pub cache_root: String,
    pub content_root: String,
    pub file_digest: String,
    pub byte_len: u64,
    pub kind: BundleKind,
    pub status: ReleaseCacheFetchStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_uri: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_key: Option<String>,
    pub attempts: Vec<ReleaseCacheFetchAttempt>,
}

/// Fetched release bundle bytes plus the cache report that records how they
/// were obtained.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseCacheFetchBytes {
    pub report: ReleaseCacheFetchReport,
    pub bytes: Vec<u8>,
}

/// Product bundle decoded from a fetched release bundle.
#[derive(Clone, Debug, PartialEq)]
pub struct ReleaseProductFetch {
    pub report: ReleaseCacheFetchReport,
    pub bundle: ArcweftBundle,
}

/// Release cache fetch status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCacheFetchStatus {
    CacheHit,
    Fetched,
}

/// One mirror attempt made by release cache fetch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseCacheFetchAttempt {
    pub uri: String,
    pub attempt: u8,
    pub status: ReleaseCacheFetchAttemptStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Mirror attempt status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseCacheFetchAttemptStatus {
    CacheMiss,
    Fetched,
    Hit,
    SkippedUnsupportedScheme,
    Failed,
}

/// Release cache fetch failure.
#[derive(Debug, Error)]
pub enum ReleaseCacheFetchError {
    #[error("failed to read release manifest `{path}`: {source}")]
    ReadManifest {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read release bundle mirror `{path}`: {source}")]
    ReadMirror {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Manifest(#[from] ReleaseManifestError),
    #[error(transparent)]
    Cache(#[from] CacheStoreError),
    #[error("release manifest has no usable local or cache mirror for content root {0}")]
    NoUsableMirror(BundleDigest),
}

/// Release product fetch/decode failure.
#[derive(Debug, Error)]
pub enum ReleaseProductFetchError {
    #[error(transparent)]
    Fetch(#[from] ReleaseCacheFetchError),
    #[error(transparent)]
    DecodeProduct(#[from] BundleCodecError),
}

/// Fetches a release-manifest bundle through local/cache mirrors and stores it
/// in the filesystem cache with a stable release-bundle record.
pub fn fetch_release_bundle_to_cache(
    manifest_path: &Path,
    content_root: BundleDigest,
    cache_root: &Path,
) -> Result<ReleaseCacheFetchReport, ReleaseCacheFetchError> {
    fetch_release_bundle_bytes_to_cache(manifest_path, content_root, cache_root)
        .map(|fetched| fetched.report)
}

/// Fetches a release-manifest bundle through local/cache mirrors, stores it in
/// the filesystem cache, and returns the verified bytes.
pub fn fetch_release_bundle_bytes_to_cache(
    manifest_path: &Path,
    content_root: BundleDigest,
    cache_root: &Path,
) -> Result<ReleaseCacheFetchBytes, ReleaseCacheFetchError> {
    let manifest_bytes =
        fs::read(manifest_path).map_err(|source| ReleaseCacheFetchError::ReadManifest {
            path: manifest_path.to_path_buf(),
            source,
        })?;
    let manifest = ReleaseManifest::from_json_slice(&manifest_bytes)?;
    let plan = manifest.fetch_plan(content_root)?;
    let manifest_dir = manifest_path.parent().unwrap_or_else(|| Path::new("."));
    let store = FilesystemCacheStore::new(cache_root);
    let mut attempts = Vec::new();

    for mirror in &plan.mirrors {
        match mirror_scheme(mirror) {
            Some("arcweft-cache") => {
                if let Some(report) = try_cache_mirror(
                    &store,
                    manifest_path,
                    cache_root,
                    &plan,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            Some("file") => {
                if let Some(report) = fetch_file_mirror(
                    &store,
                    manifest_path,
                    manifest_dir,
                    cache_root,
                    &plan,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            Some("http") => {
                if let Some(report) = fetch_http_mirror(
                    &store,
                    manifest_path,
                    cache_root,
                    &plan,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            Some("https") => {
                if let Some(report) = fetch_https_mirror(
                    &store,
                    manifest_path,
                    cache_root,
                    &plan,
                    mirror,
                    &mut attempts,
                )? {
                    return Ok(report);
                }
            }
            _ => attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt: 1,
                status: ReleaseCacheFetchAttemptStatus::SkippedUnsupportedScheme,
                message: Some("unsupported mirror scheme".to_owned()),
            }),
        }
    }

    Err(ReleaseCacheFetchError::NoUsableMirror(plan.content_root))
}

/// Fetches a release-manifest product bundle and decodes it with optional
/// external AWFB section payloads supplied by the caller.
pub fn fetch_release_product_bundle(
    manifest_path: &Path,
    content_root: BundleDigest,
    cache_root: &Path,
    external_sections: &[ExternalSectionPayload],
) -> Result<ReleaseProductFetch, ReleaseProductFetchError> {
    let fetched = fetch_release_bundle_bytes_to_cache(manifest_path, content_root, cache_root)?;
    let bundle =
        ArcweftBundle::from_awfb_slice_with_external_sections(&fetched.bytes, external_sections)?;
    Ok(ReleaseProductFetch {
        report: fetched.report,
        bundle,
    })
}

fn try_cache_mirror(
    store: &FilesystemCacheStore,
    manifest_path: &Path,
    cache_root: &Path,
    plan: &ReleaseFetchPlan,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ReleaseCacheFetchAttempt>,
) -> Result<Option<ReleaseCacheFetchBytes>, ReleaseCacheFetchError> {
    let object_digest = build_digest(plan.file_digest);
    match store.read_object(object_digest) {
        Ok(bytes) => {
            plan.verify_bytes(&bytes)?;
            let key = store_release_bundle_record(store, plan, &bytes)?;
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt: 1,
                status: ReleaseCacheFetchAttemptStatus::Hit,
                message: None,
            });
            Ok(Some(ReleaseCacheFetchBytes {
                report: report(
                    manifest_path,
                    cache_root,
                    plan,
                    ReleaseCacheFetchStatus::CacheHit,
                    Some(mirror.uri.clone()),
                    Some(key),
                    attempts.clone(),
                ),
                bytes,
            }))
        }
        Err(error) => {
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt: 1,
                status: ReleaseCacheFetchAttemptStatus::CacheMiss,
                message: Some(error.to_string()),
            });
            Ok(None)
        }
    }
}

fn fetch_file_mirror(
    store: &FilesystemCacheStore,
    manifest_path: &Path,
    manifest_dir: &Path,
    cache_root: &Path,
    plan: &ReleaseFetchPlan,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ReleaseCacheFetchAttempt>,
) -> Result<Option<ReleaseCacheFetchBytes>, ReleaseCacheFetchError> {
    let path = file_mirror_path(manifest_dir, &mirror.uri);
    for attempt in 1..=plan.fetch_policy.max_attempts_per_mirror {
        let metadata = match fs::metadata(&path) {
            Ok(metadata) => metadata,
            Err(source) => {
                attempts.push(ReleaseCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    attempt,
                    status: ReleaseCacheFetchAttemptStatus::Failed,
                    message: Some(source.to_string()),
                });
                continue;
            }
        };
        if metadata.len() > plan.fetch_policy.candidate_byte_budget {
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt,
                status: ReleaseCacheFetchAttemptStatus::Failed,
                message: Some(format!(
                    "candidate byte budget exceeded: {} byte(s) > {} byte(s)",
                    metadata.len(),
                    plan.fetch_policy.candidate_byte_budget
                )),
            });
            return Ok(None);
        }
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) => {
                attempts.push(ReleaseCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    attempt,
                    status: ReleaseCacheFetchAttemptStatus::Failed,
                    message: Some(source.to_string()),
                });
                continue;
            }
        };
        if let Err(error) = plan.verify_bytes(&bytes) {
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt,
                status: ReleaseCacheFetchAttemptStatus::Failed,
                message: Some(error.to_string()),
            });
            continue;
        }
        let key = store_release_bundle_record(store, plan, &bytes)?;
        attempts.push(ReleaseCacheFetchAttempt {
            uri: mirror.uri.clone(),
            attempt,
            status: ReleaseCacheFetchAttemptStatus::Fetched,
            message: None,
        });
        return Ok(Some(ReleaseCacheFetchBytes {
            report: report(
                manifest_path,
                cache_root,
                plan,
                ReleaseCacheFetchStatus::Fetched,
                Some(mirror.uri.clone()),
                Some(key),
                attempts.clone(),
            ),
            bytes,
        }));
    }
    Ok(None)
}

fn fetch_http_mirror(
    store: &FilesystemCacheStore,
    manifest_path: &Path,
    cache_root: &Path,
    plan: &ReleaseFetchPlan,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ReleaseCacheFetchAttempt>,
) -> Result<Option<ReleaseCacheFetchBytes>, ReleaseCacheFetchError> {
    if let Some(message) = network_policy_rejection(plan, "http") {
        attempts.push(ReleaseCacheFetchAttempt {
            uri: mirror.uri.clone(),
            attempt: 1,
            status: ReleaseCacheFetchAttemptStatus::Failed,
            message: Some(message),
        });
        return Ok(None);
    }
    for attempt in 1..=plan.fetch_policy.max_attempts_per_mirror {
        let bytes = match read_http_mirror(&mirror.uri, plan) {
            Ok(bytes) => bytes,
            Err(message) => {
                attempts.push(ReleaseCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    attempt,
                    status: ReleaseCacheFetchAttemptStatus::Failed,
                    message: Some(message),
                });
                continue;
            }
        };
        if let Err(error) = plan.verify_bytes(&bytes) {
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt,
                status: ReleaseCacheFetchAttemptStatus::Failed,
                message: Some(error.to_string()),
            });
            continue;
        }
        let key = store_release_bundle_record(store, plan, &bytes)?;
        attempts.push(ReleaseCacheFetchAttempt {
            uri: mirror.uri.clone(),
            attempt,
            status: ReleaseCacheFetchAttemptStatus::Fetched,
            message: None,
        });
        return Ok(Some(ReleaseCacheFetchBytes {
            report: report(
                manifest_path,
                cache_root,
                plan,
                ReleaseCacheFetchStatus::Fetched,
                Some(mirror.uri.clone()),
                Some(key),
                attempts.clone(),
            ),
            bytes,
        }));
    }
    Ok(None)
}

fn fetch_https_mirror(
    store: &FilesystemCacheStore,
    manifest_path: &Path,
    cache_root: &Path,
    plan: &ReleaseFetchPlan,
    mirror: &ReleaseMirror,
    attempts: &mut Vec<ReleaseCacheFetchAttempt>,
) -> Result<Option<ReleaseCacheFetchBytes>, ReleaseCacheFetchError> {
    if let Some(message) = network_policy_rejection(plan, "https") {
        attempts.push(ReleaseCacheFetchAttempt {
            uri: mirror.uri.clone(),
            attempt: 1,
            status: ReleaseCacheFetchAttemptStatus::Failed,
            message: Some(message),
        });
        return Ok(None);
    }
    for attempt in 1..=plan.fetch_policy.max_attempts_per_mirror {
        let bytes = match read_https_mirror(&mirror.uri, plan) {
            Ok(bytes) => bytes,
            Err(message) => {
                attempts.push(ReleaseCacheFetchAttempt {
                    uri: mirror.uri.clone(),
                    attempt,
                    status: ReleaseCacheFetchAttemptStatus::Failed,
                    message: Some(message),
                });
                continue;
            }
        };
        if let Err(error) = plan.verify_bytes(&bytes) {
            attempts.push(ReleaseCacheFetchAttempt {
                uri: mirror.uri.clone(),
                attempt,
                status: ReleaseCacheFetchAttemptStatus::Failed,
                message: Some(error.to_string()),
            });
            continue;
        }
        let key = store_release_bundle_record(store, plan, &bytes)?;
        attempts.push(ReleaseCacheFetchAttempt {
            uri: mirror.uri.clone(),
            attempt,
            status: ReleaseCacheFetchAttemptStatus::Fetched,
            message: None,
        });
        return Ok(Some(ReleaseCacheFetchBytes {
            report: report(
                manifest_path,
                cache_root,
                plan,
                ReleaseCacheFetchStatus::Fetched,
                Some(mirror.uri.clone()),
                Some(key),
                attempts.clone(),
            ),
            bytes,
        }));
    }
    Ok(None)
}

fn read_http_mirror(uri: &str, plan: &ReleaseFetchPlan) -> Result<Vec<u8>, String> {
    let url = HttpMirrorUrl::parse(uri)?;
    let mut stream = connect_http(&url, plan.fetch_policy.cancel_after_millis)?;
    let host_header = url.host_header();
    let user_agent = &plan.fetch_policy.network_policy.user_agent;
    write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nUser-Agent: {}\r\nAccept: application/octet-stream\r\nConnection: close\r\n\r\n",
        url.target,
        host_header,
        user_agent
    )
    .map_err(|error| format!("failed to write HTTP request: {error}"))?;
    stream
        .flush()
        .map_err(|error| format!("failed to flush HTTP request: {error}"))?;
    let response = read_http_response(&mut stream, plan.fetch_policy.candidate_byte_budget)?;
    decode_http_response(&response, plan.fetch_policy.candidate_byte_budget)
}

fn read_https_mirror(uri: &str, plan: &ReleaseFetchPlan) -> Result<Vec<u8>, String> {
    if !uri.starts_with("https://") {
        return Err("HTTPS mirror URI must start with https://".to_owned());
    }
    let mut config = ureq::Agent::config_builder();
    if let Some(cancel_after_millis) = plan.fetch_policy.cancel_after_millis {
        let timeout = Duration::from_millis(cancel_after_millis);
        config = config
            .timeout_global(Some(timeout))
            .timeout_connect(Some(timeout));
    }
    let agent = ureq::Agent::new_with_config(config.build());
    let mut response = agent
        .get(uri)
        .header("User-Agent", &plan.fetch_policy.network_policy.user_agent)
        .header("Accept", "application/octet-stream")
        .call()
        .map_err(|error| format!("failed to fetch HTTPS mirror: {error}"))?;
    response
        .body_mut()
        .with_config()
        .limit(plan.fetch_policy.candidate_byte_budget)
        .read_to_vec()
        .map_err(|error| format!("failed to read HTTPS mirror response: {error}"))
}

fn network_policy_rejection(plan: &ReleaseFetchPlan, scheme: &str) -> Option<String> {
    let policy = &plan.fetch_policy.network_policy;
    if policy.require_https && scheme == "http" {
        return Some("network policy requires HTTPS; plain HTTP mirror is not allowed".to_owned());
    }
    if let Some(proxy_profile) = &policy.proxy_profile {
        return Some(format!(
            "network policy requires proxy profile `{proxy_profile}`, but this cache adapter has no proxy provider"
        ));
    }
    if let Some(auth_profile) = &policy.auth_profile {
        return Some(format!(
            "network policy requires auth profile `{auth_profile}`, but this cache adapter has no credential provider"
        ));
    }
    if let Some(client_profile) = &policy.client_profile {
        return Some(format!(
            "network policy requires client profile `{client_profile}`, but this cache adapter is using the default client"
        ));
    }
    None
}

fn connect_http(
    url: &HttpMirrorUrl,
    cancel_after_millis: Option<u64>,
) -> Result<TcpStream, String> {
    let addrs = (url.host.as_str(), url.port)
        .to_socket_addrs()
        .map_err(|error| format!("failed to resolve HTTP mirror host: {error}"))?;
    let timeout = cancel_after_millis.map(Duration::from_millis);
    let mut last_error = None;
    for addr in addrs {
        let stream = match timeout {
            Some(timeout) => TcpStream::connect_timeout(&addr, timeout),
            None => TcpStream::connect(addr),
        };
        match stream {
            Ok(stream) => {
                stream
                    .set_read_timeout(timeout)
                    .map_err(|error| format!("failed to set HTTP read timeout: {error}"))?;
                stream
                    .set_write_timeout(timeout)
                    .map_err(|error| format!("failed to set HTTP write timeout: {error}"))?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.map_or_else(
        || "HTTP mirror host resolved to no socket addresses".to_owned(),
        |error| format!("failed to connect HTTP mirror: {error}"),
    ))
}

fn read_http_response(stream: &mut TcpStream, body_budget: u64) -> Result<Vec<u8>, String> {
    const HEADER_BUDGET: u64 = 16 * 1024;
    let response_budget = body_budget.saturating_add(HEADER_BUDGET);
    let mut response = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let read = match stream.read(&mut buffer) {
            Ok(read) => read,
            Err(error) if error.kind() == ErrorKind::ConnectionReset && !response.is_empty() => {
                break;
            }
            Err(error) => return Err(format!("failed to read HTTP response: {error}")),
        };
        if read == 0 {
            break;
        }
        response.extend_from_slice(&buffer[..read]);
        if u64::try_from(response.len()).unwrap_or(u64::MAX) > response_budget {
            return Err(format!(
                "HTTP response exceeds candidate byte budget: response > {body_budget} byte(s) plus header allowance"
            ));
        }
    }
    Ok(response)
}

fn decode_http_response(response: &[u8], body_budget: u64) -> Result<Vec<u8>, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "HTTP response is missing header terminator".to_owned())?;
    let header = std::str::from_utf8(&response[..header_end])
        .map_err(|error| format!("HTTP response headers are not UTF-8: {error}"))?;
    let status = header.lines().next().unwrap_or_default();
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(format!("HTTP mirror returned non-200 status: {status}"));
    }
    if header.lines().skip(1).any(|line| {
        line.split_once(':').is_some_and(|(name, value)| {
            name.eq_ignore_ascii_case("transfer-encoding")
                && value.to_ascii_lowercase().contains("chunked")
        })
    }) {
        return Err("HTTP chunked transfer encoding is not supported by this adapter".to_owned());
    }
    let body = &response[header_end + 4..];
    if u64::try_from(body.len()).unwrap_or(u64::MAX) > body_budget {
        return Err(format!(
            "candidate byte budget exceeded: {} byte(s) > {} byte(s)",
            body.len(),
            body_budget
        ));
    }
    Ok(body.to_vec())
}

fn store_release_bundle_record(
    store: &FilesystemCacheStore,
    plan: &ReleaseFetchPlan,
    bytes: &[u8],
) -> Result<ArtifactKey, ReleaseCacheFetchError> {
    let key = release_bundle_artifact_key(plan);
    let logical_item = release_bundle_logical_item(plan);
    store.store_artifact_with_logical_item(
        QueryKind::BundleIndex,
        key,
        ArtifactKind::BundleIndex,
        Some(&logical_item),
        bytes,
    )?;
    Ok(key)
}

fn release_bundle_logical_item(plan: &ReleaseFetchPlan) -> String {
    format!("content-root:{}", plan.content_root)
}

fn release_bundle_artifact_key(plan: &ReleaseFetchPlan) -> ArtifactKey {
    ArtifactKey::derive(&ArtifactKeyInput {
        compiler_build_id: "release-manifest-v1".to_owned(),
        query: QueryKind::BundleIndex,
        artifact_kind: ArtifactKind::BundleIndex,
        target_triple: "external-release".to_owned(),
        target_features: Vec::new(),
        profile: "release-cache".to_owned(),
        package: "external-content".to_owned(),
        logical_item: release_bundle_logical_item(plan),
        source_digest: build_digest(plan.file_digest),
        dependency_interface_digests: Vec::new(),
        dependency_body_digests: Vec::new(),
        adapter_environment_digest: BuildDigest::ZERO,
        launch_profile_digest: build_digest(plan.content_root),
        declared_environment_digest: BuildDigest::ZERO,
        format_options_digest: BuildDigest::ZERO,
    })
}

fn report(
    manifest_path: &Path,
    cache_root: &Path,
    plan: &ReleaseFetchPlan,
    status: ReleaseCacheFetchStatus,
    source_uri: Option<String>,
    key: Option<ArtifactKey>,
    attempts: Vec<ReleaseCacheFetchAttempt>,
) -> ReleaseCacheFetchReport {
    ReleaseCacheFetchReport {
        manifest: manifest_path.display().to_string(),
        cache_root: cache_root.display().to_string(),
        content_root: plan.content_root.to_string(),
        file_digest: plan.file_digest.to_string(),
        byte_len: plan.byte_len,
        kind: plan.kind,
        status,
        source_uri,
        record_key: key.map(|key| key.digest().to_string()),
        attempts,
    }
}

fn file_mirror_path(manifest_dir: &Path, uri: &str) -> PathBuf {
    let path = uri.strip_prefix("file:").unwrap_or(uri);
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}

struct HttpMirrorUrl {
    host: String,
    port: u16,
    target: String,
}

impl HttpMirrorUrl {
    fn parse(uri: &str) -> Result<Self, String> {
        let rest = uri
            .strip_prefix("http://")
            .ok_or_else(|| "HTTP mirror URI must start with http://".to_owned())?;
        let (authority, path) = rest
            .split_once('/')
            .map_or((rest, "/"), |(authority, path)| (authority, path));
        if authority.is_empty() {
            return Err("HTTP mirror URI is missing a host".to_owned());
        }
        if authority.contains('@') {
            return Err("HTTP mirror URI userinfo is not supported".to_owned());
        }
        let (host, port) = parse_http_authority(authority)?;
        if host.is_empty() {
            return Err("HTTP mirror URI is missing a host".to_owned());
        }
        Ok(Self {
            host,
            port,
            target: if path == "/" {
                "/".to_owned()
            } else {
                format!("/{path}")
            },
        })
    }

    fn host_header(&self) -> String {
        if self.port == 80 {
            self.host.clone()
        } else if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

fn parse_http_authority(authority: &str) -> Result<(String, u16), String> {
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, after_host) = rest
            .split_once(']')
            .ok_or_else(|| "HTTP IPv6 host is missing closing bracket".to_owned())?;
        let port = if after_host.is_empty() {
            80
        } else {
            after_host
                .strip_prefix(':')
                .ok_or_else(|| "HTTP IPv6 host has invalid port separator".to_owned())?
                .parse::<u16>()
                .map_err(|error| format!("HTTP mirror port is invalid: {error}"))?
        };
        return Ok((host.to_owned(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => Ok((
            host.to_owned(),
            port.parse::<u16>()
                .map_err(|error| format!("HTTP mirror port is invalid: {error}"))?,
        )),
        _ => Ok((authority.to_owned(), 80)),
    }
}

fn mirror_scheme(mirror: &ReleaseMirror) -> Option<&str> {
    mirror.uri.split_once(':').map(|(scheme, _)| scheme)
}

fn build_digest(digest: BundleDigest) -> BuildDigest {
    BuildDigest::from_bytes(digest.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{
        ArcweftBundle, BundleFormat, BundleManifest, BundleRuntimeSummary,
        container::{
            BundleKind, BundleSectionKind, BundleView, ContentResidency, ReadBudget, SectionId,
            SectionInput, encode_bundle,
        },
        release::{
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1, ReleaseFetchPolicy, ReleaseNetworkFetchPolicy,
            ReleaseSignatureEnvelope, ReleaseSignaturePolicy,
        },
        release::{ReleaseBundleRef, ReleaseManifest, ReleaseMirror},
    };
    use arcweft_core::awbc::schema::{
        AwbcBlock, AwbcBlockId, AwbcEffectSetId, AwbcEntry, AwbcEntryKind, AwbcEntryTarget,
        AwbcFrameLayout, AwbcFrameLayoutId, AwbcFunction, AwbcFunctionFlags, AwbcFunctionId,
        AwbcFunctionKind, AwbcProgram, AwbcSafePointKind, AwbcSignature, AwbcSignatureId,
        AwbcStringId, AwbcTableRange, AwbcTerminator,
    };
    use arcweft_core::bytecode::BytecodeProgram;
    use arcweft_render_text::LineDisplayCatalog;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
    use std::{
        io::{Read, Write},
        net::{Shutdown, TcpListener},
        thread,
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn fetch_release_bundle_reads_file_mirror_and_stores_cache_record() {
        let root = temp_root("file-fetch");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let bundle_path = root.join("content.awfb");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(&bundle_path, &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch succeeds");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        let cached = FilesystemCacheStore::new(&cache)
            .read_object(build_digest(bundle_ref.file_digest))
            .expect("cached object reads");
        assert_eq!(cached, bundle);
        assert!(report.record_key.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_bytes_returns_verified_cached_bytes() {
        let root = temp_root("file-fetch-bytes");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let bundle_path = root.join("content.awfb");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(&bundle_path, &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let fetched =
            fetch_release_bundle_bytes_to_cache(&manifest_path, bundle_ref.content_root, &cache)
                .expect("fetch bytes succeeds");

        assert_eq!(fetched.report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(fetched.bytes, bundle);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_product_bundle_decodes_cached_awfb_product() {
        let root = temp_root("product-fetch");
        let cache = root.join("cache");
        let bundle = game_bundle();
        let bytes = bundle
            .to_format_bytes(BundleFormat::Awfb)
            .expect("product bundle encodes");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("game.awfb"), &bytes).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bytes,
            [ReleaseMirror::new("file:game.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let fetched =
            fetch_release_product_bundle(&manifest_path, bundle_ref.content_root, &cache, &[])
                .expect("product fetch succeeds");

        assert_eq!(fetched.report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(fetched.bundle, bundle);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_uses_cache_mirror_before_file_mirror() {
        let root = temp_root("cache-hit");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let store = FilesystemCacheStore::new(&cache);
        store.put_object(&bundle).expect("cached object stores");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority("arcweft-cache:content", 0).expect("cache mirror"),
                ReleaseMirror::with_priority("file:missing.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        fs::create_dir_all(&root).expect("root creates");
        let manifest = ReleaseManifest::new([bundle_ref.clone()]).expect("manifest");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch succeeds");

        assert_eq!(report.status, ReleaseCacheFetchStatus::CacheHit);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Hit
        );
        assert!(report.record_key.is_some());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_retries_failed_file_mirror_then_uses_next_mirror() {
        let root = temp_root("file-retry-fallback");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority("file:missing.awfb", 0).expect("missing mirror"),
                ReleaseMirror::with_priority("file:content.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(2, u64::MAX, None).expect("policy"),
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch succeeds");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(report.attempts.len(), 3);
        assert_eq!(report.attempts[0].attempt, 1);
        assert_eq!(report.attempts[1].attempt, 2);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Failed
        );
        assert_eq!(
            report.attempts[1].status,
            ReleaseCacheFetchAttemptStatus::Failed
        );
        assert_eq!(
            report.attempts[2].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_skips_file_candidate_over_byte_budget() {
        let root = temp_root("file-budget-fallback");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let mut oversized = bundle.clone();
        oversized.extend_from_slice(b"too-large");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("oversized.awfb"), &oversized).expect("oversized writes");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority("file:oversized.awfb", 0).expect("oversized mirror"),
                ReleaseMirror::with_priority("file:content.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, None).expect("policy"),
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch succeeds");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Failed
        );
        assert!(
            report.attempts[0]
                .message
                .as_deref()
                .is_some_and(|message| message.contains("candidate byte budget exceeded"))
        );
        assert_eq!(
            report.attempts[1].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_reads_http_mirror_with_timeout_and_budget() {
        let root = temp_root("http-fetch");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let (direct_uri, direct_server) = spawn_http_server(bundle.clone(), "HTTP/1.1 200 OK");
        let (uri, server) = spawn_http_server(bundle.clone(), "HTTP/1.1 200 OK");
        let bundle_ref =
            ReleaseBundleRef::from_awfb_bytes(&bundle, [ReleaseMirror::new(uri).expect("mirror")])
                .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, Some(5_000))
                .expect("policy"),
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        let direct_plan = manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("direct fetch plan");
        let direct = read_http_mirror(&direct_uri, &direct_plan).expect("HTTP body reads");
        assert_eq!(direct, bundle);
        direct_server.join().expect("direct HTTP server exits");
        fs::create_dir_all(&root).expect("root creates");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch succeeds");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        let cached = FilesystemCacheStore::new(&cache)
            .read_object(build_digest(bundle_ref.file_digest))
            .expect("cached object reads");
        assert_eq!(cached, bundle);
        server.join().expect("http server exits");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_requires_https_policy_skips_http_then_uses_file_mirror() {
        let root = temp_root("https-policy-fallback");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority("http://127.0.0.1:9/content.awfb", 0)
                    .expect("http mirror"),
                ReleaseMirror::with_priority("file:content.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, Some(5_000))
                .expect("policy")
                .with_network_policy(ReleaseNetworkFetchPolicy::require_https())
                .expect("network policy"),
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch falls back to file mirror");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(report.attempts.len(), 2);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Failed
        );
        assert!(
            report.attempts[0]
                .message
                .as_deref()
                .is_some_and(|message| message.contains("requires HTTPS"))
        );
        assert_eq!(
            report.attempts[1].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn network_policy_rejects_profiles_unavailable_to_default_cache_client() {
        let proxy_plan = fetch_plan_with_network_policy(
            ReleaseNetworkFetchPolicy::default()
                .with_proxy_profile("corp-egress")
                .expect("proxy policy"),
        );
        let auth_plan = fetch_plan_with_network_policy(
            ReleaseNetworkFetchPolicy::default()
                .with_auth_profile("release-token")
                .expect("auth policy"),
        );
        let client_plan = fetch_plan_with_network_policy(
            ReleaseNetworkFetchPolicy::default()
                .with_client_profile("strict-tls")
                .expect("client policy"),
        );

        assert!(
            network_policy_rejection(&proxy_plan, "https")
                .is_some_and(|message| message.contains("proxy profile `corp-egress`"))
        );
        assert!(
            network_policy_rejection(&auth_plan, "https")
                .is_some_and(|message| message.contains("auth profile `release-token`"))
        );
        assert!(
            network_policy_rejection(&client_plan, "https")
                .is_some_and(|message| message.contains("client profile `strict-tls`"))
        );
    }

    #[test]
    fn fetch_release_bundle_attempts_https_mirror_then_uses_file_mirror() {
        let root = temp_root("https-fallback");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let (https_uri, server) = spawn_tls_refusing_server();
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [
                ReleaseMirror::with_priority(https_uri, 0).expect("https mirror"),
                ReleaseMirror::with_priority("file:content.awfb", 10).expect("file mirror"),
            ],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, Some(2_000))
                .expect("policy"),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let report = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect("fetch falls back to file mirror");

        assert_eq!(report.status, ReleaseCacheFetchStatus::Fetched);
        assert_eq!(report.attempts.len(), 2);
        assert_eq!(
            report.attempts[0].status,
            ReleaseCacheFetchAttemptStatus::Failed
        );
        assert!(
            report.attempts[0]
                .message
                .as_deref()
                .is_some_and(|message| message.contains("HTTPS mirror"))
        );
        assert_eq!(
            report.attempts[1].status,
            ReleaseCacheFetchAttemptStatus::Fetched
        );
        server.join().expect("TLS-refusing server exits");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_rejects_http_body_over_byte_budget() {
        let root = temp_root("http-budget");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        let mut oversized = bundle.clone();
        oversized.extend_from_slice(b"too-large");
        let (uri, server) = spawn_http_server(oversized, "HTTP/1.1 200 OK");
        let bundle_ref =
            ReleaseBundleRef::from_awfb_bytes(&bundle, [ReleaseMirror::new(uri).expect("mirror")])
                .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, Some(5_000))
                .expect("policy"),
            signature_policy: arcweft_bundle::release::ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        fs::create_dir_all(&root).expect("root creates");
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let error = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect_err("oversized HTTP body rejects");

        assert!(matches!(
            error,
            ReleaseCacheFetchError::NoUsableMirror(content_root)
                if content_root == bundle_ref.content_root
        ));
        server.join().expect("http server exits");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_rejects_unsigned_file_when_signature_required() {
        let root = temp_root("file-signature-required");
        let cache = root.join("cache");
        let bundle = content_pack(b"voice");
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_signature(None).expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let error = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect_err("unsigned file mirror rejects");

        assert!(matches!(
            error,
            ReleaseCacheFetchError::NoUsableMirror(content_root)
                if content_root == bundle_ref.content_root
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn fetch_release_bundle_rejects_untrusted_signature_envelope() {
        let root = temp_root("file-untrusted-signature");
        let cache = root.join("cache");
        let unsigned = content_pack(b"voice");
        let unsigned_ref = ReleaseBundleRef::from_awfb_bytes(
            &unsigned,
            [ReleaseMirror::new("file:unsigned.awfb").expect("mirror")],
        )
        .expect("unsigned bundle ref");
        let envelope = ReleaseSignatureEnvelope::new(
            "other-key",
            RELEASE_SIGNATURE_ALGORITHM_ED25519_V1,
            unsigned_ref.content_root,
            unsigned_ref.kind,
            BundleView::parse(&unsigned, ReadBudget::default())
                .expect("unsigned bundle parses")
                .signing_digest()
                .expect("unsigned signing digest computes"),
            "sig",
        )
        .expect("signature envelope")
        .to_json_bytes()
        .expect("signature envelope encodes");
        let bundle = append_signature_block(unsigned, &envelope);
        fs::create_dir_all(&root).expect("root creates");
        fs::write(root.join("content.awfb"), &bundle).expect("bundle writes");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("file:content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::default(),
            signature_policy: ReleaseSignaturePolicy::require_trusted_signers(
                None,
                ["release-key"],
            )
            .expect("policy"),
            bundles: vec![bundle_ref.clone()],
        };
        let manifest_path = root.join("game.awfr");
        fs::write(
            &manifest_path,
            manifest.to_json_bytes().expect("manifest encodes"),
        )
        .expect("manifest writes");

        let error = fetch_release_bundle_to_cache(&manifest_path, bundle_ref.content_root, &cache)
            .expect_err("untrusted signature rejects");

        assert!(matches!(
            error,
            ReleaseCacheFetchError::NoUsableMirror(content_root)
                if content_root == bundle_ref.content_root
        ));
        let _ = fs::remove_dir_all(root);
    }

    fn content_pack(bytes: &'static [u8]) -> Vec<u8> {
        encode_bundle(
            BundleKind::ContentPack,
            br#"{"kind":"content"}"#,
            vec![SectionInput::embedded(
                SectionId::from_bytes([2; 16]),
                BundleSectionKind::AssetBlob,
                1,
                ContentResidency::OnDemand,
                false,
                bytes,
            )],
        )
        .expect("content pack encodes")
    }

    fn fetch_plan_with_network_policy(
        network_policy: ReleaseNetworkFetchPolicy,
    ) -> ReleaseFetchPlan {
        let bundle = content_pack(b"voice");
        let bundle_ref = ReleaseBundleRef::from_awfb_bytes(
            &bundle,
            [ReleaseMirror::new("https://cdn.example.test/content.awfb").expect("mirror")],
        )
        .expect("bundle ref");
        let manifest = ReleaseManifest {
            schema_version: arcweft_bundle::release::RELEASE_MANIFEST_SCHEMA_VERSION,
            fetch_policy: ReleaseFetchPolicy::new(1, bundle.len() as u64, Some(5_000))
                .expect("policy")
                .with_network_policy(network_policy)
                .expect("network policy"),
            signature_policy: ReleaseSignaturePolicy::default(),
            bundles: vec![bundle_ref.clone()],
        };
        manifest
            .fetch_plan(bundle_ref.content_root)
            .expect("fetch plan")
    }

    fn game_bundle() -> ArcweftBundle {
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 0,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("main.arcw", "flow @flow.main main { return \"ok\" }"),
            BytecodeProgram::default(),
            LineDisplayCatalog::default(),
        )
        .expect("standard dialogue source joins source map")
        .with_product_awbc(minimal_awbc_program())
    }

    fn source_map(label: &str, text: &str) -> SourceMapSection {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new(label).expect("source ID"),
            SourceName::path(label),
            text,
        )
        .expect("source document");
        SourceMapSection::try_from_documents(&[&document]).expect("source map")
    }

    fn minimal_awbc_program() -> AwbcProgram {
        AwbcProgram {
            strings: vec!["entry.main".to_owned()],
            signatures: vec![AwbcSignature {
                params: Vec::new(),
                result: None,
                effects: AwbcEffectSetId(0),
            }],
            frame_layouts: vec![AwbcFrameLayout {
                slots: Vec::new(),
                max_scope_depth: 0,
            }],
            functions: vec![AwbcFunction {
                public_id: Some(AwbcStringId(0)),
                kind: AwbcFunctionKind::Flow,
                signature: AwbcSignatureId(0),
                frame_layout: AwbcFrameLayoutId(0),
                blocks: AwbcTableRange::new(0, 1),
                entry_block: AwbcBlockId(0),
                flags: AwbcFunctionFlags(AwbcFunctionFlags::DETERMINISTIC),
            }],
            blocks: vec![AwbcBlock {
                owner: AwbcFunctionId(0),
                instructions: AwbcTableRange::new(0, 0),
                terminator: AwbcTerminator::Return { value: None },
                safe_point: AwbcSafePointKind::FlowEntry,
                source_map: None,
            }],
            entries: vec![AwbcEntry {
                runtime_id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body(
                    "entry.main",
                )
                .expect("test entry ID is valid"),
                binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
                public_id: AwbcStringId(0),
                kind: AwbcEntryKind::Cli,
                signature: AwbcSignatureId(0),
                target: AwbcEntryTarget::Function(AwbcFunctionId(0)),
                roles: arcweft_core::entry::RuntimeEntryRoles::None,
            }],
            ..AwbcProgram::default()
        }
    }

    fn append_signature_block(mut bytes: Vec<u8>, signature: &[u8]) -> Vec<u8> {
        let signature_offset = bytes.len();
        bytes.extend_from_slice(signature);
        write_u64(&mut bytes, 56, signature_offset as u64);
        write_u64(&mut bytes, 64, signature.len() as u64);
        let file_len = bytes.len() as u64;
        write_u64(&mut bytes, 72, file_len);
        bytes
    }

    fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn spawn_http_server(body: Vec<u8>, status: &'static str) -> (String, thread::JoinHandle<()>) {
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
                "{status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .expect("test HTTP headers write");
            stream.write_all(&body).expect("test HTTP body writes");
            stream.flush().expect("test HTTP response flushes");
            stream
                .shutdown(Shutdown::Write)
                .expect("test HTTP response shuts down");
        });
        (format!("http://{addr}/content.awfb"), handle)
    }

    fn spawn_tls_refusing_server() -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("test HTTPS listener binds");
        let addr = listener.local_addr().expect("test HTTPS local addr");
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("test HTTPS accepts request");
            let mut buffer = [0_u8; 256];
            let _ = stream.read(&mut buffer);
            let _ = stream.shutdown(Shutdown::Both);
        });
        (format!("https://{addr}/content.awfb"), handle)
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-release-cache-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
