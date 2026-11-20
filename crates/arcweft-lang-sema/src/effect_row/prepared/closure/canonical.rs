//! Independent replay through canonical formula algebra, without scheme cells.

use std::{borrow::Cow, cell::RefCell, collections::btree_map::Entry, rc::Rc};

use super::*;
use crate::effects::EffectSet;

#[cfg(test)]
mod tests;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Variable {
    Parameter(GenericEffectReference),
    Choice(u64),
}

impl Variable {
    fn empty_row_parameter(&self) -> Option<bool> {
        matches!(self, Self::Parameter(_)).then_some(false)
    }
}

type Formula = arcweft_core::effect_row::EffectFormula<Variable>;
type Predicate = arcweft_core::effect_row::EffectPredicate<Variable>;

struct FunctionFormula {
    formula: Formula,
    uses_parameters: bool,
}

enum RowFormula {
    Derived(Formula),
    Function(Rc<FunctionFormula>),
}

impl RowFormula {
    fn as_formula(&self) -> &Formula {
        match self {
            Self::Derived(formula) => formula,
            Self::Function(function) => &function.formula,
        }
    }
}

struct Row {
    formula: RowFormula,
    admitted: Decision,
}

struct RowView<'a> {
    formula: Cow<'a, Formula>,
    admitted: Cow<'a, Decision>,
}

pub(super) struct CanonicalProof<'a> {
    principal: &'a PrincipalEffectClosure,
    functions: RefCell<BTreeMap<u64, Rc<FunctionFormula>>>,
}

impl<'a> CanonicalProof<'a> {
    pub(super) fn new(principal: &'a PrincipalEffectClosure) -> Self {
        Self {
            principal,
            functions: RefCell::new(BTreeMap::new()),
        }
    }

    pub(super) fn replay<'scope>(
        &'scope self,
        scope: FiniteRowValuation<'scope>,
    ) -> CanonicalReplay<'scope> {
        CanonicalReplay {
            proof: self,
            scope,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    fn function<C: DecisionControl>(
        &self,
        id: u64,
        work: &mut Work<'_, C>,
    ) -> Result<Rc<FunctionFormula>, PreparedEffectFailure<C::Error>> {
        work.charge(DecisionWork::Visit)?;
        let mut functions = self.functions.borrow_mut();
        match functions.entry(id) {
            Entry::Occupied(slot) => Ok(Rc::clone(slot.get())),
            Entry::Vacant(slot) => {
                // The typed references of a function's canonical formula are
                // independent of the caller's valuation. Scalar admission is
                // still evaluated within each replay.
                let row = self
                    .principal
                    .functions
                    .get(&id)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                let mut uses_parameters = false;
                let formula = row.known()?.map_references(work, &mut |reference, _| {
                    uses_parameters = true;
                    Ok(Variable::Parameter(reference.clone()))
                })?;
                work.charge(DecisionWork::Node)?;
                Ok(Rc::clone(slot.insert(Rc::new(FunctionFormula {
                    formula,
                    uses_parameters,
                }))))
            }
        }
    }
}

pub(super) struct CanonicalReplay<'a> {
    proof: &'a CanonicalProof<'a>,
    scope: FiniteRowValuation<'a>,
    rows: BTreeMap<u64, Row>,
    predicates: BTreeMap<u64, Decision>,
}

