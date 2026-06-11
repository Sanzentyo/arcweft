//! Native/headless rich-text player adapter for Arcweft.

pub mod native;

use crate::native::NativeFrameContentBBox;
use arcweft_core::engine::{Engine, FlowFiberStatus};
use arcweft_core::plan::FlowEvent;
use arcweft_core::step::{
    RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepStopReason,
};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::check::analyze_types;
use arcweft_lang_sema::env::TypeCheckEnv;
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{LineDisplayCatalog, LineDisplayFrame, RuntimeLineContext};
use arcweft_runtime_plan::flow::lower_runtime_plan_with_stats;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_capture: Option<NativePlayerCaptureMetadata>,
}

/// Metadata for a native offscreen framebuffer capture emitted by the player binary.
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
    #[error("parse errors: {0:?}")]
    Parse(Vec<arcweft_lang_syntax::parser::recovery::ParseError>),
    #[error("HIR lowering errors: {0:?}")]
    Hir(Vec<arcweft_lang_hir::model::HirLowerError>),
    #[error("type errors: {0:?}")]
    Type(Vec<arcweft_lang_sema::diagnostics::TypeCheckError>),
    #[error("runtime-plan lowering errors: {0:?}")]
    RuntimePlan(Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>),
    #[error("no display frame was produced")]
    NoDisplayFrame,
}

/// Compiles source into runtime code plus a line display catalog.
pub fn compile_source(source: &str) -> Result<NativePlayerProgram, NativePlayerError> {
    let parsed = parse_source(source.to_owned());
    if !parsed.errors().is_empty() {
        return Err(NativePlayerError::Parse(parsed.errors().to_vec()));
    }
    let hir = lower_to_hir(parsed.typed_tree()).map_err(NativePlayerError::Hir)?;
    let type_report = analyze_types(&hir, &TypeCheckEnv::standard());
    type_report.into_result().map_err(NativePlayerError::Type)?;
    let report = lower_runtime_plan_with_stats(&hir).map_err(NativePlayerError::RuntimePlan)?;
    Ok(NativePlayerProgram {
        plan: report.plan,
        display: report.line_display_catalog,
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
                native_capture: None,
            });
        }
    }
    Ok(HeadlessPlayerReport {
        frames,
        diagnostics,
        steps,
        status: flow_status_label(&engine.fiber().status),
        native_capture: None,
    })
}

/// Compiles and runs source until the first display frame is available.
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
    use arcweft_render_text::{DialogueHostEvent, RichTextNode};

    #[test]
    fn headless_player_resolves_rich_text_frame() {
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
