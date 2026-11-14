//! A projection closes its real dependency component and every original C.
//! Other source declarations remain original declarations, never pure bodies.

use super::*;

enum Dependency<'a> {
    Function(u64),
    Row(&'a PreparedEffectRow),
    Predicate(&'a PreparedEffectPredicate),
}

struct Reachable<'a> {
    graph: &'a PreparedEffectGraph,
    functions: BTreeSet<u64>,
    rows: BTreeSet<u64>,
    predicates: BTreeSet<u64>,
    pending: Vec<Dependency<'a>>,
}

impl<'a> Reachable<'a> {
    fn push<C: DecisionControl>(
        &mut self,
        value: Dependency<'a>,
        control: &mut C,
    ) -> Result<(), PreparedEffectFailure<C::Error>> {
        control
            .charge(DecisionWork::Node)
            .map_err(PreparedEffectFailure::Control)?;
        self.pending.push(value);
        Ok(())
    }

    fn collect<C: DecisionControl>(
        mut self,
        control: &mut C,
    ) -> Result<BTreeSet<u64>, PreparedEffectFailure<C::Error>> {
        while let Some(value) = self.pending.pop() {
            control
                .charge(DecisionWork::Visit)
                .map_err(PreparedEffectFailure::Control)?;
            match value {
                Dependency::Function(id) => {
                    if self.functions.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.functions.insert(id);
                    let function = self
                        .graph
                        .functions
                        .get(&id)
                        .ok_or(PreparedEffectInvariant::StaleReference)?;
                    match function.as_ref() {
                        Function::Scheme(scheme) => {
                            let SchemeState::BodyPrepared { body, obligations } = &scheme.state
                            else {
                                return Err(PreparedEffectInvariant::SchemeState.into());
                            };
                            self.push(Dependency::Row(body), control)?;
                            for predicate in obligations {
                                self.push(Dependency::Predicate(predicate), control)?;
                            }
                        }
                        Function::Imported(imported) => {
                            for future in imported.futures.values() {
                                self.push(
                                    Dependency::Function(self.graph.validate(future.reference())?),
                                    control,
                                )?;
                            }
                        }
                        Function::Unknown { .. } => {}
                    }
                }
                Dependency::Row(PreparedEffectRow::Canonical(_))
                | Dependency::Predicate(PreparedEffectPredicate::Canonical(_)) => {}
                Dependency::Row(PreparedEffectRow::Term(reference)) => {
                    let id = self.graph.validate(&reference.0)?;
                    if self.rows.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.rows.insert(id);
                    match self.graph.row(reference)? {
                        RowTerm::Application {
                            function,
                            arguments,
                        } => {
                            self.push(
                                Dependency::Function(self.graph.validate(function.reference())?),
                                control,
                            )?;
                            for row in arguments {
                                self.push(Dependency::Row(row), control)?;
                            }
                        }
                        RowTerm::Substitution { arguments, .. } => {
                            for row in arguments.values() {
                                self.push(Dependency::Row(row), control)?;
                            }
                        }
                        RowTerm::Union(rows) => {
                            for row in rows {
                                self.push(Dependency::Row(row), control)?;
                            }
                        }
                        RowTerm::Select { yes, no, .. } => {
                            self.push(Dependency::Row(yes), control)?;
                            self.push(Dependency::Row(no), control)?;
                        }
                    }
                }
                Dependency::Predicate(PreparedEffectPredicate::Term(reference)) => {
                    let id = self.graph.validate(&reference.0)?;
                    if self.predicates.contains(&id) {
                        continue;
                    }
                    control
                        .charge(DecisionWork::Node)
                        .map_err(PreparedEffectFailure::Control)?;
                    self.predicates.insert(id);
                    match self.graph.predicate(reference)? {
                        PredicateTerm::Substitution { arguments, .. } => {
                            for row in arguments.values() {
                                self.push(Dependency::Row(row), control)?;
                            }
                        }
                        PredicateTerm::Relation {
                            actual, permitted, ..
                        } => {
                            self.push(Dependency::Row(actual), control)?;
                            self.push(Dependency::Row(permitted), control)?;
                        }
                        PredicateTerm::And(values) | PredicateTerm::Or(values) => {
                            for predicate in values {
                                self.push(Dependency::Predicate(predicate), control)?;
                            }
                        }
                        PredicateTerm::Not(value) => {
                            self.push(Dependency::Predicate(value), control)?
                        }
                        PredicateTerm::Select { yes, no, .. } => {
                            self.push(Dependency::Predicate(yes), control)?;
                            self.push(Dependency::Predicate(no), control)?;
                        }
                    }
                }
            }
        }
        Ok(self.functions)
    }
}

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
        let mut reachable = Reachable {
            graph: self,
            functions: BTreeSet::new(),
            rows: BTreeSet::new(),
            predicates: BTreeSet::new(),
            pending: Vec::new(),
        };
        for root in roots {
            self.scheme(root)?;
            reachable.push(Dependency::Function(self.validate(&root.0)?), control)?;
        }
        for obligation in &self.obligations {
            reachable.push(Dependency::Predicate(&obligation.domain), control)?;
            reachable.push(Dependency::Predicate(&obligation.predicate), control)?;
        }
        let functions = reachable.collect(control)?;
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
