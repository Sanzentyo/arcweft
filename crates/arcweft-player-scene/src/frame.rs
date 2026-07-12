use crate::action_buttons::{RuntimeActionButtonLowerer, RuntimeActionButtonLoweringError};
use crate::frame::focus_navigation::{render_focus_groups, render_focus_navigation};
use crate::images::{BundleImageCatalog, BundleImageCatalogError};
use crate::input::InputController;
use crate::text_controls::{RuntimeTextControlLowerer, RuntimeTextControlLoweringError};
use arcweft_bundle::fx_definitions::FxDefinitions;
use arcweft_layout::{ContentRect, LayoutError, LayoutSize, ScalePolicy};
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_editor::TextEditorError;
use arcweft_render_wgpu::geometry::{
    FramePlanError, PreparedFrame, RenderChoiceItem, RenderFocusAutoScrollPolicy,
    RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollIndicatorsPolicy,
    RenderScrollOverflow, RenderScrollOverscrollPolicy, RenderScrollRegion, RenderViewport,
    SharedFramePlanContext, SharedFramePlanStats,
};
use arcweft_runtime_driver::display::{BundlePresentationSnapshot, BundleViewportFit};
use num_traits::ToPrimitive;
use thiserror::Error;

mod focus_navigation;
mod surfaces;
mod textboxes;
mod view_text;

/// Player-owned frame inputs shared by native, web, and Agent observation.
#[derive(Clone, Copy, Debug)]
pub struct PlayerFrameRequest<'a> {
    pub presentation: &'a BundlePresentationSnapshot,
    pub fx_definitions: &'a FxDefinitions,
    pub images: &'a BundleImageCatalog,
    pub viewport: RenderViewport,
    pub fit: PlayerFrameFit,
    pub image_time_millis: u64,
    pub visual_time_millis: u64,
    pub dialogue_reveal_complete: bool,
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
/// All interactive hosts should use this path so runtime View controls, semantic
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
    /// Returns standard Rust-backed `TextBox` View bounds in stable target order.
    #[must_use]
    pub fn standard_textbox_bounds(viewport: RenderViewport, count: usize) -> Vec<HitRect> {
        textboxes::standard_textbox_bounds(viewport, count)
    }

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
            content_avoidance_regions: textboxes::standard_textbox_bounds(
                request.viewport,
                request
                    .presentation
                    .textboxes
                    .iter()
                    .filter(|textbox| textbox.active_entry().is_some())
                    .count(),
            ),
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
                .map(|region| {
                    render_scroll_region(
                        input,
                        region,
                        request.visual_time_millis,
                        request.preferences.reduce_motion,
                    )
                })
                .collect(),
        })
    }

    pub fn prepare(
        input: &mut InputController,
        request: PlayerFrameRequest<'_>,
    ) -> Result<PlayerPreparedFrame, PlayerFrameError> {
        let mut planner = PlayerFramePlannerState::new();
        for bytes in crate::fonts::DEFAULT_PLAYER_FONT_RESOURCE_BYTES {
            planner.register_font_bytes(bytes.to_vec())?;
        }
        planner.prepare(input, request)
    }
}

fn render_scroll_region(
    input: &mut InputController,
    region: &arcweft_bundle::resource_codec::ViewRuntimeScrollRegion,
    visual_time_millis: u64,
    reduce_motion: bool,
) -> RenderScrollRegion {
    let mut render_region = RenderScrollRegion {
        id: region.public_id.clone(),
        bounds: HitRect::new(
            milli_i32_to_f32(region.bounds.x_milli),
            milli_i32_to_f32(region.bounds.y_milli),
            milli_u32_to_f32(region.bounds.width_milli),
            milli_u32_to_f32(region.bounds.height_milli),
        ),
        content_width: milli_u32_to_f32(region.content_width_milli),
        content_height: milli_u32_to_f32(region.content_height_milli),
        offset_x: 0.0,
        offset_y: 0.0,
        overscroll_x: 0.0,
        overscroll_y: 0.0,
        axis: render_scroll_axis(region.axis),
        overflow: render_scroll_overflow(region.overflow),
        indicators: render_scroll_indicators_policy(region.indicators),
        overscroll: render_scroll_overscroll_policy(region.overscroll),
        auto_scroll_focus: render_focus_auto_scroll_policy(region.auto_scroll_focus),
        indicator_activity_millis: None,
    };
    input.resolve_scroll_region(&mut render_region, visual_time_millis, reduce_motion);
    render_region
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

const fn render_scroll_indicators_policy(
    policy: arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy,
) -> RenderScrollIndicatorsPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Auto => {
            RenderScrollIndicatorsPolicy::Auto
        }
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Visible => {
            RenderScrollIndicatorsPolicy::Visible
        }
        arcweft_bundle::resource_codec::ViewScrollIndicatorsPolicy::Hidden => {
            RenderScrollIndicatorsPolicy::Hidden
        }
    }
}

