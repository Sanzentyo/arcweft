//! Iterative term compilation. Applications reference cells, never callee bodies.

use super::*;

pub(super) struct CompiledRow {
    pub(super) membership: Vec<Decision>,
    pub(super) admitted: Decision,
}

pub(super) struct Compilation<'a> {
    graph: &'a PreparedEffectGraph,
    layout: &'a Layout,
    scheme: u64,
    parameters: &'a [GenericEffectReference],
    rows: BTreeMap<u64, CompiledRow>,
    predicates: BTreeMap<u64, Decision>,
}

fn copy<C: DecisionControl>(
    decision: &Decision,
    work: &mut Work<'_, C>,
) -> Result<Decision, PreparedEffectFailure<C::Error>> {
    work.charge(DecisionWork::Visit)?;
    for _ in 0..decision.node_count() {
        work.charge(DecisionWork::Node)?;
    }
    Ok(decision.clone())
}

impl<'a> Compilation<'a> {
    pub(super) fn new(
        graph: &'a PreparedEffectGraph,
        layout: &'a Layout,
        scheme: u64,
        parameters: &'a [GenericEffectReference],
    ) -> Self {
        Self {
            graph,
            layout,
            scheme,
            parameters,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    pub(super) fn formals<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<Atom>, PreparedEffectFailure<C::Error>> {
        let mut result = BTreeSet::new();
        for class in 0..self.layout.classes() {
            for parameter in 0..self.parameters.len() {
                work.charge(DecisionWork::Node)?;
                result.insert(Atom::Formal {
                    scheme: self.scheme,
                    class,
                    parameter,
                });
            }
        }
        Ok(result)
    }

    pub(super) fn parameters<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<Vec<CompiledRow>, PreparedEffectFailure<C::Error>> {
        let mut result = Vec::new();
        for parameter in 0..self.parameters.len() {
            work.charge(DecisionWork::Node)?;
            let mut membership = Vec::new();
            for class in 0..self.layout.classes() {
                membership.push(Decision::variable(
                    Atom::Formal {
                        scheme: self.scheme,
                        class,
                        parameter,
                    },
                    work,
                )?);
            }
            result.push(CompiledRow {
                membership,
                admitted: Decision::constant(true),
            });
        }
        Ok(result)
    }

    fn member<C: DecisionControl>(
        &self,
        source: &EffectDecision<GenericEffectReference>,
        class: usize,
        arguments: &BTreeMap<GenericEffectReference, CompiledRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut replacements = BTreeMap::new();
        for parameter in source.variables() {
            work.charge(DecisionWork::Visit)?;
            if replacements.contains_key(&Atom::Reference(parameter.clone())) {
                continue;
            }
            let value = if let Some(argument) = arguments.get(parameter) {
                copy(&argument.membership[class], work)?
            } else {
                let slot = self
                    .parameters
                    .iter()
                    .position(|declared| declared == parameter)
                    .ok_or_else(|| {
                        PreparedEffectInvariant::UndeclaredParameter(parameter.clone())
                    })?;
                Decision::variable(
                    Atom::Formal {
                        scheme: self.scheme,
                        class,
                        parameter: slot,
                    },
                    work,
                )?
            };
            replacements.insert(Atom::Reference(parameter.clone()), value);
        }
        source
            .map_references(work, &mut |reference, _| {
                Ok(Atom::Reference(reference.clone()))
            })?
            .substitute(&replacements, work)
    }

    fn canonical_row<C: DecisionControl>(
        &self,
        row: &EffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<CompiledRow, PreparedEffectFailure<C::Error>> {
        let view = row.known()?.membership();
        let mut membership = Vec::new();
        for class in 0..self.layout.classes() {
            work.charge(DecisionWork::Node)?;
            membership.push(self.member(
                self.layout.at(view, class),
                class,
                &BTreeMap::new(),
                work,
            )?);
        }
        Ok(CompiledRow {
            membership,
            admitted: Decision::constant(true),
        })
    }

    fn read_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<CompiledRow, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => self.canonical_row(row, work),
            PreparedEffectRow::Term(reference) => {
                self.graph.row(reference)?;
                let row = self
                    .rows
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                let mut membership = Vec::new();
                for decision in &row.membership {
                    membership.push(copy(decision, work)?);
                }
                Ok(CompiledRow {
                    membership,
                    admitted: copy(&row.admitted, work)?,
                })
            }
        }
    }

    pub(super) fn application<C: DecisionControl>(
        &self,
        scheme: u64,
        arguments: &[CompiledRow],
        work: &mut Work<'_, C>,
    ) -> Result<Vec<Decision>, PreparedEffectFailure<C::Error>> {
        let table = self.layout.tables[&scheme];
        let mut result = Vec::new();
        for class in 0..self.layout.classes() {
            let mut frontier = Vec::new();
            for mask in 0..table {
                frontier.push(Decision::variable(
                    Atom::Cell {
                        scheme,
                        class,
                        mask,
                    },
                    work,
                )?);
            }
            for argument in arguments {
                let mut next = Vec::new();
                for pair in frontier.chunks_exact(2) {
                    next.push(argument.membership[class].conditional(&pair[1], &pair[0], work)?);
                }
                frontier = next;
            }
            result.push(
                frontier
                    .pop()
                    .ok_or(PreparedEffectInvariant::ApplicationArity)?,
            );
        }
        Ok(result)
    }

    pub(super) fn row<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<CompiledRow, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(row) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectRow::Term(reference) = row {
                let term = self.graph.row(reference)?;
                if !self.rows.contains_key(&reference.0.ordinal)
                    && reachable.insert(reference.0.ordinal)
                {
                    for child in term.children() {
                        work.charge(DecisionWork::Node)?;
                        todo.push(child);
                    }
                }
            }
        }
        // Child terms are issued before their parents. Recursive edges occur
        // only through declared scheme references and do not enter this DAG.
        for id in reachable {
            let term = self.graph.rows[&id].as_ref();
            let value = match term {
                RowTerm::Application { scheme, arguments } => {
                    let mut compiled = Vec::new();
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    let header = &self.graph.scheme(scheme)?.header;
                    for (parameter, argument) in header.parameters.iter().zip(arguments.iter()) {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        compiled.push(self.read_row(&arguments[compiled.len()], work)?);
                        work.charge(DecisionWork::Node)?;
                        substitutions.insert(parameter.clone(), argument);
                    }
                    for class in 0..self.layout.classes() {
                        let domain = self.member(
                            self.layout.at(header.domain.membership(), class),
                            class,
                            &substitutions,
                            work,
                        )?;
                        admitted = admitted.and(&domain, work)?;
                    }
                    CompiledRow {
                        membership: self.application(scheme.0.ordinal, &compiled, work)?,
                        admitted,
                    }
                }
                RowTerm::Substitution { formula, arguments } => {
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    for (parameter, row) in arguments {
                        let row = self.read_row(row, work)?;
                        admitted = admitted.and(&row.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions.insert(parameter.clone(), row);
                    }
                    let mut membership = Vec::new();
                    for class in 0..self.layout.classes() {
                        membership.push(self.member(
                            self.layout.at(formula.membership(), class),
                            class,
                            &substitutions,
                            work,
                        )?);
                    }
                    CompiledRow {
                        membership,
                        admitted,
                    }
                }
                RowTerm::Union(arguments) => {
                    let mut membership = Vec::new();
                    for _ in 0..self.layout.classes() {
                        work.charge(DecisionWork::Node)?;
                        membership.push(Decision::constant(false));
                    }
                    let mut admitted = Decision::constant(true);
                    for row in arguments {
                        let row = self.read_row(row, work)?;
                        admitted = admitted.and(&row.admitted, work)?;
                        for (left, right) in membership.iter_mut().zip(&row.membership) {
                            *left = left.or(right, work)?;
                        }
                    }
                    CompiledRow {
                        membership,
                        admitted,
                    }
                }
                RowTerm::Select { choice, yes, no } => {
                    self.graph.validate_choice(choice)?;
                    let choice = Decision::variable(Atom::Choice(choice.0.ordinal), work)?;
                    let yes = self.read_row(yes, work)?;
                    let no = self.read_row(no, work)?;
                    let mut membership = Vec::new();
                    for (yes, no) in yes.membership.iter().zip(&no.membership) {
                        membership.push(choice.conditional(yes, no, work)?);
                    }
                    CompiledRow {
                        membership,
                        admitted: choice.conditional(&yes.admitted, &no.admitted, work)?,
                    }
                }
            };
            work.charge(DecisionWork::Node)?;
            self.rows.insert(id, value);
        }
        self.read_row(root, work)
    }

