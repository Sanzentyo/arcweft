use super::*;
use crate::{
    effect_row::{
        EffectPredicate,
        prepared::{PreparedEffectGraph, PreparedEffectPredicate, PreparedEffectRow},
    },
    effects::EffectSet,
    types::{
        CharacterId, PreparedEffects, StageActorHandleType,
        constraints::{
            NoConstraintClient, context::TypeConstraintLimits, test_support::ConstraintTestSetup,
        },
    },
};
use std::sync::atomic::AtomicBool;

fn prepare_relation(
    pattern: TypeKind<PreparedEffects>,
    actual: TypeKind<PreparedEffects>,
    relation: ConstraintRelation,
) -> usize {
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let setup: ConstraintTestSetup<'_, _, NoConstraintClient> = ConstraintTestSetup::new(
        TypeConstraintLimits::new(4096, 4096, 128, 128),
        &cancellation,
    );
    let (mut context, path) = setup.into_path(&mut graph);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let pattern = context.fixture_operand(pattern);
    let actual = context.fixture_operand(actual);
    let retained = graph.retained_terms();
    let paths = relate_constraint(
        &mut graph,
        &pattern.view(),
        &actual.view(),
        path,
        &mut context,
        relation,
    )
    .unwrap();
    assert_eq!(graph.retained_terms(), retained);
    // Provisional structural relation neither solves nor prepares the unrelated
    // Named body. The real first preparation must still succeed exactly once.
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(crate::effect_row::EffectRow::closed(EffectSet::new())),
            vec![],
            &mut context,
        )
        .unwrap();
    paths.len()
}

#[test]
fn scalar_mismatch_is_pruned_without_closing_an_unprepared_original_body() {
    assert_eq!(
        prepare_relation(
            TypeKind::Bool,
            TypeKind::I64,
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
        ),
        0
    );
    assert_eq!(
        prepare_relation(
            TypeKind::Bool,
            TypeKind::Bool,
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
        ),
        1
    );
}

#[test]
fn scalar_widening_preserves_direction_and_is_not_original_equality() {
    for direction in [
        ConstraintAcceptance::PatternAcceptsActual,
        ConstraintAcceptance::ActualAcceptsPattern,
    ] {
        assert_eq!(
            prepare_relation(
                TypeKind::ActionName,
                TypeKind::String,
                ConstraintRelation::Accepts(direction),
            ),
            usize::from(direction == ConstraintAcceptance::PatternAcceptsActual)
        );
    }
    assert_eq!(
        prepare_relation(
            TypeKind::ActionName,
            TypeKind::String,
            ConstraintRelation::Equal,
        ),
        0
    );
}

#[test]
fn erased_actor_handles_do_not_erase_an_exact_expected_identity() {
    let exact = || {
        TypeKind::StageActorHandle(StageActorHandleType::Exact(
            CharacterId::try_new("character.scalar_fixture").unwrap(),
        ))
    };
    assert_eq!(
        prepare_relation(
            TypeKind::StageActorHandle(StageActorHandleType::Any),
            exact(),
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
        ),
        1
    );
    assert_eq!(
        prepare_relation(
            exact(),
            TypeKind::StageActorHandle(StageActorHandleType::Any),
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
        ),
        0
    );
    assert_eq!(
        prepare_relation(
            TypeKind::StageActorHandle(StageActorHandleType::Any),
            exact(),
            ConstraintRelation::Equal,
        ),
        0
    );
}

#[test]
fn reference_payload_presence_preserves_direction_and_original_equality() {
    use crate::types::{EntityKind, EntityType};
    let family = || TypeKind::Ref(EntityType::new(EntityKind::Signal, None));
    let specialized = || TypeKind::Ref(EntityType::new(EntityKind::Signal, Some(TypeKind::Bool)));
    for direction in [
        ConstraintAcceptance::PatternAcceptsActual,
        ConstraintAcceptance::ActualAcceptsPattern,
    ] {
        assert_eq!(
            prepare_relation(
                family(),
                specialized(),
                ConstraintRelation::Accepts(direction),
            ),
            usize::from(direction == ConstraintAcceptance::PatternAcceptsActual)
        );
        assert_eq!(
            prepare_relation(
                specialized(),
                family(),
                ConstraintRelation::Accepts(direction),
            ),
            usize::from(direction == ConstraintAcceptance::ActualAcceptsPattern)
        );
    }
    assert_eq!(
        prepare_relation(family(), specialized(), ConstraintRelation::Equal),
        0
    );
    assert_eq!(
        prepare_relation(specialized(), family(), ConstraintRelation::Equal),
        0
    );
}

#[test]
fn reference_payload_mismatch_is_pruned_before_unrelated_body_preparation() {
    use crate::types::{EntityKind, EntityType};
    assert_eq!(
        prepare_relation(
            TypeKind::Ref(EntityType::new(EntityKind::Signal, Some(TypeKind::Bool))),
            TypeKind::Ref(EntityType::new(EntityKind::Signal, Some(TypeKind::I64))),
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
        ),
        0
    );
}

