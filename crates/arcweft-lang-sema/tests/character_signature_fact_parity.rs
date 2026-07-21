use std::sync::{Arc, atomic::AtomicBool};

use arcweft_character::{
    id::{CharacterId, CharacterLookId, CharacterPartId, CharacterVariantId},
    manifest::{
        CharacterAssetPath, CharacterBlendMode, CharacterCanvas, CharacterLook, CharacterManifest,
        CharacterPart, CharacterPartSelection, CharacterPoint, CharacterRect, CharacterVariant,
        registration::{
            CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
        },
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolWorldId,
    },
};
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallTargetFact, CallTargetFacts, CallableArgumentPolicy,
        CallableCandidateId, CallableDocumentation, CallableEffectSchema, CallableGroupIndex,
        CallableGroupKind, CallableLookupKey, CallableName, CallableOverloadIndex,
        CallableParameter, CallableParameterGroup, CallableParameterIndex,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallablePath,
        CallableSignatureSchema, CallableValidator, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentCallablePublication,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
        PRODUCTION_CALLABLE_LIMITS, RustCallableProvenance, RustCallablePurity, RustItemPath,
        RustPackageProvenance, SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    checker::{TypeCheckReport, TypeExpressionId, analyze_registered_project_types},
    effect_row::EffectRow,
    effects::EffectSet,
    env::TypeCheckEnv,
    registration::{
        CharacterRegistrar, CharacterRegistrationRequest, ExternalRegistrationFact,
        ProjectRegistrationFacts, RegisteredExternalOwner, RegisteredSemanticWorld,
    },
    signature::{
        SignatureNotApplicable, SignatureQuery, SignatureQueryControl, SignatureQueryOutcome,
        query_signature,
    },
    types::TypeKind,
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSpan};

struct SignatureFixture {
    document: Arc<SourceDocument>,
    project: HirProject,
    world: RegisteredSemanticWorld,
}

impl SignatureFixture {
    fn new(
        name: &str,
        source: &str,
        manifests: &[CharacterManifest],
        publications: Vec<EnvironmentCallablePublication>,
    ) -> Self {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("memory:///{name}.arcw")).expect("source ID"),
                SourceName::path(format!("memory:///{name}.arcw")),
                source,
            )
            .expect("source document"),
        );
        let parsed = parse_source(source);
        assert!(
            parsed.errors().is_empty(),
            "signature parity fixture must parse: {:?}",
            parsed.errors()
        );
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
        let package = CallablePackageId::try_new(name).expect("package");
        let project = HirProject::new(
            package.as_str(),
            [HirProjectModule::try_new(
                CanonicalModulePath::crate_root(),
                document.identity().clone(),
                hir,
            )
            .expect("root module")],
        )
        .expect("HIR project");
        let world_id = ProjectSymbolWorldId::try_new(
            package,
            document.identity().id().clone(),
            "character-signature-facts",
        )
        .expect("symbol world");
        let registration = registration_facts(&document, world_id, manifests);
        let mut request = CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            &project,
            &registration,
            None,
        );
        for publication in publications {
            request = request.with_callable_publication(publication);
        }
        let world = CharacterRegistrar::register(request).expect("registered semantic world");
        Self {
            document,
            project,
            world,
        }
    }

    fn analyze(&self) -> TypeCheckReport {
        analyze_registered_project_types(&self.project.linked_module(), &self.world)
    }

    fn query(&self, call: &str, occurrence: usize) -> SignatureQueryOutcome {
        let call_start = nth_offset(self.document.text(), call, occurrence);
        let cursor = call_start + call.find('(').expect("call has argument list") + 1;
        let cancelled = AtomicBool::new(false);
        let hir = self.project.linked_module();
        query_signature(
            SignatureQuery::production(
                &self.world,
                &self.document,
                &hir,
                cursor,
                SignatureQueryControl::new(&cancelled, None),
            )
            .expect("fixture retains one accepted document/HIR/world tuple"),
        )
        .expect("signature query succeeds")
    }
}

