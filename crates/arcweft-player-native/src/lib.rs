//! Native/headless rich-text player host for Arcweft.

mod clipboard;
#[cfg(feature = "dev-capture")]
mod dev_capture;
mod native_audio;
mod patch_endpoint;
mod scene_windowed;
mod text_input_bridge;
mod window_driver;
mod windowed_environment_ingress;
mod windowed_ingress;
pub mod windowed_patch;
mod windowed_player_ingress;
mod windowed_runtime;

#[cfg(feature = "dev-capture")]
pub use dev_capture::{
    NativePlayerCaptureContentBBox, NativePlayerCaptureError, NativePlayerCaptureRequest,
    NativePlayerFrameCapture, capture_bundle_frame,
};
pub use patch_endpoint::{
    NativePatchEndpoint, NativePatchEndpointError, NativePatchOutcome, NativePatchTransportAction,
    NativePatchTransportEnvelope, NativePreparedPatch,
};
pub use scene_windowed::{
    NativePlayerOptions, run_bundle_windowed, run_bundle_windowed_with_ingress,
    run_bundle_windowed_with_ingress_and_options,
    run_bundle_windowed_with_ingress_and_text_input_options, run_bundle_windowed_with_options,
    run_bundle_windowed_with_text_input_options,
};
pub use text_input_bridge::{NativeTextInputBridgeOptions, NativeTextInputTraceOptions};
pub use windowed_environment_ingress::{
    DEFAULT_WINDOWED_ENVIRONMENT_INGRESS_CAPACITY, WindowedEnvironmentIngress,
    WindowedEnvironmentIngressCommand, WindowedEnvironmentIngressConfig,
    WindowedEnvironmentIngressReceipt, WindowedEnvironmentIngressReport,
    WindowedEnvironmentIngressReportState, WindowedEnvironmentUpdateError,
    WindowedEnvironmentUpdateErrorKind,
};
pub use windowed_ingress::{
    WindowedLocalSidecar, WindowedPatchIngress, WindowedPatchIngressAccepted,
    WindowedPatchIngressConfig, WindowedPatchIngressError, WindowedPatchIngressErrorKind,
    WindowedPatchIngressReport, WindowedPatchIngressReportState, WindowedPatchTransportActionSet,
};
pub use windowed_player_ingress::WindowedPlayerIngress;
pub use windowed_runtime::{
    WindowedRuntimeOutcome, WindowedRuntimeOwner, WindowedRuntimeOwnerError,
};

use arcweft_bundle::ArcweftBundle;
use arcweft_core::plan::FlowEvent;
use arcweft_runtime_host::{
    BundleRunnerOptions, BundleRunnerStepMode, NativeTaskStats, RuntimeExecutorStats,
    run_bundle_with_native_adapters,
};

#[cfg(test)]
pub(crate) fn test_character_plan()
-> arcweft_dialogue::character_presentation::CheckedCharacterPresentationPlan {
    use arcweft_character::presentation_name::{
        CharacterPresentationCatalogGeneration, CharacterPresentationCatalogRevision,
    };
    use arcweft_dialogue::character_presentation::{
        CharacterPresentationTargetEvidence, CheckedCharacterPresentationPlan,
    };
    let catalog = test_character_catalog();
    CheckedCharacterPresentationPlan::try_new(
        CharacterPresentationTargetEvidence::Exact(
            arcweft_character::id::CharacterId::try_new("character.fixture").unwrap(),
        ),
        CharacterPresentationCatalogGeneration::new(
            CharacterPresentationCatalogRevision::INITIAL,
            catalog.semantic_digest(),
            catalog.locale_policy_digest(),
        ),
    )
    .unwrap()
}

