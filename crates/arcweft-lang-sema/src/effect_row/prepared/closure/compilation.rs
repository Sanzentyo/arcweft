//! Iterative term compilation. Applications reference cells, never callee bodies.

use super::*;
use arcweft_core::effect_row::{EffectDecisionReplacement, EffectDecisionRequirement};
use std::borrow::{Borrow, Cow};

mod terms;

pub(super) struct CompiledRow {
    pub(super) membership: Vec<Decision>,
    pub(super) admitted: Decision,
}

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
enum PredicateInputs {
    AtValuation,
    Unrestricted,
}

impl PredicateInputs {
    fn children(self, term: &PredicateTerm) -> Self {
        match term {
            // A choice is fixed for the complete assignment of input rows.
            PredicateTerm::And(_) | PredicateTerm::Select { .. } => self,
            // Equivalence compares the operands at the same valuation before
            // quantifying the resulting predicate.
            _ => Self::AtValuation,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ApplicationCells<'a> {
    Prepared(&'a FixedCells),
    Solved {
        basis: ClassLayout<'a>,
        coefficients: &'a ClosedEffectCoefficients,
    },
}

impl<'a> ApplicationCells<'a> {
    fn basis(self, receiving: ClassLayout<'a>) -> ClassLayout<'a> {
        match self {
            Self::Prepared(_) => receiving,
            Self::Solved { basis, .. } => basis,
        }
    }

    fn complete_import<C: DecisionControl>(
        self,
        decision: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        match self {
            Self::Prepared(_) => work.copy_decision(decision),
            Self::Solved { coefficients, .. } => {
                Ok(Decision::constant(coefficients.evaluate(decision, work)?))
            }
        }
    }

    fn local(
        self,
        cell: Atom,
    ) -> Result<EffectDecisionReplacement<'static, Atom>, PreparedEffectInvariant> {
        match self {
            Self::Prepared(fixed) => Ok(fixed.operand(cell)),
            Self::Solved { coefficients, .. } => Ok(EffectDecisionReplacement::Constant(
                coefficients.value(&cell)?,
            )),
        }
    }
}

pub(super) struct Compilation<'a> {
    graph: &'a PreparedEffectDefinitions,
    layout: ClassLayout<'a>,
    owner: FormalOwner,
    parameters: &'a [GenericEffectReference],
    cells: ApplicationCells<'a>,
    residual_rows: Option<&'a BTreeMap<u64, GenericEffectReference>>,
    rows: BTreeMap<u64, Arc<CompiledRow>>,
    predicates: BTreeMap<(u64, PredicateInputs), Arc<Decision>>,
}

impl<'a> Compilation<'a> {
    pub(super) fn new(
        graph: &'a PreparedEffectDefinitions,
        layout: ClassLayout<'a>,
        owner: FormalOwner,
        parameters: &'a [GenericEffectReference],
        cells: ApplicationCells<'a>,
    ) -> Self {
        Self {
            graph,
            layout,
            owner,
            parameters,
            cells,
            residual_rows: None,
            rows: BTreeMap::new(),
            predicates: BTreeMap::new(),
        }
    }

    pub(super) fn with_residual_rows(
        mut self,
        rows: &'a BTreeMap<u64, GenericEffectReference>,
    ) -> Self {
        self.residual_rows = Some(rows);
        self
    }

    pub(super) fn valuation(&self) -> FiniteRowValuation<'_> {
        FiniteRowValuation {
            classes: self.layout.label_classes(),
            owner: self.owner,
            parameters: self.parameters,
        }
    }

    pub(super) fn whole<C: DecisionControl>(
        &self,
        members: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        self.valuation().whole(members, work)
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
                        owner: self.owner,
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

    fn member<'r, C: DecisionControl>(
        &self,
        source: &EffectDecision<GenericEffectReference>,
        class: usize,
        argument: impl Fn(&GenericEffectReference) -> Option<&'r CompiledRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        source.substitute_references(work, |parameter, _| {
            if let Some(argument) = argument(parameter) {
                return Ok(EffectDecisionReplacement::Decision(
                    &argument.membership[class],
                ));
            }
            let slot = self
                .parameters
                .iter()
                .position(|declared| declared == parameter)
                .ok_or_else(|| PreparedEffectInvariant::UndeclaredParameter(parameter.clone()))?;
            Ok(EffectDecisionReplacement::Variable(Atom::Formal {
                owner: self.owner,
                class,
                parameter: slot,
            }))
        })
    }

    pub(super) fn canonical_row<C: DecisionControl>(
        &self,
        row: &EffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<CompiledRow>, PreparedEffectFailure<C::Error>> {
        let view = row.known()?.membership();
        let mut membership = Vec::new();
        for class in 0..self.layout.classes() {
            work.charge(DecisionWork::Node)?;
            membership.push(self.member(self.layout.at(view, class), class, |_| None, work)?);
        }
        work.charge(DecisionWork::Node)?;
        Ok(Arc::new(CompiledRow {
            membership,
            admitted: Decision::constant(true),
        }))
    }

    fn read_row<C: DecisionControl>(
        &self,
        row: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<CompiledRow>, PreparedEffectFailure<C::Error>> {
        match row {
            PreparedEffectRow::Canonical(row) => self.canonical_row(row, work),
            PreparedEffectRow::Term(reference) => {
                let row = self
                    .rows
                    .get(&reference.0.ordinal)
                    .ok_or(PreparedEffectInvariant::StaleReference)?;
                work.charge(DecisionWork::Visit)?;
                Ok(Arc::clone(row))
            }
        }
    }

    pub(super) fn extensional_agreement<C: DecisionControl>(
        &self,
        obligation: &ScopedObligation,
        work: &mut Work<'_, C>,
    ) -> Result<Option<Decision>, PreparedEffectFailure<C::Error>> {
        let PreparedEffectPredicate::Canonical(domain) = &obligation.domain else {
            return Ok(None);
        };
        if !domain.is_unconstrained() {
            return Ok(None);
        }
        let PreparedEffectPredicate::Term(predicate) = &obligation.predicate else {
            return Ok(None);
        };
        let PredicateTerm::Relation {
            relation: PreparedEffectRelation::Equal,
            actual: PreparedEffectRow::Term(actual),
            permitted: PreparedEffectRow::Term(permitted),
        } = self.graph.predicate(predicate)?
        else {
            return Ok(None);
        };
        let RowTerm::Application {
            function: local @ FunctionRef::Unknown(_),
            arguments: actual_arguments,
            ..
        } = self.graph.row(actual)?
        else {
            return Ok(None);
        };
        let RowTerm::Application {
            function: imported @ FunctionRef::Imported(_),
            arguments: permitted_arguments,
            ..
        } = self.graph.row(permitted)?
        else {
            return Ok(None);
        };
        self.graph.function(local)?;
        let imported_definition = self.graph.function(imported)?;
        if imported_definition
            .domain()
            .is_some_and(|domain| !domain.is_unconstrained())
        {
            return Ok(None);
        }
        let parameters = &obligation.parameters;
        for arguments in [actual_arguments, permitted_arguments] {
            if arguments.len() != parameters.len() {
                return Ok(None);
            }
            for (argument, parameter) in arguments.iter().zip(parameters) {
                work.charge(DecisionWork::Visit)?;
                let PreparedEffectRow::Canonical(argument) = argument else {
                    return Ok(None);
                };
                let argument = argument.known()?;
                if argument.membership().overrides().next().is_some()
                    || argument.single_reference() != Some(parameter)
                {
                    return Ok(None);
                }
            }
        }
        let local = local.reference().ordinal;
        let imported = self
            .layout
            .imports
            .functions
            .get(&imported.reference().ordinal)
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        let table = self
            .layout
            .tables
            .get(&local)
            .copied()
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        // The original obligation quantifies the complete argument tuple. Its
        // equality holds exactly when every named and outside coefficient is
        // equal and the imported relation admits its arguments.
        let admitted = self
            .cells
            .complete_import(imported.relation.principal(), work)?;
        let cells = self
            .layout
            .classes()
            .checked_mul(table)
            .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        let equations = (0..cells).map(|position| {
            let class = position / table;
            let mask = position % table;
            Ok([
                self.cells.local(Atom::Cell {
                    scheme: local,
                    class,
                    mask,
                })?,
                EffectDecisionReplacement::Decision(imported.coefficient(class, mask)?),
            ])
        });
        let admission = (!admitted.is_constant(true)).then_some(Ok([
            EffectDecisionReplacement::Constant(true),
            EffectDecisionReplacement::Decision(&admitted),
        ]));
        Decision::all_equalities(admission.into_iter().chain(equations), work).map(Some)
    }

    pub(super) fn application<C: DecisionControl>(
        &self,
        scheme: u64,
        arguments: &[impl Borrow<CompiledRow>],
        work: &mut Work<'_, C>,
    ) -> Result<Vec<Decision>, PreparedEffectFailure<C::Error>> {
        let function = self
            .graph
            .functions
            .get(&scheme)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        if arguments.len() != function.parameters().len() {
            return Err(PreparedEffectInvariant::ApplicationArity.into());
        }
        let basis = self.cells.basis(self.layout);
        let table = basis.function_table(scheme, function)?;
        let mut result = Vec::new();
        for class in 0..self.layout.classes() {
            let basis_class = self
                .layout
                .label_classes()
                .class_in(class, basis.label_classes());
            result.push(
                Decision::select_table(
                    arguments.len(),
                    table.entries,
                    work,
                    |index| {
                        EffectDecisionReplacement::Decision(
                            &arguments[index].borrow().membership[class],
                        )
                    },
                    |mask, work| {
                        Ok(match (self.cells, table.imported) {
                            (ApplicationCells::Prepared(_), Some(imported)) => {
                                EffectDecisionReplacement::Decision(
                                    imported.coefficient(basis_class, mask)?,
                                )
                            }
                            (ApplicationCells::Prepared(fixed), None) => {
                                let cell = table.cell(basis_class, mask);
                                if fixed.reads(&cell) {
                                    work.charge(DecisionWork::Visit)?;
                                }
                                fixed.operand(cell)
                            }
                            (ApplicationCells::Solved { coefficients, .. }, imported) => {
                                if imported.is_none() {
                                    work.charge(DecisionWork::Visit)?;
                                }
                                EffectDecisionReplacement::Constant(table.closed_value(
                                    basis_class,
                                    mask,
                                    coefficients,
                                    work,
                                )?)
                            }
                        })
                    },
                )?
                .ok_or(PreparedEffectInvariant::ApplicationArity)?,
            );
        }
        Ok(result)
    }

    pub(super) fn row<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectRow,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<CompiledRow>, PreparedEffectFailure<C::Error>> {
        match root {
            PreparedEffectRow::Canonical(row) => self.canonical_row(row, work),
            PreparedEffectRow::Term(reference) => {
                self.prepare(terms::Reference::Row(reference), work)?;
                Ok(Arc::clone(&self.rows[&reference.0.ordinal]))
            }
        }
    }

    fn compile_row<C: DecisionControl>(
        &mut self,
        id: u64,
        term: &RowTerm,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let value = match term {
            RowTerm::Application {
                function,
                arguments,
                authority,
            } => {
                let mut compiled = Vec::new();
                let mut admitted = Decision::constant(true);
                let definition = self.graph.function(function)?;
                for argument in arguments {
                    let argument = self.read_row(argument, work)?;
                    admitted = admitted.and(&argument.admitted, work)?;
                    work.charge(DecisionWork::Node)?;
                    compiled.push(argument);
                }
                if let Some(domain) = definition
                    .domain()
                    .filter(|domain| !domain.is_unconstrained())
                {
                    let mut substitutions = BTreeMap::new();
                    for (parameter, argument) in definition.parameters().iter().zip(&compiled) {
                        work.charge(DecisionWork::Node)?;
                        substitutions.insert(parameter, argument);
                    }
                    let mut domains = Vec::new();
                    for class in 0..self.layout.classes() {
                        let domain = self.member(
                            self.layout.at(domain.membership(), class),
                            class,
                            |reference| substitutions.get(reference).map(|row| row.as_ref()),
                            work,
                        )?;
                        work.charge(DecisionWork::Node)?;
                        domains.push(domain);
                    }
                    admitted = admitted.and(&self.whole(&domains, work)?, work)?;
                }
                if let Function::Imported(_) = definition {
                    let imported = self
                        .layout
                        .imports
                        .functions
                        .get(&function.reference().ordinal)
                        .ok_or(PreparedEffectInvariant::UnclosedCell)?;
                    let guard = self
                        .cells
                        .complete_import(imported.relation.principal(), work)?;
                    admitted = admitted.and(&guard, work)?;
                }
                let id = function.reference().ordinal;
                let membership =
                    if let Some(reference) = self.residual_rows.and_then(|rows| rows.get(&id)) {
                        // One residual row abbreviates F at its actual Γ. An
                        // application at any other arguments can observe another
                        // value of F, even if it has the same function identity.
                        if !arguments.is_empty()
                            && *authority != ApplicationArgumentAuthority::Introduction
                        {
                            return Err(PreparedEffectFailure::NonPointwiseObservation);
                        }
                        let parameter = self.parameters.binary_search(reference).map_err(|_| {
                            PreparedEffectInvariant::UndeclaredParameter(reference.clone())
                        })?;
                        (0..self.layout.classes())
                            .map(|class| {
                                work.charge(DecisionWork::Node)?;
                                Decision::variable(
                                    Atom::Formal {
                                        owner: self.owner,
                                        class,
                                        parameter,
                                    },
                                    work,
                                )
                            })
                            .collect::<Result<_, _>>()?
                    } else {
                        self.application(id, &compiled, work)?
                    };
                CompiledRow {
                    membership,
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
                        |reference| substitutions.get(reference).map(Arc::as_ref),
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
        work.charge(DecisionWork::Node)?;
        self.rows.insert(id, Arc::new(value));
        Ok(())
    }

    pub(super) fn canonical_predicate<C: DecisionControl>(
        &self,
        predicate: &EffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if predicate.is_unconstrained() {
            work.charge(DecisionWork::Visit)?;
            return Ok(Decision::constant(true));
        }
        let mut members = Vec::new();
        for class in 0..self.layout.classes() {
            work.charge(DecisionWork::Node)?;
            members.push(self.member(
                self.layout.at(predicate.membership(), class),
                class,
                |_| None,
                work,
            )?);
        }
        self.whole(&members, work)
    }

    fn closed<C: DecisionControl>(
        &self,
        row: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if self.parameters.is_empty() {
            return Ok(Decision::constant(true));
        }
        let mut result = Decision::constant(true);
        for (class, decision) in row.iter().enumerate() {
            let mut parameters = BTreeSet::new();
            for atom in decision.variables() {
                work.charge(DecisionWork::Visit)?;
                if matches!(atom, Atom::Formal { owner, class: member_class, .. }
                    if *owner == self.owner && *member_class == class)
                    && !parameters.contains(atom)
                {
                    work.charge(DecisionWork::Node)?;
                    parameters.insert(atom.clone());
                }
            }
            let independent = decision.independence_region(&parameters, work)?;
            result = result.and(&independent, work)?;
        }
        Ok(self
            .valuation()
            .quantify_inputs(&result, work)?
            .unwrap_or(result))
    }

    fn exact_membership<C: DecisionControl>(
        &self,
        relation: PreparedEffectRelation,
        inputs: PredicateInputs,
        actual: &[Decision],
        permitted: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if actual.len() != self.layout.classes() || permitted.len() != self.layout.classes() {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        if self.parameters.is_empty() && relation == PreparedEffectRelation::Equal {
            // There is no local input pattern to eliminate from the outside
            // point. Compare every class in one controlled conjunction.
            return Decision::all_equalities(
                actual.iter().zip(permitted).map(|(left, right)| {
                    Ok([
                        EffectDecisionReplacement::Decision(left),
                        EffectDecisionReplacement::Decision(right),
                    ])
                }),
                work,
            );
        }
        let ((actual_outside, actual_named), (permitted_outside, permitted_named)) = actual
            .split_first()
            .zip(permitted.split_first())
            .ok_or(PreparedEffectInvariant::UnclosedCell)?;
        let outside =
            self.compare_membership(relation, inputs, 0, actual_outside, permitted_outside, work)?;
        let mut members = Vec::new();
        for (index, (left, right)) in actual_named.iter().zip(permitted_named).enumerate() {
            let test = self.compare_membership(relation, inputs, index + 1, left, right, work)?;
            work.charge(DecisionWork::Node)?;
            members.push(test);
        }
        FiniteRowValuation::join_named(outside, &members, work)
    }

    fn covered<C: DecisionControl>(
        &self,
        actual: &[Decision],
        permitted: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if actual.len() != self.layout.classes() || permitted.len() != self.layout.classes() {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        // Capability admission is selected only where both rows are independent
        // of the formal inputs. Empty input rows are a valid representative of
        // that region, including the outside-support membership point.
        let valuation = self.valuation();
        let outside = valuation.empty_point(0, &actual[0], work)?;
        if outside.is_constant(true) {
            return Ok(Decision::constant(false));
        }
        let mut requirements = Vec::new();
        let denied = Decision::constant(false);
        let classes = self.layout.label_classes();
        for (index, label) in self.layout.labels.iter().enumerate() {
            work.charge(DecisionWork::Visit)?;
            let class = index + 1;
            let required = valuation.empty_point(class, &actual[class], work)?;
            if required.is_constant(false) {
                continue;
            }
            let mut allowance: Option<Cow<'_, Decision>> = None;
            for other in classes.with_path(label) {
                work.charge(DecisionWork::Visit)?;
                let capability = &self.layout.labels[other];
                if !capability.covers(label) || permitted[other + 1].is_constant(false) {
                    continue;
                }
                let grant = valuation.empty_point(other + 1, &permitted[other + 1], work)?;
                if grant.is_constant(false) {
                    continue;
                }
                if grant.is_constant(true) {
                    allowance = Some(grant);
                    break;
                }
                let grant = match allowance {
                    Some(previous) => Cow::Owned(previous.or(&grant, work)?),
                    None => grant,
                };
                let total = grant.is_constant(true);
                allowance = Some(grant);
                if total {
                    break;
                }
            }
            let allowance = allowance.unwrap_or(Cow::Borrowed(&denied));
            if allowance.is_constant(true) {
                continue;
            }
            if allowance.is_constant(false) && required.is_constant(true) {
                return Ok(Decision::constant(false));
            }
            work.charge(DecisionWork::Node)?;
            requirements.push((required, allowance));
        }
        let outside_requirement = EffectDecisionRequirement::Implication {
            premise: outside.as_ref(),
            consequence: &denied,
        };
        EffectDecisionRequirement::all(
            std::iter::once(outside_requirement).chain(requirements.iter().map(
                |(required, allowance)| EffectDecisionRequirement::Implication {
                    premise: required.as_ref(),
                    consequence: allowance.as_ref(),
                },
            )),
            work,
        )
    }

    fn relation<C: DecisionControl>(
        &self,
        relation: PreparedEffectRelation,
        actual: &PreparedEffectRow,
        permitted: &PreparedEffectRow,
        inputs: PredicateInputs,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let actual = self.read_row(actual, work)?;
        let permitted = self.read_row(permitted, work)?;
        let result = relation.decide_membership(
            [&actual.membership, &permitted.membership],
            work,
            |row, work| self.closed(row, work),
            |work| {
                self.exact_membership(
                    relation,
                    inputs,
                    &actual.membership,
                    &permitted.membership,
                    work,
                )
            },
            |work| self.covered(&actual.membership, &permitted.membership, work),
        )?;
        result
            .and(&actual.admitted, work)?
            .and(&permitted.admitted, work)
    }

    fn compare_membership<C: DecisionControl>(
        &self,
        relation: PreparedEffectRelation,
        inputs: PredicateInputs,
        class: usize,
        left: &Decision,
        right: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        let valuation = self.valuation();
        match relation {
            PreparedEffectRelation::Equal => {
                let same = left.equivalent(right, work)?;
                match (inputs, class) {
                    (PredicateInputs::Unrestricted, _) => valuation.all_class_inputs(
                        class,
                        EffectDecisionRequirement::Predicate(&same),
                        work,
                    ),
                    (PredicateInputs::AtValuation, 0) => valuation.outside(&same, work),
                    (PredicateInputs::AtValuation, _) => Ok(same),
                }
            }
            PreparedEffectRelation::AdmittedSubset => {
                let requirement = EffectDecisionRequirement::Implication {
                    premise: left,
                    consequence: right,
                };
                match (inputs, class) {
                    (PredicateInputs::Unrestricted, _) => {
                        valuation.all_class_inputs(class, requirement, work)
                    }
                    (PredicateInputs::AtValuation, 0) => {
                        valuation.outside_requirement(requirement, work)
                    }
                    (PredicateInputs::AtValuation, _) => requirement.decision(work),
                }
            }
        }
    }

    fn read_predicate<C: DecisionControl>(
        &self,
        predicate: &PreparedEffectPredicate,
        inputs: PredicateInputs,
        work: &mut Work<'_, C>,
    ) -> Result<Cow<'_, Decision>, PreparedEffectFailure<C::Error>> {
        match predicate {
            PreparedEffectPredicate::Canonical(predicate) => {
                self.canonical_predicate(predicate, work).map(Cow::Owned)
            }
            PreparedEffectPredicate::Term(reference) => {
                work.charge(DecisionWork::Visit)?;
                self.predicates
                    .get(&(reference.0.ordinal, inputs))
                    .map(|value| Cow::Borrowed(value.as_ref()))
                    .ok_or_else(|| PreparedEffectInvariant::StaleReference.into())
            }
        }
    }

    pub(super) fn predicate<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        self.predicate_with_inputs(root, PredicateInputs::AtValuation, work)
    }

    /// Prequantifies membership clauses on distributive paths. The caller
    /// still quantifies other predicates and checks row admission.
    pub(super) fn predicate_in_unrestricted_scope<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        if self.parameters.is_empty() {
            return self.predicate(root, work);
        }
        self.predicate_with_inputs(root, PredicateInputs::Unrestricted, work)
    }

    fn predicate_with_inputs<C: DecisionControl>(
        &mut self,
        root: &PreparedEffectPredicate,
        inputs: PredicateInputs,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        match root {
            PreparedEffectPredicate::Canonical(value) => {
                let decision = self.canonical_predicate(value, work)?;
                work.share_decision(decision)
            }
            PreparedEffectPredicate::Term(reference) => {
                self.prepare(terms::Reference::Predicate(reference, inputs), work)?;
                Ok(Arc::clone(&self.predicates[&(reference.0.ordinal, inputs)]))
            }
        }
    }

    fn compile_predicate<C: DecisionControl>(
        &mut self,
        id: u64,
        term: &PredicateTerm,
        inputs: PredicateInputs,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        let value = match term {
            PredicateTerm::ImportedPrincipal(imported) => {
                let guard = self.layout.imports.relation(imported, work)?.principal();
                self.cells.complete_import(guard, work)?
            }
            PredicateTerm::Substitution {
                predicate,
                arguments,
            } => {
                let mut substitutions = BTreeMap::new();
                let mut result = Decision::constant(true);
                for (parameter, row) in arguments {
                    let row = self.read_row(row, work)?;
                    result = result.and(&row.admitted, work)?;
                    work.charge(DecisionWork::Node)?;
                    substitutions.insert(parameter.clone(), row);
                }
                let mut members = Vec::new();
                for class in 0..self.layout.classes() {
                    let decision = self.member(
                        self.layout.at(predicate.membership(), class),
                        class,
                        |reference| substitutions.get(reference).map(Arc::as_ref),
                        work,
                    )?;
                    work.charge(DecisionWork::Node)?;
                    members.push(decision);
                }
                result.and(&self.whole(&members, work)?, work)?
            }
            PredicateTerm::Relation {
                relation,
                actual,
                permitted,
            } => self.relation(*relation, actual, permitted, inputs, work)?,
            PredicateTerm::And(children) => work
                .conjoin_prepared_predicates(children, |child, work| {
                    self.read_predicate(child, inputs.children(term), work)
                })?,
            PredicateTerm::Or(children) => {
                let mut result = Decision::constant(false);
                for child in children {
                    let child = self.read_predicate(child, inputs.children(term), work)?;
                    result = result.or(&child, work)?;
                }
                result
            }
            PredicateTerm::Equivalent([left, right]) => {
                let inputs = inputs.children(term);
                let left = self.read_predicate(left, inputs, work)?;
                let right = self.read_predicate(right, inputs, work)?;
                left.equivalent(&right, work)?
            }
            PredicateTerm::Not(child) => self
                .read_predicate(child, inputs.children(term), work)?
                .not(work)?,
            PredicateTerm::Select { choice, yes, no } => {
                self.graph.validate_choice(choice)?;
                let condition = Decision::variable(Atom::Choice(choice.0.ordinal), work)?;
                let yes = self.read_predicate(yes, inputs.children(term), work)?;
                let no = self.read_predicate(no, inputs.children(term), work)?;
                condition.conditional(&yes, &no, work)?
            }
        };
        work.charge(DecisionWork::Node)?;
        self.predicates
            .insert((id, inputs), work.share_decision(value)?);
        Ok(())
    }
}
