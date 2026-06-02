use arcweft_rust_abi::{ArcweftRustPurity, ArcweftRustTypeKind, ArcweftRustTypeRef};
use arcweft_rust_abi::{ArcweftType as _, ArcweftTypeMetadata as _};
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

#[arcweft_export(name = "mini_games.truck.score_to_rank", pure)]
fn score_to_rank(score: i32, label: String) -> Rank {
    let _ = (score, label);
    Rank::Gold
}

#[test]
fn derive_emits_struct_and_enum_metadata() {
    let player = PlayerScore::arcweft_type_decl();
    assert_eq!(player.name, "PlayerScore");
    let ArcweftRustTypeKind::Struct { fields } = player.kind else {
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
    assert_eq!(variants[1].fields[0].ty, ArcweftRustTypeRef::I32);
    assert_eq!(
        Rank::arcweft_type_ref(),
        ArcweftRustTypeRef::Named {
            name: "Rank".to_owned()
        }
    );
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
    assert_eq!(
        function.return_type,
        ArcweftRustTypeRef::Named {
            name: "Rank".to_owned()
        }
    );
    assert_eq!(function.purity, ArcweftRustPurity::Pure);
}
