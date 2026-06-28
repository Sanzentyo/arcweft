#[path = "support/cli.rs"]
mod cli;
#[path = "support/release_trust_fixture.rs"]
mod release_trust_fixture;

use cli::CommandOutput;
use release_trust_fixture::{
    CHANNEL, KEY_EPOCH, ReleaseTrustCase, build_release_trust_fixture, cleanup_fixture,
};
use serde_json::Value;

#[test]
fn release_verify_json_success_reports_machine_readable_evidence() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessCacheHit);
    let output = run_verify(&fixture, CHANNEL, "required-residency");
    output.assert_success();
    let json = parse_stdout_json(&output);

    assert_eq!(json["success"], Value::Bool(true));
    assert!(contains_code(&json, "base_signature_valid"));
    assert!(contains_code(&json, "patch_signature_valid"));
    assert!(contains_code(&json, "external_payload_verified"));
    cleanup_fixture(&fixture);
}

#[test]
fn release_verify_json_failure_still_prints_typed_evidence() {
    let cases = [
        (
            ReleaseTrustCase::MissingPatchSignature,
            "missing_patch_signature",
        ),
        (
            ReleaseTrustCase::ExternalPayloadMissing,
            "external_payload_missing",
        ),
        (
            ReleaseTrustCase::DetachedSignatureTranscriptMismatch,
            "detached_signature_transcript_mismatch",
        ),
    ];

    for (case, expected_code) in cases {
        let fixture = build_release_trust_fixture(case);
        let output = run_verify(&fixture, CHANNEL, "required-residency");
        output.assert_failure();
        let json = parse_stdout_json(&output);
        assert_eq!(json["success"], Value::Bool(false));
        assert!(
            contains_code(&json, expected_code),
            "missing {expected_code} in {json:#}"
        );
        cleanup_fixture(&fixture);
    }
}

#[test]
fn release_verify_json_wrong_policy_is_failure_for_same_archive() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessCacheHit);
    let output = run_verify(&fixture, "seq02-9-wrong-channel", "required-residency");
    output.assert_failure();
    let json = parse_stdout_json(&output);
    assert_eq!(json["success"], Value::Bool(false));
    assert!(contains_code(&json, "wrong_signing_policy"));
    cleanup_fixture(&fixture);
}

fn run_verify(
    fixture: &release_trust_fixture::BuiltReleaseTrustFixture,
    channel: &str,
    payload_mode: &str,
) -> CommandOutput {
    CommandOutput::run([
        "release".to_owned(),
        "verify".to_owned(),
        "--archive".to_owned(),
        fixture.archive_path.display().to_string(),
        "--cache-root".to_owned(),
        fixture.cache_root.display().to_string(),
        "--policy".to_owned(),
        "release-consume".to_owned(),
        "--channel".to_owned(),
        channel.to_owned(),
        "--key-epoch-min".to_owned(),
        KEY_EPOCH.to_string(),
        "--key-epoch-max".to_owned(),
        (KEY_EPOCH + 1).to_string(),
        "--payload-mode".to_owned(),
        payload_mode.to_owned(),
        "--json".to_owned(),
    ])
    .expect("arcw release verify runs")
}

fn parse_stdout_json(output: &CommandOutput) -> Value {
    serde_json::from_str(&output.stdout()).unwrap_or_else(|error| {
        panic!(
            "stdout should be JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            output.stdout(),
            output.stderr()
        )
    })
}

fn contains_code(json: &Value, expected_code: &str) -> bool {
    json["trust"].as_array().is_some_and(|evidence| {
        evidence
            .iter()
            .any(|entry| entry["code"].as_str() == Some(expected_code))
    })
}
