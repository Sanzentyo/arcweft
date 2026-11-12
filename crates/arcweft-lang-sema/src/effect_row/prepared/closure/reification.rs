//! Confirmation of the canonical scheme tuple under all retained witnesses.

use super::*;

impl PrincipalEffectClosure<'_> {
    pub(crate) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&EffectRow, PreparedEffectInvariant> {
        self.graph.scheme(reference)?;
        self.schemes
            .get(&reference.0.ordinal)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(super) fn confirm<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for (&id, scheme) in &self.graph.schemes {
            let row = self.schemes[&id].known()?;
            for (label, _) in row.membership().overrides() {
                work.charge(DecisionWork::Visit)?;
                if self.layout.labels.binary_search(label).is_err() {
                    return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                }
            }
            for reference in row.variables() {
                work.charge(DecisionWork::Visit)?;
                if !scheme.header.parameters.contains(reference) {
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
                for (parameter, reference) in scheme.header.parameters.iter().enumerate() {
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
                    let least = self
                        .completion
                        .lower_envelope()
                        .get(&Atom::Cell {
                            scheme: id,
                            class,
                            mask,
                        })
                        .ok_or(PreparedEffectInvariant::UnclosedCell)?;
                    if !least.is_constant(value) {
                        return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
                    }
                }
            }
            let scope = FiniteRowValuation {
                classes: self.layout.view().label_classes(),
                owner: FormalOwner::Scheme(id),
                parameters: &scheme.header.parameters,
            };
            canonical::CanonicalReplay::new(self, scope).confirm(id, scheme, work)?;
        }
        Ok(())
    }
}
