//! Joint closure with dependency-preserving late row-function bindings.

use super::*;
use crate::effect_row::prepared::completion::residual::{
    EffectResidualProgram, ResidualNode, ResidualPredicateSource, ResidualRowScope,
    ResidualSourcePredicate, source_body,
};

#[cfg(test)]
mod tests;

pub(super) struct CompiledComponent {
    pub(super) layout: Layout,
    pub(super) relation: Decision,
    fixed: BTreeMap<Atom, bool>,
    minimized: BTreeSet<Atom>,
    choices: BTreeSet<Atom>,
}

/// One completion boundary. Future coefficients remain parameters of both the
/// least tuple and its principal domain; they are never minimized with locals.
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
    fixed: BTreeMap<Atom, bool>,
    least: DecisionLowerEnvelope<Atom>,
    zero: Decision,
    one: Decision,
}

impl EffectCoefficients {
    fn new(fixed: BTreeMap<Atom, bool>, least: DecisionLowerEnvelope<Atom>) -> Self {
        Self {
            fixed,
            least,
            zero: Decision::constant(false),
            one: Decision::constant(true),
        }
    }

    pub(super) fn get(&self, cell: &Atom) -> Option<&Decision> {
        self.fixed
            .get(cell)
            .map(|&value| if value { &self.one } else { &self.zero })
            .or_else(|| self.least.get(cell))
    }

    pub(super) fn specialize<C: DecisionControl>(
        &self,
        mut actuals: BTreeMap<Atom, bool>,
        work: &mut Work<'_, C>,
    ) -> Result<ClosedEffectCoefficients, PreparedEffectFailure<C::Error>> {
        let least = self.least.specialize(&actuals, work)?;
        for (cell, &value) in &self.fixed {
            work.charge(DecisionWork::Node)?;
            if let std::collections::btree_map::Entry::Vacant(entry) = actuals.entry(cell.clone()) {
                entry.insert(value);
            } else {
                return Err(PreparedEffectInvariant::DuplicateFutureBinding.into());
            }
        }
        Self::new(actuals, least).close(work)
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
        self.fixed.insert(cell, value);
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
    body: Option<Arc<compilation::CompiledRow>>,
    formals: Option<BTreeSet<Atom>>,
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
                    ApplicationCells::Variables,
                ),
                body: None,
                formals: None,
            }))
        }
    }
}

