use super::{
    Arc, BTreeMap, PreparedEffectDelta, PreparedEffectFailure, PreparedEffectGraph,
    PreparedEffectInvariant, PreparedEffectPredicate, PreparedEffectReplayMismatch,
    PreparedEffectReplayRoot, PreparedEffectRow, PreparedEffectSchemeRef, PreparedEffectUnknownRef,
};
use crate::effect_row::EffectPredicate;
use crate::effect_row::prepared::test_support::{Budget, Exhausted, budget, literal};
use crate::effect_row::prepared::{PreparedEffectRelation, PreparedEffectUnknownEligibility};

struct Candidate {
    first: PreparedEffectSchemeRef,
    second: PreparedEffectSchemeRef,
    row: PreparedEffectRow,
    predicate: PreparedEffectPredicate,
    delta: PreparedEffectDelta,
}

fn candidate(graph: &mut PreparedEffectGraph, reverse_allocations: bool) -> Candidate {
    let mut work = budget();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let a = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let b = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let (first, second) = if reverse_allocations { (b, a) } else { (a, b) };
    let first_row = graph.application(&first, vec![], &mut work).unwrap();
    let second_row = graph.application(&second, vec![], &mut work).unwrap();
    let body = graph
        .union(vec![second_row.clone(), literal(&["log.write"])], &mut work)
        .unwrap();
    graph.prepare_body(&first, body, vec![], &mut work).unwrap();
    graph
        .prepare_body(&second, first_row.clone(), vec![], &mut work)
        .unwrap();
    let choice = graph.choice(&mut work).unwrap();
    let row = graph
        .select_row(&choice, first_row, second_row, &mut work)
        .unwrap();
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
        first,
        second,
        row,
        predicate,
        delta,
    }
}

fn roots<'a>(left: &'a Candidate, right: &'a Candidate) -> [PreparedEffectReplayRoot<'a>; 4] {
    [
        PreparedEffectReplayRoot::Scheme(&left.first, &right.first),
        PreparedEffectReplayRoot::Scheme(&left.second, &right.second),
        PreparedEffectReplayRoot::Row(&left.row, &right.row),
        PreparedEffectReplayRoot::Predicate(&left.predicate, &right.predicate),
    ]
}

#[test]
fn recursive_replay_uses_actual_ports_despite_different_allocation_order_and_gaps() {
    let mut graph = PreparedEffectGraph::default();
    let left = candidate(&mut graph, false);
    let right = candidate(&mut graph, true);
    assert_ne!(left.first, right.first);
    let mut work = budget();
    assert_eq!(
        graph
            .replay_delta_mismatch(&left.delta, &right.delta, roots(&left, &right), &mut work)
            .unwrap(),
        None
    );
    // Both affine candidates remain independently usable after comparison.
    for value in [left, right] {
        let mut outer = graph.checkpoint(&mut work).unwrap();
        graph.restore_delta(value.delta, &mut work).unwrap();
        let closure = graph.solve_principal(&mut work).unwrap();
        assert_eq!(
            closure.scheme(&value.first).unwrap(),
            closure.scheme(&value.second).unwrap()
        );
        graph.rollback(&mut outer).unwrap();
    }
}

#[test]
fn every_replay_budget_boundary_preserves_both_payloads_and_the_live_base() {
    let mut graph = PreparedEffectGraph::default();
    let left = candidate(&mut graph, false);
    let right = candidate(&mut graph, true);
    let original_state = graph.state;
    let mut full = budget();
    let start = full.0;
    assert_eq!(
        graph
            .replay_delta_mismatch(&left.delta, &right.delta, roots(&left, &right), &mut full)
            .unwrap(),
        None
    );
    let needed = start - full.0;
    assert!(needed > 30);
    for available in 0..needed {
        assert!(matches!(
            graph.replay_delta_mismatch(
                &left.delta,
                &right.delta,
                roots(&left, &right),
                &mut Budget(available)
            ),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(graph.state, original_state);
        assert_eq!(graph.retained_terms(), (0, 0));
    }
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left.delta,
                &right.delta,
                roots(&left, &right),
                &mut Budget(needed)
            )
            .unwrap(),
        None
    );
}

