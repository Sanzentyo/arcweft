use super::*;
use crate::release::signing_policy::{KeyEpochPolicy, VerificationTrustGenerationPolicy};
use ed25519_dalek::Signer as _;

const SIGNER_ID: &str = "release-key-main";
const KEY_EPOCH: u64 = 4;

#[test]
fn trust_reason_rejects_blank_and_preserves_valid_bytes() {
    assert!(matches!(
        TrustReasonWire::new(" \n\t"),
        Err(VerificationTrustError::InvalidText {
            kind: "trust reason",
            ..
        })
    ));
    let reason = TrustReasonWire::new("  signed build fact  ").expect("reason");
    assert_eq!(reason.as_str(), "  signed build fact  ");
}

#[test]
fn manifest_digest_and_json_are_independent_of_admission_order() {
    let mut manifest = sample_manifest();
    let first_digest = manifest.digest().expect("manifest digest");
    let first_json = artifact_with_placeholder_signature(manifest.clone())
        .to_json_bytes()
        .expect("canonical JSON");

    manifest.admissions.reverse();
    let second_digest = manifest.digest().expect("manifest digest");
    let second_json = artifact_with_placeholder_signature(manifest)
        .to_json_bytes()
        .expect("canonical JSON");

    assert_eq!(first_digest, second_digest);
    assert_eq!(first_json, second_json);
}

#[test]
fn canonical_digest_goldens() {
    let manifest = sample_manifest();
    let revocations = sample_revocations(vec![revoked(
        manifest.admissions[0].admission_id,
        "operator revoked",
    )]);
    assert_eq!(
        manifest.admissions[0].admission_id.to_string(),
        "3653ca8b3609f2aa9253ddb3cce2874de3eec34d8e8639ff3c4a7a23dad799f7"
    );
    assert_eq!(
        manifest.digest().expect("manifest digest").to_hex(),
        "cd255da2c203c95bfddf4e0b6ce17e3e4df990e4ff1d85a6fe94375a089068e4"
    );
    assert_eq!(
        revocations.digest().expect("revocations digest").to_hex(),
        "7f546f4eb98590d1f622137509cfb0d6c7bc6847b8c445f8f558ddfda3ab7ddb"
    );
}

#[test]
fn manifest_rejects_wrong_admission_id_and_duplicate_subject() {
    let mut wrong_id = sample_manifest();
    wrong_id.admissions[0].admission_id =
        TrustedProofAdmissionId::from_digest(BundleDigest::of(b"wrong"));
    assert!(matches!(
        wrong_id.validate(),
        Err(VerificationTrustError::AdmissionIdMismatch { .. })
    ));

    let mut duplicate = sample_manifest();
    duplicate.admissions.push(duplicate.admissions[0].clone());
    assert!(matches!(
        duplicate.validate(),
        Err(VerificationTrustError::DuplicateAdmissionId(_))
    ));

    let mut duplicate_subject = sample_manifest();
    let subject = duplicate_subject.admissions[0].subject.clone();
    let contract_digest = subject.contract_digest;
    duplicate_subject.admissions.push(
        TrustedProofAdmission::new(
            &duplicate_subject.policy_id,
            subject,
            TrustedEvidence::SignedBuildAttestation {
                producer_id: BuildAttestationProducerId::new("second-producer").expect("producer"),
                statement_digest: contract_digest,
                artifact_digest: BundleDigest::of(b"second artifact"),
            },
        )
        .expect("second admission"),
    );
    assert!(matches!(
        duplicate_subject.validate(),
        Err(VerificationTrustError::DuplicateAdmissionSubject(_))
    ));
}

#[test]
fn admission_rejects_evidence_for_another_contract() {
    let subject = sample_subject("proof", BundleDigest::of(b"contract"));
    let evidence = TrustedEvidence::ExplicitPolicyAdmission {
        authority_case_id: AuthorityCaseId::new("case-1").expect("case id"),
        statement_digest: BundleDigest::of(b"other contract"),
    };
    assert_eq!(
        TrustedProofAdmission::new(&sample_policy_id(), subject, evidence),
        Err(VerificationTrustError::EvidenceStatementMismatch)
    );
}

