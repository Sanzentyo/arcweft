//! Typed signing policy and deterministic digest transcripts for release flows.
//!
//! This module keeps signing decisions and transcript construction in the
//! Sans I/O bundle crate while leaving clocks, key stores, platform trust roots,
//! and actual signature creation/verification adapters outside this crate.

use super::archive::{AwfrArchiveManifest, ReleaseChannel};
use crate::container::{ArtifactIdentity, BundleDigest, BundleKind};
use std::collections::BTreeSet;
use thiserror::Error;

pub const SIGNING_POLICY_SCHEMA_VERSION: u32 = 1;
pub const SIGNING_TRANSCRIPT_SCHEMA_VERSION: u32 = 1;

/// Product signing policy mode selected by CLI, CI, loader, player, or tests.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SigningPolicyMode {
    LocalDev,
    Ci,
    ReleasePublish,
    ReleaseConsume,
    OfflineInspection,
    TestFixture,
}

/// Signable subject families owned by the release trust model.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(rename_all = "snake_case")]
pub enum SigningSubjectKind {
    AwfbBundle,
    PatchV2Artifact,
    MaterializedTargetBundle,
    AwfrReleaseArchive,
    ExternalPayload,
}

/// Key epoch acceptance window. `max` is exclusive when present.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct KeyEpochPolicy {
    #[serde(default)]
    pub min: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
}

/// Typed release signing policy. The policy says which subject families require
/// signatures and which inspection shortcuts are explicitly allowed.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SigningPolicy {
    pub schema_version: u32,
    pub mode: SigningPolicyMode,
    pub channel: ReleaseChannel,
    pub key_epoch: KeyEpochPolicy,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub required_subjects: BTreeSet<SigningSubjectKind>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_unsigned_local_artifacts: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub allow_metadata_only_external_payloads: bool,
}

/// What happens to an existing/base signature when materializing a target.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignatureDisposition {
    PreservedUnchangedTarget,
    PreservedForBaseProvenanceOnly,
    Stripped,
    InvalidatedAndRequiresAdapterSignature,
    InvalidatedAndUnsignedAllowedByPolicy,
}

/// User-visible inspection state consumed by CLI/project-loader/player layers.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SigningInspectionState {
    Valid,
    UnsignedAllowed,
    UnsignedRejected,
    Invalid,
    Expired,
    WrongKey,
    WrongEpoch,
    WrongChannel,
    MetadataOnlyExternalPayloads,
    MissingKeys,
    MissingExternalPayloadBytes,
}

/// Compact result that adapters can report without duplicating policy logic.
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SigningInspectionResult {
    pub subject: SigningSubjectKind,
    pub state: SigningInspectionState,
    pub policy_mode: SigningPolicyMode,
}

/// Deterministic transcript input for release signatures.
#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
pub struct SigningDigestTranscript {
    pub schema_version: u32,
    pub subject: SigningSubjectKind,
    pub channel: ReleaseChannel,
    pub signer_id: String,
    pub key_epoch: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle_kind: Option<BundleKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_identity: Option<ArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_artifact_identity: Option<ArtifactIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_root: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_content_root: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub manifest_digest: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub whole_file_digest: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_identity_digest: Option<BundleDigest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_payloads_digest: Option<BundleDigest>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum SigningPolicyError {
    #[error("unsupported signing policy schema version {actual}; expected {expected}")]
    UnsupportedPolicySchema { actual: u32, expected: u32 },
    #[error("unsupported signing transcript schema version {actual}; expected {expected}")]
    UnsupportedTranscriptSchema { actual: u32, expected: u32 },
    #[error("invalid signing policy: {0}")]
    InvalidPolicy(String),
    #[error("invalid signing transcript: {0}")]
    InvalidTranscript(String),
    #[error("failed to build AWFR signing transcript: {0}")]
    Archive(String),
}

impl KeyEpochPolicy {
    pub const fn contains(self, epoch: u64) -> bool {
        epoch >= self.min
            && match self.max {
                Some(max) => epoch < max,
                None => true,
            }
    }
}

impl SigningPolicy {
    pub fn local_dev(channel: ReleaseChannel) -> Self {
        Self {
            schema_version: SIGNING_POLICY_SCHEMA_VERSION,
            mode: SigningPolicyMode::LocalDev,
            channel,
            key_epoch: KeyEpochPolicy::default(),
            required_subjects: BTreeSet::new(),
            allow_unsigned_local_artifacts: true,
            allow_metadata_only_external_payloads: true,
        }
    }