#[test]
fn choices_keep_cross_observation_correlation() {
    fn selected(
        graph: &mut PreparedEffectGraph,
        split: bool,
    ) -> (PreparedEffectDelta, [PreparedEffectRow; 2]) {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let first = graph.choice(&mut work).unwrap();
        let second = if split {
            graph.choice(&mut work).unwrap()
        } else {
            first.clone()
        };
        let a = graph
            .select_row(&first, literal(&["log.write"]), literal(&[]), &mut work)
            .unwrap();
        let b = graph
            .select_row(&second, literal(&["fs.read"]), literal(&[]), &mut work)
            .unwrap();
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            [a, b],
        )
    }
    let mut graph = PreparedEffectGraph::default();
    let (left, a) = selected(&mut graph, false);
    let (right, b) = selected(&mut graph, true);
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                a.iter()
                    .zip(&b)
                    .map(|(a, b)| PreparedEffectReplayRoot::Row(a, b)),
                &mut budget()
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::ChoiceIdentity)
    );
}

#[test]
fn dag_sharing_and_unobserved_term_allocations_are_not_equation_identities() {
    fn dag(
        graph: &mut PreparedEffectGraph,
        duplicate: bool,
    ) -> (PreparedEffectDelta, PreparedEffectRow) {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let a = graph
            .union(vec![literal(&["log.write"])], &mut work)
            .unwrap();
        let b = if duplicate {
            graph
                .union(vec![literal(&["log.write"])], &mut work)
                .unwrap()
        } else {
            a.clone()
        };
        let row = graph.union(vec![a, b], &mut work).unwrap();
        let _unused = graph.union(vec![literal(&["fs.read"])], &mut work).unwrap();
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            row,
        )
    }
    let mut graph = PreparedEffectGraph::default();
    let (left, a) = dag(&mut graph, false);
    let (right, b) = dag(&mut graph, true);
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Row(&a, &b)],
                &mut budget()
            )
            .unwrap(),
        None
    );
}

#[test]
fn unknown_roles_and_unpaired_function_owners_remain_part_of_the_relation() {
    fn unknown(
        graph: &mut PreparedEffectGraph,
        role: PreparedEffectUnknownEligibility,
    ) -> (PreparedEffectDelta, PreparedEffectUnknownRef) {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let reference = graph.declare_unknown(vec![], role, &mut work).unwrap();
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            reference,
        )
    }
    let mut graph = PreparedEffectGraph::default();
    let (left, a) = unknown(&mut graph, PreparedEffectUnknownEligibility::Bindable);
    let (right, b) = unknown(&mut graph, PreparedEffectUnknownEligibility::Future);
    assert_eq!(
        graph
            .replay_delta_mismatch(&left, &right, [], &mut budget())
            .unwrap(),
        Some(PreparedEffectReplayMismatch::FunctionInventory)
    );
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Unknown(&a, &b)],
                &mut budget()
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::FunctionDefinition)
    );
}

#[test]
fn baseline_and_new_term_reuse_are_compared_by_the_same_grammar() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let first = graph
        .union(vec![literal(&["log.write"])], &mut work)
        .unwrap();
    let second = graph
        .union(vec![literal(&["log.write"])], &mut work)
        .unwrap();
    let a = graph
        .relate(
            PreparedEffectRelation::Equal,
            first.clone(),
            literal(&["log.write"]),
            &mut work,
        )
        .unwrap();
    let b = graph
        .relate(
            PreparedEffectRelation::Equal,
            second,
            literal(&["log.write"]),
            &mut work,
        )
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let left = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    let new_row = graph
        .union(vec![literal(&["log.write"])], &mut work)
        .unwrap();
    let c = graph
        .relate(
            PreparedEffectRelation::Equal,
            new_row.clone(),
            literal(&["log.write"]),
            &mut work,
        )
        .unwrap();
    let right = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [
                    PreparedEffectReplayRoot::Row(&first, &new_row),
                    PreparedEffectReplayRoot::Predicate(&a, &b),
                    PreparedEffectReplayRoot::Predicate(&a, &c),
                ],
                &mut work
            )
            .unwrap(),
        None
    );
}

