use super::{
    ReleasePublishArtifactBytes, ReleasePublishArtifactKind, validate_relative_publish_path,
};
use arcweft_bundle::container::BundleDigest;
use serde::Serialize;
use std::{
    collections::BTreeSet,
    fmt,
    path::{Component, Path},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;

mod object_directory;

pub use object_directory::ReleaseObjectDirectoryBackend;

/// Backend boundary for remote publication adapters owned by project-loader.
///
/// Implementations model object-store semantics: staged writes are not consumer
/// visible, committed keys are visible, and deletion is best-effort recovery.
pub trait ReleaseRemotePublicationBackend {
    fn backend_id(&self) -> &'static str;

    fn put_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
        bytes: &[u8],
    ) -> Result<(), ReleaseRemoteBackendError>;

    fn read_object(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<Vec<u8>, ReleaseRemoteBackendError>;

    fn copy_object(
        &mut self,
        from: &ReleaseRemoteObjectKey,
        to: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError>;

    fn delete_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError>;

    fn object_exists(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<bool, ReleaseRemoteBackendError>;
}

/// Validated relative object key. Keys are serialized with `/` separators.
#[derive(Clone, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReleaseRemoteObjectKey(String);

/// Publication target. The concrete backend owns credentials and root handles;
/// the target only carries the stable destination prefix recorded in reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseRemotePublishTarget {
    pub prefix: Option<ReleaseRemoteObjectKey>,
}

/// One release byte object to publish to a remote backend.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseRemotePublishArtifact {
    pub kind: ReleasePublishArtifactKind,
    pub object_key: ReleaseRemoteObjectKey,
    pub bytes: Vec<u8>,
    pub requires_signature: bool,
    pub external_payload_mirrors: Vec<String>,
}

/// Adapter policy for retry, timeout, and byte-budget behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReleaseRemotePublishPolicy {
    pub max_attempts: u8,
    pub byte_budget: u64,
    pub timeout_millis: Option<u64>,
}

/// Signing expectations consumed by the remote publish planner.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
pub struct ReleaseRemoteSigningRequirements {
    pub require_signature_artifact: bool,
    pub require_awfr_signature_reference: bool,
}

/// AWFR finalization guard. Seq02.10 keeps AWFR archive publication last.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseRemoteAwfrFinalization {
    pub require_exactly_one_awfr_archive: bool,
}

/// Credentials supplied by an adapter or CLI. Secrets never serialize and Debug
/// output records only whether a secret was present.
#[derive(Clone, Eq, PartialEq)]
pub struct ReleaseRemoteCredentials {
    profile: Option<String>,
    secret: Option<String>,
}

