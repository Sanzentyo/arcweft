use super::*;
use crate::effect_row::prepared::test_support::*;
use crate::{
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}
fn unknown(graph: &mut PreparedEffectGraph, future: bool) -> PreparedEffectUnknownRef {
    graph
        .declare_unknown(
            vec![],
            if future {
                PreparedEffectUnknownEligibility::Future
            } else {
                PreparedEffectUnknownEligibility::Bindable
            },
            &mut budget(),
        )
        .unwrap()
}
fn call(
    graph: &mut PreparedEffectGraph,
    reference: &PreparedEffectUnknownRef,
) -> PreparedEffectRow {
    graph
        .unknown_application(reference, vec![], &mut budget())
        .unwrap()
}
fn constrain(
    graph: &mut PreparedEffectGraph,
    relation: PreparedEffectRelation,
    actual: PreparedEffectRow,
    permitted: PreparedEffectRow,
) {
    let predicate = graph
        .relate(relation, actual, permitted, &mut budget())
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), predicate, &mut budget())
        .unwrap();
}
fn row(labels: &[&str]) -> EffectRow {
    EffectRow::closed(EffectSet::from_labels(labels.iter().copied()).unwrap())
}

#[test]
fn nullary_equation_eliminates_locals_before_future_values_exist() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let f = unknown(&mut graph, true);
    let xr = call(&mut graph, &x);
    let fr = call(&mut graph, &f);
    constrain(&mut graph, PreparedEffectRelation::Equal, xr, fr);
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    assert_eq!(reduced.satisfiable().truth_value(), Some(true));
    assert_eq!(reduced.principal().truth_value(), Some(true));
    let output = reduced.unknown(&x).unwrap();
    let membership = output.membership().unwrap();
    let variables: Vec<_> = membership.default_decision().variables().collect();
    assert_eq!(variables.len(), 1);
    assert!(
        output
            .function(variables[0])
            .unwrap()
            .is_unknown(&f)
            .unwrap()
    );
    assert_eq!(output.principal().truth_value(), Some(true));
    assert!(output.residual().lower.is_some());
    let fixed = source
        .specialize_partial(&[(&f, &row(&["signal.write"]))], &mut budget())
        .unwrap();
    let fixed = fixed.reduce(&mut budget()).unwrap();
    assert_eq!(fixed.principal().truth_value(), Some(true));
    assert!(
        fixed
            .unknown(&x)
            .unwrap()
            .membership()
            .unwrap()
            .at(&EffectId::parse("signal.write").unwrap())
            .is_constant(true)
    );
}

#[test]
fn joint_minimum_keeps_the_future_dependent_principal_guard() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let y = unknown(&mut graph, false);
    let f = unknown(&mut graph, true);
    let xr = call(&mut graph, &x);
    let yr = call(&mut graph, &y);
    let fr = call(&mut graph, &f);
    let union = graph.union(vec![xr, yr], &mut budget()).unwrap();
    constrain(&mut graph, PreparedEffectRelation::Equal, union, fr);
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    assert_eq!(reduced.satisfiable().truth_value(), Some(true));
    assert_eq!(reduced.principal().truth_value(), None);
    let ReducedEffectPredicateView::Pointwise(guard) = reduced.principal().view() else {
        panic!("Future emptiness must remain symbolic")
    };
    assert_eq!(guard.variables().count(), 1);
    for reference in [&x, &y] {
        assert!(
            reduced
                .unknown(reference)
                .unwrap()
                .membership()
                .unwrap()
                .default_decision()
                .is_constant(false)
        );
    }
    for (value, expected) in [(row(&[]), true), (row(&["signal.write"]), false)] {
        let fixed = source
            .specialize_partial(&[(&f, &value)], &mut budget())
            .unwrap();
        assert_eq!(
            fixed
                .reduce(&mut budget())
                .unwrap()
                .principal()
                .truth_value(),
            Some(expected)
        );
    }
}

#[test]
fn unique_pointwise_solution_preserves_symbolic_capability_coverage() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let f = unknown(&mut graph, true);
    let g = unknown(&mut graph, true);
    let xr = call(&mut graph, &x);
    let fr = call(&mut graph, &f);
    let gr = call(&mut graph, &g);
    constrain(&mut graph, PreparedEffectRelation::Equal, xr.clone(), fr);
    constrain(&mut graph, PreparedEffectRelation::AdmittedSubset, xr, gr);
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    let ReducedEffectPredicateView::Coverage { actual, permitted } = reduced.principal().view()
    else {
        panic!("coverage must retain its whole-row meaning")
    };
    let av = actual
        .membership()
        .default_decision()
        .variables()
        .next()
        .unwrap();
    let pv = permitted
        .membership()
        .default_decision()
        .variables()
        .next()
        .unwrap();
    assert!(actual.function(av).unwrap().is_unknown(&f).unwrap());
    assert!(permitted.function(pv).unwrap().is_unknown(&g).unwrap());
    let partial = source
        .specialize_partial(&[(&f, &row(&["fs.read(save)"]))], &mut budget())
        .unwrap();
    assert!(matches!(
        partial.reduce(&mut budget()).unwrap().principal().view(),
        ReducedEffectPredicateView::Coverage { .. }
    ));
    for (grant, expected) in [("fs.read", true), ("fs.write", false)] {
        let fixed = partial
            .specialize_partial(&[(&g, &row(&[grant]))], &mut budget())
            .unwrap();
        assert_eq!(
            fixed
                .reduce(&mut budget())
                .unwrap()
                .principal()
                .truth_value(),
            Some(expected)
        );
    }
}

