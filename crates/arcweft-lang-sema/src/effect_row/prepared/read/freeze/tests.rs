use super::*;
mod scoped_constraints;
use crate::effect_row::prepared::test_support::{Budget, Exhausted, budget, literal};

fn truth() -> PreparedEffectPredicate {
    PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained())
}

#[test]
fn constant_completion_keeps_the_receipt_and_respects_live_obligations() {
    let mut graph = PreparedEffectGraph::default();
    let mut work = budget();
    let fact = graph
        .read()
        .scoped_constraint(&[], &truth(), &truth(), &mut work)
        .unwrap();
    let proof = graph
        .read()
        .close_constant_product(&[], &[], &[&fact], &[], &mut work)
        .unwrap()
        .expect("constant implication");
    assert!(
        proof
            .into_principal()
            .confirm_scoped_constraint(&fact, &mut Budget(1))
            .unwrap()
            .is_unconstrained()
    );
    assert!(matches!(
        graph
            .read()
            .close_constant_product(&[], &[], &[&fact], &[], &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
    let foreign = PreparedEffectGraph::default();
    assert!(matches!(
        foreign
            .read()
            .close_constant_product(&[], &[], &[&fact], &[], &mut work),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    graph
        .add_scoped_obligation(
            vec![],
            truth(),
            PreparedEffectPredicate::Canonical(EffectPredicate::impossible()),
            &mut work,
        )
        .unwrap();
    assert!(
        graph
            .read()
            .close_constant_product(&[], &[], &[&fact], &[], &mut work)
            .unwrap()
            .is_none()
    );
}

fn constrained_named_value(
    graph: &mut PreparedEffectGraph,
    pure: bool,
) -> (
    PreparedEffectSchemeRef,
    PreparedEffectUnknownRef,
    PreparedEffectRow,
) {
    let mut work = budget();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut work)
        .unwrap();
    let (output, row) = constrain_named_output(graph, &named, pure);
    (named, output, row)
}

fn constrain_named_output(
    graph: &mut PreparedEffectGraph,
    named: &PreparedEffectSchemeRef,
    pure: bool,
) -> (PreparedEffectUnknownRef, PreparedEffectRow) {
    let mut work = budget();
    let output = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut work,
        )
        .unwrap();
    let actual = graph.application(named, vec![], &mut work).unwrap();
    let local = graph
        .unknown_application(&output, vec![], &mut work)
        .unwrap();
    let equality = graph
        .relate(
            PreparedEffectRelation::Equal,
            local.clone(),
            actual.clone(),
            &mut work,
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equality, &mut work)
        .unwrap();
    if pure {
        let guard = graph
            .relate(
                PreparedEffectRelation::AdmittedSubset,
                actual,
                literal(&[]),
                &mut work,
            )
            .unwrap();
        graph
            .add_scoped_obligation(vec![], truth(), guard, &mut work)
            .unwrap();
    }
    (output, local)
}

#[test]
fn freezing_a_named_value_keeps_a_scheme_input_before_and_after_its_body() {
    let mut graph = PreparedEffectGraph::default();
    let (named, output, row) = constrained_named_value(&mut graph, false);
    let before = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    let original = graph.declaration(&named, &mut budget()).unwrap();
    graph
        .prepare_body(&named, literal(&["log.write"]), vec![], &mut budget())
        .unwrap();
    // An unreachable body allocation must not change the frozen label basis.
    graph
        .union(
            vec![literal(&["fs.read"]), literal(&["net.send"])],
            &mut budget(),
        )
        .unwrap();
    let after = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    assert_eq!(before.support, after.support);
    assert_eq!(before.graph.rows.len(), after.graph.rows.len());
    assert!(
        before
            .graph
            .same_original(&after.graph, &mut budget())
            .unwrap()
    );
    for frozen in [before, after] {
        assert!(
            matches!(frozen.scheme(&named).unwrap().residual().function.role().unwrap(),
            ResidualFunctionRole::SchemeInput { declaration, .. } if declaration == &original)
        );
        let reduced = frozen.reduce(&mut budget()).unwrap();
        let value = reduced.unknown(&output).unwrap();
        let membership = value.membership().unwrap();
        let variables: Vec<_> = membership.default_decision().variables().collect();
        assert_eq!(variables.len(), 1);
        assert!(
            matches!(value.function(variables[0]).unwrap().role().unwrap(),
            ResidualFunctionRole::SchemeInput { declaration, .. } if declaration == &original)
        );
        let imported = graph
            .import_guarded_function(&frozen.unknown(&output).unwrap(), &[], &mut budget())
            .unwrap();
        let row = graph
            .imported_application(&imported, vec![], &mut budget())
            .unwrap();
        let closed = graph.solve_principal(&mut budget()).unwrap();
        assert_eq!(
            closed.resolve_row(&row, &mut budget()).unwrap(),
            EffectRow::closed(crate::effects::EffectSet::from_labels(["log.write"]).unwrap())
        );
    }
}

#[test]
fn shared_snapshot_roots_retain_one_definition_per_dependency() {
    const ROOT_COUNT: usize = 8;
    let mut graph = PreparedEffectGraph::default();
    let (_, _, row) = constrained_named_value(&mut graph, false);
    let predicate = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row.clone(),
            literal(&[]),
            &mut budget(),
        )
        .unwrap();
    let read = graph.read();
    let mut measured = budget();
    let start = measured.0;
    let single = read
        .snapshot(
            &[&row],
            &[&predicate],
            &[],
            SnapshotSource::Product(&[]),
            &mut measured,
        )
        .unwrap();
    assert!(!single.rows.is_empty() && !single.predicates.is_empty());

    let shared = read
        .snapshot(
            &[&row; ROOT_COUNT],
            &[&predicate; ROOT_COUNT],
            &[],
            SnapshotSource::Product(&[]),
            &mut Budget(start - measured.0 + 2 * (ROOT_COUNT - 1)),
        )
        .unwrap();
    assert!(single.same_original(&shared, &mut budget()).unwrap());
}

#[test]
fn original_relation_identity_distinguishes_new_constraints_on_the_same_issuer() {
    let mut graph = PreparedEffectGraph::default();
    let (_, _, row) = constrained_named_value(&mut graph, false);
    let before = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    let again = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    assert!(
        before
            .graph
            .same_original(&again.graph, &mut budget())
            .unwrap()
    );
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let guard = graph
        .relate(
            PreparedEffectRelation::AdmittedSubset,
            row.clone(),
            literal(&[]),
            &mut budget(),
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), guard, &mut budget())
        .unwrap();
    let changed = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    assert!(
        !before
            .graph
            .same_original(&changed.graph, &mut budget())
            .unwrap()
    );
    graph.rollback(&mut checkpoint).unwrap();
    let restored = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    assert!(
        before
            .graph
            .same_original(&restored.graph, &mut budget())
            .unwrap()
    );
    assert!(matches!(
        before.graph.same_original(&again.graph, &mut Budget(0)),
        Err(PreparedEffectFailure::Control(Exhausted))
    ));
}

#[test]
fn a_pure_callback_retains_a_guard_until_the_real_named_body_is_closed() {
    for body in [literal(&[]), literal(&["log.write"])] {
        let pure = matches!(&body, PreparedEffectRow::Canonical(row) if row.is_empty());
        let mut graph = PreparedEffectGraph::default();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
            .unwrap();
        let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
        let (output, row) = constrain_named_output(&mut graph, &named, true);
        let delta = graph.extract_delta(&mut checkpoint, &mut budget()).unwrap();
        let scope = graph.read_scope([&delta], &mut budget()).unwrap();
        let frozen = {
            let edit = graph.edit_scope(&scope, &mut budget()).unwrap();
            edit.read()
                .freeze_scoped(&[&row], &[], &[], &mut budget())
                .unwrap()
        };
        assert!(graph.relation.obligations.is_empty());
        assert_ne!(
            frozen
                .reduce(&mut budget())
                .unwrap()
                .principal()
                .truth_value(),
            Some(true)
        );
        let imported = graph
            .import_guarded_function(&frozen.unknown(&output).unwrap(), &[], &mut budget())
            .unwrap();
        let row = graph
            .imported_application(&imported, vec![], &mut budget())
            .unwrap();
        // Use the guarded value in the actual recursive body. Merely allocating
        // an unobserved guarded row does not require its principal condition.
        let body = graph.union(vec![row.clone(), body], &mut budget()).unwrap();
        graph
            .prepare_body(&named, body, vec![], &mut budget())
            .unwrap();
        if pure {
            let closed = graph.solve_principal(&mut budget()).unwrap();
            assert!(closed.resolve_row(&row, &mut budget()).unwrap().is_empty());
        } else {
            assert!(matches!(
                graph.solve_principal(&mut budget()),
                Err(PreparedEffectFailure::Unsatisfiable)
            ));
        }
    }
}

#[test]
fn named_and_future_inputs_share_one_import_without_minimizing_either_input() {
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut budget())
        .unwrap();
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let (output, local) = constrain_named_output(&mut graph, &named, false);
    let future = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Future,
            &mut budget(),
        )
        .unwrap();
    let future_row = graph
        .unknown_application(&future, vec![], &mut budget())
        .unwrap();
    let combined = graph.union(vec![local, future_row], &mut budget()).unwrap();
    let result = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut budget(),
        )
        .unwrap();
    let result_row = graph
        .unknown_application(&result, vec![], &mut budget())
        .unwrap();
    let equality = graph
        .relate(
            PreparedEffectRelation::Equal,
            result_row.clone(),
            combined,
            &mut budget(),
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equality, &mut budget())
        .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(&[&result_row], &[], &[], &mut budget())
        .unwrap();
    graph.rollback(&mut checkpoint).unwrap();
    graph
        .prepare_body(&named, literal(&["log.write"]), vec![], &mut budget())
        .unwrap();
    let actual = graph
        .declare_unknown(
            vec![],
            PreparedEffectUnknownEligibility::Bindable,
            &mut budget(),
        )
        .unwrap();
    let actual_row = graph
        .unknown_application(&actual, vec![], &mut budget())
        .unwrap();
    let equality = graph
        .relate(
            PreparedEffectRelation::Equal,
            actual_row,
            literal(&["fs.read"]),
            &mut budget(),
        )
        .unwrap();
    graph
        .add_scoped_obligation(vec![], truth(), equality, &mut budget())
        .unwrap();
    let imported = graph
        .import_guarded_function(
            &frozen.unknown(&result).unwrap(),
            &[(&future, &actual)],
            &mut budget(),
        )
        .unwrap();
    let row = graph
        .imported_application(&imported, vec![], &mut budget())
        .unwrap();
    let closed = graph.solve_principal(&mut budget()).unwrap();
    assert_eq!(
        closed.resolve_row(&row, &mut budget()).unwrap(),
        EffectRow::closed(
            crate::effects::EffectSet::from_labels(["log.write", "fs.read"]).unwrap()
        )
    );
    assert_eq!(
        frozen
            .unknown(&output)
            .unwrap()
            .residual()
            .function
            .parameters(),
        &[]
    );
}

