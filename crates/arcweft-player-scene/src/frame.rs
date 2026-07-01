use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use thiserror::Error;

/// Player-owned frame inputs shared by native, web, and Agent observation.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFrameRequest<'a> {
    pub presentation: &'a BundlePresentationSnapshot,
    pub images: &'a BundleImageCatalog,
    pub viewport: RenderViewport,
    pub image_time_millis: u64,
    pub visual_time_millis: u64,
    pub preferences: RenderPreferences,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PlayerPreparedFrame {
    pub scene: RenderScene,
    pub frame: PreparedFrame,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum PlayerFrameError {
    #[error(transparent)]
    TextControlLowering(#[from] RuntimeTextControlLoweringError),
    #[error(transparent)]
    Images(#[from] BundleImageCatalogError),
    #[error(transparent)]
    FramePlan(#[from] FramePlanError),
}

/// Shared player frame construction.
///
/// All interactive hosts should use this path so runtime UI controls, semantic
/// focus, and render geometry cannot drift between native, web, and Agent
/// observation.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PlayerFramePlanner;

impl PlayerFramePlanner {
    pub fn render_scene(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<RenderScene, PlayerFrameError> {
        let text_inputs =
            RuntimeTextControlLowerer::lower_for_frame(input, &request.presentation.text_inputs)?;
        Ok(RenderScene {
            dialogue: request
                .presentation
                .dialogue
                .as_ref()
                .map(RenderDialogue::from_display_frame),
            choices: request
                .presentation
                .choices
                .iter()
                .map(|choice| RenderChoiceItem {
                    id: choice.id.clone(),
                    label: choice.label.clone(),
                })
                .collect(),
            text_inputs,
            images: request
                .images
                .render_images(&request.presentation.images, request.image_time_millis)?,
            viewport: request.viewport,
            visual_time_millis: request.visual_time_millis,
            preferences: request.preferences,
            interaction: input.visual_state(),
            choice_scroll: input.choice_scroll(),
        })
    }

    pub fn prepare(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrame, PlayerFrameError> {
        let scene = Self::render_scene(input, request)?;
        let frame = SharedFramePlanner::prepare(&scene)?;
        input.ensure_choice_focus(&frame);
        let scene = Self::render_scene(input, request)?;
        let frame = SharedFramePlanner::prepare(&scene)?;
        Ok(PlayerPreparedFrame { scene, frame })
    }
}