/// Full remote publication plan. Artifact bytes stay in memory and are never
/// serialized into reports.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseRemotePublishPlan {
    pub target: ReleaseRemotePublishTarget,
    pub artifacts: Vec<ReleaseRemotePublishArtifact>,
    pub policy: ReleaseRemotePublishPolicy,
    pub signing: ReleaseRemoteSigningRequirements,
    pub awfr_finalization: ReleaseRemoteAwfrFinalization,
    pub credentials: ReleaseRemoteCredentials,
    pub run_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRemotePublishMode {
    DryRun,
    Commit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRemoteArtifactState {
    Planned,
    Staged,
    Uploaded,
    Committed,
    RolledBack,
    Abandoned,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRemotePublishOperation {
    CheckExists,
    PutStaged,
    ReadStaged,
    UploadCommitted,
    ReadCommitted,
    DeleteCommitted,
    DeleteStaged,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseRemoteBackendErrorKind {
    Retryable,
    NonRetryable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseRemoteCredentialReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    pub secret: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseRemotePublishedArtifact {
    pub kind: ReleasePublishArtifactKind,
    pub object_key: String,
    pub destination_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staging_key: Option<String>,
    pub digest: String,
    pub byte_len: u64,
    pub commit_rank: u8,
    pub commit_order: usize,
    pub state: ReleaseRemoteArtifactState,
    pub requires_signature: bool,
    pub final_awfr: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub external_payload_mirrors: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseRemotePublishEvent {
    pub operation: ReleaseRemotePublishOperation,
    pub kind: ReleasePublishArtifactKind,
    pub destination_key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staging_key: Option<String>,
    pub state: ReleaseRemoteArtifactState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseRemotePublishReport {
    pub mode: ReleaseRemotePublishMode,
    pub backend: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remote_prefix: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub staging_prefix: Option<String>,
    pub byte_budget: u64,
    pub total_byte_len: u64,
    pub signing: ReleaseRemoteSigningRequirements,
    pub awfr_finalization: ReleaseRemoteAwfrFinalization,
    pub credential: ReleaseRemoteCredentialReport,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ReleaseRemotePublishErrorKind>,
    pub artifacts: Vec<ReleaseRemotePublishedArtifact>,
    pub events: Vec<ReleaseRemotePublishEvent>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ReleaseRemotePublishErrorKind {
    #[error("remote publish plan contains no artifacts")]
    EmptyPlan,
    #[error("invalid remote object key `{key}`: {message}")]
    InvalidObjectKey { key: String, message: String },
    #[error("duplicate remote object key `{key}`")]
    DuplicateObjectKey { key: String },
    #[error(
        "remote publish byte budget exceeded: {total_byte_len} byte(s) > {byte_budget} byte(s)"
    )]
    ByteBudgetExceeded {
        total_byte_len: u64,
        byte_budget: u64,
    },
    #[error("remote publish policy is invalid: {message}")]
    InvalidPolicy { message: String },
    #[error("remote publish requires at least one signature artifact")]
    MissingSignatureArtifact,
    #[error("remote publish requires exactly one final AWFR archive artifact; found {count}")]
    MissingFinalAwfrArchive { count: usize },
    #[error("remote object `{key}` already exists")]
    DestinationExists { key: String },
    #[error("remote backend {operation:?} failed for `{key}`: {message}")]
    Backend {
        operation: ReleaseRemotePublishOperation,
        key: String,
        retryable: bool,
        message: String,
    },
    #[error(
        "remote checksum mismatch for `{key}`: expected {expected_digest}/{expected_byte_len} byte(s), actual {actual_digest}/{actual_byte_len} byte(s)"
    )]
    ChecksumMismatch {
        key: String,
        expected_digest: String,
        actual_digest: String,
        expected_byte_len: u64,
        actual_byte_len: u64,
    },
    #[error("remote publish timed out during {operation:?} for `{key}`")]
    Timeout {
        operation: ReleaseRemotePublishOperation,
        key: String,
    },
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("remote release publication failed: {error}")]
pub struct ReleaseRemotePublishFailure {
    pub error: ReleaseRemotePublishErrorKind,
    pub report: Box<ReleaseRemotePublishReport>,
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("{kind:?}: {message}")]
pub struct ReleaseRemoteBackendError {
    pub kind: ReleaseRemoteBackendErrorKind,
    pub message: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VisibleRemoteObject {
    destination_key: ReleaseRemoteObjectKey,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StagedRemoteObject {
    destination_key: ReleaseRemoteObjectKey,
    staging_key: ReleaseRemoteObjectKey,
}

impl ReleaseRemoteObjectKey {
    pub fn new(value: impl Into<String>) -> Result<Self, ReleaseRemotePublishErrorKind> {
        let value = value.into();
        Self::from_relative_path(Path::new(&value))
    }

    pub fn from_relative_path(path: &Path) -> Result<Self, ReleaseRemotePublishErrorKind> {
        validate_relative_publish_path(path).map_err(|error| {
            ReleaseRemotePublishErrorKind::InvalidObjectKey {
                key: path.display().to_string(),
                message: error.to_string(),
            }
        })?;
        let mut key = String::new();
        for component in path.components() {
            match component {
                Component::Normal(segment) => {
                    let segment = segment.to_str().ok_or_else(|| {
                        ReleaseRemotePublishErrorKind::InvalidObjectKey {
                            key: path.display().to_string(),
                            message: "object key must be valid UTF-8".to_owned(),
                        }
                    })?;
                    if !key.is_empty() {
                        key.push('/');
                    }
                    key.push_str(segment);
                }
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(ReleaseRemotePublishErrorKind::InvalidObjectKey {
                        key: path.display().to_string(),
                        message: "object key must remain inside the remote prefix".to_owned(),
                    });
                }
            }
        }
        if key.is_empty() {
            return Err(ReleaseRemotePublishErrorKind::InvalidObjectKey {
                key: path.display().to_string(),
                message: "object key must not be empty".to_owned(),
            });
        }
        Ok(Self(key))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    fn join(&self, child: &Self) -> Self {
        Self(format!("{}/{}", self.0, child.0))
    }
}

impl fmt::Debug for ReleaseRemoteObjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("ReleaseRemoteObjectKey")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for ReleaseRemoteObjectKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl ReleaseRemotePublishTarget {
    pub fn new(prefix: Option<String>) -> Result<Self, ReleaseRemotePublishErrorKind> {
        let prefix = prefix
            .filter(|value| !value.is_empty())
            .map(ReleaseRemoteObjectKey::new)
            .transpose()?;
        Ok(Self { prefix })
    }

    fn destination_key(&self, artifact_key: &ReleaseRemoteObjectKey) -> ReleaseRemoteObjectKey {
        self.prefix
            .as_ref()
            .map_or_else(|| artifact_key.clone(), |prefix| prefix.join(artifact_key))
    }
}

impl ReleaseRemotePublishArtifact {
    pub fn new(
        kind: ReleasePublishArtifactKind,
        object_key: ReleaseRemoteObjectKey,
        bytes: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            object_key,
            bytes,
            requires_signature: false,
            external_payload_mirrors: Vec::new(),
        }
    }

    pub fn from_local_artifact(
        artifact: ReleasePublishArtifactBytes,
    ) -> Result<Self, ReleaseRemotePublishErrorKind> {
        let object_key = ReleaseRemoteObjectKey::from_relative_path(&artifact.relative_path)?;
        Ok(Self::new(artifact.kind, object_key, artifact.bytes))
    }

    #[must_use]
    pub fn with_signature_requirement(mut self, required: bool) -> Self {
        self.requires_signature = required;
        self
    }

    #[must_use]
    pub fn with_external_payload_mirrors(
        mut self,
        mirrors: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.external_payload_mirrors = mirrors.into_iter().map(Into::into).collect();
        self
    }
}

impl Default for ReleaseRemotePublishPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 1,
            byte_budget: u64::MAX,
            timeout_millis: None,
        }
    }
}

impl ReleaseRemotePublishPolicy {
    pub fn new(
        max_attempts: u8,
        byte_budget: u64,
        timeout_millis: Option<u64>,
    ) -> Result<Self, ReleaseRemotePublishErrorKind> {
        let policy = Self {
            max_attempts,
            byte_budget,
            timeout_millis,
        };
        policy.validate()?;
        Ok(policy)
    }

    fn validate(&self) -> Result<(), ReleaseRemotePublishErrorKind> {
        if self.max_attempts == 0 {
            return Err(ReleaseRemotePublishErrorKind::InvalidPolicy {
                message: "max_attempts must be greater than zero".to_owned(),
            });
        }
        if self.byte_budget == 0 {
            return Err(ReleaseRemotePublishErrorKind::InvalidPolicy {
                message: "byte_budget must be greater than zero".to_owned(),
            });
        }
        if self.timeout_millis == Some(0) {
            return Err(ReleaseRemotePublishErrorKind::InvalidPolicy {
                message: "timeout_millis must be greater than zero when set".to_owned(),
            });
        }
        Ok(())
    }

    fn deadline(&self) -> Option<Instant> {
        self.timeout_millis
            .map(|timeout| Instant::now() + Duration::from_millis(timeout))
    }
}

impl Default for ReleaseRemoteAwfrFinalization {
    fn default() -> Self {
        Self {
            require_exactly_one_awfr_archive: true,
        }
    }
}

impl ReleaseRemoteCredentials {
    pub fn none() -> Self {
        Self {
            profile: None,
            secret: None,
        }
    }

    pub fn new(profile: Option<String>, secret: Option<String>) -> Self {
        Self { profile, secret }
    }

    fn report(&self) -> ReleaseRemoteCredentialReport {
        ReleaseRemoteCredentialReport {
            profile: self.profile.clone(),
            secret: self.secret.as_ref().map(|_| "<redacted>".to_owned()),
        }
    }

    fn redact(&self, message: impl Into<String>) -> String {
        let message = message.into();
        self.secret.as_ref().map_or(message.clone(), |secret| {
            if secret.is_empty() {
                message
            } else {
                message.replace(secret, "<redacted>")
            }
        })
    }
}

impl fmt::Debug for ReleaseRemoteCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ReleaseRemoteCredentials")
            .field("profile", &self.profile)
            .field("secret", &self.secret.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

impl ReleaseRemotePublishPlan {
    pub fn new(
        target: ReleaseRemotePublishTarget,
        artifacts: Vec<ReleaseRemotePublishArtifact>,
    ) -> Self {
        Self {
            target,
            artifacts,
            policy: ReleaseRemotePublishPolicy::default(),
            signing: ReleaseRemoteSigningRequirements::default(),
            awfr_finalization: ReleaseRemoteAwfrFinalization::default(),
            credentials: ReleaseRemoteCredentials::none(),
            run_id: None,
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: ReleaseRemotePublishPolicy) -> Self {
        self.policy = policy;
        self
    }

    #[must_use]
    pub fn with_signing_requirements(mut self, signing: ReleaseRemoteSigningRequirements) -> Self {
        self.signing = signing;
        self
    }

    #[must_use]
    pub fn with_awfr_finalization(mut self, finalization: ReleaseRemoteAwfrFinalization) -> Self {
        self.awfr_finalization = finalization;
        self
    }

    #[must_use]
    pub fn with_credentials(mut self, credentials: ReleaseRemoteCredentials) -> Self {
        self.credentials = credentials;
        self
    }

    #[must_use]
    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }
}

impl ReleaseRemoteBackendError {
    pub fn retryable(message: impl Into<String>) -> Self {
        Self {
            kind: ReleaseRemoteBackendErrorKind::Retryable,
            message: message.into(),
        }
    }

    pub fn non_retryable(message: impl Into<String>) -> Self {
        Self {
            kind: ReleaseRemoteBackendErrorKind::NonRetryable,
            message: message.into(),
        }
    }
}

/// Builds a stable dry-run report without writing any remote object.
pub fn dry_run_release_remote_publication(
    plan: &ReleaseRemotePublishPlan,
) -> Result<ReleaseRemotePublishReport, ReleaseRemotePublishFailure> {
    let report = report_for_plan(plan, ReleaseRemotePublishMode::DryRun, None);
    validate_plan(plan).map_err(|error| failure_with_report(report.clone(), error))?;
    Ok(ReleaseRemotePublishReport {
        success: true,
        events: report
            .artifacts
            .iter()
            .map(|artifact| ReleaseRemotePublishEvent {
                operation: ReleaseRemotePublishOperation::CheckExists,
                kind: artifact.kind,
                destination_key: artifact.destination_key.clone(),
                staging_key: None,
                state: ReleaseRemoteArtifactState::Planned,
                message: Some("dry-run only; remote backend not touched".to_owned()),
            })
            .collect(),
        ..report
    })
}

/// Publishes all artifacts through a remote backend and returns a recoverable
/// report on every failure after validation starts.
pub fn publish_release_to_remote<B: ReleaseRemotePublicationBackend>(
    backend: &mut B,
    plan: &ReleaseRemotePublishPlan,
) -> Result<ReleaseRemotePublishReport, ReleaseRemotePublishFailure> {
    let staging_prefix = staging_prefix(plan).map_err(|error| {
        failure_with_report(
            report_for_plan(plan, ReleaseRemotePublishMode::Commit, None),
            error,
        )
    })?;
    let mut report = report_for_plan(
        plan,
        ReleaseRemotePublishMode::Commit,
        Some(staging_prefix.as_str().to_owned()),
    );
    validate_plan(plan).map_err(|error| failure_with_report(report.clone(), error))?;
    backend.backend_id().clone_into(&mut report.backend);
    let deadline = plan.policy.deadline();
    let mut staged = Vec::new();
    let mut visible = Vec::new();

    if let Err(error) = stage_remote_artifacts(
        backend,
        plan,
        &mut report,
        deadline,
        &staging_prefix,
        &mut staged,
    ) {
        recover_after_failure(backend, plan, &mut report, deadline, &visible, &staged);
        return Err(failure_with_report(report, error));
    }
    if let Err(error) = commit_remote_artifacts(
        backend,
        plan,
        &mut report,
        deadline,
        &staging_prefix,
        &mut visible,
    ) {
        recover_after_failure(backend, plan, &mut report, deadline, &visible, &staged);
        return Err(failure_with_report(report, error));
    }
    cleanup_staged_objects(backend, plan, &mut report, deadline, &staged);

    report.success = true;
    Ok(report)
}

fn stage_remote_artifacts<B: ReleaseRemotePublicationBackend>(
    backend: &mut B,
    plan: &ReleaseRemotePublishPlan,
    report: &mut ReleaseRemotePublishReport,
    deadline: Option<Instant>,
    staging_prefix: &ReleaseRemoteObjectKey,
    staged: &mut Vec<StagedRemoteObject>,
) -> Result<(), ReleaseRemotePublishErrorKind> {
    for artifact in &plan.artifacts {
        let destination_key = plan.target.destination_key(&artifact.object_key);
        let staging_key = staging_prefix.join(&destination_key);
        retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::PutStaged,
            &staging_key,
            || backend.put_object(&staging_key, &artifact.bytes),
        )?;
        staged.push(StagedRemoteObject {
            destination_key: destination_key.clone(),
            staging_key: staging_key.clone(),
        });
        set_artifact_state(report, &destination_key, ReleaseRemoteArtifactState::Staged);
        push_event(
            report,
            artifact.kind,
            &destination_key,
            Some(&staging_key),
            ReleaseRemotePublishOperation::PutStaged,
            ReleaseRemoteArtifactState::Staged,
            None,
        );
        let staged_bytes = retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::ReadStaged,
            &staging_key,
            || backend.read_object(&staging_key),
        )?;
        verify_remote_bytes(&staging_key, &artifact.bytes, &staged_bytes)?;
    }
    Ok(())
}

