use arcweft_character::id::{CharacterId, CharacterLookId};
use arcweft_character::manifest::{CharacterAssetPath, CharacterManifest};
use arcweft_character::package::{CharacterLayerPayload, CharacterPackage};
use arcweft_player_scene::characters::BundleCharacterCatalog;

fn package() -> CharacterPackage {
    let manifest = CharacterManifest::decode_runtime_json(include_str!(
        "fixtures/zundamon.awchar/character.awchar.json"
    ))
    .expect("manifest");
    let payloads = [
        (
            "layers/body--default.png",
            include_bytes!("fixtures/zundamon.awchar/layers/body--default.png").as_slice(),
        ),
        (
            "layers/eyes--normal.png",
            include_bytes!("fixtures/zundamon.awchar/layers/eyes--normal.png").as_slice(),
        ),
        (
            "layers/eyes--smile.png",
            include_bytes!("fixtures/zundamon.awchar/layers/eyes--smile.png").as_slice(),
        ),
        (
            "layers/mouth--neutral.png",
            include_bytes!("fixtures/zundamon.awchar/layers/mouth--neutral.png").as_slice(),
        ),
        (
            "layers/mouth--smile.png",
            include_bytes!("fixtures/zundamon.awchar/layers/mouth--smile.png").as_slice(),
        ),
    ]
    .into_iter()
    .map(|(path, bytes)| {
        CharacterLayerPayload::new(
            CharacterAssetPath::try_new(path).expect("asset path"),
            bytes.to_vec(),
        )
    })
    .collect::<Vec<_>>();
    CharacterPackage::new(manifest, payloads).expect("package")
}

#[test]
fn selected_looks_prepare_without_flat_png_swaps() {
    let catalog = BundleCharacterCatalog::from_character_package(&package()).expect("catalog");
    let character = CharacterId::try_new("character.zundamon").expect("character");
    let normal = catalog
        .prepare(
            &character,
            Some(&CharacterLookId::try_new("normal").expect("look")),
        )
        .expect("normal");
    let smile = catalog
        .prepare(
            &character,
            Some(&CharacterLookId::try_new("smile").expect("look")),
        )
        .expect("smile");

    assert_eq!(normal.stable_bbox(), smile.stable_bbox());
    assert_eq!(normal.render_spec().layers().len(), 3);
    assert_eq!(smile.render_spec().layers().len(), 3);
    assert!(
        smile
            .render_spec()
            .layers()
            .iter()
            .any(|layer| layer.asset_path().as_str() == "layers/eyes--smile.png")
    );
    assert!(
        smile.render_spec().layers().iter().all(|layer| !layer
            .asset_path()
            .as_str()
            .ends_with("smile.png")
            || layer.part().as_str() != "body")
    );
}

#[test]
fn retained_view_layers_match_resolved_manifest_order() {
    let catalog = BundleCharacterCatalog::from_character_package(&package()).expect("catalog");
    let character = CharacterId::try_new("character.zundamon").expect("character");
    let frame = catalog.prepare(&character, None).expect("default frame");

    let parts = frame
        .view()
        .layers()
        .iter()
        .map(|layer| layer.part().as_str())
        .collect::<Vec<_>>();
    assert_eq!(parts, ["body", "eyes", "mouth"]);
}

#[test]
fn observe_reports_selected_character_look_and_stable_bbox() {
    let catalog = BundleCharacterCatalog::from_character_package(&package()).expect("catalog");
    let character = CharacterId::try_new("character.zundamon").expect("character");
    let frame = catalog
        .prepare(
            &character,
            Some(&CharacterLookId::try_new("smile").expect("look")),
        )
        .expect("smile frame");
    let observed = frame.observe_object();

    assert_eq!(observed.character, "character.zundamon");
    assert_eq!(observed.look, "smile");
    assert_eq!(observed.bbox, frame.stable_bbox());
    assert!(observed.capture_ref.contains("character.zundamon"));
    assert!(
        observed
            .layers
            .iter()
            .any(|layer| layer.part == "eyes" && layer.variant == "smile")
    );
}
