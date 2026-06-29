use super::{
    ReleaseObjectDirectoryBackend, ReleaseRemoteArtifactState, ReleaseRemoteBackendError,
    ReleaseRemoteCredentials, ReleaseRemoteObjectKey, ReleaseRemotePublicationBackend,
    ReleaseRemotePublishArtifact, ReleaseRemotePublishErrorKind, ReleaseRemotePublishMode,
    ReleaseRemotePublishOperation, ReleaseRemotePublishPlan, ReleaseRemotePublishPolicy,
    ReleaseRemotePublishTarget, ReleaseRemoteSigningRequirements,
    dry_run_release_remote_publication, publish_release_to_remote,
};
use crate::release_adapter::publish::ReleasePublishArtifactKind;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

#[test]
fn dry_run_plan_is_stable_and_does_not_touch_backend() {
    let plan = small_plan().with_run_id("dry-run-stable");
    let first = dry_run_release_remote_publication(&plan).expect("first dry run");
    let second = dry_run_release_remote_publication(&plan).expect("second dry run");

    assert_eq!(first, second);
    assert!(first.success);
    assert_eq!(first.mode, ReleaseRemotePublishMode::DryRun);
    assert!(first.staging_prefix.is_none());
    assert_eq!(
        first.artifacts.last().expect("final artifact").kind,
        ReleasePublishArtifactKind::AwfrArchive
    );
}

#[test]
fn object_directory_publish_commits_verified_bytes() {
    let root = temp_root("remote-object-directory");
    let plan = small_plan().with_run_id("object-directory-success");
    let mut backend = ReleaseObjectDirectoryBackend::new(&root);

    let report = publish_release_to_remote(&mut backend, &plan).expect("remote publish succeeds");

    assert!(report.success);
    assert_eq!(
        fs::read(root.join("game.awfb")).expect("awfb reads"),
        b"awfb"
    );
    assert_eq!(
        fs::read(root.join("game.awfr")).expect("awfr reads"),
        b"awfr"
    );
    assert!(
        report
            .artifacts
            .iter()
            .all(|artifact| artifact.state == ReleaseRemoteArtifactState::Committed)
    );
    let _ = fs::remove_dir_all(root);
}

#[test]
fn final_awfr_is_committed_last() {
    let plan = small_plan().with_run_id("awfr-last");
    let mut backend = MemoryBackend::default();
    let report = publish_release_to_remote(&mut backend, &plan).expect("publish succeeds");

    let committed = report
        .events
        .iter()
        .filter(|event| event.state == ReleaseRemoteArtifactState::Committed)
        .collect::<Vec<_>>();

    assert_eq!(
        committed.last().expect("last commit").kind,
        ReleasePublishArtifactKind::AwfrArchive
    );
}

#[test]
fn checksum_mismatch_after_upload_returns_recoverable_report() {
    let plan = small_plan().with_run_id("checksum-mismatch");
    let mut backend = MemoryBackend::default().corrupt_reads_for("game.awfb");

    let failure = publish_release_to_remote(&mut backend, &plan).expect_err("checksum fails");

    assert!(matches!(
        failure.error,
        ReleaseRemotePublishErrorKind::ChecksumMismatch { .. }
    ));
    assert!(
        failure
            .report
            .artifacts
            .iter()
            .any(|artifact| artifact.state == ReleaseRemoteArtifactState::RolledBack)
    );
}

#[test]
fn retryable_stage_failure_is_retried() {
    let plan = small_plan()
        .with_policy(ReleaseRemotePublishPolicy::new(2, u64::MAX, None).expect("policy"))
        .with_run_id("retry-stage");
    let mut backend = MemoryBackend::default().fail_once(
        ReleaseRemotePublishOperation::PutStaged,
        ".arcweft-remote-staging/retry-stage/game.awfb",
        ReleaseRemoteBackendError::retryable("temporary object directory outage"),
    );

    let report = publish_release_to_remote(&mut backend, &plan).expect("retry succeeds");

    assert!(report.success);
    assert!(report.events.iter().any(|event| {
        event
            .message
            .as_deref()
            .is_some_and(|message| message.contains("retrying after attempt"))
    }));
}

#[test]
fn non_retryable_commit_failure_rolls_back_and_reports_abandoned_cleanup() {
    let plan = small_plan().with_run_id("commit-failure");
    let mut backend = MemoryBackend::default()
        .fail_always(
            ReleaseRemotePublishOperation::UploadCommitted,
            "game.awfr",
            ReleaseRemoteBackendError::non_retryable("permission denied"),
        )
        .fail_always(
            ReleaseRemotePublishOperation::DeleteStaged,
            ".arcweft-remote-staging/commit-failure/game.awfr",
            ReleaseRemoteBackendError::non_retryable("manual cleanup required"),
        );

    let failure = publish_release_to_remote(&mut backend, &plan).expect_err("commit fails");

    assert!(matches!(
        failure.error,
        ReleaseRemotePublishErrorKind::Backend { .. }
    ));
    assert!(
        failure
            .report
            .artifacts
            .iter()
            .any(|artifact| artifact.state == ReleaseRemoteArtifactState::Abandoned)
    );
    assert!(
        failure
            .report
            .events
            .iter()
            .any(|event| event.state == ReleaseRemoteArtifactState::RolledBack)
    );
}

