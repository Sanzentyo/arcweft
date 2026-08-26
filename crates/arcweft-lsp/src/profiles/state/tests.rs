use super::*;
use crate::profiles::accepted_project::{
    AcceptedProjectSnapshot, AcceptedSourceAccess, AcceptedSourceDocumentSeed,
    AcceptedSourceLocator, AcceptedSourceOwnership,
};
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
use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, compile_project,
};
use arcweft_lang_hir::{
    database::HirDatabase,
    lowering::{HirModuleKey, LoweringRequest},
    project::{HirProject, HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolLinkError,
        ProjectSymbolRevision, ProjectSymbolWorldId,
    },
};
use arcweft_lang_sema::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableDocumentation,
        CallableGenericParameterIssuer, CallableGroupIndex, CallableGroupKind, CallableName,
        CallableOverloadIndex, CallableValidator, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentDeclarationOrdinal, ProjectCallablePath,
        SpreadArgumentPolicy, UnknownNamedArgumentPolicy,
    },
    effect_row::EffectRow,
    effects::EffectSet,
    env::{TypeCheckEnv, identity::EnvironmentBindingId},
    registration::{
        CharacterRegistrar, CharacterRegistrationDiagnosticKind, CharacterRegistrationRequest,
        EnvironmentCallableLookupInput, EnvironmentCallablePublicationMetadataInput,
        EnvironmentCallablePublicationRecordInput, EnvironmentCallableSignatureInput,
        EnvironmentManifestDigest, EnvironmentParameterGroupInput, EnvironmentPublicationItemId,
        EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode, ExternalRegistrationFact,
        ProjectRegistrationFacts, RegisteredExternalOwner, RegisteredSemanticWorld,
        RegisteredTypeCheckEnv, SourceBackedEnvironmentRegistrationInput,
    },
    signature::SignatureQueryOutcome,
    types::TypeKind,
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    incremental::SyntaxDatabase,
    parser::ParseOptions,
};
use arcweft_launch::ProfileId;
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};
use std::{
    collections::BTreeMap,
    sync::{Barrier, mpsc},
    thread,
    time::{Duration, Instant},
};

fn registered_world() -> Arc<CompiledProject> {
    registered_world_with_base(TypeCheckEnv::standard())
}

fn registered_world_with_base(base: TypeCheckEnv) -> Arc<CompiledProject> {
    let (document, _project) = project_fixture();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("cache.tests").expect("package"),
        document.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    compile_fixture(&document, base, facts, None)
}

#[allow(
    clippy::too_many_lines,
    reason = "the test fixture builds one complete source-backed character registration"
)]
fn registered_world_with_character_asset(
    asset: &str,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> (
    Arc<CompiledProject>,
    Arc<SourceDocument>,
    Arc<SourceDocument>,
) {
    let (root, _project) = project_fixture();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("cache.tests").expect("package"),
        root.identity().id().clone(),
        "test",
    )
    .expect("world");
    let part = CharacterPartId::try_new("body").expect("part");
    let variant = CharacterVariantId::try_new("default").expect("variant");
    let look = CharacterLookId::try_new("normal").expect("look");
    let manifest = CharacterManifest::new(
        CharacterId::try_new("character.akane").expect("character"),
        CharacterCanvas::new(64, 128),
        CharacterPoint::new(32, 128),
        look.clone(),
        vec![CharacterPart::new(
            part.clone(),
            0,
            vec![CharacterVariant::new(
                variant.clone(),
                CharacterAssetPath::try_new(asset).expect("asset"),
                CharacterRect::new(0, 0, 64, 128),
                u8::MAX,
                CharacterBlendMode::Normal,
                false,
            )],
        )],
        vec![CharacterLook::new(
            look,
            vec![CharacterPartSelection::new(part, variant)],
        )],
        None,
    )
    .expect("character manifest");
    let manifest_document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(
                "arcweft-project://cache-tests/assets/akane.awchar/character.awchar.json",
            )
            .expect("manifest document ID"),
            SourceName::path("assets/akane.awchar/character.awchar.json"),
            manifest.to_json_pretty().expect("manifest JSON"),
        )
        .expect("manifest document"),
    );
    let backed = SourceBackedCharacterManifest::decode_registration_json(&manifest_document)
        .expect("source-backed manifest");
    let owner = backed.manifest().character().clone();
    let declaration = backed
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("character declaration token")
        .value()
        .clone();
    let compact = owner
        .compact_segments()
        .map(|segment| ProjectSymbolSegment::try_new(segment).expect("character binding segment"))
        .collect::<Vec<_>>();
    let bindings = vec![
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            std::iter::once(
                ProjectSymbolSegment::try_new("character").expect("character namespace segment"),
            )
            .chain(compact.iter().cloned()),
        )
        .expect("qualified character binding"),
        ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact)
            .expect("compact character binding"),
    ];
    let direct_bindings = bindings
        .into_iter()
        .map(|binding| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                binding,
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
        })
        .collect::<Result<Vec<_>, _>>()
        .expect("character direct bindings");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
            .expect("character canonical path"),
        Some(Visibility::Public),
        declaration.clone(),
        direct_bindings,
    )
    .expect("character external declaration");
    let fact =
        ExternalRegistrationFact::new(seed, RegisteredExternalOwner::Character(owner), declaration);
    let catalog = SourceBackedCharacterCatalog::try_new(root.identity().clone(), vec![backed])
        .expect("character catalog");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&root), Arc::clone(&manifest_document)],
        vec![fact],
        vec![catalog],
        Vec::new(),
    )
    .expect("character registration facts");
    let registered = compile_fixture(&root, TypeCheckEnv::standard(), facts, previous);
    (registered, root, manifest_document)
}

