//! Close the actual dependency component and every original C.

use super::super::read::dependencies::{Dependency, Reachable, SchemeDependencies};
use super::*;

impl PreparedEffectGraph {
    /// A canonical projection is allowed only for this closed dependency
    /// component. Every original graph obligation participates, so selecting
    /// roots cannot hide a rejection or detach a correlated inference unknown.
    pub(crate) fn solve_reachable_principal<C: DecisionControl>(
        &self,
        roots: &[&PreparedEffectSchemeRef],
        control: &mut C,
    ) -> Result<PrincipalEffectClosure<'_>, PreparedEffectFailure<C::Error>> {
        if self.active_scope.is_some() {
            return Err(PreparedEffectInvariant::ReadScopeActive.into());
        }
        let mut reachable = Reachable::new(self.read(), SchemeDependencies::Bodies);
        for root in roots {
            self.scheme(root)?;
            reachable.push(Dependency::Function(self.validate(&root.0)?), control)?;
        }
        for obligation in &self.relation.obligations {
            reachable.push(Dependency::Predicate(&obligation.domain), control)?;
            reachable.push(Dependency::Predicate(&obligation.predicate), control)?;
        }
        let functions = reachable.collect(control)?.functions;
        self.solve_inventory(
            BTreeMap::new(),
            FunctionInventory::Selected(&functions),
            control,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::effect_row::prepared::test_support::{Budget, Exhausted, budget, literal};

    #[test]
    fn reachable_closure_retains_unprepared_headers_and_rejects_projection_outside_its_component() {
        let mut graph = PreparedEffectGraph::default();
        let mut work = budget();
        let unused = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let header = graph.declaration(&unused, &mut work).unwrap();
        let unavailable_row = graph.application(&unused, vec![], &mut work).unwrap();
        let callee = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        let root = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        graph
            .prepare_body(&callee, literal(&["log.write"]), vec![], &mut work)
            .unwrap();
        let application = graph.application(&callee, vec![], &mut work).unwrap();
        graph
            .prepare_body(&root, application, vec![], &mut work)
            .unwrap();
        let state = graph.state;
        let closed = graph
            .solve_reachable_principal(&[&root], &mut work)
            .unwrap();
        let expected =
            EffectRow::closed(crate::effects::EffectSet::from_labels(["log.write"]).unwrap());
        assert_eq!(closed.scheme(&root).unwrap(), &expected);
        assert_eq!(closed.scheme(&callee).unwrap(), &expected);
        assert_eq!(
            closed.scheme(&unused),
            Err(PreparedEffectInvariant::StaleReference)
        );
        assert!(matches!(
            closed.resolve_row(&unavailable_row, &mut work),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::StaleReference
            ))
        ));
        header.validate(&graph).unwrap();
        assert_eq!(graph.state, state);
        assert!(matches!(
            graph.scheme(&unused).unwrap().state,
            SchemeState::Declared
        ));
        assert!(matches!(
            graph.solve_principal(&mut work),
            Err(PreparedEffectFailure::Invariant(
                PreparedEffectInvariant::SchemeState
            ))
        ));
    }

    #[test]
    fn selecting_roots_keeps_every_original_obligation_and_its_inference_dependencies() {
        let mut graph = PreparedEffectGraph::default();
        let mut work = budget();
        let root = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        graph
            .prepare_body(&root, literal(&[]), vec![], &mut work)
            .unwrap();
        let unknown = graph
            .declare_unknown(
                vec![],
                PreparedEffectUnknownEligibility::Bindable,
                &mut work,
            )
            .unwrap();
        let row = graph
            .unknown_application(&unknown, vec![], &mut work)
            .unwrap();
        let predicate = graph
            .relate(
                PreparedEffectRelation::Equal,
                row.clone(),
                literal(&["fs.read"]),
                &mut work,
            )
            .unwrap();
        graph
            .add_scoped_obligation(
                vec![],
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                predicate,
                &mut work,
            )
            .unwrap();
        let closed = graph
            .solve_reachable_principal(&[&root], &mut work)
            .unwrap();
        assert_eq!(
            closed.resolve_row(&row, &mut work).unwrap(),
            EffectRow::closed(crate::effects::EffectSet::from_labels(["fs.read"]).unwrap())
        );
        let impossible = graph
            .relate(
                PreparedEffectRelation::Equal,
                literal(&["fs.read"]),
                literal(&[]),
                &mut work,
            )
            .unwrap();
        graph
            .add_scoped_obligation(
                vec![],
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                impossible,
                &mut work,
            )
            .unwrap();
        assert!(matches!(
            graph.solve_reachable_principal(&[&root], &mut work),
            Err(PreparedEffectFailure::Unsatisfiable)
        ));
    }

    #[test]
    fn dependency_selection_obeys_the_original_control_at_every_budget_boundary() {
        let mut graph = PreparedEffectGraph::default();
        let mut work = budget();
        let root = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut work)
            .unwrap();
        graph
            .prepare_body(&root, literal(&["fs.read"]), vec![], &mut work)
            .unwrap();
        let state = graph.state;
        let mut measured = budget();
        let before = measured.0;
        graph
            .solve_reachable_principal(&[&root], &mut measured)
            .unwrap();
        for remaining in 0..before - measured.0 {
            assert!(matches!(
                graph.solve_reachable_principal(&[&root], &mut Budget(remaining)),
                Err(PreparedEffectFailure::Control(Exhausted))
            ));
            assert_eq!(graph.state, state);
        }
    }
}
