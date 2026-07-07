use crate::action_buttons::{RuntimeActionButtonLowerer, RuntimeActionButtonLoweringError};
use crate::control_style::lower_control_style;
use crate::frame::focus_navigation::{render_focus_groups, render_focus_navigation};
use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_layout::{ContentRect, LayoutError, LayoutSize, ScalePolicy};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderControlVisualState, RenderDialogue,
    RenderFontFamily, RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollOverflow,
    RenderScrollRegion, RenderTextBlock, RenderTextSlant, RenderTextWeight, RenderViewport,
    SharedFramePlanContext, SharedFramePlanStats,
};
use arcweft_runtime_driver::display::{BundlePresentationSnapshot, BundleViewportFit};
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

/// Stateful player frame planner for long-lived native/web player windows.
///
/// The stateless `PlayerFramePlanner` facade remains for tests and one-shot
/// observation. Hosts that register project-owned font bytes should keep this
/// state and register the same bytes here and in the renderer.
#[derive(Debug, Default)]
pub struct PlayerFramePlannerState {
    shared: SharedFramePlanContext,
}

impl PlayerFrameFit {
    pub const fn raw() -> Self {
        Self {
            design_width: 0,
            design_height: 0,
            scale_policy: ScalePolicy::Raw,
        }
    }

    pub const fn design_1280x720(scale_policy: ScalePolicy) -> Self {
        Self::design(1280, 720, scale_policy)
    }

    pub const fn design(design_width: u32, design_height: u32, scale_policy: ScalePolicy) -> Self {
        Self {
            design_width: if design_width == 0 { 1 } else { design_width },
            design_height: if design_height == 0 { 1 } else { design_height },
            scale_policy,
        }
    }

    #[must_use]
    pub fn with_presentation_override(self, presentation: &BundlePresentationSnapshot) -> Self {
        presentation.viewport_fit.map_or(self, Self::from)
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

impl From<BundleViewportFit> for PlayerFrameFit {
    fn from(value: BundleViewportFit) -> Self {
        if value.scale_policy == ScalePolicy::Raw {
            Self::raw()
        } else {
            Self::design(value.design_width, value.design_height, value.scale_policy)
        }
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
            scroll_regions: request
                .presentation
                .scroll_regions
                .iter()
                .map(|region| render_scroll_region(input, region))
                .collect(),
        })
    }