fn compile_fixture(
    document: &Arc<SourceDocument>,
    base: TypeCheckEnv,
    facts: ProjectRegistrationFacts,
    previous: Option<&RegisteredTypeCheckEnv>,
) -> Arc<CompiledProject> {
    let manifest = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://cache-tests/arcw.toml")
                .expect("manifest document id"),
            SourceName::path("arcw.toml"),
            "",
        )
        .expect("manifest document"),
    );
    let sources = ProjectSources::new(
        std::path::PathBuf::from("arcw.toml"),
        std::path::PathBuf::new(),
        PackageSpec {
            id: PackageId::new("cache.tests").expect("package id"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        manifest,
        [ProjectSourceFile::new(
            CanonicalModulePath::crate_root(),
            std::path::PathBuf::from("src/main.arcw"),
            Arc::clone(document),
            [],
        )],
    )
    .expect("project sources");
    let context = ProjectCompilationContext::new(
        Arc::new(base),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        previous.cloned().map(Arc::new),
        None,
    );
    let mut syntax = SyntaxDatabase::try_new().expect("test syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(document),
            ParseOptions::default(),
        )
        .expect("attached test source");
    let parsed_sources = BTreeMap::from([(CanonicalModulePath::crate_root(), parsed)]);
    let mut compiler = ProjectCompilationSession::try_new().expect("test HIR database");
    Arc::new(
        compile_project(&mut compiler, &sources, &parsed_sources, &context)
            .expect("compiled test project"),
    )
}

fn project_fixture() -> (Arc<SourceDocument>, Arc<HirProject>) {
    let source = "flow @flow.main main() -> String { return \"ok\" }\n";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://cache-tests/src/main.arcw")
                .expect("document id"),
            SourceName::path("src/main.arcw"),
            source,
        )
        .expect("source document"),
    );
    let (_, project) = attached_project(&document);
    (document, project)
}

fn attached_project(
    document: &Arc<SourceDocument>,
) -> (
    arcweft_lang_syntax::incremental::ParsedSource,
    Arc<HirProject>,
) {
    let mut syntax = SyntaxDatabase::try_new().expect("test syntax database");
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(document.display_name().clone()),
            Arc::clone(document),
            ParseOptions::default(),
        )
        .expect("attached test source");
    let package = CallablePackageId::try_new("cache.tests").expect("package");
    let path = CanonicalModulePath::crate_root();
    let key = HirModuleKey::new(package.clone(), path.clone(), document.identity().clone());
    let mut database = HirDatabase::try_new().expect("test HIR database");
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        document.identity().id().clone(),
        "lsp-profile-state-tests",
    )
    .expect("test symbol world");
    let revision = ProjectSymbolRevision::try_for_documents([document.identity()])
        .expect("test symbol revision");
    let transaction = database
        .stage_proof_return_project(
            [LoweringRequest::try_new(key, &parsed).expect("attached lowering request")],
            world,
            revision,
            [document.identity()],
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("attached project stages");
    let facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("profile fixture has no authored Proof return headers");
    let mut outputs = transaction
        .publish_with_semantic_facts(&mut database, facts)
        .expect("attached project publishes");
    let hir = outputs
        .pop()
        .expect("one profile fixture module")
        .into_module();
    assert!(outputs.is_empty());
    let bound = HirProjectModule::try_new(&database, &package, &path, document.identity(), hir)
        .expect("final HIR module binding");
    let mut builder = HirProjectBuilder::new(&database, package);
    builder.insert_module(bound).expect("module insertion");
    let project = Arc::new(builder.finish().expect("final HIR project"));
    (parsed, project)
}

fn accepted_candidate(compiled: Arc<CompiledProject>) -> AcceptedProfileCandidate {
    let root = CanonicalModulePath::crate_root();
    let document = Arc::clone(
        compiled
            .hir_project()
            .view()
            .module(&root)
            .expect("compiled root HIR module")
            .provenance()
            .document(),
    );
    let source_uri = "file:///workspace/cache-tests/src/main.arcw"
        .parse::<Uri>()
        .expect("source URI");
    let project = Arc::new(
        AcceptedProjectSnapshot::try_new(
            Arc::clone(compiled.tooling_lease()),
            Some(compiled.as_ref()),
            vec![AcceptedSourceDocumentSeed::new(
                document,
                AcceptedSourceLocator::Uri { uri: source_uri },
                AcceptedSourceOwnership::Workspace,
                AcceptedSourceAccess::Writable,
            )],
        )
        .expect("accepted project snapshot"),
    );
    let workspace_uri = "file:///workspace/cache-tests"
        .parse::<Uri>()
        .expect("workspace URI");
    let manifest_uri = "file:///workspace/cache-tests/arcw.toml"
        .parse::<Uri>()
        .expect("manifest URI");
    AcceptedProfileCandidate::try_new(
        AcceptedProfileKey::new(
            &workspace_uri,
            &manifest_uri,
            ProfileId::new("test").expect("valid test profile ID"),
        ),
        Some(compiled),
        project,
        AcceptedOverlaySet::default(),
    )
    .expect("complete candidate")
}

