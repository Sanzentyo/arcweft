use arcweft_character::manifest::{CharacterAssetPath, CharacterManifest};
use arcweft_character::package::{CharacterLayerPayload, CharacterPackage, CharacterPackageError};

fn manifest() -> CharacterManifest {
    CharacterManifest::from_json(include_str!(
        "fixtures/zundamon.awchar/character.awchar.json"
    ))
    .expect("fixture manifest")
}

fn payload(path: &str) -> CharacterLayerPayload {
    CharacterLayerPayload::new(
        CharacterAssetPath::try_new(path).expect("path"),
        vec![0, 1, 2, 3],
    )
}

fn all_payloads(manifest: &CharacterManifest) -> Vec<CharacterLayerPayload> {
    manifest
        .parts()
        .iter()
        .flat_map(|part| {
            part.variants()
                .iter()
                .map(|variant| payload(variant.asset().as_str()))
        })
        .collect()
}

#[test]
fn awchar_package_accepts_every_manifest_referenced_layer() {
    let manifest = manifest();
    let package =
        CharacterPackage::new(manifest.clone(), all_payloads(&manifest)).expect("package");

    assert_eq!(
        package.manifest().character().as_str(),
        "character.zundamon"
    );
    assert!(
        std::str::from_utf8(package.manifest_bytes())
            .expect("manifest utf8")
            .contains("character.zundamon")
    );
    assert_eq!(package.layer_payloads().len(), 5);
}

#[test]
fn awchar_package_rejects_missing_layer_payloads() {
    let manifest = manifest();
    let mut payloads = all_payloads(&manifest);
    payloads.pop();

    let error = CharacterPackage::new(manifest, payloads).expect_err("missing payload");
    assert!(matches!(
        error,
        CharacterPackageError::MissingLayerPayload { .. }
    ));
}

#[test]
fn awchar_package_rejects_unreferenced_layer_payloads() {
    let manifest = manifest();
    let mut payloads = all_payloads(&manifest);
    payloads.push(payload("layers/not-in-manifest.png"));

    let error = CharacterPackage::new(manifest, payloads).expect_err("unreferenced payload");
    assert!(matches!(
        error,
        CharacterPackageError::UnreferencedLayerPayload(_)
    ));
}