#[cfg(test)]
pub(crate) fn test_character_catalog()
-> arcweft_character::presentation_name::CharacterPresentationCatalogData {
    use arcweft_character::presentation_name::{
        CharacterDisplayNameInput, CharacterDisplayNameRecordInput, CharacterDisplayNameValue,
        CharacterNameLocale, CharacterNameLocalePolicy, CharacterPresentationCatalogData,
        CharacterPresentationCatalogInput, CharacterPresentationRole,
    };
    let locale = CharacterNameLocale::new(arcweft_id::LocaleTag::try_new("en").unwrap());
    let policy = CharacterNameLocalePolicy::try_new(locale, Vec::new()).unwrap();
    let record = CharacterDisplayNameRecordInput::try_new(
        arcweft_character::id::CharacterId::try_new("character.fixture").unwrap(),
        CharacterPresentationRole::Character,
        None,
        Some(CharacterDisplayNameInput::Visible(
            CharacterDisplayNameValue::try_new("Fixture").unwrap(),
        )),
        Vec::new(),
        None,
    )
    .unwrap();
    CharacterPresentationCatalogData::try_from_inputs(
        CharacterPresentationCatalogInput::try_new(policy, vec![record]).unwrap(),
    )
    .unwrap()
}

#[cfg(test)]
pub(crate) fn test_dialogue_profile() -> arcweft_dialogue::DialoguePresentationProfile {
    arcweft_dialogue::DialoguePresentationProfile::engine_default()
}

#[cfg(test)]
pub(crate) fn test_dialogue_profile_revision() -> arcweft_dialogue::DialogueProfileRevision {
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("player-native-dialogue-profile-fixture").unwrap(),
        SourceName::Memory,
        "schema = 1\n",
    )
    .unwrap();
    let sources = SourceSetRevision::try_for_identities([source.identity()]).unwrap();
    arcweft_dialogue::DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.player_native_dialogue").unwrap(),
        AcceptedViewProgramRevision::try_from_bytes([0x45; 32]).unwrap(),
        ResourceTypeRegistry::empty().digest(),
    )
}
use arcweft_text_model::LineDisplayFrame;
use serde::Serialize;
use std::path::Path;
use thiserror::Error;

/// Headless player report used by tests and CLI automation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeadlessPlayerReport {
    pub frames: Vec<LineDisplayFrame>,
    pub diagnostics: Vec<String>,
    pub steps: usize,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<NativePlayerRuntimeMetadata>,
    #[cfg(feature = "dev-capture")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_capture: Option<NativePlayerCaptureMetadata>,
}

/// Runtime-host metadata emitted by product `.awfb` player execution.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NativePlayerRuntimeMetadata {
    pub source: String,
    pub bytecode_instructions: usize,
    pub adapter_manifests: usize,
    pub executor: &'static str,
    pub executor_stats: RuntimeExecutorStats,
    pub native_io: NativeTaskStats,
}

/// Metadata for a native offscreen framebuffer capture emitted by the player binary.
#[cfg(feature = "dev-capture")]
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct NativePlayerCaptureMetadata {
    pub renderer: String,
    pub format: String,
    pub width: u32,
    pub height: u32,
    pub pixel_format: String,
    pub row_stride_bytes: u32,
    pub content_bbox: Option<NativePlayerCaptureContentBBox>,
    pub content_pixels: u64,
    pub written: String,
}

/// Native player error.
#[derive(Debug, Error)]
pub enum NativePlayerError {
    #[error(transparent)]
    BundleRunner(#[from] arcweft_runtime_host::BundleRunnerError),
    #[error("native shared scene window failed: {0}")]
    SceneWindow(String),
    #[error(transparent)]
    Audio(#[from] native_audio::NativePlayerAudioError),
    #[error(
        "runtime dialogue line {line} has no checked CharacterDialogue presentation projection"
    )]
    DialoguePresentationUnavailable { line: String },
}

/// Runs a compiled `.awfb` bundle through the runtime-host bundle boundary.
pub fn run_bundle_headless(
    bundle: &ArcweftBundle,
    max_steps: usize,
) -> Result<HeadlessPlayerReport, NativePlayerError> {
    let runner = run_bundle_with_native_adapters(
        bundle,
        &BundleRunnerOptions {
            steps: max_steps,
            mode: BundleRunnerStepMode::Game,
            max_ops: 64,
            ..BundleRunnerOptions::default()
        },
        &[desktop_native_adapter_registrar],
    )?;
    let frames = Vec::new();
    let mut diagnostics = Vec::new();
    for step in &runner.steps {
        diagnostics.extend(step.diagnostics.iter().cloned());
        reject_unprojected_dialogue_events(&step.flow_events)?;
    }
    Ok(HeadlessPlayerReport {
        frames,
        diagnostics,
        steps: runner.steps.len(),
        status: runner.final_status,
        runtime: Some(NativePlayerRuntimeMetadata {
            source: runner.source,
            bytecode_instructions: runner.bytecode_instructions,
            adapter_manifests: runner.adapter_manifests,
            executor: "awbc_product",
            executor_stats: runner.executor_stats,
            native_io: runner.native_io,
        }),
        #[cfg(feature = "dev-capture")]
        native_capture: None,
    })
}