#[test]
fn all_evidence_variants_round_trip_with_exact_typed_fields() {
    let statement_digest = BundleDigest::of(b"statement");
    let evidence = [
        TrustedEvidence::SignedBuildAttestation {
            producer_id: BuildAttestationProducerId::new("ci.builder").expect("producer"),
            statement_digest,
            artifact_digest: BundleDigest::of(b"artifact"),
        },
        TrustedEvidence::SignedHostFact {
            host_contract_id: HostFactContractId::new("host.clock").expect("host contract"),
            statement_digest,
            manifest_digest: BundleDigest::of(b"manifest"),
        },
        TrustedEvidence::ExternalProofCertificate {
            verifier_id: ExternalVerifierId::new("proof-service").expect("verifier"),
            statement_digest,
            certificate_digest: BundleDigest::of(b"certificate"),
        },
        TrustedEvidence::ExplicitPolicyAdmission {
            authority_case_id: AuthorityCaseId::new("case-42").expect("case id"),
            statement_digest,
        },
    ];

    for value in evidence {
        let encoded = serde_json::to_vec(&value).expect("encode evidence");
        let decoded = serde_json::from_slice::<TrustedEvidence>(&encoded).expect("decode evidence");
        assert_eq!(decoded, value);
        assert_eq!(decoded.statement_digest(), statement_digest);
    }
}

#[test]
fn revocation_digest_is_order_independent_and_duplicates_reject() {
    let first = TrustedProofAdmissionId::from_digest(BundleDigest::of(b"first"));
    let second = TrustedProofAdmissionId::from_digest(BundleDigest::of(b"second"));
    let mut revocations = sample_revocations(vec![
        revoked(first, "withdrawn"),
        revoked(second, "superseded"),
    ]);
    let digest = revocations.digest().expect("revocation digest");
    revocations.revoked.reverse();
    assert_eq!(revocations.digest().expect("reordered digest"), digest);

    revocations.revoked.push(revoked(first, "again"));
    assert!(matches!(
        revocations.validate(),
        Err(VerificationTrustError::DuplicateRevocationId(id)) if id == first
    ));
}

#[test]
fn strict_json_rejects_unknown_and_duplicate_fields() {
    let artifact = artifact_with_placeholder_signature(sample_manifest());
    let mut value = serde_json::to_value(&artifact).expect("artifact value");
    value
        .as_object_mut()
        .expect("artifact object")
        .insert("unknown".to_owned(), serde_json::Value::Bool(true));
    let bytes = serde_json::to_vec(&value).expect("artifact JSON");
    assert!(matches!(
        VerificationTrustArtifact::from_json_slice(&bytes),
        Err(VerificationTrustError::DecodeJson(_))
    ));

    let duplicate = br#"{
        "manifest": null,
        "manifest": null,
        "signature": null
    }"#;
    assert!(matches!(
        VerificationTrustArtifact::from_json_slice(duplicate),
        Err(VerificationTrustError::DecodeJson(_))
    ));
}

