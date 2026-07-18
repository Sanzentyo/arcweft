use arcweft_bundle::container::{BundleDigest, BundleView, ReadBudget};
use arcweft_bundle::patch::{BundlePatchArtifact, PatchCompatibility, encode_patch_bundle};
use arcweft_bundle::resource_codec::SourceMapSection;
use arcweft_bundle::{
    ArcweftBundle, BundleFormat, BundleImageAnimation, BundleImageAsset, BundleImageDimensions,
    BundleImageObject, BundleImageObjectAlignment, BundleImageObjectBounds, BundleImageObjectFit,
    BundleImageObjectPlayback, BundleImageObjectTransform, BundleManifest, BundleRuntimeSummary,
    BundleVirtualFile, BundleVirtualFileRef, BundleVirtualFileSpace,
};
use arcweft_core::bytecode::BytecodeProgram;
use arcweft_core::line_task::LineTaskGroup;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan,
};
use arcweft_core::task::{
    AwaitTarget, HostTaskArgTemplate, HostTaskRequestTemplate, NeedId, TaskId,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeValue};
use arcweft_player_native::windowed_patch::{
    FrameBoundary, PatchEventSource, RestartReason, WindowedPatchEvent, WindowedPatchReport,
};
use arcweft_player_native::{WindowedRuntimeOutcome, WindowedRuntimeOwner};
use arcweft_player_scene::input::InputController;
use arcweft_render_text::{LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode};
use arcweft_render_wgpu::geometry::RenderViewport;
use arcweft_runtime_driver::clock::RuntimeClockStep;
use arcweft_runtime_driver::session::{BundleEntryStart, BundleSessionOptions, BundleStepInput};
use arcweft_runtime_driver::swap::GenerationId;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use serde::Serialize;
use std::fmt::Write as _;

pub const GENERATED_DIR: &str =
    "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/generated";
pub const SOURCE_DIR: &str = "crates/arcweft-player-native/tests/fixtures/windowed_live_patch/src";

const BASE_SOURCE_LABEL: &str = "tests/fixtures/windowed_live_patch/src/base.arcw";
const CONTENT_TARGET_SOURCE_LABEL: &str =
    "tests/fixtures/windowed_live_patch/src/content_target.arcw";
const CODE_GENERATIONAL_TARGET_SOURCE_LABEL: &str =
    "tests/fixtures/windowed_live_patch/src/code_generational_target.arcw";
const RESTART_REQUIRED_TARGET_SOURCE_LABEL: &str =
    "tests/fixtures/windowed_live_patch/src/restart_required_target.arcw";
const WRONG_BASE_SOURCE_LABEL: &str = "tests/fixtures/windowed_live_patch/src/wrong_base.arcw";
const AWAIT_BASE_SOURCE_LABEL: &str = "tests/fixtures/windowed_live_patch/src/await_base.arcw";
const AWAIT_CODE_GENERATIONAL_TARGET_SOURCE_LABEL: &str =
    "tests/fixtures/windowed_live_patch/src/await_code_generational_target.arcw";

const BASE_SOURCE: &str = include_str!("../fixtures/windowed_live_patch/src/base.arcw");
const CONTENT_TARGET_SOURCE: &str =
    include_str!("../fixtures/windowed_live_patch/src/content_target.arcw");
const CODE_GENERATIONAL_TARGET_SOURCE: &str =
    include_str!("../fixtures/windowed_live_patch/src/code_generational_target.arcw");
const RESTART_REQUIRED_TARGET_SOURCE: &str =
    include_str!("../fixtures/windowed_live_patch/src/restart_required_target.arcw");
const WRONG_BASE_SOURCE: &str = include_str!("../fixtures/windowed_live_patch/src/wrong_base.arcw");
const AWAIT_BASE_SOURCE: &str = include_str!("../fixtures/windowed_live_patch/src/await_base.arcw");
const AWAIT_CODE_GENERATIONAL_TARGET_SOURCE: &str =
    include_str!("../fixtures/windowed_live_patch/src/await_code_generational_target.arcw");

const RED_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];
const BLUE_PNG: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x60, 0x60, 0xf8, 0xff,
    0x1f, 0x00, 0x03, 0x02, 0x01, 0xff, 0xe6, 0x77, 0x0b, 0xae, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45,
    0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

#[derive(Clone, Debug)]
pub struct BundleFixture {
    pub name: &'static str,
    pub source_label: &'static str,
    pub awfb: Vec<u8>,
    pub content_root: BundleDigest,
}

#[derive(Clone, Debug)]
pub struct PatchFixture {
    pub name: &'static str,
    pub bytes: Vec<u8>,
    pub compatibility: PatchCompatibility,
    pub base_content_root: BundleDigest,
    pub target_content_root: BundleDigest,
    pub operation_count: usize,
}

