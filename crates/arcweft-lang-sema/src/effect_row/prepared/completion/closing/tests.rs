use super::*;
use crate::effect_row::prepared::test_support::*;

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

fn freeze(graph: &PreparedEffectGraph, rows: &[&PreparedEffectRow]) -> Arc<ScopedEffectCompletion> {
    graph
        .read()
        .freeze_scoped(rows, &[], &[], &mut budget())
        .unwrap()
}

fn literal_row(labels: &[&str]) -> EffectRow {
    match literal(labels) {
        PreparedEffectRow::Canonical(row) => row,
        _ => unreachable!(),
    }
}

#[test]
fn a_verified_closure_keeps_its_original_relation_after_preparation_changes_and_drop() {
    let (principal, row, unknown) = {
        let mut graph = PreparedEffectGraph::default();
        let unknown = graph
            .declare_unknown(
                vec![],
                PreparedEffectUnknownEligibility::Bindable,
                &mut budget(),
            )
            .unwrap();
        let row = graph
            .unknown_application(&unknown, vec![], &mut budget())
            .unwrap();
        let principal = graph.solve_principal(&mut budget()).unwrap();
        let predicate = graph
            .relate(
                PreparedEffectRelation::Equal,
                row.clone(),
                literal(&["later.label"]),
                &mut budget(),
            )
            .unwrap();
        graph
            .add_scoped_obligation(vec![], truth(), predicate, &mut budget())
            .unwrap();
        assert_eq!(
            graph
                .solve_principal(&mut budget())
                .unwrap()
                .unknown(&unknown)
                .unwrap(),
            &literal_row(&["later.label"])
        );
        (principal, row, unknown)
    };
    assert_eq!(principal.unknown(&unknown).unwrap(), &literal_row(&[]));
    assert_eq!(
        principal.resolve_row(&row, &mut budget()).unwrap(),
        literal_row(&[])
    );
}

#[test]
fn original_private_constraints_cannot_disappear_at_final_closure() {
    let graph = PreparedEffectGraph::default();
    let impossible = PreparedEffectPredicate::Canonical(EffectPredicate::impossible());
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &impossible, &mut budget())
        .unwrap();
    let source = graph
        .read()
        .freeze_scoped(&[], &[], &[&receipt], &mut budget())
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
    assert!(graph.solve_principal(&mut budget()).is_ok());
    assert!(graph.relation.obligations.is_empty());
}

