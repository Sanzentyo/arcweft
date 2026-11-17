//! Final closure reads actual bodies and inputs while preserving original C.

use super::*;
use crate::effect_row::prepared::read::dependencies::SchemeDependencies;

#[cfg(test)]
mod tests;

impl ScopedEffectCompletion {
    /// A final mathematical closure requires every actual Future row and the
    /// actual original Named bodies. It neither prepares a body nor chooses a
    /// convenient replacement for an unavailable input. No live facts change.
    pub(crate) fn close_original<C: DecisionControl>(
        &self,
        receiving: PreparedEffectReadView<'_>,
        query_rows: &[&PreparedEffectRow],
        query_predicates: &[&PreparedEffectPredicate],
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        let owner = PreparedEffectGraphOwner(Arc::clone(&self.graph.issuer));
        receiving.validate_owner(&owner)?;
        if !self.fixed.is_empty() {
            return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
        }
        for (&id, definition) in &self.graph.functions {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            if let Function::SchemeInput(declaration) = definition.as_ref() {
                declaration.validate_read(receiving)?;
            } else {
                let actual = receiving
                    .function_definition(id)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                if !Arc::ptr_eq(definition, actual) {
                    return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
                }
            }
        }
        let mut rows = Vec::new();
        for &id in self.graph.rows.keys() {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            rows.push(PreparedEffectRow::Term(PreparedEffectRowRef(
                self.graph.reference(id),
            )));
        }
        let mut predicates = Vec::new();
        for &id in self.graph.predicates.keys() {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            predicates.push(PreparedEffectPredicate::Term(PreparedEffectPredicateRef(
                self.graph.reference(id),
            )));
        }
        let mut row_ports = Vec::new();
        for row in &rows {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            row_ports.push(row);
        }
        for row in query_rows {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            row_ports.push(row);
        }
        let mut predicate_ports = Vec::new();
        for predicate in &predicates {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            predicate_ports.push(predicate);
        }
        for predicate in query_predicates {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            predicate_ports.push(predicate);
        }
        let mut receipts = Vec::new();
        for obligation in &self.graph.obligations {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            receipts.push(PreparedEffectScopedConstraint {
                owner: owner.clone(),
                obligation: Arc::clone(obligation),
            });
        }
        let mut constraints = Vec::new();
        for receipt in &receipts {
            control
                .charge(DecisionWork::Node)
                .map_err(PreparedEffectFailure::Control)?;
            constraints.push(receipt);
        }
        receiving
            .snapshot(
                &row_ports,
                &predicate_ports,
                &constraints,
                SchemeDependencies::Bodies,
                control,
            )?
            .specialize_principal(bindings, control)
    }
}
