use arcweft_bundle::{
    container::{BundleDigest, BundleView, ReadBudget},
    patch::{
        BundlePatchArtifact, PatchBundleError, PatchMaterializedTarget, apply_patch_bundle,
        decode_patch_bundle_with_signature_policy,
    },
    release::{
        ReleaseBundleRef, ReleaseManifestError, ReleaseMirror,
        archive::{AwfrArchiveManifest, AwfrPatchArtifactRef},
        signing_policy::{SigningPolicy, SigningSubjectKind},
    },
};
use serde::Serialize;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseTrustState {
    Passed,
    Skipped,
    RecoverableMiss,
    HardFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseTrustEvidenceKind {
    ArchiveManifest,
    ArchiveSignature,
    SigningPolicy,
    BaseBundleSignature,
    PatchArtifactSignature,
    PatchTargetIdentity,
    MaterializedTarget,
    MaterializedTargetSignature,
    ExternalPayload,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReleaseTrustEvidence {
    pub kind: ReleaseTrustEvidenceKind,
    pub state: ReleaseTrustState,
    pub code: String,
    pub subject: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl ReleaseTrustEvidence {
    pub fn passed(
        kind: ReleaseTrustEvidenceKind,
        code: impl Into<String>,
        subject: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            state: ReleaseTrustState::Passed,
            code: code.into(),
            subject: subject.into(),
            message: None,
        }
    }

    pub fn skipped(
        kind: ReleaseTrustEvidenceKind,
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            state: ReleaseTrustState::Skipped,
            code: code.into(),
            subject: subject.into(),
            message: Some(message.into()),
        }
    }

    pub fn recoverable_miss(
        kind: ReleaseTrustEvidenceKind,
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            state: ReleaseTrustState::RecoverableMiss,
            code: code.into(),
            subject: subject.into(),
            message: Some(message.into()),
        }
    }

    pub fn hard_failure(
        kind: ReleaseTrustEvidenceKind,
        code: impl Into<String>,
        subject: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            state: ReleaseTrustState::HardFailure,
            code: code.into(),
            subject: subject.into(),
            message: Some(message.into()),
        }
    }

    pub const fn succeeds(&self) -> bool {
        matches!(
            self.state,
            ReleaseTrustState::Passed | ReleaseTrustState::Skipped
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BundleVerificationEvidence {
    kind: ReleaseTrustEvidenceKind,
    success_code: &'static str,
    missing_signature_code: &'static str,
    digest_mismatch_code: &'static str,
}

const BASE_BUNDLE_VERIFICATION: BundleVerificationEvidence = BundleVerificationEvidence {
    kind: ReleaseTrustEvidenceKind::BaseBundleSignature,
    success_code: "base_signature_valid",
    missing_signature_code: "missing_base_signature",
    digest_mismatch_code: "base_bundle_digest_mismatch",
};

const TARGET_BUNDLE_VERIFICATION: BundleVerificationEvidence = BundleVerificationEvidence {
    kind: ReleaseTrustEvidenceKind::MaterializedTargetSignature,
    success_code: "materialized_target_signature_valid",
    missing_signature_code: "missing_materialized_target_signature",
    digest_mismatch_code: "materialized_target_digest_mismatch",
};

pub fn inspect_release_trust(
    archive_path: &Path,
    archive: &AwfrArchiveManifest,
    policy: &SigningPolicy,
) -> Vec<ReleaseTrustEvidence> {
    let mut evidence = Vec::new();
    evidence.push(ReleaseTrustEvidence::passed(
        ReleaseTrustEvidenceKind::ArchiveManifest,
        "archive_manifest_valid",
        archive_path.display().to_string(),
    ));

    if archive.channel != policy.channel {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::SigningPolicy,
            "wrong_signing_policy",
            archive_path.display().to_string(),
            format!(
                "archive channel `{}` does not match policy channel `{}`",
                archive.channel, policy.channel
            ),
        ));
    }

    if requires_awfb_family_signature(policy)
        && !archive
            .release_manifest
            .signature_policy
            .require_awfb_signature
    {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::SigningPolicy,
            "signature_policy_not_backed_by_release_manifest",
            archive_path.display().to_string(),
            "release consume policy requires AWFB-family signatures, but the AWFR release manifest signature_policy does not require them",
        ));
    }

    for patch in &archive.patches {
        inspect_patch_ref(archive_path, archive, policy, patch, &mut evidence);
    }

    if archive.patches.is_empty() {
        evidence.push(ReleaseTrustEvidence::skipped(
            ReleaseTrustEvidenceKind::MaterializedTarget,
            "no_patch_artifacts",
            archive_path.display().to_string(),
            "archive contains no patch references to materialize",
        ));
    }

    evidence
}