fn commit_remote_artifacts<B: ReleaseRemotePublicationBackend>(
    backend: &mut B,
    plan: &ReleaseRemotePublishPlan,
    report: &mut ReleaseRemotePublishReport,
    deadline: Option<Instant>,
    staging_prefix: &ReleaseRemoteObjectKey,
    visible: &mut Vec<VisibleRemoteObject>,
) -> Result<(), ReleaseRemotePublishErrorKind> {
    for artifact in ordered_artifacts(plan) {
        let destination_key = ReleaseRemoteObjectKey::new(artifact.destination_key.clone())?;
        let staging_key = staging_prefix.join(&destination_key);
        let exists = retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::CheckExists,
            &destination_key,
            || backend.object_exists(&destination_key),
        )?;
        if exists {
            return Err(ReleaseRemotePublishErrorKind::DestinationExists {
                key: destination_key.to_string(),
            });
        }
        retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::UploadCommitted,
            &destination_key,
            || backend.copy_object(&staging_key, &destination_key),
        )?;
        visible.push(VisibleRemoteObject {
            destination_key: destination_key.clone(),
        });
        set_artifact_state(
            report,
            &destination_key,
            ReleaseRemoteArtifactState::Uploaded,
        );
        push_event(
            report,
            artifact.kind,
            &destination_key,
            Some(&staging_key),
            ReleaseRemotePublishOperation::UploadCommitted,
            ReleaseRemoteArtifactState::Uploaded,
            None,
        );
        let committed_bytes = retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::ReadCommitted,
            &destination_key,
            || backend.read_object(&destination_key),
        )?;
        verify_digest_and_size(
            &destination_key,
            &artifact.digest,
            artifact.byte_len,
            &committed_bytes,
        )?;
        set_artifact_state(
            report,
            &destination_key,
            ReleaseRemoteArtifactState::Committed,
        );
        push_event(
            report,
            artifact.kind,
            &destination_key,
            Some(&staging_key),
            ReleaseRemotePublishOperation::ReadCommitted,
            ReleaseRemoteArtifactState::Committed,
            Some("uploaded bytes verified by digest and size".to_owned()),
        );
    }
    Ok(())
}

