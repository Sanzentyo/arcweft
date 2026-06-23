use super::{record::CacheRecord, store::FilesystemCacheStore};
use arcweft_project::fingerprint::BuildDigest;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

/// Filesystem cache inventory summary.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
pub struct CacheStats {
    pub root: String,
    pub object_files: usize,
    pub object_bytes: u64,
    pub record_files: usize,
    pub record_bytes: u64,
    pub lock_files: usize,
    pub temp_files: usize,
    pub other_files: usize,
}

/// Cache verification report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheVerifyReport {
    pub status: CacheVerifyStatus,
    pub stats: CacheStats,
    pub issues: Vec<CacheVerifyIssue>,
}

/// Cache explanation report for one artifact key or object digest.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheExplainReport {
    pub root: String,
    pub query: String,
    pub status: CacheExplainStatus,
    pub matches: Vec<CacheExplainMatch>,
    pub issues: Vec<CacheVerifyIssue>,
}

/// Cache prune report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachePruneReport {
    pub root: String,
    pub applied: bool,
    pub candidates: Vec<CachePruneCandidate>,
    pub removed_files: usize,
    pub removed_directories: usize,
    pub removed_bytes: u64,
    pub issues: Vec<CacheVerifyIssue>,
}

/// Cache verification status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheVerifyStatus {
    Ok,
    Failed,
}

/// Cache explanation lookup status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheExplainStatus {
    Found,
    Missing,
    InvalidQuery,
}

/// One cache file or directory selected for pruning.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CachePruneCandidate {
    pub path: String,
    pub kind: CachePruneCandidateKind,
    pub bytes: u64,
}

/// Cache prune candidate family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CachePruneCandidateKind {
    TempFile,
    UnreferencedObject,
    EmptyDirectory,
}

/// One cache verification issue.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheVerifyIssue {
    pub path: String,
    pub kind: CacheVerifyIssueKind,
    pub message: String,
}

/// One matched cache item.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CacheExplainMatch {
    pub path: String,
    pub kind: CacheExplainMatchKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logical_item: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_len: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_status: Option<CacheExplainObjectStatus>,
}

/// Kind of matched cache item.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheExplainMatchKind {
    Object,
    Record,
}

/// State of the object referenced by a cache record.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheExplainObjectStatus {
    Present,
    Missing,
    LengthMismatch,
    DigestMismatch,
}

/// Cache verification issue family.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheVerifyIssueKind {
    InvalidObjectPath,
    InvalidQuery,
    ObjectDigestMismatch,
    InvalidRecordPath,
    RecordDecode,
    RecordKeyMismatch,
    RecordObjectMissing,
    RecordObjectLengthMismatch,
    Io,
    UnsafePrunePath,
}