#[test]
fn artifact_byte_limit_is_checked_before_deserialization() {
    let oversized = vec![b' '; VERIFICATION_TRUST_ARTIFACT_MAX_BYTES + 1];
    assert_eq!(
        VerificationTrustArtifact::from_json_slice(&oversized),
        Err(VerificationTrustError::ByteLimitExceeded {
            kind: "verification trust artifact",
            limit: VERIFICATION_TRUST_ARTIFACT_MAX_BYTES,
            actual: VERIFICATION_TRUST_ARTIFACT_MAX_BYTES + 1,
        })
    );
    assert_eq!(
        VerificationTrustRevocationArtifact::from_json_slice(&oversized),
        Err(VerificationTrustError::ByteLimitExceeded {
            kind: "verification trust revocation artifact",
            limit: VERIFICATION_TRUST_ARTIFACT_MAX_BYTES,
            actual: VERIFICATION_TRUST_ARTIFACT_MAX_BYTES + 1,
        })
    );

    let at_artifact_limit = vec![b' '; VERIFICATION_TRUST_ARTIFACT_MAX_BYTES];
    assert!(matches!(
        VerificationTrustArtifact::from_json_slice(&at_artifact_limit),
        Err(VerificationTrustError::DecodeJson(_))
    ));
    assert!(matches!(
        VerificationTrustRevocationArtifact::from_json_slice(&at_artifact_limit),
        Err(VerificationTrustError::DecodeJson(_))
    ));

    let oversized_authority = vec![b' '; VERIFICATION_TRUST_AUTHORITY_MAX_BYTES + 1];
    assert_eq!(
        ValidatedVerificationTrustAuthority::from_json_slice(&oversized_authority),
        Err(VerificationTrustError::ByteLimitExceeded {
            kind: "verification trust authority document",
            limit: VERIFICATION_TRUST_AUTHORITY_MAX_BYTES,
            actual: VERIFICATION_TRUST_AUTHORITY_MAX_BYTES + 1,
        })
    );
}

#[test]
fn record_limits_are_checked_before_duplicate_processing() {
    let admission = sample_admission("bounded-proof");
    let mut manifest = sample_manifest();
    manifest.admissions = vec![admission; VERIFICATION_TRUST_MAX_RECORDS + 1];
    assert_eq!(
        manifest.validate(),
        Err(VerificationTrustError::RecordLimitExceeded {
            kind: "admissions",
            limit: VERIFICATION_TRUST_MAX_RECORDS,
            actual: VERIFICATION_TRUST_MAX_RECORDS + 1,
        })
    );

    let record = revoked(
        TrustedProofAdmissionId::from_digest(BundleDigest::of(b"bounded-revocation")),
        "bounded",
    );
    let revocations = sample_revocations(vec![record; VERIFICATION_TRUST_MAX_RECORDS + 1]);
    assert_eq!(
        revocations.validate(),
        Err(VerificationTrustError::RecordLimitExceeded {
            kind: "revocations",
            limit: VERIFICATION_TRUST_MAX_RECORDS,
            actual: VERIFICATION_TRUST_MAX_RECORDS + 1,
        })
    );
}

#[test]
fn valid_signed_authority_is_accepted_and_retains_revocations() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let manifest = sample_manifest();
    let revoked_id = manifest.admissions[0].admission_id;
    let document = signed_authority(
        &signing_key,
        manifest,
        sample_revocations(vec![revoked(revoked_id, "operator revoked")]),
    );

    let validated =
        ValidatedVerificationTrustAuthority::try_from_document(document).expect("valid authority");
    assert!(validated.is_revoked(revoked_id));
    assert_ne!(validated.manifest_digest(), BundleDigest::ZERO);
    assert_ne!(validated.revocations_digest(), BundleDigest::ZERO);
}

#[test]
fn trust_artifact_signatures_are_required_independently_of_awfb_policy() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut document = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    document.signature_policy.require_awfb_signature = false;

    ValidatedVerificationTrustAuthority::try_from_document(document)
        .expect("trust artifacts use the shared key policy independently of AWFB");
}