fn requires_awfb_family_signature(policy: &SigningPolicy) -> bool {
    policy.requires_signature(SigningSubjectKind::AwfbBundle)
        || policy.requires_signature(SigningSubjectKind::PatchV2Artifact)
        || policy.requires_signature(SigningSubjectKind::MaterializedTargetBundle)
}

fn inspect_patch_ref(
    archive_path: &Path,
    archive: &AwfrArchiveManifest,
    policy: &SigningPolicy,
    patch: &AwfrPatchArtifactRef,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    let archive_dir = archive_path.parent().unwrap_or_else(|| Path::new("."));
    let Some(artifact) = inspect_patch_artifact(archive_dir, archive, patch, evidence) else {
        return;
    };
    inspect_patch_target_identity(&artifact, patch, evidence);

    let Some(base_bytes) = inspect_base_bundle(archive, archive_dir, &artifact, evidence) else {
        return;
    };
    let Some(materialized) = materialize_patch_target(&base_bytes, &artifact, patch, evidence)
    else {
        return;
    };
    inspect_signed_target_bundle(archive, archive_dir, policy, patch, &materialized, evidence);
}

fn inspect_patch_artifact(
    archive_dir: &Path,
    archive: &AwfrArchiveManifest,
    patch: &AwfrPatchArtifactRef,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) -> Option<BundlePatchArtifact> {
    let patch_subject = format!("patch:{}", patch.patch_artifact.content_root);
    let patch_bytes = match read_first_file_mirror(archive_dir, &patch.mirrors) {
        Ok((_uri, bytes)) => bytes,
        Err(message) => {
            evidence.push(ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::PatchArtifactSignature,
                "missing_patch_artifact_bytes",
                patch_subject,
                message,
            ));
            return None;
        }
    };

    verify_patch_ref_bytes(patch, &patch_bytes, evidence);

    let patch_view = match BundleView::parse(&patch_bytes, ReadBudget::default()) {
        Ok(view) => view,
        Err(error) => {
            evidence.push(ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::PatchArtifactSignature,
                "patch_artifact_decode_failed",
                patch_subject,
                error.to_string(),
            ));
            return None;
        }
    };
    if patch_view.artifact_identity() != patch.patch_artifact {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::PatchArtifactSignature,
            "patch_artifact_identity_mismatch",
            format!("patch:{}", patch.patch_artifact.content_root),
            format!(
                "patch mirror identity {:?} does not match AWFR patch ref {:?}",
                patch_view.artifact_identity(),
                patch.patch_artifact
            ),
        ));
    }

    let artifact = match decode_patch_bundle_with_signature_policy(
        &patch_bytes,
        &archive.release_manifest.signature_policy,
    ) {
        Ok(artifact) => {
            evidence.push(ReleaseTrustEvidence::passed(
                ReleaseTrustEvidenceKind::PatchArtifactSignature,
                "patch_signature_valid",
                format!("patch:{}", patch.patch_artifact.content_root),
            ));
            artifact
        }
        Err(error) => {
            let code = patch_error_code(&error);
            evidence.push(ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::PatchArtifactSignature,
                code,
                format!("patch:{}", patch.patch_artifact.content_root),
                error.to_string(),
            ));
            return None;
        }
    };
    Some(artifact)
}

