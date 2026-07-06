use crate::control_style::lower_control_style;
use arcweft_bundle::resource_codec::ui::{
    ViewActionPayloadResource, ViewRuntimeActionButton, ViewRuntimeActionButtonAction,
    ViewRuntimeButtonBounds,
};
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_render_wgpu::geometry::{
    RenderActionButton, RenderActionButtonAction, RenderTextInputControl,
};
use num_traits::ToPrimitive;
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RuntimeActionButtonLowerer;

#[derive(Clone, Debug, Error, PartialEq)]
pub enum RuntimeActionButtonLoweringError {
    #[error("invalid runtime action-button target `{target}`")]
    InvalidTarget { target: String },
    #[error("action button `{button}` references invalid action `{action}`")]
    InvalidAction { button: String, action: String },
    #[error("action button `{button}` references missing text-control target `{target}`")]
    MissingTextControlTarget { button: String, target: String },
}

impl RuntimeActionButtonLowerer {
    pub fn lower_buttons(
        buttons: &[ViewRuntimeActionButton],
        text_inputs: &[RenderTextInputControl],
    ) -> Result<Vec<RenderActionButton>, RuntimeActionButtonLoweringError> {
        buttons
            .iter()
            .map(|button| Self::lower_button(button, text_inputs))
            .collect()
    }

    fn lower_button(
        button: &ViewRuntimeActionButton,
        text_inputs: &[RenderTextInputControl],
    ) -> Result<RenderActionButton, RuntimeActionButtonLoweringError> {
        Ok(RenderActionButton {
            target: lower_target(&button.target)?,
            label: button.label.clone(),
            enabled: button.enabled,
            containing_scroll_region: button.containing_scroll_region.clone(),
            bounds: lower_bounds(button.bounds),
            viewport_clip: None,
            style: lower_control_style(&button.style),
            action: lower_action(button, text_inputs)?,
        })
    }
}

fn lower_action(
    button: &ViewRuntimeActionButton,
    text_inputs: &[RenderTextInputControl],
) -> Result<RenderActionButtonAction, RuntimeActionButtonLoweringError> {
    match &button.action {
        ViewRuntimeActionButtonAction::Noop => Ok(RenderActionButtonAction::Noop),
        ViewRuntimeActionButtonAction::ActionInvoke { action, payload } => {
            let action = PublicId::try_new(action).map_err(|_| {
                RuntimeActionButtonLoweringError::InvalidAction {
                    button: button.public_id.clone(),
                    action: action.clone(),
                }
            })?;
            Ok(RenderActionButtonAction::ActionInvoke {
                action,
                payload: lower_action_payload(&button.public_id, payload.as_ref(), text_inputs)?,
            })
        }
    }
}

fn lower_action_payload(
    button: &str,
    payload: Option<&ViewActionPayloadResource>,
    text_inputs: &[RenderTextInputControl],
) -> Result<Option<String>, RuntimeActionButtonLoweringError> {
    payload
        .map(|payload| match payload {
            ViewActionPayloadResource::LiteralString { value } => Ok(value.clone()),
            ViewActionPayloadResource::TextControlProjection { input, .. } => text_inputs
                .iter()
                .find(|control| control.target.id().as_str() == input)
                .map(|control| control.value.clone())
                .ok_or_else(
                    || RuntimeActionButtonLoweringError::MissingTextControlTarget {
                        button: button.to_owned(),
                        target: input.clone(),
                    },
                ),
        })
        .transpose()
}

fn lower_target(target: &str) -> Result<InteractionTarget, RuntimeActionButtonLoweringError> {
    PublicId::try_new(target)
        .map(InteractionTarget::new)
        .map_err(|_| RuntimeActionButtonLoweringError::InvalidTarget {
            target: target.to_owned(),
        })
}

fn lower_bounds(bounds: ViewRuntimeButtonBounds) -> HitRect {
    HitRect::new(
        milli_i32_to_f32(bounds.x_milli),
        milli_i32_to_f32(bounds.y_milli),
        milli_u32_to_f32(bounds.width_milli),
        milli_u32_to_f32(bounds.height_milli),
    )
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
