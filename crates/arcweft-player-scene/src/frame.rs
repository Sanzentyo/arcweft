use crate::action_buttons::{RuntimeActionButtonLowerer, RuntimeActionButtonLoweringError};
use crate::frame::focus_navigation::{render_focus_groups, render_focus_navigation};
use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use thiserror::Error;

mod focus_navigation;

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
    ActionButtonLowering(#[from] RuntimeActionButtonLoweringError),
    #[error(transparent)]
    Images(#[from] BundleImageCatalogError),
    #[error(transparent)]
    TextEditor(#[from] TextEditorError),
    #[error("invalid focus navigation public id `{value}`")]
    InvalidId { value: String },
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
        let action_buttons = RuntimeActionButtonLowerer::lower_buttons(
            &request.presentation.action_buttons,
            &text_inputs,
        )?;
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
            action_buttons,
            focus_groups: render_focus_groups(&request.presentation.focus_groups)?,
            focus_navigation: render_focus_navigation(&request.presentation.focus_navigation)?,
            images: request.images.render_images(
                &request.presentation.images,
                request.image_time_millis,
                request.viewport,
            )?,
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
        if input.apply_pending_text_pointer_selection(&frame)? {
            let scene = Self::render_scene(input, request)?;
            let frame = SharedFramePlanner::prepare(&scene)?;
            return Ok(PlayerPreparedFrame { scene, frame });
        }
        Ok(PlayerPreparedFrame { scene, frame })
    }
}