/// Cache inspection failure.
#[derive(Debug, Error)]
pub enum CacheInspectError {
    #[error("failed to read cache directory `{path}`: {source}")]
    ReadDir {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to inspect cache file `{path}`: {source}")]
    Metadata {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Scans cache files without validating their contents.
pub fn cache_stats(root: &Path) -> Result<CacheStats, CacheInspectError> {
    let mut stats = CacheStats {
        root: root.display().to_string(),
        ..CacheStats::default()
    };
    if !root.exists() {
        return Ok(stats);
    }
    for file in cache_files(root)? {
        let len = metadata_len(&file)?;
        if is_under(&file, &root.join("objects").join("blake3")) {
            stats.object_files += 1;
            stats.object_bytes = stats.object_bytes.saturating_add(len);
        } else if is_under(&file, &root.join("records")) && file.extension_str() == Some("awci") {
            stats.record_files += 1;
            stats.record_bytes = stats.record_bytes.saturating_add(len);
        } else if is_under(&file, &root.join("locks")) && file.extension_str() == Some("lock") {
            stats.lock_files += 1;
        } else if is_temp_file(&file) {
            stats.temp_files += 1;
        } else {
            stats.other_files += 1;
        }
    }
    Ok(stats)
}

/// Verifies object digests, record paths, and record-object references.
pub fn verify_cache(root: &Path) -> Result<CacheVerifyReport, CacheInspectError> {
    let stats = cache_stats(root)?;
    let mut issues = Vec::new();
    if root.exists() {
        for file in cache_files(root)? {
            if is_under(&file, &root.join("objects").join("blake3")) {
                verify_object(root, &file, &mut issues);
            } else if is_under(&file, &root.join("records")) && file.extension_str() == Some("awci")
            {
                verify_record(root, &file, &mut issues);
            }
        }
    }
    Ok(CacheVerifyReport {
        status: if issues.is_empty() {
            CacheVerifyStatus::Ok
        } else {
            CacheVerifyStatus::Failed
        },
        stats,
        issues,
    })
}

/// Explains cache entries matching one 64-character BLAKE3 digest.
pub fn explain_cache(root: &Path, query: &str) -> Result<CacheExplainReport, CacheInspectError> {
    let mut report = CacheExplainReport {
        root: root.display().to_string(),
        query: query.to_owned(),
        status: CacheExplainStatus::Missing,
        matches: Vec::new(),
        issues: Vec::new(),
    };
    let Some(digest) = digest_from_hex(query) else {
        report.status = CacheExplainStatus::InvalidQuery;
        push_issue(
            &mut report.issues,
            root,
            CacheVerifyIssueKind::InvalidQuery,
            "cache explain expects a 64-character lowercase hexadecimal BLAKE3 digest",
        );
        return Ok(report);
    };
    if !root.exists() {
        return Ok(report);
    }
    for file in cache_files(root)? {
        if digest_from_object_path(root, &file) == Some(digest) {
            explain_object(&file, digest, &mut report);
        } else if is_under(&file, &root.join("records"))
            && file.extension_str() == Some("awci")
            && digest_from_record_path(root, &file) == Some(digest)
        {
            explain_record(root, &file, &mut report);
        }
    }
    if !report.matches.is_empty() {
        report.status = CacheExplainStatus::Found;
    }
    Ok(report)
}

/// Explains cache records matching one logical item label.
pub fn explain_cache_by_logical_item(
    root: &Path,
    logical_item: &str,
) -> Result<CacheExplainReport, CacheInspectError> {
    let mut report = CacheExplainReport {
        root: root.display().to_string(),
        query: logical_item.to_owned(),
        status: CacheExplainStatus::Missing,
        matches: Vec::new(),
        issues: Vec::new(),
    };
    if logical_item.is_empty() {
        report.status = CacheExplainStatus::InvalidQuery;
        push_issue(
            &mut report.issues,
            root,
            CacheVerifyIssueKind::InvalidQuery,
            "cache explain logical query must not be empty",
        );
        return Ok(report);
    }
    if !root.exists() {
        return Ok(report);
    }
    for file in cache_files(root)? {
        if is_under(&file, &root.join("records")) && file.extension_str() == Some("awci") {
            explain_record_if_logical_item(root, &file, logical_item, &mut report);
        }
    }
    if !report.matches.is_empty() {
        report.status = CacheExplainStatus::Found;
    }
    Ok(report)
}

/// Finds and optionally removes safe cache-prune candidates.
pub fn prune_cache(root: &Path, apply: bool) -> Result<CachePruneReport, CacheInspectError> {
    let mut report = CachePruneReport {
        root: root.display().to_string(),
        applied: apply,
        candidates: Vec::new(),
        removed_files: 0,
        removed_directories: 0,
        removed_bytes: 0,
        issues: Vec::new(),
    };
    if !root.exists() {
        return Ok(report);
    }
    let referenced_objects = referenced_object_digests(root)?;
    for file in cache_files(root)? {
        if is_temp_file(&file) {
            push_prune_candidate(
                &mut report,
                root,
                &file,
                CachePruneCandidateKind::TempFile,
                apply,
            )?;
        } else if let Some(digest) = digest_from_object_path(root, &file)
            && !referenced_objects.contains(&digest)
        {
            push_prune_candidate(
                &mut report,
                root,
                &file,
                CachePruneCandidateKind::UnreferencedObject,
                apply,
            )?;
        }
    }
    push_empty_directory_prune_candidates(&mut report, root, apply)?;
    Ok(report)
}

fn referenced_object_digests(root: &Path) -> Result<BTreeSet<BuildDigest>, CacheInspectError> {
    let mut digests = BTreeSet::new();
    for file in cache_files(root)? {
        if is_under(&file, &root.join("records"))
            && file.extension_str() == Some("awci")
            && let Ok(bytes) = fs::read(&file)
            && let Ok(record) = CacheRecord::from_slice(&bytes)
        {
            digests.insert(record.object_digest());
        }
    }
    Ok(digests)
}

fn push_prune_candidate(
    report: &mut CachePruneReport,
    root: &Path,
    path: &Path,
    kind: CachePruneCandidateKind,
    apply: bool,
) -> Result<(), CacheInspectError> {
    let bytes = metadata_len(path)?;
    report.candidates.push(CachePruneCandidate {
        path: path.display().to_string(),
        kind,
        bytes,
    });
    if apply {
        if path.strip_prefix(root).is_err() {
            push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::UnsafePrunePath,
                "prune candidate is outside the cache root",
            );
            return Ok(());
        }
        match fs::remove_file(path) {
            Ok(()) => {
                report.removed_files += 1;
                report.removed_bytes = report.removed_bytes.saturating_add(bytes);
            }
            Err(error) => push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::Io,
                format!("failed to remove prune candidate: {error}"),
            ),
        }
    }
    Ok(())
}

fn push_empty_directory_prune_candidates(
    report: &mut CachePruneReport,
    root: &Path,
    apply: bool,
) -> Result<(), CacheInspectError> {
    for path in cache_cleanup_directories(root)? {
        if is_empty_directory(&path)? {
            push_empty_directory_prune_candidate(report, root, &path, apply);
        }
    }
    Ok(())
}

fn push_empty_directory_prune_candidate(
    report: &mut CachePruneReport,
    root: &Path,
    path: &Path,
    apply: bool,
) {
    report.candidates.push(CachePruneCandidate {
        path: path.display().to_string(),
        kind: CachePruneCandidateKind::EmptyDirectory,
        bytes: 0,
    });
    if !apply {
        return;
    }
    if path.strip_prefix(root).is_err() || !is_cache_cleanup_directory(root, path) {
        push_issue(
            &mut report.issues,
            path,
            CacheVerifyIssueKind::UnsafePrunePath,
            "empty directory prune candidate is outside the cache cleanup roots",
        );
        return;
    }
    match fs::remove_dir(path) {
        Ok(()) => report.removed_directories += 1,
        Err(error) => push_issue(
            &mut report.issues,
            path,
            CacheVerifyIssueKind::Io,
            format!("failed to remove empty cache directory: {error}"),
        ),
    }
}

fn explain_object(path: &Path, digest: BuildDigest, report: &mut CacheExplainReport) {
    match fs::read(path) {
        Ok(bytes) => {
            let actual = BuildDigest::of(&bytes);
            let len = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
            report.matches.push(CacheExplainMatch {
                path: path.display().to_string(),
                kind: CacheExplainMatchKind::Object,
                artifact_kind: None,
                artifact_key: None,
                logical_item: None,
                object_digest: Some(digest.to_string()),
                object_len: Some(len),
                object_status: Some(if actual == digest {
                    CacheExplainObjectStatus::Present
                } else {
                    CacheExplainObjectStatus::DigestMismatch
                }),
            });
        }
        Err(error) => push_issue(
            &mut report.issues,
            path,
            CacheVerifyIssueKind::Io,
            format!("failed to read object: {error}"),
        ),
    }
}

fn explain_record(root: &Path, path: &Path, report: &mut CacheExplainReport) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::Io,
                format!("failed to read record: {error}"),
            );
            return;
        }
    };
    let record = match CacheRecord::from_slice(&bytes) {
        Ok(record) => record,
        Err(error) => {
            push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::RecordDecode,
                error.to_string(),
            );
            return;
        }
    };
    let object_status = match FilesystemCacheStore::new(root).read_object(record.object_digest()) {
        Ok(object) => match u64::try_from(object.len()).unwrap_or(u64::MAX) {
            len if len == record.object_len() => CacheExplainObjectStatus::Present,
            _ => CacheExplainObjectStatus::LengthMismatch,
        },
        Err(super::store::CacheStoreError::ObjectDigestMismatch { .. }) => {
            CacheExplainObjectStatus::DigestMismatch
        }
        Err(_) => CacheExplainObjectStatus::Missing,
    };
    report.matches.push(CacheExplainMatch {
        path: path.display().to_string(),
        kind: CacheExplainMatchKind::Record,
        artifact_kind: Some(record.artifact_kind().to_string()),
        artifact_key: Some(record.key().digest().to_string()),
        logical_item: record.logical_item().map(str::to_owned),
        object_digest: Some(record.object_digest().to_string()),
        object_len: Some(record.object_len()),
        object_status: Some(object_status),
    });
}

