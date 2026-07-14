use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
    },
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    types::{CharacterNominalKind, CharacterNominalType, TypeKind},
};
use std::collections::HashSet;

fn manifest(character: &str) -> CharacterManifest {
    let body = CharacterPartId::try_new("body").expect("part");
    let eyes = CharacterPartId::try_new("eyes").expect("part");
    let uniform = CharacterVariantId::try_new("uniform").expect("variant");
    let normal = CharacterVariantId::try_new("normal").expect("variant");
    let smile = CharacterVariantId::try_new("smile").expect("variant");
    CharacterManifest::new(
        CharacterId::try_new(character).expect("character"),
        CharacterCanvas::new(32, 64),
        CharacterPoint::new(16, 64),
        CharacterLookId::try_new("normal").expect("look"),
        vec![
            CharacterPart::new(
                body.clone(),
                0,
                vec![CharacterVariant::new(
                    uniform.clone(),
                    CharacterAssetPath::try_new(format!(
                        "layers/{}-body.png",
                        character.trim_start_matches("character.")
                    ))
                    .expect("path"),
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
                        CharacterAssetPath::try_new(format!(
                            "layers/{}-eyes-normal.png",
                            character.trim_start_matches("character.")
                        ))
                        .expect("path"),
                        CharacterRect::new(8, 12, 16, 8),
                        u8::MAX,
                        CharacterBlendMode::Normal,
                        false,
                    ),
                    CharacterVariant::new(
                        smile.clone(),
                        CharacterAssetPath::try_new(format!(
                            "layers/{}-eyes-smile.png",
                            character.trim_start_matches("character.")
                        ))
                        .expect("path"),
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
fn registers_manifest_enums_on_structural_nominal_types() {
    let manifest = manifest("character.akane");
    let character = manifest.character().clone();
    let eyes = CharacterPartId::try_new("eyes").expect("part");
    let env = TypeCheckEnv::standard().with_character_manifest(&manifest);

    assert_eq!(
        env.character_look_variants(&character),
        Some(vec!["normal".to_owned(), "smile".to_owned()])
    );
    assert_eq!(
        env.character_part_variants(&character),
        Some(vec!["body".to_owned(), "eyes".to_owned()])
    );
    assert_eq!(
        env.character_variant_variants(&character, &eyes),
        Some(vec!["normal".to_owned(), "smile".to_owned()])
    );

    let look = TypeKind::character_look(character.clone());
    assert_eq!(look.source_label(), "CharacterLook<character.akane>");
    assert_eq!(look.to_string(), "CharacterLook<character.akane>");
    assert!(matches!(
        look.character_nominal(),
        Some(CharacterNominalType::Look { character: owner }) if owner == &character
    ));
    assert_eq!(
        look.character_nominal().map(CharacterNominalType::kind),
        Some(CharacterNominalKind::Look)
    );
}

#[test]
fn equal_member_spellings_preserve_character_family_and_part_identity() {
    let akane_manifest = manifest("character.akane");
    let aoi_manifest = manifest("character.aoi");
    let akane = akane_manifest.character().clone();
    let aoi = aoi_manifest.character().clone();
    let body = CharacterPartId::try_new("body").expect("part");
    let eyes = CharacterPartId::try_new("eyes").expect("part");
    let env = TypeCheckEnv::standard()
        .with_character_manifest(&akane_manifest)
        .with_character_manifest(&aoi_manifest);

    let akane_look = TypeKind::character_look(akane.clone());
    let aoi_look = TypeKind::character_look(aoi.clone());
    let akane_part = TypeKind::character_part(akane.clone());
    let akane_eyes = TypeKind::character_variant(akane.clone(), eyes.clone());
    let akane_body = TypeKind::character_variant(akane.clone(), body);
    let aoi_eyes = TypeKind::character_variant(aoi.clone(), eyes);

    assert_ne!(akane_look, aoi_look);
    assert_ne!(akane_look, akane_part);
    assert_ne!(akane_eyes, akane_body);
    assert_ne!(akane_eyes, aoi_eyes);
    assert_ne!(
        akane_look,
        TypeKind::Named("CharacterLook<character.akane>".to_owned())
    );

    let equal_hashes = HashSet::from([akane_look.clone(), akane_look.clone()]);
    assert_eq!(equal_hashes.len(), 1);
    assert_ne!(
        TypeKind::Vec(Box::new(akane_look.clone())),
        TypeKind::Vec(Box::new(aoi_look.clone()))
    );
    assert_ne!(
        TypeKind::function([akane_look.clone()], akane_eyes.clone()),
        TypeKind::function([aoi_look.clone()], aoi_eyes.clone())
    );

    let sets = env.enum_variant_sets();
    assert!(sets.iter().any(|(ty, variants)| {
        ty == &akane_look && variants == &["normal".to_owned(), "smile".to_owned()]
    }));
    assert!(sets.iter().any(|(ty, variants)| {
        ty == &aoi_look && variants == &["normal".to_owned(), "smile".to_owned()]
    }));

    let colliding_label = akane_look.source_label();
    let synthetic = TypeKind::Named(colliding_label.clone());
    let colliding_labels = env
        .with_enum_variants(synthetic.clone(), ["synthetic"])
        .enum_variant_sets()
        .into_iter()
        .filter(|(ty, _)| ty.source_label() == colliding_label)
        .map(|(ty, _)| ty)
        .collect::<Vec<_>>();
    assert_eq!(colliding_labels, vec![akane_look, synthetic]);
}
