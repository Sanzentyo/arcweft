use arcweft_character::id::{CharacterId, CharacterPartId};
use arcweft_lang_sema::{
    effect_row::EffectRow,
    types::{
        ArrayLength, CharacterNominalFamily, EntityKind, EntityType, IteratorStateKind,
        LifetimeScopeKind, MapKind, TypeKind, TypeMismatchPathSegment, TypeMismatchReason,
    },
};
use arcweft_lang_syntax::reference::BorrowKind;

fn look(owner: &str) -> TypeKind {
    TypeKind::character_look(CharacterId::try_new(owner).expect("character id"))
}

fn variant(owner: &str, part: &str) -> TypeKind {
    TypeKind::character_variant(
        CharacterId::try_new(owner).expect("character id"),
        CharacterPartId::try_new(part).expect("part id"),
    )
}

#[test]
fn nominal_reason_precedence_is_family_owner_then_variant_part() {
    let owner_a = CharacterId::try_new("character.a").expect("owner a");
    let owner_b = CharacterId::try_new("character.b").expect("owner b");
    let part_a = CharacterPartId::try_new("body").expect("part a");
    let part_b = CharacterPartId::try_new("face").expect("part b");

    let expected = TypeKind::character_look(owner_a.clone());
    let actual = TypeKind::character_part(owner_b.clone());
    assert!(matches!(
        expected
            .first_mismatch(&actual)
            .expect("family mismatch")
            .reason(),
        TypeMismatchReason::CharacterFamily {
            expected: CharacterNominalFamily::Look,
            actual: CharacterNominalFamily::Part,
        }
    ));

    let expected = TypeKind::character_look(owner_a.clone());
    let actual = TypeKind::character_look(owner_b.clone());
    assert!(matches!(
        expected
            .first_mismatch(&actual)
            .expect("owner mismatch")
            .reason(),
        TypeMismatchReason::CharacterOwner { expected, actual }
            if expected == &owner_a && actual == &owner_b
    ));

    let expected = TypeKind::character_variant(owner_a.clone(), part_a.clone());
    let actual = TypeKind::character_variant(owner_a, part_b.clone());
    assert!(matches!(
        expected
            .first_mismatch(&actual)
            .expect("part mismatch")
            .reason(),
        TypeMismatchReason::CharacterVariantPart { expected, actual }
            if expected == &part_a && actual == &part_b
    ));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive table proves a deterministic child path for every current TypeKind family"
)]
fn every_current_type_child_has_a_deterministic_path_segment() {
    let expected = look("character.a");
    let actual = look("character.b");
    let cases = vec![
        (
            TypeKind::Range(Box::new(expected.clone())),
            TypeKind::Range(Box::new(actual.clone())),
            TypeMismatchPathSegment::RangeItem,
        ),
        (
            TypeKind::IteratorState {
                family: IteratorStateKind::Seq,
                item: Box::new(expected.clone()),
            },
            TypeKind::IteratorState {
                family: IteratorStateKind::Seq,
                item: Box::new(actual.clone()),
            },
            TypeMismatchPathSegment::IteratorItem,
        ),
        (
            TypeKind::Ref(EntityType::new(
                EntityKind::Character,
                Some(expected.clone()),
            )),
            TypeKind::Ref(EntityType::new(EntityKind::Character, Some(actual.clone()))),
            TypeMismatchPathSegment::EntityPayload,
        ),
        (
            TypeKind::Probe(Box::new(expected.clone())),
            TypeKind::Probe(Box::new(actual.clone())),
            TypeMismatchPathSegment::ProbeItem,
        ),
        (
            TypeKind::Vec(Box::new(expected.clone())),
            TypeKind::Vec(Box::new(actual.clone())),
            TypeMismatchPathSegment::VectorItem,
        ),
        (
            TypeKind::Array {
                item: Box::new(expected.clone()),
                len: ArrayLength::Const(4),
            },
            TypeKind::Array {
                item: Box::new(actual.clone()),
                len: ArrayLength::Const(4),
            },
            TypeMismatchPathSegment::ArrayItem,
        ),
        (
            TypeKind::Slice(Box::new(expected.clone())),
            TypeKind::Slice(Box::new(actual.clone())),
            TypeMismatchPathSegment::SliceItem,
        ),
        (
            TypeKind::Seq(Box::new(expected.clone())),
            TypeKind::Seq(Box::new(actual.clone())),
            TypeMismatchPathSegment::SequenceItem,
        ),
        (
            TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(expected.clone()),
                value: Box::new(TypeKind::Unit),
            },
            TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(actual.clone()),
                value: Box::new(TypeKind::Unit),
            },
            TypeMismatchPathSegment::MapKey,
        ),
        (
            TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(TypeKind::String),
                value: Box::new(expected.clone()),
            },
            TypeKind::Map {
                kind: MapKind::Ordered,
                key: Box::new(TypeKind::String),
                value: Box::new(actual.clone()),
            },
            TypeMismatchPathSegment::MapValue,
        ),
        (
            TypeKind::BorrowRef {
                kind: BorrowKind::Shared,
                lifetime: Some(LifetimeScopeKind::Flow),
                inner: Box::new(expected.clone()),
            },
            TypeKind::BorrowRef {
                kind: BorrowKind::Shared,
                lifetime: Some(LifetimeScopeKind::Flow),
                inner: Box::new(actual.clone()),
            },
            TypeMismatchPathSegment::BorrowInner,
        ),
        (
            TypeKind::Need(Box::new(expected.clone())),
            TypeKind::Need(Box::new(actual.clone())),
            TypeMismatchPathSegment::NeedItem,
        ),
        (
            TypeKind::Stream {
                item: Box::new(expected.clone()),
                error: Box::new(TypeKind::Unit),
            },
            TypeKind::Stream {
                item: Box::new(actual.clone()),
                error: Box::new(TypeKind::Unit),
            },
            TypeMismatchPathSegment::StreamItem,
        ),
        (
            TypeKind::Stream {
                item: Box::new(TypeKind::Unit),
                error: Box::new(expected.clone()),
            },
            TypeKind::Stream {
                item: Box::new(TypeKind::Unit),
                error: Box::new(actual.clone()),
            },
            TypeMismatchPathSegment::StreamError,
        ),
        (
            TypeKind::Result {
                ok: Box::new(expected.clone()),
                error: Box::new(TypeKind::Unit),
            },
            TypeKind::Result {
                ok: Box::new(actual.clone()),
                error: Box::new(TypeKind::Unit),
            },
            TypeMismatchPathSegment::ResultOk,
        ),
        (
            TypeKind::Result {
                ok: Box::new(TypeKind::Unit),
                error: Box::new(expected.clone()),
            },
            TypeKind::Result {
                ok: Box::new(TypeKind::Unit),
                error: Box::new(actual.clone()),
            },
            TypeMismatchPathSegment::ResultError,
        ),
        (
            TypeKind::Option(Box::new(expected.clone())),
            TypeKind::Option(Box::new(actual.clone())),
            TypeMismatchPathSegment::OptionItem,
        ),
        (
            TypeKind::ThreadHandle(Box::new(expected.clone())),
            TypeKind::ThreadHandle(Box::new(actual.clone())),
            TypeMismatchPathSegment::ThreadResult,
        ),
        (
            TypeKind::Shared(Box::new(expected.clone())),
            TypeKind::Shared(Box::new(actual.clone())),
            TypeMismatchPathSegment::SharedInner,
        ),
        (
            TypeKind::Function {
                params: vec![TypeKind::Unit, expected.clone()],
                return_type: Box::new(TypeKind::Unit),
                effects: EffectRow::unknown(),
            },
            TypeKind::Function {
                params: vec![TypeKind::Unit, actual.clone()],
                return_type: Box::new(TypeKind::Unit),
                effects: EffectRow::unknown(),
            },
            TypeMismatchPathSegment::FunctionParameter(1),
        ),
        (
            TypeKind::Function {
                params: vec![],
                return_type: Box::new(expected.clone()),
                effects: EffectRow::unknown(),
            },
            TypeKind::Function {
                params: vec![],
                return_type: Box::new(actual.clone()),
                effects: EffectRow::unknown(),
            },
            TypeMismatchPathSegment::FunctionReturn,
        ),
        (
            TypeKind::Projection {
                subject: Box::new(expected.clone()),
                trait_name: Some("Trait".to_owned()),
                assoc: "Item".to_owned(),
            },
            TypeKind::Projection {
                subject: Box::new(actual.clone()),
                trait_name: Some("Trait".to_owned()),
                assoc: "Item".to_owned(),
            },
            TypeMismatchPathSegment::ProjectionSubject,
        ),
        (
            TypeKind::Tuple(vec![TypeKind::Unit, expected.clone()]),
            TypeKind::Tuple(vec![TypeKind::Unit, actual.clone()]),
            TypeMismatchPathSegment::TupleElement(1),
        ),
        (
            TypeKind::Choice(vec![TypeKind::Unit, expected]),
            TypeKind::Choice(vec![TypeKind::Unit, actual]),
            TypeMismatchPathSegment::ChoiceAlternative(1),
        ),
    ];

    for (expected, actual, segment) in cases {
        let mismatch = expected
            .first_mismatch(&actual)
            .expect("nested owner mismatch");
        assert_eq!(mismatch.path().first(), Some(&segment));
        assert_eq!(
            mismatch.path().last(),
            Some(&TypeMismatchPathSegment::CharacterOwner)
        );
    }
}

