//! Joint closure with dependency-preserving late row-function bindings.

use super::*;
use crate::effect_row::prepared::completion::residual::{
    EffectResidualProgram, ResidualClauseSource, ResidualNode, ResidualNodeId, ResidualRowScope,
};
use arcweft_core::effect_row::EffectDecisionReplacement;

#[cfg(test)]
mod tests;

pub(super) struct CompiledComponent {
    layout: Layout,
    relation: Arc<Decision>,
    fixed: FixedCells,
    minimized: BTreeSet<Atom>,
    choices: BTreeSet<Atom>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum FiniteCell {
    Local,
    Input,
}

/// Actual input coefficients and the default cells constrained by this
/// component's original finite-function clauses.
pub(super) struct FixedCells {
    actual: BTreeMap<Atom, bool>,
    finite: BTreeMap<u64, FiniteCell>,
}

impl FixedCells {
    pub(super) const EMPTY: Self = Self {
        actual: BTreeMap::new(),
        finite: BTreeMap::new(),
    };

    pub(super) fn new<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        rows: &BTreeMap<u64, EffectRow>,
        layout: ClassLayout<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut fixed = Self::EMPTY;
        for (&id, row) in rows {
            let function = graph
                .functions
                .get(&id)
                .filter(|function| function.is_input())
                .ok_or(PreparedEffectInvariant::FutureBinding)?;
            let parameters = function.parameters();
            let table = *layout
                .tables
                .get(&id)
                .ok_or(PreparedEffectInvariant::FutureBinding)?;
            let membership = row.known()?.membership();
            for class in 0..layout.classes() {
                for mask in 0..table {
                    let value = layout.at(membership, class).try_evaluate_with_control(
                        |reference| {
                            let parameter = parameters
                                .iter()
                                .position(|parameter| parameter == reference)
                                .ok_or_else(|| {
                                    PreparedEffectInvariant::UndeclaredParameter(reference.clone())
                                })?;
                            Ok(mask & (1usize << parameter) != 0)
                        },
                        work,
                    )?;
                    work.charge(DecisionWork::Node)?;
                    fixed.actual.insert(
                        Atom::Cell {
                            scheme: id,
                            class,
                            mask,
                        },
                        value,
                    );
                }
            }
        }
        Ok(fixed)
    }

    pub(super) fn admit_finiteness<C: DecisionControl>(
        mut self,
        graph: &PreparedEffectDefinitions,
        program: &EffectResidualProgram,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        for node in program.relation_nodes() {
            work.charge(DecisionWork::Visit)?;
            if let ResidualNode::FiniteFunction(scheme) = node {
                let role = if graph.functions[scheme].is_input() {
                    FiniteCell::Input
                } else {
                    FiniteCell::Local
                };
                work.charge(DecisionWork::Node)?;
                self.finite.insert(*scheme, role);
            }
        }
        Ok(self)
    }

    const fn default_scheme(cell: &Atom) -> Option<u64> {
        match cell {
            Atom::Cell {
                scheme,
                class: 0,
                mask: 0,
            } => Some(*scheme),
            _ => None,
        }
    }

    fn finite_cell(&self, cell: &Atom) -> Option<FiniteCell> {
        Self::default_scheme(cell).and_then(|id| self.finite.get(&id).copied())
    }

    pub(super) fn reads(&self, cell: &Atom) -> bool {
        !self.actual.is_empty() || (!self.finite.is_empty() && Self::default_scheme(cell).is_some())
    }

    pub(super) fn value(&self, cell: &Atom) -> Option<bool> {
        self.actual
            .get(cell)
            .copied()
            .or_else(|| self.finite_cell(cell).map(|_| false))
    }

    fn determined_value(&self, cell: &Atom) -> Option<bool> {
        self.actual
            .get(cell)
            .copied()
            .or_else(|| (self.finite_cell(cell) == Some(FiniteCell::Local)).then_some(false))
    }

    pub(super) fn binds_coefficient(&self, scheme: u64) -> bool {
        self.value(&Atom::Cell {
            scheme,
            class: 0,
            mask: 0,
        })
        .is_some()
    }