    pub fn prepare(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrame, PlayerFrameError> {
        PlayerFramePlannerState::new().prepare(input, request)
    }
}

fn render_scroll_region(
    input: &InputController,
    region: &arcweft_bundle::resource_codec::ViewRuntimeScrollRegion,
) -> RenderScrollRegion {
    RenderScrollRegion {
        id: region.public_id.clone(),
        bounds: HitRect::new(
            milli_i32_to_f32(region.bounds.x_milli),
            milli_i32_to_f32(region.bounds.y_milli),
            milli_u32_to_f32(region.bounds.width_milli),
            milli_u32_to_f32(region.bounds.height_milli),
        ),
        content_width: milli_u32_to_f32(region.content_width_milli),
        content_height: milli_u32_to_f32(region.content_height_milli),
        offset_x: input.scroll_offset_x(&region.public_id),
        offset_y: input.scroll_offset_y(&region.public_id),
        axis: render_scroll_axis(region.axis),
        overflow: render_scroll_overflow(region.overflow),
    }
}

const fn render_scroll_axis(
    axis: arcweft_bundle::resource_codec::ViewScrollAxis,
) -> RenderScrollAxis {
    match axis {
        arcweft_bundle::resource_codec::ViewScrollAxis::Vertical => RenderScrollAxis::Vertical,
        arcweft_bundle::resource_codec::ViewScrollAxis::Horizontal => RenderScrollAxis::Horizontal,
    }
}

fn render_scroll_overflow(
    overflow: arcweft_bundle::resource_codec::ViewScrollOverflowPolicy,
) -> RenderScrollOverflow {
    match overflow {
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Auto => {
            RenderScrollOverflow::Auto
        }
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Scroll => {
            RenderScrollOverflow::Scroll
        }
        arcweft_bundle::resource_codec::ViewScrollOverflowPolicy::Hidden => {
            RenderScrollOverflow::Hidden
        }
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or(0.0) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}

fn render_text_blocks(
    scene: &RenderScene,
    blocks: &[arcweft_bundle::resource_codec::ViewRuntimeTextBlock],
) -> Vec<RenderTextBlock> {
    let text_scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    blocks
        .iter()
        .filter_map(|block| render_text_block(scene, block, text_scale))
        .collect()
}

fn render_text_block(
    scene: &RenderScene,
    block: &arcweft_bundle::resource_codec::ViewRuntimeTextBlock,
    text_scale: f32,
) -> Option<RenderTextBlock> {
    let bounds = HitRect::new(
        milli_i32_to_f32(block.bounds.x_milli),
        milli_i32_to_f32(block.bounds.y_milli),
        milli_u32_to_f32(block.bounds.width_milli),
        milli_u32_to_f32(block.bounds.height_milli),
    );
    let (bounds, clip_bounds) =
        scroll_adjusted_text_bounds(scene, block.containing_scroll_region.as_deref(), bounds)?;
    let visual =
        lower_control_style(&block.style).visual_for_state(RenderControlVisualState::Normal);
    let font_size = visual.font_size_px.unwrap_or(20.0) * text_scale;
    let line_height = visual
        .line_height_px
        .map_or(font_size * 1.2, |line_height| line_height * text_scale);
    Some(RenderTextBlock {
        text: block.text.clone(),
        bounds,
        clip_bounds,
        buffer_width: Some(bounds.width),
        buffer_height: Some(bounds.height),
        font_size,
        line_height,
        font_family: visual
            .font_family
            .map_or(RenderFontFamily::SansSerif, RenderFontFamily::Named),
        weight: render_text_weight(visual.font_weight),
        slant: RenderTextSlant::Upright,
        rgba: visual.text.unwrap_or([245, 245, 240, 255]),
    })
}

fn render_text_weight(weight: Option<u16>) -> RenderTextWeight {
    if weight.unwrap_or(400) >= 600 {
        RenderTextWeight::Bold
    } else {
        RenderTextWeight::Regular
    }
}

fn scroll_adjusted_text_bounds(
    scene: &RenderScene,
    containing_scroll_region: Option<&str>,
    bounds: HitRect,
) -> Option<(HitRect, Option<HitRect>)> {
    let Some(scroll_region) = containing_scroll_region else {
        return Some((bounds, None));
    };
    let region = scene
        .scroll_regions
        .iter()
        .find(|region| region.id == scroll_region)?;
    let shifted = HitRect::new(
        bounds.x - region.clamped_offset_x(region.offset_x),
        bounds.y - region.clamped_offset_y(region.offset_y),
        bounds.width,
        bounds.height,
    );
    hit_rects_intersect(shifted, region.bounds).then_some((shifted, Some(region.bounds)))
}

fn hit_rects_intersect(left: HitRect, right: HitRect) -> bool {
    let left_max_x = left.x + left.width;
    let left_max_y = left.y + left.height;
    let right_max_x = right.x + right.width;
    let right_max_y = right.y + right.height;
    left.x < right_max_x && left_max_x > right.x && left.y < right_max_y && left_max_y > right.y
}

impl PlayerFramePlannerState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_font_bytes(&mut self, bytes: Vec<u8>) -> Result<(), PlayerFrameError> {
        self.shared.register_font_bytes(bytes)?;
        Ok(())
    }

    #[must_use]
    pub fn stats(&self) -> SharedFramePlanStats {
        self.shared.stats()
    }

    pub fn prepare(
        &mut self,
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrame, PlayerFrameError> {
        let fit = request.fit.with_presentation_override(request.presentation);
        let design_request = PlayerFrameRequest {
            viewport: fit.planning_viewport(request.viewport),
            fit,
            ..request
        };
        let content_rect = fit.content_rect(request.viewport)?;
        let mut scene = PlayerFramePlanner::render_scene(input, design_request)?;
        let mut frame = map_prepared_frame(
            self.prepare_frame_with_runtime_text(&scene, design_request.presentation)?,
            request,
            content_rect,
        );
        if input.ensure_choice_focus(&frame) {
            scene = PlayerFramePlanner::render_scene(input, design_request)?;
            frame = map_prepared_frame(
                self.prepare_frame_with_runtime_text(&scene, design_request.presentation)?,
                request,
                content_rect,
            );
        }
        if input.apply_pending_text_pointer_selection(&frame)? {
            let scene = PlayerFramePlanner::render_scene(input, design_request)?;
            let frame = map_prepared_frame(
                self.prepare_frame_with_runtime_text(&scene, design_request.presentation)?,
                request,
                content_rect,
            );
            return Ok(PlayerPreparedFrame { scene, frame });
        }
        Ok(PlayerPreparedFrame { scene, frame })
    }

    fn prepare_frame_with_runtime_text(
        &mut self,
        scene: &RenderScene,
        presentation: &BundlePresentationSnapshot,
    ) -> Result<PreparedFrame, PlayerFrameError> {
        let mut frame = self.shared.prepare(scene)?;
        frame
            .text
            .extend(render_text_blocks(scene, &presentation.text_blocks));
        Ok(frame)
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