const fn render_scroll_overscroll_policy(
    policy: arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy,
) -> RenderScrollOverscrollPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Clamp => {
            RenderScrollOverscrollPolicy::Clamp
        }
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Contain => {
            RenderScrollOverscrollPolicy::Contain
        }
        arcweft_bundle::resource_codec::ViewScrollOverscrollPolicy::Elastic => {
            RenderScrollOverscrollPolicy::Elastic
        }
    }
}

const fn render_focus_auto_scroll_policy(
    policy: arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy,
) -> RenderFocusAutoScrollPolicy {
    match policy {
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Nearest => {
            RenderFocusAutoScrollPolicy::Nearest
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Start => {
            RenderFocusAutoScrollPolicy::Start
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::End => {
            RenderFocusAutoScrollPolicy::End
        }
        arcweft_bundle::resource_codec::ViewFocusAutoScrollPolicy::Disabled => {
            RenderFocusAutoScrollPolicy::Disabled
        }
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or(0.0) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}

fn scroll_adjusted_bounds(
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
        bounds.x - region.visual_offset_x(),
        bounds.y - region.visual_offset_y(),
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
        let mut frame = self.prepare_mapped_frame(
            &scene,
            design_request.presentation,
            input,
            request,
            content_rect,
        )?;
        if input.ensure_choice_focus(&frame) {
            scene = PlayerFramePlanner::render_scene(input, design_request)?;
            frame = self.prepare_mapped_frame(
                &scene,
                design_request.presentation,
                input,
                request,
                content_rect,
            )?;
        }
        if input.apply_pending_text_pointer_selection(&frame)? {
            let scene = PlayerFramePlanner::render_scene(input, design_request)?;
            let frame = self.prepare_mapped_frame(
                &scene,
                design_request.presentation,
                input,
                request,
                content_rect,
            )?;
            return Ok(PlayerPreparedFrame { scene, frame });
        }
        Ok(PlayerPreparedFrame { scene, frame })
    }

    fn prepare_mapped_frame(
        &mut self,
        scene: &RenderScene,
        presentation: &BundlePresentationSnapshot,
        input: &InputController,
        request: PlayerFrameRequest<'_>,
        content_rect: Option<ContentRect>,
    ) -> Result<PreparedFrame, PlayerFrameError> {
        let mut frame = match content_rect {
            Some(content_rect) => {
                self.shared
                    .prepare_mapped(scene, request.viewport, content_rect)?
            }
            None => self.shared.prepare(scene)?,
        };
        let prepared_view_text = view_text::prepare_runtime_view_text(
            &mut self.shared,
            &mut frame,
            input,
            scene,
            &presentation.view,
            content_rect,
        )?;
        surfaces::push_runtime_view_scene(
            &mut frame,
            scene,
            &presentation.surfaces,
            &presentation.view,
            &prepared_view_text,
            content_rect,
        );
        let textbox_request = textboxes::TextBoxViewFrameRequest::new(
            scene,
            presentation,
            request.fx_definitions,
            request.visual_time_millis,
            request.dialogue_reveal_complete,
            content_rect,
        );
        textboxes::push_textbox_views(&mut self.shared, &mut frame, &textbox_request)?;
        Ok(frame)
    }
}

fn dimension_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX)
}