    pub fn ci(channel: ReleaseChannel, key_epoch: KeyEpochPolicy) -> Self {
        Self::strict(
            SigningPolicyMode::Ci,
            channel,
            key_epoch,
            [
                SigningSubjectKind::PatchV2Artifact,
                SigningSubjectKind::MaterializedTargetBundle,
            ],
        )
    }

    pub fn release_publish(channel: ReleaseChannel, key_epoch: KeyEpochPolicy) -> Self {
        Self::strict(
            SigningPolicyMode::ReleasePublish,
            channel,
            key_epoch,
            [
                SigningSubjectKind::AwfbBundle,
                SigningSubjectKind::PatchV2Artifact,
                SigningSubjectKind::MaterializedTargetBundle,
                SigningSubjectKind::AwfrReleaseArchive,
            ],
        )
    }

    pub fn release_consume(channel: ReleaseChannel, key_epoch: KeyEpochPolicy) -> Self {
        Self::strict(
            SigningPolicyMode::ReleaseConsume,
            channel,
            key_epoch,
            [
                SigningSubjectKind::AwfbBundle,
                SigningSubjectKind::PatchV2Artifact,
                SigningSubjectKind::MaterializedTargetBundle,
                SigningSubjectKind::AwfrReleaseArchive,
            ],
        )
    }

    pub fn offline_inspection(channel: ReleaseChannel) -> Self {
        Self {
            schema_version: SIGNING_POLICY_SCHEMA_VERSION,
            mode: SigningPolicyMode::OfflineInspection,
            channel,
            key_epoch: KeyEpochPolicy::default(),
            required_subjects: BTreeSet::new(),
            allow_unsigned_local_artifacts: false,
            allow_metadata_only_external_payloads: true,
        }
    }

    pub fn test_fixture(channel: ReleaseChannel) -> Self {
        Self {
            schema_version: SIGNING_POLICY_SCHEMA_VERSION,
            mode: SigningPolicyMode::TestFixture,
            channel,
            key_epoch: KeyEpochPolicy::default(),
            required_subjects: [SigningSubjectKind::AwfbBundle].into_iter().collect(),
            allow_unsigned_local_artifacts: true,
            allow_metadata_only_external_payloads: true,
        }
    }

    fn strict(
        mode: SigningPolicyMode,
        channel: ReleaseChannel,
        key_epoch: KeyEpochPolicy,
        required_subjects: impl IntoIterator<Item = SigningSubjectKind>,
    ) -> Self {
        Self {
            schema_version: SIGNING_POLICY_SCHEMA_VERSION,
            mode,
            channel,
            key_epoch,
            required_subjects: required_subjects.into_iter().collect(),
            allow_unsigned_local_artifacts: false,
            allow_metadata_only_external_payloads: false,
        }
    }

