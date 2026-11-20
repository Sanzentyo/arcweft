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
        let proof = canonical::CanonicalProof::new(self);
        for &id in self.functions.keys() {
            let function = &self.graph.functions[&id];
            let row = self.functions[&id].known()?;
            for (label, _) in row.membership().overrides() {
                work.charge(DecisionWork::Visit)?;
                if self.layout.labels.binary_search(label).is_err() {
                    return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                }
            }
            let mut parameters = BTreeMap::new();
            for reference in row.variables() {
                work.charge(DecisionWork::Visit)?;
                if let std::collections::btree_map::Entry::Vacant(slot) =
                    parameters.entry(reference)
                {
                    let index = function
                        .parameters()
                        .iter()
                        .position(|parameter| parameter == reference)
                        .ok_or_else(|| {
                            PreparedEffectInvariant::UndeclaredParameter(reference.clone())
                        })?;
                    work.charge(DecisionWork::Node)?;
                    slot.insert(index);
                }
            }
            // Compare the original finite inputs with their admitted cells and
            // the reconstructed outputs with the least tuple. Fixed-point
            // replay alone would also accept a larger solution to S=S.
            for mask in 0..self.layout.tables[&id] {
                for class in 0..self.layout.view().classes() {
                    let value = self
                        .layout
                        .view()
                        .at(row.membership(), class)
                        .evaluate_with_control(
                            |reference| {
                                let &index = parameters.get(reference).expect(
                                    "canonical scheme references were checked against their header",
                                );
                                mask & (1usize << index) != 0
                            },
                            work,
                        )?;
                    let least = if matches!(function.as_ref(), Function::Imported(_)) {
                        let coefficient =
                            self.layout.imports.functions[&id].coefficient(class, mask)?;
                        self.coefficients.evaluate(coefficient, work)?
                    } else {
                        self.coefficients.value(&Atom::Cell {
                            scheme: id,
                            class,
                            mask,
                        })?
                    };
                    if least != value {
                        return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                    }
                }
            }
            if let Function::Scheme(scheme) = function.as_ref() {
                let scope = FiniteRowValuation {
                    classes: self.layout.view().label_classes(),
                    owner: FormalOwner::Scheme(id),
                    parameters: function.parameters(),
                };
                proof.replay(scope).confirm(id, scheme, work)?;
            }
        }
        let mut replays = BTreeMap::new();
        for (id, obligation) in self.graph.obligations.iter().enumerate() {
            work.charge(DecisionWork::Visit)?;
            // Only choices leave a proof's local valuation. With identical
            // ordered parameters, terms can share a local numbering while
            // each obligation is proved separately in its original order.
            let parameters = obligation.parameters.as_ref();
            let replay = match replays.entry(parameters) {
                std::collections::btree_map::Entry::Occupied(entry) => entry.into_mut(),
                std::collections::btree_map::Entry::Vacant(entry) => {
                    work.charge(DecisionWork::Node)?;
                    let scope = FiniteRowValuation {
                        classes: self.layout.view().label_classes(),
                        owner: FormalOwner::Obligation(id),
                        parameters,
                    };
                    entry.insert(proof.replay(scope))
                }
            };
            replay.confirm_obligation(obligation, work)?;
        }
        Ok(())
    }
}

impl PrincipalEffectClosure {
    fn imported_closures<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<Vec<PrincipalEffectClosure>, PreparedEffectFailure<C::Error>> {
        let mut children = Vec::new();
        for table in self.layout.imports.attachments() {
            work.charge(DecisionWork::Visit)?;
            if let Some(child) = table.specialize(self, work)? {
                work.charge(DecisionWork::Node)?;
                children.push(child);
            }
        }
        Ok(children)
    }
}
