//! Independent replay through canonical formula algebra, without scheme cells.

use super::*;
use crate::effects::EffectSet;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Variable {
    Parameter(GenericEffectReference),
    Choice(u64),
}
type Formula = arcweft_core::effect_row::EffectFormula<Variable>;
type Predicate = arcweft_core::effect_row::EffectPredicate<Variable>;

struct Row {
    formula: Formula,
    admitted: Decision,
}

pub(super) struct CanonicalReplay<'a> {
    principal: &'a PrincipalEffectClosure<'a>,
    scope: FiniteRowValuation<'a>,
    rows: BTreeMap<u64, Row>,
    predicates: BTreeMap<u64, Decision>,
}

impl<'a> CanonicalReplay<'a> {
    pub(super) fn new(
        principal: &'a PrincipalEffectClosure<'a>,
        scope: FiniteRowValuation<'a>,
    ) -> Self {
        Self {
            principal,
            scope,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    fn formula<C: DecisionControl>(
        &self,
        row: &EffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Formula, PreparedEffectFailure<C::Error>> {
        row.known()?.map_references(work, &mut |reference, _| {
            Ok(Variable::Parameter(reference.clone()))
        })
    }

    fn canonical_predicate<C: DecisionControl>(
        &self,
        predicate: &EffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Predicate, PreparedEffectFailure<C::Error>> {
        predicate.map_references(work, &mut |reference, _| {
            Ok(Variable::Parameter(reference.clone()))
        })
    }

    fn scalar<C: DecisionControl>(
        &self,
        decision: &EffectDecision<Variable>,
        class: usize,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        decision.map_references(work, &mut |variable, _| match variable {
            Variable::Choice(id) => Ok(Atom::Choice(*id)),
            Variable::Parameter(reference) => self
                .scope
                .parameters
                .iter()
                .position(|parameter| parameter == reference)
                .map(|parameter| Atom::Formal {
                    owner: self.scope.owner,
                    class,
                    parameter,
                })
                .ok_or_else(|| {
                    PreparedEffectInvariant::UndeclaredParameter(reference.clone()).into()
                }),
        })
    }

    fn guard<C: DecisionControl>(
        &self,
        predicate: &Predicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut members = Vec::new();
        for class in 0..self.scope.classes.classes() {
            work.charge(DecisionWork::Node)?;
            members.push(self.scalar(
                self.scope.classes.at(predicate.membership(), class),
                class,
                work,
            )?);
        }
        self.scope.whole(&members, work)
    }

    fn finite<C: DecisionControl>(
        &self,
        row: &Formula,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let mut zero = BTreeMap::new();
        for variable in row.membership().default_decision().variables() {
            if matches!(variable, Variable::Parameter(_)) {
                work.charge(DecisionWork::Node)?;
                zero.insert(variable.clone(), EffectDecision::constant(false));
            }
        }
        // Uniform choices are scalars, not finite row variables. Finiteness
        // must hold for every choice when all actual row parameters are empty.
        if !row
            .membership()
            .default_decision()
            .substitute(&zero, work)?
            .is_constant(false)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(())
    }

    fn read_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Row, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => Ok(Row {
                formula: self.formula(row, work)?,
                admitted: Decision::constant(true),
            }),
            PreparedEffectRow::Term(reference) => {
                self.principal.graph.row(reference)?;
                let row = self
                    .rows
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                Ok(Row {
                    formula: row
                        .formula
                        .map_references(work, &mut |variable, _| Ok(variable.clone()))?,
                    admitted: row
                        .admitted
                        .map_references(work, &mut |atom, _| Ok(atom.clone()))?,
                })
            }
        }
    }

    fn row<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Row, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(row) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectRow::Term(reference) = row {
                let term = self.principal.graph.row(reference)?;
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
        for id in reachable {
            let term = self.principal.graph.rows[&id].as_ref();
            let row = match term {
                RowTerm::Application {
                    function,
                    arguments,
                } => {
                    let definition = self.principal.graph.function(function)?;
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    for (parameter, argument) in
                        definition.parameters().iter().zip(arguments.iter())
                    {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions
                            .insert(Variable::Parameter(parameter.clone()), argument.formula);
                    }
                    if let Some(domain) = definition.domain() {
                        let domain = self
                            .canonical_predicate(domain, work)?
                            .substitute(&substitutions, work)?;
                        admitted = admitted.and(&self.guard(&domain, work)?, work)?;
                    }
                    let formula = self
                        .formula(
                            &self.principal.functions[&function.reference().ordinal],
                            work,
                        )?
                        .substitute(&substitutions, work)?;
                    Row { formula, admitted }
                }
                RowTerm::Substitution { formula, arguments } => {
                    let source = formula.map_references(work, &mut |reference, _| {
                        Ok(Variable::Parameter(reference.clone()))
                    })?;
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    for (parameter, argument) in arguments {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions
                            .insert(Variable::Parameter(parameter.clone()), argument.formula);
                    }
                    Row {
                        formula: source.substitute(&substitutions, work)?,
                        admitted,
                    }
                }
                RowTerm::Union(children) => {
                    let mut formula = Formula::from_set(&EffectSet::new(), work)?;
                    let mut admitted = Decision::constant(true);
                    for child in children {
                        let child = self.read_row(child, work)?;
                        formula = formula.union(&child.formula, work)?;
                        admitted = admitted.and(&child.admitted, work)?;
                    }
                    Row { formula, admitted }
                }
                RowTerm::Select { choice, yes, no } => {
                    self.principal.graph.validate_choice(choice)?;
                    let yes = self.read_row(yes, work)?;
                    let no = self.read_row(no, work)?;
                    let bit = EffectDecision::variable(Variable::Choice(choice.0.ordinal), work)?;
                    let default = bit.conditional(
                        yes.formula.membership().default_decision(),
                        no.formula.membership().default_decision(),
                        work,
                    )?;
                    let mut labels = BTreeSet::new();
                    for (label, _) in yes
                        .formula
                        .membership()
                        .overrides()
                        .chain(no.formula.membership().overrides())
                    {
                        work.charge(DecisionWork::Node)?;
                        labels.insert(label);
                    }
                    let mut overrides = BTreeMap::new();
                    for label in labels {
                        work.charge(DecisionWork::Node)?;
                        overrides.insert(
                            label.clone(),
                            bit.conditional(
                                yes.formula.membership().at(label),
                                no.formula.membership().at(label),
                                work,
                            )?,
                        );
                    }
                    let formula = Formula::try_from_decisions(default, overrides, work)?;
                    let admitted = Decision::variable(Atom::Choice(choice.0.ordinal), work)?
                        .conditional(&yes.admitted, &no.admitted, work)?;
                    Row { formula, admitted }
                }
            };
            self.finite(&row.formula, work)?;
            work.charge(DecisionWork::Node)?;
            self.rows.insert(id, row);
        }
        self.read_row(root, work)
    }

    fn closed<C: DecisionControl>(
        &self,
        row: &Formula,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut parameters = BTreeSet::new();
        let mut zero = BTreeMap::new();
        for variable in row.variables() {
            if matches!(variable, Variable::Parameter(_)) {
                work.charge(DecisionWork::Node)?;
                parameters.insert(variable.clone());
                work.charge(DecisionWork::Node)?;
                zero.insert(variable.clone(), EffectDecision::constant(false));
            }
        }
        let mut closed = EffectDecision::constant(true);
        for class in 0..self.scope.classes.classes() {
            let value = self.scope.classes.at(row.membership(), class);
            let base = value.substitute(&zero, work)?;
            let equal = value
                .conditional(&base, &base.not(work)?, work)?
                .forall(&parameters, work)?;
            closed = closed.and(&equal, work)?;
        }
        self.scalar(&closed, 0, work)
    }

    fn at_zero<C: DecisionControl>(
        &self,
        value: &EffectDecision<Variable>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut bindings = BTreeMap::new();
        for variable in value.variables() {
            if matches!(variable, Variable::Parameter(_)) {
                work.charge(DecisionWork::Node)?;
                bindings.insert(variable.clone(), EffectDecision::constant(false));
            }
        }
        self.scalar(&value.substitute(&bindings, work)?, 0, work)
    }

    fn covered<C: DecisionControl>(
        &self,
        actual: &Formula,
        permitted: &Formula,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut result = self
            .at_zero(actual.membership().default_decision(), work)?
            .not(work)?;
        for (label, member) in actual.membership().overrides() {
            let mut allowance = Decision::constant(false);
            for (capability, allowed) in permitted.membership().overrides() {
                work.charge(DecisionWork::Visit)?;
                if capability.covers(label) {
                    allowance = allowance.or(&self.at_zero(allowed, work)?, work)?;
                }
            }
            result = result.and(
                &self.at_zero(member, work)?.conditional(
                    &allowance,
                    &Decision::constant(true),
                    work,
                )?,
                work,
            )?;
        }
        Ok(result)
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
        let subset = actual.formula.subset(&permitted.formula, work)?;
        let predicate = if relation == PreparedEffectRelation::Equal {
            subset.and(&permitted.formula.subset(&actual.formula, work)?, work)?
        } else {
            subset
        };
        let exact = self.guard(&predicate, work)?;
        let result = if relation == PreparedEffectRelation::AdmittedSubset {
            let closed = self
                .closed(&actual.formula, work)?
                .and(&self.closed(&permitted.formula, work)?, work)?;
            closed.conditional(
                &self.covered(&actual.formula, &permitted.formula, work)?,
                &exact,
                work,
            )?
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
                self.guard(&self.canonical_predicate(predicate, work)?, work)
            }
            PreparedEffectPredicate::Term(reference) => {
                self.principal.graph.predicate(reference)?;
                self.predicates
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?
                    .map_references(work, &mut |atom, _| Ok(atom.clone()))
            }
        }
    }

    fn predicate<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(predicate) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectPredicate::Term(reference) = predicate {
                let term = self.principal.graph.predicate(reference)?;
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
                            for child in [yes, no] {
                                work.charge(DecisionWork::Node)?;
                                todo.push(child);
                            }
                        }
                        PredicateTerm::Relation { .. } | PredicateTerm::Substitution { .. } => {}
                    }
                }
            }
        }
        for id in reachable {
            let term = self.principal.graph.predicates[&id].as_ref();
            let value = match term {
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    let source = self.canonical_predicate(predicate, work)?;
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    for (parameter, argument) in arguments {
                        let argument = self.row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions
                            .insert(Variable::Parameter(parameter.clone()), argument.formula);
                    }
                    self.guard(&source.substitute(&substitutions, work)?, work)?
                        .and(&admitted, work)?
                }
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => self.relation(*relation, actual, permitted, work)?,
                PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                    let and = matches!(term, PredicateTerm::And(_));
                    let mut value = Decision::constant(and);
                    for child in children {
                        let child = self.read_predicate(child, work)?;
                        value = if and {
                            value.and(&child, work)?
                        } else {
                            value.or(&child, work)?
                        };
                    }
                    value
                }
                PredicateTerm::Not(child) => self.read_predicate(child, work)?.not(work)?,
                PredicateTerm::Select { choice, yes, no } => {
                    self.principal.graph.validate_choice(choice)?;
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

    pub(super) fn confirm<C: DecisionControl>(
        &mut self,
        id: u64,
        scheme: &Scheme,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let SchemeState::BodyPrepared { body, obligations } = &scheme.state else {
            return Err(PreparedEffectInvariant::SchemeState.into());
        };
        let body = self.row(body, work)?;
        let own = self.formula(&self.principal.functions[&id], work)?;
        let equal = own
            .subset(&body.formula, work)?
            .and(&body.formula.subset(&own, work)?, work)?;
        let mut expected = self.guard(&equal, work)?.and(&body.admitted, work)?;
        for obligation in obligations {
            expected = expected.and(&self.predicate(obligation, work)?, work)?;
        }
        let domain = self.guard(
            &self.canonical_predicate(&scheme.header.domain, work)?,
            work,
        )?;
        let expected = domain.conditional(&expected, &Decision::constant(true), work)?;
        if !self
            .principal
            .witnesses
            .and(&expected.not(work)?, work)?
            .is_constant(false)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(())
    }

    pub(super) fn confirm_obligation<C: DecisionControl>(
        &mut self,
        obligation: &ScopedObligation,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let domain = self.predicate(&obligation.domain, work)?;
        let predicate = self.predicate(&obligation.predicate, work)?;
        let expected = domain.conditional(&predicate, &Decision::constant(true), work)?;
        if !self
            .principal
            .witnesses
            .and(&expected.not(work)?, work)?
            .is_constant(false)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(())
    }

    pub(super) fn confirm_fixed<C: DecisionControl>(
        &mut self,
        id: u64,
        row: &EffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let actual = self.formula(&self.principal.functions[&id], work)?;
        let expected = self.formula(row, work)?;
        self.finite(&expected, work)?;
        let equal = actual
            .subset(&expected, work)?
            .and(&expected.subset(&actual, work)?, work)?;
        if !self
            .principal
            .witnesses
            .and(&self.guard(&equal, work)?.not(work)?, work)?
            .is_constant(false)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(())
    }
}