#[derive(Clone, Debug)]
pub struct WindowedLivePatchFixtures {
    pub base: BundleFixture,
    pub content_target: BundleFixture,
    pub code_generational_target: BundleFixture,
    pub restart_required_target: BundleFixture,
    pub wrong_base: BundleFixture,
    pub await_base: BundleFixture,
    pub await_code_generational_target: BundleFixture,
    pub content_patch: PatchFixture,
    pub code_generational_patch: PatchFixture,
    pub restart_required_patch: PatchFixture,
    pub wrong_base_patch: PatchFixture,
    pub await_code_generational_patch: PatchFixture,
    pub malformed_patch: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FixtureManifestSnapshot {
    pub schema_version: u32,
    pub generated_by: &'static str,
    pub source_dir: &'static str,
    pub bundles: Vec<BundleFixtureSnapshot>,
    pub patches: Vec<PatchFixtureSnapshot>,
    pub malformed_patch_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BundleFixtureSnapshot {
    pub name: String,
    pub source_label: String,
    pub awfb_path: String,
    pub content_root: String,
    pub awfb_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PatchFixtureSnapshot {
    pub name: String,
    pub patch_path: String,
    pub compatibility: String,
    pub base_content_root: String,
    pub target_content_root: String,
    pub operation_count: usize,
    pub patch_len: usize,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GeneratedFixtureFile {
    pub relative_path: String,
    #[serde(skip)]
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeReport {
    pub case_name: String,
    pub before: SmokeSnapshot,
    pub outcomes: Vec<WindowedRuntimeOutcomeSnapshot>,
    pub after_commit: SmokeSnapshot,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_observe: Option<SmokeSnapshot>,
    pub observations: Vec<SmokeObservation>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeObservation {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeSnapshot {
    pub label: String,
    pub shell: SmokeShellSnapshot,
    pub runtime: SmokeRuntimeSnapshot,
    pub patch_report: WindowedPatchReportSnapshot,
    pub direct_image_probe: ImageProbeSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeShellSnapshot {
    pub window_shell_id: u64,
    pub renderer_shell_id: u64,
    pub input_controller_id: u64,
    pub visual_clock_id: u64,
    pub presented_frames: u64,
    pub last_patch_boundary: String,
    pub prepared_frame_valid: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeRuntimeSnapshot {
    pub active_generation: u64,
    pub current_fiber_generation: Option<u64>,
    pub retired_generation_count: usize,
    pub active_content_root: Option<String>,
    pub queued_patch_count: usize,
    pub is_finished: bool,
    pub presentation_text: Option<String>,
    pub choice_count: usize,
    pub presentation_image_count: usize,
    pub last_step_status: Option<String>,
    pub last_step_finished: Option<bool>,
    pub visual_clock: SmokeVisualClockSnapshot,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SmokeVisualClockSnapshot {
    pub identity: u64,
    pub line: Option<String>,
    pub started_at_millis: u64,
    pub visual_time_millis: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WindowedPatchReportSnapshot {
    pub state: String,
    pub source: Option<String>,
    pub message: String,
    pub compatibility: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct WindowedRuntimeOutcomeSnapshot {
    pub kind: String,
    pub generation: Option<u64>,
    pub compatibility: Option<String>,
    pub content_root: Option<String>,
    pub rejection_source: Option<String>,
    pub rejection_message: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ImageProbeSnapshot {
    Rgba {
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    UnexpectedCount {
        count: usize,
    },
    Unavailable {
        message: String,
    },
}

pub fn build_windowed_live_patch_fixtures() -> WindowedLivePatchFixtures {
    let base = bundle_fixture(
        "base",
        BASE_SOURCE_LABEL,
        BASE_SOURCE,
        "Windowed smoke: base",
        Some(RED_PNG),
        false,
        false,
    );
    let content_target = bundle_fixture(
        "content_target",
        CONTENT_TARGET_SOURCE_LABEL,
        CONTENT_TARGET_SOURCE,
        "Windowed smoke: content target",
        Some(BLUE_PNG),
        false,
        false,
    );
    let code_generational_target = bundle_fixture(
        "code_generational_target",
        CODE_GENERATIONAL_TARGET_SOURCE_LABEL,
        CODE_GENERATIONAL_TARGET_SOURCE,
        "Windowed smoke: code target",
        Some(RED_PNG),
        true,
        false,
    );
    let restart_required_target = bundle_fixture(
        "restart_required_target",
        RESTART_REQUIRED_TARGET_SOURCE_LABEL,
        RESTART_REQUIRED_TARGET_SOURCE,
        "Windowed smoke: restart target",
        None,
        false,
        false,
    );
    let wrong_base = bundle_fixture(
        "wrong_base",
        WRONG_BASE_SOURCE_LABEL,
        WRONG_BASE_SOURCE,
        "Windowed smoke: wrong base",
        Some(BLUE_PNG),
        false,
        true,
    );
    let await_base = await_bundle_fixture("await_base", AWAIT_BASE_SOURCE_LABEL, AWAIT_BASE_SOURCE);
    let await_code_generational_target = await_replacement_bundle_fixture(
        "await_code_generational_target",
        AWAIT_CODE_GENERATIONAL_TARGET_SOURCE_LABEL,
        AWAIT_CODE_GENERATIONAL_TARGET_SOURCE,
    );

    let content_patch = patch_fixture("base_to_content", &base, &content_target);
    let code_generational_patch = patch_fixture(
        "base_to_code_generational",
        &base,
        &code_generational_target,
    );
    let restart_required_patch =
        patch_fixture("base_to_restart_required", &base, &restart_required_target);
    let wrong_base_patch = patch_fixture("wrong_base_to_content", &wrong_base, &content_target);
    let await_code_generational_patch = patch_fixture(
        "await_base_to_code_generational",
        &await_base,
        &await_code_generational_target,
    );

    WindowedLivePatchFixtures {
        base,
        content_target,
        code_generational_target,
        restart_required_target,
        wrong_base,
        await_base,
        await_code_generational_target,
        content_patch,
        code_generational_patch,
        restart_required_patch,
        wrong_base_patch,
        await_code_generational_patch,
        malformed_patch: b"not an AWFB patch bundle".to_vec(),
    }
}

pub fn assert_fixture_compatibility(fixtures: &WindowedLivePatchFixtures) {
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
    assert_eq!(
        fixtures.await_code_generational_patch.compatibility,
        PatchCompatibility::CodeGenerational
    );
}

pub fn fixture_manifest_snapshot(fixtures: &WindowedLivePatchFixtures) -> FixtureManifestSnapshot {
    FixtureManifestSnapshot {
        schema_version: 1,
        generated_by: "tools/regenerate-windowed-live-patch-fixtures.rs",
        source_dir: SOURCE_DIR,
        bundles: vec![
            bundle_snapshot(&fixtures.base),
            bundle_snapshot(&fixtures.content_target),
            bundle_snapshot(&fixtures.code_generational_target),
            bundle_snapshot(&fixtures.restart_required_target),
            bundle_snapshot(&fixtures.wrong_base),
            bundle_snapshot(&fixtures.await_base),
            bundle_snapshot(&fixtures.await_code_generational_target),
        ],
        patches: vec![
            patch_snapshot(&fixtures.content_patch),
            patch_snapshot(&fixtures.code_generational_patch),
            patch_snapshot(&fixtures.restart_required_patch),
            patch_snapshot(&fixtures.wrong_base_patch),
            patch_snapshot(&fixtures.await_code_generational_patch),
        ],
        malformed_patch_bytes: fixtures.malformed_patch.len(),
    }
}

pub fn all_smoke_reports(fixtures: &WindowedLivePatchFixtures) -> Result<Vec<SmokeReport>, String> {
    Ok(vec![
        content_only_smoke_report(fixtures)?,
        code_generational_smoke_report(fixtures)?,
        code_generational_task_smoke_report(fixtures)?,
        restart_required_smoke_report(fixtures)?,
        wrong_base_smoke_report(fixtures)?,
        malformed_smoke_report(fixtures)?,
    ])
}

pub fn generated_fixture_files(
    fixtures: &WindowedLivePatchFixtures,
    reports: &[SmokeReport],
) -> Result<Vec<GeneratedFixtureFile>, serde_json::Error> {
    let mut files = vec![
        binary_file("base.awfb", fixtures.base.awfb.clone()),
        binary_file("content_target.awfb", fixtures.content_target.awfb.clone()),
        binary_file(
            "code_generational_target.awfb",
            fixtures.code_generational_target.awfb.clone(),
        ),
        binary_file(
            "restart_required_target.awfb",
            fixtures.restart_required_target.awfb.clone(),
        ),
        binary_file("wrong_base.awfb", fixtures.wrong_base.awfb.clone()),
        binary_file("await_base.awfb", fixtures.await_base.awfb.clone()),
        binary_file(
            "await_code_generational_target.awfb",
            fixtures.await_code_generational_target.awfb.clone(),
        ),
        patch_file(
            "base_to_content.awfb.patch",
            fixtures.content_patch.bytes.clone(),
        ),
        patch_file(
            "base_to_code_generational.awfb.patch",
            fixtures.code_generational_patch.bytes.clone(),
        ),
        patch_file(
            "base_to_restart_required.awfb.patch",
            fixtures.restart_required_patch.bytes.clone(),
        ),
        patch_file(
            "wrong_base_to_content.awfb.patch",
            fixtures.wrong_base_patch.bytes.clone(),
        ),
        patch_file(
            "await_base_to_code_generational.awfb.patch",
            fixtures.await_code_generational_patch.bytes.clone(),
        ),
        patch_file("malformed.awfb.patch", fixtures.malformed_patch.clone()),
        json_file(
            "reports/fixture-manifest.json",
            &fixture_manifest_snapshot(fixtures),
        )?,
    ];
    for report in reports {
        files.push(json_file(
            &format!("reports/{}.expected.json", report.case_name),
            report,
        )?);
    }
    Ok(files)
}

pub fn content_only_smoke_report(
    fixtures: &WindowedLivePatchFixtures,
) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.base.awfb)?;
    harness.step_runtime(BundleStepInput::default());
    let before = harness.snapshot("content-only-before");
    harness.push_apply_patch(&fixtures.content_patch, PatchEventSource::EmbeddingApi);
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("content-only-after-commit");
    let started = harness
        .runtime_mut()
        .session_mut()
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .map_err(|error| error.to_string())?;
    harness.step_runtime(BundleStepInput::default());
    let after_observe = harness.snapshot("content-only-after-new-entry");

    Ok(SmokeReport {
        case_name: "content-only".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: Some(after_observe),
        observations: vec![
            SmokeObservation::new("expected_catalog_rgba", "[0, 0, 255, 255]"),
            SmokeObservation::generation("new_foreground_generation", started.generation),
        ],
    })
}

pub fn code_generational_smoke_report(
    fixtures: &WindowedLivePatchFixtures,
) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.base.awfb)?;
    let old_generation = harness.runtime().session().active_generation().id;
    let before = harness.snapshot("code-generational-before");
    harness.push_apply_patch(
        &fixtures.code_generational_patch,
        PatchEventSource::EmbeddingApi,
    );
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("code-generational-after-commit");
    let started = harness
        .runtime_mut()
        .session_mut()
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .map_err(|error| error.to_string())?;
    harness.step_runtime(BundleStepInput::default());
    let after_observe = harness.snapshot("code-generational-after-new-entry");

    Ok(SmokeReport {
        case_name: "code-generational".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: Some(after_observe),
        observations: vec![
            SmokeObservation::generation("old_foreground_generation", old_generation),
            SmokeObservation::generation("new_foreground_generation", started.generation),
        ],
    })
}

pub fn code_generational_task_smoke_report(
    fixtures: &WindowedLivePatchFixtures,
) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.await_base.awfb)?;
    let waiting = harness.step_runtime(BundleStepInput::default());
    let task = waiting
        .requested_tasks
        .first()
        .cloned()
        .ok_or_else(|| "await fixture did not request a host task".to_owned())?;
    let task_sequence = task.sequence;
    let old_task_generation = harness
        .runtime()
        .session()
        .task_generation(task_sequence)
        .ok_or_else(|| "await task was not pinned to a generation".to_owned())?;
    let before = harness.snapshot("code-generational-task-before");

    harness.push_apply_patch(
        &fixtures.await_code_generational_patch,
        PatchEventSource::EmbeddingApi,
    );
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("code-generational-task-after-commit");
    let task_generation_after_commit =
        harness
            .runtime()
            .session()
            .task_generation(task_sequence)
            .ok_or_else(|| "await task generation disappeared before completion".to_owned())?;
    let started = harness
        .runtime_mut()
        .session_mut()
        .start_foreground_entry_on_current_generation(BundleEntryStart::session_default())
        .map_err(|error| error.to_string())?;
    harness.step_runtime(BundleStepInput {
        task_events: vec![task.ready(RuntimePayload::new(RuntimeValue::String(
            "windowed-smoke-handle".to_owned(),
        )))],
        ..BundleStepInput::default()
    });
    let task_generation_after_completion =
        harness.runtime().session().task_generation(task_sequence);
    let after_observe = harness.snapshot("code-generational-task-after-completion");

    Ok(SmokeReport {
        case_name: "code-generational-task".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: Some(after_observe),
        observations: vec![
            SmokeObservation::generation("old_task_generation", old_task_generation),
            SmokeObservation::generation(
                "task_generation_after_commit",
                task_generation_after_commit,
            ),
            SmokeObservation::generation("new_entry_generation", started.generation),
            SmokeObservation::new(
                "task_generation_after_completion",
                optional_generation(task_generation_after_completion),
            ),
        ],
    })
}

pub fn restart_required_smoke_report(
    fixtures: &WindowedLivePatchFixtures,
) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.base.awfb)?;
    harness.step_runtime(BundleStepInput::default());
    let before = harness.snapshot("restart-required-before");
    harness.push_apply_patch(
        &fixtures.restart_required_patch,
        PatchEventSource::EmbeddingApi,
    );
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("restart-required-after-commit");
    harness.step_runtime(BundleStepInput::default());
    let after_observe = harness.snapshot("restart-required-after-observe");

    Ok(SmokeReport {
        case_name: "restart-required".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: Some(after_observe),
        observations: vec![SmokeObservation::new(
            "restart_reason",
            RestartReason::RestartRequiredPatch.label(),
        )],
    })
}

pub fn wrong_base_smoke_report(
    fixtures: &WindowedLivePatchFixtures,
) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.base.awfb)?;
    harness.step_runtime(BundleStepInput::default());
    let before = harness.snapshot("wrong-base-before");
    harness.push_apply_patch(&fixtures.wrong_base_patch, PatchEventSource::EmbeddingApi);
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("wrong-base-after-reject");

    Ok(SmokeReport {
        case_name: "wrong-base".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: None,
        observations: Vec::new(),
    })
}

pub fn malformed_smoke_report(fixtures: &WindowedLivePatchFixtures) -> Result<SmokeReport, String> {
    let mut harness = WindowedSmokeHarness::from_awfb(&fixtures.base.awfb)?;
    harness.step_runtime(BundleStepInput::default());
    let before = harness.snapshot("malformed-before");
    harness
        .runtime_mut()
        .push_patch_event(WindowedPatchEvent::ApplyBundle {
            bytes: fixtures.malformed_patch.clone(),
            source: PatchEventSource::EmbeddingApi,
        });
    let outcomes = harness.render_then_drain_patch_boundary()?;
    let after_commit = harness.snapshot("malformed-after-reject");

    Ok(SmokeReport {
        case_name: "malformed".to_owned(),
        before,
        outcomes: outcome_snapshots(&outcomes),
        after_commit,
        after_observe: None,
        observations: Vec::new(),
    })
}

impl SmokeShellSnapshot {
    pub const fn has_same_shell_identities(&self, other: &Self) -> bool {
        self.window_shell_id == other.window_shell_id
            && self.renderer_shell_id == other.renderer_shell_id
            && self.input_controller_id == other.input_controller_id
            && self.visual_clock_id == other.visual_clock_id
    }
}

impl SmokeObservation {
    fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
        }
    }

    fn generation(key: impl Into<String>, generation: GenerationId) -> Self {
        Self::new(key, generation.0.to_string())
    }
}

pub struct WindowedSmokeHarness {
    runtime: WindowedRuntimeOwner,
    window_shell: SmokeWindowShell,
    renderer_shell: SmokeRendererShell,
    input_controller: SmokeInputController,
    visual_clock: SmokeVisualClock,
    next_tick: u64,
    prepared_frame_valid: bool,
    last_step_status: Option<String>,
    last_step_finished: Option<bool>,
}

impl WindowedSmokeHarness {
    pub fn from_awfb(awfb_bytes: &[u8]) -> Result<Self, String> {
        let runtime = WindowedRuntimeOwner::from_awfb_bytes(
            awfb_bytes.to_vec(),
            BundleSessionOptions::default(),
        )
        .map_err(|error| error.to_string())?;
        Ok(Self {
            runtime,
            window_shell: SmokeWindowShell::new(7001),
            renderer_shell: SmokeRendererShell::new(7002),
            input_controller: SmokeInputController::new(7003),
            visual_clock: SmokeVisualClock::new(7004),
            next_tick: 1,
            prepared_frame_valid: false,
            last_step_status: None,
            last_step_finished: None,
        })
    }

    pub const fn runtime(&self) -> &WindowedRuntimeOwner {
        &self.runtime
    }

    pub fn runtime_mut(&mut self) -> &mut WindowedRuntimeOwner {
        &mut self.runtime
    }

    pub fn push_apply_patch(&mut self, patch: &PatchFixture, source: PatchEventSource) {
        self.runtime
            .push_patch_event(WindowedPatchEvent::ApplyBundle {
                bytes: patch.bytes.clone(),
                source,
            });
    }

    pub fn step_runtime(
        &mut self,
        input: BundleStepInput,
    ) -> arcweft_runtime_driver::session::BundleSessionStep {
        let clock =
            RuntimeClockStep::from_millis(self.next_tick, 16).expect("smoke clock is valid");
        self.next_tick = self.next_tick.saturating_add(1);
        let step = self.runtime.session_mut().step_with_clock(clock, input);
        self.last_step_status = Some(step.status_label.clone());
        self.last_step_finished = Some(step.finished);
        step
    }

    pub fn render_then_drain_patch_boundary(
        &mut self,
    ) -> Result<Vec<WindowedRuntimeOutcome>, String> {
        self.window_shell.presented_frames = self.window_shell.presented_frames.saturating_add(1);
        self.visual_clock
            .advance_from_runtime(&self.runtime, self.window_shell.presented_frames);
        let outcomes = self
            .runtime
            .drain_patch_boundary(FrameBoundary::AfterRenderSubmitted)
            .map_err(|error| error.to_string())?;
        self.prepared_frame_valid = !outcomes
            .iter()
            .any(WindowedRuntimeOutcome::invalidates_prepared_frame);
        Ok(outcomes)
    }

    pub fn snapshot(&self, label: impl Into<String>) -> SmokeSnapshot {
        let presentation = self.runtime.session().presentation();
        SmokeSnapshot {
            label: label.into(),
            shell: SmokeShellSnapshot {
                window_shell_id: self.window_shell.identity,
                renderer_shell_id: self.renderer_shell.identity,
                input_controller_id: self.input_controller.identity(),
                visual_clock_id: self.visual_clock.identity,
                presented_frames: self.window_shell.presented_frames,
                last_patch_boundary: FrameBoundary::AfterRenderSubmitted.label().to_owned(),
                prepared_frame_valid: self.prepared_frame_valid,
            },
            runtime: SmokeRuntimeSnapshot {
                active_generation: self.runtime.session().active_generation().id.0,
                current_fiber_generation: self
                    .runtime
                    .session()
                    .current_fiber_generation()
                    .map(|generation| generation.0),
                retired_generation_count: self.runtime.session().retired_generation_count(),
                active_content_root: self
                    .runtime
                    .session()
                    .active_container_content_root()
                    .map(digest_string),
                queued_patch_count: self.runtime.queued_patch_count(),
                is_finished: self.runtime.session().is_finished(),
                presentation_text: presentation
                    .dialogue
                    .latest_active()
                    .and_then(|(_, entry)| entry.current_stage())
                    .map(|stage| stage.text().to_owned()),
                choice_count: presentation.choices.len(),
                presentation_image_count: presentation.images.len(),
                last_step_status: self.last_step_status.clone(),
                last_step_finished: self.last_step_finished,
                visual_clock: self.visual_clock.snapshot(),
            },
            patch_report: patch_report_snapshot(self.runtime.last_patch_report()),
            direct_image_probe: self.direct_image_probe(),
        }
    }

    fn direct_image_probe(&self) -> ImageProbeSnapshot {
        match self.runtime.images().render_images(
            &[fixture_image_object()],
            0,
            fixture_image_probe_viewport(),
        ) {
            Ok(rendered) => match rendered.as_slice() {
                [image] => ImageProbeSnapshot::Rgba {
                    width: image.frame.width,
                    height: image.frame.height,
                    rgba: image.frame.rgba.clone(),
                },
                images => ImageProbeSnapshot::UnexpectedCount {
                    count: images.len(),
                },
            },
            Err(error) => ImageProbeSnapshot::Unavailable {
                message: error.to_string(),
            },
        }
    }
}

fn fixture_image_probe_viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 1.0,
        logical_height: 1.0,
        physical_width: 1,
        physical_height: 1,
        scale_factor: 1.0,
    }
}

