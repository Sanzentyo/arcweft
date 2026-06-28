use crate::cache::external_payload::{
    ExternalPayloadCacheFetchError, ExternalPayloadCacheFetchReport,
    fetch_external_payload_bytes_to_cache,
};
use arcweft_bundle::{
    container::{BundleDigest, ContentResidency},
    release::{
        archive::{
            AwfrArchiveError, AwfrArchiveManifest, ExternalPayloadCarrier,
            ExternalPayloadMaterializationMode,
        },
        signing_policy::{
            SigningDigestTranscript, SigningInspectionResult, SigningInspectionState,
            SigningPolicy, SigningPolicyError, SigningPolicyMode, SigningSubjectKind,
        },
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
    pub signing: Vec<SigningInspectionResult>,
    pub payloads: Vec<ReleasePayloadInspectionResult>,
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
    let signing = inspect_archive_signatures(policy, &archive, &archive_bytes)?;
    let payloads = archive
        .external_payloads
        .iter()
        .map(|carrier| inspect_payload(archive_path, cache_root, carrier, payload_mode))
        .collect::<Vec<_>>();

    Ok(ReleaseConsumeVerificationReport {
        archive: archive_path.display().to_string(),
        channel: archive.channel.to_string(),
        policy_mode: policy.mode,
        payload_mode,
        signing,
        payloads,
    })
}

fn inspect_archive_signatures(
    policy: &SigningPolicy,
    archive: &AwfrArchiveManifest,
    archive_bytes: &[u8],
) -> Result<Vec<SigningInspectionResult>, ReleaseConsumeVerificationError> {
    if archive.signatures.is_empty() {
        return Ok(vec![policy.inspect_signature_presence(
            SigningSubjectKind::AwfrReleaseArchive,
            false,
            &archive.channel,
            policy.key_epoch.min,
        )]);
    }

    let whole_file_digest = BundleDigest::of(archive_bytes);
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
    if !payload_mode_requires_bytes(carrier, payload_mode) {
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

fn payload_mode_requires_bytes(
    carrier: &ExternalPayloadCarrier,
    payload_mode: ExternalPayloadMaterializationMode,
) -> bool {
    match payload_mode {
        ExternalPayloadMaterializationMode::MetadataOnly => false,
        ExternalPayloadMaterializationMode::RequiredResidency => {
            carrier.required || carrier.residency == ContentResidency::Startup
        }
        ExternalPayloadMaterializationMode::AllPayloads => true,
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
