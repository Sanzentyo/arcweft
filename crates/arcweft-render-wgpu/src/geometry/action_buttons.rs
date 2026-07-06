use super::control_style::{
    ControlInteractionStyleState, ControlPointerStyleState, PreparedControlBackdrop,
    PreparedControlFilter, PreparedControlPaint, PreparedControlShadow, RenderControlStyle,
    control_font_family, fill_with_opacity, push_control_backdrop_plan, push_control_border,
    push_control_corner_frame, push_control_filter_plan, push_control_focus_ring,
    push_control_shadow_plan, state_from_interaction,
};
use super::{PaintRect, Palette, RenderScene, RenderTextBlock, RenderTextSlant, RenderTextWeight};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::layer::LayerId;
use arcweft_presentation::semantic::{SemanticNode, SemanticRole, SemanticTree};
use arcweft_presentation::text_input::{
    TextByteOffset, TextControlValue, TextInputSessionId, TextRange, TextRevision,
};

/// Player-rendered action button lowered from product UI resources.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderActionButton {
    pub target: InteractionTarget,
    pub label: String,
    pub enabled: bool,
    pub bounds: HitRect,
    pub style: RenderControlStyle,
    pub action: RenderActionButtonAction,
}

/// Action emitted by a player-rendered button.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderActionButtonAction {
    Noop,
    TextInputSubmit {
        input_target: InteractionTarget,
        session: TextInputSessionId,
        value: TextControlValue,
        selection: TextRange<TextByteOffset>,
        revision: TextRevision,
        ime_policy: RenderTextSubmitImePolicy,
    },
    ActionInvoke {
        action: PublicId,
        payload: Option<String>,
    },
}

/// How button activation should handle an active IME composition.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RenderTextSubmitImePolicy {
    #[default]
    Commit,
    Cancel,
    Reject,
}

/// Prepared button hit-test and activation payload.
#[derive(Clone, Debug, PartialEq)]
pub struct PreparedActionButton {
    pub target: InteractionTarget,
    pub label: String,
    pub enabled: bool,
    pub action: RenderActionButtonAction,
}

pub(super) struct ActionButtonBuildOutput<'a> {
    pub(super) semantics: &'a mut SemanticTree,
    pub(super) rectangles: &'a mut Vec<PaintRect>,
    pub(super) text: &'a mut Vec<RenderTextBlock>,
    pub(super) control_backdrops: &'a mut Vec<PreparedControlBackdrop>,
    pub(super) control_shadows: &'a mut Vec<PreparedControlShadow>,
    pub(super) control_filters: &'a mut Vec<PreparedControlFilter>,
}

pub(super) fn action_button_depth_milli(scene: &RenderScene, button: &RenderActionButton) -> i32 {
    let state = visual_state_for_button(scene, button);
    button
        .style
        .visual_for_state(state)
        .depth_milli
        .unwrap_or_default()
}