    pub(super) fn canonical_predicate<C: DecisionControl>(
        &self,
        predicate: &EffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut result = Decision::constant(true);
        for class in 0..self.layout.classes() {
            result = result.and(
                &self.member(
                    self.layout.at(predicate.membership(), class),
                    class,
                    &BTreeMap::new(),
                    work,
                )?,
                work,
            )?;
        }
        Ok(result)
    }

    fn closed<C: DecisionControl>(
        &self,
        row: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut result = Decision::constant(true);
        for (class, decision) in row.iter().enumerate() {
            for parameter in 0..self.parameters.len() {
                let atom = Atom::Formal {
                    scheme: self.scheme,
                    class,
                    parameter,
                };
                work.charge(DecisionWork::Node)?;
                let low = decision.substitute(
                    &BTreeMap::from([(atom.clone(), Decision::constant(false))]),
                    work,
                )?;
                let high = decision
                    .substitute(&BTreeMap::from([(atom, Decision::constant(true))]), work)?;
                result = result.and(&low.conditional(&high, &high.not(work)?, work)?, work)?;
            }
        }
        result.forall(&self.formals(work)?, work)
    }

    fn relation<C: DecisionControl>(
        &mut self,
        relation: PreparedEffectRelation,
        actual: &PreparedEffectRow,
        permitted: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let actual = self.row(actual, work)?;
        let permitted = self.row(permitted, work)?;
        let mut exact = Decision::constant(true);
        for (left, right) in actual.membership.iter().zip(&permitted.membership) {
            let test = match relation {
                PreparedEffectRelation::Equal => {
                    left.conditional(right, &right.not(work)?, work)?
                }
                PreparedEffectRelation::AdmittedSubset => {
                    left.conditional(right, &Decision::constant(true), work)?
                }
            };
            exact = exact.and(&test, work)?;
        }
        let result = if relation == PreparedEffectRelation::AdmittedSubset {
            let closed = self
                .closed(&actual.membership, work)?
                .and(&self.closed(&permitted.membership, work)?, work)?;
            let mut covered = actual.membership[0].not(work)?;
            for (index, label) in self.layout.labels.iter().enumerate() {
                let mut allowance = Decision::constant(false);
                for (other, capability) in self.layout.labels.iter().enumerate() {
                    work.charge(DecisionWork::Visit)?;
                    if capability.covers(label) {
                        allowance = allowance.or(&permitted.membership[other + 1], work)?;
                    }
                }
                covered = covered.and(
                    &actual.membership[index + 1].conditional(
                        &allowance,
                        &Decision::constant(true),
                        work,
                    )?,
                    work,
                )?;
            }
            closed.conditional(&covered, &exact, work)?
        } else {
            exact
        };
        result
            .and(&actual.admitted, work)?
            .and(&permitted.admitted, work)
    }