struct SmokeWindowShell {
    identity: u64,
    presented_frames: u64,
}

impl SmokeWindowShell {
    const fn new(identity: u64) -> Self {
        Self {
            identity,
            presented_frames: 0,
        }
    }
}

struct SmokeRendererShell {
    identity: u64,
}

impl SmokeRendererShell {
    const fn new(identity: u64) -> Self {
        Self { identity }
    }
}

struct SmokeInputController {
    identity: u64,
    controller: InputController,
}

impl SmokeInputController {
    fn new(identity: u64) -> Self {
        Self {
            identity,
            controller: InputController::default(),
        }
    }

    fn identity(&self) -> u64 {
        let _ = &self.controller;
        self.identity
    }
}

struct SmokeVisualClock {
    identity: u64,
    line: Option<RuntimeLineId>,
    started_at_millis: u64,
    visual_time_millis: u64,
}

impl SmokeVisualClock {
    const fn new(identity: u64) -> Self {
        Self {
            identity,
            line: None,
            started_at_millis: 0,
            visual_time_millis: 0,
        }
    }

    fn advance_from_runtime(&mut self, runtime: &WindowedRuntimeOwner, presented_frames: u64) {
        let elapsed_millis = presented_frames.saturating_mul(16);
        let dialogue = runtime.session().presentation().dialogue.latest_active();
        let Some((_, entry)) = dialogue else {
            self.line = None;
            self.started_at_millis = elapsed_millis;
            self.visual_time_millis = 0;
            return;
        };
        if self.line.as_ref() != Some(&entry.frame().line) {
            self.line = Some(entry.frame().line.clone());
            self.started_at_millis = elapsed_millis;
        }
        self.visual_time_millis = elapsed_millis.saturating_sub(self.started_at_millis);
    }