#[expect(
    clippy::too_many_arguments,
    reason = "The geometry sinks are intentionally explicit at this renderer boundary."
)]
pub(super) fn build_action_button(
    scene: &RenderScene,
    layer: &LayerId,
    button: &RenderActionButton,
    output: ActionButtonBuildOutput<'_>,
    palette: &Palette,
    action_id: &PublicId,
    font_size: f32,
    line_height: f32,
) -> (PreparedActionButton, PreparedControlPaint) {
    let ActionButtonBuildOutput {
        semantics,
        rectangles,
        text,
        control_backdrops,
        control_shadows,
        control_filters,
    } = output;
    let is_focused = scene.interaction.focused.as_ref() == Some(&button.target);
    let is_hovered = scene.interaction.hovered.as_ref() == Some(&button.target);
    let is_pressed = scene.interaction.pressed.as_ref() == Some(&button.target);
    let visual = button
        .style
        .visual_for_state(visual_state_for_button(scene, button));
    let radii = visual.radii();
    let backdrop_start = control_backdrops.len();
    push_control_backdrop_plan(control_backdrops, &button.target, button.bounds, &visual);
    let shadow_start = control_shadows.len();
    push_control_shadow_plan(control_shadows, &button.target, button.bounds, &visual);
    let fallback_fill = action_button_fill(
        button.enabled,
        is_focused || is_hovered,
        is_pressed,
        palette,
    );
    let rectangle_start = rectangles.len();
    rectangles.push(PaintRect::with_radii(
        button.bounds,
        fill_with_opacity(visual.fill.unwrap_or(fallback_fill), visual.opacity),
        radii,
    ));
    push_control_border(rectangles, button.bounds, visual.border, radii);
    push_control_corner_frame(rectangles, button.bounds, visual.corner_frame);
    if is_focused {
        if let Some(ring) = visual.focus_ring {
            push_control_focus_ring(rectangles, button.bounds, ring, radii);
        } else {
            super::push_focus_ring(rectangles, button.bounds, palette.focus_ring);
        }
    }
    let text_start = text.len();
    text.push(RenderTextBlock {
        text: button.label.clone(),
        bounds: HitRect::new(
            button.bounds.x + 18.0,
            button.bounds.y + (button.bounds.height - line_height) * 0.5,
            (button.bounds.width - 36.0).max(1.0),
            line_height,
        ),
        clip_bounds: Some(button.bounds),
        buffer_width: Some((button.bounds.width - 36.0).max(1.0)),
        buffer_height: Some(line_height),
        font_size,
        line_height,
        font_family: control_font_family(&visual),
        weight: RenderTextWeight::Bold,
        slant: RenderTextSlant::Upright,
        rgba: visual.text.unwrap_or(palette.choice_text),
    });
    let filter_start = control_filters.len();
    push_control_filter_plan(control_filters, &button.target, button.bounds, &visual);
    let paint = PreparedControlPaint {
        target: button.target.clone(),
        bounds: button.bounds,
        rectangle_range: rectangle_start..rectangles.len(),
        text_range: text_start..text.len(),
        backdrop_range: backdrop_start..control_backdrops.len(),
        shadow_range: shadow_start..control_shadows.len(),
        filter_range: filter_start..control_filters.len(),
    };
    semantics.push(
        SemanticNode::new(
            layer.clone(),
            button.target.clone(),
            SemanticRole::Button,
            button.bounds,
        )
        .with_label(button.label.clone())
        .with_action(button.action.semantic_action_id(action_id).clone())
        .with_enabled(button.enabled),
    );
    (
        PreparedActionButton {
            target: button.target.clone(),
            label: button.label.clone(),
            enabled: button.enabled,
            action: button.action.clone(),
        },
        paint,
    )
}

fn action_button_fill(enabled: bool, active: bool, pressed: bool, palette: &Palette) -> [f32; 4] {
    if !enabled {
        palette.choice_idle.map(|channel| channel * 0.72)
    } else if pressed {
        palette.choice_pressed
    } else if active {
        palette.choice_active
    } else {
        palette.choice_idle
    }
}

fn visual_state_for_button(
    scene: &RenderScene,
    button: &RenderActionButton,
) -> super::RenderControlVisualState {
    let is_focused = scene.interaction.focused.as_ref() == Some(&button.target);
    let is_hovered = scene.interaction.hovered.as_ref() == Some(&button.target);
    let is_pressed = scene.interaction.pressed.as_ref() == Some(&button.target);
    state_from_interaction(ControlInteractionStyleState {
        enabled: button.enabled,
        focused: is_focused,
        pointer: ControlPointerStyleState::from_interaction(is_hovered, is_pressed),
    })
}

impl RenderActionButtonAction {
    #[must_use]
    pub const fn input_target(&self) -> Option<&InteractionTarget> {
        match self {
            Self::TextInputSubmit { input_target, .. } => Some(input_target),
            Self::Noop | Self::ActionInvoke { .. } => None,
        }
    }

    #[must_use]
    pub const fn ime_policy(&self) -> Option<RenderTextSubmitImePolicy> {
        match self {
            Self::TextInputSubmit { ime_policy, .. } => Some(*ime_policy),
            Self::Noop | Self::ActionInvoke { .. } => None,
        }
    }

    #[must_use]
    pub const fn semantic_action_id<'a>(&'a self, fallback: &'a PublicId) -> &'a PublicId {
        match self {
            Self::ActionInvoke { action, .. } => action,
            Self::Noop | Self::TextInputSubmit { .. } => fallback,
        }
    }
}
