use std::fmt::Write as _;

use super::*;
use crate::id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId};

fn sample_manifest() -> CharacterManifest {
    let body = CharacterPart::new(
        CharacterPartId::try_new("body").expect("id"),
        0,
        vec![CharacterVariant::new(
            CharacterVariantId::try_new("default").expect("id"),
            CharacterAssetPath::try_new("layers/body.png").expect("path"),
            CharacterRect::new(0, 0, 64, 128),
            u8::MAX,
            CharacterBlendMode::Normal,
            false,
        )],
    );
    let look = CharacterLook::new(
        CharacterLookId::try_new("normal").expect("id"),
        vec![CharacterPartSelection::new(
            CharacterPartId::try_new("body").expect("id"),
            CharacterVariantId::try_new("default").expect("id"),
        )],
    );
    CharacterManifest::new(
        CharacterId::try_new("character.akane").expect("id"),
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        CharacterLookId::try_new("normal").expect("id"),
        vec![body],
        vec![look],
        None,
    )
    .expect("manifest")
}

#[test]
fn manifest_json_round_trips_and_resolves() {
    let manifest = sample_manifest();
    let json = manifest.to_json_pretty().expect("json");
    let decoded = CharacterManifest::decode_runtime_json(&json).expect("decode");
    let layers = decoded
        .resolve_look(decoded.default_look())
        .expect("resolve");
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].part().id().as_str(), "body");
}

#[test]
fn manifest_fingerprint_v1_fixed_vector() {
    let manifest = sample_manifest();
    let digest = manifest.semantic_fingerprint_v1();
    let hex = digest.as_bytes().iter().fold(
        String::with_capacity(digest.as_bytes().len() * 2),
        |mut hex, byte| {
            write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
            hex
        },
    );
    assert_eq!(
        hex,
        "69a0eec2964e4cb18b64b1db68da290fdc6c9539368831064d3d051bf2f17179"
    );
    assert_eq!(fingerprint::canonical_len_v1(&manifest), 200);
}

#[test]
fn fingerprint_ignores_source_provenance() {
    let manifest = sample_manifest();
    let expected = manifest.semantic_fingerprint_v1();

    let mut with_source = manifest.clone();
    with_source.source = Some(CharacterSource::psd(
        "akane-a.psd",
        "digest-a",
        "importer-a",
        vec!["warning-a".to_owned()],
    ));
    with_source.parts[0].variants[0].source_layer =
        Some(CharacterSourceLayer::new(4, "group-a", "layer-a"));
    assert_eq!(with_source.semantic_fingerprint_v1(), expected);

    with_source.source = Some(CharacterSource::psd(
        "akane-b.psd",
        "digest-b",
        "importer-b",
        vec!["warning-b".to_owned(), "warning-c".to_owned()],
    ));
    with_source.parts[0].variants[0].source_layer =
        Some(CharacterSourceLayer::new(9, "group-b", "layer-b"));
    assert_eq!(with_source.semantic_fingerprint_v1(), expected);
}