    fn snapshot(&self) -> SmokeVisualClockSnapshot {
        SmokeVisualClockSnapshot {
            identity: self.identity,
            line: self.line.as_ref().map(|line| format!("{line:?}")),
            started_at_millis: self.started_at_millis,
            visual_time_millis: self.visual_time_millis,
        }
    }
}

fn bundle_fixture(
    name: &'static str,
    source_label: &'static str,
    source: &str,
    display_text: &str,
    image_bytes: Option<&[u8]>,
    changed_main_code: bool,
    extra_flow: bool,
) -> BundleFixture {
    let bundle = dialogue_bundle(
        source_label,
        source,
        display_text,
        image_bytes,
        changed_main_code,
        extra_flow,
    );
    encoded_fixture(name, source_label, &bundle)
}

fn await_bundle_fixture(
    name: &'static str,
    source_label: &'static str,
    source: &str,
) -> BundleFixture {
    let bundle = await_bundle(source_label, source);
    encoded_fixture(name, source_label, &bundle)
}

fn await_replacement_bundle_fixture(
    name: &'static str,
    source_label: &'static str,
    source: &str,
) -> BundleFixture {
    let bundle = await_replacement_bundle(source_label, source);
    encoded_fixture(name, source_label, &bundle)
}

fn encoded_fixture(
    name: &'static str,
    source_label: &'static str,
    bundle: &ArcweftBundle,
) -> BundleFixture {
    let awfb = bundle
        .to_format_bytes(BundleFormat::Awfb)
        .expect("fixture encodes as AWFB");
    let content_root = awfb_root(&awfb);
    BundleFixture {
        name,
        source_label,
        awfb,
        content_root,
    }
}

