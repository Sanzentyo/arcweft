use super::*;
use crate::{
    effect_row::{
        EffectPredicate,
        prepared::{PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow},
    },
    types::{PreparedEffects, constraints::transaction::ConstraintOperand},
};
use std::sync::Arc;

#[test]
fn introduced_type_relations_prepare_bindings_without_closing_unrelated_bodies() {
    for eligibility in [
        TypeConstraintParameterEligibility::Bindable,
        TypeConstraintParameterEligibility::Rigid,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let parameter = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_901)),
            0,
        );
        let scope = TypeConstraintParameterScope::new([(parameter.clone(), eligibility)]).unwrap();
        let (mut context, path) = context(scope, &cancellation, 4096).into_path(&mut graph);
        let key = path
            .root_scope()
            .parameters()
            .type_reference(&parameter.into())
            .unwrap();
        let owner = path
            .applications
            .owner_for_parameter(&key.clone().into())
            .unwrap();
        let pattern = ConstraintOperand::introduced_type(
            &key,
            Arc::new(path.lineage.scope.clone()),
            owner,
            &mut context,
        )
        .unwrap();
        let actual = context.fixture_operand(TypeKind::<PreparedEffects>::Bool);
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let paths = crate::types::compatibility::binding_plan::relate_selected_call(
            &mut graph,
            &pattern.view(),
            &actual.view(),
            path,
            &mut context,
            ConstraintAcceptance::PatternAcceptsActual,
        )
        .unwrap();
        if eligibility == TypeConstraintParameterEligibility::Bindable {
            assert_eq!(paths.len(), 1);
            assert_eq!(
                paths[0].bindings.get(&key).unwrap().value(),
                &TypeKind::Bool
            );
        } else {
            assert!(paths.is_empty());
        }
        graph
            .prepare_body(
                &named,
                PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
                vec![],
                &mut context,
            )
            .unwrap();
    }
}

#[test]
fn an_alias_uses_the_resolved_relation_without_widening_or_early_closure() {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let parameter = GenericTypeParameterId::new(
        GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_902)),
        0,
    );
    let scope = TypeConstraintParameterScope::new([(
        parameter.clone(),
        TypeConstraintParameterEligibility::Bindable,
    )])
    .unwrap();
    let (mut context, path) = context(scope, &cancellation, 4096).into_path(&mut graph);
    let key = path
        .root_scope()
        .parameters()
        .type_reference(&parameter.into())
        .unwrap();
    let owner = path
        .applications
        .owner_for_parameter(&key.clone().into())
        .unwrap();
    let pattern = ConstraintOperand::introduced_type(
        &key,
        Arc::new(path.lineage.scope.clone()),
        owner,
        &mut context,
    )
    .unwrap();
    let actual = context.fixture_operand(TypeKind::<PreparedEffects>::Bool);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let mut paths = crate::types::compatibility::binding_plan::relate_selected_call(
        &mut graph,
        &pattern.view(),
        &actual.view(),
        path,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .unwrap();
    let path = paths.pop().unwrap();
    assert!(paths.is_empty());
    let mismatch = context.fixture_operand(TypeKind::<PreparedEffects>::I64);
    let paths = crate::types::compatibility::binding_plan::relate_selected_call(
        &mut graph,
        &pattern.view(),
        &mismatch.view(),
        path,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .unwrap();
    assert!(paths.is_empty());
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![],
            &mut context,
        )
        .unwrap();
}

fn function(argument: TypeKind<PreparedEffects>, labels: &[&str]) -> TypeKind<PreparedEffects> {
    TypeKind::function_with_contract(
        GenericBinder::EMPTY,
        PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
        [argument],
        TypeKind::I64,
        PreparedEffectRow::Canonical(EffectRow::closed(
            EffectSet::from_labels(labels.iter().copied()).unwrap(),
        )),
    )
}

/// Exercise both phases using the same original graph, source scope and work.
fn completed_relation(
    expected: TypeKind<PreparedEffects>,
    actual: TypeKind<PreparedEffects>,
) -> bool {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let setup: TestContext<'_> = ConstraintTestSetup::new(
        TypeConstraintLimits::new(131_072, 65_536, 1024, 1024),
        &cancellation,
    );
    let (mut context, path) = setup.into_path(&mut graph);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let expected = context.fixture_operand(expected);
    let actual = context.fixture_operand(actual);
    let mut paths = crate::types::compatibility::binding_plan::relate_selected_call(
        &mut graph,
        &expected.view(),
        &actual.view(),
        path,
        &mut context,
        ConstraintAcceptance::PatternAcceptsActual,
    )
    .unwrap();
    if paths.is_empty() {
        graph
            .prepare_body(
                &named,
                PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
                vec![],
                &mut context,
            )
            .unwrap();
        return false;
    }
    assert_eq!(paths.len(), 1);
    let path = paths.pop().unwrap();
    let receipt = path
        .prepare_effect_constraint(&mut graph, &mut context)
        .unwrap();
    let original = graph
        .read()
        .freeze_scoped(&[], &[], &[&receipt], &mut context)
        .unwrap();
    let original = graph
        .bind_original_scoped(&original, &mut context)
        .unwrap()
        .freeze(graph.read(), &mut context)
        .unwrap();
    // Preparation must leave the real Named body unavailable, then final
    // closure reads this actual body without preparing the source a second time.
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(EffectRow::closed(EffectSet::new())),
            vec![],
            &mut context,
        )
        .unwrap();
    match super::super::ConstraintClosedEffects::for_path(
        &path,
        &original,
        &[],
        &mut graph,
        &mut context,
    ) {
        Ok(effects) => {
            assert!(effects.predicate().value().is_unconstrained());
            true
        }
        Err(TypeConstraintError::Rejected(TypeConstraintRejection::EffectUnsatisfiable)) => false,
        Err(error) => panic!("unexpected final relation error: {error:?}"),
    }
}

#[test]
fn function_preparation_keeps_the_effect_condition_for_original_closure() {
    assert!(completed_relation(
        function(TypeKind::Bool, &["log.write"]),
        function(TypeKind::Bool, &[])
    ));
    assert!(!completed_relation(
        function(TypeKind::Bool, &[]),
        function(TypeKind::Bool, &["log.write"])
    ));
}

#[test]
fn function_preparation_preserves_contravariance_and_bottom_direction() {
    assert!(completed_relation(
        function(TypeKind::Never, &[]),
        function(TypeKind::Bool, &[])
    ));
    assert!(!completed_relation(
        function(TypeKind::Bool, &[]),
        function(TypeKind::Never, &[])
    ));
}

#[test]
fn nominal_function_arguments_remain_invariant_until_final_closure() {
    use crate::types::AcceptedNominalType;
    let envelope = |argument| {
        TypeKind::AcceptedNominal(AcceptedNominalType::new(
            crate::env::nominal::standard_nominal_id("PreparedEnvelope"),
            vec![argument],
        ))
    };
    assert!(!completed_relation(
        envelope(function(TypeKind::Bool, &["log.write"])),
        envelope(function(TypeKind::Bool, &[]))
    ));
    assert!(!completed_relation(
        envelope(function(TypeKind::Bool, &[])),
        envelope(function(TypeKind::Bool, &["log.write"]))
    ));
    assert!(completed_relation(
        envelope(function(TypeKind::Bool, &[])),
        envelope(function(TypeKind::Bool, &[]))
    ));
}
