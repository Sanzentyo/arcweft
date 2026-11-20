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
            _ => Self::AtValuation,
        }
    }
}

#[derive(Clone, Copy)]
pub(super) enum ApplicationCells<'a> {
    Variables,
    Solved {
        basis: ClassLayout<'a>,
        coefficients: &'a ClosedEffectCoefficients,
    },
}

impl<'a> ApplicationCells<'a> {
    fn basis(self, receiving: ClassLayout<'a>) -> ClassLayout<'a> {
        match self {
            Self::Variables => receiving,
            Self::Solved { basis, .. } => basis,
        }
    }

    fn complete_import<C: DecisionControl>(
        self,
        decision: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        match self {
            Self::Variables => work.copy_decision(decision),
            Self::Solved { coefficients, .. } => {
                Ok(Decision::constant(coefficients.evaluate(decision, work)?))
            }
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

    pub(super) fn formals<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<Atom>, PreparedEffectFailure<C::Error>> {
        self.valuation().formals(work)
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
        } = self.graph.row(actual)?
        else {
            return Ok(None);
        };
        let RowTerm::Application {
            function: imported @ FunctionRef::Imported(_),
            arguments: permitted_arguments,
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
        let mut equations = Vec::new();
        for class in 0..self.layout.classes() {
            for mask in 0..table {
                let left = Decision::variable(
                    Atom::Cell {
                        scheme: local,
                        class,
                        mask,
                    },
                    work,
                )?;
                let right = imported.coefficient(class, mask)?;
                work.charge(DecisionWork::Node)?;
                equations.push(left.equivalent(right, work)?);
            }
        }
        Decision::all(std::iter::once(&admitted).chain(&equations), work).map(Some)
    }

    pub(super) fn application<C: DecisionControl>(
        &self,
        scheme: u64,
        arguments: &[impl Borrow<CompiledRow>],
        work: &mut Work<'_, C>,
    ) -> Result<Vec<Decision>, PreparedEffectFailure<C::Error>> {
        if let Some(reference) = self.residual_rows.and_then(|rows| rows.get(&scheme)) {
            if !arguments.is_empty() {
                return Err(PreparedEffectInvariant::ApplicationArity.into());
            }
            let parameter = self
                .parameters
                .binary_search(reference)
                .map_err(|_| PreparedEffectInvariant::UndeclaredParameter(reference.clone()))?;
            return (0..self.layout.classes())
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
                .collect();
        }
        let table = *self
            .layout
            .tables
            .get(&scheme)
            .ok_or(PreparedEffectInvariant::StaleReference)?;
        let function = self.graph.functions[&scheme].as_ref();
        if arguments.len() != function.parameters().len() {
            return Err(PreparedEffectInvariant::ApplicationArity.into());
        }
        let imported = match function {
            Function::Imported(_) => Some(
                self.layout
                    .imports
                    .functions
                    .get(&scheme)
                    .ok_or(PreparedEffectInvariant::UnclosedCell)?,
            ),
            _ => None,
        };
        let basis = self.cells.basis(self.layout);
        let mut result = Vec::new();
        for class in 0..self.layout.classes() {
            let basis_class = self
                .layout
                .label_classes()
                .class_in(class, basis.label_classes());
            result.push(
                Decision::select_table(
                    arguments.len(),
                    table,
                    work,
                    |index| {
                        EffectDecisionReplacement::Decision(
                            &arguments[index].borrow().membership[class],
                        )
                    },
                    |mask, work| {
                        let cell = Atom::Cell {
                            scheme,
                            class: basis_class,
                            mask,
                        };
                        Ok(match (self.cells, imported) {
                            (ApplicationCells::Variables, Some(imported)) => {
                                EffectDecisionReplacement::Decision(
                                    imported.coefficient(basis_class, mask)?,
                                )
                            }
                            (ApplicationCells::Variables, None) => {
                                EffectDecisionReplacement::Variable(cell)
                            }
                            (ApplicationCells::Solved { coefficients, .. }, Some(imported)) => {
                                let value = imported.coefficient(basis_class, mask)?;
                                EffectDecisionReplacement::Constant(
                                    coefficients.evaluate(value, work)?,
                                )
                            }
                            (ApplicationCells::Solved { coefficients, .. }, None) => {
                                work.charge(DecisionWork::Visit)?;
                                EffectDecisionReplacement::Constant(coefficients.value(&cell)?)
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
                CompiledRow {
                    membership: self.application(function.reference().ordinal, &compiled, work)?,
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
        result.forall(&self.formals(work)?, work)
    }

    fn exact_membership<C: DecisionControl>(
        &self,
        relation: PreparedEffectRelation,
        inputs: PredicateInputs,
        actual: &[Decision],
        permitted: &[Decision],
        work: &mut Work<'_, C>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
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
        let mut covered = self.valuation().outside(&actual[0].not(work)?, work)?;
        for (index, label) in self.layout.labels.iter().enumerate() {
            let mut allowance = Decision::constant(false);
            for (other, capability) in self.layout.labels.iter().enumerate() {
                work.charge(DecisionWork::Visit)?;
                if capability.covers(label) {
                    allowance = allowance.or(&permitted[other + 1], work)?;
                }
            }
            covered = covered.and(
                &actual[index + 1].conditional(&allowance, &Decision::constant(true), work)?,
                work,
            )?;
        }
        Ok(covered)
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
                    (PredicateInputs::Unrestricted, _) => valuation.all_class_masks(
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
                        valuation.all_class_masks(class, requirement, work)
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
            PredicateTerm::And(children) | PredicateTerm::Or(children) => {
                let and = matches!(term, PredicateTerm::And(_));
                let mut result = Decision::constant(and);
                for child in children {
                    let child = self.read_predicate(child, inputs.children(term), work)?;
                    result = if and {
                        result.and(&child, work)?
                    } else {
                        result.or(&child, work)?
                    };
                }
                result
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
