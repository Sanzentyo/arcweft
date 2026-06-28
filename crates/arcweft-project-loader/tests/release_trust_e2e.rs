#[path = "support/release_trust_fixture.rs"]
mod release_trust_fixture;

use arcweft_bundle::release::archive::ExternalPayloadMaterializationMode;
use arcweft_project_loader::cache::external_payload::ExternalPayloadCacheFetchStatus;
use arcweft_project_loader::release_adapter::consume::verify_release_archive;
use release_trust_fixture::{
    ReleaseTrustCase, build_release_trust_fixture, cleanup_fixture, fixture_payload,
    release_consume_policy, replace_payload_mirror_with_http, spawn_http_payload_server,
    wrong_channel_policy,
};

#[test]
fn release_trust_success_cache_hit_verifies_full_graph() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessCacheHit);
    let report = verify_release_archive(
        &fixture.archive_path,
        &release_consume_policy(),
        &fixture.cache_root,
        ExternalPayloadMaterializationMode::RequiredResidency,
    )
    .expect("release verifies");

    assert!(report.success, "trust evidence: {:#?}", report.trust);
    assert!(report.payloads.iter().any(|payload| {
        payload
            .fetch_report
            .as_ref()
            .is_some_and(|fetch| fetch.status == ExternalPayloadCacheFetchStatus::CacheHit)
    }));
    assert!(
        report
            .trust
            .iter()
            .any(|evidence| evidence.code == "base_signature_valid")
    );
    assert!(
        report
            .trust
            .iter()
            .any(|evidence| evidence.code == "patch_signature_valid")
    );
    assert!(
        report
            .trust
            .iter()
            .any(|evidence| evidence.code == "materialized_target_digest_match")
    );
    cleanup_fixture(&fixture);
}

#[test]
fn release_trust_metadata_only_mode_allows_missing_payload_bytes() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessMetadataOnly);
    let report = verify_release_archive(
        &fixture.archive_path,
        &release_consume_policy(),
        &fixture.cache_root,
        ExternalPayloadMaterializationMode::MetadataOnly,
    )
    .expect("metadata-only verification reports");

    assert!(report.success, "trust evidence: {:#?}", report.trust);
    assert!(
        report
            .trust
            .iter()
            .any(|evidence| evidence.code == "external_payload_metadata_only")
    );
    cleanup_fixture(&fixture);
}

#[test]
fn release_trust_file_and_http_payload_mirrors_are_test_owned() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessFileMirror);
    let report = verify_release_archive(
        &fixture.archive_path,
        &release_consume_policy(),
        &fixture.cache_root,
        ExternalPayloadMaterializationMode::RequiredResidency,
    )
    .expect("file mirror verification reports");
    assert!(
        report.success,
        "file mirror trust evidence: {:#?}",
        report.trust
    );
    assert!(report.payloads.iter().any(|payload| {
        payload
            .fetch_report
            .as_ref()
            .is_some_and(|fetch| fetch.status == ExternalPayloadCacheFetchStatus::Fetched)
    }));
    cleanup_fixture(&fixture);

    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessFileMirror);
    let (uri, server) = spawn_http_payload_server(fixture_payload().to_vec());
    replace_payload_mirror_with_http(&fixture.archive_path, uri);
    let report = verify_release_archive(
        &fixture.archive_path,
        &release_consume_policy(),
        &fixture.cache_root,
        ExternalPayloadMaterializationMode::RequiredResidency,
    );
    server.join().expect("fixture HTTP server exits");
    let report = report.expect("HTTP mirror verification reports");
    assert!(
        report.success,
        "HTTP mirror trust evidence: {:#?}",
        report.trust
    );
    cleanup_fixture(&fixture);
}

#[test]
fn release_trust_failure_matrix_reports_typed_evidence() {
    let cases = [
        ReleaseTrustCase::MissingBaseSignature,
        ReleaseTrustCase::MissingPatchSignature,
        ReleaseTrustCase::PatchTargetIdentityMismatch,
        ReleaseTrustCase::MaterializedTargetDigestMismatch,
        ReleaseTrustCase::MissingTargetSignature,
        ReleaseTrustCase::ExternalPayloadDigestMismatch,
        ReleaseTrustCase::ExternalPayloadSizeMismatch,
        ReleaseTrustCase::ExternalPayloadMissing,
        ReleaseTrustCase::AwfrManifestTamper,
        ReleaseTrustCase::DetachedSignatureTranscriptMismatch,
    ];

    for case in cases {
        let fixture = build_release_trust_fixture(case);
        let expected = fixture
            .expected_code
            .expect("failure case has expected evidence code");
        let report = verify_release_archive(
            &fixture.archive_path,
            &release_consume_policy(),
            &fixture.cache_root,
            ExternalPayloadMaterializationMode::RequiredResidency,
        )
        .expect("failure is reported, not thrown");

        assert!(!report.success, "{case:?} should fail");
        assert!(
            report
                .trust
                .iter()
                .any(|evidence| evidence.code == expected),
            "{case:?} should contain evidence code {expected}; evidence: {:#?}",
            report.trust,
        );
        cleanup_fixture(&fixture);
    }
}

#[test]
fn release_trust_wrong_policy_fails_same_archive() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessCacheHit);
    let report = verify_release_archive(
        &fixture.archive_path,
        &wrong_channel_policy(),
        &fixture.cache_root,
        ExternalPayloadMaterializationMode::RequiredResidency,
    )
    .expect("wrong policy is a typed report");

    assert!(!report.success);
    assert!(
        report
            .trust
            .iter()
            .any(|evidence| evidence.code == "wrong_signing_policy")
    );
    cleanup_fixture(&fixture);
}