#[test]
fn freezing_reserves_work_and_leaves_the_original_checkpoint_usable_at_every_failure() {
    let mut graph = PreparedEffectGraph::default();
    let (_, _, row) = constrained_named_value(&mut graph, false);
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let state = graph.state;
    let terms = graph.retained_terms();
    let mut measured = Budget(10_000);
    graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut measured)
        .unwrap();
    let required = 10_000 - measured.0;
    assert!(required > 0);
    for available in 0..required {
        assert!(matches!(
            graph
                .read()
                .freeze_scoped(&[&row], &[], &[], &mut Budget(available)),
            Err(PreparedEffectFailure::Control(Exhausted))
        ));
        assert_eq!(graph.state, state);
        assert_eq!(graph.retained_terms(), terms);
    }
    graph.rollback(&mut checkpoint).unwrap();
}

#[test]
fn freezing_an_extracted_relation_retains_all_constraints_without_publishing_its_delta() {
    let mut graph = PreparedEffectGraph::default();
    let mut checkpoint = graph.checkpoint(&mut budget()).unwrap();
    let (named, output, row) = constrained_named_value(&mut graph, true);
    let delta = graph.extract_delta(&mut checkpoint, &mut budget()).unwrap();
    let state = graph.state;
    let read = graph.read_extracted(&delta, &mut budget()).unwrap();
    assert!(
        read.close_constant_product(&[], &[], &[], &[], &mut budget())
            .unwrap()
            .is_none()
    );
    let frozen = read.freeze_scoped(&[], &[], &[], &mut budget()).unwrap();
    assert_eq!(frozen.graph.obligations.len(), 2);
    assert_eq!(graph.state, state);
    assert_eq!(graph.retained_terms(), (0, 0));
    assert!(matches!(
        graph.import_guarded_function(&frozen.unknown(&output).unwrap(), &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::StaleReference
        ))
    ));
    let mut foreign = PreparedEffectGraph::default();
    assert!(matches!(
        foreign.import_guarded_function(&frozen.unknown(&output).unwrap(), &[], &mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::ForeignIssuer
        ))
    ));
    graph.restore_delta(delta, &mut budget()).unwrap();
    graph
        .prepare_body(&named, literal(&["log.write"]), vec![], &mut budget())
        .unwrap();
    graph
        .import_guarded_function(&frozen.unknown(&output).unwrap(), &[], &mut budget())
        .unwrap();
    graph.read().validate_retained_row(&row).unwrap();
    assert!(matches!(
        graph.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Unsatisfiable)
    ));
}

