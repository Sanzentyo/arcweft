use super::*;
use crate::effect_row::prepared::test_support::{Budget, Exhausted, budget, literal, open};
use crate::effect_row::prepared::{
    PreparedEffectChoice, PreparedEffectPredicate, PreparedEffectRelation, PreparedEffectRow,
    PreparedEffectSchemeRef, SchemeState,
};
use crate::effect_row::{EffectPredicate, EffectRow};
use crate::{
    effects::EffectSet,
    types::{GenericBinder, GenericScope},
};

struct Candidate {
    before: PreparedEffectSchemeRef,
    added: PreparedEffectSchemeRef,
    row: PreparedEffectRow,
    predicate: PreparedEffectPredicate,
    choice: PreparedEffectChoice,
    delta: PreparedEffectDelta,
}

fn candidate(graph: &mut PreparedEffectGraph) -> Candidate {
    let mut work = budget();
    let before = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let added = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let a = graph.application(&before, vec![], &mut work).unwrap();
    let b = graph.application(&added, vec![], &mut work).unwrap();
    let body = graph
        .union(vec![b.clone(), literal(&["log.write"])], &mut work)
        .unwrap();
    graph
        .prepare_body(&before, body, vec![], &mut work)
        .unwrap();
    graph
        .prepare_body(&added, a.clone(), vec![], &mut work)
        .unwrap();
    let choice = graph.choice(&mut work).unwrap();
    let row = graph.select_row(&choice, a, b, &mut work).unwrap();
    let predicate = graph
        .relate(
            PreparedEffectRelation::Equal,
            row.clone(),
            literal(&["log.write"]),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(
            vec![],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            predicate.clone(),
            &mut work,
        )
        .unwrap();
    let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    Candidate {
        before,
        added,
        row,
        predicate,
        choice,
        delta,
    }
}

fn assert_stale(graph: &PreparedEffectGraph, value: &Candidate) {
    assert_eq!(
        graph.validate_retained_row(&value.row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        graph.validate_retained_predicate(&value.predicate),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        graph.validate_choice(&value.choice),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert!(matches!(
        graph.scheme(&value.added),
        Err(PreparedEffectInvariant::StaleReference)
    ));
    assert!(matches!(
        graph.scheme(&value.before).unwrap().state,
        SchemeState::Declared
    ));
}

#[test]
fn extracted_candidate_restores_recursive_bodies_choices_and_whole_row_obligations() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let value = candidate(&mut graph);
    assert_stale(&graph, &value);
    let row = value.row;
    let predicate = value.predicate;
    let choice = value.choice;
    let before = value.before;
    let added = value.added;
    graph.restore_delta(value.delta, &mut work).unwrap();
    graph.validate_retained_row(&row).unwrap();
    graph.validate_retained_predicate(&predicate).unwrap();
    graph.validate_choice(&choice).unwrap();
    let closed = graph.solve_principal(&mut work).unwrap();
    let expected = EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap());
    assert_eq!(closed.scheme(&before).unwrap(), &expected);
    assert_eq!(closed.scheme(&added).unwrap(), &expected);
    assert_eq!(closed.resolve_row(&row, &mut work).unwrap(), expected);
}

#[test]
fn outer_rollback_removes_restored_terms_whose_ordinals_precede_its_checkpoint() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let value = candidate(&mut graph);
    let mut outer = graph.checkpoint(&mut work).unwrap();
    let inner = graph.checkpoint(&mut work).unwrap();
    let Candidate {
        before,
        added,
        row,
        predicate,
        choice,
        delta,
    } = value;
    graph.restore_delta(delta, &mut work).unwrap();
    graph.commit(&inner).unwrap();
    graph.rollback(&mut outer).unwrap();
    assert_eq!(
        graph.validate_retained_row(&row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        graph.validate_retained_predicate(&predicate),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert_eq!(
        graph.validate_choice(&choice),
        Err(PreparedEffectInvariant::StaleReference)
    );
    assert!(matches!(
        graph.scheme(&before).unwrap().state,
        SchemeState::Declared
    ));
    assert!(matches!(
        graph.scheme(&added),
        Err(PreparedEffectInvariant::StaleReference)
    ));
    assert!(graph.obligations.is_empty());
    assert!(graph.journal.is_empty());
}

#[test]
fn restore_abort_returns_the_original_delta_without_changing_the_base() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let value = candidate(&mut graph);
    let base = graph.state;
    let (error, delta) = graph
        .restore_delta(value.delta, &mut Budget(2))
        .unwrap_err()
        .into_parts();
    assert!(matches!(error, PreparedEffectFailure::Control(Exhausted)));
    assert_eq!(graph.state, base);
    assert!(graph.obligations.is_empty());
    assert_eq!(graph.retained_terms(), (0, 0));
    graph.restore_delta(delta, &mut work).unwrap();
    graph.validate_retained_row(&value.row).unwrap();
    assert!(graph.solve_principal(&mut work).is_ok());
}

#[test]
fn delta_rejects_a_changed_base_and_can_retry_after_that_change_rolls_back() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let value = candidate(&mut graph);
    let mut changed = graph.checkpoint(&mut work).unwrap();
    // Canonical-only obligations change no term or function inventory.
    graph
        .add_scoped_obligation(
            vec![],
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            &mut work,
        )
        .unwrap();
    let (error, delta) = graph
        .restore_delta(value.delta, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::DeltaBase)
    ));
    assert_eq!(graph.obligations.len(), 1);
    graph.rollback(&mut changed).unwrap();
    graph.restore_delta(delta, &mut work).unwrap();
    graph.validate_retained_row(&value.row).unwrap();
}

