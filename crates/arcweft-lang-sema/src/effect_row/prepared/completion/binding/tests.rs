use super::*;
use crate::effect_row::prepared::test_support::*;
use crate::types::{GenericBinder, GenericScope};

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

fn unknown(
    graph: &mut PreparedEffectGraph,
    future: bool,
    parameters: Vec<GenericEffectReference>,
) -> PreparedEffectUnknownRef {
    graph
        .declare_unknown(
            parameters,
            if future {
                PreparedEffectUnknownEligibility::Future
            } else {
                PreparedEffectUnknownEligibility::Bindable
            },
            &mut budget(),
        )
        .unwrap()
}

fn freeze(graph: &PreparedEffectGraph) -> Arc<ScopedEffectCompletion> {
    graph
        .read()
        .freeze_scoped(&[], &[], &[], &mut budget())
        .unwrap()
}

fn publish(graph: &mut PreparedEffectGraph, binding: &PreparedEffectScopedBinding) {
    for constraint in std::iter::once(&binding.principal).chain(&binding.agreements) {
        let obligation = &constraint.obligation;
        graph
            .add_scoped_obligation(
                obligation.parameters.to_vec(),
                obligation.domain.clone(),
                obligation.predicate.clone(),
                &mut budget(),
            )
            .unwrap();
    }
}

fn equation(
    graph: &mut PreparedEffectGraph,
    left: PreparedEffectRow,
    right: PreparedEffectRow,
    parameters: Vec<GenericEffectReference>,
) {
    let predicate = graph
        .relate(PreparedEffectRelation::Equal, left, right, &mut budget())
        .unwrap();
    graph
        .add_scoped_obligation(parameters, truth(), predicate, &mut budget())
        .unwrap();
}

#[test]
fn a_source_without_outputs_keeps_its_principal_condition_as_a_candidate_fact() {
    for admitted in [false, true] {
        let mut graph = PreparedEffectGraph::default();
        let predicate = PreparedEffectPredicate::Canonical(if admitted {
            EffectPredicate::unconstrained()
        } else {
            EffectPredicate::impossible()
        });
        let receipt = graph
            .read()
            .scoped_constraint(&[], &truth(), &predicate, &mut budget())
            .unwrap();
        let source = graph
            .read()
            .freeze_scoped(&[], &[], &[&receipt], &mut budget())
            .unwrap();
        let binding = graph.bind_original_scoped(&source, &mut budget()).unwrap();
        assert!(Arc::ptr_eq(&binding.relation.relation.completion, &source));
        assert!(binding.agreements.is_empty());
        assert!(graph.relation.functions.is_empty());
        // Candidate facts have not constrained a sibling or the live graph.
        assert!(graph.relation.obligations.is_empty());
        assert!(graph.solve_principal(&mut budget()).is_ok());
        publish(&mut graph, &binding);
        match graph.solve_principal(&mut budget()) {
            Ok(_) => assert!(admitted),
            Err(PreparedEffectFailure::Unsatisfiable) => assert!(!admitted),
            Err(error) => panic!("unexpected binding result: {error:?}"),
        }
    }
}