    pub(super) fn operand(&self, cell: Atom) -> EffectDecisionReplacement<'static, Atom> {
        self.value(&cell).map_or(
            EffectDecisionReplacement::Variable(cell),
            EffectDecisionReplacement::Constant,
        )
    }

    pub(super) fn agrees_on<C: DecisionControl>(
        &self,
        receiving: &Self,
        original: &PreparedEffectDefinitions,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        for (cell, value) in &self.actual {
            work.charge(DecisionWork::Visit)?;
            if receiving.actual.get(cell) != Some(value) {
                return Ok(false);
            }
        }
        for (cell, value) in &receiving.actual {
            work.charge(DecisionWork::Visit)?;
            let Atom::Cell { scheme, .. } = cell else {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            };
            if original.functions.contains_key(scheme) && self.actual.get(cell) != Some(value) {
                return Ok(false);
            }
        }
        // An input can become a local scheme in the receiving component.
        // Both compile their original clauses with the same default value.
        for scheme in self.finite.keys() {
            work.charge(DecisionWork::Visit)?;
            if !receiving.finite.contains_key(scheme) {
                return Ok(false);
            }
        }
        for scheme in receiving.finite.keys() {
            work.charge(DecisionWork::Visit)?;
            if original.functions.contains_key(scheme) && !self.finite.contains_key(scheme) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// One completion boundary. Future coefficients remain parameters of both the
/// least tuple and its principal domain; they are never minimized with locals.
/// The domains and coefficients can refer only to unresolved input cells.
pub(super) struct ComponentCompletion {
    pub(super) layout: Arc<Layout>,
    pub(super) satisfiable: Decision,
    pub(super) principal: Decision,
    pub(super) coefficients: EffectCoefficients,
    pub(super) choice_witnesses: Option<EffectWitnesses>,
}

/// A joint relation whose alternatives are quantified once for the whole
/// effect component.
pub(super) struct EffectWitnesses {
    pub(super) choices: BTreeSet<Atom>,
    pub(super) relation: Decision,
}

/// Witnesses that cover a total principal domain after fixing all inputs.
pub(super) struct ClosedEffectWitnesses(EffectWitnesses);

impl ClosedEffectWitnesses {
    pub(super) fn unique() -> Self {
        Self(EffectWitnesses {
            choices: BTreeSet::new(),
            relation: Decision::constant(true),
        })
    }

    pub(super) fn choices(&self) -> &BTreeSet<Atom> {
        &self.0.choices
    }

    pub(super) fn relation(&self) -> &Decision {
        &self.0.relation
    }
}

impl EffectWitnesses {
    pub(super) fn close<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<ClosedEffectWitnesses, PreparedEffectFailure<C::Error>> {
        for atom in self.relation.variables() {
            work.charge(DecisionWork::Visit)?;
            if !self.choices.contains(atom) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        if !self.relation.exists(&self.choices, work)?.is_constant(true) {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        Ok(ClosedEffectWitnesses(self))
    }

    pub(super) fn specialize<C: DecisionControl>(
        &self,
        actuals: &BTreeMap<Atom, bool>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let relation = self.relation.restrict(actuals, work)?;
        let mut choices = BTreeSet::new();
        for choice in &self.choices {
            work.charge(DecisionWork::Node)?;
            choices.insert(choice.clone());
        }
        Ok(Self { choices, relation })
    }
}

/// Fixed inputs and the least tuple have disjoint domains. Any cell outside
/// both domains still requires an actual input or belongs to another relation.
pub(super) struct EffectCoefficients {
    fixed: FixedCells,
    least: DecisionLowerEnvelope<Atom>,
    zero: Decision,
    one: Decision,
}

impl EffectCoefficients {
    pub(super) fn new(fixed: FixedCells, least: DecisionLowerEnvelope<Atom>) -> Self {
        Self {
            fixed,
            least,
            zero: Decision::constant(false),
            one: Decision::constant(true),
        }
    }

    pub(super) fn get(&self, cell: &Atom) -> Option<&Decision> {
        self.fixed
            .determined_value(cell)
            .map(|value| if value { &self.one } else { &self.zero })
            .or_else(|| self.least.get(cell))
    }

    pub(super) fn fixed(&self) -> &FixedCells {
        &self.fixed
    }

    pub(super) fn specialize<C: DecisionControl>(
        &self,
        mut actuals: BTreeMap<Atom, bool>,
        work: &mut Work<'_, C>,
    ) -> Result<ClosedEffectCoefficients, PreparedEffectFailure<C::Error>> {
        let least = self.least.specialize(&actuals, work)?;
        for (cell, &value) in &self.fixed.actual {
            work.charge(DecisionWork::Node)?;
            if let std::collections::btree_map::Entry::Vacant(entry) = actuals.entry(cell.clone()) {
                entry.insert(value);
            } else {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
        }
        let mut finite = BTreeMap::new();
        for (&scheme, &role) in &self.fixed.finite {
            work.charge(DecisionWork::Visit)?;
            if role == FiniteCell::Local {
                work.charge(DecisionWork::Node)?;
                finite.insert(scheme, role);
            }
        }
        Self::new(
            FixedCells {
                actual: actuals,
                finite,
            },
            least,
        )
        .close(work)
    }

    pub(super) fn close<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<ClosedEffectCoefficients, PreparedEffectFailure<C::Error>> {
        if !self.least.is_closed(work)? {
            return Err(PreparedEffectInvariant::UnclosedCell.into());
        }
        Ok(ClosedEffectCoefficients(self))
    }

    #[cfg(test)]
    pub(super) fn inject_coefficient_for_replay_test(&mut self, cell: Atom, value: bool) {
        self.fixed.actual.insert(cell, value);
    }
}

/// A retained tuple whose stored coefficients have no free references.
pub(super) struct ClosedEffectCoefficients(EffectCoefficients);

impl ClosedEffectCoefficients {
    pub(super) fn value(&self, cell: &Atom) -> Result<bool, PreparedEffectInvariant> {
        self.0
            .get(cell)
            .map(|value| value.is_constant(true))
            .ok_or(PreparedEffectInvariant::UnclosedCell)
    }

    pub(super) fn evaluate<C: DecisionControl>(
        &self,
        decision: &Decision,
        work: &mut Work<'_, C>,
    ) -> Result<bool, PreparedEffectFailure<C::Error>> {
        decision.try_evaluate_with_control(|cell| self.value(cell).map_err(Into::into), work)
    }
}

struct RelationScope<'a> {
    compiler: Compilation<'a>,
}

impl RelationScope<'_> {
    fn require<C: DecisionControl>(
        &self,
        domain: &Decision,
        consequence: Arc<Decision>,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        let valuation = self.compiler.valuation();
        if domain.is_constant(true) || consequence.is_constant(true) {
            return match valuation.quantify_inputs(&consequence, work)? {
                Some(result) => work.share_decision(result),
                None => Ok(consequence),
            };
        }
        let guarded = if domain.is_constant(false) {
            Decision::constant(true)
        } else {
            domain.conditional(&consequence, &Decision::constant(true), work)?
        };
        let result = valuation
            .quantify_inputs(&guarded, work)?
            .unwrap_or(guarded);
        work.share_decision(result)
    }

    fn require_scheme<C: DecisionControl>(
        &mut self,
        id: u64,
        domain: &EffectPredicate,
        body: &PreparedEffectRow,
        obligations: &[PreparedEffectPredicate],
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        let body = self.compiler.row(body, work)?;
        let unrestricted = domain.is_unconstrained();
        let equation = if unrestricted {
            self.compiler
                .valuation()
                .scheme_equation(id, &body.membership, fixed, work)?
        } else {
            let parameters = self.compiler.parameters(work)?;
            let own = self.compiler.application(id, &parameters, work)?;
            let mut members = Vec::new();
            for (left, right) in own.iter().zip(&body.membership) {
                work.charge(DecisionWork::Node)?;
                members.push(left.equivalent(right, work)?);
            }
            self.compiler.whole(&members, work)?
        };
        let mut requirements = Vec::new();
        for obligation in obligations {
            let required = if unrestricted {
                self.compiler
                    .predicate_in_unrestricted_scope(obligation, work)?
            } else {
                self.compiler.predicate(obligation, work)?
            };
            work.charge(DecisionWork::Node)?;
            requirements.push(required);
        }
        let needs_admission =
            std::iter::once(&body.admitted).chain(requirements.iter().map(Arc::as_ref));
        if unrestricted {
            let admitted = Decision::all(needs_admission, work)?;
            let admitted = self
                .compiler
                .valuation()
                .quantify_inputs(&admitted, work)?
                .unwrap_or(admitted);
            let relation = if admitted.is_constant(true) {
                equation
            } else {
                equation.and(&admitted, work)?
            };
            return work.share_decision(relation);
        }
        let consequence = Decision::all(std::iter::once(&equation).chain(needs_admission), work)?;
        let consequence = work.share_decision(consequence)?;
        let domain = self.compiler.canonical_predicate(domain, work)?;
        self.require(&domain, consequence, work)
    }

    fn require_obligation<C: DecisionControl>(
        &mut self,
        obligation: &ScopedObligation,
        work: &mut Work<'_, C>,
    ) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
        let domain = self.compiler.predicate(&obligation.domain, work)?;
        let consequence = match self.compiler.extensional_agreement(obligation, work)? {
            Some(agreement) => work.share_decision(agreement)?,
            None if domain.is_constant(true) => self
                .compiler
                .predicate_in_unrestricted_scope(&obligation.predicate, work)?,
            None => self.compiler.predicate(&obligation.predicate, work)?,
        };
        self.require(&domain, consequence, work)
    }
}

#[derive(Eq, Ord, PartialEq, PartialOrd)]
enum RelationScopeKey<'a> {
    Scheme(u64),
    Obligation(&'a [GenericEffectReference]),
}

fn relation_scope<'g, 's, C: DecisionControl>(
    scopes: &'s mut BTreeMap<RelationScopeKey<'g>, RelationScope<'g>>,
    graph: &'g PreparedEffectDefinitions,
    layout: ClassLayout<'g>,
    fixed: &'g FixedCells,
    scope: ResidualRowScope,
    work: &mut Work<'_, C>,
) -> Result<&'s mut RelationScope<'g>, PreparedEffectFailure<C::Error>> {
    let parameters = scope.parameters(graph);
    // Each obligation still quantifies its whole implication separately.
    // Identical ordered parameters can use the same local numbering and term
    // compilation because no formal survives that quantifier.
    let (key, owner) = match scope {
        ResidualRowScope::Scheme(id) => (RelationScopeKey::Scheme(id), FormalOwner::Scheme(id)),
        ResidualRowScope::Obligation(id) => (
            RelationScopeKey::Obligation(parameters),
            FormalOwner::Obligation(id),
        ),
        ResidualRowScope::Output(_) => {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
    };
    match scopes.entry(key) {
        std::collections::btree_map::Entry::Occupied(entry) => Ok(entry.into_mut()),
        std::collections::btree_map::Entry::Vacant(entry) => {
            work.charge(DecisionWork::Node)?;
            Ok(entry.insert(RelationScope {
                compiler: Compilation::new(
                    graph,
                    layout,
                    owner,
                    parameters,
                    ApplicationCells::Prepared(fixed),
                ),
            }))
        }
    }
}

#[derive(Default)]
pub(super) struct CompiledObligations<'a> {
    pub(super) inherited: imports::InheritedObligations<'a>,
    /// Retain the quantified clauses only as long as this import pass can
    /// receive an extension of their original relation.
    pub(super) retain: Option<&'a mut Vec<Arc<Decision>>>,
}

pub(super) struct SupportRestriction<'a> {
    authored: &'a BTreeSet<EffectId>,
    minimized: &'a BTreeSet<Atom>,
}

type SupportEquations = Vec<[EffectDecisionReplacement<'static, Atom>; 2]>;

impl SupportRestriction<'_> {
    /// A class may extend the authored support only if some input coefficient
    /// at that class differs from the same input's outside coefficient.
    fn equations<C: DecisionControl>(
        &self,
        layout: &Layout,
        fixed: &FixedCells,
        work: &mut Work<'_, C>,
    ) -> Result<Vec<(SupportEquations, SupportEquations)>, PreparedEffectFailure<C::Error>> {
        let mut groups = Vec::new();
        for (index, label) in layout.labels.iter().enumerate() {
            if self.authored.contains(label) {
                continue;
            }
            let class = index + 1;
            let first = Atom::Cell {
                class,
                mask: 0,
                scheme: 0,
            };
            let mut inputs = Vec::new();
            let mut outputs = Vec::new();
            for cell in layout.cells.range(first..) {
                work.charge(DecisionWork::Visit)?;
                let Atom::Cell {
                    scheme,
                    class: selected,
                    mask,
                } = cell
                else {
                    return Err(PreparedEffectInvariant::UnclosedCell.into());
                };
                if *selected != class {
                    break;
                }
                let outside = Atom::Cell {
                    scheme: *scheme,
                    class: 0,
                    mask: *mask,
                };
                work.charge(DecisionWork::Node)?;
                let pair = [fixed.operand(cell.clone()), fixed.operand(outside)];
                if self.minimized.contains(cell) {
                    outputs.push(pair);
                } else {
                    inputs.push(pair);
                }
            }
            work.charge(DecisionWork::Node)?;
            groups.push((inputs, outputs));
        }
        Ok(groups)
    }
}

pub(super) fn compile_relation<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    program: &EffectResidualProgram,
    layout: &Layout,
    fixed: &FixedCells,
    mut obligations: CompiledObligations<'_>,
    support: Option<SupportRestriction<'_>>,
    work: &mut Work<'_, C>,
) -> Result<Arc<Decision>, PreparedEffectFailure<C::Error>> {
    let Some((ResidualNode::And(children), clauses)) = program.relation_nodes().split_last() else {
        return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
    };
    let mut scopes = BTreeMap::new();
    let mut values = Vec::<Arc<Decision>>::new();
    let classes = layout.view();
    for node in clauses {
        work.charge(DecisionWork::Visit)?;
        let decision = |id: ResidualNodeId| values[id.index()].as_ref();
        let value = match node {
            ResidualNode::True => work.share_decision(Decision::constant(true))?,
            ResidualNode::And(children) => match children.as_ref() {
                [one] => Arc::clone(&values[one.index()]),
                many => {
                    let result = Decision::all(many.iter().copied().map(decision), work)?;
                    work.share_decision(result)?
                }
            },
            ResidualNode::Not(child) => {
                let result = decision(*child).not(work)?;
                work.share_decision(result)?
            }
            ResidualNode::Guarded(clause) => match clause.source(graph)? {
                ResidualClauseSource::Scheme {
                    id,
                    domain,
                    body,
                    obligations,
                } => relation_scope(&mut scopes, graph, classes, fixed, clause.scope(), work)?
                    .require_scheme(id, domain, body, obligations, fixed, work)?,
                ResidualClauseSource::Obligation { id, obligation } => {
                    let value = if let Some(inherited) = obligations.inherited.get(id) {
                        Arc::clone(inherited)
                    } else if let Some(truth) = obligation.constant_truth() {
                        work.share_decision(Decision::constant(truth))?
                    } else {
                        relation_scope(&mut scopes, graph, classes, fixed, clause.scope(), work)?
                            .require_obligation(obligation, work)?
                    };
                    if let Some(retained) = obligations.retain.as_deref_mut() {
                        debug_assert_eq!(retained.len(), id);
                        work.charge(DecisionWork::Node)?;
                        retained.push(Arc::clone(&value));
                    }
                    value
                }
            },
            ResidualNode::FiniteFunction(scheme) => {
                let cell = Atom::Cell {
                    scheme: *scheme,
                    class: 0,
                    mask: 0,
                };
                let result = match fixed.determined_value(&cell) {
                    Some(value) => Decision::constant(!value),
                    None if fixed.finite_cell(&cell) == Some(FiniteCell::Input) => {
                        // The input is read under this assumption when compiling
                        // the clauses, but the receiving caller must prove it.
                        Decision::variable(cell, work)?.not(work)?
                    }
                    None => return Err(PreparedEffectInvariant::UnclosedCell.into()),
                };
                work.share_decision(result)?
            }
            ResidualNode::ExistsLocalFunctions { .. }
            | ResidualNode::ExistsChoices { .. }
            | ResidualNode::OutputMembership { .. }
            | ResidualNode::SubstituteLower { .. } => {
                return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
            }
        };
        work.charge(DecisionWork::Node)?;
        values.push(value);
    }
    work.charge(DecisionWork::Visit)?;
    let relations = children.iter().map(|id| values[id.index()].as_ref());
    if let Some(support) = support {
        let groups = support.equations(layout, fixed, work)?;
        if !groups.is_empty() {
            let groups = groups
                .into_iter()
                .map(|(inputs, outputs)| (inputs.into_iter().map(Ok), outputs.into_iter().map(Ok)));
            let result = Decision::all_with_guarded_equalities(relations, groups, work)?;
            return work.share_decision(result);
        }
    }
    match children.as_ref() {
        [one] => Ok(Arc::clone(&values[one.index()])),
        _ => {
            let result = Decision::all(relations, work)?;
            work.share_decision(result)
        }
    }
}

