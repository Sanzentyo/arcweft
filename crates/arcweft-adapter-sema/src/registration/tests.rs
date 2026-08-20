use super::*;
use arcweft_adapter_context::manifest::{
    AdapterCallableGroupIndex, AdapterCallableName, AdapterCallableOverloadIndex,
    AdapterCallableParameterIndex, AdapterCallablePath, AdapterEffectCapability,
    AdapterFreeCallableKind, AdapterFunctionParam, AdapterFunctionSignature, AdapterManifest,
    AdapterNominalDeclaration, AdapterNominalPath, AdapterNominalPathPrefix,
    AdapterNominalPathSegment, AdapterNominalVisibility, AdapterOpaqueTypeProducerId,
    AdapterParameterGroup, AdapterParameterPassing, AdapterParameterPresence, AdapterSymbol,
    AdapterSymbolPath, AdapterSymbolSegment, AdapterToolingDoc, AdapterToolingParameterDoc,
    AdapterToolingSubject, AdapterTypeKind,
};
use arcweft_lang_sema::{
    env::{EffectCapability, TypeCheckEnv},
    registration::RegisteredExternalOwner,
};
use arcweft_rust_abi::{
    ArcweftRustFunction, ArcweftRustManifest, ArcweftRustPackage, ArcweftRustPackageId,
    ArcweftRustParam, ArcweftRustPurity, ArcweftRustTypeDecl, ArcweftRustTypeKind,
    ArcweftRustTypePath, ArcweftRustTypePathSegment, ArcweftRustTypeRef, ArcweftRustVariant,
    ArcweftRustVariantPayload,
};

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
    let semantic_registration = AdapterSemanticRegistration::new(&manifest);
    let env = semantic_registration.declare_effects(TypeCheckEnv::new());

    assert!(env.has_capability("fs.read"));
    assert!(
        env.available_effects().is_none(),
        "surface application must not select target availability"
    );
    let registration = semantic_registration
        .source_backed_facts(0)
        .expect("source-backed callable input succeeds");
    assert_eq!(registration.environment().callable_records().len(), 1);

    let target_env = semantic_registration.declare_target_effects(TypeCheckEnv::new());
    assert!(
        target_env
            .available_effects()
            .is_some_and(|effects| effects.contains(&EffectCapability::new("fs.read")))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one end-to-end publication test keeps the grouped signature, effects, and documentation assertions on the same record"
)]
fn source_backed_callable_input_preserves_groups_defaults_rest_effects_and_docs() {
    use arcweft_lang_sema::{
        callable::{
            AdapterPackageId, CallableParameterPassing, CallableParameterPresence,
            DocumentationProvenance, EnvironmentCallableOwner, SpreadArgumentPolicy,
        },
        effects::EffectId,
        registration::EnvironmentCallableLookupInput,
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
    let manifest = AdapterManifest::new("custom-network", "Custom Network")
        .with_function_signature(
            path,
            overload,
            signature,
            [AdapterEffectCapability::new("network.request")],
        )
        .with_tooling_doc(documentation);
    let registration = AdapterSemanticRegistration::new(&manifest)
        .source_backed_facts(1)
        .unwrap();

    assert_eq!(
        registration.environment().owner(),
        &EnvironmentCallableOwner::Adapter(AdapterPackageId::try_new("custom-network").unwrap())
    );
    let record = &registration.environment().callable_records()[0];
    let EnvironmentCallableLookupInput::Free(path) = record.key() else {
        panic!("free function publication must retain a typed path");
    };
    assert_eq!(
        path.path()
            .segments()
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

#[test]
fn source_backed_owner_is_derived_without_a_caller_selected_mode() {
    use arcweft_lang_sema::callable::{
        AdapterPackageId, EnvironmentCallableOwner, StandardEnvironmentId,
    };

    let standard_manifest = arcweft_adapter_context::standard::sans_io_manifest();
    let standard = AdapterSemanticRegistration::new(&standard_manifest)
        .source_backed_facts(0)
        .expect("reserved manifest ID has one fixed standard owner");
    assert_eq!(
        standard.environment().owner(),
        &EnvironmentCallableOwner::Standard(StandardEnvironmentId::SansIo)
    );

    let custom_manifest = AdapterManifest::new("not-sans-io", "Custom");
    let custom = AdapterSemanticRegistration::new(&custom_manifest)
        .source_backed_facts(1)
        .expect("custom manifest has an adapter owner");
    assert_eq!(
        custom.environment().owner(),
        &EnvironmentCallableOwner::Adapter(AdapterPackageId::try_new("not-sans-io").unwrap())
    );
}

#[test]
fn source_backed_standard_input_retains_rust_callable_identity_and_order() {
    use arcweft_lang_sema::callable::{
        EnvironmentCallableKind, EnvironmentCallableOwner, StandardEnvironmentId,
    };

    let rust = ArcweftRustManifest::new(ArcweftRustPackage {
        id: rust_package_id(),
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
    let base = arcweft_adapter_context::standard::inference_tensor_manifest();
    let expected_order = base.functions().len() + base.methods().len();
    let augmented = base
        .try_with_rust_package_mount(rust_package_id(), empty_rust_mount())
        .expect("Rust package mount is unique")
        .try_with_rust_manifest(&rust)
        .expect("Rust callable metadata augments the standard manifest");
    let registration = AdapterSemanticRegistration::new(&augmented)
        .source_backed_facts(2)
        .expect("source-backed standard registration input is valid");

    assert_eq!(
        registration.environment().owner(),
        &EnvironmentCallableOwner::Standard(StandardEnvironmentId::InferenceTensor)
    );
    let rust_records = registration
        .environment()
        .callable_records()
        .iter()
        .filter(|record| record.kind() == EnvironmentCallableKind::RustFunction)
        .collect::<Vec<_>>();
    assert_eq!(rust_records.len(), 1);
    assert_eq!(rust_records[0].declaration_order().get(), expected_order);
}

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
    let first = AdapterSemanticRegistration::new(&first_manifest)
        .source_backed_facts(7)
        .expect("first source-backed facts");
    let changed = AdapterSemanticRegistration::new(&changed_manifest)
        .source_backed_facts(7)
        .expect("changed source-backed facts");

    assert_eq!(
        first.document().identity().id().as_str(),
        "arcweft-generated://adapter-sema/7"
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
            if id.nominal_owner().as_str() == "adapter:fixture"
                && id.value_binding().as_str() == "adapter.viewport"
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
    let RegisteredExternalOwner::Environment(id) = first.externals()[0].target() else {
        panic!("adapter symbol must register an environment owner");
    };
    let binding = &first.environment().value_bindings()[0];
    assert_eq!(binding.id(), id.value_binding());
    assert!(matches!(
        binding.ty().kind(),
        arcweft_lang_sema::registration::EnvironmentTypeProjectionKind::I32
    ));
}

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
    let forward_manifest = AdapterManifest::new("fixture", "Fixture")
        .with_symbol(viewport.clone())
        .with_symbol(mode.clone());
    let forward = AdapterSemanticRegistration::new(&forward_manifest)
        .source_backed_facts(11)
        .expect("forward facts");
    let reverse_manifest = AdapterManifest::new("fixture", "Fixture")
        .with_symbol(mode)
        .with_symbol(viewport);
    let reverse = AdapterSemanticRegistration::new(&reverse_manifest)
        .source_backed_facts(11)
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
fn opaque_producer_is_source_backed_and_changes_manifest_identity() {
    fn manifest(producer: &str) -> AdapterManifest {
        let path = AdapterNominalPath::try_new([
            AdapterNominalPathSegment::try_new("Widget").expect("valid path segment")
        ])
        .expect("valid path");
        AdapterManifest::new("fixture", "Fixture")
            .try_with_nominal_declaration(
                AdapterNominalDeclaration::try_new(
                    path,
                    0,
                    AdapterOpaqueTypeProducerId::try_new(producer).expect("valid producer"),
                    AdapterNominalVisibility::Public,
                    "Widget",
                )
                .expect("valid declaration"),
            )
            .expect("unique declaration")
    }

    let left = AdapterSemanticRegistration::new(&manifest("fixture.adapter-sema.left"))
        .source_backed_facts(12)
        .expect("left facts");
    let right = AdapterSemanticRegistration::new(&manifest("fixture.adapter-sema.right"))
        .source_backed_facts(12)
        .expect("right facts");

    assert_ne!(
        left.environment().manifest_digest(),
        right.environment().manifest_digest()
    );
    assert_ne!(
        left.document().identity().revision(),
        right.document().identity().revision()
    );
    let nominal = &left.environment().nominal_inventory()[0];
    assert_eq!(
        nominal.runtime_producer().as_str(),
        "fixture.adapter-sema.left"
    );
    assert!(
        left.document()
            .text()
            .contains("producer=25:fixture.adapter-sema.left")
    );
}

#[test]
fn source_backed_adapter_facts_retain_exact_recursive_type_ranges() {
    use arcweft_lang_sema::registration::EnvironmentTypeProjectionKind;
    use arcweft_source::{SourceDocument, SourceSpan};

    fn source_text<'a>(document: &'a SourceDocument, span: &SourceSpan) -> &'a str {
        let range = span.range();
        &document.text()[range.start()..range.end()]
    }

    let symbol_type = AdapterTypeKind::Vec {
        item: Box::new(AdapterTypeKind::Option {
            item: Box::new(AdapterTypeKind::I32),
        }),
    };
    let parameter_type = AdapterTypeKind::Result {
        ok: Box::new(AdapterTypeKind::Vec {
            item: Box::new(AdapterTypeKind::String),
        }),
        error: Box::new(AdapterTypeKind::I32),
    };
    let result_type = AdapterTypeKind::Need {
        item: Box::new(AdapterTypeKind::Result {
            ok: Box::new(AdapterTypeKind::Seq {
                item: Box::new(AdapterTypeKind::U8),
            }),
            error: Box::new(AdapterTypeKind::String),
        }),
    };
    let manifest = AdapterManifest::new("fixture", "Fixture")
        .with_symbol(AdapterSymbol::new(
            adapter_symbol_path(["adapter", "values"]),
            symbol_type,
        ))
        .with_function_signature(
            adapter_path(["adapter", "read"]),
            adapter_overload(0),
            adapter_signature([("input", parameter_type)], result_type),
            [],
        );
    let facts = AdapterSemanticRegistration::new(&manifest)
        .source_backed_facts(19)
        .expect("source-backed facts");
    let document = facts.document();

    let symbol = facts.environment().value_bindings()[0].ty();
    assert_eq!(source_text(document, symbol.source()), "Vec<Option<i32>>");
    let EnvironmentTypeProjectionKind::Vec(option) = symbol.kind() else {
        panic!("symbol root must retain Vec shape");
    };
    assert_eq!(source_text(document, option.source()), "Option<i32>");
    let EnvironmentTypeProjectionKind::Option(integer) = option.kind() else {
        panic!("symbol child must retain Option shape");
    };
    assert_eq!(source_text(document, integer.source()), "i32");

    let callable = &facts.environment().callable_records()[0];
    let parameter = callable.schema().groups()[0].parameters()[0].ty();
    assert_eq!(
        source_text(document, parameter.source()),
        "Result<Vec<String>,i32>"
    );
    let arcweft_lang_sema::registration::EnvironmentParameterTypeInput::Exact(parameter) =
        parameter
    else {
        panic!("typed adapter parameter must not become unchecked")
    };
    let EnvironmentTypeProjectionKind::Result { ok, error } = parameter.kind() else {
        panic!("parameter root must retain Result shape");
    };
    assert_eq!(source_text(document, ok.source()), "Vec<String>");
    assert_eq!(source_text(document, error.source()), "i32");

    let result = callable.schema().result();
    assert_eq!(
        source_text(document, result.source()),
        "Need<Result<Seq<u8>,String>>"
    );
    let EnvironmentTypeProjectionKind::Need(item) = result.kind() else {
        panic!("result root must retain Need shape");
    };
    let EnvironmentTypeProjectionKind::Result { ok, error } = item.kind() else {
        panic!("fallible Need payload must retain Result shape");
    };
    assert_eq!(source_text(document, ok.source()), "Seq<u8>");
    assert_eq!(source_text(document, error.source()), "String");
}

#[test]
fn rust_manifest_publishes_source_backed_nominal_metadata_when_enabled() {
    let manifest = rank_rust_manifest();

    let context = AdapterManifest::new("fixture", "Fixture")
        .try_with_rust_package_mount(rust_package_id(), empty_rust_mount())
        .expect("Rust package mount is unique")
        .try_with_rust_manifest(&manifest)
        .expect("Rust callable metadata is typed");
    let registration = AdapterSemanticRegistration::new(&context)
        .source_backed_facts(3)
        .expect("Rust metadata has source-backed registration facts");

    let [metadata] = registration.environment().rust_metadata() else {
        panic!("one Rust nominal metadata record is retained")
    };
    assert_eq!(metadata.package().as_str(), "truck_game");
    assert_eq!(metadata.id().canonical_path().canonical_string(), "Rank");
    let arcweft_lang_sema::env::rust_metadata::RustTypeMetadataPublicationKind::Enum { variants } =
        metadata.kind()
    else {
        panic!("Rank remains typed enum metadata")
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name(), "Bronze");
    assert!(matches!(
        variants[0].payload(),
        arcweft_lang_sema::env::rust_metadata::RustVariantPayloadInput::Unit
    ));
    assert_eq!(variants[1].name(), "Custom");
    let arcweft_lang_sema::env::rust_metadata::RustVariantPayloadInput::Record(fields) =
        variants[1].payload()
    else {
        panic!("Custom retains its record payload")
    };
    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].0, "label");
    assert!(matches!(
        fields[0].1.kind(),
        arcweft_lang_sema::registration::EnvironmentTypeProjectionKind::String
    ));
}

fn adapter_overload(value: usize) -> AdapterCallableOverloadIndex {
    AdapterCallableOverloadIndex::try_from_usize(value).expect("test overload fits")
}

fn adapter_path<const N: usize>(segments: [&str; N]) -> AdapterCallablePath {
    AdapterCallablePath::try_new(
        segments
            .into_iter()
            .map(|segment| AdapterCallableName::try_new(segment).expect("valid test segment")),
    )
    .expect("test path is non-empty")
}

fn adapter_symbol_path<const N: usize>(segments: [&str; N]) -> AdapterSymbolPath {
    AdapterSymbolPath::try_new(segments.map(|segment| {
        AdapterSymbolSegment::try_new(segment).expect("valid test adapter symbol segment")
    }))
    .expect("test adapter symbol path is non-empty")
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

fn rank_rust_manifest() -> ArcweftRustManifest {
    ArcweftRustManifest::new(ArcweftRustPackage {
        id: rust_package_id(),
        version: "0.1.0".to_owned(),
        metadata_hash: None,
    })
    .with_type(ArcweftRustTypeDecl {
        path: rust_type_path("Rank"),
        rust_path: "truck_game::Rank".to_owned(),
        opaque_producer: arcweft_rust_abi::ArcweftRustOpaqueTypeProducerId::try_new(
            "fixture.adapter-sema.rust",
        )
        .expect("fixture producer is valid"),
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