#[test]
fn a_later_demand_cannot_replace_the_frozen_relation_least_row() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let local = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut work,
        )
        .unwrap();
    let row = graph
        .unknown_application(&local, vec![], &mut work)
        .unwrap();
    let source =
        BoundScopedEffectCompletion::pending(freeze(&graph, &[&row]), graph.read(), &mut work)
            .unwrap();
    let inputs = graph.read().actual_inputs(&[], &mut work).unwrap();
    let closed = source
        .close_original_with_inputs(&mut graph, &[&row], &[], &inputs, &mut work)
        .unwrap();
    assert_eq!(closed.unknown(&local).unwrap(), &literal_row(&[]));

    let demand = graph
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            literal(&["later.label"]),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), demand, &mut work)
        .unwrap();
    let state = graph.state;
    assert!(matches!(
        source.close_original_with_inputs(&mut graph, &[&row], &[], &inputs, &mut work),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
    assert_eq!(graph.state, state);
}

#[test]
fn final_closure_waits_for_the_original_named_body_and_never_prepares_it() {
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let row = graph.application(&named, vec![], &mut budget()).unwrap();
    let source = freeze(&graph, &[&row]);
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
    assert!(matches!(
        graph.scheme(&named).unwrap().state,
        SchemeState::Declared
    ));
    graph
        .prepare_body(&named, literal(&["actual.label"]), vec![], &mut budget())
        .unwrap();
    let principal = source
        .close_original(graph.read(), &[], &[], &[], &mut budget())
        .unwrap();
    assert_eq!(
        principal.resolve_row(&row, &mut budget()).unwrap(),
        literal_row(&["actual.label"])
    );
    assert!(matches!(
        graph.prepare_body(&named, literal(&[]), vec![], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
}

#[test]
fn final_closure_follows_the_actual_body_with_unrelated_declarations_still_open() {
    let mut graph = PreparedEffectGraph::default();
    let unrelated = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let row = graph.application(&named, vec![], &mut budget()).unwrap();
    let source = freeze(&graph, &[&row]);

    let callee = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let body = graph.application(&callee, vec![], &mut budget()).unwrap();
    graph
        .prepare_body(&named, body, vec![], &mut budget())
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
    graph
        .prepare_body(&callee, literal(&["actual.callee"]), vec![], &mut budget())
        .unwrap();
    let principal = source
        .close_original(graph.read(), &[], &[], &[], &mut budget())
        .unwrap();
    let expected = literal_row(&["actual.callee"]);
    assert_eq!(
        principal.resolve_row(&row, &mut budget()).unwrap(),
        expected
    );
    assert_eq!(principal.scheme(&callee).unwrap(), &expected);
    assert_eq!(
        principal.scheme(&unrelated),
        Err(PreparedEffectInvariant::StaleReference)
    );
}

#[test]
fn a_future_added_after_freeze_is_required_if_a_final_query_reaches_it() {
    let mut graph = PreparedEffectGraph::default();
    let source = freeze(&graph, &[]);
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut budget(),
        )
        .unwrap();
    let query = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    let principal = source
        .close_original(graph.read(), &[], &[], &[], &mut budget())
        .unwrap();
    assert_eq!(
        principal.unknown(&future),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert!(matches!(
        source.close_original(graph.read(), &[&query], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    let actual = literal_row(&["actual.future"]);
    let principal = source
        .close_original(
            graph.read(),
            &[&query],
            &[],
            &[(&future, &actual)],
            &mut budget(),
        )
        .unwrap();
    assert_eq!(
        principal.resolve_row(&query, &mut budget()).unwrap(),
        actual
    );
}

#[test]
fn a_validated_actual_batch_specializes_only_the_futures_each_product_reaches() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let first = graph
        .declare_unknown(vec![], PreparedEffectUnknownEligibility::Future, &mut work)
        .unwrap();
    let second = graph
        .declare_unknown(vec![], PreparedEffectUnknownEligibility::Future, &mut work)
        .unwrap();
    let first_row = graph
        .unknown_application(&first, vec![], &mut work)
        .unwrap();
    let second_row = graph
        .unknown_application(&second, vec![], &mut work)
        .unwrap();
    let first_source = graph
        .read()
        .freeze_product(&[&first_row], &[], &[], &[], &mut work)
        .unwrap();
    let second_source = graph
        .read()
        .freeze_product(&[&second_row], &[], &[], &[], &mut work)
        .unwrap();
    let first_actual = literal_row(&["first.actual"]);
    let second_actual = literal_row(&["second.actual"]);
    let input = graph
        .read()
        .actual_inputs(
            &[(&first, &first_actual), (&second, &second_actual)],
            &mut work,
        )
        .unwrap();
    for (source, row, expected, sibling) in [
        (first_source, first_row, &first_actual, second.clone()),
        (second_source, second_row, &second_actual, first.clone()),
    ] {
        let closed = source
            .close_original_with_inputs(graph.read(), &[], &[], &input, &mut work)
            .unwrap();
        assert_eq!(&closed.resolve_row(&row, &mut work).unwrap(), expected);
        assert_eq!(
            closed.unknown(&sibling),
            Err(PreparedEffectInvariant::StaleReference)
        );
    }
    assert!(matches!(
        graph.read().actual_inputs(
            &[(&second, &literal_row(&[])), (&second, &literal_row(&[]))],
            &mut work
        ),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
}

#[test]
fn final_closure_preserves_the_bound_original_identity_with_late_named_values() {
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let local = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut budget(),
        )
        .unwrap();
    let output = graph
        .unknown_application(&local, vec![], &mut budget())
        .unwrap();
    let actual = graph.application(&named, vec![], &mut budget()).unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            output.clone(),
            actual,
            &mut budget(),
        )
        .unwrap();
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &predicate, &mut budget())
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[&output], &[], &[&receipt], &mut budget())
        .unwrap();
    let pending =
        BoundScopedEffectCompletion::pending(Arc::clone(&original), graph.read(), &mut budget())
            .unwrap();
    let source = graph
        .bind_original_scoped(&original, &mut budget())
        .unwrap()
        .freeze(graph.read(), &mut budget())
        .unwrap();
    // The bound header only knows `named`. Final closure must also visit the
    // declaration its actual body reaches, even with no query roots.
    let child = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let body = graph.application(&child, vec![], &mut budget()).unwrap();
    graph
        .prepare_body(&named, body, vec![], &mut budget())
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
    let inputs = graph.read().actual_inputs(&[], &mut budget()).unwrap();
    let state = graph.state;
    assert!(matches!(
        pending.close_original_with_inputs(&mut graph, &[], &[], &inputs, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
    assert_eq!(graph.state, state);
    graph
        .prepare_body(&child, literal(&["late.named"]), vec![], &mut budget())
        .unwrap();
    let principal = source
        .close_original(graph.read(), &[], &[], &[], &mut budget())
        .unwrap();
    assert_eq!(
        principal.resolve_row(&output, &mut budget()).unwrap(),
        literal_row(&["late.named"])
    );
    let state = graph.state;
    let mut staged_work = budget();
    let mut checkpoint = graph.checkpoint(&mut staged_work).unwrap();
    let staged = graph
        .bind_original_scoped(&original, &mut staged_work)
        .unwrap()
        .freeze(graph.read(), &mut staged_work)
        .unwrap()
        .final_definitions(graph.read(), &[], &[], &mut staged_work)
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    let mut direct_work = budget();
    let direct = pending
        .attached_definitions(&mut graph, &[], &[], &mut direct_work)
        .unwrap();
    assert_eq!(graph.state, state);
    assert!(direct_work.0 > staged_work.0);
    assert_eq!(direct.obligations.len(), staged.obligations.len());
    assert!(Arc::ptr_eq(&direct.obligations[0], &staged.obligations[0]));
    let direct = direct
        .solve_with_actual_inputs(&inputs, &mut budget())
        .unwrap();
    assert_eq!(
        direct.resolve_row(&output, &mut budget()).unwrap(),
        principal.resolve_row(&output, &mut budget()).unwrap()
    );
    let demand = graph
        .relate(
            PreparedEffectRelation::Equal,
            output,
            literal(&["consumer.only"]),
            &mut budget(),
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), demand, &mut budget())
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
    let state = graph.state;
    assert!(matches!(
        pending.close_original_with_inputs(&mut graph, &[], &[], &inputs, &mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
    assert_eq!(graph.state, state);
    assert_eq!(
        principal.unknown(&local).unwrap(),
        &literal_row(&["late.named"])
    );
}

#[test]
fn actual_named_body_and_new_global_constraints_both_participate_in_original_checks() {
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let row = graph.application(&named, vec![], &mut budget()).unwrap();
    let required = graph
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            literal(&["required.label"]),
            &mut budget(),
        )
        .unwrap();
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &required, &mut budget())
        .unwrap();
    let source = graph
        .read()
        .freeze_scoped(&[&row], &[], &[&receipt], &mut budget())
        .unwrap();
    graph
        .prepare_body(&named, literal(&["different.label"]), vec![], &mut budget())
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));

    let mut graph = PreparedEffectGraph::default();
    let source = freeze(&graph, &[]);
    graph
        .add_scoped_obligation(
            vec![],
            truth(),
            PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
            &mut budget(),
        )
        .unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn an_actual_future_row_is_required_and_extends_the_final_label_basis() {
    let mut graph = PreparedEffectGraph::default();
    let local = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut budget(),
        )
        .unwrap();
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut budget(),
        )
        .unwrap();
    let row = graph
        .unknown_application(&local, vec![], &mut budget())
        .unwrap();
    let actual = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            actual,
            &mut budget(),
        )
        .unwrap();
    let receipt = graph
        .read()
        .scoped_constraint(&[], &truth(), &predicate, &mut budget())
        .unwrap();
    let source = graph
        .read()
        .freeze_scoped(&[&row], &[], &[&receipt], &mut budget())
        .unwrap();
    let pending =
        BoundScopedEffectCompletion::pending(source, graph.read(), &mut budget()).unwrap();
    let missing = graph.read().actual_inputs(&[], &mut budget()).unwrap();
    let state = graph.state;
    assert!(matches!(
        pending.close_original_with_inputs(&mut graph, &[], &[], &missing, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    assert_eq!(graph.state, state);
    let actual = literal_row(&["late.actual"]);
    let inputs = graph
        .read()
        .actual_inputs(&[(&future, &actual)], &mut budget())
        .unwrap();
    let mut direct_work = budget();
    let principal = pending
        .close_original_with_inputs(&mut graph, &[], &[], &inputs, &mut direct_work)
        .unwrap();
    let mut attached_work = budget();
    let attached = pending
        .attached_definitions(&mut graph, &[], &[], &mut attached_work)
        .unwrap()
        .solve_with_actual_inputs(&inputs, &mut attached_work)
        .unwrap();
    assert_eq!(attached.resolve_row(&row, &mut budget()).unwrap(), actual);
    assert_eq!(principal.resolve_row(&row, &mut budget()).unwrap(), actual);
    assert_eq!(principal.unknown(&local).unwrap(), &actual);
    assert!(direct_work.0 > attached_work.0);
    assert_eq!(graph.state, state);
    assert!(matches!(
        graph
            .read()
            .actual_inputs(&[(&future, &actual), (&future, &actual)], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::DuplicateFutureBinding
        ))
    ));
}

#[test]
fn final_closure_can_borrow_an_extracted_actual_body_without_publishing_it() {
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let row = graph.application(&named, vec![], &mut budget()).unwrap();
    let source = freeze(&graph, &[&row]);
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    graph
        .prepare_body(&named, literal(&["candidate.body"]), vec![], &mut budget())
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut budget()).unwrap();
    let principal = source
        .close_original(
            graph.read_extracted(&delta, &mut budget()).unwrap(),
            &[],
            &[],
            &[],
            &mut budget(),
        )
        .unwrap();
    assert!(matches!(
        graph.scheme(&named).unwrap().state,
        SchemeState::Declared
    ));
    drop(delta);
    drop(graph);
    assert_eq!(
        principal.resolve_row(&row, &mut budget()).unwrap(),
        literal_row(&["candidate.body"])
    );
}

#[test]
fn foreign_stale_specialized_and_exhausted_final_closures_return_no_partial_result() {
    let mut graph = PreparedEffectGraph::default();
    let unknown = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut budget(),
        )
        .unwrap();
    let query = graph
        .unknown_application(&unknown, vec![], &mut budget())
        .unwrap();
    let source = freeze(&graph, &[&query]);
    let before = (
        graph.retained_terms(),
        graph.relation.functions.len(),
        graph.relation.obligations.len(),
        graph.state,
    );
    let mut measured = Budget(10_000);
    source
        .close_original(graph.read(), &[], &[], &[], &mut measured)
        .unwrap();
    let needed = 10_000 - measured.0;
    for available in 0..needed {
        let mut work = Budget(available);
        assert!(matches!(
            source.close_original(graph.read(), &[], &[], &[], &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(
            (
                graph.retained_terms(),
                graph.relation.functions.len(),
                graph.relation.obligations.len(),
                graph.state
            ),
            before
        );
    }
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        source.close_original(foreign.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut budget(),
        )
        .unwrap();
    let source = freeze(&graph, &[]);
    let fixed = source
        .specialize_partial(&[(&future, &literal_row(&[]))], &mut budget())
        .unwrap();
    assert!(matches!(
        fixed.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ImportedRelationOrigin
        ))
    ));
    graph.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        source.close_original(graph.read(), &[], &[], &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
}
