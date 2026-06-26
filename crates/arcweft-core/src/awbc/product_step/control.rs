use super::{AwbcProductStepExecutor, ProductStepError};
use crate::awbc::fiber::FiberTrap;
use crate::awbc::schema::{AwbcFunctionId, AwbcTrapCode};
use crate::effect::LineEffectRequest;
use crate::plan::{FlowEvent, FlowRuntimeId};
use crate::step::RuntimeStepOutput;
use crate::value::RuntimeValue;

enum ProductControlEffect {
    Goto(String),
    Return(String),
    Failed(String),
}

impl AwbcProductStepExecutor {
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
                self.fiber.mark_trapped(FiberTrap {
                    code: AwbcTrapCode::ExplicitPanic,
                    message: Some(message),
                    source_map: None,
                });
            }
        }
        true
    }

    fn goto_effect_target(&mut self, target: &str, output: &mut RuntimeStepOutput) {
        let target_id = FlowRuntimeId(target.to_owned());
        output.flow_events.push(FlowEvent::Goto {
            target: target_id.clone(),
        });
        let Some(function) = self.function_for_public_id(target) else {
            self.fiber.mark_trapped(FiberTrap {
                code: AwbcTrapCode::MissingDynamicTarget,
                message: Some(format!("missing goto target {}", target_id.0)),
                source_map: None,
            });
            return;
        };
        if let Err(error) = self
            .fiber
            .replace_active_function(&self.program, function, &[])
        {
            self.fail_with_error(ProductStepError::Internal(error.to_string()), output);
        }
    }

    fn function_for_public_id(&self, target: &str) -> Option<AwbcFunctionId> {
        self.program
            .functions
            .iter()
            .enumerate()
            .find_map(|(index, function)| {
                function
                    .public_id
                    .and_then(|id| self.program.strings.get(id.index()))
                    .filter(|public_id| public_id.as_str() == target)
                    .and_then(|_| u32::try_from(index).ok())
                    .map(AwbcFunctionId)
            })
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