#[test]
fn signed_authority_json_round_trip_is_strict() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let document = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    let bytes = serde_json::to_vec(&document).expect("authority JSON");
    let validated =
        ValidatedVerificationTrustAuthority::from_json_slice(&bytes).expect("valid authority JSON");
    assert_eq!(validated.document(), &document);
    assert_eq!(
        serde_json::to_vec(validated.document()).expect("re-encoded authority JSON"),
        bytes
    );

    let mut unknown = serde_json::to_value(&document).expect("authority value");
    unknown
        .as_object_mut()
        .expect("authority object")
        .insert("unknown".to_owned(), serde_json::Value::Bool(true));
    let bytes = serde_json::to_vec(&unknown).expect("unknown-field JSON");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
        Err(VerificationTrustError::DecodeJson(_))
    ));

    let mut missing_policy_field = serde_json::to_value(&document).expect("authority value");
    missing_policy_field["signing_policy"]
        .as_object_mut()
        .expect("signing policy")
        .remove("allow_unsigned_local_artifacts");
    let bytes = serde_json::to_vec(&missing_policy_field).expect("missing-policy-field JSON");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
        Err(VerificationTrustError::DecodeJson(_))
    ));

    let mut missing_signature_policy_field =
        serde_json::to_value(&document).expect("authority value");
    missing_signature_policy_field["signature_policy"]
        .as_object_mut()
        .expect("signature policy")
        .remove("allowed_algorithms");
    let bytes =
        serde_json::to_vec(&missing_signature_policy_field).expect("missing policy field JSON");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
        Err(VerificationTrustError::DecodeJson(_))
    ));

    let mut missing_signature = serde_json::to_value(&document).expect("authority value");
    missing_signature["trust_manifest"]
        .as_object_mut()
        .expect("trust artifact")
        .remove("signature");
    let bytes = serde_json::to_vec(&missing_signature).expect("missing-signature JSON");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
        Err(VerificationTrustError::DecodeJson(_))
    ));
}

#[test]
fn authority_json_requires_all_top_level_fields_and_both_signatures() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let document = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    let value = serde_json::to_value(document).expect("authority value");

    for field in [
        "schema_version",
        "signing_policy",
        "signature_policy",
        "trust_manifest",
        "revocations",
    ] {
        let mut missing = value.clone();
        missing
            .as_object_mut()
            .expect("authority object")
            .remove(field);
        let bytes = serde_json::to_vec(&missing).expect("missing-field JSON");
        assert!(
            matches!(
                ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
                Err(VerificationTrustError::DecodeJson(_))
            ),
            "authority field `{field}` must be required"
        );
    }

    for artifact in ["trust_manifest", "revocations"] {
        let mut missing = value.clone();
        missing[artifact]
            .as_object_mut()
            .expect("signed artifact object")
            .remove("signature");
        let bytes = serde_json::to_vec(&missing).expect("missing-signature JSON");
        assert!(
            matches!(
                ValidatedVerificationTrustAuthority::from_json_slice(&bytes),
                Err(VerificationTrustError::DecodeJson(_))
            ),
            "authority artifact `{artifact}` must carry a signature"
        );
    }
}

#[test]
fn canonical_artifact_json_round_trip_preserves_bytes_and_digests() {
    let manifest = sample_manifest();
    let mut reversed_manifest = manifest.clone();
    reversed_manifest.admissions.reverse();
    let artifact = artifact_with_placeholder_signature(reversed_manifest);
    let canonical_json = artifact.to_json_bytes().expect("canonical trust JSON");
    let decoded = VerificationTrustArtifact::from_json_slice(&canonical_json).expect("trust JSON");
    assert_eq!(
        decoded.to_json_bytes().expect("re-encoded trust JSON"),
        canonical_json
    );
    assert_eq!(
        decoded.manifest.digest().expect("decoded trust digest"),
        manifest.digest().expect("original trust digest")
    );

    let first_id = manifest.admissions[0].admission_id;
    let second_id = manifest.admissions[1].admission_id;
    let revocations = sample_revocations(vec![
        revoked(second_id, "second"),
        revoked(first_id, "first"),
    ]);
    let expected_digest = revocations.digest().expect("original revocation digest");
    let artifact = revocation_artifact_with_placeholder_signature(revocations);
    let canonical_json = artifact.to_json_bytes().expect("canonical revocation JSON");
    let decoded = VerificationTrustRevocationArtifact::from_json_slice(&canonical_json)
        .expect("revocation JSON");
    assert_eq!(
        decoded.to_json_bytes().expect("re-encoded revocation JSON"),
        canonical_json
    );
    assert_eq!(
        decoded
            .manifest
            .digest()
            .expect("decoded revocation digest"),
        expected_digest
    );
}