fn accepted_character_candidate(
    compiled: Arc<CompiledProject>,
    root: Arc<SourceDocument>,
    manifest: Arc<SourceDocument>,
) -> AcceptedProfileCandidate {
    let source_uri = "file:///workspace/cache-tests/src/main.arcw"
        .parse::<Uri>()
        .expect("source URI");
    let manifest_uri = "file:///workspace/cache-tests/assets/akane.awchar/character.awchar.json"
        .parse::<Uri>()
        .expect("manifest URI");
    let project = Arc::new(
        AcceptedProjectSnapshot::try_new(
            Arc::clone(compiled.tooling_lease()),
            Some(compiled.as_ref()),
            vec![
                AcceptedSourceDocumentSeed::new(
                    root,
                    AcceptedSourceLocator::Uri { uri: source_uri },
                    AcceptedSourceOwnership::Workspace,
                    AcceptedSourceAccess::Writable,
                ),
                AcceptedSourceDocumentSeed::new(
                    manifest,
                    AcceptedSourceLocator::Uri { uri: manifest_uri },
                    AcceptedSourceOwnership::Workspace,
                    AcceptedSourceAccess::ReadOnly,
                ),
            ],
        )
        .expect("accepted character project snapshot"),
    );
    let workspace_uri = "file:///workspace/cache-tests"
        .parse::<Uri>()
        .expect("workspace URI");
    let profile_manifest_uri = "file:///workspace/cache-tests/arcw.toml"
        .parse::<Uri>()
        .expect("profile manifest URI");
    AcceptedProfileCandidate::try_new(
        AcceptedProfileKey::new(
            &workspace_uri,
            &profile_manifest_uri,
            ProfileId::new("test").expect("profile"),
        ),
        Some(compiled),
        project,
        AcceptedOverlaySet::default(),
    )
    .expect("accepted character candidate")
}

fn external_registration_fact(
    document: &SourceDocument,
    owner: &str,
    binding: ProjectSymbolPath,
) -> ExternalRegistrationFact {
    let declaration = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("external declaration span");
    let direct_binding = ProjectDirectBinding::try_new(
        CanonicalModulePath::crate_root(),
        binding,
        Some(Visibility::Public),
        declaration.clone(),
        false,
    )
    .expect("typed direct binding");
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner)
            .expect("opaque canonical path"),
        Some(Visibility::Public),
        declaration.clone(),
        vec![direct_binding],
    )
    .expect("external declaration seed");
    ExternalRegistrationFact::new(
        seed,
        {
            let owner = EnvironmentBindingId::try_new(owner).expect("environment owner");
            RegisteredExternalOwner::environment(owner.clone(), owner)
        },
        declaration,
    )
}

fn colliding_typed_binding_registration()
-> arcweft_lang_sema::registration::CharacterRegistrationReport {
    let (root, project) = project_fixture();
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("cache.tests").expect("package"),
        root.identity().id().clone(),
        "test",
    )
    .expect("world");
    let first = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-generated://cache-tests/adapter-first")
                .expect("document id"),
            SourceName::Generated,
            "adapter.first",
        )
        .expect("first adapter document"),
    );
    let second = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-generated://cache-tests/adapter-second")
                .expect("document id"),
            SourceName::Generated,
            "adapter.second",
        )
        .expect("second adapter document"),
    );
    let shared = || {
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new("shared").expect("valid shared segment")],
        )
        .expect("shared typed binding path")
    };
    let first_fact = external_registration_fact(&first, "adapter.first", shared());
    let second_fact = external_registration_fact(&second, "adapter.second", shared());
    let facts = ProjectRegistrationFacts::try_new(
        world,
        vec![root, first, second],
        vec![first_fact, second_fact],
        Vec::new(),
        Vec::new(),
    )
    .expect("colliding facts retain typed evidence");
    let base = TypeCheckEnv::standard()
        .with_symbol("adapter.first", TypeKind::I32)
        .with_symbol("adapter.second", TypeKind::I64);

    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        project.view(),
        &facts,
        None,
    ))
    .expect_err("typed binding collision rejects the semantic candidate")
}

#[derive(Clone, Copy, Debug)]
enum CallableCatalogFailureFixture {
    ProjectWorldPackageMismatch,
    DuplicateTypedId,
    SameRankCollision,
    DuplicateProviderOverload,
    NonContiguousOverloads,
}

