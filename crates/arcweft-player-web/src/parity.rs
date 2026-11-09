pub mod inset_shadow;

use arcweft_core::step::RuntimeStepStopReason;
use arcweft_layout::ScalePolicy;
use arcweft_player_scene::{
    fonts::{PlayerFontRegistrationError, PlayerFontSet},
    frame::{
        PlayerFrameError, PlayerFrameFit, PlayerFramePlannerState, PlayerFrameRequest,
        PlayerFrameTime,
    },
    images::{BundleImageCatalog, BundleImageCatalogError},
    input::{DialogueProgress, InputController, InputPointerModifiers},
};
use arcweft_presentation::input::{KeyPhase, PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{PreparedFrame, RenderPreferences, RenderViewport};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{
    BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput,
};
use std::str::FromStr;
use thiserror::Error;

const PARITY_STEP_MILLIS: u32 = 16;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WebGpuParityFrameOptions {
    pub viewport: RenderViewport,
    pub visual_time_millis: u64,
    pub max_ticks: u64,
    pub interaction: WebGpuParityInteraction,
}

#[derive(Debug, Error)]
pub enum WebGpuParityFrameError {
    #[error(transparent)]
    Session(#[from] BundleSessionError),
    #[error(transparent)]
    Clock(#[from] RuntimeClockError),
    #[error(transparent)]
    Images(#[from] BundleImageCatalogError),
    #[error(transparent)]
    Font(#[from] PlayerFontRegistrationError),
    #[error("capture time {millis} ms must be a positive multiple of 16 within {max_ticks} ticks")]
    CaptureClock { millis: u64, max_ticks: u64 },
    #[error("demo bundle did not reach a dialogue/choice/image frame by capture tick {tick}")]
    FrameNotReady { tick: u64 },
    #[error("frame planning failed: {0}")]
    FramePlan(#[source] Box<PlayerFrameError>),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WebGpuParityInteraction {
    Neutral,
    #[default]
    FocusFirstChoice,
    HoverFirstChoice,
    HoverSecondChoice,
    PressFirstChoice,
}

impl WebGpuParityInteraction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::FocusFirstChoice => "focus-first-choice",
            Self::HoverFirstChoice => "hover-first-choice",
            Self::HoverSecondChoice => "hover-second-choice",
            Self::PressFirstChoice => "press-first-choice",
        }
    }
}

impl FromStr for WebGpuParityInteraction {
    type Err = WebGpuParityCheckpointParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "neutral" => Ok(Self::Neutral),
            "focus-first-choice" => Ok(Self::FocusFirstChoice),
            "hover-first-choice" => Ok(Self::HoverFirstChoice),
            "hover-second-choice" => Ok(Self::HoverSecondChoice),
            "press-first-choice" => Ok(Self::PressFirstChoice),
            unknown => Err(WebGpuParityCheckpointParseError {
                value: unknown.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WebGpuParityCheckpoint {
    Neutral,
    #[default]
    FocusFirstChoice,
    HoverFirstChoice,
    HoverSecondChoice,
    PressFirstChoice,
    CompactFocusFirstChoice,
    HidpiFocusFirstChoice,
}

impl WebGpuParityCheckpoint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::FocusFirstChoice => "focus-first-choice",
            Self::HoverFirstChoice => "hover-first-choice",
            Self::HoverSecondChoice => "hover-second-choice",
            Self::PressFirstChoice => "press-first-choice",
            Self::CompactFocusFirstChoice => "compact-focus-first-choice",
            Self::HidpiFocusFirstChoice => "hidpi-focus-first-choice",
        }
    }

    pub const fn viewport(self) -> RenderViewport {
        match self {
            Self::CompactFocusFirstChoice => RenderViewport {
                logical_width: 960.0,
                logical_height: 540.0,
                physical_width: 960,
                physical_height: 540,
                scale_factor: 1.0,
            },
            Self::HidpiFocusFirstChoice => RenderViewport {
                logical_width: 640.0,
                logical_height: 360.0,
                physical_width: 1280,
                physical_height: 720,
                scale_factor: 2.0,
            },
            Self::Neutral
            | Self::FocusFirstChoice
            | Self::HoverFirstChoice
            | Self::HoverSecondChoice
            | Self::PressFirstChoice => default_parity_viewport(),
        }
    }

    pub const fn interaction(self) -> WebGpuParityInteraction {
        match self {
            Self::Neutral => WebGpuParityInteraction::Neutral,
            Self::FocusFirstChoice | Self::CompactFocusFirstChoice => {
                WebGpuParityInteraction::FocusFirstChoice
            }
            Self::HidpiFocusFirstChoice => WebGpuParityInteraction::FocusFirstChoice,
            Self::HoverFirstChoice => WebGpuParityInteraction::HoverFirstChoice,
            Self::HoverSecondChoice => WebGpuParityInteraction::HoverSecondChoice,
            Self::PressFirstChoice => WebGpuParityInteraction::PressFirstChoice,
        }
    }

    pub const fn options(self, visual_time_millis: u64) -> WebGpuParityFrameOptions {
        WebGpuParityFrameOptions {
            viewport: self.viewport(),
            visual_time_millis,
            max_ticks: 16,
            interaction: self.interaction(),
        }
    }
}

impl FromStr for WebGpuParityCheckpoint {
    type Err = WebGpuParityCheckpointParseError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "neutral" => Ok(Self::Neutral),
            "focus-first-choice" => Ok(Self::FocusFirstChoice),
            "hover-first-choice" => Ok(Self::HoverFirstChoice),
            "hover-second-choice" => Ok(Self::HoverSecondChoice),
            "press-first-choice" => Ok(Self::PressFirstChoice),
            "compact-focus-first-choice" => Ok(Self::CompactFocusFirstChoice),
            "hidpi-focus-first-choice" => Ok(Self::HidpiFocusFirstChoice),
            unknown => Err(WebGpuParityCheckpointParseError {
                value: unknown.to_owned(),
            }),
        }
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
#[error("unknown WebGPU parity checkpoint `{value}`")]
pub struct WebGpuParityCheckpointParseError {
    value: String,
}

impl Default for WebGpuParityFrameOptions {
    fn default() -> Self {
        Self {
            viewport: default_parity_viewport(),
            visual_time_millis: 160,
            max_ticks: 16,
            interaction: WebGpuParityInteraction::default(),
        }
    }
}

/// Prepares a parity frame from the actual product and an explicit shared font set.
pub fn prepare_awfb_parity_frame(
    bundle_bytes: &[u8],
    fonts: &PlayerFontSet,
    options: WebGpuParityFrameOptions,
) -> Result<PreparedFrame, WebGpuParityFrameError> {
    let quantum = u64::from(PARITY_STEP_MILLIS);
    let capture_tick = options.visual_time_millis / quantum;
    if capture_tick == 0
        || capture_tick > options.max_ticks
        || !options.visual_time_millis.is_multiple_of(quantum)
    {
        return Err(WebGpuParityFrameError::CaptureClock {
            millis: options.visual_time_millis,
            max_ticks: options.max_ticks,
        });
    }

    let (mut session, artifact) =
        BundleSession::from_awfb_bytes_with_bundle(bundle_bytes, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(artifact.bundle())?;
    let mut input = InputController::default();
    let mut planner = PlayerFramePlannerState::new();
    fonts.register_with_planner(&mut planner)?;

    for tick in 1..capture_tick {
        let clock = RuntimeClockStep::from_millis(tick, PARITY_STEP_MILLIS)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let frame = prepare_parity_frame(
            &session,
            &images,
            &mut planner,
            &mut input,
            options.viewport,
            tick * quantum,
        )?;
        if step.stop_reason == RuntimeStepStopReason::Blocked
            && step.presentation.choices.is_empty()
            && step.presentation.dialogue.latest_active().is_some()
        {
            match input
                .keyboard(&frame, "Enter", KeyPhase::Down)
                .dialogue_progress
            {
                DialogueProgress::Reveal { target } => {
                    session.queue_dialogue_reveal_completion(target);
                }
                DialogueProgress::Advance { target } => session.queue_dialogue_advance(target),
                DialogueProgress::None => {}
            }
        }
    }
    let clock = RuntimeClockStep::from_millis(capture_tick, PARITY_STEP_MILLIS)?;
    let presentation = session
        .step_with_clock(clock, BundleStepInput::default())
        .presentation;
    if presentation.choices.len() != 2 || presentation.images.len() != 4 {
        return Err(WebGpuParityFrameError::FrameNotReady { tick: capture_tick });
    }

    let frame = prepare_parity_frame(
        &session,
        &images,
        &mut planner,
        &mut input,
        options.viewport,
        options.visual_time_millis,
    )?;
    if options.interaction == WebGpuParityInteraction::Neutral {
        return Ok(frame);
    }
    options.interaction.apply(&mut input, &frame);
    prepare_parity_frame(
        &session,
        &images,
        &mut planner,
        &mut input,
        options.viewport,
        options.visual_time_millis,
    )
}

fn prepare_parity_frame(
    session: &BundleSession,
    images: &BundleImageCatalog,
    planner: &mut PlayerFramePlannerState,
    input: &mut InputController,
    viewport: RenderViewport,
    visual_time_millis: u64,
) -> Result<PreparedFrame, WebGpuParityFrameError> {
    let style_environment = session.presentation_environment();
    let request = PlayerFrameRequest {
        presentation: session.presentation(),
        fx_definitions: session.fx_definitions(),
        images,
        style_program: session.view_style_program(),
        style_environment: &style_environment,
        style_palettes: session.view_style_palettes(),
        viewport,
        fit: PlayerFrameFit::design_1280x720(ScalePolicy::Contain),
        time: PlayerFrameTime::runtime(visual_time_millis),
        preferences: RenderPreferences::default(),
    };
    planner
        .prepare_candidate(input, request)
        .and_then(|candidate| {
            planner
                .publication_guard()
                .publish_with(candidate, input, |_| ())
        })
        .map(|(prepared, ())| prepared.frame)
        .map_err(|error| WebGpuParityFrameError::FramePlan(Box::new(error)))
}

impl WebGpuParityInteraction {
    fn apply(self, input: &mut InputController, prepared: &PreparedFrame) {
        let first = prepared.choices.first().map(|choice| &choice.target);
        let second = prepared.choices.get(1).map(|choice| &choice.target);
        let first_position = first.and_then(|target| choice_center(prepared, target));
        let second_position = second.and_then(|target| choice_center(prepared, target));
        match self {
            Self::Neutral => {}
            Self::FocusFirstChoice => {
                let _ = input.ensure_choice_focus(prepared);
            }
            Self::HoverFirstChoice | Self::HoverSecondChoice => {
                let _ = input.ensure_choice_focus(prepared);
                let position = if self == Self::HoverFirstChoice {
                    first_position
                } else {
                    second_position
                };
                if let Some(position) = position {
                    let _ = input.pointer_move(prepared, PointerId(0), position);
                }
            }
            Self::PressFirstChoice => {
                let _ = input.ensure_choice_focus(prepared);
                if let Some(position) = first_position {
                    let _ = input.pointer_move(prepared, PointerId(0), position);
                    let _ = input.pointer_down(
                        prepared,
                        PointerId(0),
                        position,
                        InputPointerModifiers::NONE,
                    );
                }
            }
        }
    }
}

fn choice_center(
    frame: &PreparedFrame,
    target: &arcweft_presentation::input::InteractionTarget,
) -> Option<ViewportPoint> {
    let bounds = frame.hits.find_target(target)?.bounds();
    Some(ViewportPoint::new(
        bounds.x + bounds.width * 0.5,
        bounds.y + bounds.height * 0.5,
    ))
}

const fn default_parity_viewport() -> RenderViewport {
    RenderViewport {
        logical_width: 1280.0,
        logical_height: 720.0,
        physical_width: 1280,
        physical_height: 720,
        scale_factor: 1.0,
    }
}