fn manifest(owner: &str) -> CharacterManifest {
    let owner = CharacterId::try_new(owner).expect("character ID");
    let happy_look = CharacterLookId::try_new("happy").expect("look ID");
    let happy_variant = CharacterVariantId::try_new("happy").expect("variant ID");
    let parts = ["face", "body"]
        .into_iter()
        .enumerate()
        .map(|(z_order, part)| {
            let part = CharacterPartId::try_new(part).expect("part ID");
            CharacterPart::new(
                part.clone(),
                i32::try_from(z_order).expect("small z order"),
                vec![CharacterVariant::new(
                    happy_variant.clone(),
                    CharacterAssetPath::try_new(format!(
                        "layers/{}-{}-happy.png",
                        owner.compact_str(),
                        part.as_str()
                    ))
                    .expect("asset path"),
                    CharacterRect::new(0, 0, 32, 64),
                    u8::MAX,
                    CharacterBlendMode::Normal,
                    false,
                )],
            )
        })
        .collect::<Vec<_>>();
    let selections = parts
        .iter()
        .map(|part| CharacterPartSelection::new(part.id().clone(), happy_variant.clone()))
        .collect();
    CharacterManifest::new(
        owner,
        CharacterCanvas::new(32, 64),
        CharacterPoint::new(16, 64),
        happy_look.clone(),
        parts,
        vec![CharacterLook::new(happy_look, selections)],
        None,
    )
    .expect("character manifest")
}

fn registration_facts(
    source: &Arc<SourceDocument>,
    world: ProjectSymbolWorldId,
    manifests: &[CharacterManifest],
) -> ProjectRegistrationFacts {
    let mut documents = vec![Arc::clone(source)];
    let mut source_backed = Vec::new();
    let mut externals = Vec::new();
    for (index, manifest) in manifests.iter().enumerate() {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!("memory:///character-{index}.json"))
                    .expect("manifest source ID"),
                SourceName::path(format!("memory:///character-{index}.json")),
                manifest.to_json_pretty().expect("manifest JSON"),
            )
            .expect("manifest document"),
        );
        let backed = SourceBackedCharacterManifest::decode_registration_json(&document)
            .expect("source-backed manifest");
        let owner = backed.manifest().character().clone();
        let declaration = backed
            .source_map()
            .token(&CharacterManifestTokenPath::Root(
                CharacterManifestRootField::Character,
            ))
            .expect("character token")
            .value()
            .clone();
        let seed = ExternalDeclarationSeed::try_new(
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
                .expect("canonical character path"),
            Some(Visibility::Public),
            declaration.clone(),
            character_direct_bindings(&owner, &declaration),
        )
        .expect("external declaration");
        externals.push(ExternalRegistrationFact::new(
            seed,
            RegisteredExternalOwner::Character(owner),
            declaration,
        ));
        documents.push(document);
        source_backed.push(backed);
    }
    let catalogs = if source_backed.is_empty() {
        Vec::new()
    } else {
        vec![
            SourceBackedCharacterCatalog::try_new(source.identity().clone(), source_backed)
                .expect("source-backed character catalog"),
        ]
    };
    ProjectRegistrationFacts::try_new(world, documents, externals, catalogs)
        .expect("registration facts")
}