fn cleanup_staged_objects<B: ReleaseRemotePublicationBackend>(
    backend: &mut B,
    plan: &ReleaseRemotePublishPlan,
    report: &mut ReleaseRemotePublishReport,
    deadline: Option<Instant>,
    staged: &[StagedRemoteObject],
) {
    for staged_object in staged.iter().rev() {
        let _ = retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::DeleteStaged,
            &staged_object.staging_key,
            || backend.delete_object(&staged_object.staging_key),
        );
    }
}

fn report_for_plan(
    plan: &ReleaseRemotePublishPlan,
    mode: ReleaseRemotePublishMode,
    staging_prefix: Option<String>,
) -> ReleaseRemotePublishReport {
    let artifacts = ordered_artifacts(plan)
        .into_iter()
        .map(|mut artifact| {
            artifact.staging_key = staging_prefix
                .as_ref()
                .map(|prefix| format!("{prefix}/{}", artifact.destination_key));
            artifact
        })
        .collect::<Vec<_>>();
    ReleaseRemotePublishReport {
        mode,
        backend: "unbound".to_owned(),
        remote_prefix: plan.target.prefix.as_ref().map(ToString::to_string),
        staging_prefix,
        byte_budget: plan.policy.byte_budget,
        total_byte_len: artifacts.iter().map(|artifact| artifact.byte_len).sum(),
        signing: plan.signing,
        awfr_finalization: plan.awfr_finalization,
        credential: plan.credentials.report(),
        success: false,
        error: None,
        artifacts,
        events: Vec::new(),
    }
}