fn patch_fixture(name: &'static str, base: &BundleFixture, target: &BundleFixture) -> PatchFixture {
    let base_view = BundleView::parse(&base.awfb, ReadBudget::default()).expect("base AWFB parses");
    let target_view =
        BundleView::parse(&target.awfb, ReadBudget::default()).expect("target AWFB parses");
    let artifact = BundlePatchArtifact::from_views(&base_view, &target_view)
        .expect("patch artifact builds from real AWFB views");
    let operation_count = artifact.plan.operations.len();
    let compatibility = artifact.manifest.compatibility;
    let bytes = encode_patch_bundle(&artifact).expect("patch artifact encodes as AWFB patch");
    PatchFixture {
        name,
        bytes,
        compatibility,
        base_content_root: artifact.manifest.base_content_root,
        target_content_root: artifact.manifest.target_content_root,
        operation_count,
    }
}

fn dialogue_bundle(
    source_label: &str,
    source: &str,
    display_text: &str,
    image_bytes: Option<&[u8]>,
    changed_main_code: bool,
    extra_flow: bool,
) -> ArcweftBundle {
    let line = RuntimeLineId::from_runtime_line_value("line.opening").expect("runtime line id");
    let plan = dialogue_runtime_plan(&line, changed_main_code, extra_flow);
    let display = dialogue_display_catalog(line, display_text);
    with_optional_fixture_image(
        bundle_from_runtime_parts(source_label, source, plan, display, "dialogue"),
        image_bytes,
    )
}

