//! Native/headless rich-text player host for Arcweft.

mod native_audio;
mod patch_endpoint;
mod scene_windowed;
mod window_driver;
mod windowed;
mod windowed_ingress;
pub mod windowed_patch;
mod windowed_runtime;

pub use patch_endpoint::{
    NativePatchEndpoint, NativePatchEndpointError, NativePatchOutcome, NativePatchTransportAction,
    NativePatchTransportEnvelope,
};
pub use scene_windowed::{run_bundle_windowed, run_bundle_windowed_with_ingress};
pub use windowed::run_bundle_windowed as run_bundle_adapter_windowed;
pub use windowed_ingress::{
    WindowedLocalSidecar, WindowedPatchIngress, WindowedPatchIngressError,
    WindowedPatchIngressReport, WindowedPatchIngressState, WindowedPatchTransportActionSet,
};
pub use windowed_runtime::{
    WindowedRuntimeOutcome, WindowedRuntimeOwner, WindowedRuntimeOwnerError,
};

use arcweft_bundle::ArcweftBundle;
use arcweft_core::plan::FlowEvent;
#[cfg(feature = "dev-capture")]
use arcweft_render_native::NativeFrameContentBBox;
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
    pub content_bbox: Option<NativeFrameContentBBox>,
    pub content_pixels: u64,
    pub written: String,
}

/// Native player error.
#[derive(Debug, Error)]
pub enum NativePlayerError {
    #[error(transparent)]
    BundleRunner(#[from] arcweft_runtime_host::BundleRunnerError),
    #[error(transparent)]
    NativeWindow(#[from] arcweft_render_native::NativeWindowError),
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

    #[test]
    fn bundle_headless_uses_runtime_host_flow_events_for_display_frames() {
        use arcweft_bundle::{BundleManifest, BundleRuntimeSummary, BundleSource};
        use arcweft_core::bytecode::BytecodeProgram;
        use arcweft_core::line_task::LineTaskGroup;
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan};
        use arcweft_render_text::{LineDisplaySpec, RichTextDocument, RichTextNode};
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

        let line = RuntimeLineId("line.opening".to_owned());
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
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
        .expect("runtime plan is valid");
        let display = LineDisplayCatalog::new(vec![LineDisplaySpec {
            line,
            callee: "alice".to_owned(),
            text_key: None,
            window: None,
            voice: None,
            look: None,
            style: None,
            base_styles: Vec::new(),
            default_inline_failure_policy: None,
            style_contributions: Vec::new(),
            args: Vec::new(),
            content: RichTextDocument::new(vec![RichTextNode::Text {
                text: "Hello bundle".to_owned(),
            }]),
        }]);
        let product_awbc = AwbcLowerer::new(&plan, &display, "bundle-display.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        let bundle = ArcweftBundle::new(
            BundleManifest {
                source_label: "bundle-display.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: None,
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
            BundleSource {
                label: "bundle-display.arcw".to_owned(),
                text: "flow main { dialogue }".to_owned(),
            },
            BytecodeProgram::from_runtime_plan(plan),
            display,
        )
        .with_product_awbc(product_awbc);

        let report = run_bundle_headless(&bundle, 8).expect("bundle runs through runtime host");

        assert_eq!(report.status, "dialogue line.opening");
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
        use arcweft_bundle::{BundleManifest, BundleRuntimeSummary, BundleSource};
        use arcweft_core::bytecode::BytecodeProgram;
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
        use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid");
        let display = LineDisplayCatalog::default();
        let product_awbc = AwbcLowerer::new(&plan, &display, "return-only.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        ArcweftBundle::new(
            BundleManifest {
                source_label: "return-only.arcw".to_owned(),
                profile_id: None,
                profile_kind: None,
                entry: None,
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
            BundleSource {
                label: "return-only.arcw".to_owned(),
                text: "flow main { return \"done\" }".to_owned(),
            },
            BytecodeProgram::from_runtime_plan(plan),
            display,
        )
        .with_product_awbc(product_awbc)
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
                renderer: "native_offscreen_wgpu_glyphon".to_owned(),
                format: "png".to_owned(),
                width: 1,
                height: 1,
                pixel_format: "rgba8_unorm".to_owned(),
                row_stride_bytes: 4,
                content_bbox: Some(NativeFrameContentBBox {
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
