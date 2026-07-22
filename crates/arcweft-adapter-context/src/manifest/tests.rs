use super::*;
#[cfg(feature = "sema")]
use arcweft_lang_sema::registration::RegisteredExternalOwner;
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustParam,
    ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind, ArcweftRustTypeRef,
    ArcweftRustVariant,
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

#[cfg(feature = "sema")]
#[test]
fn adapter_manifest_applies_effect_capabilities_and_function_effects() {
    let manifest = AdapterManifest::new("fixture", "Fixture")
        .with_effect(AdapterEffectCapability::new("fs.read"))
        .with_function_signature(
            adapter_path(["adapter", "read_text"]),
            adapter_overload(0),
            adapter_signature([("path", AdapterTypeKind::String)], AdapterTypeKind::String),
            [AdapterEffectCapability::new("fs.read")],
        );
    let env = manifest.apply_to_env(TypeCheckEnv::new());

    assert!(env.has_capability("fs.read"));
    assert!(
        env.available_effects().is_none(),
        "surface application must not select target availability"
    );
    let publication = manifest
        .try_callable_publication(
            crate::publication::AdapterManifestSource::SelectedAdapter,
            &arcweft_lang_sema::callable::PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("typed callable publication succeeds");
    assert_eq!(publication.records().len(), 1);

    let target_env = manifest.apply_to_target_env(TypeCheckEnv::new());
    assert!(
        target_env
            .available_effects()
            .is_some_and(|effects| effects.contains(&EffectCapability::new("fs.read")))
    );
}

#[cfg(feature = "sema")]
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end publication test keeps the grouped signature, effects, and documentation assertions on the same record"
)]
fn callable_publication_preserves_groups_defaults_rest_effects_and_docs() {
    use arcweft_lang_sema::{
        callable::{
            AdapterPackageId, CallableLookupKey, CallableParameterPassing,
            CallableParameterPresence, DocumentationProvenance, EnvironmentCallableOwner,
            SpreadArgumentPolicy,
        },
        effects::EffectId,
    };

    let path = adapter_path(["network", "request"]);
    let overload = adapter_overload(0);
    let first = AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(0).unwrap(),
        Some(AdapterCallableName::try_new("url").unwrap()),
        AdapterTypeKind::String,
        AdapterParameterPassing::PositionalOrNamed,
        AdapterParameterPresence::Defaulted,
    )
    .unwrap();
    let rest = AdapterFunctionParam::try_new(
        AdapterCallableParameterIndex::try_from_usize(0).unwrap(),
        Some(AdapterCallableName::try_new("headers").unwrap()),
        AdapterTypeKind::String,
        AdapterParameterPassing::RestNamed,
        AdapterParameterPresence::Required,
    )
    .unwrap();
    let signature = AdapterFunctionSignature::try_new(
        vec![
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(0).unwrap(),
                vec![first],
            )
            .unwrap(),
            AdapterParameterGroup::try_new(
                AdapterCallableGroupIndex::try_from_usize(1).unwrap(),
                vec![rest],
            )
            .unwrap(),
        ],
        AdapterTypeKind::String,
    )
    .unwrap();
    let subject = AdapterToolingSubject::Free {
        kind: AdapterFreeCallableKind::Function,
        path: path.clone(),
        overload,
    };
    let documentation = AdapterToolingDoc::try_new(
        subject,
        Some("Sends a request.".to_owned()),
        Some("Uses the selected host adapter.".to_owned()),
        vec![
            AdapterToolingParameterDoc::try_new(
                AdapterCallableGroupIndex::try_from_usize(1).unwrap(),
                AdapterCallableParameterIndex::try_from_usize(0).unwrap(),
                "Additional named headers.",
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let publication = AdapterManifest::new("custom-network", "Custom Network")
        .with_function_signature(
            path,
            overload,
            signature,
            [AdapterEffectCapability::new("network.request")],
        )
        .with_tooling_doc(documentation)
        .try_callable_publication(
            crate::publication::AdapterManifestSource::SelectedAdapter,
            &arcweft_lang_sema::callable::PRODUCTION_CALLABLE_LIMITS,
        )
        .unwrap();

    assert_eq!(
        publication.owner(),
        &EnvironmentCallableOwner::Adapter(AdapterPackageId::try_new("custom-network").unwrap())
    );
    let record = &publication.records()[0];
    let CallableLookupKey::Free(path) = record.key() else {
        panic!("free function publication must retain a typed path");
    };
    assert_eq!(
        path.segments()
            .iter()
            .map(arcweft_lang_sema::callable::CallableName::as_str)
            .collect::<Vec<_>>(),
        ["network", "request"]
    );
    assert_eq!(record.schema().groups().len(), 2);
    assert_eq!(
        record.schema().groups()[0].parameters()[0].presence(),
        CallableParameterPresence::Defaulted
    );
    assert_eq!(
        record.schema().groups()[1].parameters()[0].passing(),
        CallableParameterPassing::RestNamed
    );
    assert_eq!(
        record.schema().argument_policy().spread(),
        SpreadArgumentPolicy::TypedRest
    );
    assert!(
        record
            .schema()
            .effects()
            .declared()
            .concrete()
            .contains(&EffectId::parse("network.request").unwrap())
    );
    assert_eq!(record.documentation().summary(), Some("Sends a request."));
    assert_eq!(
        record.documentation().parameter(
            arcweft_lang_sema::callable::CallableGroupIndex::try_from_usize(1).unwrap(),
            arcweft_lang_sema::callable::CallableParameterIndex::try_from_usize(0).unwrap(),
        ),
        Some("Additional named headers.")
    );
    assert!(matches!(
        record.documentation().provenance(),
        DocumentationProvenance::AdapterTooling { package }
            if package.as_str() == "custom-network"
    ));
}

#[cfg(feature = "sema")]
#[test]
fn callable_publication_rejects_reserved_and_mismatched_standard_owners() {
    use crate::publication::{AdapterCallablePublicationError, AdapterManifestSource};
    use arcweft_lang_sema::callable::{PRODUCTION_CALLABLE_LIMITS, StandardEnvironmentId};

    let reserved = AdapterManifest::new(crate::standard::SANS_IO_ADAPTER_ID, "Reserved")
        .try_callable_publication(
            AdapterManifestSource::SelectedAdapter,
            &PRODUCTION_CALLABLE_LIMITS,
        );
    assert!(matches!(
        reserved,
        Err(AdapterCallablePublicationError::ReservedStandardIdClaimed { .. })
    ));

    let mismatch = AdapterManifest::new("not-sans-io", "Mismatch").try_callable_publication(
        AdapterManifestSource::Standard(StandardEnvironmentId::SansIo),
        &PRODUCTION_CALLABLE_LIMITS,
    );
    assert!(matches!(
        mismatch,
        Err(AdapterCallablePublicationError::StandardIdMismatch {
            source: StandardEnvironmentId::SansIo,
            ..
        })
    ));
}

#[cfg(feature = "sema")]
#[test]
fn rust_callable_publication_is_a_typed_delta_for_augmented_standard_manifest() {
    use crate::publication::AdapterManifestSource;
    use arcweft_lang_sema::callable::{
        EnvironmentCallableKind, EnvironmentCallableOwner, PRODUCTION_CALLABLE_LIMITS,
        StandardEnvironmentId,
    };

    let rust = ArcweftRustManifest::new(ArcweftRustPackage {
        name: "truck_game".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_function(ArcweftRustFunction {
        name: "score_to_rank".to_owned(),
        rust_path: "truck_game::score_to_rank".to_owned(),
        params: vec![ArcweftRustParam {
            name: "score".to_owned(),
            ty: ArcweftRustTypeRef::I32,
        }],
        return_type: ArcweftRustTypeRef::String,
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    });
    let base = crate::standard::inference_tensor_manifest();
    let expected_order = base.functions().len() + base.methods().len();
    let augmented = base
        .try_with_rust_manifest(&rust)
        .expect("Rust callable metadata augments the standard manifest");
    let publication = augmented
        .try_rust_callable_publication(
            AdapterManifestSource::Standard(StandardEnvironmentId::InferenceTensor),
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("Rust delta publication is valid");

    assert_eq!(
        publication.owner(),
        &EnvironmentCallableOwner::Standard(StandardEnvironmentId::InferenceTensor)
    );
    assert_eq!(publication.records().len(), 1);
    assert_eq!(
        publication.records()[0].kind(),
        EnvironmentCallableKind::RustFunction
    );
    assert_eq!(
        publication.records()[0].declaration_order().get(),
        expected_order
    );
}

#[cfg(feature = "sema")]
#[test]
fn source_backed_adapter_facts_bind_exact_environment_keys_and_base_revision() {
    let first_manifest =
        AdapterManifest::new("fixture", "Fixture").with_symbol(AdapterSymbol::new(
            adapter_symbol_path(["adapter", "viewport"]),
            AdapterTypeKind::I32,
        ));
    let changed_manifest =
        AdapterManifest::new("fixture", "Fixture").with_symbol(AdapterSymbol::new(
            adapter_symbol_path(["adapter", "viewport"]),
            AdapterTypeKind::I64,
        ));
    let first = first_manifest
        .source_backed_registration_facts(7)
        .expect("first source-backed facts");
    let changed = changed_manifest
        .source_backed_registration_facts(7)
        .expect("changed source-backed facts");

    assert_eq!(
        first.document().identity().id().as_str(),
        "arcweft-generated://adapter-context/7"
    );
    assert_ne!(
        first.document().identity().revision(),
        changed.document().identity().revision(),
        "a base-environment type change must change the complete fact revision"
    );
    assert_eq!(first.externals().len(), 1);
    assert!(matches!(
        first.externals()[0].target(),
        RegisteredExternalOwner::Environment(id)
            if id.as_str() == "adapter.viewport"
    ));
    assert_eq!(
        first.externals()[0].declaration().direct_bindings()[0]
            .path()
            .segments()
            .iter()
            .map(arcweft_lang_syntax::ast::symbol_path::ProjectSymbolSegment::as_str)
            .collect::<Vec<_>>(),
        ["adapter", "viewport"]
    );
    let base = first_manifest.apply_to_env(TypeCheckEnv::new());
    let RegisteredExternalOwner::Environment(id) = first.externals()[0].target() else {
        panic!("adapter symbol must register an environment owner");
    };
    assert_eq!(base.environment_binding(id), Some(&TypeKind::I32));
}

#[cfg(feature = "sema")]
#[test]
fn source_backed_adapter_facts_are_independent_of_symbol_insertion_order() {
    let viewport = AdapterSymbol::new(
        adapter_symbol_path(["adapter", "viewport"]),
        AdapterTypeKind::I32,
    );
    let mode = AdapterSymbol::new(
        adapter_symbol_path(["adapter", "mode"]),
        AdapterTypeKind::String,
    );
    let forward = AdapterManifest::new("fixture", "Fixture")
        .with_symbol(viewport.clone())
        .with_symbol(mode.clone())
        .source_backed_registration_facts(11)
        .expect("forward facts");
    let reverse = AdapterManifest::new("fixture", "Fixture")
        .with_symbol(mode)
        .with_symbol(viewport)
        .source_backed_registration_facts(11)
        .expect("reverse facts");

    assert_eq!(forward.document().text(), reverse.document().text());
    assert_eq!(
        forward.document().identity().revision(),
        reverse.document().identity().revision()
    );
    assert_eq!(forward.externals(), reverse.externals());
    assert_eq!(
        forward
            .externals()
            .iter()
            .map(|fact| fact.declaration().direct_bindings()[0].path().to_string())
            .collect::<Vec<_>>(),
        ["adapter.mode", "adapter.viewport"]
    );
}

#[test]
fn rust_manifest_injects_full_function_signature() {
    let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
        name: "truck_game".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        name: "Rank".to_owned(),
        rust_path: "truck_game::Rank".to_owned(),
        kind: ArcweftRustTypeKind::Enum {
            variants: vec![
                ArcweftRustVariant {
                    name: "Bronze".to_owned(),
                    fields: Vec::new(),
                },
                ArcweftRustVariant {
                    name: "Custom".to_owned(),
                    fields: vec![arcweft_rust_abi::ArcweftRustField {
                        name: "label".to_owned(),
                        ty: ArcweftRustTypeRef::String,
                    }],
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
        return_type: ArcweftRustTypeRef::Named {
            name: "Rank".to_owned(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    });

    let context = AdapterManifest::new("fixture", "Fixture")
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
        &adapter_signature(
            [("score", AdapterTypeKind::I32)],
            AdapterTypeKind::Named("Rank".to_owned())
        )
    );
}

#[cfg(feature = "sema")]
#[test]
fn rust_manifest_applies_to_semantic_env_when_enabled() {
    let manifest = ArcweftRustManifest::new(ArcweftRustPackage {
        name: "truck_game".to_owned(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        name: "Rank".to_owned(),
        rust_path: "truck_game::Rank".to_owned(),
        kind: ArcweftRustTypeKind::Enum {
            variants: vec![
                ArcweftRustVariant {
                    name: "Bronze".to_owned(),
                    fields: Vec::new(),
                },
                ArcweftRustVariant {
                    name: "Custom".to_owned(),
                    fields: vec![arcweft_rust_abi::ArcweftRustField {
                        name: "label".to_owned(),
                        ty: ArcweftRustTypeRef::String,
                    }],
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
        return_type: ArcweftRustTypeRef::Named {
            name: "Rank".to_owned(),
        },
        purity: ArcweftRustPurity::Pure,
        effects: Vec::new(),
    });

    let env = AdapterManifest::new("fixture", "Fixture")
        .try_with_rust_manifest(&manifest)
        .expect("Rust callable metadata is typed")
        .apply_to_env(TypeCheckEnv::new());

    assert_eq!(
        env,
        TypeCheckEnv::new()
            .try_with_rust_type_export(
                RustPackageId::try_new("truck_game").expect("package id"),
                rust_type_path("Rank"),
            )
            .expect("Rust type export")
            .try_with_enum_variants(TypeKind::Named("Rank".to_owned()), ["Bronze"])
            .expect("non-character Rust enum variants are accepted"),
        "only non-callable Rust type metadata stays on the existing environment route"
    );
}

#[cfg(feature = "sema")]
fn adapter_overload(value: usize) -> AdapterCallableOverloadIndex {
    AdapterCallableOverloadIndex::try_from_usize(value).expect("test overload fits")
}

#[cfg(feature = "sema")]
fn adapter_path<const N: usize>(segments: [&str; N]) -> AdapterCallablePath {
    AdapterCallablePath::try_new(
        segments
            .into_iter()
            .map(|segment| AdapterCallableName::try_new(segment).expect("valid test segment")),
    )
    .expect("test path is non-empty")
}

#[cfg(feature = "sema")]
fn adapter_symbol_path<const N: usize>(segments: [&str; N]) -> AdapterSymbolPath {
    AdapterSymbolPath::try_new(segments.map(|segment| {
        AdapterSymbolSegment::try_new(segment).expect("valid test adapter symbol segment")
    }))
    .expect("test adapter symbol path is non-empty")
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
