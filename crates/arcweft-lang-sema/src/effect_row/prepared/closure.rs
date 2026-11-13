//! Joint closure of issuer-owned, parameterized effect schemes.

use arcweft_core::effect_row::{
    ConditionalDecisionCompletion, EffectDecision, EffectMembershipView,
};

use super::*;
use crate::effects::EffectId;

mod canonical;
mod compilation;
mod component;
use component::CompiledComponent;
mod observation;
mod reification;
pub(crate) use observation::{
    EffectPredicateObservation, EffectPredicateRegion, EffectRowObservation, EffectRowRegion,
    GuardedEffectPredicate, GuardedEffectRow,
};
mod valuation;
use valuation::FiniteRowValuation;

use compilation::Compilation;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Atom {
    Cell {
        scheme: u64,
        class: usize,
        mask: usize,
    },
    Formal {
        owner: FormalOwner,
        class: usize,
        parameter: usize,
    },
    /// Whether an outside-support label has this nonzero input membership
    /// pattern. Pattern zero always occurs because each parameter row is finite.
    Presence {
        owner: FormalOwner,
        mask: usize,
    },
    Choice(u64),
    /// Temporary substitution key, eliminated before any relation is retained.
    Reference(GenericEffectReference),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FormalOwner {
    Scheme(u64),
    Obligation(usize),
    Observation,
}

type Decision = EffectDecision<Atom>;

struct Layout {
    labels: Box<[EffectId]>,
    tables: BTreeMap<u64, usize>,
    cells: BTreeSet<Atom>,
}

/// Compilation reads a class partition, not the solver's cell inventory. An
/// observation can extend label support while retaining the same scheme tables.
#[derive(Clone, Copy)]
struct ClassLayout<'a> {
    labels: &'a [EffectId],
    tables: &'a BTreeMap<u64, usize>,
}

/// Label classes are independent of the solver's scheme-cell tables.
#[derive(Clone, Copy)]
struct LabelClasses<'a> {
    labels: &'a [EffectId],
}

impl Layout {
    fn new<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut labels = BTreeSet::new();
        let mut support = |membership: EffectMembershipView<'_, GenericEffectReference>| -> Result<(), PreparedEffectFailure<C::Error>> {
            for (label, _) in membership.overrides() {
                work.charge(DecisionWork::Visit)?;
                labels.insert(label.clone());
            }
            Ok(())
        };
        for function in graph.functions.values() {
            if let Some(domain) = function.domain() {
                support(domain.membership())?;
            }
        }
        for row in fixed.values() {
            support(row.known()?.membership())?;
        }
        // Include all retained alternatives, not just unconditional constants.
        for term in graph.rows.values() {
            match term.as_ref() {
                RowTerm::Substitution { formula, arguments } => {
                    support(formula.membership())?;
                    for row in arguments.values() {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                RowTerm::Application { arguments, .. } | RowTerm::Union(arguments) => {
                    for row in arguments {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                RowTerm::Select { yes, no, .. } => {
                    for row in [yes, no] {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
            }
        }
        for function in graph.functions.values() {
            let Function::Scheme(scheme) = function.as_ref() else {
                continue;
            };
            if let SchemeState::BodyPrepared {
                body: PreparedEffectRow::Canonical(row),
                ..
            } = &scheme.state
            {
                support(row.known()?.membership())?;
            }
            if let SchemeState::BodyPrepared { obligations, .. } = &scheme.state {
                for predicate in obligations {
                    if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                        support(predicate.membership())?;
                    }
                }
            }
        }
        for obligation in &graph.obligations {
            for predicate in [&obligation.domain, &obligation.predicate] {
                if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                    support(predicate.membership())?;
                }
            }
        }
        for predicate in graph.predicates.values() {
            match predicate.as_ref() {
                PredicateTerm::Substitution {
                    predicate,
                    arguments,
                } => {
                    support(predicate.membership())?;
                    for row in arguments.values() {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                PredicateTerm::Relation {
                    actual, permitted, ..
                } => {
                    for row in [actual, permitted] {
                        if let PreparedEffectRow::Canonical(row) = row {
                            support(row.known()?.membership())?;
                        }
                    }
                }
                PredicateTerm::And(predicates) | PredicateTerm::Or(predicates) => {
                    for predicate in predicates {
                        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                            support(predicate.membership())?;
                        }
                    }
                }
                PredicateTerm::Not(predicate) => {
                    if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                        support(predicate.membership())?;
                    }
                }
                PredicateTerm::Select { yes, no, .. } => {
                    for predicate in [yes, no] {
                        if let PreparedEffectPredicate::Canonical(predicate) = predicate {
                            support(predicate.membership())?;
                        }
                    }
                }
            }
        }
        let classes = labels
            .len()
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        let mut total = 0usize;
        // Admit the entire exponential shape before allocating any cell table.
        for function in graph.functions.values() {
            work.charge(DecisionWork::Visit)?;
            let shift = u32::try_from(function.parameters().len())
                .map_err(|_| PreparedEffectInvariant::TruthTableOverflow)?;
            let table = 1usize
                .checked_shl(shift)
                .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
            total = total
                .checked_add(
                    table
                        .checked_mul(classes)
                        .ok_or(PreparedEffectInvariant::TruthTableOverflow)?,
                )
                .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        }
        for _ in 0..total {
            work.charge(DecisionWork::Node)?;
        }
        let mut tables = BTreeMap::new();
        let mut cells = BTreeSet::new();
        for (&id, function) in &graph.functions {
            let table = 1usize << function.parameters().len();
            tables.insert(id, table);
            for class in 0..classes {
                for mask in 0..table {
                    cells.insert(Atom::Cell {
                        scheme: id,
                        class,
                        mask,
                    });
                }
            }
        }
        Ok(Self {
            labels: labels.into_iter().collect(),
            tables,
            cells,
        })
    }

    fn view(&self) -> ClassLayout<'_> {
        ClassLayout {
            labels: &self.labels,
            tables: &self.tables,
        }
    }
}

impl<'a> ClassLayout<'a> {
    fn label_classes(self) -> LabelClasses<'a> {
        LabelClasses {
            labels: self.labels,
        }
    }

    fn classes(self) -> usize {
        self.label_classes().classes()
    }

    fn at<'v, V: Clone + Ord>(
        self,
        view: EffectMembershipView<'v, V>,
        class: usize,
    ) -> &'v EffectDecision<V> {
        self.label_classes().at(view, class)
    }
}