#[test]
fn signed_authority_rejects_manifest_and_transcript_digest_tampering() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);

    let mut manifest_digest = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    manifest_digest.trust_manifest.signature.manifest_digest = BundleDigest::of(b"tampered");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(manifest_digest),
        Err(VerificationTrustError::ManifestDigestMismatch { .. })
    ));

    let mut manifest_transcript = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    manifest_transcript.trust_manifest.signature.signing_digest = BundleDigest::of(b"tampered");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(manifest_transcript),
        Err(VerificationTrustError::SigningDigestMismatch { .. })
    ));

    let mut revocation_digest = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    revocation_digest.revocations.signature.manifest_digest = BundleDigest::of(b"tampered");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(revocation_digest),
        Err(VerificationTrustError::ManifestDigestMismatch { .. })
    ));

    let mut revocation_transcript = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    revocation_transcript.revocations.signature.signing_digest = BundleDigest::of(b"tampered");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(revocation_transcript),
        Err(VerificationTrustError::SigningDigestMismatch { .. })
    ));
}

#[test]
fn signed_authority_rejects_stale_generations_and_wrong_channel() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);

    let mut stale = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    stale
        .signing_policy
        .verification_trust
        .minimum_policy_generation = 8;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(stale),
        Err(VerificationTrustError::StalePolicyGeneration {
            actual: 7,
            minimum: 8,
        })
    );

    let mut wrong_channel = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    wrong_channel.revocations.manifest.channel = ReleaseChannel::new("beta").expect("channel");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(wrong_channel),
        Err(VerificationTrustError::ChannelMismatch { .. })
    ));
}

#[test]
fn signed_authority_rejects_stale_revocations_and_an_older_signed_pair_replay() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);

    let mut stale_revocation = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    stale_revocation
        .signing_policy
        .verification_trust
        .minimum_revocation_generation = 13;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(stale_revocation),
        Err(VerificationTrustError::StaleRevocationGeneration {
            actual: 12,
            minimum: 13,
        })
    );

    let mut old_manifest = sample_manifest();
    old_manifest.generation = 6;
    let mut old_revocations = sample_revocations(Vec::new());
    old_revocations.generation = 11;
    let replay = signed_authority(&signing_key, old_manifest, old_revocations);

    let mut valid_at_old_snapshot = replay.clone();
    valid_at_old_snapshot.signing_policy.verification_trust = VerificationTrustGenerationPolicy {
        minimum_policy_generation: 6,
        minimum_revocation_generation: 11,
    };
    ValidatedVerificationTrustAuthority::try_from_document(valid_at_old_snapshot)
        .expect("the older pair has valid signatures at its original generation floors");

    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(replay),
        Err(VerificationTrustError::StalePolicyGeneration {
            actual: 6,
            minimum: 7,
        })
    );
}

#[test]
fn signed_authority_rejects_wrong_algorithm_untrusted_signer_and_bad_signature() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);

    let mut wrong_algorithm = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    wrong_algorithm.trust_manifest.signature.algorithm = "test-algorithm".to_owned();
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(wrong_algorithm),
        Err(VerificationTrustError::InvalidSignature(_))
    ));

    let mut untrusted = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    let manifest_digest = untrusted
        .trust_manifest
        .manifest
        .digest()
        .expect("manifest digest");
    untrusted.trust_manifest.signature = signed_signature_for(
        &signing_key,
        "untrusted-signer",
        SigningSubjectKind::VerificationTrustManifest,
        &untrusted.trust_manifest.manifest.channel,
        manifest_digest,
    );
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(untrusted),
        Err(VerificationTrustError::MissingTrustedPublicKey { signer_id, .. })
            if signer_id == "untrusted-signer"
    ));

    let mut bad_signature = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    bad_signature
        .trust_manifest
        .signature
        .signature
        .replace_range(0..2, "01");
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(bad_signature),
        Err(VerificationTrustError::SignatureVerificationFailed { .. })
    ));
}

