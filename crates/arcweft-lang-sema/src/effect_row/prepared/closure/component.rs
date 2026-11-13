//! Joint closure with dependency-preserving late row-function bindings.

use super::*;

#[cfg(test)]
mod tests;

pub(super) struct CompiledComponent {
    pub(super) layout: Layout,
    pub(super) relation: Decision,
}

impl CompiledComponent {
    pub(super) fn new<C: DecisionControl>(
        graph: &PreparedEffectGraph,
        fixed: &BTreeMap<u64, EffectRow>,
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
        let layout = Layout::new(graph, fixed, work)?;
        let mut relation = Decision::constant(true);
        for (&id, function) in &graph.functions {
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
        Ok(Self { layout, relation })
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