impl CompiledComponent {
    #[cfg(test)]
    pub(super) fn new<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        fixed: &BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        Self::with_inventory(graph, fixed, FunctionInventory::All, work)
    }

    pub(super) fn with_inventory<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        fixed: &BTreeMap<u64, EffectRow>,
        inventory: FunctionInventory<'_>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let layout = Layout::new(graph, fixed, inventory, work)?;
        let fixed = FixedCells::new(graph, fixed, layout.view(), work)?;
        imports::compile_with_imports(graph, layout, fixed, inventory, work)
    }

    pub(super) fn compile<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        program: &EffectResidualProgram,
        fixed: FixedCells,
        layout: Layout,
        obligations: CompiledObligations<'_>,
        authored_support: Option<&BTreeSet<EffectId>>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut minimized = BTreeSet::new();
        for cell in &layout.cells {
            let Atom::Cell { scheme, .. } = cell else {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            };
            work.charge(DecisionWork::Visit)?;
            if !graph.functions[scheme].is_input() && fixed.determined_value(cell).is_none() {
                minimized.insert(cell.clone());
            }
        }
        let support = authored_support
            .filter(|_| !minimized.is_empty())
            .map(|authored| SupportRestriction {
                authored,
                minimized: &minimized,
            });
        let relation =
            compile_relation(graph, program, &layout, &fixed, obligations, support, work)?;
        let mut choices = BTreeSet::new();
        for &id in &graph.choices {
            work.charge(DecisionWork::Visit)?;
            choices.insert(Atom::Choice(id));
        }
        Ok(Self {
            layout,
            relation,
            fixed,
            minimized,
            choices,
        })
    }

    pub(super) fn complete<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<ComponentCompletion, PreparedEffectFailure<C::Error>> {
        // Every derived decision uses this same checked source vocabulary.
        // Fixed inputs were compiled as constants. Only unresolved input
        // cells and choices can survive minimization of the local cells.
        let relation = self.relation;
        for atom in relation.variables() {
            work.charge(DecisionWork::Visit)?;
            if self.fixed.determined_value(atom).is_some()
                || (!self.layout.cells.contains(atom) && !self.choices.contains(atom))
            {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        let projected = if self.choices.is_empty() {
            None
        } else {
            Some(relation.exists(&self.choices, work)?)
        };
        let (satisfiable, principal, least) = projected
            .as_ref()
            .unwrap_or(&relation)
            .complete_conditionally(self.minimized, work)?
            .into_parts();
        let choice_witnesses = if self.choices.is_empty() {
            None
        } else {
            let relation = if least.domain().is_empty() || relation.node_count() == 0 {
                match Arc::try_unwrap(relation) {
                    Ok(relation) => relation,
                    Err(relation) => work.copy_decision(&relation)?,
                }
            } else {
                least.instantiate(&relation, work)?
            };
            // Projecting the joint witnesses must give the same principal guard
            // without choosing a different alternative for each label.
            if relation.exists(&self.choices, work)? != principal {
                return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
            }
            Some(EffectWitnesses {
                choices: self.choices,
                relation,
            })
        };
        Ok(ComponentCompletion {
            layout: Arc::new(self.layout),
            satisfiable,
            principal,
            coefficients: EffectCoefficients::new(self.fixed, least),
            choice_witnesses,
        })
    }
}

impl PreparedEffectGraph {
    /// All future functions retain their introduction dependencies and scoped
    /// obligations until their actual rows are supplied. Include those rows'
    /// named support before building any table, then solve the tuple jointly.
    pub(crate) fn specialize_principal<C: DecisionControl>(
        &self,
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        self.relation.specialize_principal(bindings, control)
    }
}

impl PreparedEffectDefinitions {
    pub(in crate::effect_row::prepared) fn specialize_principal<C: DecisionControl>(
        &self,
        bindings: &[(&PreparedEffectUnknownRef, &EffectRow)],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        let inputs = self.read().actual_inputs(bindings, control)?;
        ClosureDefinitions::Borrowed(self).solve_with_actual_inputs(
            &inputs,
            FunctionInventory::All,
            control,
        )
    }

    /// Close one dependency component using the shared invocation's actuals.
    /// Other products can own distinct Future identities on the same graph.
    pub(in crate::effect_row::prepared) fn solve_with_actual_inputs<C: DecisionControl>(
        self,
        inputs: &PreparedEffectActualInputs<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        ClosureDefinitions::Owned(self).solve_with_actual_inputs(
            inputs,
            FunctionInventory::All,
            control,
        )
    }
}

impl ClosureDefinitions<'_> {
    pub(in crate::effect_row::prepared) fn solve_with_actual_inputs<C: DecisionControl>(
        self,
        inputs: &PreparedEffectActualInputs<'_>,
        inventory: FunctionInventory<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        let definitions = self.definitions();
        definitions.read().validate_owner(&inputs.owner)?;
        let mut fixed = BTreeMap::new();
        let mut work = Work(control);
        for (&id, input) in &inputs.rows {
            work.charge(DecisionWork::Visit)?;
            if definitions.functions.contains_key(&id) {
                let formula = work.copy_formula(input.known()?)?;
                work.charge(DecisionWork::Node)?;
                fixed.insert(
                    id,
                    EffectRow {
                        formula: Some(formula),
                    },
                );
            }
        }
        self.solve_inventory(fixed, inventory, control)
    }
}