#[test]
fn signed_authority_rejects_revoked_or_wrong_epoch_key() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut revoked_key = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    revoked_key.signature_policy.trusted_public_keys[0].revoked = true;
    assert!(matches!(
        ValidatedVerificationTrustAuthority::try_from_document(revoked_key),
        Err(VerificationTrustError::NoValidTrustedPublicKey { .. })
    ));

    let mut wrong_epoch = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    wrong_epoch.trust_manifest.signature.key_epoch = 5;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(wrong_epoch),
        Err(VerificationTrustError::KeyEpochRejected { epoch: 5 })
    );
}

#[test]
fn signing_policy_key_epoch_window_is_inclusive_min_exclusive_max() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let at_minimum = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    ValidatedVerificationTrustAuthority::try_from_document(at_minimum)
        .expect("the inclusive minimum key epoch is accepted");

    for rejected_epoch in [KEY_EPOCH - 1, KEY_EPOCH + 1, KEY_EPOCH + 2] {
        let mut outside = signed_authority(
            &signing_key,
            sample_manifest(),
            sample_revocations(Vec::new()),
        );
        outside.trust_manifest.signature.key_epoch = rejected_epoch;
        assert_eq!(
            ValidatedVerificationTrustAuthority::try_from_document(outside),
            Err(VerificationTrustError::KeyEpochRejected {
                epoch: rejected_epoch,
            })
        );
    }
}

#[test]
fn authority_rejects_revocation_policy_id_mismatch() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut document = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    document.revocations.manifest.policy_id =
        VerificationTrustPolicyId::new("release-security/other-policy")
            .expect("different policy id");
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(document),
        Err(VerificationTrustError::RevocationPolicyMismatch)
    );
}