fn inspect_patch_target_identity(
    artifact: &BundlePatchArtifact,
    patch: &AwfrPatchArtifactRef,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    if artifact.manifest.target_artifact == patch.target_artifact {
        evidence.push(ReleaseTrustEvidence::passed(
            ReleaseTrustEvidenceKind::PatchTargetIdentity,
            "patch_target_identity_match",
            format!("target:{}", patch.target_artifact.content_root),
        ));
    } else {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::PatchTargetIdentity,
            "patch_target_identity_mismatch",
            format!("patch:{}", patch.patch_artifact.content_root),
            format!(
                "patch manifest target {:?} does not match AWFR patch ref target {:?}",
                artifact.manifest.target_artifact, patch.target_artifact
            ),
        ));
    }
}

fn inspect_base_bundle(
    archive: &AwfrArchiveManifest,
    archive_dir: &Path,
    artifact: &BundlePatchArtifact,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) -> Option<Vec<u8>> {
    let Some(base_ref) = archive
        .release_manifest
        .bundle(artifact.manifest.base_content_root)
    else {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::BaseBundleSignature,
            "missing_base_bundle_ref",
            format!("base:{}", artifact.manifest.base_content_root),
            "release manifest has no bundle for the patch base content root",
        ));
        return None;
    };
    let base_bytes = read_bundle_bytes(archive_dir, base_ref, "base", evidence)?;
    verify_bundle_ref_bytes(
        archive,
        base_ref,
        &base_bytes,
        BASE_BUNDLE_VERIFICATION,
        evidence,
    );
    Some(base_bytes)
}

fn materialize_patch_target(
    base_bytes: &[u8],
    artifact: &BundlePatchArtifact,
    patch: &AwfrPatchArtifactRef,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) -> Option<PatchMaterializedTarget> {
    let materialized = match apply_patch_bundle(base_bytes, artifact) {
        Ok(materialized) => materialized,
        Err(error) => {
            let code = materialization_error_code(&error);
            evidence.push(ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::MaterializedTarget,
                code,
                format!("target:{}", patch.target_artifact.content_root),
                error.to_string(),
            ));
            return None;
        }
    };
    inspect_materialized_target_identity(patch, &materialized, evidence);
    Some(materialized)
}

fn inspect_materialized_target_identity(
    patch: &AwfrPatchArtifactRef,
    materialized: &PatchMaterializedTarget,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    if materialized.report.target_artifact == patch.target_artifact {
        evidence.push(ReleaseTrustEvidence::passed(
            ReleaseTrustEvidenceKind::MaterializedTarget,
            "materialized_target_digest_match",
            format!("target:{}", patch.target_artifact.content_root),
        ));
    } else {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::MaterializedTarget,
            "materialized_target_digest_mismatch",
            format!("target:{}", patch.target_artifact.content_root),
            format!(
                "materialized target {:?} does not match AWFR target {:?}",
                materialized.report.target_artifact, patch.target_artifact
            ),
        ));
    }
}

