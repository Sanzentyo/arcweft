use super::test_support::*;
use super::*;
use crate::{
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

#[test]
fn recursive_identity_retains_a_parametric_default_class() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let scheme = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let recursive = graph
        .application(&scheme, vec![open(&e)], &mut control)
        .unwrap();
    let body = graph
        .union(vec![recursive, open(&e)], &mut control)
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut control)
        .unwrap();
    let call = graph
        .application(&scheme, vec![literal(&["fs.read"])], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.scheme(&scheme).unwrap(),
        &EffectRow::open(EffectSet::new(), e)
    );
    assert_eq!(
        closed.resolve_row(&call, &mut control).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
}

#[test]
fn mutual_schemes_compose_simultaneous_parameter_permutations() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let a = parameter(&scope, 0);
    let b = parameter(&scope, 1);
    let f = graph
        .declare(
            vec![a.clone(), b.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let g = graph
        .declare(
            vec![a.clone(), b.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let g_swapped = graph
        .application(&g, vec![open(&b), open(&a)], &mut control)
        .unwrap();
    let f_body = graph
        .union(vec![g_swapped, open(&a)], &mut control)
        .unwrap();
    let g_body = graph
        .application(&f, vec![open(&b), open(&a)], &mut control)
        .unwrap();
    graph
        .prepare_body(&f, f_body, vec![], &mut control)
        .unwrap();
    graph
        .prepare_body(&g, g_body, vec![], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.scheme(&f).unwrap(),
        &EffectRow::open(EffectSet::new(), a)
    );
    assert_eq!(
        closed.scheme(&g).unwrap(),
        &EffectRow::open(EffectSet::new(), b)
    );
}

#[test]
fn rolled_back_terms_and_schemes_never_alias_fresh_references() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let before = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut control).unwrap();
    let lost = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let term = graph.application(&lost, vec![], &mut control).unwrap();
    graph
        .prepare_body(&before, literal(&["fs.read"]), vec![], &mut control)
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let fresh = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    assert_ne!(lost, fresh);
    assert!(matches!(
        graph.application(&lost, vec![], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert!(matches!(
        graph.union(vec![term], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    graph
        .prepare_body(&before, literal(&[]), vec![], &mut control)
        .unwrap();
    let other = PreparedEffectGraph::default();
    assert!(matches!(
        other.scheme(&before),
        Err(PreparedEffectInvariant::ForeignIssuer)
    ));
}

#[test]
fn checkpoint_stack_rejects_out_of_order_completion_without_restoring_state() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let mut outer = graph.checkpoint(&mut control).unwrap();
    let mut inner = graph.checkpoint(&mut control).unwrap();
    assert_eq!(
        graph.rollback(&mut outer).unwrap_err(),
        PreparedEffectInvariant::CheckpointOrder
    );
    graph.rollback(&mut inner).unwrap();
    graph.commit(&outer).unwrap();
    assert_eq!(
        graph.rollback(&mut outer).unwrap_err(),
        PreparedEffectInvariant::CheckpointOrder
    );
}

#[test]
fn exponential_cell_admission_fails_before_building_a_truth_table() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 25));
    let parameters = (0..25).map(|index| parameter(&scope, index)).collect();
    let scheme = graph
        .declare(parameters, EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    graph
        .prepare_body(&scheme, literal(&[]), vec![], &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut Budget(100)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(graph.rows.len(), 0);
    assert_eq!(graph.functions.len(), 1);
}

#[test]
fn least_cell_closure_distinguishes_unsatisfiable_and_nonprincipal_equations() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let mut control = budget();
    let label =
        EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut control)
            .unwrap();
    let variable = EffectFormula::variable(e.clone(), &mut control).unwrap();
    let complement = label.difference(&variable, &mut control).unwrap();

    let mut impossible = PreparedEffectGraph::default();
    let f = impossible
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let self_call = impossible.application(&f, vec![], &mut control).unwrap();
    let body = impossible
        .apply_formula(
            complement.clone(),
            BTreeMap::from([(e.clone(), self_call)]),
            &mut control,
        )
        .unwrap();
    impossible
        .prepare_body(&f, body, vec![], &mut control)
        .unwrap();
    assert!(matches!(
        impossible.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));

    let mut incomparable = PreparedEffectGraph::default();
    let f = incomparable
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let g = incomparable
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let f_call = incomparable.application(&f, vec![], &mut control).unwrap();
    let g_call = incomparable.application(&g, vec![], &mut control).unwrap();
    let f_body = incomparable
        .apply_formula(
            complement.clone(),
            BTreeMap::from([(e.clone(), g_call)]),
            &mut control,
        )
        .unwrap();
    let g_body = incomparable
        .apply_formula(complement, BTreeMap::from([(e, f_call)]), &mut control)
        .unwrap();
    incomparable
        .prepare_body(&f, f_body, vec![], &mut control)
        .unwrap();
    incomparable
        .prepare_body(&g, g_body, vec![], &mut control)
        .unwrap();
    assert!(matches!(
        incomparable.solve_principal(&mut control),
        Err(PreparedEffectFailure::NonPrincipal)
    ));
}

#[test]
fn one_choice_cannot_select_different_candidates_for_different_labels() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let f = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let body = graph
        .select_row(
            &choice,
            literal(&["fs.read"]),
            literal(&["log.write"]),
            &mut control,
        )
        .unwrap();
    let both = graph
        .relate(
            PreparedEffectRelation::Equal,
            body.clone(),
            literal(&["fs.read", "log.write"]),
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&f, body, vec![both], &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn one_choice_must_work_for_every_formal_valuation() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let f = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let empty = graph
        .relate(
            PreparedEffectRelation::Equal,
            open(&e),
            literal(&[]),
            &mut control,
        )
        .unwrap();
    let has_read = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read"]),
            open(&e),
            &mut control,
        )
        .unwrap();
    let chosen = graph
        .select_predicate(&choice, empty, has_read, &mut control)
        .unwrap();
    graph
        .prepare_body(&f, literal(&[]), vec![chosen], &mut control)
        .unwrap();
    assert!(matches!(
        graph.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn closure_does_not_minimize_discrete_choices_or_issue_an_arbitrary_witness() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let f = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let body = graph
        .select_row(&choice, literal(&[]), literal(&[]), &mut control)
        .unwrap();
    let selected = graph
        .select_predicate(
            &choice,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&f, body.clone(), vec![], &mut control)
        .unwrap();
    let principal = graph.solve_principal(&mut control).unwrap();
    assert!(principal.scheme(&f).unwrap().is_empty());
    assert!(
        principal
            .resolve_row(&body, &mut control)
            .unwrap()
            .is_empty()
    );
    let observation = principal
        .observe_predicate(
            &PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            &selected,
            &mut control,
        )
        .unwrap();
    assert!(observation.region_is_universal(EffectPredicateRegion::Ambiguous));
    assert_eq!(observation.total_truth(), None);
}

#[test]
fn a_forced_choice_is_recovered_after_least_scheme_closure() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let f = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let choice = graph.choice(&mut control).unwrap();
    let body = graph
        .select_row(
            &choice,
            literal(&["fs.read"]),
            literal(&["log.write"]),
            &mut control,
        )
        .unwrap();
    let required = graph
        .relate(
            PreparedEffectRelation::Equal,
            body.clone(),
            literal(&["fs.read"]),
            &mut control,
        )
        .unwrap();
    let selected = graph
        .select_predicate(
            &choice,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&f, body.clone(), vec![required], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed
            .observe_predicate(
                &PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                &selected,
                &mut control
            )
            .unwrap()
            .total_truth(),
        Some(true)
    );
    assert_eq!(
        closed.resolve_row(&body, &mut control).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
}

#[test]
fn closed_capability_coverage_preserves_the_existing_symbolic_boundary() {
    let mut control = budget();
    let mut closed = PreparedEffectGraph::default();
    let f = closed
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let admission = closed
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read"]),
            literal(&["fs.read(save)"]),
            &mut control,
        )
        .unwrap();
    closed
        .prepare_body(&f, literal(&[]), vec![admission], &mut control)
        .unwrap();
    closed.solve_principal(&mut control).unwrap();

    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let mut symbolic = PreparedEffectGraph::default();
    let f = symbolic
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let permitted = symbolic
        .union(vec![literal(&["fs.read(save)"]), open(&e)], &mut control)
        .unwrap();
    let admission = symbolic
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read"]),
            permitted,
            &mut control,
        )
        .unwrap();
    symbolic
        .prepare_body(&f, literal(&[]), vec![admission], &mut control)
        .unwrap();
    assert!(matches!(
        symbolic.solve_principal(&mut control),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn declared_domains_guard_body_equations_and_application_admission() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let formula = EffectFormula::variable(e.clone(), &mut control).unwrap();
    let allowed =
        EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut control)
            .unwrap();
    let domain = formula.subset(&allowed, &mut control).unwrap();
    let f = graph
        .declare(vec![e.clone()], domain, &mut control)
        .unwrap();
    graph
        .prepare_body(&f, open(&e), vec![], &mut control)
        .unwrap();
    let valid = graph
        .application(&f, vec![literal(&["fs.read"])], &mut control)
        .unwrap();
    let invalid = graph
        .application(&f, vec![literal(&["log.write"])], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.resolve_row(&valid, &mut control).unwrap(),
        EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    assert!(matches!(
        closed.resolve_row(&invalid, &mut control),
        Err(PreparedEffectFailure::ObservationNotTotal)
    ));
    assert!(
        closed
            .observe_row(
                &PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                &invalid,
                &mut control
            )
            .unwrap()
            .region_is_universal(EffectRowRegion::Rejected)
    );
}

#[test]
fn mutual_boolean_equations_match_exhaustive_truth_tables() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    for f_rule in 0..4u8 {
        for g_rule in 0..4u8 {
            let mut control = budget();
            let mut graph = PreparedEffectGraph::default();
            let f = graph
                .declare(vec![], EffectPredicate::unconstrained(), &mut control)
                .unwrap();
            let g = graph
                .declare(vec![], EffectPredicate::unconstrained(), &mut control)
                .unwrap();
            let f_call = graph.application(&f, vec![], &mut control).unwrap();
            let g_call = graph.application(&g, vec![], &mut control).unwrap();
            let label = EffectFormula::from_set(
                &EffectSet::from_labels(["fs.read"]).unwrap(),
                &mut control,
            )
            .unwrap();
            let variable = EffectFormula::variable(e.clone(), &mut control).unwrap();
            let mut make_body = |rule, argument| {
                let formula = match rule {
                    0 => EffectFormula::empty(),
                    1 => label.clone(),
                    2 => variable.intersection(&label, &mut control).unwrap(),
                    3 => label.difference(&variable, &mut control).unwrap(),
                    _ => unreachable!(),
                };
                graph
                    .apply_formula(
                        formula,
                        BTreeMap::from([(e.clone(), argument)]),
                        &mut control,
                    )
                    .unwrap()
            };
            let f_body = make_body(f_rule, g_call);
            let g_body = make_body(g_rule, f_call);
            graph
                .prepare_body(&f, f_body, vec![], &mut control)
                .unwrap();
            graph
                .prepare_body(&g, g_body, vec![], &mut control)
                .unwrap();
            let eval = |rule, input| match rule {
                0 => false,
                1 => true,
                2 => input,
                3 => !input,
                _ => unreachable!(),
            };
            let witnesses: Vec<_> = [false, true]
                .into_iter()
                .flat_map(|x| [false, true].into_iter().map(move |y| (x, y)))
                .filter(|(x, y)| *x == eval(f_rule, *y) && *y == eval(g_rule, *x))
                .collect();
            let least = witnesses
                .iter()
                .fold((true, true), |(x, y), (a, b)| (x && *a, y && *b));
            match graph.solve_principal(&mut control) {
                Err(PreparedEffectFailure::Unsatisfiable) => {
                    assert!(witnesses.is_empty(), "{f_rule}/{g_rule}")
                }
                Err(PreparedEffectFailure::NonPrincipal) => assert!(
                    !witnesses.is_empty() && !witnesses.contains(&least),
                    "{f_rule}/{g_rule}"
                ),
                Ok(closed) => {
                    assert!(witnesses.contains(&least), "{f_rule}/{g_rule}");
                    for (scheme, expected) in [(&f, least.0), (&g, least.1)] {
                        assert_eq!(
                            !closed.scheme(scheme).unwrap().is_empty(),
                            expected,
                            "{f_rule}/{g_rule}"
                        );
                    }
                }
                Err(error) => panic!("unexpected {f_rule}/{g_rule}: {error}"),
            }
        }
    }
}

#[test]
fn cell_dependent_closedness_is_decided_before_capability_admission() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let g = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let f = graph
        .declare(
            vec![e.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let self_call = graph.application(&g, vec![open(&e)], &mut control).unwrap();
    // G = G has the finite, closed least solution empty, even though its
    // unfinished application carries all of the caller's formal parameters.
    graph
        .prepare_body(&g, self_call.clone(), vec![], &mut control)
        .unwrap();
    let permitted = graph
        .union(vec![literal(&["fs.read(save)"]), self_call], &mut control)
        .unwrap();
    let admission = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            literal(&["fs.read"]),
            permitted,
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&f, literal(&[]), vec![admission], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert!(closed.scheme(&f).unwrap().is_empty());
    assert!(closed.scheme(&g).unwrap().is_empty());
}

#[test]
fn closure_budget_boundary_preserves_the_original_graph_on_failure() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let f = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    let self_call = graph.application(&f, vec![], &mut control).unwrap();
    let body = graph
        .union(vec![self_call, literal(&["fs.read"])], &mut control)
        .unwrap();
    graph.prepare_body(&f, body, vec![], &mut control).unwrap();
    let before = control.0;
    let expected = graph
        .solve_principal(&mut control)
        .unwrap()
        .scheme(&f)
        .unwrap()
        .clone();
    let cost = before - control.0;
    assert!(cost > 1);
    assert!(matches!(
        graph.solve_principal(&mut Budget(cost - 1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(
        graph
            .solve_principal(&mut Budget(cost))
            .unwrap()
            .scheme(&f)
            .unwrap(),
        &expected
    );
    assert_eq!(graph.functions.len(), 1);
    assert_eq!(graph.rows.len(), 2);
}

#[test]
fn row_substitution_composes_pending_applications_and_existing_formula_bindings() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let a = parameter(&scope, 0);
    let b = parameter(&scope, 1);
    let f = graph
        .declare(
            vec![a.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    let f_body = graph
        .union(vec![open(&a), literal(&["fs.read"])], &mut control)
        .unwrap();
    graph
        .prepare_body(&f, f_body, vec![], &mut control)
        .unwrap();
    let applied = graph.application(&f, vec![open(&b)], &mut control).unwrap();
    let formula = EffectFormula::variable(a.clone(), &mut control)
        .unwrap()
        .difference(
            &EffectFormula::variable(b.clone(), &mut control).unwrap(),
            &mut control,
        )
        .unwrap();
    let inner = graph
        .apply_formula(
            formula,
            BTreeMap::from([(a.clone(), applied)]),
            &mut control,
        )
        .unwrap();
    let source = graph
        .union(vec![inner, literal(&["event.emit"])], &mut control)
        .unwrap();
    let arguments = BTreeMap::from([
        (a, literal(&["metric.write"])),
        (b, literal(&["log.write"])),
    ]);
    let substituted = graph.substitute(&source, &arguments, &mut control).unwrap();
    let result = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    graph
        .prepare_body(&result, substituted, vec![], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.scheme(&result).unwrap().closed_value().unwrap(),
        EffectSet::from_labels(["fs.read", "event.emit"]).unwrap()
    );
}

#[test]
fn row_substitution_is_simultaneous_and_does_not_rewrite_replacement_rows() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let a = parameter(&scope, 0);
    let b = parameter(&scope, 1);
    let source = graph.union(vec![open(&a), open(&b)], &mut control).unwrap();
    let arguments = BTreeMap::from([(a, open(&b)), (b.clone(), literal(&["log.write"]))]);
    let substituted = graph.substitute(&source, &arguments, &mut control).unwrap();
    let result = graph
        .declare(
            vec![b.clone()],
            EffectPredicate::unconstrained(),
            &mut control,
        )
        .unwrap();
    graph
        .prepare_body(&result, substituted, vec![], &mut control)
        .unwrap();
    let closed = graph.solve_principal(&mut control).unwrap();
    assert_eq!(
        closed.scheme(&result).unwrap(),
        &EffectRow::open(EffectSet::from_labels(["log.write"]).unwrap(), b)
    );
}

#[test]
fn shared_deep_row_substitution_is_iterative_and_preserves_dag_sharing() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let a = parameter(&scope, 0);
    let mut source = open(&a);
    for _ in 0..512 {
        source = graph
            .union(vec![source.clone(), source], &mut control)
            .unwrap();
    }
    let before = graph.rows.len();
    let arguments = BTreeMap::from([(a, literal(&["fs.read"]))]);
    let substituted = graph
        .substitute(&source, &arguments, &mut Budget(20_000))
        .unwrap();
    // Growth follows unique terms, not the exponentially many DAG paths.
    assert!(graph.rows.len() - before <= 2 * before + 2);
    let result = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut control)
        .unwrap();
    graph
        .prepare_body(&result, substituted, vec![], &mut control)
        .unwrap();
    assert_eq!(
        graph
            .solve_principal(&mut control)
            .unwrap()
            .scheme(&result)
            .unwrap()
            .closed_value()
            .unwrap(),
        EffectSet::from_labels(["fs.read"]).unwrap()
    );
}

#[test]
fn predicate_substitution_keeps_pending_rows_and_outer_binder_substitution() {
    for (labels, expected) in [(&[][..], true), (&["log.write"][..], false)] {
        let mut graph = PreparedEffectGraph::default();
        let mut control = budget();
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
        let a = parameter(&scope, 0);
        let b = parameter(&scope, 1);
        let f = graph
            .declare(
                vec![a.clone()],
                EffectPredicate::unconstrained(),
                &mut control,
            )
            .unwrap();
        let body = graph
            .union(vec![open(&a), literal(&["fs.read"])], &mut control)
            .unwrap();
        graph.prepare_body(&f, body, vec![], &mut control).unwrap();
        let applied = graph.application(&f, vec![open(&b)], &mut control).unwrap();
        let permitted =
            EffectFormula::from_set(&EffectSet::from_labels(["fs.read"]).unwrap(), &mut control)
                .unwrap();
        let predicate = EffectFormula::variable(a.clone(), &mut control)
            .unwrap()
            .subset(&permitted, &mut control)
            .unwrap();
        let inner = graph
            .substitute_predicate(
                &PreparedEffectPredicate::Canonical(predicate),
                &BTreeMap::from([(a.clone(), applied)]),
                &mut control,
            )
            .unwrap();
        let projected = graph
            .substitute_predicate(
                &inner,
                &BTreeMap::from([(a, literal(&["metric.write"])), (b, literal(labels))]),
                &mut control,
            )
            .unwrap();
        let result = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut control)
            .unwrap();
        graph
            .prepare_body(&result, literal(&[]), vec![projected], &mut control)
            .unwrap();
        match graph.solve_principal(&mut control) {
            Ok(_) => assert!(expected),
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(!expected),
            Err(error) => panic!("unexpected result: {error}"),
        }
    }
}

#[test]
fn failed_substitution_removes_all_partial_rows_and_predicates() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let a = parameter(&scope, 0);
    let row = graph
        .union(vec![open(&a), literal(&["log.write"])], &mut control)
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row,
            literal(&["fs.read", "log.write"]),
            &mut control,
        )
        .unwrap();
    let arguments = BTreeMap::from([(a, literal(&["fs.read"]))]);
    let before = control.0;
    graph
        .substitute_predicate(&predicate, &arguments, &mut control)
        .unwrap();
    let cost = before - control.0;
    let rows: Vec<_> = graph.rows.keys().copied().collect();
    let predicates: Vec<_> = graph.predicates.keys().copied().collect();
    let next_row = graph.next_row;
    let next_predicate = graph.next_predicate;
    assert!(matches!(
        graph.substitute_predicate(&predicate, &arguments, &mut Budget(cost - 1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(graph.rows.keys().copied().collect::<Vec<_>>(), rows);
    assert_eq!(
        graph.predicates.keys().copied().collect::<Vec<_>>(),
        predicates
    );
    assert!(graph.next_row > next_row);
    assert!(graph.next_predicate > next_predicate);
    graph
        .substitute_predicate(&predicate, &arguments, &mut Budget(cost))
        .unwrap();
}

#[test]
fn failed_term_edit_cannot_commit_or_reuse_its_successful_prefix() {
    let mut graph = PreparedEffectGraph::default();
    let mut control = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let a = parameter(&scope, 0);
    let source = open(&a);
    let before = graph.retained_terms();
    let mut edit = graph.edit_terms();
    let output = edit
        .map_row_parameters(&source, &mut control, &mut |_, _| {
            Ok(Some(literal(&["fs.read"])))
        })
        .unwrap();
    assert!(matches!(
        edit.map_row_parameters(&source, &mut Budget(0), &mut |_, _| Ok(None)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(matches!(
        edit.map_row_parameters(&source, &mut control, &mut |_, _| Ok(None)),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::TermEditState
        ))
    ));
    assert_eq!(
        edit.commit().unwrap_err(),
        PreparedEffectInvariant::TermEditState
    );
    assert_eq!(graph.retained_terms(), before);
    assert!(matches!(
        graph.union(vec![output], &mut control),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
}
