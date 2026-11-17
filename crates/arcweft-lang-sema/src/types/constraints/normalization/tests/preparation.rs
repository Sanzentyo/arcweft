use super::*;
use crate::{
    effect_row::{
        EffectPredicate,
        prepared::{PreparedEffectGraph, PreparedEffectRow},
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