fn dialogue_runtime_plan(
    line: &RuntimeLineId,
    changed_main_code: bool,
    extra_flow: bool,
) -> RuntimePlan {
    let mut flows = vec![
        RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
            ops: dialogue_main_ops(line, changed_main_code),
        },
        RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.done").expect("flow runtime id"),
            ops: vec![FlowOp::Return("done".to_owned())],
        },
    ];
    if extra_flow {
        flows.push(RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.extra").expect("flow runtime id"),
            ops: vec![FlowOp::Return("extra".to_owned())],
        });
    }
    RuntimePlan::new(flows, vec![LineTaskGroup::default()])
        .expect("dialogue fixture runtime plan is valid")
        .with_entries(vec![cli_main_entry()])
}

fn dialogue_main_ops(line: &RuntimeLineId, changed_main_code: bool) -> Vec<FlowOp> {
    if changed_main_code {
        return vec![FlowOp::Return("changed".to_owned())];
    }
    vec![
        FlowOp::Dialogue {
            line: line.clone(),
            task_group: 0,
        },
        FlowOp::Choice {
            id: Some("choice.opening".to_owned()),
            options: vec![ChoiceRuntimeOption {
                id: Some("choice.opening.next".to_owned()),
                label: "Next".to_owned(),
                target: Some(
                    FlowRuntimeId::from_runtime_target_value("flow.done").expect("flow runtime id"),
                ),
                out: None,
                effects: Vec::new(),
            }],
        },
    ]
}

