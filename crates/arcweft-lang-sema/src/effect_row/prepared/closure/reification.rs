//! Canonical projection and independent validation after the joint cell solve.

use super::*;

type CanonicalDecision = EffectDecision<(usize, GenericEffectReference)>;

struct CanonicalRow {
    row: EffectRow,
    admitted: CanonicalDecision,
}

struct Reification<'a> {
    component: &'a ClosedEffectComponent<'a>,
    layout: &'a Layout,
    rows: BTreeMap<u64, CanonicalRow>,
    predicates: BTreeMap<u64, CanonicalDecision>,
}

impl Reification<'_> {
    fn membership<C: DecisionControl>(
        &self,
        predicate: &EffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalDecision, PreparedEffectFailure<C::Error>> {
        let mut result = CanonicalDecision::constant(true);
        for class in 0..self.layout.classes() {
            let decision = self
                .layout
                .at(predicate.membership(), class)
                .map_references(work, &mut |parameter, _| Ok((class, parameter.clone())))?;
            result = result.and(&decision, work)?;
        }
        Ok(result)
    }

    fn copy_row<C: DecisionControl>(
        &self,
        value: &CanonicalRow,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalRow, PreparedEffectFailure<C::Error>> {
        value.row.equal_with(&value.row, work)?;
        Ok(CanonicalRow {
            row: value.row.clone(),
            admitted: value
                .admitted
                .map_references(work, &mut |reference, _| Ok(reference.clone()))?,
        })
    }

    fn read_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalRow, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => {
                row.known()?;
                row.equal_with(row, work)?;
                Ok(CanonicalRow {
                    row: row.clone(),
                    admitted: CanonicalDecision::constant(true),
                })
            }
            PreparedEffectRow::Term(reference) => {
                self.component.graph.row(reference)?;
                self.copy_row(
                    self.rows
                        .get(&reference.0.ordinal)
                        .ok_or(PreparedEffectInvariant::StaleReference)?,
                    work,
                )
            }
        }
    }

    fn row<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalRow, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(row) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectRow::Term(reference) = row {
                let term = self.component.graph.row(reference)?;
                if !self.rows.contains_key(&reference.0.ordinal)
                    && reachable.insert(reference.0.ordinal)
                {
                    // Unselected alternatives need not be admitted or projected.
                    if let RowTerm::Select { choice, yes, no } = term {
                        work.charge(DecisionWork::Node)?;
                        todo.push(if self.component.choice(choice)? {
                            yes
                        } else {
                            no
                        });
                    } else {
                        for child in term.children() {
                            work.charge(DecisionWork::Node)?;
                            todo.push(child);
                        }
                    }
                }
            }
        }
        for id in reachable {
            let term = self.component.graph.rows[&id].as_ref();
            let value = match term {
                RowTerm::Application { scheme, arguments } => {
                    let header = &self.component.graph.scheme(scheme)?.header;
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = CanonicalDecision::constant(true);
                    for (parameter, argument) in header.parameters.iter().zip(arguments.iter()) {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        substitutions.insert(parameter.clone(), argument.row.known()?.clone());
                    }
                    let domain = header.domain.substitute(&substitutions, work)?;
                    admitted = admitted.and(&self.membership(&domain, work)?, work)?;
                    let source = self
                        .component
                        .schemes
                        .get(&scheme.0.ordinal)
                        .ok_or(PreparedEffectInvariant::StaleReference)?;
                    let row = EffectRow {
                        formula: Some(source.known()?.substitute(&substitutions, work)?),
                    };
                    CanonicalRow { row, admitted }
                }
                RowTerm::Substitution { formula, arguments } => {
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = CanonicalDecision::constant(true);
                    for (parameter, argument) in arguments {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        substitutions.insert(parameter.clone(), argument.row.known()?.clone());
                    }
                    CanonicalRow {
                        row: EffectRow {
                            formula: Some(formula.substitute(&substitutions, work)?),
                        },
                        admitted,
                    }
                }
                RowTerm::Union(children) => {
                    let mut row = EffectRow::closed(Default::default());
                    let mut admitted = CanonicalDecision::constant(true);
                    for child in children {
                        let child = self.read_row(child, work)?;
                        row = row.union(&child.row, work)?;
                        admitted = admitted.and(&child.admitted, work)?;
                    }
                    CanonicalRow { row, admitted }
                }
                RowTerm::Select { choice, yes, no } => self.read_row(
                    if self.component.choice(choice)? {
                        yes
                    } else {
                        no
                    },
                    work,
                )?,
            };
            work.charge(DecisionWork::Node)?;
            self.rows.insert(id, value);
        }
        self.read_row(root, work)
    }

    fn read_predicate<C: DecisionControl>(
        &self,
        predicate: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalDecision, PreparedEffectFailure<C::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => self.membership(predicate, work),
            PreparedEffectPredicate::Term(reference) => {
                self.component.graph.predicate(reference)?;
                self.predicates
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?
                    .map_references(work, &mut |reference, _| Ok(reference.clone()))
            }
        }
    }

    fn predicate<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<CanonicalDecision, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(predicate) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectPredicate::Term(reference) = predicate {
                let term = self.component.graph.predicate(reference)?;
                if !self.predicates.contains_key(&reference.0.ordinal)
                    && reachable.insert(reference.0.ordinal)
                {
                    match term {
                        PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                            for child in children {
                                work.charge(DecisionWork::Node)?;
                                todo.push(child);
                            }
                        }
                        PredicateTerm::Not(child) => {
                            work.charge(DecisionWork::Node)?;
                            todo.push(child);
                        }
                        PredicateTerm::Select { choice, yes, no } => {
                            work.charge(DecisionWork::Node)?;
                            todo.push(if self.component.choice(choice)? {
                                yes
                            } else {
                                no
                            });
                        }
                        PredicateTerm::Relation { .. } | PredicateTerm::Substitution { .. } => {}
                    }
                }
            }
        }
        for id in reachable {
            let term = self.component.graph.predicates[&id].as_ref();
            let value = match term {
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = CanonicalDecision::constant(true);
                    for (parameter, argument) in arguments {
                        let argument = self.row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions.insert(parameter.clone(), argument.row.known()?.clone());
                    }
                    let substituted = predicate.substitute(&substitutions, work)?;
                    self.membership(&substituted, work)?.and(&admitted, work)?
                }
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => {
                    let actual = self.row(actual, work)?;
                    let permitted = self.row(permitted, work)?;
                    let relation = if *relation == PreparedEffectRelation::AdmittedSubset
                        && actual.row.is_closed()
                        && permitted.row.is_closed()
                    {
                        CanonicalDecision::constant(actual.row.is_covered_by(&permitted.row, work)?)
                    } else {
                        let actual_formula = actual.row.known()?;
                        let permitted_formula = permitted.row.known()?;
                        let subset = actual_formula.subset(permitted_formula, work)?;
                        let predicate = if *relation == PreparedEffectRelation::Equal {
                            subset.and(&permitted_formula.subset(actual_formula, work)?, work)?
                        } else {
                            subset
                        };
                        self.membership(&predicate, work)?
                    };
                    relation
                        .and(&actual.admitted, work)?
                        .and(&permitted.admitted, work)?
                }
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    let and = matches!(term, PredicateTerm::And(_));
                    let mut result = CanonicalDecision::constant(and);
                    for child in children {
                        let child = self.read_predicate(child, work)?;
                        result = if and {
                            result.and(&child, work)?
                        } else {
                            result.or(&child, work)?
                        };
                    }
                    result
                }
                PredicateTerm::Not(child) => self.read_predicate(child, work)?.not(work)?,
                PredicateTerm::Select { choice, yes, no } => self.read_predicate(
                    if self.component.choice(choice)? {
                        yes
                    } else {
                        no
                    },
                    work,
                )?,
            };
            work.charge(DecisionWork::Node)?;
            self.predicates.insert(id, value);
        }
        self.read_predicate(root, work)
    }
}