fn inspect_signed_target_bundle(
    archive: &AwfrArchiveManifest,
    archive_dir: &Path,
    policy: &SigningPolicy,
    patch: &AwfrPatchArtifactRef,
    materialized: &PatchMaterializedTarget,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    match archive
        .release_manifest
        .bundle(patch.target_artifact.content_root)
    {
        Some(target_ref) => {
            let Some(target_bytes) = read_bundle_bytes(archive_dir, target_ref, "target", evidence)
            else {
                return;
            };
            verify_bundle_ref_bytes(
                archive,
                target_ref,
                &target_bytes,
                TARGET_BUNDLE_VERIFICATION,
                evidence,
            );
            inspect_target_file_identity(target_ref, &target_bytes, materialized, evidence);
        }
        None if policy.requires_signature(SigningSubjectKind::MaterializedTargetBundle) => {
            evidence.push(ReleaseTrustEvidence::hard_failure(
                ReleaseTrustEvidenceKind::MaterializedTargetSignature,
                "missing_materialized_target_signature",
                format!("target:{}", patch.target_artifact.content_root),
                "release manifest has no signed target bundle reference for the materialized target",
            ));
        }
        None => evidence.push(ReleaseTrustEvidence::skipped(
            ReleaseTrustEvidenceKind::MaterializedTargetSignature,
            "materialized_target_signature_not_required",
            format!("target:{}", patch.target_artifact.content_root),
            "selected signing policy does not require a materialized target signature",
        )),
    }
}

fn verify_patch_ref_bytes(
    patch: &AwfrPatchArtifactRef,
    patch_bytes: &[u8],
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    let subject = format!("patch:{}", patch.patch_artifact.content_root);
    let actual_len = u64::try_from(patch_bytes.len()).unwrap_or(u64::MAX);
    if actual_len != patch.byte_len {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::PatchArtifactSignature,
            "patch_artifact_byte_length_mismatch",
            subject.clone(),
            format!(
                "expected {} byte(s), actual {} byte(s)",
                patch.byte_len, actual_len
            ),
        ));
    }
    let actual_digest = BundleDigest::of(patch_bytes);
    if actual_digest != patch.file_digest {
        evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::PatchArtifactSignature,
            "patch_artifact_file_digest_mismatch",
            subject,
            format!("expected {}, actual {}", patch.file_digest, actual_digest),
        ));
    }
}

fn read_bundle_bytes(
    archive_dir: &Path,
    bundle: &ReleaseBundleRef,
    label: &str,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) -> Option<Vec<u8>> {
    match read_first_file_mirror(archive_dir, &bundle.mirrors) {
        Ok((_uri, bytes)) => Some(bytes),
        Err(message) => {
            evidence.push(ReleaseTrustEvidence::hard_failure(
                bundle_evidence_kind(label),
                format!("missing_{label}_bundle_bytes"),
                format!("{label}:{}", bundle.content_root),
                message,
            ));
            None
        }
    }
}

fn verify_bundle_ref_bytes(
    archive: &AwfrArchiveManifest,
    bundle: &ReleaseBundleRef,
    bytes: &[u8],
    verification: BundleVerificationEvidence,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    let subject = format!("bundle:{}", bundle.content_root);
    let plan = bundle.fetch_plan_with_policy(
        archive.release_manifest.fetch_policy.clone(),
        archive.release_manifest.signature_policy.clone(),
    );
    match plan.verify_bytes(bytes) {
        Ok(()) => evidence.push(ReleaseTrustEvidence::passed(
            verification.kind,
            verification.success_code,
            subject,
        )),
        Err(error) => evidence.push(ReleaseTrustEvidence::hard_failure(
            verification.kind,
            bundle_error_code(
                &error,
                verification.missing_signature_code,
                verification.digest_mismatch_code,
            ),
            subject,
            error.to_string(),
        )),
    }
}

fn inspect_target_file_identity(
    target_ref: &ReleaseBundleRef,
    target_bytes: &[u8],
    materialized: &arcweft_bundle::patch::PatchMaterializedTarget,
    evidence: &mut Vec<ReleaseTrustEvidence>,
) {
    match BundleView::parse(target_bytes, ReadBudget::default()) {
        Ok(view) if view.artifact_identity() == materialized.report.target_artifact => {}
        Ok(view) => evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::MaterializedTarget,
            "materialized_target_digest_mismatch",
            format!("target:{}", target_ref.content_root),
            format!(
                "target mirror identity {:?} does not match materialized target {:?}",
                view.artifact_identity(),
                materialized.report.target_artifact
            ),
        )),
        Err(error) => evidence.push(ReleaseTrustEvidence::hard_failure(
            ReleaseTrustEvidenceKind::MaterializedTarget,
            "materialized_target_decode_failed",
            format!("target:{}", target_ref.content_root),
            error.to_string(),
        )),
    }
}