#[test]
fn fingerprint_normalizes_collection_order() {
    let body_default = CharacterVariant::new(
        CharacterVariantId::try_new("default").expect("id"),
        CharacterAssetPath::try_new("layers/body-default.png").expect("path"),
        CharacterRect::new(0, 0, 64, 128),
        u8::MAX,
        CharacterBlendMode::Normal,
        false,
    );
    let body_alt = CharacterVariant::new(
        CharacterVariantId::try_new("alt").expect("id"),
        CharacterAssetPath::try_new("layers/body-alt.png").expect("path"),
        CharacterRect::new(0, 0, 64, 128),
        200,
        CharacterBlendMode::Multiply,
        true,
    );
    let face_default = CharacterVariant::new(
        CharacterVariantId::try_new("default").expect("id"),
        CharacterAssetPath::try_new("layers/face-default.png").expect("path"),
        CharacterRect::new(8, 12, 48, 48),
        u8::MAX,
        CharacterBlendMode::Normal,
        false,
    );
    let face_alt = CharacterVariant::new(
        CharacterVariantId::try_new("alt").expect("id"),
        CharacterAssetPath::try_new("layers/face-alt.png").expect("path"),
        CharacterRect::new(8, 12, 48, 48),
        240,
        CharacterBlendMode::Screen,
        false,
    );
    let body = CharacterPart::new(
        CharacterPartId::try_new("body").expect("id"),
        0,
        vec![body_default, body_alt],
    );
    let face = CharacterPart::new(
        CharacterPartId::try_new("face").expect("id"),
        1,
        vec![face_default, face_alt],
    );
    let normal = CharacterLook::new(
        CharacterLookId::try_new("normal").expect("id"),
        vec![
            CharacterPartSelection::new(
                CharacterPartId::try_new("body").expect("id"),
                CharacterVariantId::try_new("default").expect("id"),
            ),
            CharacterPartSelection::new(
                CharacterPartId::try_new("face").expect("id"),
                CharacterVariantId::try_new("default").expect("id"),
            ),
        ],
    );
    let alt = CharacterLook::new(
        CharacterLookId::try_new("alt").expect("id"),
        vec![
            CharacterPartSelection::new(
                CharacterPartId::try_new("body").expect("id"),
                CharacterVariantId::try_new("alt").expect("id"),
            ),
            CharacterPartSelection::new(
                CharacterPartId::try_new("face").expect("id"),
                CharacterVariantId::try_new("alt").expect("id"),
            ),
        ],
    );
    let manifest = CharacterManifest::new(
        CharacterId::try_new("character.akane").expect("id"),
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        CharacterLookId::try_new("normal").expect("id"),
        vec![body, face],
        vec![normal, alt],
        None,
    )
    .expect("manifest");
    let expected = manifest.semantic_fingerprint_v1();

    let mut reordered = manifest;
    reordered.parts.reverse();
    for part in &mut reordered.parts {
        part.variants.reverse();
    }
    reordered.looks.reverse();
    for look in &mut reordered.looks {
        look.select.reverse();
    }
    assert_eq!(reordered.semantic_fingerprint_v1(), expected);
}

#[test]
fn fingerprint_observes_semantic_fields() {
    let manifest = sample_manifest();
    let expected = manifest.semantic_fingerprint_v1();
    let mutations: [fn(&mut CharacterManifest); 20] = [
        |value| value.character = CharacterId::try_new("character.aoi").expect("id"),
        |value| value.canvas.width += 1,
        |value| value.canvas.height += 1,
        |value| value.anchor.x += 1,
        |value| value.anchor.y += 1,
        |value| value.default_look = CharacterLookId::try_new("alt").expect("id"),
        |value| value.parts[0].id = CharacterPartId::try_new("face").expect("id"),
        |value| value.parts[0].z += 1,
        |value| {
            value.parts[0].variants[0].id = CharacterVariantId::try_new("alt").expect("id");
        },
        |value| {
            value.parts[0].variants[0].asset =
                CharacterAssetPath::try_new("layers/alt.png").expect("path");
        },
        |value| value.parts[0].variants[0].rect.x += 1,
        |value| value.parts[0].variants[0].rect.y += 1,
        |value| value.parts[0].variants[0].rect.width += 1,
        |value| value.parts[0].variants[0].rect.height += 1,
        |value| value.parts[0].variants[0].opacity -= 1,
        |value| value.parts[0].variants[0].blend = CharacterBlendMode::Multiply,
        |value| value.parts[0].variants[0].clipping = true,
        |value| value.looks[0].id = CharacterLookId::try_new("alt").expect("id"),
        |value| {
            value.looks[0].select[0].part = CharacterPartId::try_new("face").expect("id");
        },
        |value| {
            value.looks[0].select[0].variant = CharacterVariantId::try_new("alt").expect("id");
        },
    ];

    for mutate in mutations {
        let mut changed = manifest.clone();
        mutate(&mut changed);
        assert_ne!(changed.semantic_fingerprint_v1(), expected);
    }
}

#[test]
fn photoshop_blend_names_map_on_the_domain_enum() {
    assert_eq!(
        CharacterBlendMode::from_photoshop_debug_name("Multiply"),
        Some(CharacterBlendMode::Multiply)
    );
    assert_eq!(
        CharacterBlendMode::from_photoshop_debug_name("Unknown"),
        None
    );
}

#[test]
fn incomplete_look_is_rejected() {
    let mut manifest = sample_manifest();
    manifest.looks[0].select.clear();
    assert!(matches!(
        manifest.validate(),
        Err(CharacterManifestError::MissingLookPart { .. })
    ));
}
