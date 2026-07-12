//! Development-only full player-frame capture through the shared renderer.

use arcweft_bundle::ArcweftBundle;
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    fonts::{PlayerFontRegistrationError, PlayerFontSet},
    frame::{
        PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
        PlayerPreparedFrame,
    },
    images::{BundleImageCatalog, BundleImageCatalogError},
    input::InputController,
};
use arcweft_render_wgpu::{
    geometry::{PreparedFrame, RenderPreferences, RenderViewport},
    offscreen::{
        CaptureAttachment, CaptureRequest, SharedOffscreenCapture, SharedOffscreenCaptureError,
    },
};
use arcweft_runtime_driver::{
    clock::{RuntimeClockError, RuntimeClockStep},
    session::{BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput},
};
use num_traits::ToPrimitive;
use serde::Serialize;
use thiserror::Error;

const CAPTURE_STEP_MILLIS: u32 = 16;

/// Pixel-space bounds containing every pixel different from the renderer clear color.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct NativePlayerCaptureContentBBox {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Inputs for one deterministic development capture.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativePlayerCaptureRequest {
    pub width: u32,
    pub height: u32,
    pub max_steps: usize,
}

impl NativePlayerCaptureRequest {
    #[must_use]
    pub const fn new(width: u32, height: u32, max_steps: usize) -> Self {
        Self {
            width,
            height,
            max_steps,
        }
    }
}

/// Unpadded RGBA8 pixels and diagnostic content statistics from the shared renderer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativePlayerFrameCapture {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub content_bbox: Option<NativePlayerCaptureContentBBox>,
    pub content_pixels: u64,
}

