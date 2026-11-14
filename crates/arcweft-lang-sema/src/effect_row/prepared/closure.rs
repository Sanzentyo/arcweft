//! Joint closure of issuer-owned, parameterized effect schemes.

use arcweft_core::effect_row::{
    ConditionalDecisionCompletion, EffectDecision, EffectMembershipView,
};

use super::*;
use crate::effects::EffectId;

mod canonical;
mod compilation;
mod component;
use component::{CompiledComponent, ComponentCompletion};
mod imports;
use imports::ImportedFunctionTable;
mod observation;
mod reification;
pub(crate) use observation::{
    EffectPredicateObservation, EffectPredicateRegion, EffectRowObservation, EffectRowRegion,
    GuardedEffectPredicate, GuardedEffectRow,
};
mod valuation;
use valuation::FiniteRowValuation;
mod reachable;

#[derive(Clone, Copy)]
pub(in crate::effect_row::prepared) enum FunctionInventory<'a> {
    All,
    Selected(&'a BTreeSet<u64>),
}

impl FunctionInventory<'_> {
    pub(in crate::effect_row::prepared) fn contains(self, id: u64) -> bool {
        match self {
            Self::All => true,
            Self::Selected(ids) => ids.contains(&id),
        }
    }
}

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
    imports: BTreeMap<u64, ImportedFunctionTable>,
}

/// Compilation reads a class partition, not the solver's cell inventory. An
/// observation can extend label support while retaining the same scheme tables.
#[derive(Clone, Copy)]
struct ClassLayout<'a> {
    labels: &'a [EffectId],
    tables: &'a BTreeMap<u64, usize>,
    imports: &'a BTreeMap<u64, ImportedFunctionTable>,
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
        inventory: FunctionInventory<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let labels = Self::support(graph, fixed, work)?;
        Self::with_inventory(graph, labels, inventory, work)
    }

    fn support<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<EffectId>, PreparedEffectFailure<C::Error>> {
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
        for function in graph.functions.values() {
            if let Function::Imported(imported) = function.as_ref() {
                for label in &imported.completion.support {
                    work.charge(DecisionWork::Node)?;
                    labels.insert(label.clone());
                }
            }
        }
        Ok(labels)
    }

    fn with_labels<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        labels: BTreeSet<EffectId>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        Self::with_inventory(graph, labels, FunctionInventory::All, work)
    }

    fn with_inventory<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        labels: BTreeSet<EffectId>,
        inventory: FunctionInventory<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let classes = labels
            .len()
            .checked_add(1)
            .ok_or(PreparedEffectInvariant::TruthTableOverflow)?;
        let mut total = 0usize;
        // Admit the entire exponential shape before allocating any cell table.
        for (_, function) in graph
            .functions
            .iter()
            .filter(|(id, _)| inventory.contains(**id))
        {
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
            if !inventory.contains(id) {
                continue;
            }
            let table = 1usize << function.parameters().len();
            tables.insert(id, table);
            if matches!(function.as_ref(), Function::Imported(_)) {
                continue;
            }
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
            imports: BTreeMap::new(),
        })
    }

    fn view(&self) -> ClassLayout<'_> {
        ClassLayout {
            labels: &self.labels,
            tables: &self.tables,
            imports: &self.imports,
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
    layout: Arc<Layout>,
    functions: BTreeMap<u64, EffectRow>,
    coefficients: BTreeMap<Atom, Decision>,
    choices: BTreeSet<Atom>,
    witnesses: Decision,
    future_bindings: BTreeMap<u64, EffectRow>,
}

impl PreparedEffectGraph {
    pub(super) fn label_support<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<EffectId>, PreparedEffectFailure<C::Error>> {
        Layout::support(self, &BTreeMap::new(), work)
    }

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
        self.solve_inventory(fixed, FunctionInventory::All, control)
    }

    fn solve_inventory<C: DecisionControl>(
        &self,
        fixed: BTreeMap<u64, EffectRow>,
        inventory: FunctionInventory<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure<'_>, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        let mut work = Work(control);
        for (&id, function) in &self.functions {
            if !inventory.contains(id) {
                continue;
            }
            work.charge(DecisionWork::Visit)?;
            if function.is_future() && !fixed.contains_key(&id) {
                return Err(PreparedEffectInvariant::UnresolvedFuture.into());
            }
        }
        let ComponentCompletion {
            layout,
            completion,
            coefficients,
            choices,
            witnesses,
        } = CompiledComponent::with_inventory(self, &fixed, inventory, &mut work)?
            .complete(&mut work)?;
        if !completion.satisfiable().is_constant(true) {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        if !completion.principal().is_constant(true) {
            return Err(PreparedEffectFailure::NonPrincipal);
        }
        for atom in witnesses.variables() {
            work.charge(DecisionWork::Visit)?;
            if !choices.contains(atom) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        if !witnesses.exists(&choices, &mut work)?.is_constant(true) {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        let closed = PrincipalEffectClosure::reify(
            self,
            layout,
            coefficients,
            choices,
            witnesses,
            fixed,
            &mut work,
        )?;
        // Recheck original typed terms against the canonical scheme projection
        // under every retained witness, without choosing a discrete alternative.
        closed.confirm(&mut work)?;
        Ok(closed)
    }
}

impl<'a> PrincipalEffectClosure<'a> {
    fn reify<C: DecisionControl>(
        graph: &'a PreparedEffectGraph,
        layout: Arc<Layout>,
        coefficients: BTreeMap<Atom, Decision>,
        choices: BTreeSet<Atom>,
        witnesses: Decision,
        future_bindings: BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut functions = BTreeMap::new();
        for (&id, function) in &graph.functions {
            if !layout.tables.contains_key(&id) {
                continue;
            }
            let compilation = Compilation::new(
                graph,
                layout.view(),
                FormalOwner::Scheme(id),
                function.parameters(),
            );
            let parameters = compilation.parameters(work)?;
            let membership = compilation.application(id, &parameters, work)?;
            let mut output = Vec::new();
            for decision in membership {
                let decision = decision.substitute(&coefficients, work)?;
                let mapped = decision.map_references(work, &mut |atom, _| match atom {
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
            let formula = EffectFormula::try_from_decisions(default, overrides, work)?;
            functions.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        Ok(PrincipalEffectClosure {
            graph,
            layout,
            functions,
            coefficients,
            choices,
            witnesses,
            future_bindings,
        })
    }
}