impl CanonicalReplay<'_> {
    fn formula<C: DecisionControl>(
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
        if predicate.is_unconstrained() {
            work.charge(DecisionWork::Visit)?;
            return Ok(Decision::constant(true));
        }
        let membership = predicate.membership();
        if membership.default_decision().is_constant(true) {
            let mut named = Vec::new();
            for (label, point) in membership.overrides() {
                work.charge(DecisionWork::Visit)?;
                let class = self
                    .scope
                    .classes
                    .named(label)
                    .ok_or(PreparedEffectInvariant::UnclosedCell)?;
                let point = self.scalar(point, class, work)?;
                work.charge(DecisionWork::Node)?;
                named.push(point);
            }
            return Decision::all(&named, work);
        }
        let mut members = Vec::new();
        for class in 0..self.scope.classes.classes() {
            work.charge(DecisionWork::Node)?;
            members.push(self.scalar(self.scope.classes.at(membership, class), class, work)?);
        }
        self.scope.whole(&members, work)
    }

    fn finite<C: DecisionControl>(
        &self,
        row: &Formula,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        // Uniform choices are scalars, not finite row variables. Finiteness
        // must hold for every choice when all actual row parameters are empty.
        if !row
            .membership()
            .default_decision()
            .restrict_references(Variable::empty_row_parameter, work)?
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
    ) -> Result<RowView<'_>, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => Ok(RowView {
                formula: Cow::Owned(Self::formula(row, work)?),
                admitted: Cow::Owned(Decision::constant(true)),
            }),
            PreparedEffectRow::Term(reference) => {
                self.proof.principal.graph.row(reference)?;
                work.charge(DecisionWork::Visit)?;
                let row = self
                    .rows
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                Ok(RowView {
                    formula: Cow::Borrowed(row.formula.as_formula()),
                    admitted: Cow::Borrowed(&row.admitted),
                })
            }
        }
    }

    fn prepare_row<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if matches!(root, PreparedEffectRow::Canonical(_)) {
            return Ok(());
        }
        let principal = self.proof.principal;
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(row) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectRow::Term(reference) = row {
                let term = principal.graph.row(reference)?;
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
            let term = principal.graph.rows[&id].as_ref();
            let row = match term {
                RowTerm::Application {
                    function,
                    arguments,
                } => {
                    let definition = principal.graph.function(function)?;
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
                    if let Some(domain) = definition
                        .domain()
                        .filter(|domain| !domain.is_unconstrained())
                    {
                        let domain = self.canonical_predicate(domain, work)?;
                        let domain = if substitutions.is_empty() {
                            domain
                        } else {
                            domain.substitute(&substitutions, work)?
                        };
                        admitted = admitted.and(&self.guard(&domain, work)?, work)?;
                    }
                    if let Function::Imported(_) = definition {
                        let imported = principal
                            .layout
                            .imports
                            .functions
                            .get(&function.reference().ordinal)
                            .ok_or(PreparedEffectInvariant::PrincipalConfirmation)?;
                        let admitted_by_import = principal
                            .coefficients
                            .evaluate(imported.relation.principal(), work)?;
                        admitted = admitted.and(&Decision::constant(admitted_by_import), work)?;
                    }
                    let formula = self.proof.function(function.reference().ordinal, work)?;
                    let formula = if substitutions.is_empty() || !formula.uses_parameters {
                        RowFormula::Function(formula)
                    } else {
                        RowFormula::Derived(formula.formula.substitute(&substitutions, work)?)
                    };
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
                    let formula = if substitutions.is_empty() {
                        source
                    } else {
                        source.substitute(&substitutions, work)?
                    };
                    Row {
                        formula: RowFormula::Derived(formula),
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
                    Row {
                        formula: RowFormula::Derived(formula),
                        admitted,
                    }
                }
                RowTerm::Select { choice, yes, no } => {
                    principal.graph.validate_choice(choice)?;
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
                    let mut formula = Formula::builder(default, work)?;
                    for label in labels {
                        formula.set(
                            label,
                            bit.conditional(
                                yes.formula.membership().at(label),
                                no.formula.membership().at(label),
                                work,
                            )?,
                            work,
                        )?;
                    }
                    let formula = formula.finish();
                    let admitted = Decision::variable(Atom::Choice(choice.0.ordinal), work)?
                        .conditional(&yes.admitted, &no.admitted, work)?;
                    Row {
                        formula: RowFormula::Derived(formula),
                        admitted,
                    }
                }
            };
            // The identity mapping of a canonical function's parameters
            // preserves finiteness. Only derived rows can add uniform choices.
            if let RowFormula::Derived(formula) = &row.formula {
                self.finite(formula, work)?;
            }
            work.charge(DecisionWork::Node)?;
            self.rows.insert(id, row);
        }
        Ok(())
    }

    fn closed<C: DecisionControl>(
        &self,
        row: &Formula,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let mut has_choices = false;
        let mut parameters = BTreeSet::new();
        for variable in row.variables() {
            work.charge(DecisionWork::Visit)?;
            match variable {
                Variable::Choice(_) => has_choices = true,
                Variable::Parameter(_) if !parameters.contains(variable) => {
                    work.charge(DecisionWork::Node)?;
                    parameters.insert(variable.clone());
                }
                Variable::Parameter(_) => {}
            }
        }
        // A reachable parameter in a reduced point decision is essential.
        // Choices are the only variables that can switch that dependence.
        if parameters.is_empty() || !has_choices {
            return Ok(Decision::constant(parameters.is_empty()));
        }
        let mut closed = EffectDecision::constant(true);
        for class in 0..self.scope.classes.classes() {
            let independent = self
                .scope
                .classes
                .at(row.membership(), class)
                .independence_region(&parameters, work)?;
            closed = closed.and(&independent, work)?;
        }
        self.scalar(&closed, 0, work)
    }

    fn at_zero<C: DecisionControl>(
        &self,
        value: &EffectDecision<Variable>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.scalar(
            &value.restrict_references(Variable::empty_row_parameter, work)?,
            0,
            work,
        )
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
        self.prepare_row(actual, work)?;
        self.prepare_row(permitted, work)?;
        let actual = self.read_row(actual, work)?;
        let permitted = self.read_row(permitted, work)?;
        let result = relation.decide_membership(
            [actual.formula.as_ref(), permitted.formula.as_ref()],
            work,
            |row, work| self.closed(row, work),
            |work| {
                let predicate = if relation == PreparedEffectRelation::Equal {
                    actual.formula.equivalent(&permitted.formula, work)?
                } else {
                    actual.formula.subset(&permitted.formula, work)?
                };
                self.guard(&predicate, work)
            },
            |work| self.covered(&actual.formula, &permitted.formula, work),
        )?;
        result
            .and(&actual.admitted, work)?
            .and(&permitted.admitted, work)
    }

    fn read_predicate<C: DecisionControl>(
        &self,
        predicate: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Cow<'_, Decision>, PreparedEffectFailure<C::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) if predicate.is_unconstrained() => {
                work.charge(DecisionWork::Visit)?;
                Ok(Cow::Owned(Decision::constant(true)))
            }
            PreparedEffectPredicate::Canonical(predicate) => self
                .guard(&self.canonical_predicate(predicate, work)?, work)
                .map(Cow::Owned),
            PreparedEffectPredicate::Term(reference) => {
                self.proof.principal.graph.predicate(reference)?;
                work.charge(DecisionWork::Visit)?;
                let predicate = self
                    .predicates
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                Ok(Cow::Borrowed(predicate))
            }
        }
    }

    fn prepare_predicate<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if matches!(root, PreparedEffectPredicate::Canonical(_)) {
            return Ok(());
        }
        let principal = self.proof.principal;
        let mut todo = vec![root];
        let mut reachable = BTreeSet::new();
        while let Some(predicate) = todo.pop() {
            work.charge(DecisionWork::Visit)?;
            if let PreparedEffectPredicate::Term(reference) = predicate {
                let term = principal.graph.predicate(reference)?;
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
                        PredicateTerm::Relation { .. }
                        | PredicateTerm::Substitution { .. }
                        | PredicateTerm::ImportedPrincipal(_) => {}
                    }
                }
            }
        }
        for id in reachable {
            let term = principal.graph.predicates[&id].as_ref();
            let value = match term {
                PredicateTerm::ImportedPrincipal(imported) => {
                    let guard = principal
                        .layout
                        .imports
                        .relation(imported, work)?
                        .principal();
                    Decision::constant(principal.coefficients.evaluate(guard, work)?)
                }
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    for argument in arguments.values() {
                        self.prepare_row(argument, work)?;
                    }
                    let source = self.canonical_predicate(predicate, work)?;
                    let mut substitutions = BTreeMap::new();
                    let mut admitted = Decision::constant(true);
                    for (parameter, argument) in arguments {
                        let argument = self.read_row(argument, work)?;
                        admitted = admitted.and(&argument.admitted, work)?;
                        work.charge(DecisionWork::Node)?;
                        substitutions
                            .insert(Variable::Parameter(parameter.clone()), argument.formula);
                    }
                    let predicate = if substitutions.is_empty() {
                        source
                    } else {
                        source.substitute(&substitutions, work)?
                    };
                    self.guard(&predicate, work)?.and(&admitted, work)?
                }
                PredicateTerm::Relation {
                    relation,
                    actual,
                    permitted,
                } => self.relation(*relation, actual, permitted, work)?,
                PredicateTerm::And(children) => work
                    .conjoin_prepared_predicates(children, |child, work| {
                        self.read_predicate(child, work)
                    })?,
                PredicateTerm::Or(children) => {
                    let mut value = Decision::constant(false);
                    for child in children {
                        let child = self.read_predicate(child, work)?;
                        value = value.or(&child, work)?;
                    }
                    value
                }
                PredicateTerm::Not(child) => self.read_predicate(child, work)?.not(work)?,
                PredicateTerm::Select { choice, yes, no } => {
                    principal.graph.validate_choice(choice)?;
                    Decision::variable(Atom::Choice(choice.0.ordinal), work)?.conditional(
                        self.read_predicate(yes, work)?.as_ref(),
                        self.read_predicate(no, work)?.as_ref(),
                        work,
                    )?
                }
            };
            work.charge(DecisionWork::Node)?;
            self.predicates.insert(id, value);
        }
        Ok(())
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
        self.prepare_row(body, work)?;
        let body = self.read_row(body, work)?;
        let own = self.proof.function(id, work)?;
        let equal = own.formula.equivalent(&body.formula, work)?;
        let mut expected = self.guard(&equal, work)?.and(&body.admitted, work)?;
        for obligation in obligations {
            self.prepare_predicate(obligation, work)?;
            expected = expected.and(self.read_predicate(obligation, work)?.as_ref(), work)?;
        }
        let domain = self.guard(
            &self.canonical_predicate(&scheme.header.domain, work)?,
            work,
        )?;
        let expected = domain.conditional(&expected, &Decision::constant(true), work)?;
        self.confirm_expected(&expected, work)
    }

    pub(super) fn confirm_obligation<C: DecisionControl>(
        &mut self,
        obligation: &ScopedObligation,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        self.prepare_predicate(&obligation.domain, work)?;
        self.prepare_predicate(&obligation.predicate, work)?;
        let domain = self.read_predicate(&obligation.domain, work)?;
        let predicate = self.read_predicate(&obligation.predicate, work)?;
        let expected = domain.conditional(&predicate, &Decision::constant(true), work)?;
        self.confirm_expected(&expected, work)
    }

    fn confirm_expected<C: DecisionControl>(
        &self,
        expected: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        if expected.is_constant(true) {
            return Ok(());
        }
        let witnesses = self.proof.principal.witnesses.relation();
        if witnesses.is_constant(true)
            || !witnesses
                .and(&expected.not(work)?, work)?
                .is_constant(false)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(())
    }
}
