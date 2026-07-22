use super::*;

fn package(value: &str) -> ArcweftRustPackageId {
    ArcweftRustPackageId::try_new(value).expect("valid package")
}

fn path(segments: &[&str]) -> ArcweftRustTypePath {
    ArcweftRustTypePath::try_new(
        segments
            .iter()
            .map(|segment| ArcweftRustTypePathSegment::try_new(*segment).expect("valid segment")),
    )
    .expect("non-empty path")
}

fn manifest() -> ArcweftRustManifest {
    let package_id = package("truck_game");
    ArcweftRustManifest::new(ArcweftRustPackage {
        id: package_id.clone(),
        version: "0.1.0".to_owned(),
        metadata_hash: Some("fixture-hash".to_owned()),
    })
    .with_type(ArcweftRustTypeDecl {
        path: path(&["model", "Rank"]),
        rust_path: "truck_game::model::Rank".to_owned(),
        parameters: Vec::new(),
        kind: ArcweftRustTypeKind::Enum {
            variants: vec![ArcweftRustVariant {
                name: "Gold".to_owned(),
                payload: ArcweftRustVariantPayload::Unit,
            }],
        },
    })
    .with_function(ArcweftRustFunction {
        name: "mini_games.truck.score_to_rank".to_owned(),
        rust_path: "truck_game::score_to_rank".to_owned(),
        params: vec![ArcweftRustParam {
            name: "score".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::Nominal {
            package: package_id,
            path: path(&["model", "Rank"]),
            arguments: Vec::new(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    })
}

#[test]
fn json_round_trip_preserves_typed_nominal_identity() {
    let manifest = manifest();
    let json = manifest.to_json_pretty().expect("valid JSON");
    let repeated = manifest.to_json_pretty().expect("repeated valid JSON");
    let decoded = ArcweftRustManifest::from_json(&json).expect("valid manifest");

    assert_eq!(repeated, json, "pretty JSON must be byte deterministic");
    assert_eq!(decoded, manifest);
    assert_eq!(decoded.package.id.as_str(), "truck_game");
    assert!(!json.contains("D:\\"));
    assert!(!json.contains("/tmp/"));
}

#[test]
fn nominal_arguments_and_nested_composites_round_trip() {
    let nested = ArcweftRustTypeRef::Option {
        item: Box::new(ArcweftRustTypeRef::Result {
            ok: Box::new(ArcweftRustTypeRef::Tuple {
                items: vec![
                    ArcweftRustTypeRef::Vec {
                        item: Box::new(ArcweftRustTypeRef::I32),
                    },
                    ArcweftRustTypeRef::Seq {
                        item: Box::new(ArcweftRustTypeRef::String),
                    },
                ],
            }),
            error: Box::new(ArcweftRustTypeRef::Nominal {
                package: package("errors"),
                path: path(&["Failure"]),
                arguments: vec![ArcweftRustTypeRef::U32],
            }),
        }),
    };
    let json = serde_json::to_string(&nested).expect("type encodes");
    let decoded: ArcweftRustTypeRef = serde_json::from_str(&json).expect("type decodes");
    assert_eq!(decoded, nested);
}

#[test]
fn declaration_parameter_indices_are_contiguous_and_bound() {
    let mut manifest = manifest();
    manifest.types = vec![ArcweftRustTypeDecl {
        path: path(&["Envelope"]),
        rust_path: "truck_game::Envelope".to_owned(),
        parameters: vec![ArcweftRustTypeParameter {
            index: ArcweftRustTypeParameterIndex::try_from_usize(0).expect("index"),
            name: ArcweftRustTypeParameterName::try_new("T").expect("name"),
        }],
        kind: ArcweftRustTypeKind::Struct {
            shape: ArcweftRustStructShape::Record {
                fields: vec![ArcweftRustField {
                    name: "value".to_owned(),
                    ty: ArcweftRustTypeRef::TypeParameter {
                        index: ArcweftRustTypeParameterIndex::try_from_usize(0).expect("index"),
                    },
                }],
            },
        },
    }];
    manifest.functions.clear();
    assert_eq!(manifest.validate(ArcweftRustAbiLimits::PRODUCTION), Ok(()));

    manifest.types[0].parameters[0].index =
        ArcweftRustTypeParameterIndex::try_from_usize(1).expect("index");
    assert!(matches!(
        manifest.validate(ArcweftRustAbiLimits::PRODUCTION),
        Err(ArcweftRustManifestError::NonContiguousTypeParameterIndex { .. })
    ));
}

#[test]
fn duplicate_package_local_paths_fail_validation() {
    let mut manifest = manifest();
    manifest.types.push(manifest.types[0].clone());
    assert!(matches!(
        manifest.validate(ArcweftRustAbiLimits::PRODUCTION),
        Err(ArcweftRustManifestError::DuplicateTypePath {
            first: 0,
            duplicate: 1,
            ..
        })
    ));
}

#[test]
fn type_graph_limits_are_bounded_and_structured() {
    let mut ty = ArcweftRustTypeRef::I32;
    for _ in 0..3 {
        ty = ArcweftRustTypeRef::Option { item: Box::new(ty) };
    }
    let mut manifest = manifest();
    manifest.functions[0].return_type = ty;
    assert!(matches!(
        manifest.validate(ArcweftRustAbiLimits::new(16, 3, 8, 8)),
        Err(ArcweftRustManifestError::RecursiveDepthLimit {
            observed: 4,
            maximum: 3,
            ..
        })
    ));

    manifest.functions[0].return_type = ArcweftRustTypeRef::Tuple {
        items: vec![ArcweftRustTypeRef::I32; 3],
    };
    assert!(matches!(
        manifest.validate(ArcweftRustAbiLimits::new(16, 8, 2, 8)),
        Err(ArcweftRustManifestError::GenericArgumentLimit {
            observed: 3,
            maximum: 2,
            ..
        })
    ));
}

#[test]
fn callable_type_parameters_fail_closed() {
    let mut manifest = manifest();
    manifest.functions[0].return_type = ArcweftRustTypeRef::TypeParameter {
        index: ArcweftRustTypeParameterIndex::try_from_usize(0).expect("index"),
    };
    assert!(matches!(
        manifest.validate(ArcweftRustAbiLimits::PRODUCTION),
        Err(ArcweftRustManifestError::FreeTypeParameterInCallable { index: 0, .. })
    ));
}

#[test]
fn display_is_presentation_only_and_preserves_shapes() {
    let package = package("game");
    let nominal = ArcweftRustTypeRef::Nominal {
        package,
        path: path(&["model", "Rank"]),
        arguments: vec![ArcweftRustTypeRef::String],
    };
    let tuple = ArcweftRustTypeDecl {
        path: path(&["Point"]),
        rust_path: "game::Point".to_owned(),
        parameters: Vec::new(),
        kind: ArcweftRustTypeKind::Struct {
            shape: ArcweftRustStructShape::Tuple {
                fields: vec![ArcweftRustTypeRef::I32, ArcweftRustTypeRef::I32],
            },
        },
    };

    assert_eq!(nominal.to_string(), "game::model::Rank<String>");
    assert_eq!(tuple.to_string(), "struct Point(i32, i32)");
}
