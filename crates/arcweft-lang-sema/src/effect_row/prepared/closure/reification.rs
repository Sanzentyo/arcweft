//! Confirmation of the canonical scheme tuple under all retained witnesses.

use super::*;

impl PrincipalEffectClosure {
    pub(crate) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&EffectRow, PreparedEffectInvariant> {
        self.graph.scheme(reference)?;
        self.functions
            .get(&reference.0.ordinal)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(crate) fn unknown(
        &self,
        reference: &PreparedEffectUnknownRef,
    ) -> Result<&EffectRow, PreparedEffectInvariant> {
        let function = FunctionRef::Unknown(reference.clone());
        self.graph.function(&function)?;
        self.functions
            .get(&function.reference().ordinal)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(super) fn confirm<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.confirm_local(work)?;
        let mut todo = self.imported_closures(work)?;
        while let Some(child) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            child.confirm_local(work)?;
            for nested in child.imported_closures(work)? {
                work.charge(DecisionWork::Node)?;
                todo.push(nested);
            }
        }
        Ok(())
    }

    fn confirm_local<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for &id in self.functions.keys() {
            let function = &self.graph.functions[&id];
            let row = self.functions[&id].known()?;
            for (label, _) in row.membership().overrides() {
                work.charge(DecisionWork::Visit)?;
                if self.layout.labels.binary_search(label).is_err() {
                    return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                }
            }
            for reference in row.variables() {
                work.charge(DecisionWork::Visit)?;
                if !function.parameters().contains(reference) {
                    return Err(
                        PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into(),
                    );
                }
            }
            // Fixed-point replay alone would also accept a larger solution to
            // S=S. Check that every canonical table entry is the retained least
            // cell before independently replaying the original typed terms.
            for mask in 0..self.layout.tables[&id] {
                let mut values = BTreeMap::new();
                for (parameter, reference) in function.parameters().iter().enumerate() {
                    work.charge(DecisionWork::Node)?;
                    values.insert(reference, mask & (1usize << parameter) != 0);
                }
                for class in 0..self.layout.view().classes() {
                    let value = self
                        .layout
                        .view()
                        .at(row.membership(), class)
                        .evaluate_with_control(
                            |reference| {
                                *values.get(reference).expect(
                                    "canonical scheme references were checked against their header",
                                )
                            },
                            work,
                        )?;
                    let imported;
                    let least = if matches!(function.as_ref(), Function::Imported(_)) {
                        imported = self.layout.imports.functions[&id]
                            .coefficient(self.layout.view(), class, mask, work)?
                            .substitute(&self.coefficients, work)?;
                        &imported
                    } else {
                        self.coefficients
                            .get(&Atom::Cell {
                                scheme: id,
                                class,
                                mask,
                            })
                            .ok_or(PreparedEffectInvariant::UnclosedCell)?
                    };
                    if !least.is_constant(value) {
                        return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                    }
                }
            }
            let scope = FiniteRowValuation {
                classes: self.layout.view().label_classes(),
                owner: FormalOwner::Scheme(id),
                parameters: function.parameters(),
            };
            if let Function::Scheme(scheme) = function.as_ref() {
                canonical::CanonicalReplay::new(self, scope).confirm(id, scheme, work)?;
            } else if function.is_input() {
                let row = self
                    .input_bindings
                    .get(&id)
                    .ok_or(PreparedEffectInvariant::UnresolvedFuture)?;
                canonical::CanonicalReplay::new(self, scope).confirm_fixed(id, row, work)?;
            }
        }
        for (id, obligation) in self.graph.obligations.iter().enumerate() {
            let scope = FiniteRowValuation {
                classes: self.layout.view().label_classes(),
                owner: FormalOwner::Obligation(id),
                parameters: &obligation.parameters,
            };
            canonical::CanonicalReplay::new(self, scope).confirm_obligation(obligation, work)?;
        }
        Ok(())
    }
}

impl PrincipalEffectClosure {
    fn imported_closures<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<Vec<PrincipalEffectClosure>, PreparedEffectFailure<C::Error>> {
        let graph = &self.graph;
        let mut children = Vec::new();
        let functions = self
            .functions
            .keys()
            .filter_map(|id| match graph.functions[id].as_ref() {
                Function::Imported(imported) => Some(&imported.relation),
                _ => None,
            });
        let predicates =
            graph
                .predicates
                .values()
                .filter_map(|predicate| match predicate.as_ref() {
                    PredicateTerm::ImportedPrincipal(imported) => Some(imported),
                    _ => None,
                });
        let mut relations: Vec<
            &Arc<crate::effect_row::prepared::completion::ImportedEffectRelation>,
        > = Vec::new();
        for imported in functions.chain(predicates) {
            work.charge(DecisionWork::Visit)?;
            let mut seen = false;
            for previous in &relations {
                work.charge(DecisionWork::Visit)?;
                seen |= Arc::ptr_eq(imported, previous);
            }
            if seen {
                continue;
            }
            work.charge(DecisionWork::Node)?;
            relations.push(imported);
        }
        for imported in relations {
            let table = self.layout.imports.relation(imported, work)?;
            if let Some(child) = table.specialize(imported, self, work)? {
                work.charge(DecisionWork::Node)?;
                children.push(child);
            }
        }
        Ok(children)
    }
}