#[test]
fn nonunique_cross_label_constraints_retain_the_quantified_program() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let f = unknown(&mut graph, true);
    let xr = call(&mut graph, &x);
    let fr = call(&mut graph, &f);
    let union = graph
        .union(vec![xr.clone(), fr.clone()], &mut budget())
        .unwrap();
    constrain(&mut graph, PreparedEffectRelation::Equal, union, fr.clone());
    constrain(&mut graph, PreparedEffectRelation::AdmittedSubset, fr, xr);
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    assert_eq!(reduced.principal().truth_value(), None);
    assert!(matches!(
        reduced.principal().view(),
        ReducedEffectPredicateView::Original(_)
    ));
    assert!(reduced.unknown(&x).unwrap().membership().is_none());
    assert!(reduced.unknown(&x).unwrap().residual().lower.is_some());
}

#[test]
fn choices_and_active_lexical_parameters_keep_their_original_quantifiers() {
    let mut graph = PreparedEffectGraph::default();
    let choice = graph.choice(&mut budget()).unwrap();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let body = graph
        .select_row(
            &choice,
            literal(&["log.write"]),
            literal(&["metric.write"]),
            &mut budget(),
        )
        .unwrap();
    graph
        .prepare_body(&scheme, body, vec![], &mut budget())
        .unwrap();
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    assert!(matches!(
        reduced.principal().view(),
        ReducedEffectPredicateView::Original(_)
    ));
    assert!(reduced.scheme(&scheme).unwrap().membership().is_none());

    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let e = parameter(&scope, 0);
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let xr = call(&mut graph, &x);
    let predicate = graph
        .relate(PreparedEffectRelation::Equal, xr, open(&e), &mut budget())
        .unwrap();
    graph
        .add_scoped_obligation(vec![e], truth(), predicate, &mut budget())
        .unwrap();
    let source = graph.complete_scoped(&mut budget()).unwrap();
    assert!(matches!(
        source.reduce(&mut budget()).unwrap().principal().view(),
        ReducedEffectPredicateView::Original(_)
    ));
}

#[test]
fn whole_row_implication_does_not_become_pointwise_implication() {
    // (x is empty) => (x = {a}) admits every nonempty subset of K, whose
    // intersection is empty when K={a,b}; that intersection is not a model.
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let xr = call(&mut graph, &x);
    let domain = graph
        .relate(
            PreparedEffectRelation::Equal,
            xr.clone(),
            literal(&[]),
            &mut budget(),
        )
        .unwrap();
    let consequence = graph
        .relate(
            PreparedEffectRelation::Equal,
            xr,
            literal(&["log.write"]),
            &mut budget(),
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], domain, consequence, &mut budget())
        .unwrap();
    // Retain b in the actual graph's support, without imposing a constraint.
    let _ = graph
        .union(vec![literal(&["metric.write"])], &mut budget())
        .unwrap();
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let reduced = source.reduce(&mut budget()).unwrap();
    assert_eq!(reduced.principal().truth_value(), None);
    assert!(matches!(
        reduced.principal().view(),
        ReducedEffectPredicateView::Original(_)
    ));
}

