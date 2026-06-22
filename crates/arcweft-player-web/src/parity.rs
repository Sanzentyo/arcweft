use crate::images::{BrowserImageCatalog, BrowserImageCatalogError};
use arcweft_bundle::ArcweftBundle;
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, InteractionVisualState, PreparedFrame, RenderChoiceItem, RenderDialogue,
    RenderPreferences, RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_runtime_driver::clock::{RuntimeClockError, RuntimeClockStep};
use arcweft_runtime_driver::session::{
    BundleSession, BundleSessionError, BundleSessionOptions, BundleStepInput,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WebGpuParityFrameOptions {
    pub viewport: RenderViewport,
    pub visual_time_millis: u64,
    pub max_ticks: u64,
    pub focus_first_choice: bool,
}

#[derive(Debug, Error)]
pub enum WebGpuParityFrameError {
    #[error(transparent)]
    Session(#[from] BundleSessionError),
    #[error(transparent)]
    Clock(#[from] RuntimeClockError),
    #[error(transparent)]
    Images(#[from] BrowserImageCatalogError),
    #[error("demo bundle did not reach a dialogue/choice/image frame within {max_ticks} ticks")]
    FrameNotReady { max_ticks: u64 },
    #[error("frame planning failed: {0}")]
    FramePlan(String),
}

impl Default for WebGpuParityFrameOptions {
    fn default() -> Self {
        Self {
            viewport: RenderViewport {
                logical_width: 1280.0,
                logical_height: 720.0,
                physical_width: 1280,
                physical_height: 720,
                scale_factor: 1.0,
            },
            visual_time_millis: 160,
            max_ticks: 16,
            focus_first_choice: true,
        }
    }
}

pub fn prepare_bundle_parity_frame(
    bundle: &ArcweftBundle,
    options: WebGpuParityFrameOptions,
) -> Result<PreparedFrame, WebGpuParityFrameError> {
    let mut session = BundleSession::new(bundle, BundleSessionOptions::default())?;
    let images = BrowserImageCatalog::from_bundle(bundle)?;
    let mut presentation = None;
    for tick in 1..=options.max_ticks {
        let clock = RuntimeClockStep::from_millis(tick, 16)?;
        let step = session.step_with_clock(clock, BundleStepInput::default());
        let ready = step.presentation.choices.len() == 2 && step.presentation.images.len() == 4;
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
            .map(|dialogue| RenderDialogue {
                speaker: dialogue.callee.clone(),
                text: dialogue.text.clone(),
            }),
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
        preferences: RenderPreferences::default(),
        interaction: InteractionVisualState::default(),
        choice_scroll: ChoiceScroll::default(),
    };
    let prepared = SharedFramePlanner::prepare(&scene)
        .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))?;
    let interaction = if options.focus_first_choice {
        InteractionVisualState {
            focused: prepared.first_choice_target(),
            hovered: None,
            pressed: None,
        }
    } else {
        InteractionVisualState::default()
    };
    SharedFramePlanner::prepare(&RenderScene {
        interaction,
        ..scene
    })
    .map_err(|error| WebGpuParityFrameError::FramePlan(error.to_string()))
}
