//! Borrow the free references of an unevaluated term, without reading bodies.

use super::*;

enum ParameterInput<'a> {
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

enum ParameterChildren<'a> {
    None,
    Rows(RowChildren<'a>),
    Predicates(std::slice::Iter<'a, PreparedEffectPredicate>),
    PredicatePair(std::array::IntoIter<&'a PreparedEffectPredicate, 2>),
}

impl ParameterChildren<'_> {
    fn is_empty(&self) -> bool {
        match self {
            Self::None => true,
            Self::Rows(rows) => rows.is_empty(),
            Self::Predicates(predicates) => predicates.as_slice().is_empty(),
            Self::PredicatePair(predicates) => predicates.as_slice().is_empty(),
        }
    }
}

impl<'a> Iterator for ParameterChildren<'a> {
    type Item = ParameterInput<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        match self {
            Self::None => None,
            Self::Rows(rows) => rows.next().map(ParameterInput::Row),
            Self::Predicates(predicates) => predicates.next().map(ParameterInput::Predicate),
            Self::PredicatePair(predicates) => predicates.next().map(ParameterInput::Predicate),
        }
    }
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
        let mut next = Some(root);
        let mut pending: Vec<ParameterChildren<'_>> = Vec::new();
        let mut rows = BTreeSet::new();
        let mut predicates = BTreeSet::new();
        loop {
            let input = if let Some(input) = next.take() {
                input
            } else if let Some(siblings) = pending.last_mut() {
                let Some(input) = siblings.next() else {
                    pending.pop();
                    continue;
                };
                input
            } else {
                return Ok(());
            };
            work.charge(DecisionWork::Visit)?;
            let mut children = match input {
                ParameterInput::Row(PreparedEffectRow::Canonical(row)) => {
                    for parameter in row.formula().into_iter().flat_map(EffectFormula::variables) {
                        work.charge(DecisionWork::Visit)?;
                        visit(parameter, work.0).map_err(PreparedEffectFailure::Control)?;
                    }
                    ParameterChildren::None
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
                    ParameterChildren::Rows(term.children())
                }
                ParameterInput::Predicate(PreparedEffectPredicate::Canonical(predicate)) => {
                    for parameter in predicate.variables() {
                        work.charge(DecisionWork::Visit)?;
                        visit(parameter, work.0).map_err(PreparedEffectFailure::Control)?;
                    }
                    ParameterChildren::None
                }
                ParameterInput::Predicate(PreparedEffectPredicate::Term(reference)) => {
                    let term = self.predicate(reference)?;
                    if predicates.contains(&reference.0.ordinal) {
                        continue;
                    }
                    work.charge(DecisionWork::Node)?;
                    predicates.insert(reference.0.ordinal);
                    match term {
                        PredicateTerm::ImportedPrincipal(imported) => {
                            imported.validate(self, work.0)?;
                            ParameterChildren::None
                        }
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
                            ParameterChildren::Rows(RowChildren::Arguments(arguments.values()))
                        }
                        PredicateTerm::Relation {
                            actual, permitted, ..
                        } => ParameterChildren::Rows(RowChildren::Pair(
                            [actual, permitted].into_iter(),
                        )),
                        PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                            ParameterChildren::Predicates(children.iter())
                        }
                        PredicateTerm::Equivalent(children) => {
                            ParameterChildren::Predicates(children.iter())
                        }
                        PredicateTerm::Not(child) => {
                            ParameterChildren::Predicates(std::slice::from_ref(child).iter())
                        }
                        PredicateTerm::Select { choice, yes, no } => {
                            self.validate_choice(choice)?;
                            ParameterChildren::PredicatePair([yes, no].into_iter())
                        }
                    }
                }
            };
            next = children.next();
            // Keep a borrowed cursor for siblings. A wide term does not need
            // one queued allocation per edge, and unary terms need no frame.
            if !children.is_empty() {
                work.charge(DecisionWork::Node)?;
                pending.push(children);
            }
        }
    }
}