#[test]
fn authority_rejects_nested_manifest_and_signature_schema_mismatches() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);

    let mut manifest = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    manifest.trust_manifest.manifest.schema_version = 2;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(manifest),
        Err(VerificationTrustError::UnsupportedSchema {
            kind: "verification trust manifest",
            actual: 2,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    );

    let mut manifest_signature = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    manifest_signature.trust_manifest.signature.schema_version = 2;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(manifest_signature),
        Err(VerificationTrustError::UnsupportedSchema {
            kind: "verification trust signature",
            actual: 2,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    );

    let mut revocations = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    revocations.revocations.manifest.schema_version = 2;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(revocations),
        Err(VerificationTrustError::UnsupportedSchema {
            kind: "verification trust revocations",
            actual: 2,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    );

    let mut revocation_signature = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    revocation_signature.revocations.signature.schema_version = 2;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(revocation_signature),
        Err(VerificationTrustError::UnsupportedSchema {
            kind: "verification trust signature",
            actual: 2,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    );
}

#[test]
fn authority_rejects_unsupported_schema_before_signature_work() {
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&[7; 32]);
    let mut document = signed_authority(
        &signing_key,
        sample_manifest(),
        sample_revocations(Vec::new()),
    );
    document.schema_version = 2;
    assert_eq!(
        ValidatedVerificationTrustAuthority::try_from_document(document),
        Err(VerificationTrustError::UnsupportedSchema {
            kind: "verification trust authority",
            actual: 2,
            expected: VERIFICATION_TRUST_SCHEMA_VERSION,
        })
    );
}

fn sample_policy_id() -> VerificationTrustPolicyId {
    VerificationTrustPolicyId::new("release-security/resource-manifest-v1").expect("policy id")
}

fn sample_package() -> VerificationPackageId {
    VerificationPackageId::new("opening-game").expect("package")
}

fn sample_subject(name: &str, contract_digest: BundleDigest) -> TrustedProofSubject {
    TrustedProofSubject {
        declaration: ProofDeclarationId::new(
            sample_package(),
            CanonicalModulePathWire::new("crate.resources").expect("module"),
            ProofName::new(name).expect("proof name"),
        ),
        contract_digest,
        reason: TrustReasonWire::new(format!("external evidence for {name}")).expect("reason"),
    }
}

fn sample_admission(name: &str) -> TrustedProofAdmission {
    let contract_digest = BundleDigest::of(name.as_bytes());
    TrustedProofAdmission::new(
        &sample_policy_id(),
        sample_subject(name, contract_digest),
        TrustedEvidence::ExplicitPolicyAdmission {
            authority_case_id: AuthorityCaseId::new(format!("case-{name}"))
                .expect("authority case"),
            statement_digest: contract_digest,
        },
    )
    .expect("admission")
}

fn sample_manifest() -> VerificationTrustManifest {
    VerificationTrustManifest {
        schema_version: VERIFICATION_TRUST_SCHEMA_VERSION,
        policy_id: sample_policy_id(),
        generation: 7,
        channel: ReleaseChannel::new("stable").expect("channel"),
        package: sample_package(),
        profile: VerificationProfileId::new("release").expect("profile"),
        admissions: vec![
            sample_admission("resource_manifest_hashes"),
            sample_admission("host_fact"),
        ],
    }
}

fn revoked(admission_id: TrustedProofAdmissionId, reason: &str) -> RevokedTrustedProofAdmission {
    RevokedTrustedProofAdmission {
        admission_id,
        reason: RevocationReason::new(reason).expect("revocation reason"),
    }
}

fn sample_revocations(revoked: Vec<RevokedTrustedProofAdmission>) -> VerificationTrustRevocations {
    VerificationTrustRevocations {
        schema_version: VERIFICATION_TRUST_SCHEMA_VERSION,
        policy_id: sample_policy_id(),
        generation: 12,
        channel: ReleaseChannel::new("stable").expect("channel"),
        revoked,
    }
}

fn artifact_with_placeholder_signature(
    manifest: VerificationTrustManifest,
) -> VerificationTrustArtifact {
    let manifest_digest = manifest.digest().expect("manifest digest");
    let signature = placeholder_signature(
        SigningSubjectKind::VerificationTrustManifest,
        &manifest.channel,
        manifest_digest,
    );
    VerificationTrustArtifact {
        manifest,
        signature,
    }
}

fn revocation_artifact_with_placeholder_signature(
    manifest: VerificationTrustRevocations,
) -> VerificationTrustRevocationArtifact {
    let manifest_digest = manifest.digest().expect("revocation digest");
    let signature = placeholder_signature(
        SigningSubjectKind::VerificationTrustRevocations,
        &manifest.channel,
        manifest_digest,
    );
    VerificationTrustRevocationArtifact {
        manifest,
        signature,
    }
}

fn placeholder_signature(
    subject: SigningSubjectKind,
    channel: &ReleaseChannel,
    manifest_digest: BundleDigest,
) -> VerificationTrustSignature {
    let transcript = trust_transcript(subject, channel, manifest_digest);
    VerificationTrustSignature {
        schema_version: VERIFICATION_TRUST_SCHEMA_VERSION,
        signer_id: SIGNER_ID.to_owned(),
        algorithm: RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
        key_epoch: KEY_EPOCH,
        manifest_digest,
        signing_digest: transcript.digest().expect("signing digest"),
        signature: "00".repeat(64),
    }
}

fn signed_authority(
    signing_key: &ed25519_dalek::SigningKey,
    manifest: VerificationTrustManifest,
    revocations: VerificationTrustRevocations,
) -> VerificationTrustAuthorityDocument {
    let channel = ReleaseChannel::new("stable").expect("channel");
    let key_epoch = KeyEpochPolicy {
        min: KEY_EPOCH,
        max: Some(KEY_EPOCH + 1),
    };
    let verification_trust = VerificationTrustGenerationPolicy {
        minimum_policy_generation: 7,
        minimum_revocation_generation: 12,
    };
    let signing_policy = SigningPolicy::release_consume(channel, key_epoch, verification_trust);
    let trusted_key = ReleaseTrustedPublicKey::ed25519_v1(
        SIGNER_ID,
        lower_hex(&signing_key.verifying_key().to_bytes()),
    )
    .expect("trusted key")
    .with_key_epoch_validity(KEY_EPOCH, Some(KEY_EPOCH + 1))
    .expect("key epoch");
    let signature_policy =
        ReleaseSignaturePolicy::require_trusted_public_keys(Some(64), [trusted_key])
            .expect("signature policy");

    let manifest_digest = manifest.digest().expect("manifest digest");
    let trust_manifest = VerificationTrustArtifact {
        signature: signed_signature(
            signing_key,
            SigningSubjectKind::VerificationTrustManifest,
            &manifest.channel,
            manifest_digest,
        ),
        manifest,
    };
    let revocations_digest = revocations.digest().expect("revocations digest");
    let revocations = VerificationTrustRevocationArtifact {
        signature: signed_signature(
            signing_key,
            SigningSubjectKind::VerificationTrustRevocations,
            &revocations.channel,
            revocations_digest,
        ),
        manifest: revocations,
    };
    VerificationTrustAuthorityDocument {
        schema_version: VERIFICATION_TRUST_SCHEMA_VERSION,
        signing_policy,
        signature_policy,
        trust_manifest,
        revocations,
    }
}

fn signed_signature(
    signing_key: &ed25519_dalek::SigningKey,
    subject: SigningSubjectKind,
    channel: &ReleaseChannel,
    manifest_digest: BundleDigest,
) -> VerificationTrustSignature {
    signed_signature_for(signing_key, SIGNER_ID, subject, channel, manifest_digest)
}

fn signed_signature_for(
    signing_key: &ed25519_dalek::SigningKey,
    signer_id: &str,
    subject: SigningSubjectKind,
    channel: &ReleaseChannel,
    manifest_digest: BundleDigest,
) -> VerificationTrustSignature {
    let transcript = trust_transcript_for(subject, signer_id, channel, manifest_digest);
    let signing_digest = transcript.digest().expect("signing digest");
    let signature = signing_key.sign(&signing_digest.as_bytes());
    VerificationTrustSignature {
        schema_version: VERIFICATION_TRUST_SCHEMA_VERSION,
        signer_id: signer_id.to_owned(),
        algorithm: RELEASE_SIGNATURE_ALGORITHM_ED25519_V1.to_owned(),
        key_epoch: KEY_EPOCH,
        manifest_digest,
        signing_digest,
        signature: lower_hex(&signature.to_bytes()),
    }
}

fn trust_transcript(
    subject: SigningSubjectKind,
    channel: &ReleaseChannel,
    manifest_digest: BundleDigest,
) -> SigningDigestTranscript {
    trust_transcript_for(subject, SIGNER_ID, channel, manifest_digest)
}

fn trust_transcript_for(
    subject: SigningSubjectKind,
    signer_id: &str,
    channel: &ReleaseChannel,
    manifest_digest: BundleDigest,
) -> SigningDigestTranscript {
    match subject {
        SigningSubjectKind::VerificationTrustManifest => {
            SigningDigestTranscript::verification_trust_manifest(
                manifest_digest,
                signer_id,
                channel.clone(),
                KEY_EPOCH,
            )
        }
        SigningSubjectKind::VerificationTrustRevocations => {
            SigningDigestTranscript::verification_trust_revocations(
                manifest_digest,
                signer_id,
                channel.clone(),
                KEY_EPOCH,
            )
        }
        _ => panic!("test helper only accepts verification trust subjects"),
    }
    .expect("trust transcript")
}

fn lower_hex(bytes: &[u8]) -> String {
    bytes.iter().fold(
        String::with_capacity(bytes.len().saturating_mul(2)),
        |mut output, byte| {
            use std::fmt::Write as _;
            write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
            output
        },
    )
}
