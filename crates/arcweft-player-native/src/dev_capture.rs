//! Development-only full player-frame capture through the shared renderer.

use arcweft_bundle::ArcweftBundle;
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    fonts::{PlayerFontRegistrationError, PlayerFontSet},
    frame::{
        PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
        PlayerPreparedFrame, PlayerPreparedFrameCandidate, ViewGeometryConsumer,
        ViewGeometryConversionError, ViewGeometryConversionField, ViewGeometryPlatform,
        ViewGeometryRuntimeError,
    },
    images::{BundleImageCatalog, BundleImageCatalogError},
    input::InputController,
};
use arcweft_render_wgpu::geometry::view_final::PreparedViewRenderCandidate;
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
    Frame(Box<PlayerFrameError>),
    #[error(transparent)]
    Font(#[from] Box<PlayerFontRegistrationError>),
    #[error(transparent)]
    Offscreen(#[from] SharedOffscreenCaptureError),
    #[error("shared offscreen capture did not return its requested color attachment")]
    MissingColorAttachment,
    #[error("no renderable player frame was produced within {max_steps} runtime steps")]
    NoRenderableFrame { max_steps: usize },
    #[error("capture extent must be non-zero, got {width}x{height}")]
    InvalidCaptureExtent { width: u32, height: u32 },
    #[error("capture step {step_index} cannot be represented as logical time")]
    StepIndexOverflow { step_index: usize },
    #[error("capture logical time overflow for tick {tick}")]
    CaptureTimeOverflow { tick: u64 },
    #[error(transparent)]
    GeometryConversion(#[from] ViewGeometryConversionError),
    #[error("captured RGBA length is {actual}, expected {expected}")]
    CapturedColorLength { expected: usize, actual: usize },
    #[error("captured content bounds overflow for {width}x{height}")]
    ContentBoundsOverflow { width: u32, height: u32 },
}

impl From<PlayerFrameError> for NativePlayerCaptureError {
    fn from(error: PlayerFrameError) -> Self {
        Self::Frame(Box::new(error))
    }
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
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let fonts = PlayerFontSet::bundled_default();
    let mut planner = PlayerFramePlannerState::new();
    fonts.register_with_planner(&mut planner)?;
    let mut input = InputController::default();
    let viewport = capture_viewport(request)?;
    let mut capture = pollster::block_on(SharedOffscreenCapture::new(
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ))?;
    fonts.register_with_offscreen_capture(&mut capture)?;
    let mut first_visual_capture = None;

    for step_index in 0..request.max_steps {
        let tick = u64::try_from(step_index)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(NativePlayerCaptureError::StepIndexOverflow { step_index })?;
        let clock = RuntimeClockStep::from_millis(tick, CAPTURE_STEP_MILLIS)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let visual_time_millis = tick
            .checked_mul(u64::from(CAPTURE_STEP_MILLIS))
            .ok_or(NativePlayerCaptureError::CaptureTimeOverflow { tick })?;
        let style_environment = session.presentation_environment();
        let candidate = planner.prepare_candidate(
            &input,
            PlayerFrameRequest {
                presentation: &step.presentation,
                fx_definitions: session.fx_definitions(),
                images: &images,
                style_program: session.view_style_program(),
                style_environment: &style_environment,
                style_palettes: session.view_style_palettes(),
                viewport,
                fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
                image_time_millis: visual_time_millis,
                visual_time_millis,
                dialogue_reveal_complete: true,
                preferences: RenderPreferences::default(),
            },
        )?;
        let dialogue_ready = step.presentation.dialogue.latest_active().is_some();
        let first_visual =
            first_visual_capture.is_none() && frame_has_visual_content(candidate.prepared());
        if dialogue_ready || first_visual {
            let publication = planner
                .publication_guard()
                .preflight_candidate(&candidate)?;
            let headless_candidate =
                HeadlessFramePublicationCandidate::prepare(&mut capture, &candidate)?;
            let (_, captured) = publication.publish_with(candidate, &mut input, |frame| {
                headless_candidate.commit(frame)
            })?;
            if dialogue_ready {
                return Ok(captured);
            }
            first_visual_capture = Some(captured);
        } else {
            planner
                .publication_guard()
                .publish_with(candidate, &mut input, |_| ())?;
        }
        if step.finished {
            break;
        }
    }

    first_visual_capture.ok_or(NativePlayerCaptureError::NoRenderableFrame {
        max_steps: request.max_steps,
    })
}

struct HeadlessFramePublicationCandidate {
    view_render: PreparedViewRenderCandidate,
    captured: NativePlayerFrameCapture,
}

impl HeadlessFramePublicationCandidate {
    fn prepare(
        capture: &mut SharedOffscreenCapture,
        candidate: &PlayerPreparedFrameCandidate,
    ) -> Result<Self, NativePlayerCaptureError> {
        let view_render = PreparedViewRenderCandidate::prepare(
            candidate.view_geometry().generation().value(),
            candidate
                .view_geometry()
                .final_nodes()
                .map(|(_, geometry)| geometry),
        )
        .map_err(ViewGeometryRuntimeError::from)
        .map_err(PlayerFrameError::from)?;
        let captured =
            capture.capture(candidate.prepared(), &CaptureRequest::whole_frame_color())?;
        let rgba = captured
            .attachment_rgba(CaptureAttachment::Color)
            .ok_or(NativePlayerCaptureError::MissingColorAttachment)?
            .to_vec();
        let stats = capture_content_stats(&rgba, captured.width, captured.height)?;
        Ok(Self {
            view_render,
            captured: NativePlayerFrameCapture {
                width: captured.width,
                height: captured.height,
                rgba,
                content_bbox: stats.content_bbox,
                content_pixels: stats.content_pixels,
            },
        })
    }

    fn commit(self, frame: &PlayerPreparedFrame) -> NativePlayerFrameCapture {
        debug_assert_eq!(
            self.view_render.generation(),
            frame.view_geometry().generation().value()
        );
        self.captured
    }
}

fn capture_viewport(
    request: NativePlayerCaptureRequest,
) -> Result<RenderViewport, NativePlayerCaptureError> {
    if request.width == 0 || request.height == 0 {
        return Err(NativePlayerCaptureError::InvalidCaptureExtent {
            width: request.width,
            height: request.height,
        });
    }
    Ok(RenderViewport {
        logical_width: capture_dimension(
            request.width,
            ViewGeometryConversionField::ViewportWidth,
        )?,
        logical_height: capture_dimension(
            request.height,
            ViewGeometryConversionField::ViewportHeight,
        )?,
        physical_width: request.width,
        physical_height: request.height,
        scale_factor: 1.0,
    })
}

fn capture_dimension(
    value: u32,
    field: ViewGeometryConversionField,
) -> Result<f32, ViewGeometryConversionError> {
    ViewGeometryConversionError::exact_f32(
        None,
        ViewGeometryPlatform::Headless,
        ViewGeometryConsumer::Capture,
        field,
        i64::from(value) * 1_000,
    )
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

fn capture_content_stats(
    rgba: &[u8],
    width: u32,
    height: u32,
) -> Result<CaptureContentStats, NativePlayerCaptureError> {
    if width == 0 || height == 0 {
        return Ok(CaptureContentStats {
            content_bbox: None,
            content_pixels: 0,
        });
    }
    let expected = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|bytes| usize::try_from(bytes).ok())
        .ok_or(NativePlayerCaptureError::ContentBoundsOverflow { width, height })?;
    if rgba.len() != expected {
        return Err(NativePlayerCaptureError::CapturedColorLength {
            expected,
            actual: rgba.len(),
        });
    }
    let width_usize = usize::try_from(width)
        .map_err(|_| NativePlayerCaptureError::ContentBoundsOverflow { width, height })?;
    let mut min_x = width;
    let mut min_y = height;
    let mut max_x = 0;
    let mut max_y = 0;
    let mut content_pixels = 0_u64;
    for (index, pixel) in rgba.chunks_exact(4).enumerate() {
        if pixel == [0, 0, 0, 255] {
            continue;
        }
        let x = u32::try_from(index % width_usize)
            .map_err(|_| NativePlayerCaptureError::ContentBoundsOverflow { width, height })?;
        let y = u32::try_from(index / width_usize)
            .map_err(|_| NativePlayerCaptureError::ContentBoundsOverflow { width, height })?;
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        content_pixels += 1;
    }
    let content_bbox = if content_pixels == 0 {
        None
    } else {
        Some(NativePlayerCaptureContentBBox {
            x: min_x,
            y: min_y,
            width: max_x
                .checked_sub(min_x)
                .and_then(|extent| extent.checked_add(1))
                .ok_or(NativePlayerCaptureError::ContentBoundsOverflow { width, height })?,
            height: max_y
                .checked_sub(min_y)
                .and_then(|extent| extent.checked_add(1))
                .ok_or(NativePlayerCaptureError::ContentBoundsOverflow { width, height })?,
        })
    };
    Ok(CaptureContentStats {
        content_bbox,
        content_pixels,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_bundle::resource_codec::SourceMapSection;
    use arcweft_bundle::{BundleManifest, BundleRuntimeSummary};
    use arcweft_core::{
        bytecode::BytecodeProgram,
        line_task::LineTaskGroup,
        plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeLineId, RuntimePlan},
    };
    use arcweft_dialogue::{DialogueProfileRevision, InlineFailurePolicy};
    use arcweft_render_text::{
        LineDisplayCatalog, LineDisplaySpec, RichTextDocument, RichTextNode,
    };
    use arcweft_resource_model::registry::ResourceTypeRegistry;
    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
    use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

    fn test_dialogue_revision() -> DialogueProfileRevision {
        let manifest = SourceDocument::try_new(
            SourceDocumentId::try_new("player-native-dev-capture-test").expect("document ID"),
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
            ViewProgramId::try_new("view_program.player-native-dev-capture-test")
                .expect("View program ID"),
            AcceptedViewProgramRevision::try_from_bytes([0x5a; 32]).expect("View program revision"),
            ResourceTypeRegistry::empty().digest(),
        )
    }
    use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

    #[test]
    fn content_stats_are_derived_from_shared_capture_pixels() {
        let rgba = [
            0, 0, 0, 255, 1, 2, 3, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 4, 5, 6, 255,
        ];

        assert_eq!(
            capture_content_stats(&rgba, 3, 2).expect("valid RGBA length"),
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
            capture_content_stats(&[0, 0, 0, 255], 1, 1).expect("valid RGBA length"),
            CaptureContentStats {
                content_bbox: None,
                content_pixels: 0,
            }
        );
    }

    #[test]
    fn capture_preparation_uses_the_normal_prepared_text_batch() {
        let bundle = dialogue_bundle();
        let images = BundleImageCatalog::from_bundle(&bundle).expect("image catalog builds");
        let mut session =
            BundleSession::new(&bundle, BundleSessionOptions::default()).expect("session starts");
        let mut planner = PlayerFramePlannerState::new();
        PlayerFontSet::bundled_default()
            .register_with_planner(&mut planner)
            .expect("fonts register");
        let input = InputController::default();
        let step = session.step_with_clock(
            RuntimeClockStep::from_millis(1, CAPTURE_STEP_MILLIS).expect("clock is valid"),
            BundleStepInput::default(),
        );
        let style_environment = session.presentation_environment();
        let candidate = planner
            .prepare_candidate(
                &input,
                PlayerFrameRequest {
                    presentation: &step.presentation,
                    fx_definitions: session.fx_definitions(),
                    images: &images,
                    style_program: session.view_style_program(),
                    style_environment: &style_environment,
                    style_palettes: session.view_style_palettes(),
                    viewport: capture_viewport(NativePlayerCaptureRequest::new(640, 360, 8))
                        .expect("viewport converts"),
                    fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
                    image_time_millis: u64::from(CAPTURE_STEP_MILLIS),
                    visual_time_millis: u64::from(CAPTURE_STEP_MILLIS),
                    dialogue_reveal_complete: true,
                    preferences: RenderPreferences::default(),
                },
            )
            .expect("dialogue frame candidate prepares");

        assert_eq!(candidate.prepared().dialogue_views().len(), 1);
        assert!(!candidate.prepared().text.is_empty());
    }

    #[test]
    fn zero_capture_extent_is_rejected_without_clamping() {
        assert!(matches!(
            capture_viewport(NativePlayerCaptureRequest::new(0, 360, 1)),
            Err(NativePlayerCaptureError::InvalidCaptureExtent {
                width: 0,
                height: 360,
            })
        ));
    }

    fn dialogue_bundle() -> ArcweftBundle {
        let line = RuntimeLineId::from_runtime_line_value("line.capture")
            .expect("runtime line id is valid");
        let plan = RuntimePlan::new(
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
        .expect("runtime plan is valid")
        .with_entries(vec![arcweft_core::plan::RuntimeEntrySpec {
            id: arcweft_core::plan::EntryRuntimeId::from_source_entity_body("entry.main")
                .expect("test entry ID is valid"),
            kind: arcweft_core::plan::RuntimeEntryKind::Cli,
            binding: arcweft_core::entry::EntryBindingIdentity::from_bytes([1; 32]),
            target: arcweft_core::plan::RuntimeEntryTarget::Flow(
                FlowRuntimeId::from_runtime_target_value("flow.main").expect("flow id is valid"),
            ),
            roles: arcweft_core::entry::RuntimeEntryRoles::None,
        }]);
        let display = LineDisplayCatalog::try_from_lines(
            test_dialogue_revision(),
            vec![LineDisplaySpec {
                line,
                callee: "narrator".to_owned(),
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
                    text: "shared prepared capture".to_owned(),
                }]),
            }],
        )
        .expect("test display catalog is revision-consistent");
        let product_awbc = AwbcLowerer::new(&plan, &display, "capture.arcw")
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
                    bytecode_instructions: 2,
                    line_task_groups: 1,
                    stream_plans: 0,
                    source_plans: 0,
                },
            },
            source_map("capture.arcw", "flow @flow.main main { dialogue }"),
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
}
