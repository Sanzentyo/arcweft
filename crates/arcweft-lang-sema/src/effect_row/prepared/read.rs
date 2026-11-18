//! Borrow the original live or extracted relation without publishing it.

use super::*;
use super::{scoped::ScopedObligation, transaction::PreparedEffectDeltaData};

mod parameters;

pub(super) mod dependencies;
mod freeze;

pub(super) use freeze::SnapshotSource;

#[cfg(test)]
mod tests;

/// A borrowed read capability, never a checkpoint or publication permit. The
/// extracted variant retains the complete original delta beside its exact live
/// base; immutable borrows prevent that base changing during an operation.
#[derive(Clone, Copy)]
pub(crate) struct PreparedEffectReadView<'a> {
    relation: &'a PreparedEffectDefinitions,
    scope: Option<&'a scope::ActiveEffectScope>,
    extracted: Option<&'a PreparedEffectDeltaData>,
    dependencies: Option<&'a PreparedEffectReadScope>,
}

impl PreparedEffectGraph {
    pub(crate) fn read(&self) -> PreparedEffectReadView<'_> {
        PreparedEffectReadView {
            relation: &self.relation,
            scope: self.active_scope.as_ref(),
            extracted: None,
            dependencies: None,
        }
    }

    pub(crate) fn read_extracted<'a, C: DecisionControl>(
        &'a self,
        delta: &'a PreparedEffectDelta,
        control: &mut C,
    ) -> Result<PreparedEffectReadView<'a>, PreparedEffectFailure<C::Error>> {
        self.validate_delta(delta, control)?;
        Ok(PreparedEffectReadView {
            relation: &self.relation,
            scope: self.active_scope.as_ref(),
            extracted: Some(&delta.data),
            dependencies: delta.dependencies.as_ref(),
        })
    }
}

impl PreparedEffectDefinitions {
    pub(super) fn read(&self) -> PreparedEffectReadView<'_> {
        PreparedEffectReadView {
            relation: self,
            scope: None,
            extracted: None,
            dependencies: None,
        }
    }
}

impl<'a> PreparedEffectReadView<'a> {
    /// Check a live contextual hint for the test that distinguishes a bare
    /// open parameter from a constant body row. Identity substitutions keep
    /// the property without reifying an inference function as a final row.
    #[cfg(test)]
    pub(crate) fn is_single_open_parameter(self, mut value: &'a PreparedEffectRow) -> bool {
        let mut work = super::test_support::budget();
        loop {
            if work.charge(DecisionWork::Visit).is_err() {
                return false;
            }
            let (formula, substitutions) = match value {
                PreparedEffectRow::Canonical(row) => {
                    if let Some(formula) = row.formula() {
                        (formula, None)
                    } else {
                        return false;
                    }
                }
                PreparedEffectRow::Term(reference) => match self.row(reference) {
                    Ok(RowTerm::Application {
                        function: FunctionRef::Unknown(unknown),
                        arguments,
                    }) => {
                        return arguments.is_empty() && self.validate_unknown(unknown).is_ok();
                    }
                    Ok(RowTerm::Substitution { formula, arguments }) => (formula, Some(arguments)),
                    _ => return false,
                },
            };
            let Some(variable) = formula.variables().next() else {
                return false;
            };
            let Ok(single) = EffectFormula::variable(variable.clone(), &mut work) else {
                return false;
            };
            if *formula != single {
                return false;
            }
            match substitutions.and_then(|arguments| arguments.get(variable)) {
                Some(replacement) => value = replacement,
                None => return true,
            }
        }
    }