fn character_direct_bindings(
    owner: &CharacterId,
    declaration: &SourceSpan,
) -> Vec<ProjectDirectBinding> {
    let compact = owner
        .compact_segments()
        .map(|segment| ProjectSymbolSegment::try_new(segment).expect("compact segment"))
        .collect::<Vec<_>>();
    let mut paths = vec![
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            std::iter::once(ProjectSymbolSegment::try_new("character").expect("namespace"))
                .chain(compact.iter().cloned()),
        )
        .expect("qualified character binding"),
        ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact.clone())
            .expect("compact character binding"),
    ];
    if owner.compact_str() == "alice" {
        paths.push(
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [
                    ProjectSymbolSegment::try_new("cast").expect("cast segment"),
                    ProjectSymbolSegment::try_new("alice").expect("alice segment"),
                ],
            )
            .expect("qualified project binding"),
        );
        paths.push(
            ProjectSymbolPath::new(
                ModulePathRoot::ImplicitCrate,
                [ProjectSymbolSegment::try_new("hero").expect("hero alias")],
            )
            .expect("alias binding"),
        );
    }
    paths
        .into_iter()
        .map(|path| {
            let authored_alias =
                path.segments().len() == 1 && path.last_segment().as_str() == "hero";
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                path,
                Some(Visibility::Public),
                declaration.clone(),
                authored_alias,
            )
            .expect("direct binding")
        })
        .collect()
}

struct RecordSpec {
    path: Vec<&'static str>,
    overload: usize,
    parameter: Option<(&'static str, TypeKind)>,
    result: TypeKind,
}

fn publication(owner: &str, specs: Vec<RecordSpec>) -> EnvironmentCallablePublication {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new(owner).expect("adapter package ID"),
    );
    let records = specs
        .into_iter()
        .enumerate()
        .map(|(ordinal, spec)| {
            EnvironmentCallablePublicationRecord::try_new(
                EnvironmentCallableKind::Function,
                CallableLookupKey::Free(callable_path(&spec.path)),
                CallableOverloadIndex::try_from_usize(spec.overload).expect("overload"),
                exact_schema(spec.parameter, spec.result),
                CallableDocumentation::missing(),
                None,
                None,
                EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
                    .expect("declaration ordinal"),
            )
            .expect("callable publication record")
        })
        .collect();
    EnvironmentCallablePublication::try_new(owner, records, &PRODUCTION_CALLABLE_LIMITS)
        .expect("callable publication")
}

fn exact_schema(parameter: Option<(&str, TypeKind)>, result: TypeKind) -> CallableSignatureSchema {
    let parameters = parameter
        .map(|(name, ty)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(0).expect("parameter zero"),
                Some(CallableName::try_new(name).expect("parameter name")),
                CallableParameterType::Exact(ty),
                CallableParameterPassing::PositionalOrNamed,
                CallableParameterPresence::Required,
                None,
                None,
            )
            .expect("exact parameter")
        })
        .into_iter()
        .collect();
    let zero = CallableGroupIndex::try_from_usize(0).expect("group zero");
    let group = CallableParameterGroup::try_new(
        zero,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::closed(EffectSet::new())),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("callable schema")
}

fn callable_path(segments: &[&str]) -> CallablePath {
    CallablePath::try_new(
        segments
            .iter()
            .map(|segment| CallableName::try_new(*segment).expect("callable path segment")),
    )
    .expect("callable path")
}

fn fact_for_call<'a>(
    document: &SourceDocument,
    report: &'a TypeCheckReport,
    call: &str,
    occurrence: usize,
) -> &'a CallTargetFacts {
    let start = nth_offset(document.text(), call, occurrence);
    let end = start + call.len();
    (0..report.stats.expressions)
        .find_map(|index| {
            report
                .call_target_facts(TypeExpressionId::from_index(index))
                .expect("call facts are internally valid")
                .filter(|facts| {
                    facts.call_span().range().start() == start
                        && facts.call_span().range().end() == end
                })
        })
        .expect("exact typed call fact")
}

fn nth_offset(source: &str, needle: &str, occurrence: usize) -> usize {
    source
        .match_indices(needle)
        .nth(occurrence)
        .map(|(offset, _)| offset)
        .expect("source occurrence")
}

