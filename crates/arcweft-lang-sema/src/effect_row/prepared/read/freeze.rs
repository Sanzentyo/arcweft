//! Preserve the original open relation without a mutation or publication permit.

use super::dependencies::{Dependency, Reachable, SchemeDependencies};
use super::*;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum SnapshotSource<'a> {
    #[cfg(test)]
    AllUnknowns,
    Product(&'a [&'a PreparedEffectUnknownRef]),
    Bound(&'a PreparedEffectDefinitions),
    Final(&'a PreparedEffectDefinitions),
    Observation(&'a [&'a PreparedEffectSchemeRef]),
}

impl PreparedEffectReadView<'_> {
    /// Named declarations are inputs even when their bodies are already known.
    /// Keep original C and the declared inference unknowns. Named and imported
    /// functions enter the relation through the actual roots and constraints.
    #[cfg(test)]
    pub(crate) fn freeze_scoped<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        constraints: &[&PreparedEffectScopedConstraint],
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        self.snapshot(
            rows,
            predicates,
            constraints,
            SnapshotSource::AllUnknowns,
            control,
        )?
        .complete_scoped(control)
    }

    /// An original product owns only the introduced unknowns of its selected
    /// applications. Other inputs must be reachable from its terms or C.
    pub(crate) fn freeze_product<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        constraints: &[&PreparedEffectScopedConstraint],
        introduced: &[&PreparedEffectUnknownRef],
        control: &mut C,
    ) -> Result<Arc<ScopedEffectCompletion>, PreparedEffectFailure<C::Error>> {
        self.snapshot(
            rows,
            predicates,
            constraints,
            SnapshotSource::Product(introduced),
            control,
        )?
        .complete_scoped(control)
    }

    /// Binding adds candidate receipts to original C. Final validation also
    /// includes the published obligations and follows actual Named bodies.
    pub(in crate::effect_row::prepared) fn snapshot<C: DecisionControl>(
        self,
        rows: &[&PreparedEffectRow],
        predicates: &[&PreparedEffectPredicate],
        constraints: &[&PreparedEffectScopedConstraint],
        source: SnapshotSource<'_>,
        control: &mut C,
    ) -> Result<PreparedEffectDefinitions, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let schemes = match source {
            SnapshotSource::Final(_) => SchemeDependencies::Bodies,
            SnapshotSource::Observation(_) => SchemeDependencies::Observation,
            _ => SchemeDependencies::Headers,
        };
        let original = match source {
            SnapshotSource::Bound(original) | SnapshotSource::Final(original) => Some(original),
            _ => None,
        };
        let mut retained_rows = Vec::new();
        let mut retained_predicates = Vec::new();
        let mut obligations = Vec::new();
        let mut inventory = BTreeSet::new();
        if let Some(original) = original {
            self.validate_owner(&PreparedEffectGraphOwner(Arc::clone(&original.issuer)))?;
            for (&id, definition) in &original.functions {
                work.charge(DecisionWork::Visit)?;
                if let Function::SchemeInput(declaration) = definition.as_ref() {
                    declaration.validate_read(self)?;
                } else {
                    let actual = self
                        .function_definition(id)
                        .ok_or(PreparedEffectInvariant::StaleReference)?;
                    if !Arc::ptr_eq(definition, actual) {
                        return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
                    }
                }
                work.charge(DecisionWork::Node)?;
                inventory.insert(id);
            }
            for &id in original.rows.keys() {
                work.charge(DecisionWork::Node)?;
                retained_rows.push(PreparedEffectRow::Term(PreparedEffectRowRef(
                    original.reference(id),
                )));
            }
            for &id in original.predicates.keys() {
                work.charge(DecisionWork::Node)?;
                retained_predicates.push(PreparedEffectPredicate::Term(
                    PreparedEffectPredicateRef(original.reference(id)),
                ));
            }
            for obligation in &original.obligations {
                work.charge(DecisionWork::Node)?;
                obligations.push(Arc::clone(obligation));
            }
        } else if let SnapshotSource::Product(introduced) = source {
            for reference in introduced {
                work.charge(DecisionWork::Visit)?;
                self.validate_unknown(reference)?;
                work.charge(DecisionWork::Node)?;
                inventory.insert(reference.reference().ordinal);
            }
        } else if let SnapshotSource::Observation(roots) = source {
            for root in roots {
                self.scheme(root)?;
                work.charge(DecisionWork::Node)?;
                inventory.insert(self.relation.validate(&root.0)?);
            }
        }
        #[cfg(test)]
        if matches!(source, SnapshotSource::AllUnknowns) {
            for id in self.visible_function_ids() {
                work.charge(DecisionWork::Visit)?;
                if matches!(
                    self.function_definition(id).map(Arc::as_ref),
                    Some(Function::Unknown { .. })
                ) {
                    work.charge(DecisionWork::Node)?;
                    inventory.insert(id);
                }
            }
        }
        if !matches!(source, SnapshotSource::Bound(_)) {
            self.retain_published_obligations(&mut obligations, &mut work)?;
        }
        for constraint in constraints {
            work.charge(DecisionWork::Visit)?;
            self.validate_owner(&constraint.owner)?;
            self.validate_retained_predicate(&constraint.obligation.domain)?;
            self.validate_retained_predicate(&constraint.obligation.predicate)?;
            work.charge(DecisionWork::Node)?;
            obligations.push(Arc::clone(&constraint.obligation));
        }
        let mut reachable = Reachable::new(self, schemes);
        for id in inventory {
            reachable.push(Dependency::Function(id), work.0)?;
        }
        for row in rows.iter().copied().chain(&retained_rows) {
            reachable.push(Dependency::Row(row), work.0)?;
        }
        for predicate in predicates.iter().copied().chain(&retained_predicates) {
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
                Function::Scheme(scheme) if matches!(schemes, SchemeDependencies::Headers) => {
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
        Ok(PreparedEffectDefinitions {
            issuer: Arc::clone(&self.relation.issuer),
            choices: reachable.choices,
            functions,
            obligations,
            rows,
            predicates,
        })
    }

    fn retain_published_obligations<C: DecisionControl>(
        self,
        obligations: &mut Vec<Arc<ScopedObligation>>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
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
        Ok(())
    }
}
