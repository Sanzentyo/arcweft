//! Target availability checks consume the final selected application rows.

use super::statements::expression_span;
use super::{Analyzer, CheckedCallableCatalog, EffectId, EffectSet, FinalSemanticAnalysisError};
use crate::callable::CheckedCallResult;

impl Analyzer<'_, '_, '_> {
    pub(super) fn validate_target_capabilities(
        &mut self,
        callables: &CheckedCallableCatalog,
    ) -> Result<(), FinalSemanticAnalysisError> {
        let environment = self.catalogs.world.environment().typecheck_env();
        let available_effects = environment
            .available_effects()
            .map(|effects| {
                let mut effects =
                    EffectSet::from_labels(effects.iter().map(|effect| effect.as_str()))
                        .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                // Suspension is supplied by the execution engine, not a host adapter.
                effects.insert(EffectId::control_suspend());
                Ok::<_, FinalSemanticAnalysisError>(effects)
            })
            .transpose()?;
        let available_calls = environment.available_host_calls();
        if available_effects.is_none() && available_calls.is_none() {
            return Ok(());
        }
        for (owner, fact) in self.facts.calls() {
            self.control.check()?;
            let Some(application) = fact.selected_application() else {
                continue;
            };
            if let Some(available) = &available_effects {
                let required = application
                    .core()
                    .effects()
                    .closed_value()
                    .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
                let unavailable = required.effects_not_covered_by(available);
                if !unavailable.is_empty() {
                    return Err(FinalSemanticAnalysisError::TargetCapabilityUnavailable {
                        owner: *owner,
                        unavailable,
                        call_source: expression_span(self.module(owner.module())?, *owner)?,
                    });
                }
            }
            if let Some(available) = available_calls
                && !matches!(application.result(), CheckedCallResult::Continuation(_))
                && let Some(checked) = application.core().candidates().selected().checked()
                && let Some(contract) = callables
                    .callable(checked)
                    .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?
                    .host_call_contract()
                && !available.contains(&contract)
            {
                return Err(FinalSemanticAnalysisError::TargetHostCallUnavailable {
                    owner: *owner,
                    contract,
                    call_source: expression_span(self.module(owner.module())?, *owner)?,
                });
            }
        }
        Ok(())
    }
}
