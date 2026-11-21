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
            Arc::new(path.scope.clone()),
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
    let pattern =
        ConstraintOperand::introduced_type(&key, Arc::new(path.scope.clone()), owner, &mut context)
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

#[test]
fn closed_scalar_expectations_infer_real_template_byte_and_agent_elements() {
    use crate::types::constraints::transaction::TypeConstraintTransaction;
    for bytes in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let parameter = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_920)),
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
            .type_reference(&parameter.clone().into())
            .unwrap();
        let application = path.root_id();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let mut transaction =
            TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
                .unwrap();
        transaction.constrain_application_template(
            &mut context,
            &mut graph,
            &if bytes {
                TypeKind::Bytes
            } else {
                TypeKind::AgentValue
            },
            &TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter.into()))),
            ConstraintAcceptance::PatternAcceptsActual,
        );
        let path = transaction.test_single_path();
        assert_eq!(
            path.bindings.get(&key).unwrap().value(),
            &if bytes {
                TypeKind::U8
            } else {
                TypeKind::AgentValue
            }
        );
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
fn agent_choice_membership_retains_real_equation_keys_in_both_directions() {
    use crate::types::constraints::transaction::{ChoiceForkRole, TypeConstraintTransaction};
    for direction in [
        ConstraintAcceptance::PatternAcceptsActual,
        ConstraintAcceptance::ActualAcceptsPattern,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let (mut context, path) = context(empty_scope(), &cancellation, 4096).into_path(&mut graph);
        let application = path.root_id();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let mut transaction =
            TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
                .unwrap();
        let choice = TypeKind::Choice(vec![TypeKind::Bool, TypeKind::String]);
        let (pattern, actual) = if direction == ConstraintAcceptance::PatternAcceptsActual {
            (TypeKind::AgentValue, choice)
        } else {
            (choice, TypeKind::AgentValue)
        };
        transaction.constrain(&mut context, &mut graph, &pattern, &actual, direction);
        let path = transaction.test_single_path();
        assert_eq!(path.choice_key.len(), 2);
        for (index, step) in path.choice_key.iter().enumerate() {
            assert_eq!(step.equation, path.equations[0].ordinal);
            assert_eq!(step.direction, direction);
            if direction == ConstraintAcceptance::PatternAcceptsActual {
                assert_eq!(step.role, ChoiceForkRole::ActualAlternative);
                assert_eq!(
                    (step.expected, step.actual),
                    (None, Some(u32::try_from(index).unwrap()))
                );
            } else {
                assert_eq!(step.role, ChoiceForkRole::ExpectedAlternative);
                assert_eq!(
                    (step.expected, step.actual),
                    (Some(u32::try_from(index).unwrap()), None)
                );
            }
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
fn generated_map_key_expectations_preserve_the_original_choice_side() {
    use crate::types::{
        MapKind,
        constraints::transaction::{ChoiceForkRole, TypeConstraintTransaction},
    };
    for direction in [
        ConstraintAcceptance::PatternAcceptsActual,
        ConstraintAcceptance::ActualAcceptsPattern,
    ] {
        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let (mut context, path) = context(empty_scope(), &cancellation, 4096).into_path(&mut graph);
        let application = path.root_id();
        let named = graph
            .declare(vec![], EffectPredicate::unconstrained(), &mut context)
            .unwrap();
        let mut transaction =
            TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
                .unwrap();
        let map = TypeKind::Map {
            kind: MapKind::Ordered,
            key: Box::new(TypeKind::Choice(vec![TypeKind::String, TypeKind::Never])),
            value: Box::new(TypeKind::Bool),
        };
        let (pattern, actual) = if direction == ConstraintAcceptance::PatternAcceptsActual {
            (TypeKind::AgentValue, map)
        } else {
            (map, TypeKind::AgentValue)
        };
        transaction.constrain(&mut context, &mut graph, &pattern, &actual, direction);
        let path = transaction.test_single_path();
        assert_eq!(path.choice_key.len(), 2);
        for (index, step) in path.choice_key.iter().enumerate() {
            assert_eq!(step.equation, path.equations[0].ordinal);
            assert_eq!(step.direction, direction);
            assert_eq!(
                step.role,
                if direction == ConstraintAcceptance::PatternAcceptsActual {
                    ChoiceForkRole::ActualAlternative
                } else {
                    ChoiceForkRole::ExpectedAlternative
                }
            );
            assert_eq!(
                (step.expected, step.actual),
                if direction == ConstraintAcceptance::PatternAcceptsActual {
                    (None, Some(u32::try_from(index).unwrap()))
                } else {
                    (Some(u32::try_from(index).unwrap()), None)
                }
            );
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
fn agent_membership_rejects_nested_bottom_and_fixed_template_variables() {
    use crate::types::constraints::{
        TypeConstraintCandidateFailure, TypeConstraintFailure,
        transaction::TypeConstraintTransaction,
    };
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let (mut context, path) = context(empty_scope(), &cancellation, 4096).into_path(&mut graph);
    let application = path.root_id();
    let mut transaction =
        TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
            .unwrap();
    transaction.constrain(
        &mut context,
        &mut graph,
        &TypeKind::AgentValue,
        &TypeKind::Choice(vec![TypeKind::Bool, TypeKind::Never]),
        ConstraintAcceptance::PatternAcceptsActual,
    );
    assert!(matches!(
        transaction.suspend_result(&mut context, ()),
        Err(TypeConstraintFailure::Rejected(
            TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch)
        ))
    ));
    for bytes in [false, true] {
        let cancellation = AtomicBool::new(false);
        let mut graph = PreparedEffectGraph::default();
        let parameter = GenericTypeParameterId::new(
            GenericParameterOwnerId::Detached(DetachedGenericOwnerId::new(98_922)),
            0,
        );
        let scope = TypeConstraintParameterScope::new([(
            parameter.clone(),
            TypeConstraintParameterEligibility::Rigid,
        )])
        .unwrap();
        let (mut context, path) = super::context(scope, &cancellation, 4096).into_path(&mut graph);
        let application = path.root_id();
        let mut transaction =
            TypeConstraintTransaction::from_path(&mut context, &mut graph, application, path, None)
                .unwrap();
        transaction.constrain_application_template(
            &mut context,
            &mut graph,
            &if bytes {
                TypeKind::Bytes
            } else {
                TypeKind::AgentValue
            },
            &TypeKind::Vec(Box::new(TypeKind::GenericParam(parameter.into()))),
            ConstraintAcceptance::PatternAcceptsActual,
        );
        assert!(matches!(
            transaction.suspend_result(&mut context, ()),
            Err(TypeConstraintFailure::Rejected(
                TypeConstraintCandidateFailure::Constraint(TypeConstraintRejection::Mismatch)
            ))
        ));
    }
}