fn explain_record_if_logical_item(
    root: &Path,
    path: &Path,
    logical_item: &str,
    report: &mut CacheExplainReport,
) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::Io,
                format!("failed to read record: {error}"),
            );
            return;
        }
    };
    let record = match CacheRecord::from_slice(&bytes) {
        Ok(record) => record,
        Err(error) => {
            push_issue(
                &mut report.issues,
                path,
                CacheVerifyIssueKind::RecordDecode,
                error.to_string(),
            );
            return;
        }
    };
    if record.logical_item() == Some(logical_item) {
        explain_record(root, path, report);
    }
}

fn verify_object(root: &Path, path: &Path, issues: &mut Vec<CacheVerifyIssue>) {
    let Some(expected) = digest_from_object_path(root, path) else {
        push_issue(
            issues,
            path,
            CacheVerifyIssueKind::InvalidObjectPath,
            "object path must be objects/blake3/<2 hex>/<62 hex>",
        );
        return;
    };
    match fs::read(path) {
        Ok(bytes) => {
            let actual = BuildDigest::of(&bytes);
            if actual != expected {
                push_issue(
                    issues,
                    path,
                    CacheVerifyIssueKind::ObjectDigestMismatch,
                    format!("expected {expected}, actual {actual}"),
                );
            }
        }
        Err(error) => push_issue(
            issues,
            path,
            CacheVerifyIssueKind::Io,
            format!("failed to read object: {error}"),
        ),
    }
}

