use crate::control_style::lower_control_style;
use arcweft_bundle::resource_codec::ui::{
    UiRuntimeActionButton, UiRuntimeActionButtonAction, UiRuntimeButtonBounds,
    UiTextSubmitImePolicy,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{TextControlValue, TextInputPrivacy, TextRevision};
use arcweft_render_wgpu::geometry::{
    RenderActionButton, RenderActionButtonAction, RenderTextInputControl, RenderTextSubmitImePolicy,
};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeActionButtonLowerer;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeActionButtonLoweringError {
    #[error("invalid runtime action-button target `{target}`")]
    InvalidTarget { target: String },
    #[error("action button `{button}` references missing text-control target `{target}`")]
    MissingTextControlTarget { button: String, target: String },
}

impl RuntimeActionButtonLowerer {
    pub fn lower_buttons(
        buttons: &[UiRuntimeActionButton],
        text_inputs: &[RenderTextInputControl],
    ) -> Result<Vec<RenderActionButton>, RuntimeActionButtonLoweringError> {
        buttons
            .iter()
            .map(|button| Self::lower_button(button, text_inputs))
            .collect()
    }

    fn lower_button(
        button: &UiRuntimeActionButton,
        text_inputs: &[RenderTextInputControl],
    ) -> Result<RenderActionButton, RuntimeActionButtonLoweringError> {
        Ok(RenderActionButton {
            target: lower_target(&button.target)?,
            label: button.label.clone(),
            enabled: button.enabled,
            bounds: lower_bounds(button.bounds),
            style: lower_control_style(&button.style),
            action: lower_action(button, text_inputs)?,
        })
    }
}

fn lower_action(
    button: &UiRuntimeActionButton,
    text_inputs: &[RenderTextInputControl],
) -> Result<RenderActionButtonAction, RuntimeActionButtonLoweringError> {
    match &button.action {
        UiRuntimeActionButtonAction::TextInputSubmit {
            input_target,
            ime_policy,
        } => {
            let input = text_inputs
                .iter()
                .find(|input| input.target.id().as_str() == input_target)
                .ok_or_else(
                    || RuntimeActionButtonLoweringError::MissingTextControlTarget {
                        button: button.public_id.clone(),
                        target: input_target.clone(),
                    },
                )?;
            let privacy = if input.options.is_secure() {
                TextInputPrivacy::Sensitive
            } else {
                TextInputPrivacy::Plain
            };
            Ok(RenderActionButtonAction::TextInputSubmit {
                input_target: input.target.clone(),
                session: input.session,
                value: TextControlValue::new(input.value.clone(), privacy),
                selection: input.selection,
                revision: TextRevision::default(),
                ime_policy: lower_ime_policy(*ime_policy),
            })
        }
    }
}

fn lower_target(target: &str) -> Result<InteractionTarget, RuntimeActionButtonLoweringError> {
    PublicId::try_new(target)
        .map(InteractionTarget::new)
        .map_err(|_| RuntimeActionButtonLoweringError::InvalidTarget {
            target: target.to_owned(),
        })
}

fn lower_bounds(bounds: UiRuntimeButtonBounds) -> HitRect {
    HitRect::new(
        milli_i32_to_f32(bounds.x_milli),
        milli_i32_to_f32(bounds.y_milli),
        milli_u32_to_f32(bounds.width_milli),
        milli_u32_to_f32(bounds.height_milli),
    )
}

fn lower_ime_policy(policy: UiTextSubmitImePolicy) -> RenderTextSubmitImePolicy {
    match policy {
        UiTextSubmitImePolicy::Commit => RenderTextSubmitImePolicy::Commit,
        UiTextSubmitImePolicy::Cancel => RenderTextSubmitImePolicy::Cancel,
        UiTextSubmitImePolicy::Reject => RenderTextSubmitImePolicy::Reject,
    }
}

fn milli_i32_to_f32(value: i32) -> f32 {
    value.to_f32().unwrap_or_else(|| {
        if value.is_negative() {
            f32::MIN
        } else {
            f32::MAX
        }
    }) / 1_000.0
}

fn milli_u32_to_f32(value: u32) -> f32 {
    value.to_f32().unwrap_or(f32::MAX) / 1_000.0
}
