//! One retained-value ABI for content values and line-owned effect sites.

#[cfg(test)]
mod tests;

use super::{RuntimeDialogueEffectSiteSeed, RuntimePlanBuildError, RuntimePlanBuilder};
use crate::plan::{
    RuntimeDialogueContentEffectSlot, RuntimeDialogueEffectSite, RuntimeFunctionSiteBodyKind,
    RuntimePlanTypeProjection,
};

impl RuntimePlanBuilder {
    pub(super) fn lower_dialogue_effect_site(
        &self,
        effect: RuntimeDialogueEffectSiteSeed,
        expected: &RuntimeDialogueContentEffectSlot,
    ) -> Result<RuntimeDialogueEffectSite, RuntimePlanBuildError> {
        self.root_body_context()
            .lower_dialogue_effect_site(effect, expected)
    }
}

impl super::lower::RuntimePlanBodyConstruction<'_> {
    pub(super) fn lower_dialogue_effect_site(
        &self,
        effect: RuntimeDialogueEffectSiteSeed,
        expected: &RuntimeDialogueContentEffectSlot,
    ) -> Result<RuntimeDialogueEffectSite, RuntimePlanBuildError> {
        if effect.site != expected.site() {
            return Err(RuntimePlanBuildError::NonCanonicalDialogueEffectSite {
                expected: expected.site(),
                actual: effect.site,
            });
        }
        let state = self.resolve_callable_state_seed(&effect.state)?;
        let definition = self.callable_states.borrow().states[state.index()]
            .clone()
            .ok_or(RuntimePlanBuildError::UnknownCallableState { state })?;
        self.validate_value_type_scope("dialogue effect callback type", definition.function_type)?;
        let invocation = definition.zero_argument_invocation().ok_or(
            RuntimePlanBuildError::InvalidTypeProjection {
                context: "dialogue effect callback state",
                ty: definition.function_type,
            },
        )?;
        if !matches!(
            self.projection(definition.result)?,
            RuntimePlanTypeProjection::Unit
        ) {
            return Err(RuntimePlanBuildError::DialogueEffectResultMismatch { site: effect.site });
        }
        let function = *invocation.function();
        let header = self
            .function_sites
            .get(function.get().get() as usize - 1)
            .ok_or(RuntimePlanBuildError::ForeignFunctionSiteSeed)?;
        if header.body_kind != RuntimeFunctionSiteBodyKind::Executable {
            return Err(RuntimePlanBuildError::FunctionSiteBodyKindMismatch {
                site: function,
                expected: RuntimeFunctionSiteBodyKind::Executable,
                actual: header.body_kind,
            });
        }
        if !definition
            .retained
            .iter()
            .map(|input| input.ty)
            .eq(expected.capture_types().iter().copied())
        {
            return Err(RuntimePlanBuildError::DialogueEffectCaptureTypeMismatch {
                site: effect.site,
            });
        }
        if effect.captures.len() != definition.retained.len() {
            return Err(RuntimePlanBuildError::DialogueEffectCaptureCountMismatch {
                site: effect.site,
                expected: definition.retained.len(),
                actual: effect.captures.len(),
            });
        }
        let captures = effect
            .captures
            .into_vec()
            .into_iter()
            .zip(&definition.retained)
            .map(|(value, input)| {
                let value = self.lower_expression(value)?;
                super::lower::require_same(
                    "dialogue effect capture expression",
                    input.ty,
                    value.ty(),
                )?;
                Ok(value)
            })
            .collect::<Result<_, RuntimePlanBuildError>>()?;
        Ok(RuntimeDialogueEffectSite::new(effect.site, state, captures))
    }
}
