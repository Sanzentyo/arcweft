use crate::action_buttons::{RuntimeActionButtonLowerer, RuntimeActionButtonLoweringError};
use crate::frame::focus_navigation::{render_focus_groups, render_focus_navigation};
use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_layout::{ContentRect, LayoutError, LayoutSize, ScalePolicy};
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderDialogue, RenderPreferences,
    RenderScene, RenderViewport, SharedFramePlanner,
};
use arcweft_runtime_driver::display::BundlePresentationSnapshot;
use num_traits::ToPrimitive;
use thiserror::Error;

mod focus_navigation;

/// Player-owned frame inputs shared by native, web, and Agent observation.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFrameRequest<'a> {
    pub presentation: &'a BundlePresentationSnapshot,
    pub images: &'a BundleImageCatalog,
    pub viewport: RenderViewport,
    pub fit: PlayerFrameFit,
    pub image_time_millis: u64,
    pub visual_time_millis: u64,
    pub preferences: RenderPreferences,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlayerFrameFit {
    pub design_width: u32,
    pub design_height: u32,
    pub scale_policy: ScalePolicy,
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
    #[error(transparent)]
    Layout(#[from] LayoutError),
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

impl PlayerFrameFit {
    pub const fn raw() -> Self {
        Self {
            design_width: 0,
            design_height: 0,
            scale_policy: ScalePolicy::Raw,
        }
    }

    pub const fn design_1280x720(scale_policy: ScalePolicy) -> Self {
        Self {
            design_width: 1280,
            design_height: 720,
            scale_policy,
        }
    }

    fn planning_viewport(self, output: RenderViewport) -> RenderViewport {
        if self.scale_policy == ScalePolicy::Raw {
            return output;
        }
        RenderViewport {
            logical_width: dimension_to_f32(self.design_width),
            logical_height: dimension_to_f32(self.design_height),
            physical_width: output.physical_width,
            physical_height: output.physical_height,
            scale_factor: output.scale_factor,
        }
    }

    fn content_rect(self, output: RenderViewport) -> Result<Option<ContentRect>, LayoutError> {
        if self.scale_policy == ScalePolicy::Raw {
            return Ok(None);
        }
        ContentRect::calculate(
            LayoutSize::new(
                dimension_to_f32(self.design_width),
                dimension_to_f32(self.design_height),
            ),
            LayoutSize::new(output.logical_width, output.logical_height),
            self.scale_policy,
        )
        .map(Some)
    }
}

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
        let design_request = PlayerFrameRequest {
            viewport: request.fit.planning_viewport(request.viewport),
            ..request
        };
        let content_rect = request.fit.content_rect(request.viewport)?;
        let scene = Self::render_scene(input, design_request)?;
        let frame = map_prepared_frame(SharedFramePlanner::prepare(&scene)?, request, content_rect);
        input.ensure_choice_focus(&frame);
        let scene = Self::render_scene(input, design_request)?;
        let frame = map_prepared_frame(SharedFramePlanner::prepare(&scene)?, request, content_rect);
        if input.apply_pending_text_pointer_selection(&frame)? {
            let scene = Self::render_scene(input, design_request)?;
            let frame =
                map_prepared_frame(SharedFramePlanner::prepare(&scene)?, request, content_rect);
            return Ok(PlayerPreparedFrame { scene, frame });
        }
        Ok(PlayerPreparedFrame { scene, frame })
    }
}

fn map_prepared_frame(
    frame: PreparedFrame,
    request: PlayerFrameRequest<'_>,
    content_rect: Option<ContentRect>,
) -> PreparedFrame {
    match content_rect {
        Some(content_rect) => frame.mapped_to_viewport(request.viewport, content_rect),
        None => frame,
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}
