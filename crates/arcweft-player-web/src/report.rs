use arcweft_render_wgpu::geometry::{PreparedFrame, RenderImage, RenderTextBlock};
use arcweft_runtime_driver::session::BundleSessionStep;
use num_traits::ToPrimitive;
use serde::{Deserialize, Serialize};

/// Path-free diagnostic/observation envelope emitted to JavaScript.
///
/// It is not a render protocol and contains no DOM construction instructions.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebObservationReport {
    pub schema_version: String,
    pub step_index: usize,
    pub logical_tick: u64,
    pub logical_dt_millis: u64,
    pub stop_reason: String,
    pub status: String,
    pub finished: bool,
    pub diagnostics: Vec<String>,
    pub presentation_revision: u64,
    pub dialogue_present: bool,
    pub choice_count: usize,
    pub image_count: usize,
    pub flow_event_count: usize,
    pub requested_tasks: usize,
    pub queued_task_events: usize,
}

impl WebObservationReport {
    pub fn from_step(step: &BundleSessionStep, queued_task_events: usize) -> Self {
        Self {
            schema_version: "arcweft.web_observation.v2".to_owned(),
            step_index: step.index,
            logical_tick: step.clock.tick().0,
            logical_dt_millis: step.clock.dt_millis(),
            stop_reason: step.stop_reason_label.clone(),
            status: step.status_label.clone(),
            finished: step.finished,
            diagnostics: step.diagnostics.clone(),
            presentation_revision: step.presentation.revision,
            dialogue_present: step.presentation.dialogue.is_some(),
            choice_count: step.presentation.choices.len(),
            image_count: step.presentation.images.len(),
            flow_event_count: step.flow_events.len(),
            requested_tasks: step.requested_tasks.len(),
            queued_task_events,
        }
    }
}

/// Host-neutral summary of the canvas frame produced by the shared planner.
///
/// This is diagnostic evidence for native/Web parity. It is not a renderer
/// command stream and intentionally carries normalized scalar values instead of
/// platform resources.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameObservationReport {
    pub schema_version: String,
    pub viewport: WebFrameViewport,
    pub rectangle_count: usize,
    pub image_count: usize,
    pub text_count: usize,
    pub choice_count: usize,
    pub images: Vec<WebFrameImage>,
    pub text: Vec<WebFrameText>,
    pub choices: Vec<WebFrameChoice>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameViewport {
    pub logical_width_milli: i64,
    pub logical_height_milli: i64,
    pub physical_width: u32,
    pub physical_height: u32,
    pub scale_factor_milli: i64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameBounds {
    pub x_milli: i64,
    pub y_milli: i64,
    pub width_milli: i64,
    pub height_milli: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameImage {
    pub id: String,
    pub bounds: WebFrameBounds,
    pub frame_width: u32,
    pub frame_height: u32,
    pub opacity_milli: u16,
    pub rgba_hash: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameText {
    pub text: String,
    pub bounds: WebFrameBounds,
    pub font_size_milli: i64,
    pub line_height_milli: i64,
    pub rgba: [u8; 4],
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WebFrameChoice {
    pub option_id: String,
    pub label: String,
    pub target: String,
    pub bounds: WebFrameBounds,
}

impl WebFrameObservationReport {
    pub fn from_prepared_frame(frame: &PreparedFrame) -> Self {
        Self {
            schema_version: "arcweft.web_frame_observation.v1".to_owned(),
            viewport: WebFrameViewport {
                logical_width_milli: f64_milli(f64::from(frame.viewport.logical_width)),
                logical_height_milli: f64_milli(f64::from(frame.viewport.logical_height)),
                physical_width: frame.viewport.physical_width,
                physical_height: frame.viewport.physical_height,
                scale_factor_milli: f64_milli(frame.viewport.scale_factor),
            },
            rectangle_count: frame.rectangles.len(),
            image_count: frame.images.len(),
            text_count: frame.text.len(),
            choice_count: frame.choices.len(),
            images: frame
                .images
                .iter()
                .map(WebFrameImage::from_render_image)
                .collect(),
            text: frame
                .text
                .iter()
                .map(WebFrameText::from_text_block)
                .collect(),
            choices: frame
                .choices
                .iter()
                .filter_map(|choice| {
                    frame
                        .semantics
                        .find(&choice.target)
                        .map(|node| WebFrameChoice {
                            option_id: choice.option_id.clone(),
                            label: choice.label.clone(),
                            target: choice.target.id().as_str().to_owned(),
                            bounds: WebFrameBounds::from_hit_rect(node.bounds()),
                        })
                })
                .collect(),
        }
    }
}

impl WebFrameBounds {
    fn from_hit_rect(bounds: arcweft_presentation::hit::HitRect) -> Self {
        Self {
            x_milli: f32_milli(bounds.x),
            y_milli: f32_milli(bounds.y),
            width_milli: f32_milli(bounds.width),
            height_milli: f32_milli(bounds.height),
        }
    }
}

impl WebFrameImage {
    fn from_render_image(image: &RenderImage) -> Self {
        Self {
            id: image.id.clone(),
            bounds: WebFrameBounds::from_hit_rect(image.bounds),
            frame_width: image.frame.width,
            frame_height: image.frame.height,
            opacity_milli: image.opacity_milli,
            rgba_hash: stable_hash(&image.frame.rgba),
        }
    }
}

impl WebFrameText {
    fn from_text_block(text: &RenderTextBlock) -> Self {
        Self {
            text: text.text.clone(),
            bounds: WebFrameBounds::from_hit_rect(text.bounds),
            font_size_milli: f32_milli(text.font_size),
            line_height_milli: f32_milli(text.line_height),
            rgba: text.rgba,
        }
    }
}

fn f32_milli(value: f32) -> i64 {
    f64_milli(f64::from(value))
}

fn f64_milli(value: f64) -> i64 {
    let scaled = (value * 1_000.0).round();
    if !scaled.is_finite() {
        return 0;
    }
    scaled.to_i64().unwrap_or_else(|| {
        if scaled.is_sign_negative() {
            i64::MIN
        } else {
            i64::MAX
        }
    })
}

fn stable_hash(bytes: &[u8]) -> String {
    let hash = bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    });
    format!("{hash:016x}")
}