#[test]
fn foreign_graph_rejection_preserves_the_delta_for_its_actual_issuer() {
    let mut graph = PreparedEffectGraph::default();
    let mut foreign = PreparedEffectGraph::default();
    let mut work = budget();
    let value = candidate(&mut graph);
    let (error, delta) = foreign
        .restore_delta(value.delta, &mut work)
        .unwrap_err()
        .into_parts();
    assert!(matches!(
        error,
        PreparedEffectFailure::Invariant(PreparedEffectInvariant::ForeignIssuer)
    ));
    assert_eq!(foreign.retained_terms(), (0, 0));
    graph.restore_delta(delta, &mut work).unwrap();
    graph.validate_retained_row(&value.row).unwrap();
}

#[test]
fn extraction_abort_preserves_the_live_candidate_and_checkpoint() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scheme = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let row = graph.application(&scheme, vec![], &mut work).unwrap();
    graph
        .prepare_body(&scheme, literal(&["fs.read"]), vec![], &mut work)
        .unwrap();
    let original = graph.state;
    assert!(matches!(
        graph.extract_delta(&mut checkpoint, &mut Budget(1)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    assert_eq!(graph.state, original);
    graph.validate_retained_row(&row).unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    assert_eq!(
        graph.validate_retained_row(&row),
        Err(PreparedEffectInvariant::StaleReference)
    );
    graph
        .prepare_body(&scheme, literal(&[]), vec![], &mut work)
        .unwrap();
}

#[test]
fn dropped_term_edit_restores_the_candidate_journal_and_base_stamp() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let formal = scope.bound_effect(0, 0).unwrap();
    let scheme = graph
        .declare(
            vec![formal.clone()],
            EffectPredicate::unconstrained(),
            &mut work,
        )
        .unwrap();
    let input = graph
        .application(&scheme, vec![open(&formal)], &mut work)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let base = graph.state;
    let lost = {
        let mut edit = graph.edit_terms();
        edit.map_row_parameters(&input, &mut work, &mut |_, _| {
            Ok(Some(literal(&["fs.read"])))
        })
        .unwrap()
    };
    assert_eq!(graph.state, base);
    assert_eq!(
        graph.validate_retained_row(&lost),
        Err(PreparedEffectInvariant::StaleReference)
    );
    let delta = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    graph.restore_delta(delta, &mut work).unwrap();
    graph.validate_retained_row(&input).unwrap();
    assert_eq!(
        graph.validate_retained_row(&lost),
        Err(PreparedEffectInvariant::StaleReference)
    );
}

#[test]
fn candidate_work_does_not_scan_or_copy_unmodified_declaration_headers() {
    fn measure(unrelated: usize) -> usize {
        let mut graph = PreparedEffectGraph::default();
        let mut setup = budget();
        let scheme = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut setup)
            .unwrap();
        for _ in 0..unrelated {
            graph
                .declare(vec![], EffectPredicate::unconstrained(), &mut setup)
                .unwrap();
        }
        let references = Arc::strong_count(&graph.functions[&scheme.0.ordinal]);
        let mut work = Budget(4096);
        let mut first = graph.checkpoint(&mut work).unwrap();
        assert_eq!(
            Arc::strong_count(&graph.functions[&scheme.0.ordinal]),
            references
        );
        graph
            .prepare_body(&scheme, literal(&["fs.read"]), vec![], &mut work)
            .unwrap();
        let left = graph.extract_delta(&mut first, &mut work).unwrap();
        let mut second = graph.checkpoint(&mut work).unwrap();
        graph
            .prepare_body(&scheme, literal(&["fs.read"]), vec![], &mut work)
            .unwrap();
        let right = graph.extract_delta(&mut second, &mut work).unwrap();
        assert_eq!(graph.functions.len(), unrelated + 1);
        assert_eq!(
            Arc::strong_count(&graph.functions[&scheme.0.ordinal]),
            references
        );
        assert_eq!(
            graph
                .replay_delta_mismatch(
                    &left,
                    &right,
                    [PreparedEffectReplayRoot::Scheme(&scheme, &scheme)],
                    &mut work
                )
                .unwrap(),
            None
        );
        4096 - work.0
    }

    assert_eq!(measure(0), measure(4096));
}