#[derive(Clone, Debug)]
struct EnvironmentCallableFailureRecord {
    kind: EnvironmentCallableKind,
    overload: usize,
    result: EnvironmentTypeProjectionKind,
    declaration_order: usize,
}

fn environment_callable_failure_input(
    owner_id: &str,
    records: &[EnvironmentCallableFailureRecord],
) -> (
    Arc<SourceDocument>,
    SourceBackedEnvironmentRegistrationInput,
) {
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new(owner_id).expect("adapter package ID"),
    );
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(format!("arcweft-generated://cache-tests/{owner_id}"))
                .expect("environment document ID"),
            SourceName::Generated,
            "callable-catalog-failure",
        )
        .expect("environment document"),
    );
    let span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("environment type span");
    let package = match &owner {
        EnvironmentCallableOwner::Adapter(package) => package,
        EnvironmentCallableOwner::Standard(_) => unreachable!("fixture uses adapter owners"),
    };
    let path = ProjectCallablePath::new(
        CallablePackageId::try_new(package.as_str()).expect("callable package ID"),
        CanonicalModulePath::crate_root(),
        arcweft_lang_sema::callable::CallablePath::try_new([CallableName::try_new(
            "catalog_failure",
        )
        .expect("callable name")])
        .expect("callable path"),
    );
    let records = records
        .iter()
        .map(|record| {
            let overload = CallableOverloadIndex::try_from_usize(record.overload)
                .expect("fixture overload index");
            EnvironmentCallablePublicationRecordInput::new(
                EnvironmentPublicationItemId::AdapterFunction {
                    owner: owner.clone(),
                    path: path.clone(),
                    overload,
                },
                record.kind,
                EnvironmentCallableLookupInput::Free(path.clone()),
                overload,
                EnvironmentCallableSignatureInput::new(
                    [EnvironmentParameterGroupInput::new(
                        CallableGroupIndex::try_from_usize(0).expect("initial group index"),
                        CallableGroupKind::Initial,
                        [],
                    )],
                    EnvironmentTypeProjectionNode::new(span.clone(), record.result.clone()),
                    EffectRow::closed(EffectSet::default()),
                    CallableArgumentPolicy::new(
                        UnknownNamedArgumentPolicy::Reject,
                        SpreadArgumentPolicy::Reject,
                    ),
                    CallableValidator::Ordinary,
                    CallableGenericParameterIssuer::empty(),
                ),
                EnvironmentDeclarationOrdinal::try_from_usize(record.declaration_order)
                    .expect("declaration order"),
                EnvironmentCallablePublicationMetadataInput::new(
                    CallableDocumentation::missing(),
                    None,
                    None,
                ),
            )
        })
        .collect::<Vec<_>>();
    let manifest_digest = EnvironmentManifestDigest::from_bytes(
        [u8::try_from(package.as_str().len()).expect("short test package ID"); 32],
    );
    let input = SourceBackedEnvironmentRegistrationInput::new(
        owner,
        document.identity().clone(),
        manifest_digest,
        [],
        [],
        [],
        records,
    );
    (document, input)
}

fn callable_catalog_failure_registration(
    fixture: CallableCatalogFailureFixture,
    previous: &RegisteredTypeCheckEnv,
) -> arcweft_lang_sema::registration::CharacterRegistrationReport {
    let (root, project) = project_fixture();
    let (world, documents, environment_inputs) =
        callable_catalog_failure_inputs(&root, fixture, previous);
    let facts = ProjectRegistrationFacts::try_new(
        world,
        documents,
        Vec::new(),
        Vec::new(),
        environment_inputs,
    )
    .expect("typed callable failure facts");

    CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        project.view(),
        &facts,
        Some(previous),
    ))
    .expect_err("callable catalog fixture must reject the candidate world")
}