#[test]
fn changed_baseline_declarations_cannot_be_alpha_renamed() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let a = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let b = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(&a, literal(&["log.write"]), vec![], &mut work)
        .unwrap();
    let left = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    let mut checkpoint = graph.checkpoint(&mut work).unwrap();
    graph
        .prepare_body(&b, literal(&["log.write"]), vec![], &mut work)
        .unwrap();
    let right = graph.extract_delta(&mut checkpoint, &mut work).unwrap();
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Scheme(&a, &b)],
                &mut work
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::FunctionInventory)
    );
}

#[test]
fn whole_row_domains_are_not_erased_by_a_true_selected_predicate() {
    fn obligation(graph: &mut PreparedEffectGraph, domain: bool) -> PreparedEffectDelta {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        graph
            .add_scoped_obligation(
                vec![],
                PreparedEffectPredicate::Canonical(if domain {
                    EffectPredicate::unconstrained()
                } else {
                    EffectPredicate::impossible()
                }),
                PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
                &mut work,
            )
            .unwrap();
        graph.extract_delta(&mut checkpoint, &mut work).unwrap()
    }
    let mut graph = PreparedEffectGraph::default();
    let left = obligation(&mut graph, true);
    let right = obligation(&mut graph, false);
    assert_eq!(
        graph
            .replay_delta_mismatch(&left, &right, [], &mut budget())
            .unwrap(),
        Some(PreparedEffectReplayMismatch::Predicate)
    );
}

#[test]
fn replay_requires_the_actual_live_base_and_rejects_stale_observation_ports() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let left = candidate(&mut graph, false);
    let right = candidate(&mut graph, false);
    let mut changed = graph.checkpoint(&mut work).unwrap();
    let stale = graph.union(vec![], &mut work).unwrap();
    assert_eq!(
        graph
            .replay_delta_mismatch(&left.delta, &right.delta, roots(&left, &right), &mut work)
            .unwrap(),
        Some(PreparedEffectReplayMismatch::Base)
    );
    graph.rollback(&mut changed).unwrap();
    assert!(matches!(
        graph.replay_delta_mismatch(
            &left.delta,
            &right.delta,
            [PreparedEffectReplayRoot::Row(&stale, &stale)],
            &mut work
        ),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    let foreign = PreparedEffectGraph::default();
    assert_eq!(
        foreign
            .replay_delta_mismatch(&left.delta, &right.delta, roots(&left, &right), &mut work)
            .unwrap(),
        Some(PreparedEffectReplayMismatch::Issuer)
    );
}

#[test]
fn deep_row_replay_uses_a_bounded_iterative_walk() {
    fn deep(graph: &mut PreparedEffectGraph) -> (PreparedEffectDelta, PreparedEffectRow) {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let mut row = literal(&["log.write"]);
        for _ in 0..8_000 {
            row = graph.union(vec![row], &mut work).unwrap();
        }
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            row,
        )
    }
    let mut graph = PreparedEffectGraph::default();
    let (left, a) = deep(&mut graph);
    let (right, b) = deep(&mut graph);
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Row(&a, &b)],
                &mut budget()
            )
            .unwrap(),
        None
    );
}