    fn read_predicate<C: DecisionControl>(
        &self,
        predicate: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                self.canonical_predicate(predicate, work)
            }
            PreparedEffectPredicate::Term(reference) => {
                self.graph.predicate(reference)?;
                copy(
                    self.predicates
                        .get(&reference.0.ordinal)
                        .ok_or(PreparedEffectInvariant::StaleReference)?,
                    work,
                )
            }
        }
    }

    pub(super) fn predicate<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(predicate) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectPredicate::Term(reference) = predicate {
                let term = self.graph.predicate(reference)?;
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
                        PredicateTerm::Select { yes, no, .. } => {
                            work.charge(DecisionWork::Node)?;
                            todo.push(yes);
                            work.charge(DecisionWork::Node)?;
                            todo.push(no);
                        }
                        PredicateTerm::Relation { .. } => {}
                    }
                }
            }
        }
        for id in reachable {
            let term = self.graph.predicates[&id].as_ref();
            let value = match term {
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => self.relation(*relation, actual, permitted, work)?,
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    let and = matches!(term, PredicateTerm::And(_));
                    let mut result = Decision::constant(and);
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
                PredicateTerm::Select { choice, yes, no } => {
                    self.graph.validate_choice(choice)?;
                    Decision::variable(Atom::Choice(choice.0.ordinal), work)?.conditional(
                        &self.read_predicate(yes, work)?,
                        &self.read_predicate(no, work)?,
                        work,
                    )?
                }
            };
            work.charge(DecisionWork::Node)?;
            self.predicates.insert(id, value);
        }
        self.read_predicate(root, work)
    }
}
