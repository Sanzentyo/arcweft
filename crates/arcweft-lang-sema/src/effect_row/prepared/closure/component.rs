//! Joint closure with dependency-preserving late row-function bindings.

use super::*;
use crate::effect_row::prepared::completion::residual::{
    EffectResidualProgram, ResidualNode, ResidualRowScope, ResidualSourcePredicate, source_body,
};

#[cfg(test)]
mod tests;

pub(super) struct CompiledComponent {
    pub(super) layout: Layout,
    pub(super) relation: Decision,
    fixed: BTreeMap<Atom, Decision>,
    minimized: BTreeSet<Atom>,
    choices: BTreeSet<Atom>,
}

/// One completion boundary. Future coefficients remain parameters of both the
/// least tuple and its principal domain; they are never minimized with locals.
pub(super) struct ComponentCompletion {
    pub(super) layout: Arc<Layout>,
    pub(super) completion: ConditionalDecisionCompletion<Atom>,
    pub(super) coefficients: BTreeMap<Atom, Decision>,
    pub(super) choices: BTreeSet<Atom>,
    pub(super) witnesses: Decision,
}

struct RelationScope<'a> {
    compiler: Compilation<'a>,
    body: Option<compilation::CompiledRow>,
}

fn relation_scope<'g, 's, C: DecisionControl>(
    scopes: &'s mut BTreeMap<ResidualRowScope, RelationScope<'g>>,
    graph: &'g PreparedEffectGraph,
    layout: ClassLayout<'g>,
    scope: ResidualRowScope,
    work: &mut Work<'_, C>,
) -> Result<&'s mut RelationScope<'g>, PreparedEffectFailure<C::Error>> {
    if let std::collections::btree_map::Entry::Vacant(entry) = scopes.entry(scope) {
        let owner = match scope {
            ResidualRowScope::Scheme(id) => FormalOwner::Scheme(id),
            ResidualRowScope::Obligation(id) => FormalOwner::Obligation(id),
            ResidualRowScope::Output(_) => {
                return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
            }
        };
        work.charge(DecisionWork::Node)?;
        entry.insert(RelationScope {
            compiler: Compilation::new(graph, layout, owner, scope.parameters(graph)),
            body: None,
        });
    }
    Ok(scopes
        .get_mut(&scope)
        .expect("the actual relation scope was inserted"))
}

fn compile_relation<C: DecisionControl>(
    graph: &PreparedEffectGraph,
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
                let mut result = Decision::constant(true);
                for child in children {
                    result = result.and(&values[child.index()], work)?;
                }
                result
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
                let scoped = relation_scope(&mut scopes, graph, layout, *scope, work)?;
                values[body.index()].forall(&scoped.compiler.formals(work)?, work)?
            }
            ResidualNode::SourcePredicate(source) => {
                let scoped = relation_scope(&mut scopes, graph, layout, source.scope(), work)?;
                match source.value(graph)? {
                    ResidualSourcePredicate::Canonical(value) => {
                        scoped.compiler.canonical_predicate(value, work)?
                    }
                    ResidualSourcePredicate::Prepared(value) => {
                        scoped.compiler.predicate(value, work)?
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
                    compilation::copy(&body.admitted, work)?
                } else {
                    let parameters = scoped.compiler.parameters(work)?;
                    let own = scoped.compiler.application(*scheme, &parameters, work)?;
                    let mut members = Vec::new();
                    for (left, right) in own.iter().zip(&body.membership) {
                        work.charge(DecisionWork::Node)?;
                        members.push(left.conditional(right, &right.not(work)?, work)?);
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
    pub(super) fn new<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        Self::with_inventory(graph, fixed, FunctionInventory::All, work)
    }

    pub(super) fn with_inventory<C: DecisionControl>(
        graph: &PreparedEffectGraph,
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
        graph: &PreparedEffectGraph,
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
                        Decision::constant(value),
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
            if !graph.functions[scheme].is_future() {
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
        let relation = self.relation.substitute(&self.fixed, work)?;
        let projected = relation.exists(&self.choices, work)?;
        let completion = projected.complete_conditionally(&self.minimized, work)?;
        let witnesses = relation.substitute(completion.lower_envelope(), work)?;
        for atom in witnesses.variables() {
            work.charge(DecisionWork::Visit)?;
            let future = self.layout.cells.contains(atom) && !self.minimized.contains(atom);
            if !self.choices.contains(atom) && !future {
                return Err(PreparedEffectInvariant::UnclosedCell.into());
            }
        }
        // The guard and all remaining discrete witnesses describe the same
        // region. A caller may restrict that region, never re-minimize locals.
        if witnesses.exists(&self.choices, work)? != *completion.principal() {
            return Err(PreparedEffectInvariant::PrincipalConfirmation.into());
        }
        let mut coefficients = self.fixed;
        for (cell, decision) in completion.lower_envelope() {
            work.charge(DecisionWork::Node)?;
            coefficients.insert(cell.clone(), compilation::copy(decision, work)?);
        }
        Ok(ComponentCompletion {
            layout: Arc::new(self.layout),
            completion,
            coefficients,
            choices: self.choices,
            witnesses,
        })
    }

    /// A larger receiving basis does not grant extra exceptions to this child.
    /// A new class is available only when an actual Future coefficient differs
    /// from its outside/default coefficient at that class.
    pub(super) fn restrict_support<C: DecisionControl>(
        &mut self,
        graph: &PreparedEffectGraph,
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
                if !function.is_future() {
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
    ) -> Result<PrincipalEffectClosure<'_>, PreparedEffectFailure<C::Error>> {
        let mut work = Work(control);
        let mut fixed = BTreeMap::new();
        for (reference, row) in bindings {
            work.charge(DecisionWork::Visit)?;
            let reference = FunctionRef::Unknown((*reference).clone());
            let function = self.function(&reference)?;
            if !function.is_future() {
                return Err(PreparedEffectInvariant::FutureBinding.into());
            }
            let id = reference.reference().ordinal;
            if fixed.contains_key(&id) {
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
            let formula = row
                .known()?
                .map_references(&mut work, &mut |parameter, _| Ok(parameter.clone()))?;
            work.charge(DecisionWork::Node)?;
            fixed.insert(
                id,
                EffectRow {
                    formula: Some(formula),
                },
            );
        }
        self.solve_with_bindings(fixed, control)
    }
}
