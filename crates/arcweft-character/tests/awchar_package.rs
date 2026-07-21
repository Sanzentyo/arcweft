use arcweft_character::manifest::{
    CharacterAssetPath, CharacterManifest, registration::SourceBackedCharacterManifest,
};
use arcweft_character::package::{CharacterLayerPayload, CharacterPackage, CharacterPackageError};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

fn manifest() -> CharacterManifest {
    CharacterManifest::decode_runtime_json(include_str!(
        "fixtures/zundamon.awchar/character.awchar.json"
    ))
    .expect("fixture manifest")
}

fn payload(path: &str) -> CharacterLayerPayload {
    CharacterLayerPayload::new(
        CharacterAssetPath::try_new(path).expect("path"),
        match path {
            "layers/body--default.png" => {
                include_bytes!("fixtures/zundamon.awchar/layers/body--default.png").as_slice()
            }
            "layers/eyes--normal.png" => {
                include_bytes!("fixtures/zundamon.awchar/layers/eyes--normal.png").as_slice()
            }
            "layers/eyes--smile.png" => {
                include_bytes!("fixtures/zundamon.awchar/layers/eyes--smile.png").as_slice()
            }
            "layers/mouth--neutral.png" => {
                include_bytes!("fixtures/zundamon.awchar/layers/mouth--neutral.png").as_slice()
            }
            "layers/mouth--smile.png" => {
                include_bytes!("fixtures/zundamon.awchar/layers/mouth--smile.png").as_slice()
            }
            _ => &[0, 1, 2, 3],
        },
    )
}

#[test]
fn awchar_package_rejects_corrupt_png_payload() {
    let manifest = manifest();
    let mut payloads = all_payloads(&manifest);
    payloads[0] = CharacterLayerPayload::new(payloads[0].path().clone(), &b"not a PNG"[..]);

    let error = CharacterPackage::new(manifest, payloads).expect_err("invalid PNG");
    assert!(matches!(
        error,
        CharacterPackageError::InvalidLayerPng { .. }
    ));
}

#[test]
fn awchar_package_rejects_png_dimensions_that_disagree_with_manifest() {
    let source = include_str!("fixtures/zundamon.awchar/character.awchar.json").replacen(
        "\"width\": 96,\n            \"height\": 128",
        "\"width\": 95,\n            \"height\": 128",
        1,
    );
    let manifest = CharacterManifest::decode_runtime_json(&source).expect("mismatched manifest");
    let error = CharacterPackage::new(manifest.clone(), all_payloads(&manifest))
        .expect_err("dimension mismatch");

    assert!(matches!(
        error,
        CharacterPackageError::LayerDimensionsMismatch {
            expected_width: 95,
            actual_width: 96,
            ..
        }
    ));
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
fn source_backed_package_retains_exact_manifest_bytes_and_identity() {
    let source = include_str!("fixtures/zundamon.awchar/character.awchar.json");
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("zundamon-package").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap();
    let accepted = SourceBackedCharacterManifest::decode_registration_json(&document).unwrap();
    let package = CharacterPackage::from_source_backed_manifest(
        &document,
        &accepted,
        all_payloads(accepted.manifest()),
    )
    .unwrap();

    assert_eq!(package.manifest_bytes(), source.as_bytes());

    let other_document = SourceDocument::try_new(
        SourceDocumentId::try_new("other-zundamon-package").unwrap(),
        SourceName::Memory,
        source,
    )
    .unwrap();
    let error = CharacterPackage::from_source_backed_manifest(
        &other_document,
        &accepted,
        all_payloads(accepted.manifest()),
    )
    .expect_err("document identity mismatch");
    assert!(matches!(
        error,
        CharacterPackageError::ManifestSourceIdentityMismatch
    ));
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