fn dialogue_display_catalog(line: RuntimeLineId, display_text: &str) -> LineDisplayCatalog {
    LineDisplayCatalog::new(vec![LineDisplaySpec {
        line,
        callee: "alice".to_owned(),
        speaker_label: None,
        text_key: None,
        view: None,
        voice: None,
        look: None,
        style: None,
        base_styles: Vec::new(),
        default_inline_failure_policy: None,
        style_contributions: Vec::new(),
        args: Vec::new(),
        content: RichTextDocument::new(vec![RichTextNode::Text {
            text: display_text.to_owned(),
        }]),
    }])
}

fn bundle_from_runtime_parts(
    source_label: &str,
    source: &str,
    plan: RuntimePlan,
    display: LineDisplayCatalog,
    fixture_name: &str,
) -> ArcweftBundle {
    let product_awbc = AwbcLowerer::new(&plan, &display, source_label)
        .lower()
        .unwrap_or_else(|error| panic!("{fixture_name} fixture product AWBC lowers: {error:?}"))
        .program;
    let bytecode = BytecodeProgram::from_runtime_plan(plan);
    let stats = bytecode.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(source_label, source),
        bytecode,
        display,
    )
    .expect("standard dialogue source joins source map")
    .with_product_awbc(product_awbc)
}

fn with_optional_fixture_image(bundle: ArcweftBundle, image_bytes: Option<&[u8]>) -> ArcweftBundle {
    match image_bytes {
        Some(bytes) => bundle
            .with_virtual_files([BundleVirtualFile {
                space: BundleVirtualFileSpace::Asset,
                path: "sprite.png".to_owned(),
                bytes: bytes.to_vec(),
            }])
            .with_image_assets([BundleImageAsset {
                id: "sprite".to_owned(),
                file: BundleVirtualFileRef {
                    space: BundleVirtualFileSpace::Asset,
                    path: "sprite.png".to_owned(),
                },
                format: arcweft_bundle::BundleImageFormat::Png,
                animation: BundleImageAnimation::Static,
                dimensions: Some(BundleImageDimensions {
                    width: 1,
                    height: 1,
                }),
            }])
            .with_image_objects([fixture_image_object()]),
        None => bundle,
    }
}

fn await_bundle(source_label: &str, source: &str) -> ArcweftBundle {
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
            ops: vec![
                FlowOp::Await {
                    binding: None,
                    target: AwaitTarget {
                        need: NeedId("need.bg".to_owned()),
                        task: TaskId("task.bg".to_owned()),
                        request: HostTaskRequestTemplate::new(
                            "asset",
                            "image",
                            [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                RuntimeValue::String("asset.bg.room".to_owned()),
                            ))],
                        ),
                    },
                    pending: Vec::new(),
                },
                FlowOp::Return("ready".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("await fixture runtime plan is valid")
    .with_entries(vec![cli_main_entry()]);
    let display = LineDisplayCatalog::default();
    let product_awbc = AwbcLowerer::new(&plan, &display, source_label)
        .lower()
        .expect("await fixture product AWBC lowers")
        .program;
    let bytecode = BytecodeProgram::from_runtime_plan(plan);
    let stats = bytecode.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(source_label, source),
        bytecode,
        display,
    )
    .expect("standard dialogue source joins source map")
    .with_product_awbc(product_awbc)
}

fn await_replacement_bundle(source_label: &str, source: &str) -> ArcweftBundle {
    let plan = RuntimePlan::new(
        vec![RuntimeFlow {
            id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
            ops: vec![FlowOp::Return("changed".to_owned())],
        }],
        Vec::new(),
    )
    .expect("await replacement runtime plan is valid")
    .with_entries(vec![cli_main_entry()]);
    let display = LineDisplayCatalog::default();
    let product_awbc = AwbcLowerer::new(&plan, &display, source_label)
        .lower()
        .expect("await replacement product AWBC lowers")
        .program;
    let bytecode = BytecodeProgram::from_runtime_plan(plan);
    let stats = bytecode.stats();
    ArcweftBundle::try_new(
        BundleManifest {
            profile_id: None,
            profile_kind: None,
            entry: Some("entry.main".to_owned()),
            adapter: None,
            adapter_manifest_ids: Vec::new(),
            required_host_calls: Vec::new(),
            runtime: BundleRuntimeSummary {
                entry_flow: Some("flow.main".to_owned()),
                flows: stats.flows,
                bytecode_instructions: stats.instructions,
                line_task_groups: stats.line_task_groups,
                stream_plans: stats.stream_plans,
                source_plans: stats.source_plans,
            },
        },
        source_map(source_label, source),
        bytecode,
        display,
    )
    .expect("standard dialogue source joins source map")
    .with_product_awbc(product_awbc)
}

fn source_map(label: &str, text: &str) -> SourceMapSection {
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new(label).expect("source ID"),
        SourceName::path(label),
        text,
    )
    .expect("source document");
    SourceMapSection::try_from_documents(&[&document]).expect("source map")
}

