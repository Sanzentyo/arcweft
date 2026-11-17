use super::*;
use crate::{
    effect_row::{
        EffectPredicate,
        prepared::{PreparedEffectGraph, PreparedEffectRow},
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