fn ordered_artifacts(plan: &ReleaseRemotePublishPlan) -> Vec<ReleaseRemotePublishedArtifact> {
    let mut artifacts = plan
        .artifacts
        .iter()
        .map(|artifact| {
            let destination_key = plan.target.destination_key(&artifact.object_key);
            ReleaseRemotePublishedArtifact {
                kind: artifact.kind,
                object_key: artifact.object_key.to_string(),
                destination_key: destination_key.to_string(),
                staging_key: None,
                digest: BundleDigest::of(&artifact.bytes).to_string(),
                byte_len: u64::try_from(artifact.bytes.len()).unwrap_or(u64::MAX),
                commit_rank: artifact.kind.commit_rank(),
                commit_order: 0,
                state: ReleaseRemoteArtifactState::Planned,
                requires_signature: artifact.requires_signature,
                final_awfr: artifact.kind.is_final_awfr(),
                external_payload_mirrors: artifact.external_payload_mirrors.clone(),
            }
        })
        .collect::<Vec<_>>();
    artifacts.sort_by(|left, right| {
        left.commit_rank
            .cmp(&right.commit_rank)
            .then_with(|| left.destination_key.cmp(&right.destination_key))
    });
    for (index, artifact) in artifacts.iter_mut().enumerate() {
        artifact.commit_order = index;
    }
    artifacts
}