fn callable_catalog_failure_inputs(
    root: &Arc<SourceDocument>,
    fixture: CallableCatalogFailureFixture,
    previous: &RegisteredTypeCheckEnv,
) -> (
    ProjectSymbolWorldId,
    Vec<Arc<SourceDocument>>,
    Vec<SourceBackedEnvironmentRegistrationInput>,
) {
    match fixture {
        CallableCatalogFailureFixture::ProjectWorldPackageMismatch => {
            let world = ProjectSymbolWorldId::try_new(
                CallablePackageId::try_new("cache.tests.other").expect("mismatched package"),
                root.identity().id().clone(),
                "test",
            )
            .expect("mismatched world");
            (world, vec![Arc::clone(root)], Vec::new())
        }
        CallableCatalogFailureFixture::DuplicateTypedId => {
            let record = EnvironmentCallableFailureRecord {
                kind: EnvironmentCallableKind::Function,
                overload: 0,
                result: EnvironmentTypeProjectionKind::I32,
                declaration_order: 0,
            };
            let duplicate = EnvironmentCallableFailureRecord {
                declaration_order: 1,
                ..record.clone()
            };
            let (document, input) =
                environment_callable_failure_input("adapter-duplicate-id", &[record, duplicate]);
            (
                previous.world().clone(),
                vec![Arc::clone(root), document],
                vec![input],
            )
        }
        CallableCatalogFailureFixture::SameRankCollision => {
            let first = EnvironmentCallableFailureRecord {
                kind: EnvironmentCallableKind::Function,
                overload: 0,
                result: EnvironmentTypeProjectionKind::I32,
                declaration_order: 0,
            };
            let second = EnvironmentCallableFailureRecord {
                result: EnvironmentTypeProjectionKind::I64,
                ..first.clone()
            };
            let (first_document, first_input) =
                environment_callable_failure_input("adapter-collision-a", &[first]);
            let (second_document, second_input) =
                environment_callable_failure_input("adapter-collision-b", &[second]);
            (
                previous.world().clone(),
                vec![Arc::clone(root), first_document, second_document],
                vec![first_input, second_input],
            )
        }
        CallableCatalogFailureFixture::DuplicateProviderOverload => {
            let first = EnvironmentCallableFailureRecord {
                kind: EnvironmentCallableKind::Function,
                overload: 0,
                result: EnvironmentTypeProjectionKind::I32,
                declaration_order: 0,
            };
            let second = EnvironmentCallableFailureRecord {
                kind: EnvironmentCallableKind::Method,
                declaration_order: 1,
                ..first.clone()
            };
            let (document, input) =
                environment_callable_failure_input("adapter-duplicate-overload", &[first, second]);
            (
                previous.world().clone(),
                vec![Arc::clone(root), document],
                vec![input],
            )
        }
        CallableCatalogFailureFixture::NonContiguousOverloads => {
            let record = EnvironmentCallableFailureRecord {
                kind: EnvironmentCallableKind::Function,
                overload: 1,
                result: EnvironmentTypeProjectionKind::I32,
                declaration_order: 0,
            };
            let (document, input) =
                environment_callable_failure_input("adapter-overload-gap", &[record]);
            (
                previous.world().clone(),
                vec![Arc::clone(root), document],
                vec![input],
            )
        }
    }
}

fn insert_signature_cache(environment: &AcceptedProfileEnvironment) {
    environment.seed_signature_cache_for_test(0);
}

fn cache_snapshot(environment: &AcceptedProfileEnvironment) -> SignatureCacheTestSnapshot {
    environment.signature_cache_snapshot_for_test()
}

fn accepted_executable(environment: &AcceptedProfileEnvironment) -> &Arc<CompiledProject> {
    environment
        .executable()
        .expect("test environment is executable")
}

fn accepted_world_arc(environment: &AcceptedProfileEnvironment) -> Arc<RegisteredSemanticWorld> {
    environment
        .registered_world_arc()
        .expect("test environment has a registered world")
}

#[test]
fn successful_identical_rebuild_increments_generation() {
    let state = LspProfileState::new();
    let world = registered_world();
    let first = state
        .replace_accepted(accepted_candidate(Arc::clone(&world)))
        .expect("first accepted environment");
    assert!(Arc::ptr_eq(
        accepted_executable(&first).hir_project(),
        first.project().hir_project()
    ));
    assert!(Arc::ptr_eq(
        accepted_executable(&first).tooling_lease(),
        first.project().tooling_lease()
    ));
    insert_signature_cache(&first);
    let second = state
        .replace_accepted(accepted_candidate(world))
        .expect("identical complete rebuild is still a new generation");
    assert!(Arc::ptr_eq(
        accepted_executable(&second).hir_project(),
        second.project().hir_project()
    ));
    assert!(Arc::ptr_eq(
        accepted_executable(&second).tooling_lease(),
        second.project().tooling_lease()
    ));
    assert_eq!(first.generation().get(), 1);
    assert_eq!(second.generation().get(), 2);
    assert_eq!(cache_snapshot(&first).entries, 1);
    assert_eq!(cache_snapshot(&second).entries, 0);
}

