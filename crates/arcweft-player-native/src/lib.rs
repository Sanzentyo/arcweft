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
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame, RuntimeLineContext};
use arcweft_runtime_host::{
    BundleRunnerExecutor, BundleRunnerOptions, BundleRunnerStepMode, NativeTaskStats,
    RuntimeExecutorStats, run_bundle_with_native_adapters,
};
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
    pub executor: BundleRunnerExecutor,
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
            executor: BundleRunnerExecutor::AwbcProduct,
            ..BundleRunnerOptions::default()
        },
        &[desktop_native_adapter_registrar],
    )?;
    let mut frames = Vec::new();
    let mut diagnostics = Vec::new();
    for step in &runner.steps {
        diagnostics.extend(step.diagnostics.iter().cloned());
        append_display_frames(
            &bundle.display,
            &step.flow_events,
            &mut frames,
            &mut diagnostics,
        );
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
            executor: runner.executor,
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

fn append_display_frames(
    catalog: &LineDisplayCatalog,
    events: &[FlowEvent],
    frames: &mut Vec<LineDisplayFrame>,
    diagnostics: &mut Vec<String>,
) {
    for event in events {
        match event {
            FlowEvent::DialogueLine { line, bindings } => {
                if let Some(spec) = catalog.find(line) {
                    match spec.resolve_frame(&RuntimeLineContext::new(bindings.clone())) {
                        Ok(frame) => frames.push(frame),
                        Err(error) => diagnostics.push(error.to_string()),
                    }
                }
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_dialogue::{DialogueProfileRevision, InlineFailurePolicy};
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

    fn test_dialogue_revision() -> DialogueProfileRevision {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("player-native-lib-test").expect("document ID"),
            SourceName::Memory,
            "test manifest",
        )
        .expect("test document");
        let sources = SourceSetRevision::try_for_identities([manifest.identity()])
            .expect("test source revision");
        DialogueProfileRevision::from_admitted_parts(
            manifest.identity().clone(),
            sources,
            sources,
            ViewProgramId::try_new("view_program.player-native-lib-test").expect("View program ID"),
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
            ResourceTypeRegistry::empty().digest(),
        )
    }

    #[test]
    fn bundle_headless_uses_runtime_host_flow_events_for_display_frames() {
        use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
        use arcweft_core::bytecode::BytecodeProgram;
        use arcweft_core::line_task::LineTaskGroup;
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
        use arcweft_render_text::{LineDisplaySpec, RichTextDocument, RichTextNode};
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

        let line = RuntimeLineId::from_runtime_line_value("line.opening").expect("runtime line id");
        let expected_status = format!("dialogue {}", line.canonical_label());
        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
                ops: vec![
                    FlowOp::Dialogue {
                        line: line.clone(),
                        task_group: 0,
                    },
                    FlowOp::Return("done".to_owned()),
                ],
            }],
            vec![LineTaskGroup::default()],
        )
        .expect("runtime plan is valid")
        .with_entries(vec![cli_main_entry()]);
        let display = LineDisplayCatalog::try_from_lines(
            test_dialogue_revision(),
            vec![LineDisplaySpec {
                line,
                callee: "alice".to_owned(),
                speaker_label: None,
                text_key: None,
                view: arcweft_bundle::standard_view::dialogue_view_id(),
                profile_style: None,
                dialogue_revision: test_dialogue_revision(),
                voice: None,
                look: None,
                style: None,
                base_styles: Vec::new(),
                inline_failure: InlineFailurePolicy::FailLine,
                style_contributions: Vec::new(),
                args: Vec::new(),
                content: RichTextDocument::new(vec![RichTextNode::Text {
                    text: "Hello bundle".to_owned(),
                }]),
            }],
        )
        .expect("test display catalog is revision-consistent");
        let product_awbc = AwbcLowerer::new(&plan, &display, "bundle-display.arcw")
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
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 2,
                    line_task_groups: 1,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("bundle-display.arcw", "flow main { dialogue }"),
            BytecodeProgram::from_runtime_plan(plan),
            display,
        )
        .expect("standard dialogue source joins source map")
        .with_product_awbc(product_awbc);

        let report = run_bundle_headless(&bundle, 8).expect("bundle runs through runtime host");

        assert_eq!(report.status, expected_status);
        assert!((1..=8).contains(&report.steps));
        assert_eq!(report.frames.len(), 1);
        assert_eq!(report.frames[0].text, "Hello bundle");
        let runtime = report
            .runtime
            .as_ref()
            .expect("bundle player report includes runtime metadata");
        assert_eq!(runtime.source, "bundle-display.arcw");
        assert_eq!(runtime.bytecode_instructions, 2);
        assert_eq!(runtime.adapter_manifests, 0);
        assert_eq!(runtime.executor, BundleRunnerExecutor::AwbcProduct);
        assert_eq!(runtime.native_io.scheduler.submitted, 0);
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
        use arcweft_core::bytecode::BytecodeProgram;
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

        let plan = RuntimePlan::new(
            vec![RuntimeFlow {
                id: FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow runtime id"),
                ops: vec![FlowOp::Return("done".to_owned())],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid")
        .with_entries(vec![cli_main_entry()]);
        let display = LineDisplayCatalog::new(test_dialogue_revision());
        let product_awbc = AwbcLowerer::new(&plan, &display, "return-only.arcw")
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
                    entry_flow: Some("flow.main".to_owned()),
                    flows: 1,
                    bytecode_instructions: 1,
                    line_task_groups: 0,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("return-only.arcw", "flow main { return \"done\" }"),
            BytecodeProgram::from_runtime_plan(plan),
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