/// One validated batch of concrete Future functions for the issuing graph.
/// The batch has no access to the graph or permission to specialize a source.
pub(crate) struct PreparedEffectActualInputs<'a> {
    owner: PreparedEffectGraphOwner,
    rows: BTreeMap<u64, &'a EffectRow>,
}

impl PreparedEffectGraph {
    /// Check supplied Future identities against the live graph and each
    /// candidate's own verified scope. The candidate relations stay separate.
    pub(crate) fn actual_inputs_with_scopes<'a, C: DecisionControl>(
        &self,
        bindings: &[(&PreparedEffectUnknownRef, &'a EffectRow)],
        scopes: &[PreparedEffectReadScope],
        control: &mut C,
    ) -> Result<PreparedEffectActualInputs<'a>, PreparedEffectFailure<C::Error>> {
        let mut views = Vec::new();
        let mut work = Work(control);
        for scope in scopes {
            let view = self.read_scoped(scope, work.0)?;
            work.charge(DecisionWork::Node)?;
            views.push(view);
        }
        self.read()
            .actual_inputs_with_views(bindings, &views, work.0)
    }
}

impl<'g> PreparedEffectReadView<'g> {
    pub(crate) fn actual_inputs<'a, C: DecisionControl>(
        self,
        bindings: &[(&PreparedEffectUnknownRef, &'a EffectRow)],
        control: &mut C,
    ) -> Result<PreparedEffectActualInputs<'a>, PreparedEffectFailure<C::Error>> {
        self.actual_inputs_with_views(bindings, &[], control)
    }

