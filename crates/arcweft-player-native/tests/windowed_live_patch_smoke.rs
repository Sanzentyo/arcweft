mod support;

use arcweft_bundle::patch::PatchCompatibility;
use support::windowed_live_patch_fixtures::{
    ImageProbeSnapshot, SmokeReport, all_smoke_reports, build_windowed_live_patch_fixtures,
    content_only_smoke_report, generated_fixture_files, restart_required_smoke_report,
    summarize_report, wrong_base_smoke_report,
};

#[test]
fn generated_awfb_patch_fixtures_have_expected_compatibility_classes() {
    let fixtures = build_windowed_live_patch_fixtures();
    support::windowed_live_patch_fixtures::assert_fixture_compatibility(&fixtures);

    assert_ne!(
        fixtures.base.content_root,
        fixtures.content_target.content_root
    );
    assert_ne!(
        fixtures.base.content_root,
        fixtures.code_generational_target.content_root
    );
    assert_ne!(
        fixtures.base.content_root,
        fixtures.restart_required_target.content_root
    );
    assert_ne!(fixtures.base.content_root, fixtures.wrong_base.content_root);
    assert_eq!(
        fixtures.content_patch.base_content_root,
        fixtures.base.content_root
    );
    assert_eq!(
        fixtures.content_patch.target_content_root,
        fixtures.content_target.content_root
    );
    assert_eq!(
        fixtures.content_patch.compatibility,
        PatchCompatibility::ContentOnly
    );
    assert_eq!(
        fixtures.code_generational_patch.compatibility,
        PatchCompatibility::CodeGenerational
    );
    assert_eq!(
        fixtures.restart_required_patch.compatibility,
        PatchCompatibility::RestartRequired
    );
}

#[test]
fn content_only_patch_refreshes_catalog_and_preserves_window_renderer_input_clock_shells() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report = content_only_smoke_report(&fixtures).expect("content-only smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "applied", Some("content-only"));
    assert_eq!(
        image_probe_rgba(&report.before.direct_image_probe),
        Some(&[255, 0, 0, 255][..])
    );
    assert_eq!(
        image_probe_rgba(&report.after_commit.direct_image_probe),
        Some(&[0, 0, 255, 255][..])
    );
    assert_eq!(
        report
            .after_observe
            .as_ref()
            .and_then(|snapshot| snapshot.runtime.presentation_text.as_deref()),
        Some("Windowed smoke: content target")
    );
}

#[test]
fn code_generational_patch_commits_new_generation_and_keeps_old_foreground_state() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report = support::windowed_live_patch_fixtures::code_generational_smoke_report(&fixtures)
        .expect("code-generational smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "applied", Some("code-generational"));
    assert_ne!(
        report.before.runtime.active_generation,
        report.after_commit.runtime.active_generation
    );
    assert_eq!(
        report.after_commit.runtime.current_fiber_generation,
        Some(report.before.runtime.active_generation)
    );
    assert!(report.after_commit.runtime.retired_generation_count >= 1);
    let after_observe = report
        .after_observe
        .as_ref()
        .expect("new entry snapshot is recorded");
    assert_eq!(
        after_observe.runtime.active_generation,
        report.after_commit.runtime.active_generation
    );
    assert_eq!(after_observe.runtime.current_fiber_generation, None);
    assert_eq!(after_observe.runtime.last_step_finished, Some(true));
    assert!(report.observations.iter().any(|observation| {
        observation.key == "new_foreground_generation"
            && observation.value == report.after_commit.runtime.active_generation.to_string()
    }));
}

#[test]
fn code_generational_patch_keeps_pending_task_generation_until_task_completion() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report =
        support::windowed_live_patch_fixtures::code_generational_task_smoke_report(&fixtures)
            .expect("code-generational task smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "applied", Some("code-generational"));
    assert!(report.observations.iter().any(|observation| {
        observation.key == "task_generation_after_commit"
            && observation.value == report.before.runtime.active_generation.to_string()
    }));
    assert!(report.observations.iter().any(|observation| {
        observation.key == "task_generation_after_completion" && observation.value == "none"
    }));
}

#[test]
fn restart_required_patch_restarts_session_but_preserves_shell_state() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report = restart_required_smoke_report(&fixtures).expect("restart-required smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "restarted", Some("restart-required"));
    assert_ne!(
        report.before.runtime.active_content_root,
        report.after_commit.runtime.active_content_root
    );
    assert_eq!(
        report.outcomes[0].content_root,
        report.after_commit.runtime.active_content_root
    );
}

#[test]
fn wrong_base_patch_rejects_without_mutating_active_session_or_catalog() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report = wrong_base_smoke_report(&fixtures).expect("wrong-base smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "rejected", None);
    assert_eq!(
        report.before.runtime.active_content_root,
        report.after_commit.runtime.active_content_root
    );
    assert_eq!(
        report.before.direct_image_probe,
        report.after_commit.direct_image_probe
    );
    assert_eq!(report.after_commit.patch_report.state, "rejected");
}

#[test]
fn malformed_patch_rejects_without_mutating_active_session_or_catalog() {
    let fixtures = build_windowed_live_patch_fixtures();
    let report = support::windowed_live_patch_fixtures::malformed_smoke_report(&fixtures)
        .expect("malformed smoke passes");

    assert_shell_preserved(&report);
    assert_single_outcome(&report, "rejected", None);
    assert_eq!(
        report.before.runtime.active_content_root,
        report.after_commit.runtime.active_content_root
    );
    assert_eq!(
        report.before.direct_image_probe,
        report.after_commit.direct_image_probe
    );
    assert_eq!(report.after_commit.patch_report.state, "rejected");
}

#[test]
fn fixture_regeneration_set_contains_all_binary_patches_and_reports() {
    let fixtures = build_windowed_live_patch_fixtures();
    let reports = all_smoke_reports(&fixtures).expect("reports generate");
    let files = generated_fixture_files(&fixtures, &reports).expect("generated files serialize");

    for required in [
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/base.awfb",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/content_target.awfb",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/patches/base_to_content.awfb.patch",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/patches/base_to_code_generational.awfb.patch",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/patches/base_to_restart_required.awfb.patch",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/patches/wrong_base_to_content.awfb.patch",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/patches/malformed.awfb.patch",
        "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated/reports/fixture-manifest.json",
    ] {
        assert!(
            files.iter().any(|file| file.relative_path == required),
            "missing generated fixture file {required}"
        );
    }
    assert!(reports.iter().all(|report| {
        let summary = summarize_report(report);
        summary.contains("shell_preserved=true")
    }));
}

fn assert_shell_preserved(report: &SmokeReport) {
    assert!(
        report
            .before
            .shell
            .has_same_shell_identities(&report.after_commit.shell),
        "shell identities changed for {}",
        report.case_name
    );
}

fn image_probe_rgba(snapshot: &ImageProbeSnapshot) -> Option<&[u8]> {
    match snapshot {
        ImageProbeSnapshot::Rgba { rgba, .. } => Some(rgba.as_slice()),
        ImageProbeSnapshot::UnexpectedCount { .. } | ImageProbeSnapshot::Unavailable { .. } => None,
    }
}

fn assert_single_outcome(
    report: &SmokeReport,
    expected_kind: &str,
    expected_compatibility: Option<&str>,
) {
    assert_eq!(report.outcomes.len(), 1, "{report:?}");
    assert_eq!(report.outcomes[0].kind, expected_kind);
    assert_eq!(
        report.outcomes[0].compatibility.as_deref(),
        expected_compatibility
    );
}