#[test]
fn nested_function_journal_restores_original_definitions_after_transfer_and_commit() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let original = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let original_definition = Arc::clone(&graph.functions[&original.0.ordinal]);
    let mut outer = graph.checkpoint(&mut work).unwrap();
    let added = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let inner = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(&original, literal(&["fs.read"]), vec![], &mut work)
        .unwrap();
    graph
        .prepare_body(&added, literal(&["log.write"]), vec![], &mut work)
        .unwrap();
    graph.commit(&inner).unwrap();
    let delta = graph.extract_delta(&mut outer, &mut work).unwrap();
    assert!(Arc::ptr_eq(
        &graph.functions[&original.0.ordinal],
        &original_definition
    ));
    assert!(matches!(
        graph.scheme(&added),
        Err(PreparedEffectInvariant::StaleReference)
    ));
    let mut destination = graph.checkpoint(&mut work).unwrap();
    let child = graph.checkpoint(&mut work).unwrap();
    graph.restore_delta(delta, &mut work).unwrap();
    graph.commit(&child).unwrap();
    let closed = graph.solve_principal(&mut work).unwrap();
    assert_eq!(
        closed.scheme(&original).unwrap(),
        &EffectRow::closed(EffectSet::from_labels(["fs.read"]).unwrap())
    );
    assert_eq!(
        closed.scheme(&added).unwrap(),
        &EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap())
    );
    graph.rollback(&mut destination).unwrap();
    assert!(Arc::ptr_eq(
        &graph.functions[&original.0.ordinal],
        &original_definition
    ));
    assert!(matches!(
        graph.scheme(&added),
        Err(PreparedEffectInvariant::StaleReference)
    ));
    assert!(graph.journal.is_empty());
}