fn validate_plan(plan: &ReleaseRemotePublishPlan) -> Result<(), ReleaseRemotePublishErrorKind> {
    plan.policy.validate()?;
    if plan.artifacts.is_empty() {
        return Err(ReleaseRemotePublishErrorKind::EmptyPlan);
    }
    let mut seen = BTreeSet::new();
    let mut total_byte_len = 0_u64;
    let mut signature_count = 0_usize;
    let mut awfr_count = 0_usize;
    for artifact in &plan.artifacts {
        let destination_key = plan
            .target
            .destination_key(&artifact.object_key)
            .to_string();
        if !seen.insert(destination_key.clone()) {
            return Err(ReleaseRemotePublishErrorKind::DuplicateObjectKey {
                key: destination_key,
            });
        }
        let byte_len = u64::try_from(artifact.bytes.len()).unwrap_or(u64::MAX);
        total_byte_len = total_byte_len.saturating_add(byte_len);
        if artifact.kind.is_signature() {
            signature_count += 1;
        }
        if artifact.kind.is_final_awfr() {
            awfr_count += 1;
        }
    }
    if total_byte_len > plan.policy.byte_budget {
        return Err(ReleaseRemotePublishErrorKind::ByteBudgetExceeded {
            total_byte_len,
            byte_budget: plan.policy.byte_budget,
        });
    }
    if plan.signing.require_signature_artifact && signature_count == 0 {
        return Err(ReleaseRemotePublishErrorKind::MissingSignatureArtifact);
    }
    if plan.awfr_finalization.require_exactly_one_awfr_archive && awfr_count != 1 {
        return Err(ReleaseRemotePublishErrorKind::MissingFinalAwfrArchive { count: awfr_count });
    }
    Ok(())
}

