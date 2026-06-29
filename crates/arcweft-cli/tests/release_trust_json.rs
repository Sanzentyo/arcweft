#[path = "support/cli.rs"]
mod cli;
#[path = "support/release_trust_fixture.rs"]
mod release_trust_fixture;

use cli::CommandOutput;
use release_trust_fixture::{
    CHANNEL, KEY_EPOCH, ReleaseTrustCase, build_release_trust_fixture, cleanup_fixture,
};
use serde_json::Value;
use std::path::Path;

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

#[test]
fn release_publish_remote_dry_run_json_does_not_write_objects() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessFileMirror);
    let remote_root = fixture.root.join("remote-dry-run");
    let output = CommandOutput::run(publish_args(&fixture.root, &remote_root, true, true))
        .expect("arcw release publish dry-run runs");
    output.assert_success();
    let json = parse_stdout_json(&output);

    assert_eq!(json["mode"], Value::String("dry_run".to_owned()));
    assert_eq!(json["success"], Value::Bool(true));
    assert_eq!(
        json["artifacts"]
            .as_array()
            .and_then(|artifacts| artifacts.last())
            .and_then(|artifact| artifact["kind"].as_str()),
        Some("awfr_archive")
    );
    assert!(
        !remote_root.exists(),
        "dry-run should not create the object-directory root"
    );
    cleanup_fixture(&fixture);
}

#[test]
fn release_publish_remote_json_then_release_verify_json_succeeds() {
    let fixture = build_release_trust_fixture(ReleaseTrustCase::SuccessFileMirror);
    let remote_root = fixture.root.join("remote-publish");
    let output = CommandOutput::run(publish_args(&fixture.root, &remote_root, false, true))
        .expect("arcw release publish runs");
    output.assert_success();
    let publish_json = parse_stdout_json(&output);
    assert_eq!(publish_json["success"], Value::Bool(true));

    let verify_output = CommandOutput::run([
        "release".to_owned(),
        "verify".to_owned(),
        "--archive".to_owned(),
        remote_root.join("game.awfr").display().to_string(),
        "--cache-root".to_owned(),
        fixture.cache_root.display().to_string(),
        "--policy".to_owned(),
        "release-consume".to_owned(),
        "--channel".to_owned(),
        CHANNEL.to_owned(),
        "--key-epoch-min".to_owned(),
        KEY_EPOCH.to_string(),
        "--key-epoch-max".to_owned(),
        (KEY_EPOCH + 1).to_string(),
        "--payload-mode".to_owned(),
        "required-residency".to_owned(),
        "--json".to_owned(),
    ])
    .expect("arcw release verify runs");
    verify_output.assert_success();
    let verify_json = parse_stdout_json(&verify_output);
    assert_eq!(verify_json["success"], Value::Bool(true));
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

fn publish_args(root: &Path, remote_root: &Path, dry_run: bool, json: bool) -> Vec<String> {
    let mut args = vec![
        "release".to_owned(),
        "publish".to_owned(),
        "--backend".to_owned(),
        "object-directory".to_owned(),
        "--destination-root".to_owned(),
        remote_root.display().to_string(),
        "--require-signature-artifact".to_owned(),
        "--artifact".to_owned(),
        artifact_arg("awfb", root, "artifacts/base.awfb"),
        "--artifact".to_owned(),
        artifact_arg("patch", root, "artifacts/patch.awfb"),
        "--artifact".to_owned(),
        artifact_arg("awfb", root, "artifacts/target.awfb"),
        "--artifact".to_owned(),
        artifact_arg("external_payload", root, "artifacts/payload.bin"),
        "--artifact".to_owned(),
        artifact_arg("signature", root, "game.awfr.sig"),
        "--artifact".to_owned(),
        artifact_arg("awfr", root, "game.awfr"),
    ];
    if dry_run {
        args.push("--dry-run".to_owned());
    }
    if json {
        args.push("--json".to_owned());
    }
    args
}

fn artifact_arg(kind: &str, root: &Path, relative: &str) -> String {
    format!("{kind}:{}:{relative}", root.join(relative).display())
}