#[test]
fn freezing_an_input_only_relation_retains_its_original_constraint_once() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let future = unknown(&mut graph, true, vec![]);
    let value = graph
        .unknown_application(&future, vec![], &mut work)
        .unwrap();
    let expected =
        EffectRow::closed(crate::effects::EffectSet::from_labels(["log.write"]).unwrap());
    let condition = graph
        .relate(
            PreparedEffectRelation::Equal,
            value,
            PreparedEffectRow::Canonical(expected.clone()),
            &mut work,
        )
        .unwrap();
    let constraint = graph
        .read()
        .scoped_constraint(&[], &truth(), &condition, &mut work)
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[], &[], &[&constraint], &mut work)
        .unwrap();
    let bound = graph
        .bind_original_scoped(&original, &mut work)
        .unwrap()
        .freeze(graph.read(), &mut work)
        .unwrap();

    let [retained] = bound.definitions().obligations.as_slice() else {
        panic!("the original obligation is retained once");
    };
    assert!(Arc::ptr_eq(retained, &constraint.obligation));
    assert!(
        bound
            .close_original(graph.read(), &[], &[], &[(&future, &expected)], &mut work)
            .is_ok()
    );
    let empty = EffectRow::closed(crate::effects::EffectSet::new());
    assert!(matches!(
        bound.close_original(graph.read(), &[], &[], &[(&future, &empty)], &mut work),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn local_function_identity_is_tied_to_the_least_tuple_not_just_a_satisfying_tuple() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let value = graph
        .unknown_application(&x, vec![], &mut budget())
        .unwrap();
    let binding = graph
        .bind_original_scoped(&freeze(&graph), &mut budget())
        .unwrap();
    publish(&mut graph, &binding);
    // The original unconstrained function's least solution is pure. A later
    // consumer cannot widen the same original identity to satisfy its demand.
    equation(&mut graph, value, literal(&["log.write"]), vec![]);
    assert!(matches!(
        graph.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn every_declared_argument_is_quantified_even_when_the_source_never_applies_it() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let parameters = [parameter(&scope, 0), parameter(&scope, 1)];
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, parameters.to_vec());
    let binding = graph
        .bind_original_scoped(&freeze(&graph), &mut budget())
        .unwrap();
    assert_eq!(binding.agreements.len(), 1);
    assert_eq!(
        binding.agreements[0].obligation.parameters.as_ref(),
        &parameters
    );
    publish(&mut graph, &binding);
    let value = graph
        .unknown_application(
            &x,
            vec![literal(&[]), literal(&["outside.label"])],
            &mut budget(),
        )
        .unwrap();
    equation(&mut graph, value, literal(&["outside.label"]), vec![]);
    assert!(matches!(
        graph.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn a_named_override_is_not_a_quantified_function_argument() {
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 1));
    let effect = parameter(&scope, 0);
    let rigid = open(&effect);
    for augmented in [false, true] {
        let mut graph = PreparedEffectGraph::default();
        let child = unknown(&mut graph, false, vec![effect.clone()]);
        let child_row = graph
            .unknown_application(&child, vec![rigid.clone()], &mut budget())
            .unwrap();
        equation(&mut graph, child_row, rigid.clone(), vec![effect.clone()]);
        let original = freeze(&graph);
        let imported = graph
            .import_guarded_function(&original.unknown(&child).unwrap(), &[], &mut budget())
            .unwrap();
        let selected = graph
            .imported_application(&imported, vec![rigid.clone()], &mut budget())
            .unwrap();

        let local = unknown(&mut graph, false, vec![effect.clone()]);
        let argument = if augmented {
            PreparedEffectRow::Canonical(EffectRow::open(
                crate::effects::EffectSet::from_labels(["log.write"]).unwrap(),
                effect.clone(),
            ))
        } else {
            rigid.clone()
        };
        let actual = graph
            .unknown_application(&local, vec![argument], &mut budget())
            .unwrap();
        equation(&mut graph, actual, selected, vec![effect.clone()]);
        match graph.solve_principal(&mut budget()) {
            Ok(closure) if !augmented => {
                assert_eq!(
                    closure
                        .unknown(&local)
                        .unwrap()
                        .known()
                        .unwrap()
                        .single_reference(),
                    Some(&effect)
                );
            }
            Err(PreparedEffectFailure::Unsatisfiable) if augmented => {}
            outcome => panic!(
                "unexpected quantified function agreement: {}",
                if outcome.is_ok() {
                    "accepted"
                } else {
                    "rejected"
                }
            ),
        }
    }
}

#[test]
fn an_original_future_stays_the_same_input_until_its_actual_row_is_supplied() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let future = unknown(&mut graph, true, vec![]);
    let left = graph
        .unknown_application(&x, vec![], &mut budget())
        .unwrap();
    let right = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    equation(&mut graph, left, right, vec![]);
    let binding = graph
        .bind_original_scoped(&freeze(&graph), &mut budget())
        .unwrap();
    assert_eq!(binding.relation.relation.inputs.len(), 1);
    assert_eq!(
        binding
            .relation
            .relation
            .inputs
            .values()
            .next()
            .unwrap()
            .reference(),
        future.reference()
    );
    publish(&mut graph, &binding);
    assert!(matches!(
        graph.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::UnresolvedFuture
        ))
    ));
    let actual = match literal(&["late.label"]) {
        PreparedEffectRow::Canonical(row) => row,
        _ => unreachable!(),
    };
    let closed = graph
        .specialize_principal(&[(&future, &actual)], &mut budget())
        .unwrap();
    assert_eq!(closed.unknown(&x).unwrap(), &actual);
}

