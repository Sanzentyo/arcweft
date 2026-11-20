//! Joint closure of issuer-owned, parameterized effect schemes.

use arcweft_core::effect_row::{DecisionLowerEnvelope, EffectDecision, EffectMembershipView};

use super::*;
use crate::effects::EffectId;

mod canonical;
mod compilation;
mod component;
pub(crate) use component::PreparedEffectActualInputs;
use component::{
    ClosedEffectCoefficients, ClosedEffectWitnesses, CompiledComponent, ComponentCompletion,
};
mod imports;
use imports::ImportedRelations;
mod observation;
mod reification;
pub(crate) use observation::{
    EffectPredicateObservation, EffectPredicateRegion, EffectRowObservation, EffectRowRegion,
    GuardedEffectPredicate, GuardedEffectRow, ResidualEffectClosure,
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

use compilation::{ApplicationCells, Compilation};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Atom {
    // Select table masks by their formal inputs before testing coefficients.
    // Group coefficients that the same (class, mask) comparison reads.
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
    Cell {
        class: usize,
        mask: usize,
        scheme: u64,
    },
    Choice(u64),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum FormalOwner {
    Scheme(u64),
    Obligation(usize),
    Observation,
    Residual,
}

type Decision = EffectDecision<Atom>;

impl PreparedEffectRelation {
    fn decide_membership<C: DecisionControl, Row>(
        self,
        rows: [&Row; 2],
        work: &mut Work<'_, C>,
        closed: impl Fn(&Row, &mut Work<'_, C>) -> Result<Decision, PreparedEffectFailure<C::Error>>,
        exact: impl FnOnce(&mut Work<'_, C>) -> Result<Decision, PreparedEffectFailure<C::Error>>,
        covered: impl FnOnce(&mut Work<'_, C>) -> Result<Decision, PreparedEffectFailure<C::Error>>,
    ) -> Result<Decision, PreparedEffectFailure<C::Error>> {
        if self == Self::Equal {
            return exact(work);
        }
        let [actual, permitted] = rows;
        let actual_closed = closed(actual, work)?;
        if actual_closed.is_constant(false) {
            return exact(work);
        }
        let both_closed = actual_closed.and(&closed(permitted, work)?, work)?;
        if both_closed.is_constant(true) {
            return covered(work);
        }
        if both_closed.is_constant(false) {
            return exact(work);
        }
        // The selector is independent of input rows. A symbolic selector
        // keeps both membership relations under the original quantifier.
        let exact = exact(work)?;
        both_closed.conditional(&covered(work)?, &exact, work)
    }
}

struct Layout {
    labels: Arc<[EffectId]>,
    tables: BTreeMap<u64, usize>,
    cells: BTreeSet<Atom>,
    imports: ImportedRelations,
}

/// Compilation reads a class partition, not the solver's cell inventory. An
/// observation can extend label support while retaining the same scheme tables.
#[derive(Clone, Copy)]
struct ClassLayout<'a> {
    labels: &'a [EffectId],
    tables: &'a BTreeMap<u64, usize>,
    imports: &'a ImportedRelations,
}

/// Label classes are independent of the solver's scheme-cell tables.
#[derive(Clone, Copy)]
struct LabelClasses<'a> {
    labels: &'a [EffectId],
}

impl Layout {
    fn new<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        fixed: &BTreeMap<u64, EffectRow>,
        inventory: FunctionInventory<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let labels = Self::support(graph, fixed, work)?;
        work.charge(DecisionWork::Node)?;
        Self::with_inventory(graph, labels.into_iter().collect(), inventory, work)
    }

    fn support<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
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
                PredicateTerm::ImportedPrincipal(_) => {}
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
        for predicate in graph.predicates.values() {
            if let PredicateTerm::ImportedPrincipal(imported) = predicate.as_ref() {
                for label in &imported.completion().support {
                    work.charge(DecisionWork::Node)?;
                    labels.insert(label.clone());
                }
            }
        }
        for function in graph.functions.values() {
            if let Function::Imported(imported) = function.as_ref() {
                for label in &imported.relation.completion().support {
                    work.charge(DecisionWork::Node)?;
                    labels.insert(label.clone());
                }
            }
        }
        Ok(labels)
    }

    fn with_shared_labels<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        labels: Arc<[EffectId]>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        Self::with_inventory(graph, labels, FunctionInventory::All, work)
    }

    fn with_inventory<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        labels: Arc<[EffectId]>,
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
            labels,
            tables,
            cells,
            imports: ImportedRelations::default(),
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

    fn class_in(self, class: usize, basis: LabelClasses<'_>) -> usize {
        if class == 0 || std::ptr::eq(self.labels, basis.labels) {
            class
        } else {
            basis
                .labels
                .binary_search(&self.labels[class - 1])
                .map_or(0, |index| index + 1)
        }
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
pub(crate) struct PrincipalEffectClosure {
    graph: Arc<PreparedEffectDefinitions>,
    layout: Arc<Layout>,
    functions: BTreeMap<u64, EffectRow>,
    coefficients: ClosedEffectCoefficients,
    witnesses: ClosedEffectWitnesses,
}

pub(crate) enum PreparedEffectCompletion {
    Closed(PrincipalEffectClosure),
    Residual(ResidualEffectClosure),
}

#[derive(Clone, Copy)]
pub(crate) enum PreparedEffectTerms<'a> {
    Closed(&'a PrincipalEffectClosure),
    Residual(&'a ResidualEffectClosure),
}

impl<'a> PreparedEffectTerms<'a> {
    fn definitions(self) -> &'a PreparedEffectDefinitions {
        match self {
            Self::Closed(principal) => &principal.graph,
            Self::Residual(residual) => residual.definitions(),
        }
    }

    pub(crate) fn resolve_row<C: DecisionControl>(
        self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<EffectRow, PreparedEffectFailure<C::Error>> {
        match self {
            Self::Closed(principal) => principal.resolve_row(row, control),
            Self::Residual(residual) => residual.resolve_row(row, control),
        }
    }

    pub(crate) fn resolve_predicate<C: DecisionControl>(
        self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<EffectPredicate, PreparedEffectFailure<C::Error>> {
        match self {
            Self::Closed(principal) => principal.resolve_predicate(predicate, control),
            Self::Residual(residual) => residual.resolve_predicate(predicate, control),
        }
    }
}

/// Reuse resolutions within one ordered projection of an immutable relation.
pub(crate) struct PreparedEffectRowProjection {
    completion: Arc<PreparedEffectCompletion>,
    rows: BTreeMap<u64, EffectRow>,
}

impl PreparedEffectCompletion {
    pub(crate) fn terms(&self) -> PreparedEffectTerms<'_> {
        match self {
            Self::Closed(principal) => PreparedEffectTerms::Closed(principal),
            Self::Residual(residual) => PreparedEffectTerms::Residual(residual),
        }
    }

    pub(crate) fn project_rows(self: &Arc<Self>) -> PreparedEffectRowProjection {
        PreparedEffectRowProjection {
            completion: Arc::clone(self),
            rows: BTreeMap::new(),
        }
    }
}

impl PreparedEffectRowProjection {
    pub(crate) fn resolve<C: DecisionControl>(
        &mut self,
        reference: &PreparedEffectRowRef,
        control: &mut C,
    ) -> Result<EffectRow, PreparedEffectFailure<C::Error>> {
        use std::collections::btree_map::Entry;

        match self.rows.entry(reference.0.ordinal) {
            Entry::Occupied(entry) => {
                let mut work = Work(control);
                work.charge(DecisionWork::Visit)?;
                self.completion.terms().definitions().row(reference)?;
                entry.get().copy_with_control(&mut work)
            }
            Entry::Vacant(entry) => {
                let row = self
                    .completion
                    .terms()
                    .resolve_row(&PreparedEffectRow::Term(reference.clone()), control)?;
                let mut work = Work(control);
                work.charge(DecisionWork::Node)?;
                let retained = row.copy_with_control(&mut work)?;
                entry.insert(retained);
                Ok(row)
            }
        }
    }
}

/// Functions actually used by one query, borrowed from its verified owner.
/// Declaration formals remain inside their functions, never ambient Γ ports.
pub(crate) struct PreparedEffectFunctionUses<'g> {
    graph: &'g PreparedEffectDefinitions,
    functions: BTreeSet<u64>,
}

impl PreparedEffectFunctionUses<'_> {
    pub(crate) fn contains_unknown(
        &self,
        unknown: &PreparedEffectUnknownRef,
    ) -> Result<bool, PreparedEffectInvariant> {
        let reference = FunctionRef::Unknown(unknown.clone());
        self.graph.function(&reference)?;
        Ok(self.functions.contains(&reference.reference().ordinal))
    }
}

impl<'a> PreparedEffectTerms<'a> {
    pub(crate) fn row_function_uses<C: DecisionControl>(
        self,
        row: &PreparedEffectRow,
        control: &mut C,
    ) -> Result<PreparedEffectFunctionUses<'a>, PreparedEffectFailure<C::Error>> {
        use super::read::dependencies::{Dependency, Reachable, SchemeDependencies};
        let graph = self.definitions();
        let mut reachable = Reachable::new(graph.read(), SchemeDependencies::Headers);
        reachable.push(Dependency::Row(row), control)?;
        Ok(PreparedEffectFunctionUses {
            graph,
            functions: reachable.collect(control)?.functions,
        })
    }

    pub(crate) fn predicate_function_uses<C: DecisionControl>(
        self,
        predicate: &PreparedEffectPredicate,
        control: &mut C,
    ) -> Result<PreparedEffectFunctionUses<'a>, PreparedEffectFailure<C::Error>> {
        use super::read::dependencies::{Dependency, Reachable, SchemeDependencies};
        let graph = self.definitions();
        let mut reachable = Reachable::new(graph.read(), SchemeDependencies::Headers);
        reachable.push(Dependency::Predicate(predicate), control)?;
        Ok(PreparedEffectFunctionUses {
            graph,
            functions: reachable.collect(control)?.functions,
        })
    }
}