fn desktop_native_adapter_registrar(
    _source_path: &Path,
    builder: arcweft_host_adapter::HostAdapterRegistryBuilder,
) -> Result<arcweft_host_adapter::HostAdapterRegistryBuilder, arcweft_host_adapter::HostAdapterError>
{
    let adapter_set = arcweft_adapter_desktop::DesktopAdapterSet::bind_current_thread(
        arcweft_desktop_native::NativeDesktopBackend::builder().build(),
    );
    adapter_set.register(builder).map(|(builder, _)| builder)
}

fn reject_unprojected_dialogue_events(events: &[FlowEvent]) -> Result<(), NativePlayerError> {
    for event in events {
        match event {
            FlowEvent::DialogueLine { line, .. } => {
                return Err(NativePlayerError::DialoguePresentationUnavailable {
                    line: line.canonical_label().clone(),
                });
            }
            FlowEvent::LineCancelled { .. }
            | FlowEvent::ChoicePresented { .. }
            | FlowEvent::ChoiceSelected { .. }
            | FlowEvent::AwaitStarted { .. }
            | FlowEvent::AwaitReady { .. }
            | FlowEvent::AwaitProgress { .. }
            | FlowEvent::Goto { .. }
            | FlowEvent::Return { .. }
            | FlowEvent::Done => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    fn fixture_runtime_artifact_fingerprint() -> arcweft_core::effect::RuntimeArtifactFingerprint {
        arcweft_core::effect::RuntimeArtifactFingerprint::try_from_bytes([0x6a; 32])
            .expect("fixture runtime artifact fingerprint is non-zero")
    }

    #[test]
    fn bundle_headless_rejects_dialogue_without_checked_presentation_projection() {
        use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
        use arcweft_core::entry::{FlowContractHash, RuntimeFlowExecutable, RuntimeFlowSchema};
        use arcweft_core::plan::{
            FlowRuntimeId, RuntimeDialogueContentPlanSeed, RuntimeFlowOpSeed, RuntimeFlowSeed,
            RuntimeLineId, RuntimePlanBuilder,
        };
        use arcweft_id::TextKey;
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
        use arcweft_text_model::{
            DialogueContentCatalog, DialogueContentSpec, RichTextDocument, RichTextNode,
        };

        let line = RuntimeLineId::from_runtime_line_value("line.opening").expect("runtime line id");
        let expected_line = line.canonical_label().clone();
        let flow = FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id");
        let mut builder = RuntimePlanBuilder::new();
        let content = builder
            .push_dialogue_content_seed(RuntimeDialogueContentPlanSeed {
                line: line.clone(),
                values: Box::default(),
                marks: Box::default(),
            })
            .expect("dialogue content admits");
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                [],
                vec![
                    RuntimeFlowOpSeed::Dialogue { content },
                    RuntimeFlowOpSeed::Return("done".to_owned()),
                ],
            ))
            .expect("flow admits");
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("flow schema admits");
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow: flow.clone(),
                contract: FlowContractHash::from_bytes([0x71; 32]),
                controller: None,
            })
            .expect("flow executable admits");
        builder.push_entry(cli_main_entry()).expect("entry admits");
        let plan = builder.finish().expect("runtime plan is valid");
        let source_map = source_map("bundle-display.arcw", "flow main { dialogue }");
        let source = source_map
            .primary_document()
            .expect("fixture source map retains its source")
            .product_source_ref();
        let dialogue_content =
            DialogueContentCatalog::try_from_records(vec![DialogueContentSpec::new(
                line,
                TextKey::try_new("text.bundle.opening").expect("text key"),
                RichTextDocument::new(vec![RichTextNode::Text {
                    text: "Hello bundle".to_owned(),
                }]),
                crate::test_character_plan(),
                arcweft_text_model::DialoguePresentationSnapshot::new(
                    crate::test_dialogue_profile(),
                    crate::test_dialogue_profile_revision(),
                ),
                Vec::new(),
                source,
            )])
            .expect("test dialogue content is canonical");
        let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "bundle-display.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bundle = ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 2,
                    line_task_groups: 1,
                    stream_plans: 0,
                },
            },
            source_map,
            product_awbc,
            dialogue_content,
        )
        .expect("standard dialogue source joins source map");

        let error = run_bundle_headless(&bundle, 8)
            .expect_err("dialogue cannot run before checked presentation is bundled");
        assert!(matches!(
            error,
            NativePlayerError::DialoguePresentationUnavailable { line } if line == expected_line
        ));
    }

    #[cfg(not(feature = "dev-capture"))]
    #[test]
    fn headless_report_json_omits_capture_metadata_without_dev_capture() {
        let report = run_bundle_headless(&return_only_bundle(), 8).expect("bundle runs");
        let json = serde_json::to_value(&report).expect("report serializes");

        assert!(json.get("native_capture").is_none());
    }

    #[cfg(not(feature = "dev-capture"))]
    fn return_only_bundle() -> ArcweftBundle {
        use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
        use arcweft_core::entry::{FlowContractHash, RuntimeFlowExecutable, RuntimeFlowSchema};
        use arcweft_core::plan::{
            FlowRuntimeId, RuntimeFlowOpSeed, RuntimeFlowSeed, RuntimePlanBuilder,
        };
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

        let flow = FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id");
        let mut builder = RuntimePlanBuilder::new();
        builder
            .push_flow_seed(RuntimeFlowSeed::new(
                flow.clone(),
                [],
                vec![RuntimeFlowOpSeed::Return("done".to_owned())],
            ))
            .expect("flow admits");
        builder
            .push_flow_schema(RuntimeFlowSchema {
                flow: flow.clone(),
                parameters: Vec::new(),
            })
            .expect("flow schema admits");
        builder
            .push_flow_executable(RuntimeFlowExecutable {
                flow: flow.clone(),
                contract: FlowContractHash::from_bytes([0x72; 32]),
                controller: None,
            })
            .expect("flow executable admits");
        builder.push_entry(cli_main_entry()).expect("entry admits");
        let plan = builder.finish().expect("runtime plan is valid");
        let dialogue_content = arcweft_text_model::DialogueContentCatalog::new();
        let product_awbc = AwbcLowerer::new(&plan, &dialogue_content, "return-only.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        ArcweftBundle::try_new(
            BundleManifest {
                profile_id: None,
                profile_kind: None,
                entry: Some("entry.main".to_owned()),
                adapter: None,
                adapter_manifest_ids: Vec::new(),
                required_host_calls: Vec::new(),
                runtime: BundleRuntimeSummary {
                    artifact_fingerprint: fixture_runtime_artifact_fingerprint(),
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 1,
                    line_task_groups: 0,
                    stream_plans: 0,
                },
            },
            source_map("return-only.arcw", "flow main { return \"done\" }"),
            product_awbc,
            dialogue_content,
        )
        .expect("standard dialogue source joins source map")
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
                arcweft_core::plan::FlowRuntimeId::from_runtime_target_value("flow.main")
                    .expect("test flow ID is valid"),
            ),
            roles: arcweft_core::entry::RuntimeEntryRoles::None,
        }
    }

    #[cfg(feature = "dev-capture")]
    #[test]
    fn headless_report_serializes_capture_metadata_with_dev_capture() {
        let report = HeadlessPlayerReport {
            frames: Vec::new(),
            diagnostics: Vec::new(),
            steps: 0,
            status: "done".to_owned(),
            runtime: None,
            native_capture: Some(NativePlayerCaptureMetadata {
                renderer: "shared_offscreen_wgpu".to_owned(),
                format: "png".to_owned(),
                width: 1,
                height: 1,
                pixel_format: "rgba8_unorm_srgb".to_owned(),
                row_stride_bytes: 4,
                content_bbox: Some(NativePlayerCaptureContentBBox {
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                }),
                content_pixels: 1,
                written: "capture.png".to_owned(),
            }),
        };
        let json = serde_json::to_value(&report).expect("report serializes");

        assert!(json.get("native_capture").is_some());
    }
}