fn verify_record(root: &Path, path: &Path, issues: &mut Vec<CacheVerifyIssue>) {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            push_issue(
                issues,
                path,
                CacheVerifyIssueKind::Io,
                format!("failed to read record: {error}"),
            );
            return;
        }
    };
    let record = match CacheRecord::from_slice(&bytes) {
        Ok(record) => record,
        Err(error) => {
            push_issue(
                issues,
                path,
                CacheVerifyIssueKind::RecordDecode,
                error.to_string(),
            );
            return;
        }
    };
    let Some(path_digest) = digest_from_record_path(root, path) else {
        push_issue(
            issues,
            path,
            CacheVerifyIssueKind::InvalidRecordPath,
            "record path must be records/<query>/<2 hex>/<62 hex>.awci",
        );
        return;
    };
    if path_digest != record.key().digest() {
        push_issue(
            issues,
            path,
            CacheVerifyIssueKind::RecordKeyMismatch,
            format!(
                "record path key is {path_digest}, record payload key is {}",
                record.key().digest()
            ),
        );
    }

    let store = FilesystemCacheStore::new(root);
    match store.read_object(record.object_digest()) {
        Ok(object) => {
            let actual = u64::try_from(object.len()).unwrap_or(u64::MAX);
            if actual != record.object_len() {
                push_issue(
                    issues,
                    path,
                    CacheVerifyIssueKind::RecordObjectLengthMismatch,
                    format!(
                        "record object length expected {}, actual {actual}",
                        record.object_len()
                    ),
                );
            }
        }
        Err(error) => push_issue(
            issues,
            path,
            CacheVerifyIssueKind::RecordObjectMissing,
            error.to_string(),
        ),
    }
}