fn compile_relation<C: DecisionControl>(
    graph: &PreparedEffectDefinitions,
    program: &EffectResidualProgram,
    layout: ClassLayout<'_>,
    work: &mut Work<'_, C>,
) -> Result<Decision, PreparedEffectFailure<C::Error>> {
    let mut scopes = BTreeMap::new();
    let mut values: Vec<Decision> = Vec::new();
    for node in program.relation_nodes() {
        work.charge(DecisionWork::Visit)?;
        let value = match node {
            ResidualNode::True => Decision::constant(true),
            ResidualNode::And(children) => {
                Decision::all(children.iter().map(|child| &values[child.index()]), work)?
            }
            ResidualNode::Not(child) => values[child.index()].not(work)?,
            ResidualNode::Implies {
                domain,
                consequence,
            } => values[domain.index()].conditional(
                &values[consequence.index()],
                &Decision::constant(true),
                work,
            )?,
            ResidualNode::ForAllFiniteRows { scope, body } => {
                let body = &values[body.index()];
                if scope.parameters(graph).is_empty() {
                    work.copy_decision(body)?
                } else {
                    let scoped = relation_scope(&mut scopes, graph, layout, *scope, work)?;
                    if scoped.formals.is_none() {
                        scoped.formals = Some(scoped.compiler.formals(work)?);
                    }
                    body.forall(
                        scoped.formals.as_ref().expect("formals were computed"),
                        work,
                    )?
                }
            }
            ResidualNode::SourcePredicate(source) => {
                let scoped = relation_scope(&mut scopes, graph, layout, source.scope(), work)?;
                match source.value(graph)? {
                    ResidualSourcePredicate::Canonical(value) => {
                        scoped.compiler.canonical_predicate(value, work)?
                    }
                    ResidualSourcePredicate::Prepared(value) => {
                        let agreement =
                            match source {
                                ResidualPredicateSource::ObligationConsequence(id) => scoped
                                    .compiler
                                    .extensional_agreement(&graph.obligations[*id], work)?,
                                _ => None,
                            };
                        match agreement {
                            Some(agreement) => agreement,
                            None => scoped.compiler.predicate(value, work)?,
                        }
                    }
                }
            }
            ResidualNode::BodyEquation { scheme } | ResidualNode::BodyAdmitted { scheme } => {
                let scoped = relation_scope(
                    &mut scopes,
                    graph,
                    layout,
                    ResidualRowScope::Scheme(*scheme),
                    work,
                )?;
                if scoped.body.is_none() {
                    scoped.body = Some(scoped.compiler.row(source_body(graph, *scheme)?, work)?);
                }
                let body = scoped
                    .body
                    .as_ref()
                    .expect("the original prepared body was compiled");
                if matches!(node, ResidualNode::BodyAdmitted { .. }) {
                    work.copy_decision(&body.admitted)?
                } else {
                    let parameters = scoped.compiler.parameters(work)?;
                    let own = scoped.compiler.application(*scheme, &parameters, work)?;
                    let mut members = Vec::new();
                    for (left, right) in own.iter().zip(&body.membership) {
                        work.charge(DecisionWork::Node)?;
                        members.push(left.equivalent(right, work)?);
                    }
                    scoped.compiler.whole(&members, work)?
                }
            }
            ResidualNode::FiniteFunction(scheme) => Decision::variable(
                Atom::Cell {
                    scheme: *scheme,
                    class: 0,
                    mask: 0,
                },
                work,
            )?
            .not(work)?,
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
    values
        .pop()
        .ok_or_else(|| PreparedEffectInvariant::PrincipalConfirmation.into())
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
        let mut layout = Layout::new(graph, fixed, inventory, work)?;
        layout.imports = imports::lower_selected_imports(graph, &layout.labels, inventory, work)?;
        let program = EffectResidualProgram::with_inventory(graph, inventory, work)?;
        Self::compile(graph, &program, fixed, layout, work)
    }

    pub(super) fn compile<C: DecisionControl>(
        graph: &PreparedEffectDefinitions,
        program: &EffectResidualProgram,
        fixed: &BTreeMap<u64, EffectRow>,
        layout: Layout,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let relation = compile_relation(graph, program, layout.view(), work)?;
        let mut fixed_cells = BTreeMap::new();
        for (&id, row) in fixed {
            let parameters = graph.functions[&id].parameters();
            for class in 0..layout.view().classes() {
                for mask in 0..layout.tables[&id] {
                    let value = layout
                        .view()
                        .at(row.known()?.membership(), class)
                        .evaluate_with_control(
                            |reference| {
                                let parameter = parameters
                                    .iter()
                                    .position(|parameter| parameter == reference)
                                    .expect("future binding dependencies were validated");
                                mask & (1usize << parameter) != 0
                            },
                            work,
                        )?;
                    work.charge(DecisionWork::Node)?;
                    fixed_cells.insert(
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
        let mut minimized = BTreeSet::new();
        for cell in &layout.cells {
            let Atom::Cell { scheme, .. } = cell else {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            };
            work.charge(DecisionWork::Visit)?;
            if !graph.functions[scheme].is_input() {
                minimized.insert(cell.clone());
            }
        }
        let mut choices = BTreeSet::new();
        for &id in &graph.choices {
            work.charge(DecisionWork::Visit)?;
            choices.insert(Atom::Choice(id));
        }
        Ok(Self {
            layout,
            relation,
            fixed: fixed_cells,
            minimized,
            choices,
        })
    }

    pub(super) fn complete<C: DecisionControl>(
        self,
        work: &mut Work<'_, C>,
    ) -> Result<ComponentCompletion, PreparedEffectFailure<C::Error>> {
        // Substitute actual Future functions before minimization. Unresolved
        // Future cells stay free in the conditional result, including guards.
        // Identity operations can move or borrow the already admitted relation.
        let relation = if self.fixed.is_empty() || self.relation.node_count() == 0 {
            self.relation
        } else {
            self.relation.restrict(&self.fixed, work)?
        };
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
                relation
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
        // With no choices the principal guard already describes exactly where
        // the completed tuple satisfies the original relation.
        let (domain, choices) = match &choice_witnesses {
            Some(witnesses) => (&witnesses.relation, Some(&witnesses.choices)),
            None => (&principal, None),
        };
        for atom in domain.variables() {
            work.charge(DecisionWork::Visit)?;
            let future = self.layout.cells.contains(atom) && !least.domain().contains(atom);
            if !future && !choices.is_some_and(|choices| choices.contains(atom)) {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        Ok(ComponentCompletion {
            layout: Arc::new(self.layout),
            satisfiable,
            principal,
            coefficients: EffectCoefficients::new(self.fixed, least),
            choice_witnesses,
        })
    }

    /// A larger receiving basis does not grant extra exceptions to this child.
    /// A new class is available only when an actual Future coefficient differs
    /// from its outside/default coefficient at that class.
    pub(super) fn restrict_support<C: DecisionControl>(
        &mut self,
        graph: &PreparedEffectDefinitions,
        support: &BTreeSet<EffectId>,
        work: &mut Work<'_, C>,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        for (index, label) in self.layout.labels.iter().enumerate() {
            if support.contains(label) {
                continue;
            }
            let class = index + 1;
            let mut admitted = Decision::constant(false);
            for (&id, function) in &graph.functions {
                if !function.is_input() {
                    continue;
                }
                for mask in 0..self.layout.tables[&id] {
                    let named = Decision::variable(
                        Atom::Cell {
                            scheme: id,
                            class,
                            mask,
                        },
                        work,
                    )?;
                    let default = Decision::variable(
                        Atom::Cell {
                            scheme: id,
                            class: 0,
                            mask,
                        },
                        work,
                    )?;
                    let differs = named.conditional(&default.not(work)?, &default, work)?;
                    admitted = admitted.or(&differs, work)?;
                }
            }
            for cell in &self.minimized {
                let Atom::Cell {
                    scheme,
                    class: selected,
                    mask,
                } = cell
                else {
                    return Err(PreparedEffectInvariant::UnclosedCell.into());
                };
                if *selected != class {
                    continue;
                }
                let named = Decision::variable(cell.clone(), work)?;
                let default = Decision::variable(
                    Atom::Cell {
                        scheme: *scheme,
                        class: 0,
                        mask: *mask,
                    },
                    work,
                )?;
                let same = named.conditional(&default, &default.not(work)?, work)?;
                self.relation = self.relation.and(&admitted.or(&same, work)?, work)?;
            }
        }
        Ok(())
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
        self.solve_with_actual_inputs(&inputs, control)
    }

    /// Close one dependency component using the shared invocation's actuals.
    /// Other products can own distinct Future identities on the same graph.
    pub(in crate::effect_row::prepared) fn solve_with_actual_inputs<C: DecisionControl>(
        &self,
        inputs: &PreparedEffectActualInputs<'_>,
        control: &mut C,
    ) -> Result<PrincipalEffectClosure, PreparedEffectFailure<C::Error>> {
        self.read().validate_owner(&inputs.owner)?;
        let mut fixed = BTreeMap::new();
        let mut work = Work(control);
        for (&id, input) in &inputs.rows {
            work.charge(DecisionWork::Visit)?;
            if self.functions.contains_key(&id) {
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
        self.solve_with_bindings(fixed, control)
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