#[test]
fn credentials_are_redacted_from_report_and_debug() {
    let secret = "super-secret-token";
    let plan = small_plan()
        .with_credentials(ReleaseRemoteCredentials::new(
            Some("ci-profile".to_owned()),
            Some(secret.to_owned()),
        ))
        .with_run_id("redaction");
    let mut backend = MemoryBackend::default().fail_always(
        ReleaseRemotePublishOperation::PutStaged,
        ".arcweft-remote-staging/redaction/game.awfb",
        ReleaseRemoteBackendError::non_retryable(format!("auth failed for {secret}")),
    );

    let failure = publish_release_to_remote(&mut backend, &plan).expect_err("auth fails");
    let json = serde_json::to_string(&failure.report).expect("report serializes");
    let debug = format!("{:?}", plan.credentials);

    assert!(!json.contains(secret));
    assert!(!debug.contains(secret));
    assert!(json.contains("<redacted>"));
}

fn small_plan() -> ReleaseRemotePublishPlan {
    let artifacts = vec![
        artifact(
            ReleasePublishArtifactKind::AwfrArchive,
            "game.awfr",
            b"awfr",
        ),
        artifact(ReleasePublishArtifactKind::AwfbBundle, "game.awfb", b"awfb"),
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
    ];
    ReleaseRemotePublishPlan::new(
        ReleaseRemotePublishTarget::new(None).expect("target"),
        artifacts,
    )
    .with_signing_requirements(ReleaseRemoteSigningRequirements {
        require_signature_artifact: true,
        require_awfr_signature_reference: false,
    })
}

fn artifact(
    kind: ReleasePublishArtifactKind,
    key: &str,
    bytes: &[u8],
) -> ReleaseRemotePublishArtifact {
    ReleaseRemotePublishArtifact::new(
        kind,
        ReleaseRemoteObjectKey::new(key).expect("key"),
        bytes.to_vec(),
    )
}

fn temp_root(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "arcweft-remote-publish-{label}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ))
}

#[derive(Default)]
struct MemoryBackend {
    objects: BTreeMap<String, Vec<u8>>,
    failures:
        BTreeMap<(ReleaseRemotePublishOperation, String), VecDeque<ReleaseRemoteBackendError>>,
    corrupt_read_keys: BTreeSet<String>,
}

impl MemoryBackend {
    fn fail_once(
        mut self,
        operation: ReleaseRemotePublishOperation,
        key: &str,
        error: ReleaseRemoteBackendError,
    ) -> Self {
        self.failures
            .entry((operation, key.to_owned()))
            .or_default()
            .push_back(error);
        self
    }

    fn fail_always(
        mut self,
        operation: ReleaseRemotePublishOperation,
        key: &str,
        error: ReleaseRemoteBackendError,
    ) -> Self {
        self.failures
            .entry((operation, key.to_owned()))
            .or_default()
            .extend([error.clone(), error.clone(), error]);
        self
    }

    fn corrupt_reads_for(mut self, key: &str) -> Self {
        self.corrupt_read_keys.insert(key.to_owned());
        self
    }

    fn fail_if_scripted(
        &mut self,
        operation: ReleaseRemotePublishOperation,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError> {
        let Some(errors) = self.failures.get_mut(&(operation, key.to_string())) else {
            return Ok(());
        };
        errors.pop_front().map_or(Ok(()), Err)
    }
}

impl ReleaseRemotePublicationBackend for MemoryBackend {
    fn backend_id(&self) -> &'static str {
        "memory"
    }

    fn put_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
        bytes: &[u8],
    ) -> Result<(), ReleaseRemoteBackendError> {
        self.fail_if_scripted(ReleaseRemotePublishOperation::PutStaged, key)?;
        self.objects.insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn read_object(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<Vec<u8>, ReleaseRemoteBackendError> {
        let mut bytes = self.objects.get(key.as_str()).cloned().ok_or_else(|| {
            ReleaseRemoteBackendError::non_retryable(format!("missing object {key}"))
        })?;
        if self.corrupt_read_keys.contains(key.as_str()) {
            bytes.push(b'!');
        }
        Ok(bytes)
    }

    fn copy_object(
        &mut self,
        from: &ReleaseRemoteObjectKey,
        to: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError> {
        self.fail_if_scripted(ReleaseRemotePublishOperation::UploadCommitted, to)?;
        if self.objects.contains_key(to.as_str()) {
            return Err(ReleaseRemoteBackendError::non_retryable(
                "destination exists",
            ));
        }
        let bytes = self.objects.get(from.as_str()).cloned().ok_or_else(|| {
            ReleaseRemoteBackendError::non_retryable(format!("missing source {from}"))
        })?;
        self.objects.insert(to.to_string(), bytes);
        Ok(())
    }

    fn delete_object(
        &mut self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<(), ReleaseRemoteBackendError> {
        self.fail_if_scripted(ReleaseRemotePublishOperation::DeleteCommitted, key)?;
        self.fail_if_scripted(ReleaseRemotePublishOperation::DeleteStaged, key)?;
        self.objects.remove(key.as_str());
        Ok(())
    }

    fn object_exists(
        &self,
        key: &ReleaseRemoteObjectKey,
    ) -> Result<bool, ReleaseRemoteBackendError> {
        Ok(self.objects.contains_key(key.as_str()))
    }
}
