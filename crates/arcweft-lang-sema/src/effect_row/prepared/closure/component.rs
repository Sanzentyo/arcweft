//! Joint closure with dependency-preserving late row-function bindings.

use super::*;

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

impl CompiledComponent {
    pub(super) fn new<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        let mut layout = Layout::new(graph, fixed, work)?;
        layout.imports = imports::lower_imports(graph, &layout.labels, work)?;
        Self::compile(graph, fixed, layout, work)
    }

    pub(super) fn compile<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
        layout: Layout,
        work: &mut Work<'_, C>,
    ) -> Result<Self, PreparedEffectFailure<C::Error>> {
        for function in graph.functions.values() {
            work.charge(DecisionWork::Visit)?;
            if matches!(
                function.as_ref(),
                Function::Scheme(Scheme {
                    state: SchemeState::Declared,
                    ..
                })
            ) {
                return Err(PreparedEffectInvariant::SchemeState.into());
            }
        }
        let mut relation = Decision::constant(true);
        for (&id, function) in &graph.functions {
            if matches!(function.as_ref(), Function::Imported(_)) {
                continue;
            }
            let mut compilation = Compilation::new(
                graph,
                layout.view(),
                FormalOwner::Scheme(id),
                function.parameters(),
            );
            let body = match function.as_ref() {
                Function::Scheme(Scheme {
                    state: SchemeState::BodyPrepared { body, .. },
                    ..
                }) => Some(compilation.row(body, work)?),
                Function::Scheme(_) => return Err(PreparedEffectInvariant::SchemeState.into()),
                Function::Unknown { .. } => fixed
                    .get(&id)
                    .map(|row| compilation.canonical_row(row, work))
                    .transpose()?,
                Function::Imported(_) => unreachable!("derived imports have no cell equation"),
            };
            if let Some(body) = body {
                let parameters = compilation.parameters(work)?;
                let own = compilation.application(id, &parameters, work)?;
                let mut members = Vec::new();
                for (left, right) in own.iter().zip(&body.membership) {
                    work.charge(DecisionWork::Node)?;
                    members.push(left.conditional(right, &right.not(work)?, work)?);
                }
                let mut equation = compilation
                    .whole(&members, work)?
                    .and(&body.admitted, work)?;
                if let Function::Scheme(scheme) = function.as_ref() {
                    let SchemeState::BodyPrepared { obligations, .. } = &scheme.state else {
                        return Err(PreparedEffectInvariant::SchemeState.into());
                    };
                    for predicate in obligations {
                        equation = equation.and(&compilation.predicate(predicate, work)?, work)?;
                    }
                    let domain = compilation.canonical_predicate(&scheme.header.domain, work)?;
                    equation = domain.conditional(&equation, &Decision::constant(true), work)?;
                }
                // Every function keeps its declared dependency signature. A
                // future function is fixed only by its supplied actual row.
                equation = equation.forall(&compilation.formals(work)?, work)?;
                relation = relation.and(&equation, work)?;
            }
            let finite = Decision::variable(
                Atom::Cell {
                    scheme: id,
                    class: 0,
                    mask: 0,
                },
                work,
            )?
            .not(work)?;
            relation = relation.and(&finite, work)?;
        }
        for (id, obligation) in graph.obligations.iter().enumerate() {
            let mut compilation = Compilation::new(
                graph,
                layout.view(),
                FormalOwner::Obligation(id),
                &obligation.parameters,
            );
            let domain = compilation.predicate(&obligation.domain, work)?;
            let predicate = compilation.predicate(&obligation.predicate, work)?;
            let guarded = domain.conditional(&predicate, &Decision::constant(true), work)?;
            let equation = guarded.forall(&compilation.formals(work)?, work)?;
            relation = relation.and(&equation, work)?;
        }
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