#[test]
fn composite_headers_reject_arity_kind_and_binder_mismatches_without_closure() {
    use crate::types::GenericBinder;
    let function = |binder, params| {
        TypeKind::function_with_contract(
            binder,
            PreparedEffectPredicate::Canonical(EffectPredicate::unconstrained()),
            params,
            TypeKind::I64,
            PreparedEffectRow::Canonical(crate::effect_row::EffectRow::closed(EffectSet::new())),
        )
    };
    let cases = [
        (
            TypeKind::Tuple(vec![TypeKind::Bool]),
            TypeKind::Tuple(vec![]),
        ),
        (
            TypeKind::Vec(Box::new(TypeKind::Bool)),
            TypeKind::Slice(Box::new(TypeKind::Bool)),
        ),
        (
            function(GenericBinder::EMPTY, vec![]),
            function(GenericBinder::EMPTY, vec![TypeKind::Bool]),
        ),
        (
            function(GenericBinder::new(1, 0, 0), vec![]),
            function(GenericBinder::EMPTY, vec![]),
        ),
    ];
    for (expected, actual) in cases {
        assert_eq!(
            prepare_relation(
                expected,
                actual,
                ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual)
            ),
            0
        );
    }
}

#[test]
fn bottom_subtyping_preserves_both_directions_inside_composites() {
    for direction in [
        ConstraintAcceptance::PatternAcceptsActual,
        ConstraintAcceptance::ActualAcceptsPattern,
    ] {
        assert_eq!(
            prepare_relation(
                TypeKind::Tuple(vec![TypeKind::Bool]),
                TypeKind::Tuple(vec![TypeKind::Never]),
                ConstraintRelation::Accepts(direction)
            ),
            usize::from(direction == ConstraintAcceptance::PatternAcceptsActual)
        );
    }
    assert_eq!(
        prepare_relation(
            TypeKind::Tuple(vec![TypeKind::Bool]),
            TypeKind::Tuple(vec![TypeKind::Never]),
            ConstraintRelation::Equal
        ),
        0
    );
}

#[test]
fn dependent_payloads_do_not_widen_bottom_family_or_choice_children() {
    use crate::types::{
        EntityKind, EntityType, VariantPayloadOwnerFamily, VariantPayloadType,
        VariantPayloadTypeShape,
    };
    let payload = |item: TypeKind<PreparedEffects>| {
        TypeKind::VariantPayload(Box::new(VariantPayloadType::from_prepared_case(
            VariantPayloadOwnerFamily::Result,
            TypeKind::Result {
                ok: Box::new(item.clone()),
                error: Box::new(TypeKind::String),
            },
            0,
            VariantPayloadTypeShape::Tuple(vec![item].into_boxed_slice()),
        )))
    };
    let cases = [
        (TypeKind::Bool, TypeKind::Never),
        (
            TypeKind::Ref(EntityType::new(EntityKind::Signal, None)),
            TypeKind::Ref(EntityType::new(EntityKind::Signal, Some(TypeKind::Bool))),
        ),
        (
            TypeKind::Choice(vec![TypeKind::Bool, TypeKind::I64]),
            TypeKind::Choice(vec![TypeKind::I64, TypeKind::Bool]),
        ),
    ];
    for (expected, actual) in cases {
        assert_eq!(
            prepare_relation(
                payload(expected),
                payload(actual),
                ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual)
            ),
            0
        );
    }
    assert_eq!(
        prepare_relation(
            payload(TypeKind::Bool),
            payload(TypeKind::Bool),
            ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual)
        ),
        1
    );
}

#[test]
fn a_mismatched_header_does_not_hide_an_unresolved_descendant() {
    use crate::types::TypePoisonId;
    let cancellation = AtomicBool::new(false);
    let mut graph = PreparedEffectGraph::default();
    let setup: ConstraintTestSetup<'_, _, NoConstraintClient> = ConstraintTestSetup::new(
        TypeConstraintLimits::new(4096, 4096, 128, 128),
        &cancellation,
    );
    let (mut context, path) = setup.into_path(&mut graph);
    let named = graph
        .declare(vec![], EffectPredicate::unconstrained(), &mut context)
        .unwrap();
    let expected =
        context.fixture_operand(TypeKind::<PreparedEffects>::Tuple(vec![TypeKind::Bool]));
    let actual = context.fixture_operand(TypeKind::<PreparedEffects>::Vec(Box::new(
        TypeKind::Error(TypePoisonId::from_index(98_990)),
    )));
    let result = relate_constraint(
        &mut graph,
        &expected.view(),
        &actual.view(),
        path,
        &mut context,
        ConstraintRelation::Accepts(ConstraintAcceptance::PatternAcceptsActual),
    );
    assert!(matches!(
        result,
        Err(TypeConstraintError::Rejected(
            TypeConstraintRejection::UnresolvedType
        ))
    ));
    graph
        .prepare_body(
            &named,
            PreparedEffectRow::Canonical(crate::effect_row::EffectRow::closed(EffectSet::new())),
            vec![],
            &mut context,
        )
        .unwrap();
}