fn cli_main_entry() -> arcweft_core::plan::RuntimeEntrySpec {
    arcweft_core::plan::RuntimeEntrySpec {
        id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
            .expect("test entry ID is valid"),
        kind: arcweft_core::plan::RuntimeEntryKind::Cli,
        binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
        target: arcweft_core::plan::RuntimeEntryTarget::Flow(
            FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
        ),
        roles: arcweft_core::entry::RuntimeEntryRoles::None,
    }
}

fn fixture_image_object() -> BundleImageObject {
    BundleImageObject {
        id: "sprite.object".to_owned(),
        asset: "sprite".to_owned(),
        target: None,
        layer: None,
        view: None,
        containing_scroll_region: None,
        bounds: BundleImageObjectBounds {
            x_milli: 0,
            y_milli: 0,
            width_milli: 1000,
            height_milli: 1000,
        },
        placement: None,
        fit: BundleImageObjectFit::Stretch,
        alignment: BundleImageObjectAlignment {
            x_milli: 500,
            y_milli: 500,
        },
        playback: BundleImageObjectPlayback {
            start_time_millis: 0,
            rate_milli: 1000,
            paused_at_millis: None,
            pinned_local_time_millis: None,
        },
        transform: BundleImageObjectTransform {
            m11_milli: 1000,
            m12_milli: 0,
            m21_milli: 0,
            m22_milli: 1000,
            tx_milli: 0,
            ty_milli: 0,
        },
        depth_milli: 0,
        opacity_milli: 1000,
        actions: Vec::new(),
        params: std::collections::BTreeMap::default(),
        proxies: Vec::new(),
        visible: true,
    }
}

fn bundle_snapshot(fixture: &BundleFixture) -> BundleFixtureSnapshot {
    BundleFixtureSnapshot {
        name: fixture.name.to_owned(),
        source_label: fixture.source_label.to_owned(),
        awfb_path: format!("{GENERATED_DIR}/{}.awfb", fixture.name),
        content_root: digest_string(fixture.content_root),
        awfb_len: fixture.awfb.len(),
    }
}

fn patch_snapshot(fixture: &PatchFixture) -> PatchFixtureSnapshot {
    PatchFixtureSnapshot {
        name: fixture.name.to_owned(),
        patch_path: format!("{GENERATED_DIR}/patches/{}.awfb.patch", fixture.name),
        compatibility: fixture.compatibility.label().to_owned(),
        base_content_root: digest_string(fixture.base_content_root),
        target_content_root: digest_string(fixture.target_content_root),
        operation_count: fixture.operation_count,
        patch_len: fixture.bytes.len(),
    }
}

fn binary_file(name: &str, bytes: Vec<u8>) -> GeneratedFixtureFile {
    GeneratedFixtureFile {
        relative_path: format!("{GENERATED_DIR}/{name}"),
        bytes,
    }
}

fn patch_file(name: &str, bytes: Vec<u8>) -> GeneratedFixtureFile {
    GeneratedFixtureFile {
        relative_path: format!("{GENERATED_DIR}/patches/{name}"),
        bytes,
    }
}

fn json_file<T: Serialize>(
    name: &str,
    value: &T,
) -> Result<GeneratedFixtureFile, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(GeneratedFixtureFile {
        relative_path: format!("{GENERATED_DIR}/{name}"),
        bytes,
    })
}

fn awfb_root(bytes: &[u8]) -> BundleDigest {
    BundleView::parse(bytes, ReadBudget::default())
        .expect("fixture AWFB parses")
        .content_root()
}

fn outcome_snapshots(outcomes: &[WindowedRuntimeOutcome]) -> Vec<WindowedRuntimeOutcomeSnapshot> {
    outcomes.iter().map(outcome_snapshot).collect()
}

fn outcome_snapshot(outcome: &WindowedRuntimeOutcome) -> WindowedRuntimeOutcomeSnapshot {
    WindowedRuntimeOutcomeSnapshot {
        kind: outcome.kind_label().to_owned(),
        generation: outcome.generation().map(|generation| generation.0),
        compatibility: outcome
            .compatibility()
            .map(|compatibility| compatibility.label().to_owned()),
        content_root: outcome.content_root().map(digest_string),
        rejection_source: outcome
            .rejection_source()
            .map(PatchEventSource::label)
            .map(str::to_owned),
        rejection_message: outcome.rejection_message().map(str::to_owned),
    }
}

fn patch_report_snapshot(report: &WindowedPatchReport) -> WindowedPatchReportSnapshot {
    WindowedPatchReportSnapshot {
        state: report.state.label().to_owned(),
        source: report
            .source
            .as_ref()
            .map(PatchEventSource::label)
            .map(str::to_owned),
        message: report.message.clone(),
        compatibility: report
            .compatibility
            .map(|compatibility| compatibility.label().to_owned()),
    }
}

fn digest_string(digest: BundleDigest) -> String {
    digest.to_string()
}

fn optional_generation(generation: Option<GenerationId>) -> String {
    generation.map_or_else(|| "none".to_owned(), |generation| generation.0.to_string())
}

pub fn summarize_report(report: &SmokeReport) -> String {
    let mut summary = String::new();
    let _ = writeln!(summary, "case={}", report.case_name);
    let _ = writeln!(summary, "outcomes={}", report.outcomes.len());
    for outcome in &report.outcomes {
        let _ = writeln!(
            summary,
            "  outcome={} compatibility={:?} generation={:?}",
            outcome.kind, outcome.compatibility, outcome.generation
        );
    }
    let _ = writeln!(
        summary,
        "shell_preserved={}",
        report
            .before
            .shell
            .has_same_shell_identities(&report.after_commit.shell)
    );
    summary
}