#[test]
fn registered_character_spellings_preserve_one_structural_owner() {
    const SOURCE: &str = "fn main() -> Unit { () }\n";
    let alice_manifest = manifest("character.alice");
    let fixture = SignatureFixture::new(
        "character-owner-spellings",
        SOURCE,
        std::slice::from_ref(&alice_manifest),
        Vec::new(),
    );
    let reference_source = fixture
        .document
        .span(SourceRange::new(0, 1))
        .expect("reference source");
    let cases = [
        (
            "character.alice",
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "character.alice")
                .expect("canonical owner"),
        ),
        (
            "alice",
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "alice")
                .expect("compact owner"),
        ),
        (
            "cast.alice",
            SymbolPath::try_new(
                ModulePathRoot::ImplicitCrate,
                vec![ModuleSegment::new("cast").expect("cast qualifier")],
                "alice",
            )
            .expect("qualified owner"),
        ),
        (
            "hero",
            SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), "hero")
                .expect("authored alias"),
        ),
    ];
    let expected = CharacterId::try_new("character.alice").expect("Alice ID");
    for (authored, reference) in cases {
        assert_eq!(reference.canonical_string(), authored);
        let owner = fixture
            .world
            .environment()
            .resolve_character_owner(
                fixture.world.symbols(),
                &CanonicalModulePath::crate_root(),
                &reference,
                &reference_source,
            )
            .expect("registered character owner");
        assert_eq!(owner, expected);
        assert_eq!(
            TypeKind::character_look(owner).source_label(),
            "CharacterLook<character.alice>"
        );
    }

    let alias_docs = CallableDocumentation::missing().with_canonical_owner_note(expected.as_str());
    assert_eq!(
        alias_docs.details(),
        Some("Canonical owner: `character.alice`.")
    );
}

#[test]
fn nominal_overloads_have_checker_and_signature_query_fact_parity() {
    let (fixture, [alice_look, bob_look, alice_face, alice_body]) = nominal_parity_fixture();
    let report = fixture.analyze();
    assert!(report.diagnostics.is_empty(), "{:?}", report.diagnostics);

    assert_ne!(alice_look, bob_look);
    assert_ne!(alice_look, alice_face);
    assert_ne!(alice_face, alice_body);
    assert_registered_happy_variant(&fixture, [&alice_look, &bob_look, &alice_face, &alice_body]);

    assert_nominal_parity_cases(
        &fixture,
        &report,
        [
            ("choose_owner(alice_look_value())", 0, 0, &alice_look),
            ("choose_owner(bob_look_value())", 0, 1, &bob_look),
            ("choose_family(alice_look_value())", 0, 0, &alice_look),
            ("choose_family(alice_face_value())", 0, 1, &alice_face),
            ("choose_family(alice_body_value())", 0, 2, &alice_body),
            ("choose_owner(alice_look_value())", 1, 0, &alice_look),
        ],
    );
}

fn nominal_parity_fixture() -> (SignatureFixture, [TypeKind; 4]) {
    const SOURCE: &str = r"
fn main() -> Unit {
    let owner_alice: String = choose_owner(alice_look_value())
    let owner_bob: String = choose_owner(bob_look_value())
    let family_look: String = choose_family(alice_look_value())
    let family_face: String = choose_family(alice_face_value())
    let family_body: String = choose_family(alice_body_value())
    let exact_range: String = choose_owner(alice_look_value()) // choose_owner(bob_look_value())
    ()
}
";
    let alice = CharacterId::try_new("character.alice").expect("Alice ID");
    let bob = CharacterId::try_new("character.bob").expect("Bob ID");
    let face = CharacterPartId::try_new("face").expect("face part");
    let body = CharacterPartId::try_new("body").expect("body part");
    let alice_look = TypeKind::character_look(alice.clone());
    let bob_look = TypeKind::character_look(bob.clone());
    let alice_face = TypeKind::character_variant(alice.clone(), face.clone());
    let alice_body = TypeKind::character_variant(alice.clone(), body.clone());
    let publication = nominal_parity_publication(&alice_look, &bob_look, &alice_face, &alice_body);
    let manifests = [manifest("character.alice"), manifest("character.bob")];
    let fixture = SignatureFixture::new(
        "character-nominal-parity",
        SOURCE,
        &manifests,
        vec![publication],
    );
    (fixture, [alice_look, bob_look, alice_face, alice_body])
}

