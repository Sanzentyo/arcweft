use super::*;
use crate::{effects::EffectSet, types::AcceptedNominalType};

fn envelope(argument: TypeKind) -> TypeKind {
    TypeKind::AcceptedNominal(AcceptedNominalType::new(
        crate::env::nominal::standard_nominal_id("InvariantEnvelope"),
        vec![argument],
    ))
}

fn accepts(expected: &TypeKind, actual: &TypeKind) -> bool {
    expected
        .accepts_with(
            actual,
            TypeCompatibilityPolicy::SelectedCall,
            &mut NoopTypeCompatibilityControl,
        )
        .unwrap()
}

#[test]
fn nominal_callback_arguments_do_not_widen_their_effect_rows() {
    let pure =
        TypeKind::function_with_effects([], TypeKind::I64, EffectRow::closed(EffectSet::new()));
    let logged = TypeKind::function_with_effects(
        [],
        TypeKind::I64,
        EffectRow::closed(EffectSet::from_labels(["log.write"]).unwrap()),
    );
    assert!(accepts(&logged, &pure));
    let pure = envelope(pure);
    let logged = envelope(logged);
    assert!(!accepts(&logged, &pure));
    assert!(!accepts(&pure, &logged));
    assert!(accepts(&pure, &pure));
    assert!(accepts(&logged, &logged));
}

#[test]
fn bottom_and_erased_actor_arguments_do_not_change_a_nominal_instantiation() {
    use crate::types::{CharacterId, StageActorHandleType};
    let integer = envelope(TypeKind::I64);
    let bottom = envelope(TypeKind::Never);
    assert!(!accepts(&integer, &bottom));
    assert!(!accepts(&bottom, &integer));
    let any = envelope(TypeKind::StageActorHandle(StageActorHandleType::Any));
    let exact = envelope(TypeKind::StageActorHandle(StageActorHandleType::Exact(
        CharacterId::try_new("character.alice").unwrap(),
    )));
    assert!(!accepts(&any, &exact));
    assert!(!accepts(&exact, &any));
}
