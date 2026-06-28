use super::trust::{ReleaseTrustEvidence, ReleaseTrustEvidenceKind, inspect_release_trust};
use crate::cache::external_payload::{
    ExternalPayloadCacheFetchError, ExternalPayloadCacheFetchReport,
    fetch_external_payload_bytes_to_cache,
};
use arcweft_bundle::release::{
    archive::{
        AwfrArchiveError, AwfrArchiveManifest, ExternalPayloadCarrier,
        ExternalPayloadMaterializationMode,
    },
    signing_policy::{
        SigningDigestTranscript, SigningInspectionResult, SigningInspectionState, SigningPolicy,
        SigningPolicyError, SigningPolicyMode, SigningSubjectKind,
    },
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};
use thiserror::Error;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseConsumeVerificationReport {
    pub archive: String,
    pub channel: String,
    pub policy_mode: SigningPolicyMode,
    pub payload_mode: ExternalPayloadMaterializationMode,
    pub success: bool,
    pub signing: Vec<SigningInspectionResult>,
    pub payloads: Vec<ReleasePayloadInspectionResult>,
    pub trust: Vec<ReleaseTrustEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleasePayloadInspectionResult {
    pub bundle_content_root: String,
    pub descriptor_id: String,
    pub required: bool,
    pub residency: String,
    pub state: ReleasePayloadInspectionState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fetch_report: Option<ExternalPayloadCacheFetchReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleasePayloadInspectionState {
    MetadataOnly,
    Verified,
    MissingBytes,
    Invalid,
}

#[derive(Debug, Error)]
pub enum ReleaseConsumeVerificationError {
    #[error("failed to read AWFR archive `{path}`: {source}")]
    ReadArchive {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Archive(#[from] AwfrArchiveError),
    #[error(transparent)]
    SigningPolicy(#[from] SigningPolicyError),
}

pub fn verify_release_archive(
    archive_path: &Path,
    policy: &SigningPolicy,
    cache_root: &Path,
    payload_mode: ExternalPayloadMaterializationMode,
) -> Result<ReleaseConsumeVerificationReport, ReleaseConsumeVerificationError> {
    policy.validate()?;
    let archive_bytes =
        fs::read(archive_path).map_err(|source| ReleaseConsumeVerificationError::ReadArchive {
            path: archive_path.to_path_buf(),
            source,
        })?;
    let archive = AwfrArchiveManifest::from_json_slice(&archive_bytes)?;
    let signing = inspect_archive_signatures(policy, &archive)?;
    let payloads = archive
        .external_payloads
        .iter()
        .map(|carrier| inspect_payload(archive_path, cache_root, carrier, payload_mode))
        .collect::<Vec<_>>();
    let mut trust = inspect_release_trust(archive_path, &archive, policy);
    trust.extend(signing.iter().map(|result| signing_trust_evidence(*result)));
    trust.extend(payloads.iter().map(payload_trust_evidence));
    let success = report_success(&signing, &payloads, &trust);

    Ok(ReleaseConsumeVerificationReport {
        archive: archive_path.display().to_string(),
        channel: archive.channel.to_string(),
        policy_mode: policy.mode,
        payload_mode,
        success,
        signing,
        payloads,
        trust,
    })
}

fn inspect_archive_signatures(
    policy: &SigningPolicy,
    archive: &AwfrArchiveManifest,
) -> Result<Vec<SigningInspectionResult>, ReleaseConsumeVerificationError> {
    if archive.signatures.is_empty() {
        return Ok(vec![policy.inspect_signature_presence(
            SigningSubjectKind::AwfrReleaseArchive,
            false,
            &archive.channel,
            policy.key_epoch.min,
        )]);
    }

    let whole_file_digest = archive.unsigned_whole_file_digest()?;
    archive
        .signatures
        .iter()
        .map(|signature| {
            let mut result = policy.inspect_signature_presence(
                SigningSubjectKind::AwfrReleaseArchive,
                true,
                &archive.channel,
                signature.key_epoch,
            );
            let transcript = SigningDigestTranscript::awfr_release_archive(
                archive,
                whole_file_digest,
                &signature.signer_id,
                signature.key_epoch,
            )?;
            if signature.signing_digest != transcript.digest()? {
                result.state = SigningInspectionState::Invalid;
            }
            Ok(result)
        })
        .collect()
}

fn inspect_payload(
    archive_path: &Path,
    cache_root: &Path,
    carrier: &ExternalPayloadCarrier,
    payload_mode: ExternalPayloadMaterializationMode,
) -> ReleasePayloadInspectionResult {
    if !carrier.requires_payload_bytes(payload_mode) {
        return payload_result(
            carrier,
            ReleasePayloadInspectionState::MetadataOnly,
            None,
            None,
        );
    }

    match fetch_external_payload_bytes_to_cache(
        archive_path,
        carrier.bundle_content_root,
        carrier.descriptor_id,
        cache_root,
    ) {
        Ok(fetched) => payload_result(
            carrier,
            ReleasePayloadInspectionState::Verified,
            Some(fetched.report),
            None,
        ),
        Err(error) => payload_result(
            carrier,
            payload_error_state(&error),
            None,
            Some(error.to_string()),
        ),
    }
}

fn payload_result(
    carrier: &ExternalPayloadCarrier,
    state: ReleasePayloadInspectionState,
    fetch_report: Option<ExternalPayloadCacheFetchReport>,
    message: Option<String>,
) -> ReleasePayloadInspectionResult {
    ReleasePayloadInspectionResult {
        bundle_content_root: carrier.bundle_content_root.to_string(),
        descriptor_id: carrier.descriptor_id.to_string(),
        required: carrier.required,
        residency: format!("{:?}", carrier.residency),
        state,
        fetch_report,
        message,
    }
}

fn report_success(
    signing: &[SigningInspectionResult],
    payloads: &[ReleasePayloadInspectionResult],
    trust: &[ReleaseTrustEvidence],
) -> bool {
    signing
        .iter()
        .all(|result| signing_state_is_success(*result))
        && payloads.iter().all(payload_state_is_success)
        && trust.iter().all(ReleaseTrustEvidence::succeeds)
}

fn signing_state_is_success(result: SigningInspectionResult) -> bool {
    matches!(
        result.state,
        SigningInspectionState::Valid
            | SigningInspectionState::UnsignedAllowed
            | SigningInspectionState::MetadataOnlyExternalPayloads
    )
}

fn payload_state_is_success(result: &ReleasePayloadInspectionResult) -> bool {
    matches!(
        result.state,
        ReleasePayloadInspectionState::MetadataOnly | ReleasePayloadInspectionState::Verified
    )
}

fn signing_trust_evidence(result: SigningInspectionResult) -> ReleaseTrustEvidence {
    match result.state {
        SigningInspectionState::Valid | SigningInspectionState::UnsignedAllowed => {
            ReleaseTrustEvidence::passed(
                ReleaseTrustEvidenceKind::ArchiveSignature,
                "awfr_signature_valid",
                format!("{:?}", result.subject),
            )
        }
        SigningInspectionState::WrongChannel | SigningInspectionState::WrongEpoch => {
            ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::SigningPolicy,
                "wrong_signing_policy",
                format!("{:?}", result.subject),
                format!("signing state {:?}", result.state),
            )
        }
        SigningInspectionState::Invalid => ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::ArchiveSignature,
            "detached_signature_transcript_mismatch",
            format!("{:?}", result.subject),
            "detached AWFR signature transcript did not match the unsigned archive transcript",
        ),
        _ => ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::ArchiveSignature,
            "required_signature_missing",
            format!("{:?}", result.subject),
            format!("signing state {:?}", result.state),
        ),
    }
}

fn payload_trust_evidence(result: &ReleasePayloadInspectionResult) -> ReleaseTrustEvidence {
    let subject = format!("{}:{}", result.bundle_content_root, result.descriptor_id);
    match result.state {
        ReleasePayloadInspectionState::MetadataOnly => ReleaseTrustEvidence::passed(
            ReleaseTrustEvidenceKind::ExternalPayload,
            "external_payload_metadata_only",
            subject,
        ),
        ReleasePayloadInspectionState::Verified => ReleaseTrustEvidence::passed(
            ReleaseTrustEvidenceKind::ExternalPayload,
            "external_payload_verified",
            subject,
        ),
        ReleasePayloadInspectionState::MissingBytes => ReleaseTrustEvidence::recoverable_miss(
            ReleaseTrustEvidenceKind::ExternalPayload,
            "external_payload_missing",
            subject,
            result
                .message
                .clone()
                .unwrap_or_else(|| "payload bytes are missing".to_owned()),
        ),
        ReleasePayloadInspectionState::Invalid => {
            let message = result
                .message
                .clone()
                .unwrap_or_else(|| "payload bytes are invalid".to_owned());
            let code = if message.contains("byte length mismatch") {
                "external_payload_size_mismatch"
            } else if message.contains("digest mismatch") {
                "external_payload_digest_mismatch"
            } else {
                "external_payload_invalid"
            };
            ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::ExternalPayload,
                code,
                subject,
                message,
            )
        }
    }
}

fn payload_error_state(error: &ExternalPayloadCacheFetchError) -> ReleasePayloadInspectionState {
    match error {
        ExternalPayloadCacheFetchError::Archive(
            AwfrArchiveError::ByteLengthMismatch { .. }
            | AwfrArchiveError::DigestMismatch { .. }
            | AwfrArchiveError::DecodeCompressedPayload { .. },
        ) => ReleasePayloadInspectionState::Invalid,
        _ => ReleasePayloadInspectionState::MissingBytes,
    }
}
