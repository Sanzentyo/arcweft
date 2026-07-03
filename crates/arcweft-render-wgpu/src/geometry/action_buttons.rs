use super::{
    PaintRect, Palette, RenderFontFamily, RenderScene, RenderTextBlock, RenderTextSlant,
    RenderTextWeight,
};
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
    pub action: RenderActionButtonAction,
}

/// Action emitted by a player-rendered button.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RenderActionButtonAction {
    TextInputSubmit {
        input_target: InteractionTarget,
        session: TextInputSessionId,
        value: TextControlValue,
        selection: TextRange<TextByteOffset>,
        revision: TextRevision,
        ime_policy: RenderTextSubmitImePolicy,
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

pub(super) fn build_action_buttons(
    scene: &RenderScene,
    layer: &LayerId,
    semantics: &mut SemanticTree,
    rectangles: &mut Vec<PaintRect>,
    text: &mut Vec<RenderTextBlock>,
    palette: &Palette,
    action_id: &PublicId,
) -> Vec<PreparedActionButton> {
    let scale = f32::from(scene.preferences.text_scale_milli) / 1_000.0;
    let font_size = 20.0 * scale;
    let line_height = 28.0 * scale;
    scene
        .action_buttons
        .iter()
        .map(|button| {
            let is_focused = scene.interaction.focused.as_ref() == Some(&button.target);
            let is_hovered = scene.interaction.hovered.as_ref() == Some(&button.target);
            let is_pressed = scene.interaction.pressed.as_ref() == Some(&button.target);
            let state = ActionButtonVisualState::from_interaction(
                button.enabled,
                is_focused || is_hovered,
                is_pressed,
            );
            rectangles.push(PaintRect {
                bounds: button.bounds,
                rgba: action_button_fill(state, palette),
            });
            if is_focused {
                super::push_focus_ring(rectangles, button.bounds, palette.focus_ring);
            }
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
                font_family: RenderFontFamily::SansSerif,
                weight: RenderTextWeight::Bold,
                slant: RenderTextSlant::Upright,
                rgba: palette.choice_text,
            });
            semantics.push(
                SemanticNode::new(
                    layer.clone(),
                    button.target.clone(),
                    SemanticRole::Button,
                    button.bounds,
                )
                .with_label(button.label.clone())
                .with_action(action_id.clone())
                .with_enabled(button.enabled),
            );
            PreparedActionButton {
                target: button.target.clone(),
                label: button.label.clone(),
                enabled: button.enabled,
                action: button.action.clone(),
            }
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionButtonVisualState {
    Disabled,
    Pressed,
    Active,
    Idle,
}

impl ActionButtonVisualState {
    const fn from_interaction(enabled: bool, active: bool, pressed: bool) -> Self {
        if !enabled {
            Self::Disabled
        } else if pressed {
            Self::Pressed
        } else if active {
            Self::Active
        } else {
            Self::Idle
        }
    }
}

fn action_button_fill(state: ActionButtonVisualState, palette: &Palette) -> [f32; 4] {
    match state {
        ActionButtonVisualState::Disabled => palette.choice_idle.map(|channel| channel * 0.72),
        ActionButtonVisualState::Pressed => palette.choice_pressed,
        ActionButtonVisualState::Active => palette.choice_active,
        ActionButtonVisualState::Idle => palette.choice_idle,
    }
}

impl RenderActionButtonAction {
    #[must_use]
    pub const fn input_target(&self) -> &InteractionTarget {
        match self {
            Self::TextInputSubmit { input_target, .. } => input_target,
        }
    }

    #[must_use]
    pub const fn ime_policy(&self) -> RenderTextSubmitImePolicy {
        match self {
            Self::TextInputSubmit { ime_policy, .. } => *ime_policy,
        }
    }
}