    fn actual_inputs_with_views<'a, C: DecisionControl>(
        self,
        bindings: &[(&PreparedEffectUnknownRef, &'a EffectRow)],
        scopes: &[PreparedEffectReadView<'g>],
        control: &mut C,
    ) -> Result<PreparedEffectActualInputs<'a>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut rows = BTreeMap::new();
        for (reference, row) in bindings {
            work.charge(DecisionWork::Visit)?;
            let reference = FunctionRef::Unknown((*reference).clone());
            let mut function = self.function(&reference);
            for scope in scopes {
                if !matches!(function, Err(PreparedEffectInvariant::StaleReference)) {
                    break;
                }
                work.charge(DecisionWork::Visit)?;
                function = scope.function(&reference);
            }
            let function = function?;
            if !function.is_future() {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
            let id = reference.reference().ordinal;
            if rows.contains_key(&id) {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
            let allowed =
                PreparedEffectGraph::validate_parameters(function.parameters(), &mut work)?;
            for parameter in row.known()?.variables() {
                work.charge(DecisionWork::Visit)?;
                if !allowed.contains(parameter) {
                    return Err(
                        PreparedEffectInvariant::UndeclaredParameter(parameter.clone()).into(),
                    );
                }
            }
            work.charge(DecisionWork::Node)?;
            rows.insert(id, *row);
        }
        work.charge(DecisionWork::Node)?;
        Ok(PreparedEffectActualInputs {
            owner: self.graph_owner(),
            rows,
        })
    }
}