#[test]
fn unrepresentable_complete_entry_size_returns_outcome_without_caching() {
    let state = LspProfileState::new();
    let accepted = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("accepted environment");
    let outcome = Arc::new(SignatureQueryOutcome::NotApplicable(
        arcweft_lang_sema::signature::SignatureNotApplicable::CursorOutsideArgumentList,
    ));

    let insertion = accepted.signature_cache().insert(
        accepted.signature_cache_key_for_test(0),
        Arc::clone(&outcome),
        u64::MAX,
    );

    assert_eq!(insertion, SignatureCacheInsertion::NotCachedUnrepresentable);
    assert_eq!(Arc::strong_count(&outcome), 1);
    assert_eq!(
        outcome.as_ref(),
        &SignatureQueryOutcome::NotApplicable(
            arcweft_lang_sema::signature::SignatureNotApplicable::CursorOutsideArgumentList,
        )
    );
    assert_eq!(accepted.signature_cache_snapshot_for_test().entries, 0);
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the contract test varies every independent typed signature-cache key field"
)]
fn signature_cache_key_misses_when_any_single_identity_field_changes() {
    let state = LspProfileState::new();
    let (world, root, manifest) = registered_world_with_character_asset("layers/body.png", None);
    let accepted = state
        .replace_accepted(accepted_character_candidate(
            Arc::clone(&world),
            root,
            manifest,
        ))
        .expect("accepted character environment");
    let source = accepted
        .project()
        .sources()
        .documents()
        .find_map(|accepted_source| {
            let identity = accepted_source.document().identity().clone();
            accepted
                .project()
                .module_key(&identity)
                .is_some()
                .then_some(identity)
        })
        .expect("module-backed source identity");
    let symbols = world.project_symbols();
    let environment = world.registered_environment();
    let generation = accepted.generation();
    let world_id = symbols.world().clone();
    let symbol_revision = *symbols.revision();
    let character_revision = environment.character_revision();
    let character_digest = environment.character_digest();
    let environment_digest = environment.environment_digest();
    let outcome = Arc::new(SignatureQueryOutcome::NotApplicable(
        arcweft_lang_sema::signature::SignatureNotApplicable::CursorOutsideArgumentList,
    ));
    let base = SignatureCacheKey::new(
        generation,
        world_id.clone(),
        symbol_revision,
        character_revision,
        character_digest,
        environment_digest,
        source.clone(),
        Some(1),
        0,
    );
    assert_eq!(
        accepted.signature_cache().insert(
            base.clone(),
            Arc::clone(&outcome),
            accepted.project().footprint().source_bytes(),
        ),
        SignatureCacheInsertion::Cached
    );

    let changed_world = ProjectSymbolWorldId::try_new(
        world_id.package().clone(),
        world_id.root_document().clone(),
        "other-profile",
    )
    .expect("changed world");
    let changed_document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-project://cache-tests/src/other.arcw")
            .expect("changed document id"),
        SourceName::path("src/other.arcw"),
        "flow @flow.other other {}\n",
    )
    .expect("changed document");
    let changed_symbol_revision =
        ProjectSymbolRevision::try_for_documents([changed_document.identity()])
            .expect("changed symbol revision");
    let (changed_characters, _, _) = registered_world_with_character_asset(
        "layers/body-updated.png",
        Some(world.registered_environment()),
    );
    let changed_environment = changed_characters.registered_environment();
    let variations = [
        SignatureCacheKey::new(
            AcceptedEnvironmentGeneration::for_test(generation.get() + 1),
            world_id.clone(),
            symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            changed_world,
            symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            changed_symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            symbol_revision,
            changed_environment.character_revision(),
            character_digest,
            environment_digest,
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            symbol_revision,
            character_revision,
            changed_environment.character_digest(),
            environment_digest,
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            symbol_revision,
            character_revision,
            character_digest,
            changed_environment.environment_digest(),
            source.clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            changed_document.identity().clone(),
            Some(1),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id.clone(),
            symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            source.clone(),
            Some(2),
            0,
        ),
        SignatureCacheKey::new(
            generation,
            world_id,
            symbol_revision,
            character_revision,
            character_digest,
            environment_digest,
            source,
            Some(1),
            1,
        ),
    ];
    for variation in variations {
        assert_ne!(variation, base);
        assert!(
            accepted.signature_cache().cached(&variation).is_none(),
            "each independently changed cache-key field must miss"
        );
    }
    assert!(Arc::ptr_eq(
        &accepted
            .signature_cache()
            .cached(&base)
            .expect("exact full key remains cacheable"),
        &outcome,
    ));
}

#[test]
fn failed_typed_binding_collision_preserves_accepted_pointer_and_caches() {
    let state = LspProfileState::new();
    let accepted = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("baseline accepted environment");
    insert_signature_cache(&accepted);
    let cache = cache_snapshot(&accepted);
    let accepted_world = accepted_world_arc(&accepted);

    let report = colliding_typed_binding_registration();
    assert!(
        report.diagnostics().iter().any(|diagnostic| matches!(
            diagnostic.kind(),
            CharacterRegistrationDiagnosticKind::ProjectSymbol {
                error: ProjectSymbolLinkError::DuplicateDeclaration { name, .. },
            }
                if name == "shared"
        )),
        "{:?}",
        report.diagnostics()
    );
    let retained = state.current().expect("baseline remains accepted");
    let retained_world = accepted_world_arc(&retained);
    assert!(Arc::ptr_eq(&retained, &accepted));
    assert!(Arc::ptr_eq(&retained_world, &accepted_world));
    assert!(std::ptr::eq(
        retained_world.symbols(),
        accepted_world.symbols()
    ));
    assert!(std::ptr::eq(
        retained_world.environment(),
        accepted_world.environment()
    ));
    assert!(std::ptr::eq(
        retained_world.environment().callable_catalog(),
        accepted_world.environment().callable_catalog()
    ));
    assert!(std::ptr::eq(
        retained_world.character_definition_index(),
        accepted_world.character_definition_index()
    ));
    assert_eq!(retained.generation().get(), 1);
    assert_eq!(cache_snapshot(&retained), cache);

    let replacement = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("next valid candidate is accepted");
    assert_eq!(replacement.generation().get(), 2);
    assert!(!Arc::ptr_eq(&replacement, &accepted));
    assert_eq!(cache_snapshot(&replacement).entries, 0);
}

