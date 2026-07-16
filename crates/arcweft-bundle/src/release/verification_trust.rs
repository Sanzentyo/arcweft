//! Signed verification-trust authority artifacts.
//!
//! This module owns the strict schema-1 wire model and deterministic domain
//! transcripts. It is deliberately Sans I/O: callers provide authority bytes
//! and the release trust policy, while filesystem discovery and key storage
//! remain in adapters.

use super::archive::ReleaseChannel;
use super::signing_policy::{
    SigningDigestTranscript, SigningPolicy, SigningPolicyMode, SigningSubjectKind,
};
use super::{
    RELEASE_SIGNATURE_ALGORITHM_ED25519_V1, ReleaseSignaturePolicy, ReleaseTrustedPublicKey,
};
use crate::container::BundleDigest;
use std::collections::BTreeSet;
use thiserror::Error;

pub const VERIFICATION_TRUST_SCHEMA_VERSION: u32 = 1;
pub const VERIFICATION_TRUST_AUTHORITY_MAX_BYTES: usize = 32 * 1024 * 1024;
pub const VERIFICATION_TRUST_ARTIFACT_MAX_BYTES: usize = 16 * 1024 * 1024;
pub const VERIFICATION_TRUST_MAX_RECORDS: usize = 65_536;

macro_rules! validated_string_type {
    ($name:ident, $label:literal, $reject_blank:expr) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, VerificationTrustError> {
                let value = value.into();
                let invalid = if $reject_blank {
                    value.trim().is_empty()
                } else {
                    value.is_empty()
                };
                if invalid {
                    return Err(VerificationTrustError::InvalidText {
                        kind: $label,
                        message: "must not be empty".to_owned(),
                    });
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str(self.as_str())
            }
        }

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Self::new(value).map_err(serde::de::Error::custom)
            }
        }
    };
}

validated_string_type!(
    VerificationTrustPolicyId,
    "verification trust policy id",
    false
);
validated_string_type!(VerificationPackageId, "verification package id", false);
validated_string_type!(VerificationProfileId, "verification profile id", false);
validated_string_type!(CanonicalModulePathWire, "canonical module path", false);
validated_string_type!(ProofName, "proof name", false);
validated_string_type!(TrustReasonWire, "trust reason", true);
validated_string_type!(
    BuildAttestationProducerId,
    "build attestation producer id",
    false
);
validated_string_type!(HostFactContractId, "host fact contract id", false);
validated_string_type!(ExternalVerifierId, "external verifier id", false);
validated_string_type!(AuthorityCaseId, "authority case id", false);
validated_string_type!(RevocationReason, "revocation reason", true);

/// Stable digest identity for one exact trusted-proof admission.
#[derive(
    Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(transparent)]
pub struct TrustedProofAdmissionId(BundleDigest);

impl TrustedProofAdmissionId {
    pub const fn from_digest(digest: BundleDigest) -> Self {
        Self(digest)
    }

    pub const fn digest(self) -> BundleDigest {
        self.0
    }
}

impl std::fmt::Display for TrustedProofAdmissionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct ProofDeclarationId {
    pub package: VerificationPackageId,
    pub module: CanonicalModulePathWire,
    pub name: ProofName,
}

impl ProofDeclarationId {
    pub fn new(
        package: VerificationPackageId,
        module: CanonicalModulePathWire,
        name: ProofName,
    ) -> Self {
        Self {
            package,
            module,
            name,
        }
    }
}