fn read_first_file_mirror(
    archive_dir: &Path,
    mirrors: &[ReleaseMirror],
) -> Result<(String, Vec<u8>), String> {
    let mut sorted = mirrors.to_vec();
    sorted.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| left.uri.cmp(&right.uri))
    });
    let mut failures = Vec::new();
    for mirror in &sorted {
        if !mirror.uri.starts_with("file:") {
            failures.push(format!("skipped non-file mirror `{}`", mirror.uri));
            continue;
        }
        let path = file_mirror_path(archive_dir, &mirror.uri);
        match fs::read(&path) {
            Ok(bytes) => return Ok((mirror.uri.clone(), bytes)),
            Err(error) => failures.push(format!("{}: {error}", path.display())),
        }
    }
    Err(if failures.is_empty() {
        "no file mirrors were present".to_owned()
    } else {
        failures.join("; ")
    })
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

fn bundle_evidence_kind(label: &str) -> ReleaseTrustEvidenceKind {
    match label {
        "base" => ReleaseTrustEvidenceKind::BaseBundleSignature,
        "target" => ReleaseTrustEvidenceKind::MaterializedTargetSignature,
        _ => ReleaseTrustEvidenceKind::ArchiveManifest,
    }
}

fn bundle_error_code(
    error: &ReleaseManifestError,
    missing_signature_code: &'static str,
    digest_mismatch_code: &'static str,
) -> &'static str {
    match error {
        ReleaseManifestError::MissingAwfbSignature { .. } => missing_signature_code,
        ReleaseManifestError::ByteLengthMismatch { .. }
        | ReleaseManifestError::FileDigestMismatch { .. }
        | ReleaseManifestError::SignatureContentRootMismatch { .. }
        | ReleaseManifestError::SignatureKindMismatch { .. }
        | ReleaseManifestError::SignatureDigestMismatch { .. } => digest_mismatch_code,
        ReleaseManifestError::SignatureVerificationFailed { .. } => "signature_verification_failed",
        ReleaseManifestError::UntrustedSigner { .. } => "untrusted_signer",
        ReleaseManifestError::MissingTrustedPublicKey { .. } => "missing_trusted_public_key",
        ReleaseManifestError::NoValidTrustedPublicKey { .. } => "wrong_signing_policy",
        _ => "release_bundle_verification_failed",
    }
}

fn patch_error_code(error: &PatchBundleError) -> &'static str {
    match error {
        PatchBundleError::SignaturePolicy(ReleaseManifestError::MissingAwfbSignature {
            ..
        }) => "missing_patch_signature",
        PatchBundleError::SignaturePolicy(ReleaseManifestError::SignatureVerificationFailed {
            ..
        }) => "patch_signature_verification_failed",
        PatchBundleError::SignaturePolicy(ReleaseManifestError::NoValidTrustedPublicKey {
            ..
        }) => "wrong_signing_policy",
        PatchBundleError::TargetIdentityMismatch { .. } => "patch_target_identity_mismatch",
        PatchBundleError::TargetContentRootMismatch { .. } => "materialized_target_digest_mismatch",
        _ => "patch_artifact_verification_failed",
    }
}

fn materialization_error_code(error: &PatchBundleError) -> &'static str {
    match error {
        PatchBundleError::TargetIdentityMismatch { .. }
        | PatchBundleError::TargetContentRootMismatch { .. } => {
            "materialized_target_digest_mismatch"
        }
        PatchBundleError::BaseIdentityMismatch { .. } | PatchBundleError::WrongBase { .. } => {
            "patch_base_identity_mismatch"
        }
        _ => "patch_materialization_failed",
    }
}