#[test]
fn a_frozen_named_input_retains_unused_formals_and_the_original_domain() {
    use crate::types::{GenericBinder, GenericScope};
    let scope = GenericScope::default().with_binder(GenericBinder::new(0, 0, 2));
    let parameters = vec![
        scope.bound_effect(0, 0).unwrap(),
        scope.bound_effect(0, 1).unwrap(),
    ];
    let domain = EffectFormula::literal(
        crate::effects::EffectSet::new(),
        Some(parameters[0].clone()),
    )
    .subset(
        &EffectFormula::literal(
            crate::effects::EffectSet::from_labels(["log.write"]).unwrap(),
            None,
        ),
        &mut budget(),
    )
    .unwrap();
    let mut graph = PreparedEffectGraph::default();
    let named = graph
        .declare(parameters.clone(), domain.clone(), &mut budget())
        .unwrap();
    let original = graph.declaration(&named, &mut budget()).unwrap();
    let row = graph
        .application(
            &named,
            vec![literal(&[]), literal(&["net.send"])],
            &mut budget(),
        )
        .unwrap();
    let frozen = graph
        .read()
        .freeze_scoped(&[&row], &[], &[], &mut budget())
        .unwrap();
    let guarded = frozen.scheme(&named).unwrap();
    let ResidualFunctionRole::SchemeInput { declaration, .. } =
        guarded.residual().function.role().unwrap()
    else {
        panic!("named input was replaced")
    };
    assert_eq!(declaration, &original);
    assert_eq!(declaration.parameters(), &parameters);
    assert_eq!(declaration.domain(), &domain);
    assert!(matches!(
        graph.solve_principal(&mut budget()),
        Err(PreparedEffectFailure::Invariant(
            PreparedEffectInvariant::SchemeState
        ))
    ));
}
