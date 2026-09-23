//! Target availability checks consume the final selected application rows.

use super::statements::expression_span;
use super::{Analyzer, EffectId, EffectSet, FinalSemanticAnalysisError};

impl Analyzer<'_, '_, '_> {
    pub(super) fn validate_target_effects(&mut self) -> Result<(), FinalSemanticAnalysisError> {
        let Some(available) = self
            .catalogs
            .world
            .environment()
            .typecheck_env()
            .available_effects()
        else {
            return Ok(());
        };
        let mut available = EffectSet::from_labels(available.iter().map(|effect| effect.as_str()))
            .map_err(|_| FinalSemanticAnalysisError::CheckedCallableCatalog)?;
        // Suspension is supplied by the execution engine, not a host adapter.
        available.insert(EffectId::control_suspend());
        for (owner, fact) in self.facts.calls() {
            self.control.check()?;
            let Some(application) = fact.selected_application() else {
                continue;
            };
            let required = application
                .core()
                .effects()
                .closed_value()
                .ok_or(FinalSemanticAnalysisError::CheckedCallableCatalog)?;
            let unavailable = required.effects_not_covered_by(&available);
            if !unavailable.is_empty() {
                return Err(FinalSemanticAnalysisError::TargetCapabilityUnavailable {
                    owner: *owner,
                    unavailable,
                    call_source: expression_span(self.module(owner.module())?, *owner)?,
                });
            }
        }
        Ok(())
    }
}