#[test]
fn reduction_control_and_issuer_failures_do_not_mutate_frozen_sources() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false);
    let f = unknown(&mut graph, true);
    let xr = call(&mut graph, &x);
    let fr = call(&mut graph, &f);
    constrain(&mut graph, PreparedEffectRelation::Equal, xr, fr);
    let source = graph.complete_scoped(&mut budget()).unwrap();
    let graph = Arc::clone(&source.graph);
    let program = Arc::clone(&source.program);
    assert!(matches!(
        source.reduce(&mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert!(Arc::ptr_eq(&graph, &source.graph));
    assert!(Arc::ptr_eq(&program, &source.program));
    assert!(source.fixed.is_empty());
    let reduced = source.reduce(&mut budget()).unwrap();
    let output = reduced.unknown(&x).unwrap();
    let variable = output
        .membership()
        .unwrap()
        .default_decision()
        .variables()
        .next()
        .unwrap();
    let mut foreign = PreparedEffectGraph::default();
    let foreign_f = unknown(&mut foreign, true);
    let foreign = foreign.complete_scoped(&mut budget()).unwrap();
    let foreign = foreign.reduce(&mut budget()).unwrap();
    assert!(matches!(
        foreign.principal().function(variable),
        Err(PreparedEffectInvariant::ForeignIssuer)
    ));
    assert!(matches!(
        reduced.unknown(&foreign_f),
        Err(PreparedEffectInvariant::ForeignIssuer)
    ));
}

#[test]
fn symbolic_guards_and_rows_agree_with_original_replay_for_all_small_inputs() {
    // a is retained K; fresh is deliberately absent from K. Each Future takes
    // every subset of both labels, including simultaneous fresh exceptions.
    let labels = [
        EffectId::parse("log.write").unwrap(),
        EffectId::parse("signal.write").unwrap(),
    ];
    for case in 0..4 {
        let mut graph = PreparedEffectGraph::default();
        let x = unknown(&mut graph, false);
        let y = unknown(&mut graph, false);
        let f = unknown(&mut graph, true);
        let g = unknown(&mut graph, true);
        let xr = call(&mut graph, &x);
        let yr = call(&mut graph, &y);
        let fr = call(&mut graph, &f);
        let gr = call(&mut graph, &g);
        let _ = graph
            .union(vec![literal(&["log.write"])], &mut budget())
            .unwrap();
        match case {
            0 => constrain(&mut graph, PreparedEffectRelation::Equal, xr, fr),
            1 => {
                let union = graph.union(vec![xr, yr], &mut budget()).unwrap();
                constrain(&mut graph, PreparedEffectRelation::Equal, union, fr);
            }
            2 => {
                let union = graph.union(vec![fr, gr], &mut budget()).unwrap();
                constrain(&mut graph, PreparedEffectRelation::Equal, xr, union);
            }
            _ => {
                constrain(&mut graph, PreparedEffectRelation::Equal, xr.clone(), fr);
                constrain(&mut graph, PreparedEffectRelation::Equal, xr, gr);
            }
        }
        let source = graph.complete_scoped(&mut budget()).unwrap();
        let reduced = source.reduce(&mut budget()).unwrap();
        for fm in 0..4 {
            for gm in 0..4 {
                let names = |mask: usize| {
                    let mut result = Vec::new();
                    if mask & 1 != 0 {
                        result.push("log.write");
                    }
                    if mask & 2 != 0 {
                        result.push("signal.write");
                    }
                    row(&result)
                };
                let fv = names(fm);
                let gv = names(gm);
                let values = |variable: &ResidualFunctionVariable, label: usize| {
                    let function = resolve_function(&source, variable).unwrap();
                    let mask = if function.is_unknown(&f).unwrap() {
                        fm
                    } else {
                        assert!(function.is_unknown(&g).unwrap());
                        gm
                    };
                    mask & (1 << label) != 0
                };
                let evaluate = |predicate: ReducedEffectPredicate<'_>| match predicate.view() {
                    ReducedEffectPredicateView::True => true,
                    ReducedEffectPredicateView::False => false,
                    ReducedEffectPredicateView::Pointwise(value) => {
                        assert!(
                            value
                                .membership()
                                .default_decision()
                                .evaluate_with_control(|_| false, &mut budget())
                                .unwrap()
                        );
                        labels.iter().enumerate().all(|(i, label)| {
                            value
                                .membership()
                                .at(label)
                                .evaluate_with_control(|v| values(v, i), &mut budget())
                                .unwrap()
                        })
                    }
                    _ => panic!("pointwise graphs must eliminate all local quantifiers"),
                };
                let sat = evaluate(reduced.satisfiable());
                let principal = evaluate(reduced.principal());
                let closure = source
                    .graph
                    .specialize_principal(&[(&f, &fv), (&g, &gv)], &mut budget());
                match closure {
                    Ok(closure) => {
                        assert!(sat && principal, "case {case}, inputs {fm}/{gm}");
                        for reference in [&x, &y] {
                            let output = reduced.unknown(reference).unwrap();
                            let membership = output.membership().unwrap();
                            let original = closure.unknown(reference).unwrap();
                            for (i, label) in labels.iter().enumerate() {
                                let actual = membership
                                    .at(label)
                                    .evaluate_with_control(|v| values(v, i), &mut budget())
                                    .unwrap();
                                assert_eq!(
                                    actual,
                                    original
                                        .known()
                                        .unwrap()
                                        .membership()
                                        .at(label)
                                        .is_constant(true),
                                    "case {case}, inputs {fm}/{gm}"
                                );
                            }
                        }
                    }
                    Err(PreparedEffectFailure::NonPrincipal) => {
                        assert!(sat);
                        assert!(!principal);
                    }
                    Err(PreparedEffectFailure::Unsatisfiable) => {
                        assert!(!sat);
                        assert!(!principal);
                    }
                    Err(error) => panic!("unexpected original relation result: {error:?}"),
                }
                let fixed = source
                    .specialize_partial(&[(&f, &fv), (&g, &gv)], &mut budget())
                    .unwrap();
                let fixed = fixed.reduce(&mut budget()).unwrap();
                assert_eq!(fixed.satisfiable().truth_value(), Some(sat));
                assert_eq!(fixed.principal().truth_value(), Some(principal));
            }
        }
    }
}