fn retry_backend_operation<T>(
    report: &mut ReleaseRemotePublishReport,
    plan: &ReleaseRemotePublishPlan,
    deadline: Option<Instant>,
    operation: ReleaseRemotePublishOperation,
    key: &ReleaseRemoteObjectKey,
    mut action: impl FnMut() -> Result<T, ReleaseRemoteBackendError>,
) -> Result<T, ReleaseRemotePublishErrorKind> {
    let mut last_error = None;
    for attempt in 1..=plan.policy.max_attempts {
        if deadline.is_some_and(|deadline| Instant::now() > deadline) {
            return Err(ReleaseRemotePublishErrorKind::Timeout {
                operation,
                key: key.to_string(),
            });
        }
        match action() {
            Ok(value) => return Ok(value),
            Err(error) => {
                let retryable = error.kind == ReleaseRemoteBackendErrorKind::Retryable;
                let redacted = plan.credentials.redact(error.message);
                last_error = Some(ReleaseRemotePublishErrorKind::Backend {
                    operation,
                    key: key.to_string(),
                    retryable,
                    message: redacted.clone(),
                });
                if !retryable || attempt == plan.policy.max_attempts {
                    break;
                }
                push_backend_retry_event(report, operation, key, attempt, &redacted);
            }
        }
    }
    Err(
        last_error.unwrap_or(ReleaseRemotePublishErrorKind::Backend {
            operation,
            key: key.to_string(),
            retryable: false,
            message: "backend operation did not run".to_owned(),
        }),
    )
}

fn verify_remote_bytes(
    key: &ReleaseRemoteObjectKey,
    expected: &[u8],
    actual: &[u8],
) -> Result<(), ReleaseRemotePublishErrorKind> {
    verify_digest_and_size(
        key,
        &BundleDigest::of(expected).to_string(),
        u64::try_from(expected.len()).unwrap_or(u64::MAX),
        actual,
    )
}

fn verify_digest_and_size(
    key: &ReleaseRemoteObjectKey,
    expected_digest: &str,
    expected_byte_len: u64,
    actual: &[u8],
) -> Result<(), ReleaseRemotePublishErrorKind> {
    let actual_byte_len = u64::try_from(actual.len()).unwrap_or(u64::MAX);
    let actual_digest = BundleDigest::of(actual).to_string();
    if actual_byte_len == expected_byte_len && actual_digest == expected_digest {
        return Ok(());
    }
    Err(ReleaseRemotePublishErrorKind::ChecksumMismatch {
        key: key.to_string(),
        expected_digest: expected_digest.to_owned(),
        actual_digest,
        expected_byte_len,
        actual_byte_len,
    })
}

