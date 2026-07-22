use arcweft_rust_abi::{
    ArcweftRustPurity, ArcweftRustStructShape, ArcweftRustTypeKind, ArcweftRustTypeRef,
    ArcweftRustVariantPayload, ArcweftType as _, ArcweftTypeMetadata as _,
};
use arcweft_rust_abi_macros::{ArcweftType, arcweft_export};

#[derive(ArcweftType)]
struct PlayerScore {
    score: i32,
    label: String,
}

#[derive(ArcweftType)]
enum Rank {
    Gold,
    Silver { threshold: i32 },
}

#[derive(ArcweftType)]
struct Pair<Z, A> {
    first: Z,
    second: A,
    history: Vec<Option<Z>>,
}

#[arcweft_export(name = "mini_games.truck.score_to_rank", pure)]
fn score_to_rank(score: i32, label: String) -> Rank {
    let _ = (score, label);
    Rank::Gold
}

#[test]
fn derive_emits_struct_and_enum_metadata() {
    let player = PlayerScore::arcweft_type_decl();
    assert_eq!(player.path.to_string(), "PlayerScore");
    let ArcweftRustTypeKind::Struct {
        shape: ArcweftRustStructShape::Record { fields },
    } = player.kind
    else {
        panic!("expected struct metadata");
    };
    assert_eq!(fields[0].name, "score");
    assert_eq!(fields[0].ty, ArcweftRustTypeRef::I32);
    assert_eq!(fields[1].ty, ArcweftRustTypeRef::String);

    let rank = Rank::arcweft_type_decl();
    let ArcweftRustTypeKind::Enum { variants } = rank.kind else {
        panic!("expected enum metadata");
    };
    assert_eq!(variants[0].name, "Gold");
    let ArcweftRustVariantPayload::Record { fields } = &variants[1].payload else {
        panic!("expected record payload");
    };
    assert_eq!(fields[0].ty, ArcweftRustTypeRef::I32);
    assert!(matches!(
        Rank::arcweft_type_ref(),
        ArcweftRustTypeRef::Nominal { arguments, .. } if arguments.is_empty()
    ));
}

#[test]
fn generic_derive_preserves_argument_and_template_order() {
    let value = Pair {
        first: 7_i32,
        second: "typed".to_owned(),
        history: vec![Some(9)],
    };
    assert_eq!(
        (value.first, value.second.as_str(), value.history[0]),
        (7, "typed", Some(9))
    );

    let concrete = Pair::<i32, String>::arcweft_type_ref();
    let ArcweftRustTypeRef::Nominal { arguments, .. } = concrete else {
        panic!("derived generic type is nominal");
    };
    assert_eq!(
        arguments,
        vec![ArcweftRustTypeRef::I32, ArcweftRustTypeRef::String]
    );

    let declaration = Pair::<i32, String>::arcweft_type_decl();
    assert_eq!(declaration.parameters[0].name.as_str(), "Z");
    assert_eq!(declaration.parameters[0].index.get(), 0);
    assert_eq!(declaration.parameters[1].name.as_str(), "A");
    assert_eq!(declaration.parameters[1].index.get(), 1);
    let ArcweftRustTypeKind::Struct {
        shape: ArcweftRustStructShape::Record { fields },
    } = declaration.kind
    else {
        panic!("expected record metadata");
    };
    assert!(matches!(
        fields[0].ty,
        ArcweftRustTypeRef::TypeParameter { index } if index.get() == 0
    ));
    assert!(matches!(
        fields[1].ty,
        ArcweftRustTypeRef::TypeParameter { index } if index.get() == 1
    ));
    let ArcweftRustTypeRef::Vec { item } = &fields[2].ty else {
        panic!("history is a vector");
    };
    let ArcweftRustTypeRef::Option { item } = item.as_ref() else {
        panic!("history items are optional");
    };
    assert!(matches!(
        item.as_ref(),
        ArcweftRustTypeRef::TypeParameter { index } if index.get() == 0
    ));
}

#[test]
fn export_emits_function_signature_metadata() {
    let player = PlayerScore {
        score: 1,
        label: "demo".to_owned(),
    };
    assert_eq!(player.score, 1);
    assert_eq!(player.label, "demo");
    assert!(matches!(score_to_rank(1, "demo".to_owned()), Rank::Gold));
    let silver = Rank::Silver { threshold: 2 };
    assert!(matches!(silver, Rank::Silver { threshold: 2 }));

    let function = __arcweft_export_score_to_rank_metadata();

    assert_eq!(function.name, "mini_games.truck.score_to_rank");
    assert_eq!(function.params.len(), 2);
    assert_eq!(function.params[0].name, "score");
    assert_eq!(function.params[0].ty, ArcweftRustTypeRef::I32);
    assert_eq!(function.params[1].ty, ArcweftRustTypeRef::String);
    assert_eq!(function.return_type, Rank::arcweft_type_ref());
    assert_eq!(function.purity, ArcweftRustPurity::Pure);
}
