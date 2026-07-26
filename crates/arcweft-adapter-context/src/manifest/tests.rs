use super::*;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId,
    ArcweftRustParam, ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind,
    ArcweftRustTypePath, ArcweftRustTypePathSegment, ArcweftRustTypeRef, ArcweftRustVariant,
    ArcweftRustVariantPayload,
};

#[test]
fn checked_registry_insertion_rejects_duplicate_adapter_ids() {
    let registry = AdapterRegistry::new()
        .try_with_manifest(AdapterManifest::new("fixture", "First"))
        .expect("first manifest is unique");

    assert!(matches!(
        registry.try_with_manifest(AdapterManifest::new("fixture", "Second")),
        Err(AdapterRegistryError::DuplicateId { id }) if id.as_str() == "fixture"
    ));
}

#[test]
fn rust_manifest_injects_full_function_signature() {
    let manifest = rank_rust_manifest();

    let context = AdapterManifest::new("fixture", "Fixture")
        .try_with_rust_package_mount(rust_package_id(), empty_rust_mount())
        .expect("Rust package mount is unique")
        .try_with_rust_manifest(&manifest)
        .expect("Rust callable metadata is typed");

    assert_eq!(context.rust_functions().len(), 1);
    assert_eq!(
        context.rust_functions()[0].signature().groups()[0]
            .parameters()
            .len(),
        1
    );
    assert_eq!(
        context.rust_functions()[0].signature(),
        &adapter_signature([("score", AdapterTypeKind::I32)], rank_adapter_type())
    );
}

fn rust_package_id() -> ArcweftRustPackageId {
    ArcweftRustPackageId::try_new("truck_game").expect("valid test package ID")
}

fn rust_type_path(name: &str) -> ArcweftRustTypePath {
    ArcweftRustTypePath::try_new([
        ArcweftRustTypePathSegment::try_new(name).expect("valid test Rust type segment")
    ])
    .expect("one segment forms a Rust type path")
}

fn empty_rust_mount() -> AdapterNominalPathPrefix {
    AdapterNominalPathPrefix::try_new([]).expect("empty Rust mount is valid")
}

fn rank_adapter_path() -> AdapterNominalPath {
    empty_rust_mount()
        .join(&rust_type_path("Rank"))
        .expect("Rank path joins the package mount")
}

fn rank_adapter_type() -> AdapterTypeKind {
    AdapterTypeKind::Nominal {
        nominal: AdapterNominalTypeRef::try_new(
            AdapterNominalOwner::RustPackage {
                package: rust_package_id(),
            },
            rank_adapter_path(),
            [],
        )
        .expect("Rank nominal reference is valid"),
    }
}

fn rank_rust_manifest() -> ArcweftRustManifest {
    ArcweftRustManifest::new(ArcweftRustPackage {
        id: rust_package_id(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        path: rust_type_path("Rank"),
        rust_path: "truck_game::Rank".to_owned(),
        parameters: Vec::new(),
        kind: ArcweftRustTypeKind::Enum {
            variants: vec![
                ArcweftRustVariant {
                    name: "Bronze".to_owned(),
                    payload: ArcweftRustVariantPayload::Unit,
                },
                ArcweftRustVariant {
                    name: "Custom".to_owned(),
                    payload: ArcweftRustVariantPayload::Record {
                        fields: vec![arcweft_rust_abi::ArcweftRustField {
                            name: "label".to_owned(),
                            ty: ArcweftRustTypeRef::String,
                        }],
                    },
                },
            ],
        },
    })
    .with_function(ArcweftRustFunction {
        name: "score_to_rank".to_owned(),
        rust_path: "truck_game::score_to_rank".to_owned(),
        params: vec![ArcweftRustParam {
            name: "score".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::Nominal {
            package: rust_package_id(),
            path: rust_type_path("Rank"),
            arguments: Vec::new(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
}

fn adapter_signature<const N: usize>(
    parameters: [(&str, AdapterTypeKind); N],
    result: AdapterTypeKind,
) -> AdapterFunctionSignature {
    let parameters = parameters
        .into_iter()
        .enumerate()
        .map(|(index, (name, ty))| {
            AdapterFunctionParam::try_new(
                AdapterCallableParameterIndex::try_from_usize(index)
                    .expect("test parameter index fits"),
                Some(AdapterCallableName::try_new(name).expect("valid test parameter name")),
                ty,
                AdapterParameterPassing::PositionalOrNamed,
                AdapterParameterPresence::Required,
            )
            .expect("test parameter is valid")
        })
        .collect();
    AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0).expect("initial group index fits"),
                parameters,
            )
            .expect("test initial group is valid"),
        ],
        result,
    )
    .expect("test signature is valid")
}
