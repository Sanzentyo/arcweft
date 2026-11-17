//! Preserve the original open relation without a mutation or publication permit.

use super::dependencies::{Dependency, Reachable, SchemeDependencies};
use super::*;

#[cfg(test)]
mod tests;

impl PreparedEffectReadView<'_> {
    /// Named declarations are inputs even when their bodies are already known.
    /// The local relation keeps every original C and inference function, while
    /// only terms reachable from those constraints or the actual roots survive.
    pub(crate) fn freeze_scoped<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut layers = BTreeMap::new();
        for scope in self
            .scope
            .map(|scope| &scope.input)
            .into_iter()
            .chain(self.dependencies)
        {
            for (&id, layer) in &scope.data.layers {
                work.charge(DecisionWork::Visit)?;
                if let Some(previous) = layers.get(&id) {
                    if !Arc::ptr_eq(previous, layer) {
                        return Err(PreparedEffectInvariant::ReadScopeConflict.into());
                    }
                } else {
                    work.charge(DecisionWork::Node)?;
                    layers.insert(id, Arc::clone(layer));
                }
            }
        }
        let mut inventory = BTreeSet::new();
        for id in self
            .relation
            .functions
            .keys()
            .chain(
                self.scope
                    .into_iter()
                    .flat_map(|scope| scope.functions.keys()),
            )
            .chain(layers.values().flat_map(|layer| layer.functions.keys()))
            .chain(
                self.extracted
                    .into_iter()
                    .flat_map(|delta| delta.functions.keys()),
            )
        {
            work.charge(DecisionWork::Node)?;
            inventory.insert(*id);
        }
        let mut obligations = Vec::new();
        for obligation in self
            .relation
            .obligations
            .iter()
            .chain(layers.values().flat_map(|layer| layer.obligations.iter()))
            .chain(
                self.extracted
                    .into_iter()
                    .flat_map(|delta| delta.obligations.iter()),
            )
        {
            work.charge(DecisionWork::Node)?;
            obligations.push(Arc::clone(obligation));
        }
        let mut reachable = Reachable::new(self, SchemeDependencies::Headers);
        for id in inventory {
            reachable.push(Dependency::Function(id), work.0)?;
        }
        for row in rows {
            reachable.push(Dependency::Row(row), work.0)?;
        }
        for predicate in predicates {
            reachable.push(Dependency::Predicate(predicate), work.0)?;
        }
        for obligation in &obligations {
            reachable.push(Dependency::Predicate(&obligation.domain), work.0)?;
            reachable.push(Dependency::Predicate(&obligation.predicate), work.0)?;
        }
        let reachable = reachable.collect(work.0)?;
        let mut functions = BTreeMap::new();
        for id in reachable.functions {
            work.charge(DecisionWork::Node)?;
            let definition = self
                .function_definition(id)
                .ok_or(PreparedEffectInvariant::StaleReference)?;
            let value = match definition.as_ref() {
                Function::Scheme(scheme) => {
                    work.charge(DecisionWork::Node)?;
                    Arc::new(Function::SchemeInput(
                        PreparedEffectDeclaration::from_header(
                            PreparedEffectSchemeRef(self.relation.reference(id)),
                            Arc::clone(&scheme.header),
                        ),
                    ))
                }
                _ => Arc::clone(definition),
            };
            functions.insert(id, value);
        }
        let mut rows = BTreeMap::new();
        for id in reachable.rows {
            work.charge(DecisionWork::Node)?;
            rows.insert(
                id,
                Arc::clone(
                    self.row_definition(id)
                        .ok_or(PreparedEffectInvariant::StaleReference)?,
                ),
            );
        }
        let mut predicates = BTreeMap::new();
        for id in reachable.predicates {
            work.charge(DecisionWork::Node)?;
            predicates.insert(
                id,
                Arc::clone(
                    self.predicate_definition(id)
                        .ok_or(PreparedEffectInvariant::StaleReference)?,
                ),
            );
        }
        work.charge(DecisionWork::Node)?;
        PreparedEffectDefinitions {
            issuer: Arc::clone(&self.relation.issuer),
            choices: reachable.choices,
            functions,
            obligations,
            rows,
            predicates,
        }
        .complete_scoped(work.0)
    }
}