fn nominal_parity_publication(
    alice_look: &TypeKind,
    bob_look: &TypeKind,
    alice_face: &TypeKind,
    alice_body: &TypeKind,
) -> EnvironmentCallablePublication {
    publication(
        "adapter.nominal-parity",
        vec![
            RecordSpec {
                path: vec!["alice_look_value"],
                overload: 0,
                parameter: None,
                result: alice_look.clone(),
            },
            RecordSpec {
                path: vec!["bob_look_value"],
                overload: 0,
                parameter: None,
                result: bob_look.clone(),
            },
            RecordSpec {
                path: vec!["alice_face_value"],
                overload: 0,
                parameter: None,
                result: alice_face.clone(),
            },
            RecordSpec {
                path: vec!["alice_body_value"],
                overload: 0,
                parameter: None,
                result: alice_body.clone(),
            },
            RecordSpec {
                path: vec!["choose_owner"],
                overload: 0,
                parameter: Some(("look", alice_look.clone())),
                result: TypeKind::String,
            },
            RecordSpec {
                path: vec!["choose_owner"],
                overload: 1,
                parameter: Some(("look", bob_look.clone())),
                result: TypeKind::String,
            },
            RecordSpec {
                path: vec!["choose_family"],
                overload: 0,
                parameter: Some(("value", alice_look.clone())),
                result: TypeKind::String,
            },
            RecordSpec {
                path: vec!["choose_family"],
                overload: 1,
                parameter: Some(("value", alice_face.clone())),
                result: TypeKind::String,
            },
            RecordSpec {
                path: vec!["choose_family"],
                overload: 2,
                parameter: Some(("value", alice_body.clone())),
                result: TypeKind::String,
            },
        ],
    )
}

fn assert_registered_happy_variant<'a>(
    fixture: &SignatureFixture,
    types: impl IntoIterator<Item = &'a TypeKind>,
) {
    for ty in types {
        let nominal = ty.character_nominal().expect("character nominal");
        assert!(
            fixture
                .world
                .environment()
                .character_enum_variants(nominal)
                .expect("registered nominal family")
                .contains("happy")
        );
    }
}

fn assert_nominal_parity_cases<'a>(
    fixture: &SignatureFixture,
    report: &TypeCheckReport,
    cases: impl IntoIterator<Item = (&'a str, usize, usize, &'a TypeKind)>,
) {
    for (call, occurrence, expected_overload, expected_type) in cases {
        let facts = fact_for_call(&fixture.document, report, call, occurrence);
        let CallTargetFact::Selected { selected, .. } = facts.target() else {
            panic!("{call} must have one selected checker target")
        };
        let CallableCandidateId::Environment(selected_id) = selected.id() else {
            panic!("{call} must select an accepted environment callable")
        };
        assert_eq!(selected_id.overload().get(), expected_overload);
        let [argument] = facts.arguments() else {
            panic!("{call} must retain one authored argument")
        };
        let [slot] = argument.slots() else {
            panic!("{call} must retain one typed argument slot")
        };
        assert_eq!(slot.inferred(), Some(expected_type));
        assert_eq!(slot.expected(), Some(expected_type));

        let SignatureQueryOutcome::Help(help) = fixture.query(call, occurrence) else {
            panic!("{call} must produce signature help")
        };
        assert_eq!(help.call_span(), facts.call_span());
        let active = &help.signatures()[help.active_signature().get()];
        assert_eq!(active.candidate(), selected.id());
        let parameter = &active.groups()[active.current_group().get()].parameters()[0];
        assert_eq!(
            parameter.ty(),
            &CallableParameterType::Exact(expected_type.clone())
        );
    }
}