#[derive(
    Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, serde::Deserialize, serde::Serialize,
)]
#[serde(deny_unknown_fields)]
pub struct TrustedProofSubject {
    pub declaration: ProofDeclarationId,
    pub contract_digest: BundleDigest,
    pub reason: TrustReasonWire,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TrustedEvidence {
    SignedBuildAttestation {
        producer_id: BuildAttestationProducerId,
        statement_digest: BundleDigest,
        artifact_digest: BundleDigest,
    },
    SignedHostFact {
        host_contract_id: HostFactContractId,
        statement_digest: BundleDigest,
        manifest_digest: BundleDigest,
    },
    ExternalProofCertificate {
        verifier_id: ExternalVerifierId,
        statement_digest: BundleDigest,
        certificate_digest: BundleDigest,
    },
    ExplicitPolicyAdmission {
        authority_case_id: AuthorityCaseId,
        statement_digest: BundleDigest,
    },
}

impl TrustedEvidence {
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::SignedBuildAttestation { .. } => "signed_build_attestation",
            Self::SignedHostFact { .. } => "signed_host_fact",
            Self::ExternalProofCertificate { .. } => "external_proof_certificate",
            Self::ExplicitPolicyAdmission { .. } => "explicit_policy_admission",
        }
    }

    pub const fn statement_digest(&self) -> BundleDigest {
        match self {
            Self::SignedBuildAttestation {
                statement_digest, ..
            }
            | Self::SignedHostFact {
                statement_digest, ..
            }
            | Self::ExternalProofCertificate {
                statement_digest, ..
            }
            | Self::ExplicitPolicyAdmission {
                statement_digest, ..
            } => *statement_digest,
        }
    }

    fn write_canonical(
        &self,
        transcript: &mut CanonicalTranscript,
    ) -> Result<(), VerificationTrustError> {
        transcript.string(self.kind())?;
        match self {
            Self::SignedBuildAttestation {
                producer_id,
                statement_digest,
                artifact_digest,
            } => {
                transcript.string(producer_id.as_str())?;
                transcript.digest(*statement_digest);
                transcript.digest(*artifact_digest);
            }
            Self::SignedHostFact {
                host_contract_id,
                statement_digest,
                manifest_digest,
            } => {
                transcript.string(host_contract_id.as_str())?;
                transcript.digest(*statement_digest);
                transcript.digest(*manifest_digest);
            }
            Self::ExternalProofCertificate {
                verifier_id,
                statement_digest,
                certificate_digest,
            } => {
                transcript.string(verifier_id.as_str())?;
                transcript.digest(*statement_digest);
                transcript.digest(*certificate_digest);
            }
            Self::ExplicitPolicyAdmission {
                authority_case_id,
                statement_digest,
            } => {
                transcript.string(authority_case_id.as_str())?;
                transcript.digest(*statement_digest);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedProofAdmission {
    pub admission_id: TrustedProofAdmissionId,
    pub subject: TrustedProofSubject,
    pub evidence: TrustedEvidence,
}

impl TrustedProofAdmission {
    pub fn new(
        policy_id: &VerificationTrustPolicyId,
        subject: TrustedProofSubject,
        evidence: TrustedEvidence,
    ) -> Result<Self, VerificationTrustError> {
        ensure_evidence_binding(&subject, &evidence)?;
        let admission_id = Self::compute_id(policy_id, &subject, &evidence)?;
        Ok(Self {
            admission_id,
            subject,
            evidence,
        })
    }

    pub fn compute_id(
        policy_id: &VerificationTrustPolicyId,
        subject: &TrustedProofSubject,
        evidence: &TrustedEvidence,
    ) -> Result<TrustedProofAdmissionId, VerificationTrustError> {
        let mut transcript =
            CanonicalTranscript::with_domain(b"arcweft.verification-trust-admission.v1\0");
        transcript.string(policy_id.as_str())?;
        subject.write_canonical(&mut transcript)?;
        evidence.write_canonical(&mut transcript)?;
        Ok(TrustedProofAdmissionId::from_digest(
            transcript.digest_value(),
        ))
    }

    fn write_canonical(
        &self,
        transcript: &mut CanonicalTranscript,
    ) -> Result<(), VerificationTrustError> {
        transcript.digest(self.admission_id.digest());
        self.subject.write_canonical(transcript)?;
        self.evidence.write_canonical(transcript)
    }
}

impl TrustedProofSubject {
    fn write_canonical(
        &self,
        transcript: &mut CanonicalTranscript,
    ) -> Result<(), VerificationTrustError> {
        transcript.string(self.declaration.package.as_str())?;
        transcript.string(self.declaration.module.as_str())?;
        transcript.string(self.declaration.name.as_str())?;
        transcript.digest(self.contract_digest);
        transcript.string(self.reason.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustManifest {
    pub schema_version: u32,
    pub policy_id: VerificationTrustPolicyId,
    pub generation: u64,
    pub channel: ReleaseChannel,
    pub package: VerificationPackageId,
    pub profile: VerificationProfileId,
    pub admissions: Vec<TrustedProofAdmission>,
}

impl VerificationTrustManifest {
    pub fn validate(&self) -> Result<(), VerificationTrustError> {
        ensure_schema("verification trust manifest", self.schema_version)?;
        self.channel
            .validate()
            .map_err(|error| VerificationTrustError::InvalidChannel(error.to_string()))?;
        ensure_record_limit("admissions", self.admissions.len())?;

        let mut ids = BTreeSet::new();
        let mut declarations = BTreeSet::new();
        for admission in &self.admissions {
            if admission.subject.declaration.package != self.package {
                return Err(VerificationTrustError::AdmissionPackageMismatch {
                    manifest: self.package.clone(),
                    declaration: admission.subject.declaration.package.clone(),
                });
            }
            ensure_evidence_binding(&admission.subject, &admission.evidence)?;
            let expected = TrustedProofAdmission::compute_id(
                &self.policy_id,
                &admission.subject,
                &admission.evidence,
            )?;
            if admission.admission_id != expected {
                return Err(VerificationTrustError::AdmissionIdMismatch {
                    declared: admission.admission_id,
                    expected,
                });
            }
            if !ids.insert(admission.admission_id) {
                return Err(VerificationTrustError::DuplicateAdmissionId(
                    admission.admission_id,
                ));
            }
            if !declarations.insert(admission.subject.declaration.clone()) {
                return Err(VerificationTrustError::DuplicateAdmissionSubject(
                    admission.subject.declaration.clone(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<BundleDigest, VerificationTrustError> {
        self.validate()?;
        let mut transcript =
            CanonicalTranscript::with_domain(b"arcweft.verification-trust-manifest.v1\0");
        transcript.u32(self.schema_version);
        transcript.string(self.policy_id.as_str())?;
        transcript.u64(self.generation);
        transcript.string(self.channel.as_str())?;
        transcript.string(self.package.as_str())?;
        transcript.string(self.profile.as_str())?;
        transcript.count(self.admissions.len())?;
        let mut admissions = self.admissions.iter().collect::<Vec<_>>();
        admissions.sort_by_key(|admission| admission.admission_id);
        for admission in admissions {
            admission.write_canonical(&mut transcript)?;
        }
        Ok(transcript.digest_value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustSignature {
    pub schema_version: u32,
    pub signer_id: String,
    pub algorithm: String,
    pub key_epoch: u64,
    pub manifest_digest: BundleDigest,
    pub signing_digest: BundleDigest,
    pub signature: String,
}

impl VerificationTrustSignature {
    pub fn validate_binding(
        &self,
        subject: SigningSubjectKind,
        channel: &ReleaseChannel,
        manifest_digest: BundleDigest,
    ) -> Result<(), VerificationTrustError> {
        ensure_schema("verification trust signature", self.schema_version)?;
        if self.signer_id.is_empty() {
            return Err(VerificationTrustError::InvalidSignature(
                "signer_id must not be empty".to_owned(),
            ));
        }
        if self.algorithm != RELEASE_SIGNATURE_ALGORITHM_ED25519_V1 {
            return Err(VerificationTrustError::InvalidSignature(format!(
                "unsupported algorithm `{}`",
                self.algorithm
            )));
        }
        decode_lower_hex::<64>(&self.signature).map_err(|message| {
            VerificationTrustError::InvalidSignature(format!("invalid signature: {message}"))
        })?;
        if self.manifest_digest != manifest_digest {
            return Err(VerificationTrustError::ManifestDigestMismatch {
                declared: self.manifest_digest,
                expected: manifest_digest,
            });
        }
        let transcript = match subject {
            SigningSubjectKind::VerificationTrustManifest => {
                SigningDigestTranscript::verification_trust_manifest(
                    manifest_digest,
                    self.signer_id.clone(),
                    channel.clone(),
                    self.key_epoch,
                )
            }
            SigningSubjectKind::VerificationTrustRevocations => {
                SigningDigestTranscript::verification_trust_revocations(
                    manifest_digest,
                    self.signer_id.clone(),
                    channel.clone(),
                    self.key_epoch,
                )
            }
            other => {
                return Err(VerificationTrustError::InvalidSignature(format!(
                    "unsupported verification trust signing subject `{}`",
                    other.as_str()
                )));
            }
        }
        .map_err(|error| VerificationTrustError::InvalidSignature(error.to_string()))?;
        let expected = transcript
            .digest()
            .map_err(|error| VerificationTrustError::InvalidSignature(error.to_string()))?;
        if self.signing_digest != expected {
            return Err(VerificationTrustError::SigningDigestMismatch {
                declared: self.signing_digest,
                expected,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustArtifact {
    pub manifest: VerificationTrustManifest,
    pub signature: VerificationTrustSignature,
}

impl VerificationTrustArtifact {
    pub fn validate(&self) -> Result<(), VerificationTrustError> {
        let manifest_digest = self.manifest.digest()?;
        self.signature.validate_binding(
            SigningSubjectKind::VerificationTrustManifest,
            &self.manifest.channel,
            manifest_digest,
        )
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, VerificationTrustError> {
        ensure_byte_limit(
            "verification trust artifact",
            bytes.len(),
            VERIFICATION_TRUST_ARTIFACT_MAX_BYTES,
        )?;
        let artifact = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| VerificationTrustError::DecodeJson(error.to_string()))?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, VerificationTrustError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized
            .manifest
            .admissions
            .sort_by_key(|admission| admission.admission_id);
        serde_json::to_vec_pretty(&normalized)
            .map_err(|error| VerificationTrustError::EncodeJson(error.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct RevokedTrustedProofAdmission {
    pub admission_id: TrustedProofAdmissionId,
    pub reason: RevocationReason,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustRevocations {
    pub schema_version: u32,
    pub policy_id: VerificationTrustPolicyId,
    pub generation: u64,
    pub channel: ReleaseChannel,
    pub revoked: Vec<RevokedTrustedProofAdmission>,
}

impl VerificationTrustRevocations {
    pub fn validate(&self) -> Result<(), VerificationTrustError> {
        ensure_schema("verification trust revocations", self.schema_version)?;
        self.channel
            .validate()
            .map_err(|error| VerificationTrustError::InvalidChannel(error.to_string()))?;
        ensure_record_limit("revocations", self.revoked.len())?;
        let mut ids = BTreeSet::new();
        for revoked in &self.revoked {
            if !ids.insert(revoked.admission_id) {
                return Err(VerificationTrustError::DuplicateRevocationId(
                    revoked.admission_id,
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<BundleDigest, VerificationTrustError> {
        self.validate()?;
        let mut transcript =
            CanonicalTranscript::with_domain(b"arcweft.verification-trust-revocations.v1\0");
        transcript.u32(self.schema_version);
        transcript.string(self.policy_id.as_str())?;
        transcript.u64(self.generation);
        transcript.string(self.channel.as_str())?;
        transcript.count(self.revoked.len())?;
        let mut revoked = self.revoked.iter().collect::<Vec<_>>();
        revoked.sort_by_key(|record| record.admission_id);
        for record in revoked {
            transcript.digest(record.admission_id.digest());
            transcript.string(record.reason.as_str())?;
        }
        Ok(transcript.digest_value())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustRevocationArtifact {
    pub manifest: VerificationTrustRevocations,
    pub signature: VerificationTrustSignature,
}

impl VerificationTrustRevocationArtifact {
    pub fn validate(&self) -> Result<(), VerificationTrustError> {
        let manifest_digest = self.manifest.digest()?;
        self.signature.validate_binding(
            SigningSubjectKind::VerificationTrustRevocations,
            &self.manifest.channel,
            manifest_digest,
        )
    }

    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, VerificationTrustError> {
        ensure_byte_limit(
            "verification trust revocation artifact",
            bytes.len(),
            VERIFICATION_TRUST_ARTIFACT_MAX_BYTES,
        )?;
        let artifact = serde_json::from_slice::<Self>(bytes)
            .map_err(|error| VerificationTrustError::DecodeJson(error.to_string()))?;
        artifact.validate()?;
        Ok(artifact)
    }

    pub fn to_json_bytes(&self) -> Result<Vec<u8>, VerificationTrustError> {
        self.validate()?;
        let mut normalized = self.clone();
        normalized
            .manifest
            .revoked
            .sort_by_key(|record| record.admission_id);
        serde_json::to_vec_pretty(&normalized)
            .map_err(|error| VerificationTrustError::EncodeJson(error.to_string()))
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct VerificationTrustAuthorityDocument {
    pub schema_version: u32,
    pub signing_policy: SigningPolicy,
    pub signature_policy: ReleaseSignaturePolicy,
    pub trust_manifest: VerificationTrustArtifact,
    pub revocations: VerificationTrustRevocationArtifact,
}

/// Authority document after policy, freshness, binding, and cryptographic checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedVerificationTrustAuthority {
    document: VerificationTrustAuthorityDocument,
    manifest_digest: BundleDigest,
    revocations_digest: BundleDigest,
    revoked: BTreeSet<TrustedProofAdmissionId>,
}

impl ValidatedVerificationTrustAuthority {
    pub fn from_json_slice(bytes: &[u8]) -> Result<Self, VerificationTrustError> {
        ensure_byte_limit(
            "verification trust authority document",
            bytes.len(),
            VERIFICATION_TRUST_AUTHORITY_MAX_BYTES,
        )?;
        let document = serde_json::from_slice::<VerificationTrustAuthorityDocument>(bytes)
            .map_err(|error| VerificationTrustError::DecodeJson(error.to_string()))?;
        Self::try_from_document(document)
    }

    pub fn try_from_document(
        document: VerificationTrustAuthorityDocument,
    ) -> Result<Self, VerificationTrustError> {
        ensure_schema("verification trust authority", document.schema_version)?;
        document
            .signing_policy
            .validate()
            .map_err(|error| VerificationTrustError::InvalidSigningPolicy(error.to_string()))?;
        document
            .signature_policy
            .validate()
            .map_err(|error| VerificationTrustError::InvalidSignaturePolicy(error.to_string()))?;
        document.trust_manifest.manifest.validate()?;
        document.revocations.manifest.validate()?;

        let channel = &document.signing_policy.channel;
        if &document.trust_manifest.manifest.channel != channel {
            return Err(VerificationTrustError::ChannelMismatch {
                expected: channel.clone(),
                actual: document.trust_manifest.manifest.channel.clone(),
            });
        }
        if &document.revocations.manifest.channel != channel {
            return Err(VerificationTrustError::ChannelMismatch {
                expected: channel.clone(),
                actual: document.revocations.manifest.channel.clone(),
            });
        }
        if document.trust_manifest.manifest.policy_id != document.revocations.manifest.policy_id {
            return Err(VerificationTrustError::RevocationPolicyMismatch);
        }

        let generation_policy = document.signing_policy.verification_trust;
        if document.trust_manifest.manifest.generation < generation_policy.minimum_policy_generation
        {
            return Err(VerificationTrustError::StalePolicyGeneration {
                actual: document.trust_manifest.manifest.generation,
                minimum: generation_policy.minimum_policy_generation,
            });
        }
        if document.revocations.manifest.generation
            < generation_policy.minimum_revocation_generation
        {
            return Err(VerificationTrustError::StaleRevocationGeneration {
                actual: document.revocations.manifest.generation,
                minimum: generation_policy.minimum_revocation_generation,
            });
        }

        if matches!(
            document.signing_policy.mode,
            SigningPolicyMode::ReleasePublish | SigningPolicyMode::ReleaseConsume
        ) {
            for subject in [
                SigningSubjectKind::VerificationTrustManifest,
                SigningSubjectKind::VerificationTrustRevocations,
            ] {
                if !document.signing_policy.requires_signature(subject) {
                    return Err(VerificationTrustError::MissingRequiredSigningSubject(
                        subject,
                    ));
                }
            }
        }

        for signature in [
            &document.trust_manifest.signature,
            &document.revocations.signature,
        ] {
            if !document
                .signing_policy
                .key_epoch
                .contains(signature.key_epoch)
            {
                return Err(VerificationTrustError::KeyEpochRejected {
                    epoch: signature.key_epoch,
                });
            }
        }

        document.trust_manifest.validate()?;
        document.revocations.validate()?;
        for signature in [
            &document.trust_manifest.signature,
            &document.revocations.signature,
        ] {
            document
                .signature_policy
                .verify_verification_trust_signature(signature)?;
        }

        let manifest_digest = document.trust_manifest.manifest.digest()?;
        let revocations_digest = document.revocations.manifest.digest()?;
        let revoked = document
            .revocations
            .manifest
            .revoked
            .iter()
            .map(|record| record.admission_id)
            .collect();
        Ok(Self {
            document,
            manifest_digest,
            revocations_digest,
            revoked,
        })
    }

    pub fn document(&self) -> &VerificationTrustAuthorityDocument {
        &self.document
    }

    pub const fn manifest_digest(&self) -> BundleDigest {
        self.manifest_digest
    }

    pub const fn revocations_digest(&self) -> BundleDigest {
        self.revocations_digest
    }

    pub fn is_revoked(&self, admission_id: TrustedProofAdmissionId) -> bool {
        self.revoked.contains(&admission_id)
    }
}

impl ReleaseSignaturePolicy {
    /// Verifies a trust-artifact signature over the raw signing transcript digest.
    pub fn verify_verification_trust_signature(
        &self,
        signature: &VerificationTrustSignature,
    ) -> Result<(), VerificationTrustError> {
        self.validate()
            .map_err(|error| VerificationTrustError::InvalidSignaturePolicy(error.to_string()))?;
        if !self
            .allowed_algorithms
            .iter()
            .any(|algorithm| algorithm == &signature.algorithm)
        {
            return Err(VerificationTrustError::SignatureAlgorithmRejected(
                signature.algorithm.clone(),
            ));
        }
        if !self.trusted_signer_ids.is_empty()
            && !self
                .trusted_signer_ids
                .iter()
                .any(|signer_id| signer_id == &signature.signer_id)
        {
            return Err(VerificationTrustError::UntrustedSigner(
                signature.signer_id.clone(),
            ));
        }

        let signature_bytes = decode_lower_hex::<64>(&signature.signature).map_err(|message| {
            VerificationTrustError::InvalidSignature(format!("invalid signature: {message}"))
        })?;
        if let Some(minimum) = self.minimum_signature_bytes {
            let actual = u64::try_from(signature_bytes.len()).map_err(|_| {
                VerificationTrustError::InvalidSignature(
                    "signature byte length cannot be represented as u64".to_owned(),
                )
            })?;
            if actual < minimum {
                return Err(VerificationTrustError::SignatureTooSmall { minimum, actual });
            }
        }

        let matching = self
            .trusted_public_keys
            .iter()
            .filter(|key| {
                key.signer_id == signature.signer_id && key.algorithm == signature.algorithm
            })
            .collect::<Vec<_>>();
        if matching.is_empty() {
            return Err(VerificationTrustError::MissingTrustedPublicKey {
                signer_id: signature.signer_id.clone(),
                algorithm: signature.algorithm.clone(),
            });
        }
        let valid = matching
            .into_iter()
            .filter(|key| key.is_valid_for_verification_trust_epoch(signature.key_epoch))
            .collect::<Vec<_>>();
        if valid.is_empty() {
            return Err(VerificationTrustError::NoValidTrustedPublicKey {
                signer_id: signature.signer_id.clone(),
                algorithm: signature.algorithm.clone(),
                key_epoch: signature.key_epoch,
            });
        }

        for key in valid {
            if key
                .verify_verification_trust_digest(signature.signing_digest, &signature_bytes)
                .is_ok()
            {
                return Ok(());
            }
        }
        Err(VerificationTrustError::SignatureVerificationFailed {
            signer_id: signature.signer_id.clone(),
        })
    }
}

impl ReleaseTrustedPublicKey {
    fn is_valid_for_verification_trust_epoch(&self, key_epoch: u64) -> bool {
        !self.revoked
            && key_epoch >= self.valid_from_key_epoch
            && self
                .valid_until_key_epoch
                .is_none_or(|until| key_epoch < until)
    }

    fn verify_verification_trust_digest(
        &self,
        signing_digest: BundleDigest,
        signature: &[u8; 64],
    ) -> Result<(), VerificationTrustError> {
        let public_key = decode_lower_hex::<32>(&self.public_key).map_err(|message| {
            VerificationTrustError::InvalidSignaturePolicy(format!(
                "trusted public key is invalid: {message}"
            ))
        })?;
        let verifying_key =
            ed25519_dalek::VerifyingKey::from_bytes(&public_key).map_err(|error| {
                VerificationTrustError::InvalidSignaturePolicy(format!(
                    "trusted public key is invalid: {error}"
                ))
            })?;
        let signature = ed25519_dalek::Signature::from_bytes(signature);
        ed25519_dalek::Verifier::verify(&verifying_key, &signing_digest.as_bytes(), &signature)
            .map_err(|_| VerificationTrustError::SignatureVerificationFailed {
                signer_id: self.signer_id.clone(),
            })
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum VerificationTrustError {
    #[error("unsupported {kind} schema version {actual}; expected {expected}")]
    UnsupportedSchema {
        kind: &'static str,
        actual: u32,
        expected: u32,
    },
    #[error("invalid {kind}: {message}")]
    InvalidText { kind: &'static str, message: String },
    #[error("invalid release channel: {0}")]
    InvalidChannel(String),
    #[error("{kind} exceeds byte limit {limit}: actual {actual}")]
    ByteLimitExceeded {
        kind: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error("{kind} exceeds record limit {limit}: actual {actual}")]
    RecordLimitExceeded {
        kind: &'static str,
        limit: usize,
        actual: usize,
    },
    #[error("canonical {kind} length {actual} cannot be represented as u32")]
    CanonicalLengthOverflow { kind: &'static str, actual: usize },
    #[error("failed to decode verification trust JSON: {0}")]
    DecodeJson(String),
    #[error("failed to encode verification trust JSON: {0}")]
    EncodeJson(String),
    #[error("invalid signing policy: {0}")]
    InvalidSigningPolicy(String),
    #[error("invalid release signature policy: {0}")]
    InvalidSignaturePolicy(String),
    #[error("invalid verification trust signature: {0}")]
    InvalidSignature(String),
    #[error("signature manifest digest mismatch: declared {declared}, expected {expected}")]
    ManifestDigestMismatch {
        declared: BundleDigest,
        expected: BundleDigest,
    },
    #[error("signature signing digest mismatch: declared {declared}, expected {expected}")]
    SigningDigestMismatch {
        declared: BundleDigest,
        expected: BundleDigest,
    },
    #[error("signature algorithm `{0}` is not allowed")]
    SignatureAlgorithmRejected(String),
    #[error("signer `{0}` is not trusted")]
    UntrustedSigner(String),
    #[error("no trusted public key for signer `{signer_id}` and algorithm `{algorithm}`")]
    MissingTrustedPublicKey {
        signer_id: String,
        algorithm: String,
    },
    #[error(
        "no trusted public key valid for signer `{signer_id}`, algorithm `{algorithm}`, and key epoch {key_epoch}"
    )]
    NoValidTrustedPublicKey {
        signer_id: String,
        algorithm: String,
        key_epoch: u64,
    },
    #[error("signature verification failed for signer `{signer_id}`")]
    SignatureVerificationFailed { signer_id: String },
    #[error("signature is too small: minimum {minimum}, actual {actual}")]
    SignatureTooSmall { minimum: u64, actual: u64 },
    #[error("signature key epoch {epoch} is outside the signing-policy window")]
    KeyEpochRejected { epoch: u64 },
    #[error("release channel mismatch: expected `{expected}`, actual `{actual}`")]
    ChannelMismatch {
        expected: ReleaseChannel,
        actual: ReleaseChannel,
    },
    #[error("trust manifest and revocation policy ids differ")]
    RevocationPolicyMismatch,
    #[error("trust policy generation {actual} is below external floor {minimum}")]
    StalePolicyGeneration { actual: u64, minimum: u64 },
    #[error("revocation generation {actual} is below external floor {minimum}")]
    StaleRevocationGeneration { actual: u64, minimum: u64 },
    #[error("release signing policy does not require subject `{0:?}`")]
    MissingRequiredSigningSubject(SigningSubjectKind),
    #[error("admission evidence statement digest does not match the proof contract digest")]
    EvidenceStatementMismatch,
    #[error("admission id mismatch: declared {declared}, expected {expected}")]
    AdmissionIdMismatch {
        declared: TrustedProofAdmissionId,
        expected: TrustedProofAdmissionId,
    },
    #[error("duplicate admission id {0}")]
    DuplicateAdmissionId(TrustedProofAdmissionId),
    #[error("duplicate admission subject {0:?}")]
    DuplicateAdmissionSubject(ProofDeclarationId),
    #[error(
        "admission declaration package `{declaration}` does not match manifest package `{manifest}`"
    )]
    AdmissionPackageMismatch {
        manifest: VerificationPackageId,
        declaration: VerificationPackageId,
    },
    #[error("duplicate revoked admission id {0}")]
    DuplicateRevocationId(TrustedProofAdmissionId),
}

fn ensure_schema(kind: &'static str, actual: u32) -> Result<(), VerificationTrustError> {
    if actual == VERIFICATION_TRUST_SCHEMA_VERSION {
        Ok(())
    } else {
        Err(VerificationTrustError::UnsupportedSchema {
            kind,
            actual,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    }
}

fn ensure_byte_limit(
    kind: &'static str,
    actual: usize,
    limit: usize,
) -> Result<(), VerificationTrustError> {
    if actual <= limit {
        Ok(())
    } else {
        Err(VerificationTrustError::ByteLimitExceeded {
            kind,
            limit,
            actual,
        })
    }
}

fn ensure_record_limit(kind: &'static str, actual: usize) -> Result<(), VerificationTrustError> {
    if actual <= VERIFICATION_TRUST_MAX_RECORDS {
        Ok(())
    } else {
        Err(VerificationTrustError::RecordLimitExceeded {
            kind,
            limit: VERIFICATION_TRUST_MAX_RECORDS,
            actual,
        })
    }
}

fn ensure_evidence_binding(
    subject: &TrustedProofSubject,
    evidence: &TrustedEvidence,
) -> Result<(), VerificationTrustError> {
    if evidence.statement_digest() == subject.contract_digest {
        Ok(())
    } else {
        Err(VerificationTrustError::EvidenceStatementMismatch)
    }
}

struct CanonicalTranscript {
    bytes: Vec<u8>,
}

impl CanonicalTranscript {
    fn with_domain(domain: &[u8]) -> Self {
        Self {
            bytes: domain.to_vec(),
        }
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn string(&mut self, value: &str) -> Result<(), VerificationTrustError> {
        let len = u32::try_from(value.len()).map_err(|_| {
            VerificationTrustError::CanonicalLengthOverflow {
                kind: "string",
                actual: value.len(),
            }
        })?;
        self.u32(len);
        self.bytes.extend_from_slice(value.as_bytes());
        Ok(())
    }

    fn count(&mut self, count: usize) -> Result<(), VerificationTrustError> {
        let count =
            u32::try_from(count).map_err(|_| VerificationTrustError::CanonicalLengthOverflow {
                kind: "vector",
                actual: count,
            })?;
        self.u32(count);
        Ok(())
    }

    fn digest(&mut self, digest: BundleDigest) {
        self.bytes.extend_from_slice(&digest.as_bytes());
    }

    fn digest_value(self) -> BundleDigest {
        BundleDigest::of(&self.bytes)
    }
}

fn decode_lower_hex<const N: usize>(value: &str) -> Result<[u8; N], String> {
    if value.len() != N.saturating_mul(2) {
        return Err(format!(
            "expected {} lowercase hex characters",
            N.saturating_mul(2)
        ));
    }
    let bytes = value.as_bytes();
    let mut decoded = [0; N];
    for (index, slot) in decoded.iter_mut().enumerate() {
        let offset = index.saturating_mul(2);
        let high = decode_lower_hex_nibble(bytes[offset])?;
        let low = decode_lower_hex_nibble(bytes[offset + 1])?;
        *slot = (high << 4) | low;
    }
    Ok(decoded)
}

fn decode_lower_hex_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        _ => Err("value contains a non-lowercase-hex character".to_owned()),
    }
}

#[cfg(test)]
mod tests;
