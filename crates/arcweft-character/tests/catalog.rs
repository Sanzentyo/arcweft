use arcweft_character::{
    catalog::{CharacterCatalog, CharacterCatalogError},
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
    },
};

fn manifest(id: &str) -> CharacterManifest {
    let look = CharacterLookId::try_new("normal").expect("look");
    CharacterManifest::new(
        CharacterId::try_new(id).expect("character"),
        CharacterCanvas::new(8, 8),
        CharacterPoint::new(4, 8),
        look.clone(),
        vec![CharacterPart::new(
            CharacterPartId::try_new("body").expect("part"),
            0,
            vec![CharacterVariant::new(
                CharacterVariantId::try_new("default").expect("variant"),
                CharacterAssetPath::try_new("layers/body.png").expect("asset"),
                CharacterRect::new(0, 0, 8, 8),
                u8::MAX,
                CharacterBlendMode::Normal,
                false,
            )],
        )],
        vec![CharacterLook::new(
            look,
            vec![CharacterPartSelection::new(
                CharacterPartId::try_new("body").expect("part"),
                CharacterVariantId::try_new("default").expect("variant"),
            )],
        )],
        None,
    )
    .expect("manifest")
}

#[test]
fn duplicate_characters_are_rejected() {
    assert!(matches!(
        CharacterCatalog::try_from_manifests([
            manifest("character.akane"),
            manifest("character.akane"),
        ]),
        Err(CharacterCatalogError::DuplicateOwner { .. })
    ));
}