#[test]
fn every_build_error_preserves_prior_arc() {
    let state = LspProfileState::new();
    let accepted = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("baseline accepted environment");
    insert_signature_cache(&accepted);
    let accepted_cache = cache_snapshot(&accepted);
    let accepted_compiled = Arc::clone(accepted_executable(&accepted));
    let accepted_project = Arc::clone(accepted.project());
    let accepted_world = accepted_world_arc(&accepted);

    for fixture in [
        CallableCatalogFailureFixture::ProjectWorldPackageMismatch,
        CallableCatalogFailureFixture::DuplicateTypedId,
        CallableCatalogFailureFixture::SameRankCollision,
        CallableCatalogFailureFixture::DuplicateProviderOverload,
        CallableCatalogFailureFixture::NonContiguousOverloads,
    ] {
        let report = callable_catalog_failure_registration(fixture, accepted_world.environment());
        assert!(
            report.diagnostics().iter().any(|diagnostic| matches!(
                diagnostic.kind(),
                CharacterRegistrationDiagnosticKind::CallableCatalog {
                    code:
                        arcweft_lang_sema::callable::CallableDiagnosticCode::CorruptCallableCatalog,
                }
            )),
            "{fixture:?}: {:?}",
            report.diagnostics(),
        );

        let retained = state.current().expect("baseline remains accepted");
        let retained_world = accepted_world_arc(&retained);
        assert!(Arc::ptr_eq(&retained, &accepted), "{fixture:?}");
        assert!(
            Arc::ptr_eq(accepted_executable(&retained), &accepted_compiled),
            "{fixture:?}"
        );
        assert!(
            Arc::ptr_eq(retained.project(), &accepted_project),
            "{fixture:?}"
        );
        assert!(Arc::ptr_eq(&retained_world, &accepted_world), "{fixture:?}");
        assert!(
            std::ptr::eq(retained.project().sources(), accepted_project.sources()),
            "{fixture:?}"
        );
        assert!(
            std::ptr::eq(retained_world.symbols(), accepted_world.symbols()),
            "{fixture:?}"
        );
        assert!(
            std::ptr::eq(retained_world.environment(), accepted_world.environment()),
            "{fixture:?}"
        );
        assert!(
            std::ptr::eq(
                retained_world.environment().callable_catalog(),
                accepted_world.environment().callable_catalog()
            ),
            "{fixture:?}"
        );
        assert!(
            std::ptr::eq(
                retained_world.character_definition_index(),
                accepted_world.character_definition_index()
            ),
            "{fixture:?}"
        );
        assert_eq!(retained.generation().get(), 1, "{fixture:?}");
        assert_eq!(cache_snapshot(&retained), accepted_cache, "{fixture:?}");
    }
}

#[test]
fn base_change_same_character_invalidates_broad_cache() {
    let state = LspProfileState::new();
    let first_world = registered_world_with_base(
        TypeCheckEnv::standard().with_symbol("adapter.mode", TypeKind::String),
    );
    let second_world = registered_world_with_base(
        TypeCheckEnv::standard().with_symbol("adapter.mode", TypeKind::Bool),
    );
    assert_eq!(
        first_world.registered_environment().character_digest(),
        second_world.registered_environment().character_digest(),
        "the narrow character key deliberately cannot observe base facts"
    );

    let first = state
        .replace_accepted(accepted_candidate(first_world))
        .expect("first accepted environment");
    insert_signature_cache(&first);
    let second = state
        .replace_accepted(accepted_candidate(second_world))
        .expect("changed base is a complete accepted rebuild");

    assert_eq!(second.generation().get(), 2);
    assert_eq!(cache_snapshot(&second).entries, 0);
    assert!(Arc::ptr_eq(
        &state.current().expect("current environment"),
        &second
    ));
    assert_eq!(cache_snapshot(&first).entries, 1);
}

#[test]
fn character_digest_and_revision_change_publish_a_fresh_cache_namespace() {
    let state = LspProfileState::new();
    let (first_world, first_root, first_manifest) =
        registered_world_with_character_asset("layers/body.png", None);
    let first = state
        .replace_accepted(accepted_character_candidate(
            Arc::clone(&first_world),
            first_root,
            first_manifest,
        ))
        .expect("first accepted character environment");
    insert_signature_cache(&first);
    let (second_world, second_root, second_manifest) = registered_world_with_character_asset(
        "layers/body-updated.png",
        Some(first_world.registered_environment()),
    );
    assert_ne!(
        first_world.registered_environment().character_digest(),
        second_world.registered_environment().character_digest()
    );
    assert_ne!(
        first_world.registered_environment().character_revision(),
        second_world.registered_environment().character_revision()
    );

    let second = state
        .replace_accepted(accepted_character_candidate(
            second_world,
            second_root,
            second_manifest,
        ))
        .expect("changed character environment");

    assert_eq!(cache_snapshot(&first).entries, 1);
    assert_eq!(cache_snapshot(&second).entries, 0);
    assert_eq!(second.generation().get(), 2);
}