#[test]
fn borrow_kind_mismatch_precedes_nested_nominal_mismatch() {
    let expected = TypeKind::BorrowRef {
        kind: BorrowKind::Shared,
        lifetime: Some(LifetimeScopeKind::Flow),
        inner: Box::new(look("character.a")),
    };
    let actual = TypeKind::BorrowRef {
        kind: BorrowKind::Mutable,
        lifetime: Some(LifetimeScopeKind::Flow),
        inner: Box::new(look("character.b")),
    };

    let mismatch = expected
        .first_mismatch(&actual)
        .expect("borrow permission mismatch");
    assert_eq!(mismatch.path(), &[TypeMismatchPathSegment::BorrowKind]);
    assert_eq!(mismatch.reason(), &TypeMismatchReason::NonTypeParameter);
}

#[test]
fn ordinary_outer_and_non_type_mismatches_are_not_reclassified() {
    let outer = TypeKind::Option(Box::new(look("character.a")))
        .first_mismatch(&TypeKind::Vec(Box::new(look("character.b"))))
        .expect("outer constructor mismatch");
    assert!(outer.path().is_empty());
    assert_eq!(outer.reason(), &TypeMismatchReason::OuterConstructor);

    let length = TypeKind::Array {
        item: Box::new(look("character.a")),
        len: ArrayLength::Const(2),
    }
    .first_mismatch(&TypeKind::Array {
        item: Box::new(look("character.b")),
        len: ArrayLength::Const(3),
    })
    .expect("array length mismatch takes precedence");
    assert_eq!(length.path(), &[TypeMismatchPathSegment::ArrayLength]);
    assert_eq!(length.reason(), &TypeMismatchReason::NonTypeParameter);
    assert!(
        variant("character.a", "body")
            .first_mismatch(&variant("character.a", "body"))
            .is_none()
    );
}
