//! Borrow the free references of an unevaluated term, without reading bodies.

use super::*;

enum ParameterInput<'a> {
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

impl PreparedEffectReadView<'_> {
    /// Unknown annotations have no declared references to admit. This visit
    /// does not close them or establish that they denote the empty row.
    pub(crate) fn visit_row_parameters<C: DecisionControl>(
        self,
        row: &PreparedEffectRow,
        control: &mut C,
        visit: &mut impl FnMut(&GenericEffectReference, &mut C) -> Result<(), C::Error>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.visit_parameters(ParameterInput::Row(row), control, visit)
    }

    pub(crate) fn visit_predicate_parameters<C: DecisionControl>(
        self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
        visit: &mut impl FnMut(&GenericEffectReference, &mut C) -> Result<(), C::Error>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.visit_parameters(ParameterInput::Predicate(predicate), control, visit)
    }

    fn visit_parameters<C: DecisionControl>(
        self,
        root: ParameterInput<'_>,
        control: &mut C,
        visit: &mut impl FnMut(&GenericEffectReference, &mut C) -> Result<(), C::Error>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        work.charge(DecisionWork::Node)?;
        let mut pending = vec![root];
        let mut rows = BTreeSet::new();
        let mut predicates = BTreeSet::new();
        while let Some(input) = pending.pop() {
            work.charge(DecisionWork::Visit)?;
            match input {
                ParameterInput::Row(PreparedEffectRow::Canonical(row)) => {
                    for parameter in row.formula().into_iter().flat_map(EffectFormula::variables) {
                        work.charge(DecisionWork::Visit)?;
                        visit(parameter, work.0).map_err(PreparedEffectFailure::Control)?;
                    }
                }
                ParameterInput::Row(PreparedEffectRow::Term(reference)) => {
                    let term = self.row(reference)?;
                    if rows.contains(&reference.0.ordinal) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    rows.insert(reference.0.ordinal);
                    match term {
                        RowTerm::Application { function, .. } => {
                            // Scheme formals and domain belong to the declaration,
                            // not the caller's Γ. Only actual arguments are free.
                            self.function(function)?;
                        }
                        RowTerm::Substitution { formula, arguments } => {
                            for parameter in formula.variables() {
                                work.charge(DecisionWork::Visit)?;
                                if !arguments.contains_key(parameter) {
                                    visit(parameter, work.0)
                                        .map_err(PreparedEffectFailure::Control)?;
                                }
                            }
                        }
                        RowTerm::Select { choice, .. } => self.validate_choice(choice)?,
                        RowTerm::Union(_) => {}
                    }
                    for child in term.children().rev() {
                        work.charge(DecisionWork::Node)?;
                        pending.push(ParameterInput::Row(child));
                    }
                }
                ParameterInput::Predicate(PreparedEffectPredicate::Canonical(predicate)) => {
                    for parameter in predicate.variables() {
                        work.charge(DecisionWork::Visit)?;
                        visit(parameter, work.0).map_err(PreparedEffectFailure::Control)?;
                    }
                }
                ParameterInput::Predicate(PreparedEffectPredicate::Term(reference)) => {
                    let term = self.predicate(reference)?;
                    if predicates.contains(&reference.0.ordinal) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    predicates.insert(reference.0.ordinal);
                    match term {
                        PredicateTerm::Substitution {
                            predicate,
                            arguments,
                        } => {
                            for parameter in predicate.variables() {
                                work.charge(DecisionWork::Visit)?;
                                if !arguments.contains_key(parameter) {
                                    visit(parameter, work.0)
                                        .map_err(PreparedEffectFailure::Control)?;
                                }
                            }
                            for row in arguments.values().rev() {
                                work.charge(DecisionWork::Node)?;
                                pending.push(ParameterInput::Row(row));
                            }
                        }
                        PredicateTerm::Relation {
                            actual, permitted, ..
                        } => {
                            for row in [actual, permitted].into_iter().rev() {
                                work.charge(DecisionWork::Node)?;
                                pending.push(ParameterInput::Row(row));
                            }
                        }
                        PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                            for child in children.iter().rev() {
                                work.charge(DecisionWork::Node)?;
                                pending.push(ParameterInput::Predicate(child));
                            }
                        }
                        PredicateTerm::Not(child) => {
                            work.charge(DecisionWork::Node)?;
                            pending.push(ParameterInput::Predicate(child));
                        }
                        PredicateTerm::Select { choice, yes, no } => {
                            self.validate_choice(choice)?;
                            for child in [yes, no].into_iter().rev() {
                                work.charge(DecisionWork::Node)?;
                                pending.push(ParameterInput::Predicate(child));
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
