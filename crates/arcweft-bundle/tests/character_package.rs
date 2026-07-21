use arcweft_bundle::BundleCodecError;
use arcweft_bundle::character_package::BundleCharacterPackage;
use arcweft_character::manifest::{CharacterAssetPath, CharacterManifest};
use arcweft_character::package::{CharacterLayerPayload, CharacterPackage};

fn package() -> CharacterPackage {
    let manifest = CharacterManifest::decode_runtime_json(include_str!(
        "fixtures/zundamon.awchar/character.awchar.json"
    ))
    .expect("manifest");
    let payloads = manifest
        .parts()
        .iter()
        .flat_map(|part| {
            part.variants().iter().map(|variant| {
                let bytes: &[u8] = match variant.asset().as_str() {
                    "layers/body--default.png" => {
                        include_bytes!("fixtures/zundamon.awchar/layers/body--default.png")
                    }
                    "layers/eyes--normal.png" => {
                        include_bytes!("fixtures/zundamon.awchar/layers/eyes--normal.png")
                    }
                    "layers/eyes--smile.png" => {
                        include_bytes!("fixtures/zundamon.awchar/layers/eyes--smile.png")
                    }
                    "layers/mouth--neutral.png" => {
                        include_bytes!("fixtures/zundamon.awchar/layers/mouth--neutral.png")
                    }
                    "layers/mouth--smile.png" => {
                        include_bytes!("fixtures/zundamon.awchar/layers/mouth--smile.png")
                    }
                    path => panic!("unexpected fixture layer {path}"),
                };
                CharacterLayerPayload::new(
                    CharacterAssetPath::try_new(variant.asset().as_str()).expect("asset path"),
                    bytes.as_ref(),
                )
            })
        })
        .collect::<Vec<_>>();
    CharacterPackage::new(manifest, payloads).expect("package")
}

#[test]
fn bundle_character_package_emits_manifest_and_layers() {
    let package = package();
    let (bundle_package, files) =
        BundleCharacterPackage::from_character_package(&package, "characters/zundamon.awchar")
            .expect("bundle character package");

    assert_eq!(bundle_package.character, "character.zundamon");
    assert_eq!(bundle_package.layers.len(), 5);
    assert_eq!(files.len(), 6);
    bundle_package
        .validate_files(&files)
        .expect("valid bundle files");
}

#[test]
fn bundle_character_package_rejects_missing_layer_virtual_file() {
    let package = package();
    let (bundle_package, mut files) =
        BundleCharacterPackage::from_character_package(&package, "characters/zundamon.awchar")
            .expect("bundle character package");
    files.retain(|file| !file.path.ends_with("mouth--smile.png"));

    let error = bundle_package
        .validate_files(&files)
        .expect_err("missing layer file");
    assert!(matches!(
        error,
        BundleCodecError::MissingCharacterPackageFile { .. }
    ));
}

#[test]
fn missing_manifest_file_is_rejected() {
    let package = package();
    let (bundle_package, files) =
        BundleCharacterPackage::from_character_package(&package, "characters/zundamon.awchar")
            .expect("bundle character package");
    let files = files
        .into_iter()
        .filter(|file| file.path != bundle_package.manifest.path)
        .collect::<Vec<_>>();

    let error = bundle_package
        .validate_files(&files)
        .expect_err("missing manifest file");
    assert!(matches!(
        error,
        BundleCodecError::MissingCharacterPackageFile { .. }
    ));
}