#[test]
fn presentation_labels_do_not_change_typed_candidate_or_nominal_identity() {
    const SOURCE: &str = r"
fn main() -> Unit {
    let selected: String = label_probe(alice_look_value())
    ()
}
";
    let alice = CharacterId::try_new("character.alice").expect("Alice ID");
    let nominal = TypeKind::character_look(alice);
    let make_publication = |parameter_name: &'static str| {
        publication(
            "adapter.label-probe",
            vec![
                RecordSpec {
                    path: vec!["alice_look_value"],
                    overload: 0,
                    parameter: None,
                    result: nominal.clone(),
                },
                RecordSpec {
                    path: vec!["label_probe"],
                    overload: 0,
                    parameter: Some((parameter_name, nominal.clone())),
                    result: TypeKind::String,
                },
            ],
        )
    };
    let manifests = [manifest("character.alice")];
    let first = SignatureFixture::new(
        "character-label-first",
        SOURCE,
        &manifests,
        vec![make_publication("look")],
    );
    let second = SignatureFixture::new(
        "character-label-second",
        SOURCE,
        &manifests,
        vec![make_publication("appearance")],
    );
    let SignatureQueryOutcome::Help(first_help) = first.query("label_probe(alice_look_value())", 0)
    else {
        panic!("first world must produce signature help")
    };
    let SignatureQueryOutcome::Help(second_help) =
        second.query("label_probe(alice_look_value())", 0)
    else {
        panic!("second world must produce signature help")
    };
    let first_signature = &first_help.signatures()[first_help.active_signature().get()];
    let second_signature = &second_help.signatures()[second_help.active_signature().get()];
    let first_parameter = &first_signature.groups()[0].parameters()[0];
    let second_parameter = &second_signature.groups()[0].parameters()[0];

    assert_eq!(first_signature.candidate(), second_signature.candidate());
    assert_eq!(first_parameter.ty(), second_parameter.ty());
    assert_eq!(first_parameter.ty(), &CallableParameterType::Exact(nominal));
    assert_ne!(first_parameter.label(), second_parameter.label());
}

#[test]
fn unknown_dotted_callee_does_not_fall_back_to_rust_export_suffix() {
    const SOURCE: &str = r"
fn main() -> Unit {
    let missing: String = unregistered.resolve(1i32)
    ()
}
";
    let adapter = AdapterPackageId::try_new("adapter.rust-suffix").expect("adapter ID");
    let record = EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::RustFunction,
        CallableLookupKey::Free(callable_path(&["resolve"])),
        CallableOverloadIndex::try_from_usize(0).expect("overload"),
        exact_schema(Some(("value", TypeKind::I32)), TypeKind::String),
        CallableDocumentation::missing(),
        None,
        Some(
            RustCallableProvenance::try_new(
                adapter.clone(),
                RustPackageProvenance::try_new("rust_suffix", "0.1.0", None)
                    .expect("Rust package provenance"),
                RustItemPath::try_new("rust_suffix::resolve").expect("Rust item path"),
                RustCallablePurity::Pure,
            )
            .expect("Rust callable provenance"),
        ),
        EnvironmentDeclarationOrdinal::try_from_usize(0).expect("declaration ordinal"),
    )
    .expect("Rust publication record");
    let publication = EnvironmentCallablePublication::try_new(
        EnvironmentCallableOwner::Adapter(adapter),
        vec![record],
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("Rust callable publication");
    let fixture = SignatureFixture::new("rust-suffix-no-fallback", SOURCE, &[], vec![publication]);
    assert_eq!(
        fixture.query("unregistered.resolve(1i32)", 0),
        SignatureQueryOutcome::NotApplicable(SignatureNotApplicable::UnknownCallee)
    );

    let report = fixture.analyze();
    let facts = fact_for_call(&fixture.document, &report, "unregistered.resolve(1i32)", 0);
    assert!(matches!(facts.target(), CallTargetFact::Missing { .. }));
}