#[test]
fn imported_relations_keep_the_original_lexical_owner_and_actual_fixed_inputs() {
    let mut work = budget();
    let mut child = PreparedEffectGraph::default();
    let future = child
        .declare_unknown(vec![], PreparedEffectUnknownEligibility::Future, &mut work)
        .unwrap();
    let scheme = child
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let row = child
        .unknown_application(&future, vec![], &mut work)
        .unwrap();
    child.prepare_body(&scheme, row, vec![], &mut work).unwrap();
    let completion = child.complete_scoped(&mut work).unwrap();
    let log_row = match literal(&["log.write"]) {
        PreparedEffectRow::Canonical(row) => row,
        _ => unreachable!(),
    };
    let read_row = match literal(&["fs.read"]) {
        PreparedEffectRow::Canonical(row) => row,
        _ => unreachable!(),
    };
    let a = completion
        .specialize_partial(&[(&future, &log_row)], &mut work)
        .unwrap();
    let b = completion
        .specialize_partial(&[(&future, &log_row)], &mut work)
        .unwrap();
    let c = completion
        .specialize_partial(&[(&future, &read_row)], &mut work)
        .unwrap();
    assert!(!Arc::ptr_eq(&a, &b));
    let mut graph = PreparedEffectGraph::default();
    let mut imported = |completion: &Arc<crate::effect_row::prepared::ScopedEffectCompletion>| {
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let reference = graph
            .import_guarded_function(&completion.scheme(&scheme).unwrap(), &[], &mut work)
            .unwrap();
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            reference,
        )
    };
    let (left, a) = imported(&a);
    let (right, b) = imported(&b);
    let (changed, c) = imported(&c);
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Imported(&a, &b)],
                &mut work
            )
            .unwrap(),
        None
    );
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &changed,
                [PreparedEffectReplayRoot::Imported(&a, &c)],
                &mut work
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::ImportOrigin)
    );
}

#[test]
fn substitution_and_boolean_predicates_retain_all_prepared_ports() {
    use crate::effect_row::EffectFormula;
    use crate::types::{GenericBinder, GenericScope};
    fn prepared(
        graph: &mut PreparedEffectGraph,
        negate: bool,
    ) -> (PreparedEffectDelta, PreparedEffectPredicate) {
        let mut work = budget();
        let mut checkpoint = graph.checkpoint(&mut work).unwrap();
        let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
        let formal = scope.bound_effect(0, 0).unwrap();
        let formula = EffectFormula::variable(formal.clone(), &mut work).unwrap();
        let predicate = formula.subset(&EffectFormula::empty(), &mut work).unwrap();
        let actual = literal(&["log.write"]);
        let arguments = BTreeMap::from([(formal, actual)]);
        let row = graph
            .apply_formula(formula, arguments.clone(), &mut work)
            .unwrap();
        let a = graph
            .apply_predicate(predicate, arguments, &mut work)
            .unwrap();
        let b = graph
            .relate(
                PreparedEffectRelation::AdmittedSubset,
                row,
                literal(&["log.write"]),
                &mut work,
            )
            .unwrap();
        let and = graph.and(vec![a.clone(), b.clone()], &mut work).unwrap();
        let or = graph.or(vec![a, b], &mut work).unwrap();
        let not = graph.not(or.clone(), &mut work).unwrap();
        let choice = graph.choice(&mut work).unwrap();
        let result = graph
            .select_predicate(&choice, and, if negate { not } else { or }, &mut work)
            .unwrap();
        (
            graph.extract_delta(&mut checkpoint, &mut work).unwrap(),
            result,
        )
    }
    let mut graph = PreparedEffectGraph::default();
    let (left, a) = prepared(&mut graph, false);
    let (right, b) = prepared(&mut graph, false);
    let (changed, c) = prepared(&mut graph, true);
    let mut work = budget();
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &right,
                [PreparedEffectReplayRoot::Predicate(&a, &b)],
                &mut work
            )
            .unwrap(),
        None
    );
    assert_eq!(
        graph
            .replay_delta_mismatch(
                &left,
                &changed,
                [PreparedEffectReplayRoot::Predicate(&a, &c)],
                &mut work
            )
            .unwrap(),
        Some(PreparedEffectReplayMismatch::Predicate)
    );
}