    /// Enumerate the visible inventories without allocating a copied index.
    /// The same immutable definition can occur in several retained layers.
    pub(super) fn visible_function_ids(self) -> impl Iterator<Item = u64> + 'a {
        self.relation
            .functions
            .keys()
            .chain(
                self.scope
                    .into_iter()
                    .flat_map(|scope| scope.functions.keys()),
            )
            .chain(
                self.scope
                    .into_iter()
                    .flat_map(|scope| scope.input.data.functions.keys()),
            )
            .chain(
                self.dependencies
                    .into_iter()
                    .flat_map(|scope| scope.data.functions.keys()),
            )
            .chain(
                self.extracted
                    .into_iter()
                    .flat_map(|delta| delta.functions.keys()),
            )
            .copied()
    }

    pub(crate) fn validate_unknown(
        self,
        unknown: &PreparedEffectUnknownRef,
    ) -> Result<(), PreparedEffectInvariant> {
        self.function(&FunctionRef::Unknown(unknown.clone()))
            .map(|_| ())
    }

    pub(crate) fn unknown_dependencies(
        self,
        unknown: &PreparedEffectUnknownRef,
    ) -> Result<&'a [GenericEffectReference], PreparedEffectInvariant> {
        Ok(self
            .function(&FunctionRef::Unknown(unknown.clone()))?
            .parameters())
    }

    pub(crate) fn retain_row<C: DecisionControl>(
        self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectRow, PreparedEffectFailure<C::Error>> {
        self.validate_retained_row(row)?;
        match row {
            PreparedEffectRow::Canonical(row) => row
                .encode(&mut super::super::EffectEqualityControl(control))
                .map_err(PreparedEffectFailure::Control)?,
            PreparedEffectRow::Term(_) => control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?,
        }
        Ok(row.clone())
    }

    pub(crate) fn retain_predicate<C: DecisionControl>(
        self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectPredicate, PreparedEffectFailure<C::Error>> {
        self.validate_retained_predicate(predicate)?;
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => predicate
                .encode(&mut super::super::EffectEqualityControl(control))
                .map_err(PreparedEffectFailure::Control)?,
            PreparedEffectPredicate::Term(_) => control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?,
        }
        Ok(predicate.clone())
    }

    pub(in crate::effect_row::prepared) fn function_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<Function>> {
        self.extracted
            .and_then(|delta| delta.functions.get(&id))
            .or_else(|| {
                self.dependencies
                    .and_then(|scope| scope.data.functions.get(&id))
            })
            .or_else(|| self.scope.and_then(|scope| scope.functions.get(&id)))
            .or_else(|| {
                self.scope
                    .and_then(|scope| scope.input.data.functions.get(&id))
            })
            .or_else(|| self.relation.functions.get(&id))
    }

    pub(in crate::effect_row::prepared) fn row_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<RowTerm>> {
        self.extracted
            .and_then(|delta| delta.rows.get(&id))
            .or_else(|| self.dependencies.and_then(|scope| scope.data.rows.get(&id)))
            .or_else(|| self.relation.rows.get(&id))
            .or_else(|| self.scope.and_then(|scope| scope.input.data.rows.get(&id)))
    }

    pub(in crate::effect_row::prepared) fn predicate_definition(
        self,
        id: u64,
    ) -> Option<&'a Arc<PredicateTerm>> {
        self.extracted
            .and_then(|delta| delta.predicates.get(&id))
            .or_else(|| {
                self.dependencies
                    .and_then(|scope| scope.data.predicates.get(&id))
            })
            .or_else(|| self.relation.predicates.get(&id))
            .or_else(|| {
                self.scope
                    .and_then(|scope| scope.input.data.predicates.get(&id))
            })
    }

    pub(in crate::effect_row::prepared) fn contains_choice(self, id: u64) -> bool {
        self.extracted
            .is_some_and(|delta| delta.choices.contains(&id))
            || self
                .dependencies
                .is_some_and(|scope| scope.data.choices.contains(&id))
            || self.relation.choices.contains(&id)
            || self
                .scope
                .is_some_and(|scope| scope.input.data.choices.contains(&id))
    }

    pub(in crate::effect_row::prepared) fn validate_choice(
        self,
        choice: &PreparedEffectChoice,
    ) -> Result<(), PreparedEffectInvariant> {
        if self.contains_choice(self.relation.validate(&choice.0)?) {
            Ok(())
        } else {
            Err(PreparedEffectInvariant::StaleReference)
        }
    }
    pub(crate) fn validate_owner(
        self,
        owner: &PreparedEffectGraphOwner,
    ) -> Result<(), PreparedEffectInvariant> {
        if Arc::ptr_eq(&owner.0, &self.relation.issuer) {
            Ok(())
        } else {
            Err(PreparedEffectInvariant::ForeignIssuer)
        }
    }

    pub(crate) fn validate_retained_row(
        self,
        row: &PreparedEffectRow,
    ) -> Result<(), PreparedEffectInvariant> {
        if let PreparedEffectRow::Term(reference) = row {
            self.row(reference)?;
        }
        Ok(())
    }

    pub(crate) fn validate_retained_predicate(
        self,
        predicate: &PreparedEffectPredicate,
    ) -> Result<(), PreparedEffectInvariant> {
        if let PreparedEffectPredicate::Term(reference) = predicate {
            self.predicate(reference)?;
        }
        Ok(())
    }

    pub(in crate::effect_row::prepared) fn function(
        self,
        reference: &FunctionRef,
    ) -> Result<&'a Function, PreparedEffectInvariant> {
        let id = self.relation.validate(reference.reference())?;
        let value = self
            .function_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        match (reference, value) {
            (FunctionRef::Scheme(_), Function::Scheme(_) | Function::SchemeInput(_))
            | (FunctionRef::Unknown(_), Function::Unknown { .. })
            | (FunctionRef::Imported(_), Function::Imported(_)) => Ok(value),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }

    pub(in crate::effect_row::prepared) fn scheme(
        self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&'a Scheme, PreparedEffectInvariant> {
        match self.function(&FunctionRef::Scheme(reference.clone()))? {
            Function::Scheme(value) => Ok(value),
            _ => Err(PreparedEffectInvariant::StaleReference),
        }
    }

    pub(in crate::effect_row::prepared) fn row(
        self,
        reference: &PreparedEffectRowRef,
    ) -> Result<&'a RowTerm, PreparedEffectInvariant> {
        let id = self.relation.validate(&reference.0)?;
        self.row_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(in crate::effect_row::prepared) fn predicate(
        self,
        reference: &PreparedEffectPredicateRef,
    ) -> Result<&'a PredicateTerm, PreparedEffectInvariant> {
        let id = self.relation.validate(&reference.0)?;
        self.predicate_definition(id)
            .map(Arc::as_ref)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }
}