impl LabelClasses<'_> {
    fn classes(self) -> usize {
        self.labels.len() + 1
    }

    fn at<'a, V: Clone + Ord>(
        self,
        view: EffectMembershipView<'a, V>,
        class: usize,
    ) -> &'a EffectDecision<V> {
        if class == 0 {
            view.default_decision()
        } else {
            view.at(&self.labels[class - 1])
        }
    }
}

/// The least scheme tuple and all remaining discrete witnesses. This is a
/// mathematical result, not a selected-call or semantic-equivalence certificate.
pub(crate) struct PrincipalEffectClosure<'a> {
    graph: &'a PreparedEffectGraph,
    layout: Layout,
    functions: BTreeMap<u64, EffectRow>,
    completion: ConditionalDecisionCompletion<Atom>,
    choices: BTreeSet<Atom>,
    witnesses: Decision,
    future_bindings: BTreeMap<u64, EffectRow>,
}

impl PreparedEffectGraph {
    pub(crate) fn solve_principal<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure<'_>, PreparedEffectFailure<C::Error>> {
        self.solve_with_bindings(BTreeMap::new(), control)
    }

    fn solve_with_bindings<C: DecisionControl>(
        &self,
        fixed: BTreeMap<u64, EffectRow>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure<'_>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        for (&id, function) in &self.functions {
            work.charge(DecisionWork::Visit)?;
            if function.is_future() && !fixed.contains_key(&id) {
                return Err(PreparedEffectInvariant::UnresolvedFuture.into());
            }
        }
        let CompiledComponent { layout, relation } =
            CompiledComponent::new(self, &fixed, &mut work)?;
        let mut choices = BTreeSet::new();
        for &id in &self.choices {
            work.charge(DecisionWork::Visit)?;
            choices.insert(Atom::Choice(id));
        }
        let projected = relation.exists(&choices, &mut work)?;
        let completion = projected.complete_conditionally(&layout.cells, &mut work)?;
        if !completion.satisfiable().is_constant(true) {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        if !completion.principal().is_constant(true) {
            return Err(PreparedEffectFailure::NonPrincipal);
        }
        let choice_relation = relation.substitute(completion.lower_envelope(), &mut work)?;
        for atom in choice_relation.variables() {
            work.charge(DecisionWork::Visit)?;
            if !choices.contains(atom) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        if !choice_relation
            .exists(&choices, &mut work)?
            .is_constant(true)
        {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        let mut functions = BTreeMap::new();
        for (&id, function) in &self.functions {
            let compilation = Compilation::new(
                self,
                layout.view(),
                FormalOwner::Scheme(id),
                function.parameters(),
            );
            let parameters = compilation.parameters(&mut work)?;
            let membership = compilation.application(id, &parameters, &mut work)?;
            let mut output = Vec::new();
            for decision in membership {
                let decision = decision.substitute(completion.lower_envelope(), &mut work)?;
                let mapped = decision.map_references(&mut work, &mut |atom, _| match atom {
                    Atom::Formal {
                        owner: FormalOwner::Scheme(owner),
                        parameter,
                        ..
                    } if *owner == id => Ok(function.parameters()[*parameter].clone()),
                    _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
                })?;
                work.charge(DecisionWork::Node)?;
                output.push(mapped);
            }
            let mut output = output.into_iter();
            let default = output.next().ok_or(PreparedEffectInvariant::UnclosedCell)?;
            let mut overrides = BTreeMap::new();
            for (label, decision) in layout.labels.iter().zip(output) {
                work.charge(DecisionWork::Node)?;
                overrides.insert(label.clone(), decision);
            }
            let formula = EffectFormula::try_from_decisions(default, overrides, &mut work)?;
            functions.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        let closed = PrincipalEffectClosure {
            graph: self,
            layout,
            functions,
            completion,
            choices,
            witnesses: choice_relation,
            future_bindings: fixed,
        };
        // Recheck original typed terms against the canonical scheme projection
        // under every retained witness, without choosing a discrete alternative.
        closed.confirm(&mut work)?;
        Ok(closed)
    }
}
