use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

/// Local publication artifact family. The commit order publishes AWFR last so
/// consumers never observe the final archive before referenced bytes/signatures
/// have been staged into their destination paths.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePublishArtifactKind {
    AwfbBundle,
    PatchArtifact,
    ExternalPayload,
    Signature,
    AwfrArchive,
}

/// One byte object staged and committed by the local release-publish adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleasePublishArtifactBytes {
    pub kind: ReleasePublishArtifactKind,
    pub relative_path: PathBuf,
    pub bytes: Vec<u8>,
}

/// Atomic local publication plan for a single AWFR release cut.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleasePublishPlan {
    pub destination_root: PathBuf,
    pub staging_root: Option<PathBuf>,
    pub artifacts: Vec<ReleasePublishArtifactBytes>,
}

/// Publication report emitted after all files have been committed.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleasePublishReport {
    pub destination_root: String,
    pub staging_root: String,
    pub artifacts: Vec<ReleasePublishedArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleasePublishedArtifact {
    pub kind: ReleasePublishArtifactKind,
    pub path: String,
    pub byte_len: u64,
}

#[derive(Debug, Error)]
pub enum ReleasePublishError {
    #[error("release publish plan contains no artifacts")]
    EmptyPlan,
    #[error("invalid publish path `{path}`: {message}")]
    InvalidPublishPath { path: PathBuf, message: String },
    #[error("duplicate publish path `{0}`")]
    DuplicatePublishPath(PathBuf),
    #[error("failed to create release staging directory `{path}`: {source}")]
    CreateStaging {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create staged artifact parent `{path}`: {source}")]
    CreateStagedParent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write staged artifact `{path}`: {source}")]
    WriteStaged {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to create destination parent `{path}`: {source}")]
    CreateDestinationParent {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("destination artifact `{0}` already exists")]
    DestinationExists(PathBuf),
    #[error("failed to commit staged artifact `{from}` to `{to}`: {source}")]
    CommitArtifact {
        from: PathBuf,
        to: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to remove release staging directory `{path}` after commit: {source}")]
    CleanupStaging {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Stages all artifacts under a recoverable staging directory, validates every
/// destination path before writing, and commits by atomic renames. If any commit
/// step fails, already committed files are best-effort removed and the staging
/// directory remains available for operator recovery.
pub fn publish_release_atomically(
    plan: &ReleasePublishPlan,
) -> Result<ReleasePublishReport, ReleasePublishError> {
    if plan.artifacts.is_empty() {
        return Err(ReleasePublishError::EmptyPlan);
    }
    validate_publish_paths(
        plan.artifacts
            .iter()
            .map(|artifact| &artifact.relative_path),
    )?;
    let staging_root = staging_root(plan);
    fs::create_dir_all(&staging_root).map_err(|source| ReleasePublishError::CreateStaging {
        path: staging_root.clone(),
        source,
    })?;

    for artifact in &plan.artifacts {
        let staged_path = staging_root.join(&artifact.relative_path);
        if let Some(parent) = staged_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                ReleasePublishError::CreateStagedParent {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        fs::write(&staged_path, &artifact.bytes).map_err(|source| {
            ReleasePublishError::WriteStaged {
                path: staged_path,
                source,
            }
        })?;
    }

    let mut order = (0..plan.artifacts.len()).collect::<Vec<_>>();
    order.sort_by_key(|index| plan.artifacts[*index].kind.commit_rank());
    let mut committed = Vec::new();
    for index in order {
        let artifact = &plan.artifacts[index];
        let staged_path = staging_root.join(&artifact.relative_path);
        let destination_path = plan.destination_root.join(&artifact.relative_path);
        if destination_path.exists() {
            rollback_committed(&committed);
            return Err(ReleasePublishError::DestinationExists(destination_path));
        }
        if let Some(parent) = destination_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                rollback_committed(&committed);
                ReleasePublishError::CreateDestinationParent {
                    path: parent.to_path_buf(),
                    source,
                }
            })?;
        }
        fs::rename(&staged_path, &destination_path).map_err(|source| {
            rollback_committed(&committed);
            ReleasePublishError::CommitArtifact {
                from: staged_path,
                to: destination_path.clone(),
                source,
            }
        })?;
        committed.push(destination_path);
    }

    if staging_root.exists() {
        fs::remove_dir_all(&staging_root).map_err(|source| {
            ReleasePublishError::CleanupStaging {
                path: staging_root.clone(),
                source,
            }
        })?;
    }

    Ok(ReleasePublishReport {
        destination_root: plan.destination_root.display().to_string(),
        staging_root: staging_root.display().to_string(),
        artifacts: plan
            .artifacts
            .iter()
            .map(|artifact| ReleasePublishedArtifact {
                kind: artifact.kind,
                path: plan
                    .destination_root
                    .join(&artifact.relative_path)
                    .display()
                    .to_string(),
                byte_len: u64::try_from(artifact.bytes.len()).unwrap_or(u64::MAX),
            })
            .collect(),
    })
}

impl ReleasePublishArtifactKind {
    const fn commit_rank(self) -> u8 {
        match self {
            Self::AwfbBundle => 0,
            Self::PatchArtifact => 1,
            Self::ExternalPayload => 2,
            Self::Signature => 3,
            Self::AwfrArchive => 4,
        }
    }
}

fn validate_publish_paths<'a>(
    paths: impl IntoIterator<Item = &'a PathBuf>,
) -> Result<(), ReleasePublishError> {
    let mut normalized = Vec::new();
    for path in paths {
        validate_relative_publish_path(path)?;
        if normalized.iter().any(|seen| seen == path) {
            return Err(ReleasePublishError::DuplicatePublishPath(path.clone()));
        }
        normalized.push(path.clone());
    }
    Ok(())
}

fn validate_relative_publish_path(path: &Path) -> Result<(), ReleasePublishError> {
    if path.as_os_str().is_empty() {
        return Err(ReleasePublishError::InvalidPublishPath {
            path: path.to_path_buf(),
            message: "path must not be empty".to_owned(),
        });
    }
    if path.is_absolute() {
        return Err(ReleasePublishError::InvalidPublishPath {
            path: path.to_path_buf(),
            message: "absolute paths are not allowed".to_owned(),
        });
    }
    if path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(ReleasePublishError::InvalidPublishPath {
            path: path.to_path_buf(),
            message: "path must remain inside the destination root".to_owned(),
        });
    }
    Ok(())
}

fn staging_root(plan: &ReleasePublishPlan) -> PathBuf {
    let base = plan
        .staging_root
        .clone()
        .unwrap_or_else(|| plan.destination_root.join(".arcweft-publish-staging"));
    base.join(format!(
        "publish-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_nanos())
    ))
}

fn rollback_committed(paths: &[PathBuf]) {
    for path in paths.iter().rev() {
        let _ = fs::remove_file(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn publish_release_atomically_commits_all_artifact_families() {
        let root = temp_root("publish-all");
        let plan = ReleasePublishPlan {
            destination_root: root.join("dest"),
            staging_root: Some(root.join("stage")),
            artifacts: vec![
                artifact(ReleasePublishArtifactKind::AwfbBundle, "game.awfb", b"awfb"),
                artifact(
                    ReleasePublishArtifactKind::PatchArtifact,
                    "game.awfp",
                    b"patch",
                ),
                artifact(
                    ReleasePublishArtifactKind::ExternalPayload,
                    "payloads/voice.bin",
                    b"payload",
                ),
                artifact(
                    ReleasePublishArtifactKind::Signature,
                    "game.awfr.sig",
                    b"sig",
                ),
                artifact(
                    ReleasePublishArtifactKind::AwfrArchive,
                    "game.awfr",
                    b"awfr",
                ),
            ],
        };

        let report = publish_release_atomically(&plan).expect("publish succeeds");

        assert_eq!(report.artifacts.len(), 5);
        assert_eq!(
            fs::read(root.join("dest/game.awfb")).expect("awfb reads"),
            b"awfb"
        );
        assert_eq!(
            fs::read(root.join("dest/game.awfp")).expect("patch reads"),
            b"patch"
        );
        assert_eq!(
            fs::read(root.join("dest/payloads/voice.bin")).expect("payload reads"),
            b"payload"
        );
        assert_eq!(
            fs::read(root.join("dest/game.awfr.sig")).expect("sig reads"),
            b"sig"
        );
        assert_eq!(
            fs::read(root.join("dest/game.awfr")).expect("awfr reads"),
            b"awfr"
        );
        assert!(!Path::new(&report.staging_root).exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn publish_release_rejects_escape_paths_before_writing() {
        let root = temp_root("publish-path-reject");
        let plan = ReleasePublishPlan {
            destination_root: root.join("dest"),
            staging_root: Some(root.join("stage")),
            artifacts: vec![artifact(
                ReleasePublishArtifactKind::AwfrArchive,
                "../game.awfr",
                b"awfr",
            )],
        };

        let error = publish_release_atomically(&plan).expect_err("path escape rejects");

        assert!(matches!(
            error,
            ReleasePublishError::InvalidPublishPath { .. }
        ));
        assert!(!root.join("dest").exists());
        assert!(!root.join("stage").exists());
        let _ = fs::remove_dir_all(root);
    }

    fn artifact(
        kind: ReleasePublishArtifactKind,
        relative_path: impl Into<PathBuf>,
        bytes: &[u8],
    ) -> ReleasePublishArtifactBytes {
        ReleasePublishArtifactBytes {
            kind,
            relative_path: relative_path.into(),
            bytes: bytes.to_vec(),
        }
    }

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "arcweft-release-{label}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }
}