fn recover_after_failure<B: ReleaseRemotePublicationBackend>(
    backend: &mut B,
    plan: &ReleaseRemotePublishPlan,
    report: &mut ReleaseRemotePublishReport,
    deadline: Option<Instant>,
    visible: &[VisibleRemoteObject],
    staged: &[StagedRemoteObject],
) {
    for object in visible.iter().rev() {
        let state = match retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::DeleteCommitted,
            &object.destination_key,
            || backend.delete_object(&object.destination_key),
        ) {
            Ok(()) => ReleaseRemoteArtifactState::RolledBack,
            Err(error) => {
                report.error = Some(error);
                ReleaseRemoteArtifactState::Abandoned
            }
        };
        set_artifact_state(report, &object.destination_key, state);
        let kind = artifact_kind_for_key(report, &object.destination_key);
        push_event(
            report,
            kind,
            &object.destination_key,
            None,
            ReleaseRemotePublishOperation::DeleteCommitted,
            state,
            None,
        );
    }
    for object in staged.iter().rev() {
        let state = match retry_backend_operation(
            report,
            plan,
            deadline,
            ReleaseRemotePublishOperation::DeleteStaged,
            &object.staging_key,
            || backend.delete_object(&object.staging_key),
        ) {
            Ok(()) => ReleaseRemoteArtifactState::RolledBack,
            Err(error) => {
                report.error = Some(error);
                ReleaseRemoteArtifactState::Abandoned
            }
        };
        if artifact_state_for_key(report, &object.destination_key)
            != ReleaseRemoteArtifactState::Abandoned
        {
            set_artifact_state(report, &object.destination_key, state);
        }
        let kind = artifact_kind_for_key(report, &object.destination_key);
        push_event(
            report,
            kind,
            &object.destination_key,
            Some(&object.staging_key),
            ReleaseRemotePublishOperation::DeleteStaged,
            state,
            None,
        );
    }
}

fn failure_with_report(
    mut report: ReleaseRemotePublishReport,
    error: ReleaseRemotePublishErrorKind,
) -> ReleaseRemotePublishFailure {
    report.success = false;
    report.error = Some(error.clone());
    ReleaseRemotePublishFailure {
        error,
        report: Box::new(report),
    }
}

fn set_artifact_state(
    report: &mut ReleaseRemotePublishReport,
    key: &ReleaseRemoteObjectKey,
    state: ReleaseRemoteArtifactState,
) {
    if let Some(artifact) = report
        .artifacts
        .iter_mut()
        .find(|artifact| artifact.destination_key == key.as_str())
    {
        artifact.state = state;
    }
}

fn artifact_state_for_key(
    report: &ReleaseRemotePublishReport,
    key: &ReleaseRemoteObjectKey,
) -> ReleaseRemoteArtifactState {
    report
        .artifacts
        .iter()
        .find(|artifact| artifact.destination_key == key.as_str())
        .map_or(ReleaseRemoteArtifactState::Planned, |artifact| {
            artifact.state
        })
}

fn artifact_kind_for_key(
    report: &ReleaseRemotePublishReport,
    key: &ReleaseRemoteObjectKey,
) -> ReleasePublishArtifactKind {
    report
        .artifacts
        .iter()
        .find(|artifact| artifact.destination_key == key.as_str())
        .map_or(ReleasePublishArtifactKind::AwfrArchive, |artifact| {
            artifact.kind
        })
}

fn push_event(
    report: &mut ReleaseRemotePublishReport,
    kind: ReleasePublishArtifactKind,
    destination_key: &ReleaseRemoteObjectKey,
    staging_key: Option<&ReleaseRemoteObjectKey>,
    operation: ReleaseRemotePublishOperation,
    state: ReleaseRemoteArtifactState,
    message: Option<String>,
) {
    report.events.push(ReleaseRemotePublishEvent {
        operation,
        kind,
        destination_key: destination_key.to_string(),
        staging_key: staging_key.map(ToString::to_string),
        state,
        message,
    });
}

fn push_backend_retry_event(
    report: &mut ReleaseRemotePublishReport,
    operation: ReleaseRemotePublishOperation,
    key: &ReleaseRemoteObjectKey,
    attempt: u8,
    message: &str,
) {
    let kind = artifact_kind_for_key(report, key);
    report.events.push(ReleaseRemotePublishEvent {
        operation,
        kind,
        destination_key: key.to_string(),
        staging_key: None,
        state: ReleaseRemoteArtifactState::Planned,
        message: Some(format!("retrying after attempt {attempt}: {message}")),
    });
}

fn staging_prefix(
    plan: &ReleaseRemotePublishPlan,
) -> Result<ReleaseRemoteObjectKey, ReleaseRemotePublishErrorKind> {
    let run_id = plan.run_id.clone().unwrap_or_else(|| {
        format!(
            "publish-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_or(0, |duration| duration.as_nanos())
        )
    });
    ReleaseRemoteObjectKey::new(format!(".arcweft-remote-staging/{run_id}"))
}

#[cfg(test)]
mod tests;