fn cache_files(root: &Path) -> Result<Vec<PathBuf>, CacheInspectError> {
    let mut files = Vec::new();
    collect_files(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn cache_cleanup_directories(root: &Path) -> Result<Vec<PathBuf>, CacheInspectError> {
    let mut directories = Vec::new();
    for base in cache_cleanup_roots(root) {
        collect_child_directories(&base, &mut directories)?;
    }
    directories.sort_by(|left, right| {
        right
            .components()
            .count()
            .cmp(&left.components().count())
            .then_with(|| left.cmp(right))
    });
    Ok(directories)
}

fn cache_cleanup_roots(root: &Path) -> [PathBuf; 3] {
    [
        root.join("objects").join("blake3"),
        root.join("records"),
        root.join("locks"),
    ]
}

fn collect_child_directories(
    path: &Path,
    directories: &mut Vec<PathBuf>,
) -> Result<(), CacheInspectError> {
    if !path.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|source| CacheInspectError::ReadDir {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CacheInspectError::ReadDir {
            path: path.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| CacheInspectError::Metadata {
                path: path.clone(),
                source,
            })?;
        if metadata.is_dir() {
            collect_child_directories(&path, directories)?;
            directories.push(path);
        }
    }
    Ok(())
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<(), CacheInspectError> {
    if !path.exists() {
        return Ok(());
    }
    let entries = fs::read_dir(path).map_err(|source| CacheInspectError::ReadDir {
        path: path.to_path_buf(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| CacheInspectError::ReadDir {
            path: path.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| CacheInspectError::Metadata {
                path: path.clone(),
                source,
            })?;
        if metadata.is_dir() {
            collect_files(&path, files)?;
        } else if metadata.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn metadata_len(path: &Path) -> Result<u64, CacheInspectError> {
    path.metadata()
        .map(|metadata| metadata.len())
        .map_err(|source| CacheInspectError::Metadata {
            path: path.to_path_buf(),
            source,
        })
}

fn is_empty_directory(path: &Path) -> Result<bool, CacheInspectError> {
    let mut entries = fs::read_dir(path).map_err(|source| CacheInspectError::ReadDir {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(entries.next().is_none())
}

fn digest_from_object_path(root: &Path, path: &Path) -> Option<BuildDigest> {
    let relative = path
        .strip_prefix(root.join("objects").join("blake3"))
        .ok()?;
    let mut components = relative.components();
    let first = components.next()?.as_os_str().to_str()?;
    let rest = components.next()?.as_os_str().to_str()?;
    if components.next().is_some() || first.len() != 2 || rest.len() != 62 {
        return None;
    }
    digest_from_hex(&format!("{first}{rest}"))
}

fn digest_from_record_path(root: &Path, path: &Path) -> Option<BuildDigest> {
    let relative = path.strip_prefix(root.join("records")).ok()?;
    let components = relative.components().collect::<Vec<_>>();
    if components.len() != 3 {
        return None;
    }
    let first = components[1].as_os_str().to_str()?;
    let rest = components[2].as_os_str().to_str()?.strip_suffix(".awci")?;
    if first.len() != 2 || rest.len() != 62 {
        return None;
    }
    digest_from_hex(&format!("{first}{rest}"))
}

fn digest_from_hex(hex: &str) -> Option<BuildDigest> {
    if hex.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in hex.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        bytes[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(BuildDigest::from_bytes(bytes))
}

fn is_under(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).is_ok()
}

fn is_cache_cleanup_directory(root: &Path, path: &Path) -> bool {
    cache_cleanup_roots(root).iter().any(|cleanup_root| {
        path.strip_prefix(cleanup_root)
            .is_ok_and(|relative| !relative.as_os_str().is_empty())
    })
}

fn is_temp_file(path: &Path) -> bool {
    path.file_name_str()
        .is_some_and(|name| name.starts_with(".tmp-"))
}

fn push_issue(
    issues: &mut Vec<CacheVerifyIssue>,
    path: &Path,
    kind: CacheVerifyIssueKind,
    message: impl Into<String>,
) {
    issues.push(CacheVerifyIssue {
        path: path.display().to_string(),
        kind,
        message: message.into(),
    });
}

trait PathExt {
    fn extension_str(&self) -> Option<&str>;
    fn file_name_str(&self) -> Option<&str>;
}

impl PathExt for Path {
    fn extension_str(&self) -> Option<&str> {
        self.extension().and_then(|value| value.to_str())
    }

    fn file_name_str(&self) -> Option<&str> {
        self.file_name().and_then(|value| value.to_str())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CacheExplainStatus, CachePruneCandidateKind, CacheVerifyStatus, cache_stats, explain_cache,
        explain_cache_by_logical_item, prune_cache, verify_cache,
    };
    use crate::cache::store::FilesystemCacheStore;
    use arcweft_project::{
        artifact::{ArtifactKey, ArtifactKeyInput, ArtifactKind},
        fingerprint::BuildDigest,
        incremental::QueryKind,
    };
    use std::{
        fs,
        path::{Path, PathBuf},
        time::{SystemTime, UNIX_EPOCH},
    };

    #[test]
    fn cache_stats_and_verify_accept_store_artifacts() {
        let root = temp_root("ok");
        let store = FilesystemCacheStore::new(&root);
        store
            .store_artifact(
                QueryKind::Parse,
                key(),
                ArtifactKind::ParsedSyntax,
                b"artifact",
            )
            .expect("artifact stored");

        let stats = cache_stats(&root).expect("stats");
        let report = verify_cache(&root).expect("verify");

        assert_eq!(stats.object_files, 1);
        assert_eq!(stats.record_files, 1);
        assert_eq!(report.status, CacheVerifyStatus::Ok);
        assert!(report.issues.is_empty());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn verify_reports_corrupt_object_digest() {
        let root = temp_root("corrupt-object");
        let store = FilesystemCacheStore::new(&root);
        let digest = store.put_object(b"artifact").expect("object stored");
        fs::write(object_path(&root, digest), b"changed").expect("object corrupts");

        let report = verify_cache(&root).expect("verify");

        assert_eq!(report.status, CacheVerifyStatus::Failed);
        assert!(report.issues.iter().any(|issue| {
            matches!(
                issue.kind,
                super::CacheVerifyIssueKind::ObjectDigestMismatch
            )
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explain_cache_finds_record_by_artifact_key() {
        let root = temp_root("explain-record");
        let key = key();
        let store = FilesystemCacheStore::new(&root);
        store
            .store_artifact(
                QueryKind::Parse,
                key,
                ArtifactKind::ParsedSyntax,
                b"artifact",
            )
            .expect("artifact stored");

        let report = explain_cache(&root, &key.digest().to_hex()).expect("explain");

        assert_eq!(report.status, CacheExplainStatus::Found);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(
            report.matches[0].artifact_kind.as_deref(),
            Some("parsed_syntax")
        );
        assert_eq!(
            report.matches[0].object_status,
            Some(super::CacheExplainObjectStatus::Present)
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explain_cache_finds_record_by_logical_item() {
        let root = temp_root("explain-logical");
        let key = key();
        let store = FilesystemCacheStore::new(&root);
        store
            .store_artifact_with_logical_item(
                QueryKind::Parse,
                key,
                ArtifactKind::ParsedSyntax,
                Some("crate"),
                b"artifact",
            )
            .expect("artifact stored");

        let report = explain_cache_by_logical_item(&root, "crate").expect("explain");

        assert_eq!(report.status, CacheExplainStatus::Found);
        assert_eq!(report.matches.len(), 1);
        assert_eq!(report.matches[0].logical_item.as_deref(), Some("crate"));
        assert_eq!(report.matches[0].artifact_key, Some(key.digest().to_hex()));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn explain_cache_reports_invalid_digest_query() {
        let root = temp_root("explain-invalid");

        let report = explain_cache(&root, "not-a-digest").expect("explain");

        assert_eq!(report.status, CacheExplainStatus::InvalidQuery);
        assert_eq!(report.issues.len(), 1);
    }

    #[test]
    fn prune_cache_dry_run_reports_temp_and_unreferenced_objects_without_removing() {
        let root = temp_root("prune-dry-run");
        let store = FilesystemCacheStore::new(&root);
        let digest = store.put_object(b"unreferenced").expect("object stored");
        let temp = root
            .join("objects")
            .join("blake3")
            .join(".tmp-fixture.awci");
        fs::create_dir_all(temp.parent().expect("temp parent")).expect("temp parent creates");
        fs::write(&temp, b"temp").expect("temp writes");

        let report = prune_cache(&root, false).expect("prune");

        assert!(!report.applied);
        assert_eq!(report.removed_files, 0);
        assert!(object_path(&root, digest).is_file());
        assert!(temp.is_file());
        assert!(report.candidates.iter().any(|candidate| {
            matches!(candidate.kind, CachePruneCandidateKind::UnreferencedObject)
        }));
        assert!(
            report
                .candidates
                .iter()
                .any(|candidate| matches!(candidate.kind, CachePruneCandidateKind::TempFile))
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prune_cache_dry_run_reports_empty_directories_without_removing() {
        let root = temp_root("prune-empty-dir-dry-run");
        let empty_shard = root.join("objects").join("blake3").join("ab");
        fs::create_dir_all(&empty_shard).expect("empty shard creates");

        let report = prune_cache(&root, false).expect("prune");

        assert!(!report.applied);
        assert_eq!(report.removed_directories, 0);
        assert!(empty_shard.is_dir());
        assert!(report.candidates.iter().any(|candidate| {
            candidate.path == empty_shard.display().to_string()
                && matches!(candidate.kind, CachePruneCandidateKind::EmptyDirectory)
                && candidate.bytes == 0
        }));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prune_cache_apply_removes_only_candidates() {
        let root = temp_root("prune-apply");
        let key = key();
        let store = FilesystemCacheStore::new(&root);
        store
            .store_artifact(
                QueryKind::Parse,
                key,
                ArtifactKind::ParsedSyntax,
                b"referenced",
            )
            .expect("referenced artifact stored");
        let orphan = store.put_object(b"orphan").expect("orphan stored");

        let report = prune_cache(&root, true).expect("prune");

        assert!(report.applied);
        assert_eq!(report.removed_files, 1);
        assert!(!object_path(&root, orphan).exists());
        assert!(
            verify_cache(&root)
                .expect("cache remains valid")
                .issues
                .is_empty()
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prune_cache_apply_removes_empty_parent_directory_after_object_removal() {
        let root = temp_root("prune-object-parent-dir");
        let object = object_path(&root, BuildDigest::from_bytes([0_u8; 32]));
        let parent = object.parent().expect("object parent").to_path_buf();
        fs::create_dir_all(&parent).expect("object parent creates");
        fs::write(&object, b"orphan").expect("orphan writes");

        let report = prune_cache(&root, true).expect("prune");

        assert!(report.applied);
        assert_eq!(report.removed_files, 1);
        assert_eq!(report.removed_directories, 1);
        assert!(!object.exists());
        assert!(!parent.exists());
        assert!(report.candidates.iter().any(|candidate| {
            candidate.path == parent.display().to_string()
                && matches!(candidate.kind, CachePruneCandidateKind::EmptyDirectory)
        }));
        let _ = fs::remove_dir_all(root);
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

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-cache-inspect-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    fn object_path(root: &Path, digest: BuildDigest) -> PathBuf {
        let hex = digest.to_hex();
        root.join("objects")
            .join("blake3")
            .join(&hex[..2])
            .join(&hex[2..])
    }
}