impl PrincipalEffectClosure {
    /// Replay an additional typed source fact against the same canonical tuple
    /// and every retained witness. A successful result is the proved whole
    /// fact ∀Γ.(D ⇒ R), rather than a guessed pointwise projection of R.
    pub(crate) fn confirm_scoped_constraint<C: DecisionControl>(
        &self,
        constraint: &PreparedEffectScopedConstraint,
        control: &mut C,
    ) -> Result<EffectPredicate, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        // The closure already replayed every retained obligation against all
        // witnesses. The exact same immutable obligation carries that proof.
        if self.graph.retains_constraint(constraint, &mut work)? {
            return Ok(EffectPredicate::unconstrained());
        }
        self.graph
            .read()
            .validate_retained_predicate(&constraint.obligation.domain)?;
        self.graph
            .read()
            .validate_retained_predicate(&constraint.obligation.predicate)?;
        let scope = FiniteRowValuation {
            classes: self.layout.view().label_classes(),
            owner: FormalOwner::Observation,
            parameters: &constraint.obligation.parameters,
        };
        canonical::CanonicalReplay::new(self, scope)
            .confirm_obligation(&constraint.obligation, &mut work)?;
        Ok(EffectPredicate::unconstrained())
    }
}

impl PreparedEffectGraph {
    pub(super) fn label_support<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<EffectId>, PreparedEffectFailure<C::Error>> {
        self.relation.label_support(work)
    }

