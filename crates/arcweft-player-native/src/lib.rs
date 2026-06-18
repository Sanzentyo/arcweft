//! Native/headless rich-text player host for Arcweft.

use arcweft_bundle::ArcweftBundle;
#[cfg(feature = "dev-source")]
use arcweft_compiler::compile_source as compile_arcweft_source;
use arcweft_core::engine::{Engine, FlowFiberStatus};
use arcweft_core::plan::FlowEvent;
use arcweft_core::step::{
    RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStopReason,
};
#[cfg(feature = "dev-capture")]
use arcweft_render_native::NativeFrameContentBBox;
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame, RuntimeLineContext};
use serde::Serialize;
use thiserror::Error;

/// Compiled native-player program.
#[derive(Clone, Debug, PartialEq)]
pub struct NativePlayerProgram {
    plan: arcweft_core::plan::RuntimePlan,
    display: LineDisplayCatalog,
}

/// Headless player report used by tests and CLI automation.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HeadlessPlayerReport {
    pub frames: Vec<LineDisplayFrame>,
    pub diagnostics: Vec<String>,
    pub steps: usize,
    pub status: String,
    #[cfg(feature = "dev-capture")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_capture: Option<NativePlayerCaptureMetadata>,
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
    #[cfg(feature = "dev-source")]
    #[error(transparent)]
    Compile(#[from] arcweft_compiler::CompileSourceError),
    #[error("failed to decode bundle bytecode: {0}")]
    DecodeBytecode(arcweft_core::plan::RuntimePlanError),
    #[error("no display frame was produced")]
    NoDisplayFrame,
}

/// Compiles source into runtime code plus a line display catalog.
#[cfg(feature = "dev-source")]
pub fn compile_source(source: &str) -> Result<NativePlayerProgram, NativePlayerError> {
    let compiled = compile_arcweft_source(source)?;
    Ok(NativePlayerProgram {
        plan: compiled.plan,
        display: compiled.display,
    })
}

/// Loads a compiled bundle into the native player without invoking source compilation.
pub fn load_bundle(bundle: ArcweftBundle) -> Result<NativePlayerProgram, NativePlayerError> {
    Ok(NativePlayerProgram {
        plan: bundle
            .bytecode
            .program
            .into_runtime_plan()
            .map_err(NativePlayerError::DecodeBytecode)?,
        display: bundle.display,
    })
}

/// Runs the program without opening a window and returns resolved display frames.
pub fn run_headless(
    program: NativePlayerProgram,
    max_steps: usize,
) -> Result<HeadlessPlayerReport, NativePlayerError> {
    let mut engine = Engine::new(program.plan);
    let options = RuntimeStepOptions {
        mode: RuntimeStepMode::Game,
        budget: arcweft_core::step::RuntimeStepBudget { max_ops: 64 },
    };
    let mut frames = Vec::new();
    let mut diagnostics = Vec::new();
    let mut steps = 0;
    for index in 0..max_steps {
        let result = engine.step(RuntimeStepInput::default(), options);
        steps = index + 1;
        diagnostics.extend(
            result
                .output
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.message.clone()),
        );
        append_display_frames(
            &program.display,
            &result.output.flow_events,
            &mut frames,
            &mut diagnostics,
        );
        if matches!(
            result.stop_reason,
            RuntimeStepStopReason::Done | RuntimeStepStopReason::Failed
        ) || matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            return Ok(HeadlessPlayerReport {
                frames,
                diagnostics,
                steps,
                status: flow_status_label(&engine.fiber().status),
                #[cfg(feature = "dev-capture")]
                native_capture: None,
            });
        }
    }
    Ok(HeadlessPlayerReport {
        frames,
        diagnostics,
        steps,
        status: flow_status_label(&engine.fiber().status),
        #[cfg(feature = "dev-capture")]
        native_capture: None,
    })
}

/// Compiles and runs source until the first display frame is available.
#[cfg(feature = "dev-source")]
pub fn first_display_frame(source: &str) -> Result<LineDisplayFrame, NativePlayerError> {
    let program = compile_source(source)?;
    let report = run_headless(program, 64)?;
    report
        .frames
        .into_iter()
        .next()
        .ok_or(NativePlayerError::NoDisplayFrame)
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

fn flow_status_label(status: &FlowFiberStatus) -> String {
    match status {
        FlowFiberStatus::Running => "running".to_owned(),
        FlowFiberStatus::Waiting(_) => "waiting".to_owned(),
        FlowFiberStatus::WaitingMany(_) => "waiting_many".to_owned(),
        FlowFiberStatus::Choice(_) => "choice".to_owned(),
        FlowFiberStatus::Done(exit) => format!("done:{exit:?}"),
        FlowFiberStatus::Failed(message) => format!("failed:{message}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headless_player_runs_program_without_source_compiler() {
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};

        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid");
        let report = run_headless(
            NativePlayerProgram {
                plan,
                display: LineDisplayCatalog::default(),
            },
            8,
        )
        .expect("program runs");

        assert_eq!(report.status, "done:Return(\"done\")");
    }

    #[cfg(not(feature = "dev-capture"))]
    #[test]
    fn headless_report_json_omits_capture_metadata_without_dev_capture() {
        use arcweft_core::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};

        let plan = RuntimePlan::new(
            Some(FlowRuntimeId("flow.main".to_owned())),
            vec![RuntimeFlow {
                id: FlowRuntimeId("flow.main".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            }],
            Vec::new(),
        )
        .expect("runtime plan is valid");
        let report = run_headless(
            NativePlayerProgram {
                plan,
                display: LineDisplayCatalog::default(),
            },
            8,
        )
        .expect("program runs");
        let json = serde_json::to_value(&report).expect("report serializes");

        assert!(json.get("native_capture").is_none());
    }

    #[cfg(feature = "dev-capture")]
    #[test]
    fn headless_report_serializes_capture_metadata_with_dev_capture() {
        let report = HeadlessPlayerReport {
            frames: Vec::new(),
            diagnostics: Vec::new(),
            steps: 0,
            status: "done".to_owned(),
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

    #[cfg(feature = "dev-source")]
    #[test]
    fn headless_player_resolves_rich_text_frame() {
        use arcweft_render_text::{DialogueHostEvent, RichTextNode};

        let source = r#"
character @character.alice Alice as alice {}

flow @flow.main main {
    let player = "Aoi"
    alice: Hello #[player] |[夢](ゆめ)[r][em:quiet][voice auto][face smile][signal .seen][p]
}
"#;

        let frame = first_display_frame(source).expect("frame");

        assert!(frame.text.contains("Hello Aoi"));
        assert!(frame.text.contains("夢"));
        assert!(frame.nodes.iter().any(|node| {
            matches!(
                node,
                RichTextNode::Ruby { base, ruby } if base == "夢" && ruby == "ゆめ"
            )
        }));
        assert!(frame.host_events.iter().any(|event| {
            matches!(event, DialogueHostEvent::Voice { attrs } if attrs == "auto")
        }));
        assert!(frame.host_events.iter().any(|event| {
            matches!(event, DialogueHostEvent::Face { attrs } if attrs == "smile")
        }));
    }
}