impl PreparedEffectReadView<'_> {
    /// Admit a caller-owned Γ, D and R without appending a branch's facts to
    /// every other branch in the live graph. The lexical owner authenticates
    /// actual frames and source permissions before lending these references.
    pub(crate) fn scoped_constraint<C: DecisionControl>(
        self,
        parameters: &[GenericEffectReference],
        domain: &PreparedEffectPredicate,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectScopedConstraint, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let scope = PreparedEffectGraph::validate_parameters(parameters, &mut work)?;
        for parameter in parameters {
            work.charge(DecisionWork::Visit)?;
            if matches!(parameter, GenericEffectReference::Inference(_)) {
                return Err(PreparedEffectInvariant::ParameterContract.into());
            }
        }
        for value in [domain, predicate] {
            let mut admitted = true;
            self.visit_predicate_parameters(value, work.0, &mut |reference, _| {
                admitted &= scope.contains(reference);
                Ok(())
            })?;
            if !admitted {
                return Err(PreparedEffectInvariant::ParameterContract.into());
            }
        }
        let domain = self.retain_predicate(domain, work.0)?;
        let predicate = self.retain_predicate(predicate, work.0)?;
        let mut retained = Vec::new();
        for parameter in parameters {
            work.charge(DecisionWork::Node)?;
            retained.push(parameter.clone());
        }
        work.charge(DecisionWork::Node)?;
        Ok(PreparedEffectScopedConstraint {
            owner: PreparedEffectGraphOwner(Arc::clone(&self.relation.issuer)),
            obligation: Arc::new(ScopedObligation {
                parameters: retained.into_boxed_slice(),
                domain,
                predicate,
            }),
        })
    }
}