    pub(crate) fn solve_principal<C: DecisionControl>(
        &self,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.solve_inventory(BTreeMap::new(), FunctionInventory::All, control)
    }

    fn solve_inventory<C: DecisionControl>(
        &self,
        fixed: BTreeMap<u64, EffectRow>,
        inventory: FunctionInventory<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        self.relation.solve_inventory(fixed, inventory, control)
    }
}

impl PreparedEffectDefinitions {
    pub(super) fn label_support<C: DecisionControl>(
        &self,
        work: &mut Work<'_, C>,
    ) -> Result<BTreeSet<EffectId>, PreparedEffectFailure<C::Error>> {
        Layout::support(self, &BTreeMap::new(), work)
    }

    fn solve_with_bindings<C: DecisionControl>(
        &self,
        fixed: BTreeMap<u64, EffectRow>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.solve_inventory(fixed, FunctionInventory::All, control)
    }

    fn solve_inventory<C: DecisionControl>(
        &self,
        fixed: BTreeMap<u64, EffectRow>,
        inventory: FunctionInventory<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        for (&id, function) in &self.functions {
            if !inventory.contains(id) {
                continue;
            }
            work.charge(DecisionWork::Visit)?;
            if matches!(function.as_ref(), Function::SchemeInput(_)) && !fixed.contains_key(&id) {
                return Err(PreparedEffectInvariant::UnresolvedSchemeInput.into());
            }
            if function.is_future() && !fixed.contains_key(&id) {
                return Err(PreparedEffectInvariant::UnresolvedFuture.into());
            }
        }
        let ComponentCompletion {
            layout,
            satisfiable,
            principal,
            coefficients,
            choice_witnesses,
        } = CompiledComponent::with_inventory(self, &fixed, inventory, &mut work)?
            .complete(&mut work)?;
        if !satisfiable.is_constant(true) {
            return Err(PreparedEffectFailure::Unsatisfiable);
        }
        if !principal.is_constant(true) {
            return Err(PreparedEffectFailure::NonPrincipal);
        }
        let witnesses = match choice_witnesses {
            Some(witnesses) => witnesses.close(&mut work)?,
            None => ClosedEffectWitnesses::unique(),
        };
        let closed = PrincipalEffectClosure::reify(
            self.retain(&mut work)?,
            layout,
            coefficients.close(&mut work)?,
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

impl PrincipalEffectClosure {
    fn reify<C: DecisionControl>(
        graph: Arc<PreparedEffectDefinitions>,
        layout: Arc<Layout>,
        coefficients: ClosedEffectCoefficients,
        witnesses: ClosedEffectWitnesses,
        fixed: BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        // Fixed inputs arrived as canonical finite rows. Keep those rows in
        // the result and check their cells against the solved tuple in confirm.
        let mut functions = fixed;
        for id in functions.keys() {
            work.charge(DecisionWork::Visit)?;
            if !layout.tables.contains_key(id)
                || !graph
                    .functions
                    .get(id)
                    .is_some_and(|function| function.is_input())
            {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
        }
        for (&id, function) in &graph.functions {
            if !layout.tables.contains_key(&id) {
                continue;
            }
            if function.is_input() {
                work.charge(DecisionWork::Visit)?;
                if !functions.contains_key(&id) {
                    return Err(if function.is_future() {
                        PreparedEffectInvariant::UnresolvedFuture
                    } else {
                        PreparedEffectInvariant::UnresolvedSchemeInput
                    }
                    .into());
                }
                continue;
            }
            let compilation = Compilation::new(
                &graph,
                layout.view(),
                FormalOwner::Scheme(id),
                function.parameters(),
                ApplicationCells::Solved {
                    basis: layout.view(),
                    coefficients: &coefficients,
                },
            );
            let parameters = compilation.parameters(work)?;
            let mut membership = compilation.application(id, &parameters, work)?.into_iter();
            let mut project = |atom: &Atom, _: &mut Work<'_, C>| match atom {
                Atom::Formal {
                    owner: FormalOwner::Scheme(owner),
                    parameter,
                    ..
                } if *owner == id => Ok(function.parameters()[*parameter].clone()),
                _ => Err(PreparedEffectInvariant::UnclosedCell.into()),
            };
            let default = membership
                .next()
                .ok_or(PreparedEffectInvariant::UnclosedCell)?
                .map_references(work, &mut project)?;
            let mut formula = EffectFormula::builder(default, work)?;
            for (label, decision) in layout.labels.iter().zip(membership) {
                formula.set(label, decision.map_references(work, &mut project)?, work)?;
            }
            work.charge(DecisionWork::Node)?;
            functions.insert(
                id,
                EffectRow {
                    formula: Some(formula.finish()),
                },
            );
        }
        Ok(PrincipalEffectClosure {
            graph,
            layout,
            functions,
            coefficients,
            witnesses,
        })
    }
}