#[test]
fn a_foreign_stale_or_specialized_snapshot_cannot_impersonate_the_live_original() {
    let mut graph = PreparedEffectGraph::default();
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let future = unknown(&mut graph, true, vec![]);
    let source = freeze(&graph);
    let mut other = PreparedEffectGraph::default();
    assert!(matches!(
        other.bind_original_scoped(&source, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let actual = EffectRow::closed(crate::effects::EffectSet::new());
    let fixed = source
        .specialize_partial(&[(&future, &actual)], &mut budget())
        .unwrap();
    assert!(matches!(
        graph.bind_original_scoped(&fixed, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ImportedRelationOrigin
        ))
    ));
    graph.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        graph.bind_original_scoped(&source, &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    assert!(graph.relation.functions.is_empty());
    assert_eq!(graph.retained_terms(), (0, 0));
}

#[test]
fn every_exhausted_binding_rolls_back_terms_functions_and_candidate_facts() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let future = unknown(&mut graph, true, vec![]);
    let left = graph
        .unknown_application(&x, vec![], &mut budget())
        .unwrap();
    let right = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    equation(&mut graph, left, right, vec![]);
    let source = freeze(&graph);
    let before = (
        graph.retained_terms(),
        graph.relation.functions.len(),
        graph.relation.obligations.len(),
        graph.state,
    );
    let mut outer = graph.checkpoint(&mut budget()).unwrap();
    let mut measured = Budget(10_000);
    graph.bind_original_scoped(&source, &mut measured).unwrap();
    let needed = 10_000 - measured.0;
    graph.rollback(&mut outer).unwrap();
    for available in 0..needed {
        let mut work = Budget(available);
        assert!(matches!(
            graph.bind_original_scoped(&source, &mut work),
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
        assert!(graph.checkpoints.is_empty());
    }
}

#[test]
fn bound_freeze_preserves_original_query_ports_and_keeps_its_facts_out_of_siblings() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let value = graph
        .unknown_application(&x, vec![], &mut budget())
        .unwrap();
    let query = graph
        .union(
            vec![value.clone(), literal(&["query.label"])],
            &mut budget(),
        )
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[&query], &[], &[], &mut budget())
        .unwrap();
    let binding = graph
        .bind_original_scoped(&original, &mut budget())
        .unwrap();
    let later_future = unknown(&mut graph, true, vec![]);
    equation(&mut graph, value, literal(&["later.consumer"]), vec![]);
    let bound = binding.freeze(graph.read(), &mut budget()).unwrap();
    let closed = bound
        .definitions()
        .specialize_principal(&[], &mut budget())
        .unwrap();
    assert_eq!(
        closed.resolve_row(&query, &mut budget()).unwrap(),
        match literal(&["query.label"]) {
            PreparedEffectRow::Canonical(row) => row,
            _ => unreachable!(),
        }
    );
    assert_eq!(graph.relation.obligations.len(), 1);
    assert_eq!(
        closed.unknown(&later_future),
        Err(PreparedEffectInvariant::StaleReference)
    );
    // This sibling never received the selected candidate's receipts.
    assert!(
        graph
            .specialize_principal(
                &[(
                    &later_future,
                    &EffectRow::closed(crate::effects::EffectSet::new())
                )],
                &mut budget()
            )
            .is_ok()
    );
    assert_eq!(
        closed.unknown(&x).unwrap(),
        &EffectRow::closed(crate::effects::EffectSet::new())
    );
}

#[test]
fn bound_freeze_rejects_foreign_or_rolled_back_ports_and_spends_work_on_each_prefix() {
    let mut graph = PreparedEffectGraph::default();
    let x = unknown(&mut graph, false, vec![]);
    let query = graph
        .unknown_application(&x, vec![], &mut budget())
        .unwrap();
    let source = graph
        .read()
        .freeze_scoped(&[&query], &[], &[], &mut budget())
        .unwrap();
    let mut outer = graph.checkpoint(&mut budget()).unwrap();
    let binding = graph.bind_original_scoped(&source, &mut budget()).unwrap();
    let mut measured = Budget(10_000);
    let bound = binding.freeze(graph.read(), &mut measured).unwrap();
    let needed = 10_000 - measured.0;
    assert!(
        bound
            .definitions()
            .specialize_principal(&[], &mut budget())
            .is_ok()
    );
    graph.rollback(&mut outer).unwrap();
    for available in 0..needed {
        let mut outer = graph.checkpoint(&mut budget()).unwrap();
        let binding = graph.bind_original_scoped(&source, &mut budget()).unwrap();
        let before = (
            graph.retained_terms(),
            graph.relation.functions.len(),
            graph.state,
        );
        let mut work = Budget(available);
        assert!(matches!(
            binding.freeze(graph.read(), &mut work),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(work.0, 0);
        assert_eq!(
            (
                graph.retained_terms(),
                graph.relation.functions.len(),
                graph.state
            ),
            before
        );
        graph.rollback(&mut outer).unwrap();
    }
    let binding = graph.bind_original_scoped(&source, &mut budget()).unwrap();
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        binding.freeze(foreign.read(), &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let binding = graph.bind_original_scoped(&source, &mut budget()).unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    assert!(matches!(
        binding.freeze(graph.read(), &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
}