    pub fn validate(&self) -> Result<(), SigningPolicyError> {
        if self.schema_version != SIGNING_POLICY_SCHEMA_VERSION {
            return Err(SigningPolicyError::UnsupportedPolicySchema {
                actual: self.schema_version,
                expected: SIGNING_POLICY_SCHEMA_VERSION,
            });
        }
        self.channel
            .validate()
            .map_err(|error| SigningPolicyError::InvalidPolicy(error.to_string()))?;
        if let Some(max) = self.key_epoch.max
            && max <= self.key_epoch.min
        {
            return Err(SigningPolicyError::InvalidPolicy(
                "key epoch max must be greater than min".to_owned(),
            ));
        }
        if matches!(
            self.mode,
            SigningPolicyMode::ReleasePublish | SigningPolicyMode::ReleaseConsume
        ) && self.allow_unsigned_local_artifacts
        {
            return Err(SigningPolicyError::InvalidPolicy(
                "release policies must not allow unsigned local artifacts".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn requires_signature(&self, subject: SigningSubjectKind) -> bool {
        self.required_subjects.contains(&subject)
    }

    pub const fn allows_metadata_only_external_payloads(&self) -> bool {
        self.allow_metadata_only_external_payloads
    }

    pub fn materialized_target_signature_disposition(
        &self,
        target_bytes_changed: bool,
    ) -> SignatureDisposition {
        if !target_bytes_changed {
            return SignatureDisposition::PreservedUnchangedTarget;
        }
        if self.requires_signature(SigningSubjectKind::MaterializedTargetBundle) {
            SignatureDisposition::InvalidatedAndRequiresAdapterSignature
        } else if self.allow_unsigned_local_artifacts {
            SignatureDisposition::InvalidatedAndUnsignedAllowedByPolicy
        } else {
            SignatureDisposition::Stripped
        }
    }

    pub fn inspect_signature_presence(
        &self,
        subject: SigningSubjectKind,
        signature_present: bool,
        channel: &ReleaseChannel,
        key_epoch: u64,
    ) -> SigningInspectionResult {
        let state = if channel != &self.channel {
            SigningInspectionState::WrongChannel
        } else if !self.key_epoch.contains(key_epoch) {
            SigningInspectionState::WrongEpoch
        } else if signature_present {
            SigningInspectionState::Valid
        } else if self.requires_signature(subject) {
            if self.allow_unsigned_local_artifacts {
                SigningInspectionState::UnsignedAllowed
            } else {
                SigningInspectionState::UnsignedRejected
            }
        } else {
            SigningInspectionState::UnsignedAllowed
        };
        SigningInspectionResult {
            subject,
            state,
            policy_mode: self.mode,
        }
    }
}

impl SigningDigestTranscript {
    pub fn awfb_bundle(
        artifact_identity: ArtifactIdentity,
        whole_file_digest: BundleDigest,
        signer_id: impl Into<String>,
        channel: ReleaseChannel,
        key_epoch: u64,
    ) -> Result<Self, SigningPolicyError> {
        let transcript = Self {
            schema_version: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            subject: SigningSubjectKind::AwfbBundle,
            channel,
            signer_id: signer_id.into(),
            key_epoch,
            bundle_kind: Some(artifact_identity.kind),
            artifact_identity: Some(artifact_identity),
            target_artifact_identity: None,
            content_root: Some(artifact_identity.content_root),
            target_content_root: None,
            manifest_digest: Some(artifact_identity.manifest_digest),
            whole_file_digest: Some(whole_file_digest),
            archive_identity_digest: None,
            external_payloads_digest: None,
        };
        transcript.validate()?;
        Ok(transcript)
    }

    pub fn patch_v2_artifact(
        patch_artifact: ArtifactIdentity,
        target_artifact: ArtifactIdentity,
        whole_file_digest: BundleDigest,
        signer_id: impl Into<String>,
        channel: ReleaseChannel,
        key_epoch: u64,
    ) -> Result<Self, SigningPolicyError> {
        let transcript = Self {
            schema_version: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            subject: SigningSubjectKind::PatchV2Artifact,
            channel,
            signer_id: signer_id.into(),
            key_epoch,
            bundle_kind: Some(BundleKind::Patch),
            artifact_identity: Some(patch_artifact),
            target_artifact_identity: Some(target_artifact),
            content_root: Some(patch_artifact.content_root),
            target_content_root: Some(target_artifact.content_root),
            manifest_digest: Some(patch_artifact.manifest_digest),
            whole_file_digest: Some(whole_file_digest),
            archive_identity_digest: None,
            external_payloads_digest: None,
        };
        transcript.validate()?;
        Ok(transcript)
    }

    pub fn materialized_target_bundle(
        target_artifact: ArtifactIdentity,
        whole_file_digest: BundleDigest,
        signer_id: impl Into<String>,
        channel: ReleaseChannel,
        key_epoch: u64,
    ) -> Result<Self, SigningPolicyError> {
        let transcript = Self {
            schema_version: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            subject: SigningSubjectKind::MaterializedTargetBundle,
            channel,
            signer_id: signer_id.into(),
            key_epoch,
            bundle_kind: Some(target_artifact.kind),
            artifact_identity: Some(target_artifact),
            target_artifact_identity: Some(target_artifact),
            content_root: Some(target_artifact.content_root),
            target_content_root: Some(target_artifact.content_root),
            manifest_digest: Some(target_artifact.manifest_digest),
            whole_file_digest: Some(whole_file_digest),
            archive_identity_digest: None,
            external_payloads_digest: None,
        };
        transcript.validate()?;
        Ok(transcript)
    }

    pub fn awfr_release_archive(
        archive: &AwfrArchiveManifest,
        whole_file_digest: BundleDigest,
        signer_id: impl Into<String>,
        key_epoch: u64,
    ) -> Result<Self, SigningPolicyError> {
        let archive_identity_digest = archive
            .unsigned_identity_digest()
            .map_err(|error| SigningPolicyError::Archive(error.to_string()))?;
        let release_manifest_digest = BundleDigest::of(
            &archive
                .release_manifest
                .to_json_bytes()
                .map_err(|error| SigningPolicyError::Archive(error.to_string()))?,
        );
        let transcript = Self {
            schema_version: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            subject: SigningSubjectKind::AwfrReleaseArchive,
            channel: archive.channel.clone(),
            signer_id: signer_id.into(),
            key_epoch,
            bundle_kind: None,
            artifact_identity: None,
            target_artifact_identity: None,
            content_root: None,
            target_content_root: None,
            manifest_digest: Some(release_manifest_digest),
            whole_file_digest: Some(whole_file_digest),
            archive_identity_digest: Some(archive_identity_digest),
            external_payloads_digest: Some(archive.external_payloads_digest()),
        };
        transcript.validate()?;
        Ok(transcript)
    }

    pub fn external_payload(
        bundle_kind: BundleKind,
        bundle_content_root: BundleDigest,
        payload_digest: BundleDigest,
        signer_id: impl Into<String>,
        channel: ReleaseChannel,
        key_epoch: u64,
    ) -> Result<Self, SigningPolicyError> {
        let transcript = Self {
            schema_version: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            subject: SigningSubjectKind::ExternalPayload,
            channel,
            signer_id: signer_id.into(),
            key_epoch,
            bundle_kind: Some(bundle_kind),
            artifact_identity: None,
            target_artifact_identity: None,
            content_root: Some(bundle_content_root),
            target_content_root: None,
            manifest_digest: None,
            whole_file_digest: Some(payload_digest),
            archive_identity_digest: None,
            external_payloads_digest: None,
        };
        transcript.validate()?;
        Ok(transcript)
    }

    pub fn validate(&self) -> Result<(), SigningPolicyError> {
        if self.schema_version != SIGNING_TRANSCRIPT_SCHEMA_VERSION {
            return Err(SigningPolicyError::UnsupportedTranscriptSchema {
                actual: self.schema_version,
                expected: SIGNING_TRANSCRIPT_SCHEMA_VERSION,
            });
        }
        self.channel
            .validate()
            .map_err(|error| SigningPolicyError::InvalidTranscript(error.to_string()))?;
        if self.signer_id.is_empty() {
            return Err(SigningPolicyError::InvalidTranscript(
                "signer_id must not be empty".to_owned(),
            ));
        }
        match self.subject {
            SigningSubjectKind::AwfbBundle => {
                require("artifact_identity", self.artifact_identity.is_some())?;
                require("whole_file_digest", self.whole_file_digest.is_some())?;
            }
            SigningSubjectKind::PatchV2Artifact => {
                require("patch artifact_identity", self.artifact_identity.is_some())?;
                require(
                    "target_artifact_identity",
                    self.target_artifact_identity.is_some(),
                )?;
                require("target_content_root", self.target_content_root.is_some())?;
                require("whole_file_digest", self.whole_file_digest.is_some())?;
            }
            SigningSubjectKind::MaterializedTargetBundle => {
                require("target artifact_identity", self.artifact_identity.is_some())?;
                require("target_content_root", self.target_content_root.is_some())?;
                require("whole_file_digest", self.whole_file_digest.is_some())?;
            }
            SigningSubjectKind::AwfrReleaseArchive => {
                require(
                    "archive_identity_digest",
                    self.archive_identity_digest.is_some(),
                )?;
                require(
                    "external_payloads_digest",
                    self.external_payloads_digest.is_some(),
                )?;
                require("whole_file_digest", self.whole_file_digest.is_some())?;
            }
            SigningSubjectKind::ExternalPayload => {
                require("content_root", self.content_root.is_some())?;
                require("whole_file_digest", self.whole_file_digest.is_some())?;
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<BundleDigest, SigningPolicyError> {
        self.validate()?;
        let mut transcript = Vec::new();
        transcript.extend_from_slice(b"arcweft.signing-transcript.v1\0");
        transcript.extend_from_slice(&self.schema_version.to_le_bytes());
        put_string(&mut transcript, self.subject.as_str());
        put_string(&mut transcript, self.channel.as_str());
        put_string(&mut transcript, &self.signer_id);
        transcript.extend_from_slice(&self.key_epoch.to_le_bytes());
        put_optional_bundle_kind(&mut transcript, self.bundle_kind);
        put_optional_artifact_identity(&mut transcript, self.artifact_identity);
        put_optional_artifact_identity(&mut transcript, self.target_artifact_identity);
        put_optional_digest(&mut transcript, self.content_root);
        put_optional_digest(&mut transcript, self.target_content_root);
        put_optional_digest(&mut transcript, self.manifest_digest);
        put_optional_digest(&mut transcript, self.whole_file_digest);
        put_optional_digest(&mut transcript, self.archive_identity_digest);
        put_optional_digest(&mut transcript, self.external_payloads_digest);
        Ok(BundleDigest::of(&transcript))
    }
}

impl SigningSubjectKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AwfbBundle => "awfb_bundle",
            Self::PatchV2Artifact => "patch_v2_artifact",
            Self::MaterializedTargetBundle => "materialized_target_bundle",
            Self::AwfrReleaseArchive => "awfr_release_archive",
            Self::ExternalPayload => "external_payload",
        }
    }
}

fn require(label: &str, condition: bool) -> Result<(), SigningPolicyError> {
    if condition {
        Ok(())
    } else {
        Err(SigningPolicyError::InvalidTranscript(format!(
            "{label} is required"
        )))
    }
}

fn put_string(out: &mut Vec<u8>, value: &str) {
    let len = u32::try_from(value.len()).unwrap_or(u32::MAX);
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}

fn put_optional_digest(out: &mut Vec<u8>, value: Option<BundleDigest>) {
    if let Some(value) = value {
        out.push(1);
        out.extend_from_slice(&value.as_bytes());
    } else {
        out.push(0);
    }
}

fn put_optional_bundle_kind(out: &mut Vec<u8>, value: Option<BundleKind>) {
    if let Some(value) = value {
        out.push(1);
        out.extend_from_slice(&value.encoded().to_le_bytes());
    } else {
        out.push(0);
    }
}

fn put_optional_artifact_identity(out: &mut Vec<u8>, value: Option<ArtifactIdentity>) {
    if let Some(value) = value {
        out.push(1);
        out.extend_from_slice(&value.container_version.to_le_bytes());
        out.extend_from_slice(&value.kind.encoded().to_le_bytes());
        out.extend_from_slice(&value.content_root.as_bytes());
        out.extend_from_slice(&value.manifest_digest.as_bytes());
        out.extend_from_slice(&value.digest().as_bytes());
    } else {
        out.push(0);
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
const fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(kind: BundleKind, label: &'static [u8]) -> ArtifactIdentity {
        ArtifactIdentity::for_current_container(
            kind,
            BundleDigest::of(label),
            BundleDigest::of(&[label, b"-manifest"].concat()),
        )
    }

    #[test]
    fn release_policy_requires_target_signatures_and_rejects_unsigned() {
        let policy = SigningPolicy::release_consume(
            ReleaseChannel::new("stable").expect("channel"),
            KeyEpochPolicy {
                min: 2,
                max: Some(8),
            },
        );
        policy.validate().expect("policy validates");

        let result = policy.inspect_signature_presence(
            SigningSubjectKind::MaterializedTargetBundle,
            false,
            &ReleaseChannel::new("stable").expect("channel"),
            4,
        );

        assert_eq!(result.state, SigningInspectionState::UnsignedRejected);
        assert_eq!(
            policy.materialized_target_signature_disposition(true),
            SignatureDisposition::InvalidatedAndRequiresAdapterSignature
        );
    }

    #[test]
    fn local_dev_allows_unsigned_materialized_targets() {
        let policy = SigningPolicy::local_dev(ReleaseChannel::local_dev());

        let result = policy.inspect_signature_presence(
            SigningSubjectKind::MaterializedTargetBundle,
            false,
            &ReleaseChannel::local_dev(),
            0,
        );

        assert_eq!(result.state, SigningInspectionState::UnsignedAllowed);
        assert_eq!(
            policy.materialized_target_signature_disposition(true),
            SignatureDisposition::InvalidatedAndUnsignedAllowedByPolicy
        );
    }

    #[test]
    fn transcript_digest_changes_when_channel_changes() {
        let artifact = artifact(BundleKind::Program, b"target");
        let file_digest = BundleDigest::of(b"whole file");
        let stable = SigningDigestTranscript::awfb_bundle(
            artifact,
            file_digest,
            "release-key",
            ReleaseChannel::new("stable").expect("channel"),
            1,
        )
        .expect("transcript");
        let beta = SigningDigestTranscript::awfb_bundle(
            artifact,
            file_digest,
            "release-key",
            ReleaseChannel::new("beta").expect("channel"),
            1,
        )
        .expect("transcript");

        assert_ne!(
            stable.digest().expect("digest"),
            beta.digest().expect("digest")
        );
    }

    #[test]
    fn changed_targets_never_preserve_base_signature_validity() {
        let release = SigningPolicy::release_publish(
            ReleaseChannel::new("stable").expect("channel"),
            KeyEpochPolicy::default(),
        );
        let dev = SigningPolicy::local_dev(ReleaseChannel::local_dev());

        assert_ne!(
            release.materialized_target_signature_disposition(true),
            SignatureDisposition::PreservedUnchangedTarget
        );
        assert_ne!(
            dev.materialized_target_signature_disposition(true),
            SignatureDisposition::PreservedUnchangedTarget
        );
        assert_eq!(
            release.materialized_target_signature_disposition(false),
            SignatureDisposition::PreservedUnchangedTarget
        );
    }
}