#[test]
fn generation_overflow_preserves_state() {
    let state = LspProfileState::new();
    let AcceptedProfileCandidate {
        profile,
        executable,
        project,
        overlays,
    } = accepted_candidate(registered_world());
    let previous = Arc::new(AcceptedProfileEnvironment {
        generation: AcceptedEnvironmentGeneration::for_test(u64::MAX),
        profile,
        executable,
        stamp_world_override: None,
        project,
        overlays,
        caches: ProfileSemanticCaches::default(),
    });
    insert_signature_cache(&previous);
    state
        .accepted
        .write()
        .expect("accepted state lock")
        .replace(Arc::clone(&previous));

    assert_eq!(
        state
            .replace_accepted(accepted_candidate(registered_world()))
            .expect_err("generation overflow rejects replacement"),
        AcceptedEnvironmentReplaceError::GenerationOverflow
    );
    let retained = state.current().expect("old environment remains accepted");
    assert!(Arc::ptr_eq(&retained, &previous));
    assert_eq!(cache_snapshot(&retained).entries, 1);
}

#[test]
fn shutdown_rejects_new_rebuilds() {
    let state = LspProfileState::new();
    state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("accepted environment");

    state.shutdown();

    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    assert!(state.current().is_none());
    assert_eq!(
        state
            .replace_accepted(accepted_candidate(registered_world()))
            .expect_err("shutdown rejects replacement"),
        AcceptedEnvironmentReplaceError::ShuttingDown
    );
    state.shutdown();
    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
}

#[test]
fn shutdown_clears_cache_before_world_drop() {
    let state = LspProfileState::new();
    let reader = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("accepted environment");
    insert_signature_cache(&reader);
    assert_eq!(Arc::strong_count(&reader), 2);

    state.shutdown();

    assert_eq!(cache_snapshot(&reader).entries, 0);
    assert_eq!(Arc::strong_count(&reader), 1);
    assert!(state.current().is_none());
}

#[test]
fn shutdown_closes_admission_before_waiting_for_replacement() {
    let state = Arc::new(LspProfileState::new());
    state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("initial environment");
    let admitted = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let expected = state.current().expect("initial environment retained");
    let replacement = {
        let state = Arc::clone(&state);
        let admitted = Arc::clone(&admitted);
        let release = Arc::clone(&release);
        thread::spawn(move || {
            state.replace_accepted_with(
                Some(&expected),
                accepted_candidate(registered_world()),
                |_| {
                    admitted.wait();
                    release.wait();
                },
            )
        })
    };
    admitted.wait();
    let shutdown = {
        let state = Arc::clone(&state);
        thread::spawn(move || state.shutdown())
    };
    wait_for_lifecycle(&state, ProfileEnvironmentLifecycle::Closing);
    release.wait();
    let replacement = replacement
        .join()
        .expect("replacement thread")
        .expect("replacement passed the second admission check");
    shutdown.join().expect("shutdown thread");
    assert_eq!(replacement.generation().get(), 2);
    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    assert!(state.current().is_none());
    assert_eq!(cache_snapshot(&replacement).entries, 0);

    let state = Arc::new(LspProfileState::new());
    state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("initial environment");
    let accepted_guard = state.accepted.write().expect("accepted state lock");
    let (started_tx, started_rx) = mpsc::channel();
    let replacement = {
        let state = Arc::clone(&state);
        thread::spawn(move || {
            started_tx.send(()).expect("replacement start signal");
            state.replace_accepted(accepted_candidate(registered_world()))
        })
    };
    started_rx.recv().expect("replacement started");
    let shutdown = {
        let state = Arc::clone(&state);
        thread::spawn(move || state.shutdown())
    };
    wait_for_lifecycle(&state, ProfileEnvironmentLifecycle::Closing);
    drop(accepted_guard);
    assert_eq!(
        replacement
            .join()
            .expect("replacement thread")
            .expect_err("candidate did not pass the second admission check"),
        AcceptedEnvironmentReplaceError::ShuttingDown
    );
    shutdown.join().expect("shutdown thread");
    assert_eq!(state.lifecycle(), ProfileEnvironmentLifecycle::Closed);
    assert!(state.current().is_none());
}

#[test]
fn exact_empty_publication_rejects_an_intervening_accepted_environment() {
    let state = LspProfileState::new();
    let accepted = state
        .replace_accepted(accepted_candidate(registered_world()))
        .expect("intervening environment");

    assert_eq!(
        state
            .replace_accepted_with(None, accepted_candidate(registered_world()), |_| {})
            .expect_err("the previously empty state is no longer current"),
        AcceptedEnvironmentReplaceError::CurrentChanged
    );
    assert!(
        Arc::ptr_eq(
            state
                .current()
                .as_ref()
                .expect("accepted environment remains"),
            &accepted,
        ),
        "failed compare-and-swap must not mutate the accepted environment",
    );
}

fn wait_for_lifecycle(state: &LspProfileState, expected: ProfileEnvironmentLifecycle) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while state.lifecycle() != expected {
        assert!(
            Instant::now() < deadline,
            "profile lifecycle did not reach {expected:?}"
        );
        thread::yield_now();
    }
}
