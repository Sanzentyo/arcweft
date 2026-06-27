use arcweft_bundle::ArcweftBundle;
use arcweft_player_scene::images::{BundleImageCatalog, BundleImageCatalogError};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, RenderChoiceItem, RenderDialogue,
    RenderPreferences, RenderScene, RenderViewport, SharedFramePlanner,
};
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
        if !ready && step.presentation.dialogue.is_some() {
            session.queue_dialogue_advance();
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

    let scene = RenderScene {
        dialogue: presentation
            .dialogue
            .as_ref()
            .map(RenderDialogue::from_display_frame),
        choices: presentation
            .choices
            .iter()
            .map(|choice| RenderChoiceItem {
                id: choice.id.clone(),
                label: choice.label.clone(),
            })
            .collect(),
        images: images.render_images(&presentation.images, options.visual_time_millis)?,
        viewport: options.viewport,
        visual_time_millis: options.visual_time_millis,
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    };
    let prepared = SharedFramePlanner::prepare(&scene)
        .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))?;
    let interaction = options.interaction.visual_state(&prepared);
    SharedFramePlanner::prepare(&RenderScene {
        interaction,
        ..scene
    })
    .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))
}

impl WebGpuParityInteraction {
    fn visual_state(self, prepared: &PreparedFrame) -> InteractionVisualState {
        let first = prepared.first_choice_target();
        let second = prepared.last_choice_target();
        match self {
            Self::Neutral => InteractionVisualState::default(),
            Self::FocusFirstChoice => InteractionVisualState {
                focused: first,
                hovered: None,
                pressed: None,
            },
            Self::HoverFirstChoice => InteractionVisualState {
                focused: first.clone(),
                hovered: first,
                pressed: None,
            },
            Self::HoverSecondChoice => InteractionVisualState {
                focused: first,
                hovered: second,
                pressed: None,
            },
            Self::PressFirstChoice => InteractionVisualState {
                focused: first.clone(),
                hovered: first.clone(),
                pressed: first,
            },
        }
    }
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