impl ClosedEffectComponent<'_> {
    pub(crate) fn choice(
        &self,
        reference: &PreparedEffectChoice,
    ) -> Result<bool, PreparedEffectInvariant> {
        self.graph.validate_choice(reference)?;
        self.choices
            .get(&reference.0.ordinal)
            .copied()
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(crate) fn scheme(
        &self,
        reference: &PreparedEffectSchemeRef,
    ) -> Result<&EffectRow, PreparedEffectInvariant> {
        self.graph.scheme(reference)?;
        self.schemes
            .get(&reference.0.ordinal)
            .ok_or(PreparedEffectInvariant::StaleReference)
    }

    pub(crate) fn resolve_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<EffectRow, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut reification = Reification {
            component: self,
            layout: &self.layout,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        };
        let row = reification.row(row, &mut work)?;
        if !row.admitted.is_constant(true) {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        Ok(row.row)
    }

    pub(super) fn confirm<C: DecisionControl>(
        &self,
        layout: &Layout,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut reification = Reification {
            component: self,
            layout,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        };
        for (&id, scheme) in &self.graph.schemes {
            let SchemeState::BodyPrepared { body, obligations } = &scheme.state else {
                return Err(PreparedEffectInvariant::SchemeState.into());
            };
            let body = reification.row(body, work)?;
            let row = self.schemes[&id].known()?;
            let equal = row
                .subset(body.row.known()?, work)?
                .and(&body.row.known()?.subset(row, work)?, work)?;
            let mut expected = reification
                .membership(&equal, work)?
                .and(&body.admitted, work)?;
            for predicate in obligations {
                expected = expected.and(&reification.predicate(predicate, work)?, work)?;
            }
            let domain = reification.membership(&scheme.header.domain, work)?;
            if !domain
                .conditional(&expected, &CanonicalDecision::constant(true), work)?
                .is_constant(true)
            {
                return Err(PreparedEffectFailure::Unsatisfiable);
            }
        }
        Ok(())
    }
}
