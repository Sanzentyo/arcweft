use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
    },
};
use arcweft_lang_sema::{env::TypeCheckEnv, types::TypeKind};

fn manifest() -> CharacterManifest {
    let body = CharacterPartId::try_new("body").expect("part");
    let eyes = CharacterPartId::try_new("eyes").expect("part");
    let uniform = CharacterVariantId::try_new("uniform").expect("variant");
    let normal = CharacterVariantId::try_new("normal").expect("variant");
    let smile = CharacterVariantId::try_new("smile").expect("variant");
    CharacterManifest::new(
        CharacterId::try_new("character.akane").expect("character"),
        CharacterCanvas::new(32, 64),
        CharacterPoint::new(16, 64),
        CharacterLookId::try_new("normal").expect("look"),
        vec![
            CharacterPart::new(
                body.clone(),
                0,
                vec![CharacterVariant::new(
                    uniform.clone(),
                    CharacterAssetPath::try_new("layers/body.png").expect("path"),
                    CharacterRect::new(0, 0, 32, 64),
                    u8::MAX,
                    CharacterBlendMode::Normal,
                    false,
                )],
            ),
            CharacterPart::new(
                eyes.clone(),
                1,
                vec![
                    CharacterVariant::new(
                        normal.clone(),
                        CharacterAssetPath::try_new("layers/eyes-normal.png").expect("path"),
                        CharacterRect::new(8, 12, 16, 8),
                        u8::MAX,
                        CharacterBlendMode::Normal,
                        false,
                    ),
                    CharacterVariant::new(
                        smile.clone(),
                        CharacterAssetPath::try_new("layers/eyes-smile.png").expect("path"),
                        CharacterRect::new(8, 12, 16, 8),
                        u8::MAX,
                        CharacterBlendMode::Normal,
                        false,
                    ),
                ],
            ),
        ],
        vec![
            CharacterLook::new(
                CharacterLookId::try_new("normal").expect("look"),
                vec![
                    CharacterPartSelection::new(body.clone(), uniform.clone()),
                    CharacterPartSelection::new(eyes.clone(), normal),
                ],
            ),
            CharacterLook::new(
                CharacterLookId::try_new("smile").expect("look"),
                vec![
                    CharacterPartSelection::new(body, uniform),
                    CharacterPartSelection::new(eyes, smile),
                ],
            ),
        ],
        None,
    )
    .expect("manifest")
}

#[test]
fn registers_look_part_and_per_part_variant_enums_on_the_owned_environment() {
    let env = TypeCheckEnv::standard().with_character_manifest(&manifest());

    assert_eq!(
        env.character_look_variants("character.akane"),
        Some(vec!["normal".to_owned(), "smile".to_owned()])
    );
    assert_eq!(
        env.character_part_variants("character.akane"),
        Some(vec!["body".to_owned(), "eyes".to_owned()])
    );
    assert_eq!(
        env.character_variant_variants("character.akane", "eyes"),
        Some(vec!["normal".to_owned(), "smile".to_owned()])
    );
    assert_eq!(
        TypeKind::character_look("character.akane").character_look_character(),
        Some("character.akane")
    );
}
