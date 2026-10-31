use super::{AwbcProductStepExecutor, ProductStepError};
use crate::awbc::schema::AwbcTrapCode;
use crate::awbc::vm::VmObservation;
use crate::effect::LineEffectRequest;
use crate::plan::FlowEvent;
use crate::step::RuntimeStepOutput;
use crate::value::RuntimeValue;

enum ProductControlEffect {
    Goto(String),
    Return(String),
    Failed(String),
}

impl AwbcProductStepExecutor {
    /// Admit a terminating control failure while its producing instruction's
    /// original checkpoint still owns the bank, cursor, budget and arguments.
    pub(super) fn preflight_control_traps(
        &self,
        observations: &[VmObservation],
    ) -> Result<(), ProductStepError> {
        let trap = observations.iter().any(|observation| {
            let VmObservation::Effect { effect, args } = observation else {
                return false;
            };
            let super::MappedEffect::Line(effect) = self.program.map_product_effect(*effect, args)
            else {
                return false;
            };
            match control_effect(&effect) {
                Some(ProductControlEffect::Failed(_)) => true,
                Some(ProductControlEffect::Goto(target)) => {
                    self.program.resolve_flow_target_value(&target).is_err()
                }
                Some(ProductControlEffect::Return(_)) | None => false,
            }
        });
        if trap {
            self.program.preflight_fiber_unwinds(
                self.presentation.as_ref(),
                std::iter::once(&self.fiber),
            )?;
        }
        Ok(())
    }

    pub(super) fn apply_control_effects(
        &mut self,
        output: &mut RuntimeStepOutput,
        line_effects_before: usize,
    ) -> bool {
        let Some(effect) = output
            .effects
            .line
            .get(line_effects_before..)
            .into_iter()
            .flatten()
            .find_map(control_effect)
        else {
            return false;
        };
        match effect {
            ProductControlEffect::Return(value) => {
                output.flow_events.push(FlowEvent::Return {
                    value: value.clone(),
                });
                if let Err(error) = self.fiber.mark_returned(Some(RuntimeValue::String(value))) {
                    self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
                }
            }
            ProductControlEffect::Goto(target) => self.goto_effect_target(&target, output),
            ProductControlEffect::Failed(message) => {
                self.fail_with_trap(AwbcTrapCode::ExplicitPanic, message, None, output);
            }
        }
        true
    }

    fn goto_effect_target(&mut self, target: &str, output: &mut RuntimeStepOutput) {
        let (target_id, function) = match self.program.resolve_flow_target_value(target) {
            Ok(target) => target,
            Err(error) => {
                self.fail_with_trap(
                    AwbcTrapCode::MissingDynamicTarget,
                    error.to_string(),
                    None,
                    output,
                );
                return;
            }
        };
        let target_id = target_id.clone();
        output.flow_events.push(FlowEvent::Goto {
            target: target_id.clone(),
        });
        if let Err(error) = self
            .fiber
            .replace_active_function(&self.program, function, &[])
        {
            self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
        }
    }
}

fn control_effect(effect: &LineEffectRequest) -> Option<ProductControlEffect> {
    match effect {
        LineEffectRequest::Goto(target) => Some(ProductControlEffect::Goto(target.clone())),
        LineEffectRequest::Return(value) => Some(ProductControlEffect::Return(value.clone())),
        LineEffectRequest::Panic(message)
        | LineEffectRequest::Fail(message)
        | LineEffectRequest::Bail(message) => Some(ProductControlEffect::Failed(message.clone())),
        _ => None,
    }
}