/// Failure while preparing or rendering a development capture.
#[derive(Debug, Error)]
pub enum NativePlayerCaptureError {
    #[error(transparent)]
    Session(#[from] BundleSessionError),
    #[error(transparent)]
    ImageCatalog(#[from] BundleImageCatalogError),
    #[error(transparent)]
    Clock(#[from] RuntimeClockError),
    #[error(transparent)]
    Frame(#[from] PlayerFrameError),
    #[error(transparent)]
    Font(#[from] Box<PlayerFontRegistrationError>),
    #[error(transparent)]
    Offscreen(#[from] SharedOffscreenCaptureError),
    #[error("shared offscreen capture did not return its requested color attachment")]
    MissingColorAttachment,
    #[error("no renderable player frame was produced within {max_steps} runtime steps")]
    NoRenderableFrame { max_steps: usize },
}

impl From<PlayerFontRegistrationError> for NativePlayerCaptureError {
    fn from(error: PlayerFontRegistrationError) -> Self {
        Self::Font(Box::new(error))
    }
}

/// Captures the first renderable bundle presentation through the normal player-frame path.
pub fn capture_bundle_frame(
    bundle: &ArcweftBundle,
    request: NativePlayerCaptureRequest,
) -> Result<NativePlayerFrameCapture, NativePlayerCaptureError> {
    let (prepared, fonts) = prepare_bundle_frame(bundle, request)?;
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))?;
    fonts.register_with_offscreen_capture(&mut capture)?;
    let captured = capture.capture(&prepared.frame, &CaptureRequest::whole_frame_color())?;
    let rgba = captured
        .attachment_rgba(CaptureAttachment::Color)
        .ok_or(NativePlayerCaptureError::MissingColorAttachment)?
        .to_vec();
    let stats = capture_content_stats(&rgba, captured.width, captured.height);
    Ok(NativePlayerFrameCapture {
        width: captured.width,
        height: captured.height,
        rgba,
        content_bbox: stats.content_bbox,
        content_pixels: stats.content_pixels,
    })
}

fn prepare_bundle_frame(
    bundle: &ArcweftBundle,
    request: NativePlayerCaptureRequest,
) -> Result<(PlayerPreparedFrame, PlayerFontSet), NativePlayerCaptureError> {
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let fonts = PlayerFontSet::bundled_default();
    let mut planner = PlayerFramePlannerState::new();
    fonts.register_with_planner(&mut planner)?;
    let mut input = InputController::default();
    let viewport = capture_viewport(request);
    let mut first_visual_frame = None;

    for step_index in 0..request.max_steps {
        let tick = u64::try_from(step_index)
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let clock = RuntimeClockStep::from_millis(tick, CAPTURE_STEP_MILLIS)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let visual_time_millis = tick.saturating_mul(u64::from(CAPTURE_STEP_MILLIS));
        let prepared = planner.prepare(
            &mut input,
            PlayerFrameRequest {
                presentation: &step.presentation,
                fx_definitions: session.fx_definitions(),
                images: &images,
                viewport,
                fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
                image_time_millis: visual_time_millis,
                visual_time_millis,
                dialogue_reveal_complete: true,
                preferences: RenderPreferences::default(),
            },
        )?;
        if step.presentation.textboxes.latest_active().is_some() {
            return Ok((prepared, fonts));
        }
        if first_visual_frame.is_none() && frame_has_visual_content(&prepared.frame) {
            first_visual_frame = Some(prepared);
        }
        if step.finished {
            break;
        }
    }

    first_visual_frame.map(|prepared| (prepared, fonts)).ok_or(
        NativePlayerCaptureError::NoRenderableFrame {
            max_steps: request.max_steps,
        },
    )
}

fn capture_viewport(request: NativePlayerCaptureRequest) -> RenderViewport {
    let width = request.width.max(1);
    let height = request.height.max(1);
    RenderViewport {
        logical_width: width.to_f32().unwrap_or(f32::MAX),
        logical_height: height.to_f32().unwrap_or(f32::MAX),
        physical_width: width,
        physical_height: height,
        scale_factor: 1.0,
    }
}

fn frame_has_visual_content(frame: &PreparedFrame) -> bool {
    !frame.rectangles.is_empty()
        || !frame.images.is_empty()
        || !frame.text.is_empty()
        || !frame.choices.is_empty()
        || !frame.action_buttons.is_empty()
        || !frame.control_paints.is_empty()
        || !frame.view_scenes().is_empty()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CaptureContentStats {
    content_bbox: Option<NativePlayerCaptureContentBBox>,
    content_pixels: u64,
}

fn capture_content_stats(rgba: &[u8], width: u32, height: u32) -> CaptureContentStats {
    if width == 0 || height == 0 {
        return CaptureContentStats {
            content_bbox: None,
            content_pixels: 0,
        };
    }
    let width_usize = usize::try_from(width).unwrap_or(0);
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut content_pixels = 0_u64;
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        if pixel == [0, 0, 0, 255] {
            continue;
        }
        let x = u32::try_from(index % width_usize).unwrap_or(u32::MAX);
        let y = u32::try_from(index / width_usize).unwrap_or(u32::MAX);
        if x >= width || y >= height {
            continue;
        }
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        content_pixels = content_pixels.saturating_add(1);
    }
    CaptureContentStats {
        content_bbox: (content_pixels > 0).then_some(NativePlayerCaptureContentBBox {
            x: min_x,
            y: min_y,
            width: max_x.saturating_sub(min_x).saturating_add(1),
            height: max_y.saturating_sub(min_y).saturating_add(1),
        }),
        content_pixels,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::{BundleManifest, BundleRuntimeSummary, BundleSource};
    use arcweft_core::{
        bytecode::BytecodeProgram,
        line_task::LineTaskGroup,
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan},
    };
    use arcweft_render_text::{
        LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode,
    };
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

    #[test]
    fn content_stats_are_derived_from_shared_capture_pixels() {
        let rgba = [
            0, 0, 0, 255, 1, 2, 3, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 4, 5, 6, 255,
        ];

        assert_eq!(
            capture_content_stats(&rgba, 3, 2),
            CaptureContentStats {
                content_bbox: Some(NativePlayerCaptureContentBBox {
                    x: 1,
                    y: 0,
                    width: 2,
                    height: 2,
                }),
                content_pixels: 2,
            }
        );
    }

    #[test]
    fn empty_capture_has_no_content_bounds() {
        assert_eq!(
            capture_content_stats(&[0, 0, 0, 255], 1, 1),
            CaptureContentStats {
                content_bbox: None,
                content_pixels: 0,
            }
        );
    }

    #[test]
    fn capture_preparation_uses_the_normal_prepared_text_batch() {
        let bundle = dialogue_bundle();

        let (prepared, _) =
            prepare_bundle_frame(&bundle, NativePlayerCaptureRequest::new(640, 360, 8))
                .expect("dialogue frame prepares");

        assert_eq!(prepared.frame.textboxes().len(), 1);
        assert!(!prepared.frame.text.is_empty());
    }

    fn dialogue_bundle() -> ArcweftBundle {
        let line = RuntimeLineId::from_runtime_line_value("line.capture")
            .expect("runtime line id is valid");
        let plan = RuntimePlan::new(
            Some(FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow id is valid")),
            vec![RuntimeFlow {
                id: FlowRuntimeId::from_runtime_target_value("flow.main")
                    .expect("flow id is valid"),
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
            callee: "narrator".to_owned(),
            speaker_label: None,
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
                text: "shared prepared capture".to_owned(),
            }]),
        }]);
        let product_awbc = AwbcLowerer::new(&plan, &display, "capture.arcw")
            .lower()
            .expect("product AWBC lowers")
            .program;
        ArcweftBundle::new(
            BundleManifest {
                source_label: "capture.arcw".to_owned(),
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
                label: "capture.arcw".to_owned(),
                text: "flow @flow.main main { dialogue }".to_owned(),
            },
            BytecodeProgram::from_runtime_plan(plan),
            display,
        )
        .with_product_awbc(product_awbc)
    }
}
