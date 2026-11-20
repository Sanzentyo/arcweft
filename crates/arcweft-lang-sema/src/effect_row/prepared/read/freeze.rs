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
    #[cfg(test)]
    Bound(&'a ScopedEffectCompletion),
    #[cfg(test)]
    Final(&'a PreparedEffectDefinitions),
    FinalOriginal {
        original: &'a ScopedEffectCompletion,
        binding: &'a [&'a PreparedEffectScopedConstraint],
    },
    #[cfg(test)]
    FinalBound(&'a PreparedEffectDefinitions),
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
            #[cfg(test)]
            SnapshotSource::Final(_) | SnapshotSource::FinalBound(_) => SchemeDependencies::Bodies,
            SnapshotSource::FinalOriginal { .. } => SchemeDependencies::Bodies,
            SnapshotSource::Observation(_) => SchemeDependencies::Observation,
            _ => SchemeDependencies::Headers,
        };
        let original = match source {
            SnapshotSource::FinalOriginal { original, .. } => Some(original.definitions()),
            #[cfg(test)]
            SnapshotSource::Bound(original) => Some(original.definitions()),
            #[cfg(test)]
            SnapshotSource::Final(original) | SnapshotSource::FinalBound(original) => {
                Some(original)
            }
            _ => None,
        };
        let reuse_reachable = match source {
            SnapshotSource::FinalOriginal { original, .. } => original.has_verified_reachability(),
            #[cfg(test)]
            SnapshotSource::Bound(original) => original.has_verified_reachability(),
            #[cfg(test)]
            SnapshotSource::FinalBound(_) => true,
            _ => false,
        };
        let mut reachable = Reachable::new(self, schemes);
        let mut obligations = Vec::new();
        let mut inventory = BTreeSet::new();
        let mut first_new_obligation = 0;
        if let Some(original) = original {
            self.validate_owner(&PreparedEffectGraphOwner(Arc::clone(&original.issuer)))?;
            for (&id, definition) in &original.functions {
                work.charge(DecisionWork::Visit)?;
                let current = self
                    .function_definition(id)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                match definition.as_ref() {
                    Function::SchemeInput(declaration) => declaration.validate_read(self)?,
                    _ if !Arc::ptr_eq(definition, current) => {
                        return Err(PreparedEffectInvariant::ImportedRelationOrigin.into());
                    }
                    _ => {}
                }
                if reuse_reachable
                    && (matches!(schemes, SchemeDependencies::Headers)
                        || !matches!(
                            definition.as_ref(),
                            Function::SchemeInput(_) | Function::Scheme(_)
                        ))
                {
                    reachable.retain_function(id, current, &mut work)?;
                } else {
                    work.charge(DecisionWork::Node)?;
                    inventory.insert(id);
                }
            }
            if reuse_reachable {
                reachable.retain_terms(original, work.0)?;
                first_new_obligation = original.obligations.len();
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
        // Binding facts extend original C before any live obligations, just
        // as they do when the attachment has already been frozen.
        if let SnapshotSource::FinalOriginal { binding, .. } = source {
            self.retain_constraints(binding, &mut obligations, &mut work)?;
        }
        let include_live = match source {
            #[cfg(test)]
            SnapshotSource::Bound(_) => false,
            _ => true,
        };
        if include_live {
            self.retain_published_obligations(&mut obligations, &mut work)?;
        }
        self.retain_constraints(constraints, &mut obligations, &mut work)?;
        for id in inventory {
            reachable.push(Dependency::Function(id), work.0)?;
        }
        for row in rows {
            reachable.push(Dependency::Row(row), work.0)?;
        }
        if let Some(original) = original.filter(|_| !reuse_reachable) {
            for (&id, term) in &original.rows {
                let row = PreparedEffectRow::Term(PreparedEffectRowRef::new(
                    original.reference(id),
                    term,
                ));
                reachable.push(Dependency::Row(&row), work.0)?;
            }
        }
        for predicate in predicates {
            reachable.push(Dependency::Predicate(predicate), work.0)?;
        }
        if let Some(original) = original.filter(|_| !reuse_reachable) {
            for &id in original.predicates.keys() {
                let predicate = PreparedEffectPredicate::Term(PreparedEffectPredicateRef(
                    original.reference(id),
                ));
                reachable.push(Dependency::Predicate(&predicate), work.0)?;
            }
        }
        for obligation in &obligations[first_new_obligation..] {
            reachable.push(Dependency::Predicate(&obligation.domain), work.0)?;
            reachable.push(Dependency::Predicate(&obligation.predicate), work.0)?;
        }
        let mut reachable = reachable.collect(work.0)?;
        if matches!(schemes, SchemeDependencies::Headers) {
            for (&id, definition) in &mut reachable.functions {
                if let Function::Scheme(scheme) = definition.as_ref() {
                    work.charge(DecisionWork::Node)?;
                    *definition = Arc::new(Function::SchemeInput(
                        PreparedEffectDeclaration::from_header(
                            PreparedEffectSchemeRef(self.relation.reference(id)),
                            Arc::clone(&scheme.header),
                        ),
                    ));
                }
            }
        }
        work.charge(DecisionWork::Node)?;
        Ok(PreparedEffectDefinitions {
            issuer: Arc::clone(&self.relation.issuer),
            choices: reachable.choices,
            functions: reachable.functions,
            obligations,
            rows: reachable.rows,
            predicates: reachable.predicates,
        })
    }

    fn retain_constraints<C: DecisionControl>(
        self,
        constraints: &[&PreparedEffectScopedConstraint],
        obligations: &mut Vec<Arc<ScopedObligation>>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for constraint in constraints {
            work.charge(DecisionWork::Visit)?;
            self.validate_owner(&constraint.owner)?;
            self.validate_retained_predicate(&constraint.obligation.domain)?;
            self.validate_retained_predicate(&constraint.obligation.predicate)?;
            work.charge(DecisionWork::Node)?;
            obligations.push(Arc::clone(&constraint.obligation));
        }
        Ok(())
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
