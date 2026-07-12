use arcweft_bundle::ArcweftBundle;
use arcweft_player_scene::{
    frame::{PlayerFrameFit, PlayerFramePlanner, PlayerFrameRequest},
    images::{BundleImageCatalog, BundleImageCatalogError},
    input::{InputController, InputPointerModifiers},
};
use arcweft_presentation::input::{PointerId, ViewportPoint};
use arcweft_render_wgpu::geometry::{PreparedFrame, RenderPreferences, RenderViewport};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{
    BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput,
};
use std::str::FromStr;
use thiserror::Error;

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
    #[error("demo bundle did not reach a dialogue/choice/image frame within {max_ticks} ticks")]
    FrameNotReady { max_ticks: u64 },
    #[error("frame planning failed: {0}")]
    FramePlan(String),
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
                logical_width: 320.0,
                logical_height: 180.0,
                physical_width: 640,
                physical_height: 360,
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

pub fn prepare_bundle_parity_frame(
    bundle: &ArcweftBundle,
    options: WebGpuParityFrameOptions,
) -> Result<PreparedFrame, WebGpuParityFrameError> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BundleImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    for tick in 1..=options.max_ticks {
        let clock = RuntimeClockStep::from_millis(tick, 16)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let ready = step.presentation.choices.len() == 2 && step.presentation.images.len() == 4;
        if !ready
            && let Some(target) = step
                .presentation
                .textboxes
                .latest_active()
                .and_then(|(textbox, _)| textbox.advance_target())
        {
            session.queue_dialogue_advance(target);
        }
        presentation = Some(step.presentation);
        if ready {
            break;
        }
    }
    let presentation = presentation.ok_or(WebGpuParityFrameError::FrameNotReady {
        max_ticks: options.max_ticks,
    })?;
    if presentation.choices.len() != 2 || presentation.images.len() != 4 {
        return Err(WebGpuParityFrameError::FrameNotReady {
            max_ticks: options.max_ticks,
        });
    }

    let request = PlayerFrameRequest {
        presentation: &presentation,
        fx_definitions: &bundle.fx_definitions,
        images: &images,
        viewport: options.viewport,
        fit: PlayerFrameFit::raw(),
        image_time_millis: options.visual_time_millis,
        visual_time_millis: options.visual_time_millis,
        dialogue_reveal_complete: false,
        preferences: RenderPreferences::default(),
    };
    let mut input = InputController::default();
    let prepared = PlayerFramePlanner::prepare(&mut input, request)
        .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))?;
    options.interaction.apply(&mut input, &prepared.frame);
    if options.interaction == WebGpuParityInteraction::Neutral {
        return Ok(prepared.frame);
    }
    PlayerFramePlanner::prepare(&mut input, request)
        .map(|prepared| prepared.frame)
        .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))
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
