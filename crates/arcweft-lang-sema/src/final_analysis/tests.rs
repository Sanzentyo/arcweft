use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{Arc, atomic::AtomicBool},
};

use arcweft_character::{
    id::CharacterId,
    manifest::registration::{
        CharacterManifestRootField, CharacterManifestTokenPath, SourceBackedCharacterManifest,
    },
    registration_catalog::SourceBackedCharacterCatalog,
};
use arcweft_core::entry::TypeLayoutHash;
use arcweft_dialogue::CharacterDialogueCustomFieldId;
use arcweft_lang_hir::{
    database::HirDatabase,
    expr::{HirCallCallee, HirExprKind, HirSelectedMember},
    item::{HirFunctionBody, HirItemKind},
    lowering::{HirModuleKey, LoweringRequest},
    module::HirModule,
    pattern::HirPatternKind,
    project::{HirProject, HirProjectBuilder, HirProjectModule},
    proof_return::HirProofReturnSemanticFactSet,
    source_index::{
        HirExprSourceRole, HirSourcePresence, HirSourceQuery, HirSourceSite, HirStmtSourceRole,
    },
    stmt::HirStmtKind,
    symbol::{
        CallablePackageId, ExternalDeclarationSeed, ProjectDirectBinding, ProjectSymbolTable,
        ProjectSymbolWorldId,
    },
    type_ref::HirTypeKind,
};
use arcweft_lang_syntax::{
    ast::{
        common::Visibility,
        module_path::{CanonicalModulePath, ModulePathRoot, ModuleSegment},
        symbol_path::{ProjectSymbolPath, ProjectSymbolSegment, SymbolPath},
    },
    incremental::{ParsedSource, SyntaxDatabase},
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};

use super::{
    CallTargetFacts, CandidateEvaluationPass, CandidateExpectedType,
    CharacterDialogueFieldCoordinate, CharacterDialoguePatchContext, CheckedAssertionDisposition,
    CheckedBinding, CheckedBuiltinVariantCase, CheckedCharacterDialogueTarget, CheckedExpression,
    CheckedExpressionResolution, CheckedFunctionExecution, CheckedItem, CheckedItemRole,
    CheckedIteration, CheckedIteratorFamily, CheckedPatchOperation, CheckedPattern,
    CheckedPatternResolution, CheckedStatement, CheckedStatementRole, CheckedSuspensionRole,
    CheckedTypeSelection, CheckedValueResolution, CheckedVariantOwner, FinalSemanticAnalysis,
    FinalSemanticAnalysisControl, FinalSemanticAnalysisError, FinalSemanticAnalysisInput,
    FinalSemanticCatalogs, PhysicalArgumentEvaluationKind, RegisteredSemanticValueId,
    ResolvedCallable, analyze_final_project,
};
use crate::{
    assertion::{AssertionBuildProfile, AssertionContext, AssertionRuntimePolicy},
    callable::{
        AdapterPackageId, CallCalleeClassificationFact, CallResolverAuthority, CallResolverContext,
        CallResolverRequest, CallTargetFact, CallableAccess, CallableArgumentPolicy,
        CallableAuthorityRank, CallableCandidateId, CallableDocumentation, CallableEffectSchema,
        CallableGroupIndex, CallableGroupKind, CallableLimits, CallableLookupKey, CallableName,
        CallableOverloadIndex, CallableParameter, CallableParameterGroup, CallableParameterIndex,
        CallableParameterPassing, CallableParameterPresence, CallableParameterType, CallablePath,
        CallableProviderId, CallableRecord, CallableSignatureSchema, CallableValidator,
        CatalogCallableEntry, CheckedCallArgumentSlotSource, CheckedClosureId, DialogueCallableId,
        DomainMethodId, EffectContractOrigin, EnvironmentCallableCatalog, EnvironmentCallableId,
        EnvironmentCallableKind, EnvironmentCallableOwner, EnvironmentCallablePublicationDigest,
        EnvironmentDeclarationOrdinal, FinalCallCalleeFacts, NonEmptyCallableSet,
        PRODUCTION_CALLABLE_LIMITS, PresentationCallableId, ProjectCallablePath,
        RegisteredCallableCatalog, ResolveCallError, ResolveCallOutcome, ResolverWork,
        SemanticSignatureSurface, SpreadArgumentPolicy, UnknownCallKind,
        UnknownNamedArgumentPolicy, prepare_final_call_callee, resolve_call_target,
    },
    character_dialogue::CharacterDialogueCustomFieldBinding,
    effect_row::EffectRow,
    effects::{EffectId, EffectSet},
    entry::CheckedEntryCatalog,
    env::TypeCheckEnv,
    nominal::{ResolvedTypeRefOutcome, TypeNameResolution, TypeResolutionFailure},
    project_index::{ProgramHash, ProjectEntityId, ProjectSemanticIndex},
    registration::{
        CharacterDialogueCustomFieldInput, CharacterRegistrar, CharacterRegistrationRequest,
        EnvironmentCallableLookupInput, EnvironmentCallablePublicationMetadataInput,
        EnvironmentCallablePublicationRecordInput, EnvironmentCallableSignatureInput,
        EnvironmentManifestDigest, EnvironmentParameterGroupInput, EnvironmentParameterInput,
        EnvironmentParameterMetadataInput, EnvironmentParameterTypeInput,
        EnvironmentPublicationItemId, EnvironmentTypeProjectionKind, EnvironmentTypeProjectionNode,
        ExternalRegistrationFact, ProjectRegistrationFacts, RegisteredExternalOwner,
        RegisteredSemanticWorld, SourceBackedEnvironmentRegistrationInput,
    },
    signature::{
        SignatureQuery, SignatureQueryControl, SignatureQueryError, SignatureQueryOutcome,
        SignatureQueryStep, query_signature,
    },
    types::{
        DetachedTypeOwnerId, EntityKind, GenericTypeOwnerId, GenericTypeParameterId, TypeKind,
    },
};

struct Fixture {
    project: HirProject,
    symbols: Arc<ProjectSymbolTable>,
    registered: RegisteredSemanticWorld,
    root_document: Arc<SourceDocument>,
}

fn source_document(id: &str, path: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source ID"),
            SourceName::path(path),
            source,
        )
        .expect("source document"),
    )
}

fn parse(id: &str, path: &str, source: &str) -> (Arc<SourceDocument>, ParsedSource) {
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let name = SourceName::path(path);
    let document = source_document(id, path, source);
    let parsed = syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            Arc::clone(&document),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .expect("parsed source");
    (document, parsed)
}

fn fixture(root_source: &str, child_source: Option<&str>) -> Fixture {
    fixture_with_environment_inputs(root_source, child_source, Vec::new())
}

fn fixture_with_environment_inputs(
    root_source: &str,
    child_source: Option<&str>,
    environment_rows: Vec<(
        Arc<SourceDocument>,
        SourceBackedEnvironmentRegistrationInput,
    )>,
) -> Fixture {
    fixture_with_registration_inputs(root_source, child_source, environment_rows, Vec::new())
}

fn fixture_with_registration_inputs(
    root_source: &str,
    child_source: Option<&str>,
    environment_rows: Vec<(
        Arc<SourceDocument>,
        SourceBackedEnvironmentRegistrationInput,
    )>,
    character_rows: Vec<(Arc<SourceDocument>, SourceBackedCharacterCatalog)>,
) -> Fixture {
    fixture_with_registration_inputs_and_base(
        root_source,
        child_source,
        environment_rows,
        character_rows,
        TypeCheckEnv::standard(),
    )
}

fn fixture_with_base_environment(
    root_source: &str,
    child_source: Option<&str>,
    base: TypeCheckEnv,
) -> Fixture {
    fixture_with_registration_inputs_and_base(
        root_source,
        child_source,
        Vec::new(),
        Vec::new(),
        base,
    )
}

fn fixture_with_registration_inputs_and_base(
    root_source: &str,
    child_source: Option<&str>,
    environment_rows: Vec<(
        Arc<SourceDocument>,
        SourceBackedEnvironmentRegistrationInput,
    )>,
    character_rows: Vec<(Arc<SourceDocument>, SourceBackedCharacterCatalog)>,
    base: TypeCheckEnv,
) -> Fixture {
    fixture_with_all_registration_inputs_and_base(
        root_source,
        child_source,
        environment_rows,
        character_rows,
        Vec::new(),
        base,
    )
}

fn fixture_with_all_registration_inputs_and_base(
    root_source: &str,
    child_source: Option<&str>,
    environment_rows: Vec<(
        Arc<SourceDocument>,
        SourceBackedEnvironmentRegistrationInput,
    )>,
    character_rows: Vec<(Arc<SourceDocument>, SourceBackedCharacterCatalog)>,
    external_rows: Vec<ExternalRegistrationFact>,
    base: TypeCheckEnv,
) -> Fixture {
    let package = CallablePackageId::try_new("final-analysis-tests").expect("package");
    let root_path = CanonicalModulePath::crate_root();
    let mut database = HirDatabase::try_new().expect("HIR database");
    let (root_document, root_parsed) =
        parse("arcweft-test://sema/final/root", "root.arcw", root_source);
    let mut staged = vec![(root_path.clone(), root_document, root_parsed)];
    if let Some(child_source) = child_source {
        let child_path = root_path.join(ModuleSegment::new("child").expect("module segment"));
        let (child_document, child_parsed) = parse(
            "arcweft-test://sema/final/child",
            "child.arcw",
            child_source,
        );
        staged.push((child_path, child_document, child_parsed));
    }
    let root_document = Arc::clone(&staged[0].1);
    let world = ProjectSymbolWorldId::try_new(
        package.clone(),
        root_document.identity().id().clone(),
        "test",
    )
    .expect("symbol world");
    let mut documents = staged
        .iter()
        .map(|(_, document, _)| Arc::clone(document))
        .collect::<Vec<_>>();
    documents.extend(
        environment_rows
            .iter()
            .map(|(document, _)| Arc::clone(document)),
    );
    documents.extend(
        character_rows
            .iter()
            .map(|(document, _)| Arc::clone(document)),
    );
    let facts = ProjectRegistrationFacts::try_new(
        world.clone(),
        documents,
        external_rows,
        character_rows
            .into_iter()
            .map(|(_, catalog)| catalog)
            .collect(),
        environment_rows
            .into_iter()
            .map(|(_, input)| input)
            .collect(),
    )
    .expect("registration facts");
    let published = publish_fixture_modules(&mut database, &package, &staged, world, &facts);
    let rows = staged
        .into_iter()
        .map(|(path, document, _)| {
            let module = Arc::clone(&published[&path]);
            (path, document, module)
        })
        .collect::<Vec<_>>();
    let modules = rows
        .iter()
        .map(|(path, _, module)| {
            HirProjectModule::try_new(
                &database,
                &package,
                path,
                module.provenance().source_identity(),
                Arc::clone(module),
            )
            .expect("project module")
        })
        .collect::<Vec<_>>();
    let mut builder = HirProjectBuilder::new(&database, package.clone());
    for module in modules {
        builder.insert_module(module).expect("module insertion");
    }
    let project = builder.finish().expect("HIR project");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(base),
        project.view(),
        &facts,
        None,
    ))
    .expect("registered semantic world");
    let symbols = Arc::clone(&registered.symbols);
    Fixture {
        project,
        symbols,
        registered,
        root_document,
    }
}

fn publish_fixture_modules(
    database: &mut HirDatabase,
    package: &CallablePackageId,
    staged: &[(CanonicalModulePath, Arc<SourceDocument>, ParsedSource)],
    world: ProjectSymbolWorldId,
    facts: &ProjectRegistrationFacts,
) -> std::collections::BTreeMap<CanonicalModulePath, Arc<HirModule>> {
    let transaction = database
        .stage_proof_return_project(
            staged.iter().map(|(path, document, parsed)| {
                LoweringRequest::try_new(
                    HirModuleKey::new(package.clone(), path.clone(), document.identity().clone()),
                    parsed,
                )
                .expect("lowering request")
            }),
            world,
            *facts.symbol_revision(),
            facts.documents().map(|document| document.identity()),
            arcweft_lang_hir::lowering::HirLoweringControl::new(),
        )
        .expect("staged HIR project");
    let semantic_facts = HirProofReturnSemanticFactSet::try_new(
        Arc::clone(transaction.generation()),
        transaction.headers().cloned(),
        [],
    )
    .expect("final-analysis fixtures have no authored Proof returns");
    transaction
        .publish_with_semantic_facts(database, semantic_facts)
        .expect("published HIR project")
        .into_iter()
        .map(|output| {
            let path = output.module().key().path().clone();
            (path, output.into_module())
        })
        .collect()
}

fn environment_overload_fixture(root_source: &str) -> Fixture {
    let owner_id = "final-analysis-overloads";
    let package = AdapterPackageId::try_new(owner_id).expect("adapter package ID");
    let owner = EnvironmentCallableOwner::Adapter(package.clone());
    let document = source_document(
        "arcweft-test://sema/final/overloads",
        "overloads.environment",
        "choose i64 u64",
    );
    let span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("environment source span");
    let path = ProjectCallablePath::new(
        CallablePackageId::try_new(package.as_str()).expect("callable package ID"),
        CanonicalModulePath::crate_root(),
        crate::callable::CallablePath::try_new([
            CallableName::try_new("choose").expect("callable name")
        ])
        .expect("callable path"),
    );
    let records = [
        EnvironmentTypeProjectionKind::I64,
        EnvironmentTypeProjectionKind::U64,
    ]
    .into_iter()
    .enumerate()
    .map(|(ordinal, ty)| {
        let overload = CallableOverloadIndex::try_from_usize(ordinal).expect("overload index");
        let parameter = EnvironmentParameterInput::new(
            CallableParameterIndex::try_from_usize(0).expect("parameter index"),
            Some(CallableName::try_new("value").expect("parameter name")),
            EnvironmentParameterTypeInput::Exact(EnvironmentTypeProjectionNode::new(
                span.clone(),
                ty.clone(),
            )),
            CallableParameterPassing::PositionalOrNamed,
            CallableParameterPresence::Required,
            EnvironmentParameterMetadataInput::new(None, None),
        );
        EnvironmentCallablePublicationRecordInput::new(
            EnvironmentPublicationItemId::AdapterFunction {
                owner: owner.clone(),
                path: path.clone(),
                overload,
            },
            EnvironmentCallableKind::Function,
            EnvironmentCallableLookupInput::Free(path.clone()),
            overload,
            EnvironmentCallableSignatureInput::new(
                [EnvironmentParameterGroupInput::new(
                    CallableGroupIndex::ZERO,
                    CallableGroupKind::Initial,
                    [parameter],
                )],
                EnvironmentTypeProjectionNode::new(span.clone(), ty),
                EffectRow::closed(EffectSet::new()),
                CallableArgumentPolicy::new(
                    UnknownNamedArgumentPolicy::Reject,
                    SpreadArgumentPolicy::Reject,
                ),
                CallableValidator::Ordinary,
            ),
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal).expect("declaration ordinal"),
            EnvironmentCallablePublicationMetadataInput::new(
                CallableDocumentation::missing(),
                None,
                None,
            ),
        )
    })
    .collect::<Vec<_>>();
    let input = SourceBackedEnvironmentRegistrationInput::new(
        owner,
        document.identity().clone(),
        EnvironmentManifestDigest::from_bytes([31; 32]),
        [],
        [],
        [],
        records,
    );
    fixture_with_environment_inputs(root_source, None, vec![(document, input)])
}

const AKANE_CHARACTER_MANIFEST: &str = r#"{
  "format": "arcweft.character",
  "version": 1,
  "character": "character.akane",
  "canvas": { "width": 64, "height": 128 },
  "anchor": { "x": 32, "y": 128 },
  "default_look": "normal",
  "parts": [{
    "id": "body",
    "z": 0,
    "variants": [{
      "id": "default",
      "asset": "layers/body.png",
      "rect": { "x": 0, "y": 0, "width": 64, "height": 128 },
      "opacity": 255,
      "blend": "normal",
      "clipping": false
    }]
  }],
  "looks": [{
    "id": "normal",
    "select": [{ "part": "body", "variant": "default" }]
  }]
}"#;

fn akane_character_registration() -> (
    Arc<SourceDocument>,
    SourceBackedCharacterCatalog,
    ExternalRegistrationFact,
) {
    let manifest_document = source_document(
        "arcweft-test://sema/final/character-akane",
        "character-akane.json",
        AKANE_CHARACTER_MANIFEST,
    );
    let manifest = SourceBackedCharacterManifest::decode_registration_json(&manifest_document)
        .expect("source-backed Character manifest");
    let owner = manifest.manifest().character().clone();
    let declaration = manifest
        .source_map()
        .token(&CharacterManifestTokenPath::Root(
            CharacterManifestRootField::Character,
        ))
        .expect("Character owner token")
        .value()
        .clone();
    let compact_segments = owner
        .compact_segments()
        .map(|segment| {
            ProjectSymbolSegment::try_new(segment.to_owned()).expect("Character segment")
        })
        .collect::<Vec<_>>();
    let qualified = ProjectSymbolPath::new(
        ModulePathRoot::ImplicitCrate,
        std::iter::once(ProjectSymbolSegment::try_new("character").expect("namespace"))
            .chain(compact_segments.iter().cloned()),
    )
    .expect("qualified Character binding");
    let compact = ProjectSymbolPath::new(ModulePathRoot::ImplicitCrate, compact_segments)
        .expect("compact Character binding");
    let bindings = [qualified, compact]
        .into_iter()
        .map(|path| {
            ProjectDirectBinding::try_new(
                CanonicalModulePath::crate_root(),
                path,
                Some(Visibility::Public),
                declaration.clone(),
                false,
            )
            .expect("Character direct binding")
        })
        .collect();
    let seed = ExternalDeclarationSeed::try_new(
        SymbolPath::try_new(ModulePathRoot::ImplicitCrate, Vec::new(), owner.as_str())
            .expect("canonical Character path"),
        Some(Visibility::Public),
        declaration.clone(),
        bindings,
    )
    .expect("Character external declaration");
    let external =
        ExternalRegistrationFact::new(seed, RegisteredExternalOwner::Character(owner), declaration);
    let catalog =
        SourceBackedCharacterCatalog::try_new(manifest_document.identity().clone(), vec![manifest])
            .expect("source-backed Character catalog");
    (manifest_document, catalog, external)
}

fn character_nominal_fixture(root_source: &str) -> Fixture {
    let (manifest_document, catalog, _) = akane_character_registration();
    fixture_with_registration_inputs(
        root_source,
        None,
        Vec::new(),
        vec![(manifest_document, catalog)],
    )
}

fn external_character_fixture(root_source: &str) -> Fixture {
    let (manifest_document, catalog, external) = akane_character_registration();
    fixture_with_all_registration_inputs_and_base(
        root_source,
        None,
        Vec::new(),
        vec![(manifest_document, catalog)],
        vec![external],
        TypeCheckEnv::standard(),
    )
}

#[derive(Clone)]
struct TestCallableParameter {
    ty: CallableParameterType,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
}

impl TestCallableParameter {
    fn exact(ty: TypeKind) -> Self {
        Self {
            ty: CallableParameterType::Exact(ty),
            passing: CallableParameterPassing::PositionalOrNamed,
            presence: CallableParameterPresence::Required,
        }
    }

    fn typed_rest(ty: TypeKind) -> Self {
        Self {
            ty: CallableParameterType::Exact(ty),
            passing: CallableParameterPassing::RestPositional,
            presence: CallableParameterPresence::Required,
        }
    }
}

struct TestCallableOverload {
    parameters: Vec<TestCallableParameter>,
    result: TypeKind,
    effects: EffectSet,
    spread: SpreadArgumentPolicy,
}

impl TestCallableOverload {
    fn strict(parameters: impl IntoIterator<Item = TypeKind>, result: TypeKind) -> Self {
        Self {
            parameters: parameters
                .into_iter()
                .map(TestCallableParameter::exact)
                .collect(),
            result,
            effects: EffectSet::new(),
            spread: SpreadArgumentPolicy::Reject,
        }
    }

    fn typed_rest(item: TypeKind, result: TypeKind) -> Self {
        Self {
            parameters: vec![TestCallableParameter::typed_rest(item)],
            result,
            effects: EffectSet::new(),
            spread: SpreadArgumentPolicy::TypedRest,
        }
    }

    fn fixed_literal(parameters: impl IntoIterator<Item = TypeKind>, result: TypeKind) -> Self {
        Self {
            parameters: parameters
                .into_iter()
                .map(TestCallableParameter::exact)
                .collect(),
            result,
            effects: EffectSet::new(),
            spread: SpreadArgumentPolicy::FixedLiteralOnly,
        }
    }

    fn effectful(result: TypeKind, effect: &str) -> Self {
        Self {
            parameters: Vec::new(),
            result,
            effects: EffectSet::from_labels([effect]).expect("valid test effect"),
            spread: SpreadArgumentPolicy::Reject,
        }
    }
}

/// Replaces only the accepted environment callable catalog while preserving
/// the source-backed project, nominal generation, and symbol authority. This
/// lets the matrix exercise typed schemas that the environment input codec
/// intentionally cannot author (notably function and detached generic types).
fn typed_overload_fixture(
    root_source: &str,
    name: &str,
    overloads: Vec<TestCallableOverload>,
) -> Fixture {
    typed_overload_fixture_with_catalog_limits(
        root_source,
        name,
        overloads,
        PRODUCTION_CALLABLE_LIMITS,
    )
}

fn typed_overload_fixture_with_catalog_limits(
    root_source: &str,
    name: &str,
    overloads: Vec<TestCallableOverload>,
    catalog_limits: CallableLimits,
) -> Fixture {
    let Fixture {
        project,
        symbols,
        registered,
        root_document,
    } = fixture(root_source, None);
    let accepted = registered.environment().callable_catalog();
    let callable_path =
        CallablePath::try_new([CallableName::try_new(name).expect("test callable name")])
            .expect("test callable path");
    let key = CallableLookupKey::Free(callable_path.clone());
    let package =
        AdapterPackageId::try_new("final-analysis-typed-overloads").expect("adapter package ID");
    let owner = EnvironmentCallableOwner::Adapter(package.clone());
    let provider = CallableProviderId::Adapter(package);
    let mut entries = Vec::with_capacity(overloads.len());
    let mut by_id = HashMap::with_capacity(overloads.len());
    for (ordinal, overload) in overloads.into_iter().enumerate() {
        let (environment_id, record, entry) =
            typed_overload_entry(&owner, &provider, &key, ordinal, overload);
        entries.push(entry);
        assert!(by_id.insert(environment_id, record).is_none());
    }
    let set = NonEmptyCallableSet::try_new(entries, &catalog_limits).expect("test overload set");
    let environment = EnvironmentCallableCatalog::new(
        HashMap::from([(callable_path, set)]),
        HashMap::new(),
        by_id,
    );
    let replacement = Arc::new(RegisteredCallableCatalog::new(
        accepted.nominal_world().clone(),
        accepted.project().clone(),
        environment,
        accepted.nominal_resolutions().clone(),
    ));
    let registered = registered.with_callable_catalog_for_test(replacement);
    Fixture {
        project,
        symbols,
        registered,
        root_document,
    }
}

fn typed_overload_entry(
    owner: &EnvironmentCallableOwner,
    provider: &CallableProviderId,
    key: &CallableLookupKey,
    ordinal: usize,
    overload: TestCallableOverload,
) -> (
    EnvironmentCallableId,
    Arc<CallableRecord>,
    CatalogCallableEntry,
) {
    let parameters = overload
        .parameters
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(format!("value{index}")).expect("parameter name")),
                parameter.ty,
                parameter.passing,
                parameter.presence,
                None,
                None,
            )
            .expect("test callable parameter")
        })
        .collect::<Vec<_>>();
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        parameters,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("test callable group");
    let schema = Arc::new(
        CallableSignatureSchema::try_new(
            vec![group],
            overload.result,
            CallableEffectSchema::Fixed(EffectRow::closed(overload.effects)),
            CallableArgumentPolicy::new(UnknownNamedArgumentPolicy::Reject, overload.spread),
            CallableValidator::Ordinary,
            &PRODUCTION_CALLABLE_LIMITS,
        )
        .expect("test callable schema"),
    );
    let environment_id = EnvironmentCallableId::new(
        owner.clone(),
        EnvironmentCallableKind::Function,
        key.clone(),
        CallableOverloadIndex::try_from_usize(ordinal).expect("overload index"),
    );
    let record = Arc::new(
        CallableRecord::try_new(
            CallableCandidateId::Environment(environment_id.clone()),
            key.clone(),
            CallableAuthorityRank::Adapter,
            provider.clone(),
            CallableAccess::Environment,
            schema,
            CallableDocumentation::missing(),
            None,
            None,
            Some(test_environment_publication_digest(ordinal)),
            EnvironmentDeclarationOrdinal::try_from_usize(ordinal).expect("declaration ordinal"),
        )
        .expect("test callable record"),
    );
    let entry =
        CatalogCallableEntry::try_new(Arc::clone(&record), Vec::new(), &PRODUCTION_CALLABLE_LIMITS)
            .expect("test callable entry");
    (environment_id, record, entry)
}

fn test_environment_publication_digest(ordinal: usize) -> EnvironmentCallablePublicationDigest {
    let ordinal = u64::try_from(ordinal)
        .expect("test overload ordinal fits the publication digest")
        .checked_add(1)
        .expect("test overload ordinal increment");
    let ordinal = ordinal.to_le_bytes();
    let mut digest = [0; 32];
    digest[..ordinal.len()].copy_from_slice(&ordinal);
    EnvironmentCallablePublicationDigest::from_bytes(digest)
}

fn callable_limits_with_catalog_overloads(max_overloads_per_key: usize) -> CallableLimits {
    let production = PRODUCTION_CALLABLE_LIMITS;
    CallableLimits::for_test(
        production.max_path_segments(),
        production.max_groups_per_callable(),
        production.max_parameters_per_callable(),
        max_overloads_per_key,
        production.max_candidates_per_call(),
        production.max_nested_calls(),
        production.max_recovery_nodes(),
        production.max_diagnostics(),
        production.max_catalog_build_work(),
        production.max_query_work(),
    )
}

fn candidate_boundary_fixture(candidate_count: usize) -> Fixture {
    assert!(
        candidate_count > 0,
        "candidate boundary fixture is non-empty"
    );
    let mut overloads = Vec::with_capacity(candidate_count);
    overloads.push(TestCallableOverload::strict([TypeKind::I64], TypeKind::I64));
    overloads.extend(
        (1..candidate_count).map(|_| TestCallableOverload::strict([TypeKind::U64], TypeKind::U64)),
    );
    typed_overload_fixture_with_catalog_limits(
        "fn caller() { candidate_boundary(1i64); }\n",
        "candidate_boundary",
        overloads,
        callable_limits_with_catalog_overloads(candidate_count),
    )
}

fn checked_callables(
    fixture: &Fixture,
    input: &FinalSemanticAnalysisInput,
) -> Arc<crate::callable::CheckedCallableCatalog> {
    super::analyzer::freeze_checked_callables_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        super::FinalSemanticCatalogs::production(&fixture.registered),
        input,
    )
    .expect("checked callable catalog")
}

fn analyze(fixture: &Fixture) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    analyze_final_project(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
}

fn analyze_with_assertion_profile(
    fixture: &Fixture,
    profile: AssertionBuildProfile,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    analyze_final_project(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation).with_assertion_build_profile(profile),
    )
}

fn function_owner(fixture: &Fixture, name: &str) -> arcweft_lang_hir::identity::ItemId {
    fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .flat_map(|(_, module)| module.items())
        .find_map(|(owner, item)| match item.kind() {
            HirItemKind::Function(function)
                if function
                    .name()
                    .resolved()
                    .is_some_and(|candidate| candidate.as_str() == name) =>
            {
                Some(owner)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function `{name}`"))
}

fn checked_function_facts<'a>(
    report: &'a FinalSemanticAnalysis,
    fixture: &Fixture,
    name: &str,
) -> &'a crate::callable::CheckedCallableFacts {
    let owner = function_owner(fixture, name);
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == owner)
        .unwrap_or_else(|| panic!("missing callable symbol for `{name}`"))
        .declaration();
    report
        .checked_callables()
        .project_callable(declaration)
        .unwrap_or_else(|_| panic!("missing checked callable facts for `{name}`"))
}

fn callable_limits_with_query_work(max_query_work: u64) -> CallableLimits {
    let production = PRODUCTION_CALLABLE_LIMITS;
    CallableLimits::for_test(
        production.max_path_segments(),
        production.max_groups_per_callable(),
        production.max_parameters_per_callable(),
        production.max_overloads_per_key(),
        production.max_candidates_per_call(),
        production.max_nested_calls(),
        production.max_recovery_nodes(),
        production.max_diagnostics(),
        production.max_catalog_build_work(),
        max_query_work,
    )
}

fn analyze_with_query_work(
    fixture: &Fixture,
    max_query_work: u64,
) -> (
    Result<FinalSemanticAnalysis, FinalSemanticAnalysisError>,
    Vec<super::PhysicalCandidateArgumentEvaluation>,
) {
    let cancellation = AtomicBool::new(false);
    super::analyzer::analyze_final_project_with_physical_trace_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered)
            .with_callable_limits(callable_limits_with_query_work(max_query_work)),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
}

fn analyze_with_callable_limits(
    fixture: &Fixture,
    callable_limits: CallableLimits,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    analyze_final_project(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered)
            .with_callable_limits(callable_limits),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
}

fn resolve_single_call_directly(
    fixture: &Fixture,
) -> (
    arcweft_lang_hir::identity::ExprId,
    ResolveCallOutcome,
    crate::callable::CallResolverAccountingReport,
) {
    let checked = checked_callables(fixture, &complete_input(fixture));
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, call) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Call(call) => Some((owner, call)),
            _ => None,
        })
        .expect("one final-HIR Call expression");
    let expressions = BTreeMap::new();
    let calls = BTreeMap::new();
    let nominal_receivers = BTreeMap::new();
    let enum_variants = BTreeMap::new();
    let authority = CallResolverAuthority::accepted(
        project.project_view(),
        module,
        &fixture.symbols,
        &fixture.registered,
    );
    let prepared = prepare_final_call_callee(
        authority,
        owner,
        FinalCallCalleeFacts::new(&expressions, &calls, &nominal_receivers, &enum_variants),
        CharacterDialoguePatchContext::ReusableValue,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("free environment Call callee preparation");
    let argument_count = u64::try_from(call.arguments().len()).expect("argument count");
    let cancellation = AtomicBool::new(false);
    let mut work = ResolverWork::new(PRODUCTION_CALLABLE_LIMITS.max_query_work());
    work.record_logical_argument_checks(argument_count)
        .expect("candidate-neutral logical argument accounting");
    let request = CallResolverRequest::try_new(
        prepared.as_borrowed(),
        &CallResolverContext {
            authority,
            checked: checked.as_ref().into(),
            expected: None,
            call_group: CallableGroupIndex::ZERO,
            expression: owner,
            cancellation: &cancellation,
            limits: &PRODUCTION_CALLABLE_LIMITS,
        },
        &mut work,
    )
    .expect("validated direct shared-resolver request");
    let outcome = resolve_call_target(request);
    (owner, outcome, work.call_accounting())
}

fn set_expression_effect(
    input: &mut FinalSemanticAnalysisInput,
    owner: arcweft_lang_hir::identity::ExprId,
    effect: &str,
) -> EffectSet {
    let (_, fact) = input
        .expressions
        .iter_mut()
        .find(|(candidate, _)| *candidate == owner)
        .expect("checked expression fixture");
    let effects = EffectSet::from_labels([effect]).expect("effect fixture");
    *fact = CheckedExpression::new(
        fact.ty().clone(),
        fact.type_selection(),
        effects.clone(),
        fact.resolution().clone(),
    );
    effects
}

fn complete_input(fixture: &Fixture) -> FinalSemanticAnalysisInput {
    let executable = fixture.project.executable_view().expect("executable HIR");
    let mut input = FinalSemanticAnalysisInput::new();
    for (_, module) in executable.modules() {
        for (id, _) in module.types() {
            input.push_type(id, TypeKind::Unit);
        }
        for (id, _) in module.locals() {
            input.push_local(id, CheckedBinding::new(TypeKind::Unit));
        }
        for (id, _) in module.captures() {
            input.push_capture(id, CheckedBinding::new(TypeKind::Unit));
        }
        push_complete_expression_facts(module, &mut input);
        push_complete_pattern_facts(module, &mut input);
        push_complete_statement_facts(module, &mut input);
        push_complete_item_facts(module, &mut input);
    }
    input
}

fn push_complete_expression_facts(module: &HirModule, input: &mut FinalSemanticAnalysisInput) {
    for (id, expression) in module.expressions() {
        let (ty, resolution) = match expression.kind() {
            HirExprKind::Literal(literal) => (
                TypeKind::Unit,
                CheckedExpressionResolution::Literal(literal.clone()),
            ),
            HirExprKind::Path(_) | HirExprKind::EntityReference(_) => (
                TypeKind::Unit,
                CheckedExpressionResolution::Value(CheckedValueResolution::Registered(
                    RegisteredSemanticValueId::from_bytes([7; 32]),
                )),
            ),
            HirExprKind::Call(_) => (TypeKind::Unit, CheckedExpressionResolution::Call),
            HirExprKind::Error(_) => panic!("executable fixture contains poisoned expression"),
            _ => (TypeKind::Unit, CheckedExpressionResolution::Structural),
        };
        input.push_expression(
            id,
            CheckedExpression::new(
                ty,
                CheckedTypeSelection::Inferred,
                EffectSet::new(),
                resolution,
            ),
        );
    }
}

fn push_complete_pattern_facts(module: &HirModule, input: &mut FinalSemanticAnalysisInput) {
    for (id, pattern) in module.patterns() {
        let resolution = match pattern.kind() {
            HirPatternKind::Literal(literal) => CheckedPatternResolution::Literal(literal.clone()),
            HirPatternKind::Error(_) => panic!("executable fixture contains poisoned pattern"),
            _ => CheckedPatternResolution::Structural,
        };
        input.push_pattern(id, CheckedPattern::new(TypeKind::Unit, resolution));
    }
}

fn push_complete_statement_facts(module: &HirModule, input: &mut FinalSemanticAnalysisInput) {
    for (id, statement) in module.statements() {
        let role = match statement.kind() {
            HirStmtKind::Assertion { .. } => {
                CheckedStatementRole::Assertion(CheckedAssertionDisposition::Discharged)
            }
            HirStmtKind::For(_) => {
                CheckedStatementRole::Iteration(Box::new(CheckedIteration::Builtin {
                    family: CheckedIteratorFamily::Seq,
                    item: TypeKind::Unit,
                }))
            }
            HirStmtKind::Yield { .. } => CheckedStatementRole::Yield,
            HirStmtKind::UnsafeLifetime { .. } => CheckedStatementRole::UnsafeAudit,
            HirStmtKind::Wait { .. } | HirStmtKind::AwaitWith(_) | HirStmtKind::LetAwait { .. } => {
                CheckedStatementRole::Suspension
            }
            HirStmtKind::Error => panic!("executable fixture contains poisoned statement"),
            _ => CheckedStatementRole::Ordinary,
        };
        input.push_statement(id, CheckedStatement::new(EffectSet::new(), role));
    }
}

fn push_complete_item_facts(module: &HirModule, input: &mut FinalSemanticAnalysisInput) {
    for (id, item) in module.items() {
        let role = match item.kind() {
            HirItemKind::Module(_) => CheckedItemRole::Module,
            HirItemKind::Use(_) => CheckedItemRole::Use,
            HirItemKind::Flow(flow) => CheckedItemRole::Flow {
                identity: flow.identity().clone(),
            },
            HirItemKind::Function(_) => CheckedItemRole::Function {
                execution: CheckedFunctionExecution::DirectFrame,
                suspension: CheckedSuspensionRole::NonSuspending,
            },
            HirItemKind::Predicate(_) => CheckedItemRole::Predicate,
            HirItemKind::Proof(_) => CheckedItemRole::Proof,
            HirItemKind::Trait(_) => CheckedItemRole::Trait,
            HirItemKind::Impl(_) => CheckedItemRole::Impl,
            HirItemKind::Enum(_) => CheckedItemRole::Enum,
            HirItemKind::Struct(_) => CheckedItemRole::Struct,
            HirItemKind::TypeAlias(_) => CheckedItemRole::TypeAlias,
            HirItemKind::Resource(_) => CheckedItemRole::Resource,
            HirItemKind::Character(_) => CheckedItemRole::Character,
            HirItemKind::View(_) => CheckedItemRole::View,
            HirItemKind::Action(_) => CheckedItemRole::Action,
            HirItemKind::Activity(_) => CheckedItemRole::Activity,
            HirItemKind::Signal(_) => CheckedItemRole::Signal,
            HirItemKind::Metric(_) => CheckedItemRole::Metric,
            HirItemKind::Layer(_) => CheckedItemRole::Layer,
            HirItemKind::Entry(_) => CheckedItemRole::Entry,
            HirItemKind::ExternCapability(_) => CheckedItemRole::ExternCapability,
            HirItemKind::Test(_) => CheckedItemRole::Test,
            HirItemKind::Bench(_) => CheckedItemRole::Bench,
            HirItemKind::Source(_) => CheckedItemRole::Source,
            HirItemKind::Style(_) => CheckedItemRole::Style,
            HirItemKind::Error(_) => panic!("executable fixture contains poisoned item"),
        };
        input.push_item(id, CheckedItem::new(EffectSet::new(), role));
    }
}

#[test]
fn assertion_dispositions_require_typed_build_and_proof_admission() {
    let fixture = fixture(
        concat!(
            "flow assertions {\n",
            "    assert.debug(true)\n",
            "    assert.check(true)\n",
            "    assert.prove(true)\n",
            "}\n",
        ),
        None,
    );
    let debug = analyze_with_assertion_profile(&fixture, AssertionBuildProfile::Debug)
        .expect("Debug assertion analysis");
    let release = analyze_with_assertion_profile(&fixture, AssertionBuildProfile::Release)
        .expect("Release assertion analysis");
    let dispositions = |analysis: &FinalSemanticAnalysis| {
        analysis
            .statements()
            .filter_map(|(_, statement)| match statement.role() {
                CheckedStatementRole::Assertion(disposition) => Some(*disposition),
                _ => None,
            })
            .collect::<Vec<_>>()
    };

    assert_eq!(
        dispositions(&debug),
        vec![
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::DebugGuard),
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard),
            CheckedAssertionDisposition::PendingProof,
        ]
    );
    assert_eq!(
        dispositions(&release),
        vec![
            CheckedAssertionDisposition::OmittedDebug,
            CheckedAssertionDisposition::Runtime(AssertionRuntimePolicy::AlwaysGuard),
            CheckedAssertionDisposition::PendingProof,
        ]
    );
}

#[test]
fn proof_runtime_assertions_are_context_errors() {
    let ordinary = fixture("fn ordinary() { assert.check(true) }\n", None);
    let ordinary = analyze(&ordinary).expect("ordinary function admits Check assertions");
    assert_eq!(
        ordinary
            .statements()
            .filter_map(|(_, statement)| match statement.role() {
                CheckedStatementRole::Assertion(disposition) => Some(*disposition),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [CheckedAssertionDisposition::Runtime(
            AssertionRuntimePolicy::AlwaysGuard,
        )]
    );

    let proof = fixture("proof accepted() { assert.prove(true) }\n", None);
    let proof = analyze(&proof).expect("Proof body admits only Prove assertions");
    assert_eq!(
        proof
            .statements()
            .filter_map(|(_, statement)| match statement.role() {
                CheckedStatementRole::Assertion(disposition) => Some(*disposition),
                _ => None,
            })
            .collect::<Vec<_>>(),
        [CheckedAssertionDisposition::PendingProof]
    );

    for (spelling, expected) in [
        (
            "check",
            arcweft_lang_syntax::assertion::AssertionMode::Check,
        ),
        (
            "debug",
            arcweft_lang_syntax::assertion::AssertionMode::Debug,
        ),
    ] {
        let invalid = fixture(
            &format!("proof invalid() {{ assert.{spelling}(true) }}\n"),
            None,
        );
        assert!(matches!(
            analyze(&invalid),
            Err(FinalSemanticAnalysisError::AssertionModeNotAllowed {
                mode,
                context: AssertionContext::ProofBody,
                ..
            }) if mode == expected
        ));
    }
}

#[test]
fn predicate_assertion_is_context_error_not_reparse() {
    let source = concat!(
        "predicate invalid() {\n",
        "    assert.prove(true);\n",
        "    true\n",
        "}\n",
    );
    let fixture = fixture(source, None);
    let error = analyze(&fixture).expect_err("Predicate assertions are semantic context errors");
    let owner = match error {
        FinalSemanticAnalysisError::AssertionModeNotAllowed {
            owner,
            mode: arcweft_lang_syntax::assertion::AssertionMode::Prove,
            context: AssertionContext::PredicateBody,
        } => owner,
        other => panic!("unexpected predicate assertion result: {other:?}"),
    };
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root HIR module");
    assert!(
        !module
            .resolve_stmt(owner)
            .expect("typed statement owner")
            .is_poisoned()
    );
    let source_site = module
        .source_site(
            fixture.root_document.identity(),
            HirSourceQuery::Stmt {
                owner,
                role: HirStmtSourceRole::Whole,
            },
        )
        .expect("typed assertion source lookup");
    let HirSourcePresence::Present(HirSourceSite::Span(span)) = source_site.presence() else {
        panic!("assertion statement must retain its exact authored source span")
    };
    assert_eq!(&source[span.range().as_range()], "assert.prove(true);");
}

#[test]
fn callable_body_result_mismatch_is_rejected_by_final_typed_authority() {
    let mismatch = fixture("fn main() -> i32 { true }\n", None);
    assert!(matches!(
        analyze(&mismatch),
        Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. })
    ));
}

#[test]
fn registered_environment_binding_type_change_forces_final_semantic_recheck() {
    let accepted = fixture_with_base_environment(
        "fn main() -> i32 { configured }\n",
        None,
        TypeCheckEnv::standard().with_symbol("configured", TypeKind::I32),
    );
    let report = analyze(&accepted).expect("registered environment value has its accepted type");
    let registered = report
        .expressions()
        .find_map(|(_, checked)| match checked.resolution() {
            CheckedExpressionResolution::Value(CheckedValueResolution::Registered(value)) => {
                value.environment_binding()
            }
            _ => None,
        })
        .expect("registered environment binding identity");
    assert_eq!(registered.as_str(), "configured");

    let changed = fixture_with_base_environment(
        "fn main() -> i32 { configured }\n",
        None,
        TypeCheckEnv::standard().with_symbol("configured", TypeKind::Bool),
    );
    assert!(matches!(
        analyze(&changed),
        Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. })
    ));
}

#[test]
fn predicate_and_proof_recursion_sccs_are_rejected() {
    fn recursive_edges(fixture: &Fixture) -> Box<[super::RecursiveCallableContractEdge]> {
        let error = analyze(fixture).expect_err("Predicate/Proof recursion must be rejected");
        match error {
            FinalSemanticAnalysisError::RecursiveCallableContract { edges } => {
                let diagnostic = FinalSemanticAnalysisError::RecursiveCallableContract {
                    edges: edges.clone(),
                };
                assert_eq!(
                    diagnostic.diagnostic_code(),
                    "sema.callable.recursive_contract"
                );
                edges
            }
            other => panic!("unexpected recursion result: {other:?}"),
        }
    }

    let predicate_self = fixture("predicate self_cycle() = self_cycle()\n", None);
    let edges = recursive_edges(&predicate_self);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].caller(), edges[0].callee());

    let proof_self = fixture("proof self_cycle() { self_cycle(); }\n", None);
    let edges = recursive_edges(&proof_self);
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].caller(), edges[0].callee());

    let mutual = fixture(
        concat!(
            "predicate first() = second()\n",
            "predicate second() = first()\n",
        ),
        None,
    );
    let edges = recursive_edges(&mutual);
    assert_eq!(edges.len(), 2, "each participating call edge is retained");
    assert!(edges.iter().all(|edge| edge.caller() != edge.callee()));

    let cross_module = fixture(
        concat!(
            "use crate.child.child_cycle\n",
            "pub predicate root_cycle() = child_cycle()\n",
        ),
        Some(concat!(
            "use crate.root_cycle\n",
            "pub predicate child_cycle() = root_cycle()\n",
        )),
    );
    let edges = recursive_edges(&cross_module);
    assert_eq!(edges.len(), 2, "cross-module SCC retains both call edges");
    assert_eq!(
        edges
            .iter()
            .map(|edge| edge.expression().module())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        2
    );

    let ordinary = fixture("fn ordinary() -> Unit { ordinary(); () }\n", None);
    analyze(&ordinary).expect("ordinary function-only recursion retains its existing policy");
}

#[test]
fn final_assertion_conditions_require_bool_and_empty_effect_rows() {
    let non_bool = fixture("flow invalid_type { assert.check(1) }\n", None);
    let error = analyze(&non_bool).expect_err("non-Bool assertion condition must be rejected");
    let FinalSemanticAnalysisError::AssertionConditionNotBool {
        index: 0, actual, ..
    } = error
    else {
        panic!("unexpected non-Bool assertion result: {error:?}")
    };
    assert_eq!(*actual, TypeKind::I32);

    let effectful = typed_overload_fixture(
        "flow invalid_effect { assert.check(impure_flag()) }\n",
        "impure_flag",
        vec![TestCallableOverload::effectful(TypeKind::Bool, "fs.read")],
    );
    let error = analyze(&effectful).expect_err("effectful assertion condition must be rejected");
    let FinalSemanticAnalysisError::AssertionConditionNotPure {
        index: 0, effects, ..
    } = error
    else {
        panic!("unexpected effectful assertion result: {error:?}")
    };
    assert_eq!(effects.to_labels(), ["fs.read"]);
}

#[test]
fn multi_module_report_is_complete_generation_bound_and_exactly_accounted() {
    let fixture = fixture("fn root() {}\n", Some("fn child() {}\n"));
    let executable = fixture.project.executable_view().expect("executable HIR");
    let expected_expressions = executable
        .modules()
        .map(|(_, module)| module.expressions().len())
        .sum::<usize>();
    let expected_items = executable
        .modules()
        .map(|(_, module)| module.items().len())
        .sum::<usize>();
    let input = complete_input(&fixture);
    let checked_callables = checked_callables(&fixture, &input);
    let report = FinalSemanticAnalysis::try_new(
        executable,
        &fixture.symbols,
        Arc::clone(&checked_callables),
        input,
    )
    .expect("complete semantic generation");

    assert_eq!(
        report.work().expression_facts(),
        u64::try_from(expected_expressions).expect("expression count")
    );
    assert_eq!(
        report.work().item_facts(),
        u64::try_from(expected_items).expect("item count")
    );
    assert_eq!(report.work().call_facts(), 0);
    assert_eq!(report.work().resolver_invocations(), 0);
    assert_eq!(report.call_diagnostics().count(), 0);
    assert!(Arc::ptr_eq(report.checked_callables(), &checked_callables));
    let declaration = fixture
        .symbols
        .callable_symbols()
        .next()
        .expect("fixture callable")
        .declaration();
    let facts = report
        .checked_callables()
        .project_callable(declaration)
        .expect("structural project callable join");
    let accepted_record = fixture
        .registered
        .environment()
        .callable_catalog()
        .project_record(declaration)
        .expect("accepted callable record");
    assert!(Arc::ptr_eq(facts.record(), accepted_record));
    report
        .validate_generation(executable, &fixture.symbols)
        .expect("same accepted generation");
}

#[test]
fn ordinary_function_roles_walk_nested_final_hir_and_publish_suspend_effects() {
    let fixture = fixture(
        r"
fn nested(need: Need<i64, String>) -> Result<i64, String> {
    if true {
        await need
    } else {
        Ok(0i64)
    }
}
",
        None,
    );
    let report = analyze(&fixture).expect("nested direct suspension analysis");
    let owner = function_owner(&fixture, "nested");
    assert_eq!(
        report.item(owner).expect("function fact").role(),
        &CheckedItemRole::Function {
            execution: CheckedFunctionExecution::DirectFrame,
            suspension: CheckedSuspensionRole::MaySuspend,
        }
    );
    assert!(report.expressions().any(|(_, expression)| {
        expression
            .effects()
            .iter()
            .any(|effect| effect.as_str() == "control.suspend")
    }));
    assert!(report.expressions().any(|(_, expression)| {
        matches!(
            expression.ty(),
            TypeKind::Result { ok, error }
                if **ok == TypeKind::I64 && **error == TypeKind::String
        ) && expression
            .effects()
            .iter()
            .any(|effect| effect.as_str() == "control.suspend")
    }));
}

#[test]
fn propagating_await_unwraps_need_inside_a_matching_result_boundary() {
    let fixture = fixture(
        r"
fn nested(need: Need<i64, String>) -> Result<i64, String> {
    Ok(try await need)
}
",
        None,
    );
    let report = analyze(&fixture).expect("propagating Await final analysis");
    assert!(report.expressions().any(|(_, expression)| {
        expression.ty() == &TypeKind::I64
            && expression
                .effects()
                .iter()
                .any(|effect| effect.as_str() == "control.suspend")
    }));
}

#[test]
fn nested_yield_classifies_stream_factory_but_independent_owners_do_not_leak() {
    let generator = fixture(
        r"
fn produce() -> Stream<i64, String> {
    if false {
        yield 1i64;
    }
}
",
        None,
    );
    let report = analyze(&generator).expect("nested generator analysis");
    let owner = function_owner(&generator, "produce");
    assert_eq!(
        report.item(owner).expect("generator fact").role(),
        &CheckedItemRole::Function {
            execution: CheckedFunctionExecution::StreamFactory {
                item: TypeKind::I64,
                error: TypeKind::String,
                own_scope_yields: 1,
            },
            suspension: CheckedSuspensionRole::MaySuspend,
        }
    );

    let direct = fixture(
        r"
fn passthrough(stream: Stream<i64, String>) -> Stream<i64, String> {
    let hidden = || {
        yield 1i64;
    };
    let sequence = seq {
        yield 2i64;
    };
    stream
}
",
        None,
    );
    let report = analyze(&direct).expect("independent execution-owner analysis");
    let owner = function_owner(&direct, "passthrough");
    assert_eq!(
        report.item(owner).expect("direct function fact").role(),
        &CheckedItemRole::Function {
            execution: CheckedFunctionExecution::DirectFrame,
            suspension: CheckedSuspensionRole::NonSuspending,
        }
    );
}

#[test]
fn checked_catalog_keeps_closure_body_effects_latent() {
    let fixture = fixture("fn root() { let callback = || 1; 2 }\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root module");
    let (closure_owner, closure_body) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Closure(closure) => Some((owner, closure.body())),
            _ => None,
        })
        .expect("closure expression");
    let function_tail = module
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::Function(function) => match function.body() {
                HirFunctionBody::Block { tail, .. } => Some(*tail),
                HirFunctionBody::Error(_) => None,
            },
            _ => None,
        })
        .expect("function tail");
    let mut input = complete_input(&fixture);
    let closure_effects = set_expression_effect(&mut input, closure_body, "fs.read");
    let outer_effects = set_expression_effect(&mut input, function_tail, "fs.write");
    let catalog = checked_callables(&fixture, &input);
    let declaration = fixture
        .symbols
        .callable_symbols()
        .next()
        .expect("function declaration")
        .declaration();
    let facts = catalog
        .project_callable(declaration)
        .expect("checked function facts");
    assert_eq!(
        facts
            .actual_row()
            .map(crate::effect_row::EffectRow::concrete),
        Some(&outer_effects),
        "creating the closure must not perform its body effects"
    );

    let closure_source = module
        .source_site(
            module.provenance().source_identity(),
            HirSourceQuery::Expr {
                owner: closure_owner,
                role: HirExprSourceRole::Whole,
            },
        )
        .expect("closure source lookup");
    let HirSourcePresence::Present(HirSourceSite::Span(closure_source)) = closure_source.presence()
    else {
        panic!("closure must retain an authored source span");
    };
    let closure_id =
        CheckedClosureId::from_checked_expression(facts.id().clone(), closure_source.clone())
            .expect("source-bound closure identity");
    assert_eq!(
        catalog
            .closure_row(&closure_id)
            .expect("checked closure row")
            .concrete(),
        &closure_effects
    );
    assert_eq!(
        catalog
            .closure_at_source(closure_source)
            .expect("source-indexed checked closure row")
            .concrete(),
        &closure_effects
    );
    assert!(!outer_effects.contains(&EffectId::parse("fs.read").expect("effect identity")));
}

#[test]
fn incomplete_or_duplicate_fact_sets_never_publish() {
    let fixture = fixture("fn root() {}\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let mut missing = complete_input(&fixture);
    missing.expressions.pop();
    let missing_catalog = checked_callables(&fixture, &missing);
    assert!(matches!(
        FinalSemanticAnalysis::try_new(executable, &fixture.symbols, missing_catalog, missing),
        Err(FinalSemanticAnalysisError::MissingFact { .. })
    ));

    let mut duplicate = complete_input(&fixture);
    let expression = duplicate.expressions[0].clone();
    duplicate.expressions.push(expression);
    let duplicate_catalog = checked_callables(&fixture, &duplicate);
    assert!(matches!(
        FinalSemanticAnalysis::try_new(executable, &fixture.symbols, duplicate_catalog, duplicate),
        Err(FinalSemanticAnalysisError::DuplicateFact { .. })
    ));
}

#[test]
fn cancellation_is_terminal_before_any_report_is_observable() {
    let fixture = fixture("fn root() {}\n", None);
    let cancellation = AtomicBool::new(true);
    let input = complete_input(&fixture);
    let checked_callables = checked_callables(&fixture, &input);
    let result = FinalSemanticAnalysis::try_new_with_control(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        checked_callables,
        input,
        FinalSemanticAnalysisControl::new(&cancellation),
    );
    assert!(matches!(result, Err(FinalSemanticAnalysisError::Cancelled)));
}

#[test]
fn every_call_expression_requires_one_sealed_shared_resolver_fact() {
    let fixture = fixture("fn target() {}\nfn caller() { target(); }\n", None);
    let input = complete_input(&fixture);
    let checked_callables = checked_callables(&fixture, &input);
    let result = FinalSemanticAnalysis::try_new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        checked_callables,
        input,
    );
    assert!(matches!(
        result,
        Err(FinalSemanticAnalysisError::CallFactMismatch)
    ));
}

#[test]
fn contextual_entity_family_child_is_owned_by_its_root_resolution() {
    let fixture = fixture(
        "pub struct RouteInfo { route: Ref<Flow>, speaker: Ref<Character> }\n",
        None,
    );
    let report = analyze(&fixture).expect("entity-family roots have complete final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let roots = module
        .types()
        .filter_map(|(owner, ty)| match ty.kind() {
            HirTypeKind::Generic(generic)
                if generic.base().segments().last().is_some_and(|segment| {
                    matches!(segment, arcweft_lang_hir::leaf::HirPathSegment::Identifier(name) if name.as_str() == "Ref")
                }) => Some((owner, generic.arguments()[0])),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(roots.len(), 2);

    for (root, child) in roots {
        assert!(report.ty(root).is_some(), "Ref root is a runtime type");
        assert_eq!(
            report.ty(child),
            None,
            "contextual entity-family atom is not a standalone runtime type"
        );
        let resolution = report
            .type_resolution(root)
            .expect("one nominal report owns the complete structural root");
        let child = resolution
            .outcome()
            .product()
            .nodes()
            .iter()
            .find(|node| node.node() == child)
            .expect("root report retains the exact contextual child");
        assert!(matches!(
            child.outcome(),
            TypeNameResolution::EntityFamily(EntityKind::Flow | EntityKind::Character)
        ));
        assert_eq!(child.recovered(), None);
    }
}

#[test]
fn alias_use_reports_idempotently_share_the_declaration_target_fact() {
    let fixture = fixture(
        concat!(
            "use crate.child.PublicAlias as ImportedAlias\n",
            "fn identity(value: ImportedAlias) -> crate.child.PublicAlias { value }\n",
        ),
        Some("pub struct Record {}\npub type PublicAlias = Record\n"),
    );
    let report = analyze(&fixture).expect("overlapping alias products agree on one type fact");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let child = executable
        .module(
            &CanonicalModulePath::crate_root()
                .join(ModuleSegment::new("child").expect("module segment")),
        )
        .expect("child HIR module");
    let alias_target = child
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::TypeAlias(alias) => Some(alias.target()),
            _ => None,
        })
        .expect("alias target type owner");
    let root = executable
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let type_owners = root
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::Function(function)
                if function
                    .name()
                    .resolved()
                    .is_some_and(|name| name.as_str() == "identity") =>
            {
                Some([
                    function.parameter_groups()[0].parameters()[0].ty(),
                    function.return_type().expect("authored return type"),
                ])
            }
            _ => None,
        })
        .expect("identity function");

    for owner in type_owners {
        let resolution = report
            .type_resolution(owner)
            .expect("each authored alias use retains its root report");
        assert!(
            resolution
                .outcome()
                .product()
                .nodes()
                .iter()
                .any(|node| node.node() == alias_target),
            "each alias-use report reuses the declaration target fact"
        );
    }
    assert!(report.ty(alias_target).is_some());
}

#[test]
fn type_resolution_fact_union_rejects_disagreement() {
    let fixture = fixture("fn root(value: i32) {}\n", None);
    let owner = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .find_map(|(_, module)| module.types().next().map(|(owner, _)| owner))
        .expect("type owner");
    let mut facts = BTreeMap::new();
    let resolved = Some(TypeKind::I32);

    super::report::merge_type_resolution_fact(&mut facts, owner, &resolved)
        .expect("first fact is accepted");
    super::report::merge_type_resolution_fact(&mut facts, owner, &resolved)
        .expect("identical overlap is idempotent");
    let error = super::report::merge_type_resolution_fact(&mut facts, owner, &Some(TypeKind::Bool))
        .expect_err("disagreeing overlap is rejected");
    assert!(matches!(
        error,
        FinalSemanticAnalysisError::TypeResolutionReportMismatch { owner: rejected }
            if rejected == owner
    ));
}

#[test]
fn production_analyzer_selects_project_call_through_shared_resolver_once() {
    let fixture = fixture("fn target() {}\nfn caller() { target(); }\n", None);
    let report = analyze(&fixture).expect("project call final analysis");
    let calls = report.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (owner, call) = calls[0];
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("clean selected project call");
    };
    assert!(matches!(selected.id(), CallableCandidateId::Project(_)));
    assert_eq!(considered.len(), 1);
    assert_eq!(call.expression(), owner);
    assert_eq!(call.accounting().logical_argument_checks(), 0);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 0);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 0);
    assert_eq!(report.work().call_facts(), 1);
    assert_eq!(report.work().resolver_invocations(), 1);
    assert_eq!(report.physical_candidate_argument_evaluations().count(), 0);
}

#[test]
fn production_analyzer_routes_capacity_through_typed_associated_authority() {
    let fixture = fixture("fn caller() { String.with_capacity(8); }\n", None);
    let report = analyze(&fixture).expect("typed Capacity final analysis");
    let calls = report.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (_, call) = calls[0];
    let Some(CallCalleeClassificationFact::AssociatedType { receiver, .. }) = call.callee() else {
        panic!("Capacity call must retain its selected typed receiver")
    };
    assert!(report.type_resolution(receiver).is_some());
    assert!(report.ty(receiver).is_some());
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("clean selected Capacity call");
    };
    assert!(matches!(
        selected.id(),
        CallableCandidateId::CapacityMethod(_)
    ));
    assert_eq!(considered.len(), 1);
    assert_eq!(call.result(), Some(&TypeKind::String));
    assert_eq!(call.arguments().len(), 1);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 1);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    let physical = report
        .physical_candidate_argument_evaluations()
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 1);
    assert_eq!(physical[0].call_expression(), call.expression());
    assert_eq!(physical[0].candidate(), selected.id());
    assert_eq!(physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(physical[0].argument().get(), 0);
    assert_eq!(physical[0].slot().get(), 0);
    assert_eq!(physical[0].kind(), PhysicalArgumentEvaluationKind::Authored);
    assert_eq!(physical[0].expected(), &CandidateExpectedType::Unchecked);
}

#[test]
fn associated_capacity_checker_signature_primary_and_schema_equal() {
    const SOURCE: &str = "fn caller() { String.with_capacity(1usize, 2usize, 3usize); }\n";
    let fixture = fixture(SOURCE, None);
    let report = analyze(&fixture).expect("typed Capacity final analysis");
    let (_, call) = report.calls().next().expect("one Capacity call fact");
    let CallTargetFact::Selected { selected, .. } = call.target() else {
        panic!("Capacity checker facts retain one selected candidate")
    };
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cancellation = AtomicBool::new(false);

    for argument in ["1usize", "2usize", "3usize"] {
        let byte_offset = SOURCE.find(argument).expect("authored argument") + 2;
        let outcome = query_signature(
            SignatureQuery::production(
                &fixture.registered,
                &fixture.root_document,
                module,
                &report,
                byte_offset,
                SignatureQueryControl::new(&cancellation, None),
            )
            .expect("exact final-sema signature query"),
        )
        .expect("native Capacity signature help");
        let SignatureQueryOutcome::Help(help) = outcome else {
            panic!("cursor inside every Capacity argument must produce help")
        };
        assert_eq!(help.active_signature().get(), 0);
        let active = help.active_parameter().expect("unchecked rest parameter");
        assert_eq!(active.group().get(), 0);
        assert_eq!(active.parameter().get(), 0);
        let [signature] = help.signatures() else {
            panic!("one Capacity signature")
        };
        assert_eq!(signature.candidate(), selected.id());
        assert_eq!(signature.origin(), selected.origin());
        assert_eq!(signature.result(), selected.schema().result());
        assert_eq!(
            signature.effects(),
            selected
                .schema()
                .effects()
                .fixed_row()
                .expect("Capacity has fixed effects")
        );
        assert_eq!(signature.poison(), call.poison());
        assert_eq!(signature.groups().len(), selected.schema().groups().len());
        assert_eq!(
            selected.schema().argument_policy(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::OpenUnchecked,
                SpreadArgumentPolicy::Unchecked,
            )
        );
        let [group] = signature.groups() else {
            panic!("one Capacity group")
        };
        let [parameter] = group.parameters() else {
            panic!("one unchecked rest parameter")
        };
        assert_eq!(parameter.ty(), &CallableParameterType::Unchecked);
        assert_eq!(
            parameter.passing(),
            CallableParameterPassing::RestPositional
        );
        assert_eq!(parameter.presence(), CallableParameterPresence::Optional);
    }
}

#[test]
fn signature_query_observes_cancellation_before_surface_work() {
    const SOURCE: &str = "fn target(value: i64) {}\nfn caller() { target(1i64); }\n";
    let fixture = fixture(SOURCE, None);
    let report = analyze(&fixture).expect("accepted final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cancellation = AtomicBool::new(true);
    let byte_offset = SOURCE.find("1i64").expect("argument source") + 1;
    let request = SignatureQuery::production(
        &fixture.registered,
        &fixture.root_document,
        module,
        &report,
        byte_offset,
        SignatureQueryControl::new(&cancellation, None),
    )
    .expect("exact accepted request tuple");

    assert_eq!(
        query_signature(request),
        Err(SignatureQueryError::Cancelled)
    );
}

#[test]
fn signature_query_observes_cancellation_during_surface_traversal() {
    const SOURCE: &str = "fn target(value: i64) {}\nfn caller() { target(1i64); }\n";
    let fixture = fixture(SOURCE, None);
    let report = analyze(&fixture).expect("accepted final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cancellation = AtomicBool::new(false);
    let prior_surface_polls = Cell::new(2);
    let byte_offset = SOURCE.find("1i64").expect("argument source") + 1;
    let control = SignatureQueryControl::new(&cancellation, None)
        .with_cancellation_step_after(SignatureQueryStep::SurfaceTraversal, &prior_surface_polls);
    let request = SignatureQuery::production(
        &fixture.registered,
        &fixture.root_document,
        module,
        &report,
        byte_offset,
        control,
    )
    .expect("exact accepted request tuple");

    assert_eq!(
        query_signature(request),
        Err(SignatureQueryError::Cancelled)
    );
    assert!(cancellation.load(std::sync::atomic::Ordering::Acquire));
}

#[test]
fn signature_query_observes_deadline_at_each_bounded_control_boundary() {
    const SOURCE: &str = "fn target(value: i64) {}\nfn caller() { target(1i64); }\n";
    let fixture = fixture(SOURCE, None);
    let report = analyze(&fixture).expect("accepted final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let byte_offset = SOURCE.find("1i64").expect("argument source") + 1;

    let cancellation = AtomicBool::new(false);
    let admitted_checks = Cell::new(1);
    let request = SignatureQuery::production(
        &fixture.registered,
        &fixture.root_document,
        module,
        &report,
        byte_offset,
        SignatureQueryControl::new(&cancellation, None).with_remaining_steps(&admitted_checks),
    )
    .expect("exact accepted request tuple");
    assert_eq!(
        query_signature(request),
        Err(SignatureQueryError::DeadlineExceeded)
    );

    let cancellation = AtomicBool::new(false);
    let request = SignatureQuery::production(
        &fixture.registered,
        &fixture.root_document,
        module,
        &report,
        byte_offset,
        SignatureQueryControl::new(&cancellation, None)
            .with_deadline_step(SignatureQueryStep::SurfaceTraversal),
    )
    .expect("exact accepted request tuple");
    assert_eq!(
        query_signature(request),
        Err(SignatureQueryError::DeadlineExceeded)
    );
}

#[test]
fn character_nominal_show_checker_signature_primary_and_schema_equal() {
    const SOURCE: &str = concat!(
        "pub character @character.akane Akane as akane {}\n",
        "fn caller() { show(@character.akane, look = .normal); }\n",
    );
    let fixture = character_nominal_fixture(SOURCE);
    let report = analyze(&fixture).expect("typed Character presentation analysis");
    let (_, call) = report
        .calls()
        .next()
        .expect("one Character presentation call");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("Character presentation call retains one selected candidate")
    };
    assert_eq!(considered.len(), 1);
    assert_eq!(
        selected.id(),
        &CallableCandidateId::Presentation(PresentationCallableId::Show)
    );
    let expected_look =
        TypeKind::character_look(CharacterId::try_new("character.akane").expect("Character ID"));
    let variant = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Variant(variant) => Some(variant),
            _ => None,
        })
        .expect("accepted Character look variant fact");
    assert_eq!(
        variant.owner(),
        &CheckedVariantOwner::CharacterNominal {
            nominal: expected_look
                .character_nominal()
                .expect("Character nominal type")
                .clone(),
            cases: vec!["normal".to_owned()].into_boxed_slice(),
        }
    );
    assert_eq!(variant.ordinal(), 0);
    assert_eq!(variant.name().as_str(), "normal");
    assert_character_signature_projection(&fixture, &report, SOURCE, selected, &expected_look);
}

#[test]
fn character_dialogue_exact_target_supplies_the_manifest_look_type() {
    const SOURCE: &str = concat!(
        "pub character @character.akane Akane as akane {}\n",
        "fn configure() { let dialogue = akane(look = .normal) }\n",
    );
    let fixture = character_nominal_fixture(SOURCE);
    let report = analyze(&fixture).expect("typed exact CharacterDialogue look");
    let expected_character = CharacterId::try_new("character.akane").expect("Character identity");
    let expected_look = TypeKind::character_look(expected_character.clone());
    let factory = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::CharacterDialogueFactory(factory) => Some(factory),
            _ => None,
        })
        .expect("checked CharacterDialogue factory");
    assert_eq!(
        factory.target().character(),
        &crate::types::CharacterDialogueCharacterType::Exact(expected_character)
    );
    let [field] = factory.patch().fields() else {
        panic!("one look patch field")
    };
    assert_eq!(field.coordinate(), &CharacterDialogueFieldCoordinate::Look);
    assert!(matches!(
        field.operation(),
        CheckedPatchOperation::Set { ty, .. } if ty == &expected_look
    ));
}

#[test]
fn external_character_entity_reference_retains_registered_owner_without_hir_item() {
    const SOURCE: &str = "fn caller() { show(@character.akane); }\n";
    let fixture = external_character_fixture(SOURCE);
    let report = analyze(&fixture).expect("registered external Character analysis");
    let (checked, item) = report
        .expressions()
        .find_map(|(_, checked)| {
            let CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item)) =
                checked.resolution()
            else {
                return None;
            };
            (item.character().is_some()).then_some((checked, item))
        })
        .expect("checked external Character item");
    let expected = CharacterId::try_new("character.akane").expect("Character ID");
    assert_eq!(checked.ty(), &TypeKind::entity_ref(EntityKind::Character));
    assert_eq!(item.public_id().as_str(), expected.as_str());
    assert_eq!(item.character(), Some(expected));
    assert_eq!(item.retained_owner(), None);
    assert!(item.external_declaration().is_some());
    assert!(report.calls().next().is_some());
}

fn assert_character_signature_projection(
    fixture: &Fixture,
    report: &FinalSemanticAnalysis,
    source: &str,
    selected: &ResolvedCallable,
    expected_look: &TypeKind,
) {
    let selected_group = selected
        .schema()
        .group(CallableGroupIndex::ZERO)
        .expect("Show parameter group");
    let selected_look = selected_group
        .parameters()
        .get(1)
        .expect("Show look parameter");
    assert_eq!(
        selected_look.ty(),
        &CallableParameterType::Exact(expected_look.clone())
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cancellation = AtomicBool::new(false);
    let byte_offset = source.find(".normal").expect("authored look") + 2;
    let outcome = query_signature(
        SignatureQuery::production(
            &fixture.registered,
            &fixture.root_document,
            module,
            report,
            byte_offset,
            SignatureQueryControl::new(&cancellation, None),
        )
        .expect("exact final-sema Character signature query"),
    )
    .expect("native Character signature help");
    let SignatureQueryOutcome::Help(help) = outcome else {
        panic!("cursor inside Character look argument must produce help")
    };
    assert_eq!(help.active_signature().get(), 0);
    let active = help
        .active_parameter()
        .expect("active Character look parameter");
    assert_eq!(active.group(), CallableGroupIndex::ZERO);
    assert_eq!(active.parameter().get(), 1);
    let [signature] = help.signatures() else {
        panic!("one Character presentation signature")
    };
    assert_eq!(signature.candidate(), selected.id());
    assert_eq!(signature.origin(), selected.origin());
    assert_eq!(signature.result(), selected.schema().result());
    let [group] = signature.groups() else {
        panic!("one Character presentation group")
    };
    let signature_look = group
        .parameters()
        .get(1)
        .expect("projected Character look parameter");
    assert_eq!(signature_look.ty(), selected_look.ty());
    assert_eq!(
        signature_look.ty(),
        &CallableParameterType::Exact(expected_look.clone())
    );
}

#[test]
fn compact_numeric_spread_uses_typed_element_coordinates_without_expr_ids() {
    let fixture = fixture(
        r"
fn add(left: i64, right: i64) -> i64 { left + right }
fn caller() { add([1i64, 2i64]...); }
",
        None,
    );
    let report = analyze(&fixture).expect("compact numeric spread final analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| {
            facts
                .arguments()
                .iter()
                .any(|argument| argument.slots().len() == 2)
        })
        .expect("expanded two-slot call facts");
    let slots = call.arguments()[0].slots();

    assert_eq!(slots.len(), 2);
    for (ordinal, slot) in slots.iter().enumerate() {
        assert!(matches!(
            slot.source(),
            CheckedCallArgumentSlotSource::CompactNumericElement {
                ordinal: actual,
                ..
            } if actual == u32::try_from(ordinal).unwrap()
        ));
        assert_eq!(slot.expression(), None);
        assert_eq!(slot.inferred(), Some(&TypeKind::I64));
        assert_eq!(slot.expected(), Some(&TypeKind::I64));
    }
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(call.retained_argument_inference_facts().count(), 2);
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    for (ordinal, evaluation) in physical.iter().enumerate() {
        assert_eq!(evaluation.pass(), CandidateEvaluationPass::Probe);
        assert_eq!(evaluation.argument().get(), 0);
        assert_eq!(evaluation.slot().get(), ordinal);
        assert_eq!(
            evaluation.kind(),
            PhysicalArgumentEvaluationKind::FixedLiteralSpread
        );
        assert_eq!(
            evaluation.expected(),
            &CandidateExpectedType::Exact(TypeKind::I64)
        );
        assert!(matches!(
            evaluation.source(),
            CheckedCallArgumentSlotSource::CompactNumericElement {
                ordinal: actual,
                ..
            } if actual == u32::try_from(ordinal).unwrap()
        ));
    }
}

#[test]
fn multi_candidate_winner_replays_but_singleton_does_not() {
    let fixture = environment_overload_fixture("fn caller() { choose(1i64); }\n");
    let report = analyze(&fixture).expect("multi-candidate final analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("overloaded call facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("unique overload winner");
    };
    assert_eq!(considered.len(), 2);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 2);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(call.retained_argument_inference_facts().count(), 1);
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 3);
    assert_eq!(physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(physical[1].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(physical[2].pass(), CandidateEvaluationPass::SelectedReplay);
    assert_eq!(physical[2].candidate(), selected.id());
    assert_eq!(
        physical[0].expected(),
        &CandidateExpectedType::Exact(TypeKind::I64)
    );
    assert_eq!(
        physical[1].expected(),
        &CandidateExpectedType::Exact(TypeKind::U64)
    );
    assert_eq!(
        physical[2].expected(),
        &CandidateExpectedType::Exact(TypeKind::I64)
    );
}

#[test]
fn call_adj_a_013_three_candidate_semantic_facts_remain_complete() {
    let fixture = candidate_boundary_fixture(3);
    let report = analyze(&fixture).expect("three-candidate final analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("three-candidate call facts");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("the exact I64 overload wins");
    };

    assert_eq!(considered.len(), 3);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 3);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 4);
    assert!(
        physical[..3]
            .iter()
            .all(|evaluation| evaluation.pass() == CandidateEvaluationPass::Probe)
    );
    assert_eq!(physical[3].pass(), CandidateEvaluationPass::SelectedReplay);
    assert_eq!(physical[3].candidate(), selected.id());
}

#[test]
fn t_lim_12_007_candidate_boundary_probes_each_of_256_candidates_once() {
    let candidate_count = PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call();
    assert_eq!(
        candidate_count, 256,
        "the test is coupled to the production ceiling"
    );
    let fixture = candidate_boundary_fixture(candidate_count);
    let report = analyze(&fixture).expect("the inclusive candidate boundary is admitted");
    let calls = report.calls().collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "the boundary fixture publishes one Call fact"
    );
    let (owner, call) = calls[0];
    let CallTargetFact::Selected {
        selected,
        considered,
    } = call.target()
    else {
        panic!("the sole I64 overload wins after every candidate is probed")
    };

    assert_eq!(considered.len(), candidate_count);
    assert_eq!(call.result(), Some(&TypeKind::I64));
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(
        call.accounting().candidate_argument_probes(),
        u64::try_from(candidate_count).expect("candidate count")
    );
    assert_eq!(call.accounting().selected_replay_argument_visits(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(report.work().call_facts(), 1);
    assert_eq!(report.work().logical_argument_checks(), 1);
    assert_eq!(report.work().resolver_invocations(), 1);
    assert_eq!(
        report.work().candidate_argument_probes(),
        u64::try_from(candidate_count).expect("candidate count")
    );
    assert_eq!(report.work().selected_replay_argument_visits(), 1);
    assert_eq!(report.work().retained_argument_fact_publications(), 1);

    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == owner)
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), candidate_count + 1);
    let mut probe_counts = HashMap::with_capacity(candidate_count);
    let mut selected_replays = Vec::new();
    for evaluation in physical {
        assert_eq!(evaluation.argument().get(), 0);
        assert_eq!(evaluation.slot().get(), 0);
        assert_eq!(evaluation.kind(), PhysicalArgumentEvaluationKind::Authored);
        match evaluation.pass() {
            CandidateEvaluationPass::Probe => {
                *probe_counts
                    .entry(evaluation.candidate().clone())
                    .or_insert(0usize) += 1;
            }
            CandidateEvaluationPass::SelectedReplay => selected_replays.push(evaluation),
            CandidateEvaluationPass::RejectedRecoveryReplay => {
                panic!("the selected multi-candidate path has no rejection replay")
            }
        }
    }
    assert_eq!(probe_counts.len(), candidate_count);
    for candidate in considered.iter() {
        assert_eq!(
            probe_counts.get(candidate.id()),
            Some(&1),
            "every considered candidate is physically probed exactly once"
        );
    }
    let [selected_replay] = selected_replays.as_slice() else {
        panic!("the selected multi-candidate path replays exactly once")
    };
    assert_eq!(selected_replay.candidate(), selected.id());
}

#[test]
fn t_lim_12_008_and_t_rb_12_004_candidate_one_over_rolls_back_publication() {
    let candidate_count = PRODUCTION_CALLABLE_LIMITS
        .max_candidates_per_call()
        .checked_add(1)
        .expect("candidate one-over");
    let fixture = candidate_boundary_fixture(candidate_count);
    let (owner, outcome, accounting) = resolve_single_call_directly(&fixture);
    assert!(matches!(
        outcome,
        ResolveCallOutcome::Rejected(ResolveCallError::CandidateLimit { actual, limit })
            if actual == candidate_count
                && limit == PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call()
    ));
    assert_eq!(accounting.logical_argument_checks(), 1);
    assert_eq!(accounting.resolver_invocations(), 1);
    assert_eq!(accounting.candidate_argument_probes(), 0);
    assert_eq!(accounting.selected_replay_argument_visits(), 0);
    assert_eq!(accounting.retained_argument_fact_publications(), 0);

    let cancellation = AtomicBool::new(false);
    let (result, physical) = super::analyzer::analyze_final_project_with_physical_trace_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    );
    assert!(matches!(
        result,
        Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: rejected })
            if rejected == owner
    ));
    assert!(
        physical.is_empty(),
        "CandidateLimit occurs before any candidate probe; Err publishes no semantic report, CallTargetFacts, result, or accounting carrier"
    );
}

#[test]
fn t_res_12_006_repeated_final_authority_preserves_facts_and_projection() {
    const SOURCE: &str = "fn caller() { choose(1i64); }\n";
    let fixture = typed_overload_fixture(
        SOURCE,
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([TypeKind::U64], TypeKind::U64),
        ],
    );
    let first = analyze(&fixture).expect("first final-authority analysis");
    let second = analyze(&fixture).expect("repeated final-authority analysis");
    let first_calls = first.calls().collect::<Vec<_>>();
    let second_calls = second.calls().collect::<Vec<_>>();
    assert_eq!(
        first_calls.len(),
        1,
        "first analysis publishes one Call fact"
    );
    assert_eq!(
        second_calls.len(),
        1,
        "second analysis publishes one Call fact"
    );
    let (first_owner, first_call) = first_calls[0];
    let (second_owner, second_call) = second_calls[0];

    assert_eq!(first_owner, second_owner);
    assert_eq!(first_call, second_call);
    assert_eq!(first_call.accounting(), second_call.accounting());
    assert_eq!(first.work(), second.work());
    assert_eq!(
        first
            .physical_candidate_argument_evaluations()
            .cloned()
            .collect::<Vec<_>>(),
        second
            .physical_candidate_argument_evaluations()
            .cloned()
            .collect::<Vec<_>>()
    );

    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let byte_offset = SOURCE.find("1i64").expect("argument source") + 2;
    let cancellation = AtomicBool::new(false);
    let first_projection = query_signature(
        SignatureQuery::production(
            &fixture.registered,
            &fixture.root_document,
            module,
            &first,
            byte_offset,
            SignatureQueryControl::new(&cancellation, None),
        )
        .expect("first final-authority signature request"),
    )
    .expect("first checker-owned signature projection");
    let second_projection = query_signature(
        SignatureQuery::production(
            &fixture.registered,
            &fixture.root_document,
            module,
            &second,
            byte_offset,
            SignatureQueryControl::new(&cancellation, None),
        )
        .expect("second final-authority signature request"),
    )
    .expect("second checker-owned signature projection");
    assert_eq!(first_projection, second_projection);
    assert!(matches!(first_projection, SignatureQueryOutcome::Help(_)));
}

#[test]
fn ambiguous_overload_retains_primary_probe_without_replay() {
    let fixture = environment_overload_fixture("fn caller() { choose(1); }\n");
    let report = analyze(&fixture).expect("ambiguous call recovery facts");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("ambiguous call facts");
    let CallTargetFact::Ambiguous {
        candidates,
        considered,
    } = call.target()
    else {
        panic!("same-ranked contextual candidates remain ambiguous");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(considered.len(), 2);
    assert_eq!(call.accounting().candidate_argument_probes(), 2);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.retained_argument_inference_facts().count(), 1);
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    assert!(
        physical
            .iter()
            .all(|evaluation| evaluation.pass() == CandidateEvaluationPass::Probe)
    );
    assert_eq!(
        physical[0].expected(),
        &CandidateExpectedType::Exact(TypeKind::I64)
    );
    assert_eq!(
        physical[1].expected(),
        &CandidateExpectedType::Exact(TypeKind::U64)
    );
    assert_eq!(
        call.arguments()[0].slots()[0].inferred(),
        Some(&TypeKind::I64)
    );
}

#[test]
fn ambiguous_call_retains_complete_considered_set_beyond_the_tied_subset() {
    let fixture = typed_overload_fixture(
        "fn caller() { choose(1); }\n",
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([TypeKind::U64], TypeKind::U64),
            TestCallableOverload::strict([TypeKind::String], TypeKind::String),
        ],
    );
    let report = analyze(&fixture).expect("ambiguous call with one rejected candidate");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("ambiguous call facts");
    let CallTargetFact::Ambiguous {
        candidates,
        considered,
    } = call.target()
    else {
        panic!("the two numeric candidates remain tied");
    };

    assert_eq!(candidates.len(), 2);
    assert_eq!(considered.len(), 3);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 3);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
}

#[test]
fn rejected_overloads_retain_stable_primary_projection_without_replay() {
    let fixture = environment_overload_fixture("fn caller() { choose(\"no\"); }\n");
    let report = analyze(&fixture).expect("rejected call recovery facts");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("rejected call facts");
    let CallTargetFact::Rejected { candidates } = call.target() else {
        panic!("no viable overload publishes rejected target facts");
    };
    assert_eq!(candidates.len(), 2);
    assert_eq!(call.accounting().candidate_argument_probes(), 2);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.retained_argument_inference_facts().count(), 1);
    assert_eq!(
        call.arguments()[0].slots()[0].inferred(),
        Some(&TypeKind::String)
    );
    assert_eq!(
        call.arguments()[0].slots()[0].poison(),
        super::CallPoison::Rejected
    );
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    assert!(
        physical
            .iter()
            .all(|evaluation| evaluation.pass() == CandidateEvaluationPass::Probe)
    );
}

#[test]
fn rejected_singleton_replays_for_precise_recovery_projection() {
    let fixture = fixture(
        r#"
fn choose(value: i64) -> i64 { value }
fn caller() { choose("no"); }
"#,
        None,
    );
    let report = analyze(&fixture).expect("singleton rejected recovery facts");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("singleton rejected call facts");
    assert!(matches!(
        call.target(),
        CallTargetFact::Rejected { candidates } if candidates.len() == 1
    ));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    assert_eq!(physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(
        physical[1].pass(),
        CandidateEvaluationPass::RejectedRecoveryReplay
    );
    assert_eq!(physical[0].candidate(), physical[1].candidate());
    assert_eq!(physical[0].source(), physical[1].source());
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.retained_argument_inference_facts().count(), 1);
}

#[test]
fn work_failure_preserves_only_the_completed_physical_prefix() {
    let fixture = fixture(
        r"
fn combine(left: i64, right: i64) -> i64 { left + right }
fn caller() { combine(1i64, 2i64); }
",
        None,
    );
    let boundary = (1..=64)
        .find(|limit| {
            let (result, physical) = analyze_with_query_work(&fixture, *limit);
            result.is_err() && physical.len() == 1
        })
        .expect("one exact work limit admits the first slot and rejects the second");

    let (before, no_physical) = analyze_with_query_work(&fixture, boundary - 1);
    assert!(matches!(
        before,
        Err(FinalSemanticAnalysisError::CallResolutionFailed { .. })
    ));
    assert!(no_physical.is_empty());

    let (failed, physical_prefix) = analyze_with_query_work(&fixture, boundary);
    assert!(matches!(
        failed,
        Err(FinalSemanticAnalysisError::CallResolutionFailed { .. })
    ));
    assert_eq!(physical_prefix.len(), 1);
    assert_eq!(physical_prefix[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(physical_prefix[0].argument().get(), 0);
    assert_eq!(physical_prefix[0].slot().get(), 0);

    let (accepted, complete_physical) = analyze_with_query_work(&fixture, boundary + 1);
    let report = accepted.expect("one more work unit admits the complete singleton call");
    assert_eq!(complete_physical.len(), 2);
    assert!(
        complete_physical
            .iter()
            .all(|evaluation| evaluation.pass() == CandidateEvaluationPass::Probe)
    );
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 2)
        .expect("accepted two-argument call facts");
    assert_eq!(call.retained_argument_inference_facts().count(), 2);
}

#[test]
fn closure_arguments_are_rechecked_under_each_candidate_function_context() {
    let i64_callback = TypeKind::function([TypeKind::I64], TypeKind::I64);
    let u64_callback = TypeKind::function([TypeKind::U64], TypeKind::U64);
    let fixture = typed_overload_fixture(
        "fn caller() { choose(|value| value); }\n",
        "choose",
        vec![
            TestCallableOverload::strict([i64_callback.clone()], TypeKind::I64),
            TestCallableOverload::strict([u64_callback.clone()], TypeKind::U64),
        ],
    );
    let report = analyze(&fixture).expect("contextual closure overload analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("closure call facts");
    assert!(matches!(
        call.target(),
        CallTargetFact::Ambiguous { candidates, .. } if candidates.len() == 2
    ));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    assert_eq!(
        physical[0].expected(),
        &CandidateExpectedType::Exact(i64_callback.clone())
    );
    assert_eq!(
        physical[1].expected(),
        &CandidateExpectedType::Exact(u64_callback)
    );
    assert_eq!(
        call.arguments()[0].slots()[0].inferred(),
        Some(&i64_callback),
        "the deterministic primary probe is the only retained closure projection"
    );
}

#[test]
fn enum_shorthand_and_partial_placeholder_are_candidate_contextual() {
    let signed_option = TypeKind::Option(Box::new(TypeKind::I64));
    let unsigned_option = TypeKind::Option(Box::new(TypeKind::U64));
    for (source, expected) in [
        (
            "fn caller() { choose(.None); }\n",
            vec![signed_option, unsigned_option],
        ),
        (
            "fn caller() { choose(_); }\n",
            vec![
                TypeKind::function([TypeKind::I64], TypeKind::I64),
                TypeKind::function([TypeKind::U64], TypeKind::U64),
            ],
        ),
    ] {
        let fixture = typed_overload_fixture(
            source,
            "choose",
            vec![
                TestCallableOverload::strict([expected[0].clone()], TypeKind::Unit),
                TestCallableOverload::strict([expected[1].clone()], TypeKind::Unit),
            ],
        );
        let report = analyze(&fixture).expect("candidate-contextual argument analysis");
        let call = report
            .calls()
            .map(|(_, facts)| facts)
            .find(|facts| facts.arguments().len() == 1)
            .expect("contextual call facts");
        assert!(matches!(
            call.target(),
            CallTargetFact::Ambiguous { candidates, .. } if candidates.len() == 2
        ));
        let physical = report
            .physical_candidate_argument_evaluations()
            .filter(|evaluation| evaluation.call_expression() == call.expression())
            .collect::<Vec<_>>();
        assert_eq!(physical.len(), 2);
        for (evaluation, expected) in physical.into_iter().zip(expected) {
            assert_eq!(evaluation.pass(), CandidateEvaluationPass::Probe);
            assert_eq!(
                evaluation.expected(),
                &CandidateExpectedType::Exact(expected)
            );
        }
    }
}

#[test]
fn closed_environment_enum_forms_share_one_ordered_owner_and_ordinal() {
    let fixture = typed_overload_fixture(
        r"
fn qualified() -> DataFormat { DataFormat::Json }
fn shorthand() { choose(.Json); }
fn patterned(value: DataFormat) -> bool {
    if let DataFormat.Json = value { true } else { false }
}
",
        "choose",
        vec![TestCallableOverload::strict(
            [TypeKind::DataFormat],
            TypeKind::Unit,
        )],
    );
    let report = analyze(&fixture).expect("closed environment enum analysis");
    let expression_variants = report
        .expressions()
        .filter_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Variant(variant) => Some(variant),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(expression_variants.len(), 2);
    let pattern_variant = report
        .patterns()
        .find_map(|(_, pattern)| match pattern.resolution() {
            CheckedPatternResolution::Variant(variant) => Some(variant),
            _ => None,
        })
        .expect("qualified DataFormat pattern fact");

    for variant in expression_variants
        .iter()
        .copied()
        .chain(std::iter::once(pattern_variant))
    {
        assert_eq!(variant.ordinal(), 0);
        assert_eq!(variant.name().as_str(), "Json");
        assert_eq!(variant.owner(), expression_variants[0].owner());
    }
    let CheckedVariantOwner::BuiltinClosed { nominal, cases, .. } = expression_variants[0].owner()
    else {
        panic!("DataFormat must use the generic closed environment owner")
    };
    assert_eq!(nominal.as_str(), "DataFormat");
    assert_eq!(
        cases
            .iter()
            .map(CheckedBuiltinVariantCase::name)
            .collect::<Vec<_>>(),
        arcweft_data::DataFormat::ALL
            .map(arcweft_data::DataFormat::variant_name)
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert!(cases.iter().all(|case| case.payload().is_none()));
}

#[test]
fn unconstrained_numeric_and_partial_placeholder_types_are_final_facts() {
    let fixture = fixture(
        r"
fn root() {
    let scalar = 42;
    let values = [1, 2];
    let matcher = _ > 80i64;
    let zero = || 1;
}
",
        None,
    );
    let (_, lowered) = fixture
        .project
        .view()
        .modules()
        .next()
        .expect("lowered root module");
    assert_eq!(
        lowered.status(),
        arcweft_lang_hir::module::HirModuleStatus::Clean,
        "numeric/placeholder fixture must lower cleanly: {:?}",
        lowered.diagnostics()
    );
    let report = analyze(&fixture).expect("final numeric and placeholder analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1;
    let initializers = module
        .statements()
        .filter_map(|(_, statement)| match statement.kind() {
            HirStmtKind::Let { initializer, .. } => Some(*initializer),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(initializers.len(), 4);

    let scalar = report
        .expression(initializers[0])
        .expect("scalar initializer fact");
    assert_eq!(scalar.ty(), &TypeKind::I32);
    assert_eq!(
        scalar.type_selection(),
        CheckedTypeSelection::DefaultNumericFallback
    );

    let values = report
        .expression(initializers[1])
        .expect("numeric sequence initializer fact");
    assert_eq!(values.ty(), &TypeKind::Vec(Box::new(TypeKind::I32)));
    assert_eq!(
        values.type_selection(),
        CheckedTypeSelection::DefaultNumericFallback
    );

    assert_eq!(
        report
            .expression(initializers[2])
            .expect("partial placeholder initializer fact")
            .ty(),
        &TypeKind::function([TypeKind::I64], TypeKind::Bool)
    );
    assert_eq!(
        report
            .expression(initializers[3])
            .expect("zero-argument closure initializer fact")
            .ty(),
        &TypeKind::function([], TypeKind::I32)
    );
}

#[test]
fn annotated_flow_let_pattern_has_a_final_type_fact() {
    let fixture = fixture(
        r"
flow @flow.numeric_inlays numeric_inlays {
    let count = 42
    let ratio = 1_2.5_0
    let negative = -1
    let total = 1 + 2
    let values = [1, 2]
    let explicit: u64 = 42
}
",
        None,
    );
    let report = analyze(&fixture).expect("final numeric Flow analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1;
    for (owner, _) in module.patterns() {
        assert!(
            report.pattern(owner).is_some(),
            "final pattern fact missing for {owner:?}"
        );
    }
}

#[test]
fn annotated_function_local_tail_uses_the_pattern_owned_type() {
    let fixture = fixture(
        r"
fn smoke() -> Result<Unit, AgentError>
effects {}
{
    let result: Result<Unit, AgentError> = Ok(())
    result
}
",
        None,
    );
    let report = analyze(&fixture).expect("typed local function-tail analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1;
    let tail = module
        .items()
        .find_map(|(_, item)| match item.kind() {
            HirItemKind::Function(function) => match function.body() {
                HirFunctionBody::Block { tail, .. } => Some(*tail),
                HirFunctionBody::Error(_) => None,
            },
            _ => None,
        })
        .expect("function tail");
    assert_eq!(
        report.expression(tail).expect("function tail fact").ty(),
        &TypeKind::Result {
            ok: Box::new(TypeKind::Unit),
            error: Box::new(TypeKind::Named("AgentError".to_owned())),
        }
    );
}

#[test]
fn ordinary_function_effect_contract_preserves_omitted_empty_and_nonempty_states() {
    let fixture = fixture(
        concat!(
            "fn inferred() {}\n",
            "fn empty() effects {} {}\n",
            "fn bounded() effects { fs.read, debug.record } {}\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("three-state function effect analysis");

    let inferred = checked_function_facts(&report, &fixture, "inferred");
    assert_eq!(
        inferred.effect_contract_origin(),
        Some(EffectContractOrigin::BodyInference)
    );
    assert!(inferred.exposed_row().concrete().is_empty());

    let empty = checked_function_facts(&report, &fixture, "empty");
    assert_eq!(
        empty.effect_contract_origin(),
        Some(EffectContractOrigin::Authored)
    );
    assert!(empty.exposed_row().concrete().is_empty());

    let bounded = checked_function_facts(&report, &fixture, "bounded");
    assert_eq!(
        bounded.effect_contract_origin(),
        Some(EffectContractOrigin::Authored)
    );
    assert_eq!(
        bounded.exposed_row().concrete().to_labels(),
        ["debug.record", "fs.read"]
    );

    let bounded_owner = function_owner(&fixture, "bounded");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .find_map(|(_, module)| (module.module_id() == bounded_owner.module()).then_some(module))
        .expect("bounded function module");
    let item = module
        .resolve_item(bounded_owner)
        .expect("bounded function item");
    let HirItemKind::Function(function) = item.kind() else {
        panic!("bounded owner must remain an ordinary function")
    };
    let [clause] = function.effect_clauses() else {
        panic!("bounded function must own one effect clause")
    };
    for (&owner, expected) in clause.operands().iter().zip(["fs.read", "debug.record"]) {
        assert!(matches!(
            report
                .expression(owner)
                .expect("effect operand fact")
                .resolution(),
            CheckedExpressionResolution::Effect(effect) if effect.as_str() == expected
        ));
    }
}

#[test]
fn source_independent_root_inventory_reaches_for_synthetic_chain() {
    let fixture = fixture(
        r"
flow root {
    let values: Vec<i64> = [1i64, 2i64]
    for value in values {
        let copy = value
    }
}
",
        None,
    );
    let report = analyze(&fixture).expect("final source-independent synthetic analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1;
    let synthetic = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::ForSynthetic(_)).then_some(owner)
        })
        .collect::<Vec<_>>();
    assert_eq!(synthetic.len(), 2);
    let types = synthetic
        .iter()
        .map(|owner| {
            report
                .expression(*owner)
                .expect("synthetic fact")
                .ty()
                .clone()
        })
        .collect::<Vec<_>>();
    assert!(types.iter().any(|ty| ty == &TypeKind::I64), "{types:?}");
    assert!(
        types.iter().any(|ty| matches!(
            ty,
            TypeKind::IteratorState { item, .. } if item.as_ref() == &TypeKind::I64
        )),
        "{types:?}"
    );
}

#[test]
fn dialogue_content_application_resolves_exact_character_item() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

flow @flow.root root {
    alice[Hello[p]]
}
",
        None,
    );
    let report = analyze(&fixture).unwrap_or_else(|error| {
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .modules()
            .next()
            .expect("root module")
            .1;
        let expressions = module
            .expressions()
            .map(|(owner, expression)| (owner, expression.kind().clone()))
            .collect::<Vec<_>>();
        panic!("final dialogue application analysis: {error:?}; expressions={expressions:#?}")
    });
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .next()
        .expect("root module")
        .1;
    let (application, target) = report
        .expressions()
        .find_map(|(owner, checked)| {
            if !matches!(
                checked.resolution(),
                CheckedExpressionResolution::DialogueApplication { .. }
            ) {
                return None;
            }
            let expression = module.resolve_expr(owner).ok()?;
            let HirExprKind::DialogueContentApplication(application) = expression.kind() else {
                return None;
            };
            Some((owner, application.target()))
        })
        .expect("dialogue application expression");
    let checked = report
        .expression(application)
        .expect("dialogue application fact");
    assert_eq!(
        checked.ty(),
        &TypeKind::DialogueLine(Box::new(TypeKind::Unit))
    );
    let CheckedExpressionResolution::DialogueApplication {
        target: dialogue_target,
        rich_text,
        ..
    } = checked.resolution()
    else {
        panic!("dialogue application must retain exact Character owner")
    };
    assert!(rich_text.is_valid());
    let CheckedCharacterDialogueTarget::Character {
        item: Some(character),
        ..
    } = dialogue_target
    else {
        panic!("direct Character application must retain its checked item")
    };
    assert!(matches!(
        report
            .expression(target)
            .expect("dialogue target fact")
            .resolution(),
        CheckedExpressionResolution::Value(CheckedValueResolution::ProjectItem(item))
            if item.retained_owner() == character.retained_owner()
    ));
}

#[test]
fn dialogue_line_reference_uses_accepted_project_inventory() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

fn opening() {
    alice[前[strong]強調[/strong]後];
}

fn reference() {
    let selected: Ref<DialogueLine> = @say.fn.final-analysis-tests.function.opening.001
}
",
        None,
    );
    let analysis = analyze(&fixture).unwrap_or_else(|error| {
        panic!(
            "typed dialogue-line reference analysis: {error:?}; accepted={:?}",
            fixture
                .project
                .dialogue_lines()
                .records()
                .iter()
                .map(|line| line.id().as_str())
                .collect::<Vec<_>>()
        )
    });
    let (expression, target) = analysis
        .expressions()
        .find_map(|(owner, checked)| {
            let CheckedExpressionResolution::DialogueLineReference(target) = checked.resolution()
            else {
                return None;
            };
            Some((owner, target))
        })
        .expect("accepted dialogue-line reference fact");
    assert_eq!(
        target.as_str(),
        "say.fn.final-analysis-tests.function.opening.001"
    );

    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("dialogue-line-reference"),
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        &analysis,
        &CheckedEntryCatalog::default(),
    )
    .expect("dialogue-line project index");
    let [reference] = index.dialogue_line_references() else {
        panic!("one accepted dialogue-line reference")
    };
    assert_eq!(reference.target(), target);
    assert_eq!(reference.expression(), expression);
    assert_eq!(reference.module().package(), fixture.project.package());
    let reference_start = fixture
        .root_document
        .text()
        .rfind("@say.fn.final-analysis-tests.function.opening.001")
        .expect("reference source spelling");
    assert_eq!(
        reference.source().range(),
        SourceRange::new(
            reference_start,
            reference_start + "@say.fn.final-analysis-tests.function.opening.001".len()
        )
    );
}

#[test]
fn dialogue_line_reference_rejects_target_outside_accepted_inventory() {
    let fixture = fixture(
        r"
fn reference() {
    let selected: Ref<DialogueLine> = @say.missing
}
",
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::ValueResolutionFailed { .. })
    ));
}

#[test]
fn dialogue_configuration_coordinates_are_typed_semantic_metadata() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

fn opening() {
    alice(
        id = @say.story.greeting,
        text_key = @text.story.greeting,
    )[前[strong]強調[/strong]後];
}
",
        None,
    );
    let analysis = analyze(&fixture).expect("typed dialogue configuration analysis");
    let resolutions = analysis
        .expressions()
        .map(|(_, checked)| checked.resolution())
        .collect::<Vec<_>>();
    assert!(resolutions.iter().any(|resolution| matches!(
        resolution,
        CheckedExpressionResolution::CharacterDialogueFactory(_)
    )));
    assert!(resolutions.iter().any(|resolution| matches!(
        resolution,
        CheckedExpressionResolution::DialogueLineCoordinate(id)
            if id.as_str() == "say.story.greeting"
    )));
    assert!(resolutions.iter().any(|resolution| matches!(
        resolution,
        CheckedExpressionResolution::DialogueTextKeyCoordinate(key)
            if key.as_str() == "text.story.greeting"
    )));
}

#[test]
fn character_dialogue_patch_retains_typed_fields_in_source_order() {
    let fixture = fixture(
        r#"
pub character @character.alice Alice as alice {}

flow @flow.root root {
    alice(source_locale = "ja-JP", inline_error = None)[Hello[p]]
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("typed CharacterDialogue patch analysis");
    let (factory_owner, factory) = report
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
                Some((owner, factory))
            }
            _ => None,
        })
        .expect("Character factory fact");
    let call = report
        .call(factory_owner)
        .expect("Character factory is retained as one shared-resolver call fact");
    let CallTargetFact::Selected { selected, .. } = call.target() else {
        panic!("Character factory must select one callable candidate")
    };
    assert_eq!(
        selected.id(),
        &CallableCandidateId::Dialogue(DialogueCallableId::CharacterFactory)
    );
    assert_eq!(
        selected.schema().validator(),
        &CallableValidator::Dialogue(DialogueCallableId::CharacterFactory)
    );
    assert_eq!(call.accounting().resolver_invocations(), 1);
    let [locale, inline_failure] = factory.patch().fields() else {
        panic!("factory patch must retain two fields")
    };
    assert_eq!(
        locale.coordinate(),
        &CharacterDialogueFieldCoordinate::SourceLocale
    );
    assert!(matches!(
        locale.operation(),
        CheckedPatchOperation::Set {
            ty: TypeKind::String,
            ..
        }
    ));
    assert_eq!(
        inline_failure.coordinate(),
        &CharacterDialogueFieldCoordinate::InlineFailure
    );
    assert_eq!(inline_failure.operation(), &CheckedPatchOperation::Clear);
    let (application_owner, application_patch) = report
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::DialogueApplication {
                application_patch: Some(patch),
                ..
            } => Some((owner, patch)),
            _ => None,
        })
        .expect("immediate application patch");
    assert_eq!(application_patch, factory.patch());
    let application_call = report
        .call(application_owner)
        .expect("content application retains the shared resolver selection");
    let CallTargetFact::Selected {
        selected,
        considered,
    } = application_call.target()
    else {
        panic!("content application must select one callable candidate")
    };
    assert_eq!(considered.len(), 1);
    assert_eq!(
        selected.id(),
        &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication)
    );
    assert_eq!(
        selected.schema().validator(),
        &CallableValidator::Dialogue(DialogueCallableId::ContentApplication)
    );
    assert!(application_call.arguments().is_empty());
    assert_eq!(application_call.accounting().resolver_invocations(), 1);
}

#[test]
fn character_dialogue_application_only_coordinates_are_rejected_in_reusable_calls() {
    let fixture = fixture(
        r#"
pub character @character.alice Alice as alice {}

fn configure() {
    let configured = alice(id = "not-an-application-coordinate")
}
"#,
        None,
    );
    let error = analyze(&fixture).expect_err("id is an application-only coordinate");
    let FinalSemanticAnalysisError::CharacterDialogueApplicationOnlyField { field, field_span } =
        &error
    else {
        panic!("unexpected application-only result: {error:?}")
    };
    assert_eq!(field, "id");
    assert_eq!(error.diagnostic_code(), "AW-CD-007");
    assert_eq!(
        &fixture.root_document.text()[field_span.range().start()..field_span.range().end()],
        "id"
    );
    assert_eq!(
        error
            .source_diagnostic()
            .expect("typed application-only diagnostic")
            .labels()
            .len(),
        1
    );
}

#[test]
fn character_dialogue_inline_failure_aliases_share_one_semantic_coordinate() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

flow @flow.root root {
    alice(inline_error = None, inline_fallback = None)[Hello[p]]
}
",
        None,
    );
    let result = analyze(&fixture);
    let error = result.expect_err("inline-failure aliases share one coordinate");
    let FinalSemanticAnalysisError::DuplicateCharacterDialogueField {
        first_span,
        duplicate_span,
        ..
    } = &error
    else {
        panic!("unexpected alias-conflict result: {error:?}")
    };
    assert_eq!(error.diagnostic_code(), "AW-CD-005");
    assert_eq!(
        &fixture.root_document.text()[first_span.range().start()..first_span.range().end()],
        "inline_error"
    );
    assert_eq!(
        &fixture.root_document.text()[duplicate_span.range().start()..duplicate_span.range().end()],
        "inline_fallback"
    );
    assert_eq!(
        error
            .source_diagnostic()
            .expect("typed duplicate-coordinate diagnostic")
            .labels()
            .len(),
        2
    );
}

#[test]
fn character_dialogue_unknown_custom_field_has_typed_diagnostic() {
    let fixture = fixture(
        r#"
pub character @character.alice Alice as alice {}

fn configure() {
    let configured = alice(mood = "quiet")
}
"#,
        None,
    );
    let error = analyze(&fixture).expect_err("unknown custom field must fail closed");
    let FinalSemanticAnalysisError::UnknownCharacterDialogueField {
        name,
        field_span,
        scope,
    } = &error
    else {
        panic!("unexpected unknown-field result: {error:?}")
    };
    assert_eq!(name, "mood");
    assert_eq!(scope, &CanonicalModulePath::crate_root());
    assert_eq!(error.diagnostic_code(), "AW-CD-014");
    assert_eq!(
        &fixture.root_document.text()[field_span.range().start()..field_span.range().end()],
        "mood"
    );
    assert_eq!(
        error
            .source_diagnostic()
            .expect("typed unknown-field diagnostic")
            .labels()
            .len(),
        1
    );
}

#[test]
fn character_dialogue_custom_field_resolves_through_accepted_world_registry() {
    let (fixture, field) = custom_dialogue_field_fixture(
        r#"
pub character @character.alice Alice as alice {}

flow @flow.root root {
    alice(mood = "quiet")[Hello[p]]
}
"#,
        true,
    );
    let report = analyze(&fixture).expect("typed custom CharacterDialogue field");
    let (factory_owner, custom) = report
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::CharacterDialogueFactory(factory) => {
                factory.patch().fields().first().map(|field| (owner, field))
            }
            _ => None,
        })
        .expect("custom patch field");
    assert_eq!(
        custom.coordinate(),
        &CharacterDialogueFieldCoordinate::Custom(field)
    );
    assert!(matches!(
        custom.operation(),
        CheckedPatchOperation::Set {
            ty: TypeKind::String,
            ..
        }
    ));
    let CallTargetFact::Selected { selected, .. } = report
        .call(factory_owner)
        .expect("custom field call fact")
        .target()
    else {
        panic!("custom field call must select its Dialogue candidate")
    };
    let mood = selected.schema().groups()[0]
        .parameters()
        .iter()
        .find(|parameter| parameter.name().is_some_and(|name| name.as_str() == "mood"))
        .expect("accepted custom binding is part of the shared signature schema");
    assert_eq!(mood.ty(), &CallableParameterType::Exact(TypeKind::String));
}

fn custom_dialogue_field_fixture(
    source: &str,
    clearable: bool,
) -> (Fixture, CharacterDialogueCustomFieldId) {
    let document = source_document(
        "arcweft-test://sema/final/dialogue-custom-field",
        "dialogue-custom-field.environment",
        "mood",
    );
    let declaration = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("custom field declaration span");
    let owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("dialogue-custom-field").expect("adapter package ID"),
    );
    let item = EnvironmentPublicationItemId::AdapterSymbol {
        owner: owner.clone(),
        path: ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new("mood").expect("custom binding")],
        )
        .expect("custom field publication path"),
    };
    let field = CharacterDialogueCustomFieldId::try_new("character_dialogue_field.mood")
        .expect("custom field ID");
    let input = SourceBackedEnvironmentRegistrationInput::new(
        owner,
        document.identity().clone(),
        EnvironmentManifestDigest::from_bytes([91; 32]),
        [],
        [],
        [],
        [],
    )
    .with_character_dialogue_fields([CharacterDialogueCustomFieldInput::new(
        item,
        field.clone(),
        [CharacterDialogueCustomFieldBinding::global("mood")],
        EnvironmentTypeProjectionNode::new(
            declaration.clone(),
            EnvironmentTypeProjectionKind::String,
        ),
        None,
        TypeLayoutHash::from_bytes([9; 32]),
        clearable,
        BTreeSet::new(),
        declaration,
    )]);
    let fixture = fixture_with_environment_inputs(source, None, vec![(document, input)]);
    (fixture, field)
}

#[test]
fn character_dialogue_custom_field_type_mismatch_has_typed_diagnostic() {
    let (fixture, field) = custom_dialogue_field_fixture(
        r"
pub character @character.alice Alice as alice {}

fn configure() {
    let configured = alice(mood = 7)
}
",
        true,
    );
    let error = analyze(&fixture).expect_err("custom field type mismatch must reject");
    let FinalSemanticAnalysisError::CharacterDialogueCustomFieldTypeMismatch {
        field: actual_field,
        declared,
        actual,
        value_span,
        declaration_span,
    } = &error
    else {
        panic!("unexpected custom-field mismatch result: {error:?}")
    };
    assert_eq!(actual_field, &field);
    assert_eq!(declared.as_ref(), &TypeKind::String);
    assert_eq!(actual.as_ref(), &TypeKind::I32);
    assert_eq!(error.diagnostic_code(), "AW-CD-015");
    assert_eq!(
        &fixture.root_document.text()[value_span.range().start()..value_span.range().end()],
        "7"
    );
    assert_eq!(
        declaration_span.source().id().as_str(),
        "arcweft-test://sema/final/dialogue-custom-field"
    );
    assert_eq!(
        error
            .source_diagnostic()
            .expect("typed custom-field mismatch diagnostic")
            .labels()
            .len(),
        2
    );
}

#[test]
fn character_dialogue_non_clearable_custom_field_has_typed_diagnostic() {
    let (fixture, field) = custom_dialogue_field_fixture(
        r"
pub character @character.alice Alice as alice {}

flow @flow.root root {
    alice(mood = None)[Hello[p]]
}
",
        false,
    );
    let error = analyze(&fixture).expect_err("non-clearable custom field must reject Clear");
    let FinalSemanticAnalysisError::CharacterDialogueFieldNotClearable {
        field: actual,
        field_span,
        declaration_span,
    } = &error
    else {
        panic!("unexpected non-clearable field result: {error:?}")
    };
    assert_eq!(actual, &field);
    assert_eq!(error.diagnostic_code(), "AW-CD-016");
    assert_eq!(
        &fixture.root_document.text()[field_span.range().start()..field_span.range().end()],
        "mood"
    );
    assert_eq!(
        declaration_span.source().id().as_str(),
        "arcweft-test://sema/final/dialogue-custom-field"
    );
    let diagnostic = error.source_diagnostic().expect("typed source diagnostic");
    assert_eq!(
        diagnostic
            .code()
            .map(arcweft_source::DiagnosticCode::as_str),
        Some("AW-CD-016")
    );
    assert_eq!(diagnostic.labels().len(), 2);
}

#[test]
fn coordinate_free_dialogue_call_is_typed_configuration_metadata() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

fn opening() {
    alice()[前[strong]強調[/strong]後];
}
",
        None,
    );
    let analysis = analyze(&fixture).expect("coordinate-free dialogue configuration analysis");
    assert!(analysis.expressions().any(|(_, checked)| matches!(
        checked.resolution(),
        CheckedExpressionResolution::CharacterDialogueFactory(_)
    )));
}

#[test]
fn character_dialogue_flows_through_branch_reconfigure_collection_and_capture() {
    let fixture = fixture(
        r#"
pub character @character.alice Alice as alice {}
pub character @character.bob Bob as bob {}

fn configure(condition: bool) {
    let dialogue = if condition { alice() } else { bob() }
    let patched = dialogue(source_locale = "ja-JP")
    let values = [dialogue, patched]
    let captured = || { dialogue }
}
"#,
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let analysis = analyze(&fixture).expect("CharacterDialogue value-flow matrix");
    let local_type = |name: &str| {
        analysis.locals().find_map(|(owner, binding)| {
            module
                .resolve_local(owner)
                .ok()
                .is_some_and(|local| local.name().as_str() == name)
                .then(|| binding.ty().clone())
        })
    };
    let any_dialogue = TypeKind::CharacterDialogue(crate::types::CharacterDialogueType::new(
        crate::types::CharacterDialogueCharacterType::Any,
    ));
    assert_eq!(local_type("dialogue"), Some(any_dialogue.clone()));
    assert_eq!(local_type("patched"), Some(any_dialogue.clone()));
    assert_eq!(
        local_type("values"),
        Some(TypeKind::Vec(Box::new(any_dialogue.clone())))
    );
    assert_eq!(
        local_type("captured"),
        Some(TypeKind::function(Vec::new(), any_dialogue.clone()))
    );
    assert!(
        analysis
            .captures()
            .any(|(_, capture)| capture.ty() == &any_dialogue)
    );
    assert!(analysis.expressions().any(|(_, expression)| matches!(
        expression.resolution(),
        CheckedExpressionResolution::CharacterDialogueReconfigure(_)
    )));
}

#[test]
fn character_dialogue_is_an_authored_parameter_return_and_alias_type() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

type Dialogue = CharacterDialogue

fn passthrough(value: Dialogue) -> CharacterDialogue {
    value
}

fn configure() {
    let result = passthrough(alice())
}
",
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let analysis = analyze(&fixture).expect("authored CharacterDialogue boundary");
    let any_dialogue = TypeKind::CharacterDialogue(crate::types::CharacterDialogueType::any());
    for name in ["value", "result"] {
        let actual = analysis.locals().find_map(|(owner, binding)| {
            module
                .resolve_local(owner)
                .ok()
                .is_some_and(|local| local.name().as_str() == name)
                .then(|| binding.ty())
        });
        assert_eq!(actual, Some(&any_dialogue), "local `{name}`");
    }
    assert!(analysis.types().any(|(_, ty)| ty == &any_dialogue));
}

#[test]
fn generic_identity_preserves_exact_character_dialogue_type() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

fn identity<T>(value: T) -> T {
    value
}

fn configure() {
    let result = identity(alice())
}
",
        None,
    );
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let analysis = analyze(&fixture).expect("generic CharacterDialogue identity");
    let alice = CharacterId::try_new("character.alice").expect("Character ID");
    let exact = TypeKind::CharacterDialogue(crate::types::CharacterDialogueType::exact(alice));
    let actual = analysis.locals().find_map(|(owner, binding)| {
        module
            .resolve_local(owner)
            .ok()
            .is_some_and(|local| local.name().as_str() == "result")
            .then(|| binding.ty())
    });
    assert_eq!(actual, Some(&exact));
}

#[test]
fn dialogue_content_signature_help_uses_the_shared_application_schema() {
    const SOURCE: &str = r"
pub character @character.alice Alice as alice {}

fn opening() {
    alice()[前[strong]強調[/strong]後];
}
";
    let fixture = fixture(SOURCE, None);
    let analysis = analyze(&fixture).expect("typed dialogue application analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let cancellation = AtomicBool::new(false);
    let byte_offset = SOURCE.find("強調").expect("dialogue content") + "強".len();
    let outcome = query_signature(
        SignatureQuery::production(
            &fixture.registered,
            &fixture.root_document,
            module,
            &analysis,
            byte_offset,
            SignatureQueryControl::new(&cancellation, None),
        )
        .expect("generation-bound signature query"),
    )
    .expect("dialogue content signature help");
    let SignatureQueryOutcome::Help(help) = outcome else {
        panic!("dialogue content must be a native signature surface")
    };
    assert_eq!(help.surface(), SemanticSignatureSurface::DialogueContent);
    let active = help.active_parameter().expect("content parameter");
    assert_eq!(active.group(), CallableGroupIndex::ZERO);
    assert_eq!(active.parameter().get(), 1);
    let [signature] = help.signatures() else {
        panic!("one content-application signature")
    };
    assert_eq!(
        signature.candidate(),
        &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication)
    );
    let [group] = signature.groups() else {
        panic!("one content-application parameter group")
    };
    assert_eq!(group.parameters().len(), 3);
    assert_eq!(
        group.parameters()[1].ty(),
        &CallableParameterType::Exact(TypeKind::Named("DialogueContent".to_owned()))
    );
    assert_eq!(
        signature.result(),
        &TypeKind::DialogueLine(Box::new(TypeKind::Unit))
    );
}

#[test]
fn dialogue_line_operation_cannot_escape_into_a_local_binding() {
    let fixture = fixture(
        r"
pub character @character.alice Alice as alice {}

fn opening() {
    let escaped = alice[Hello[p]]
}
",
        None,
    );
    let error = analyze(&fixture).expect_err("DialogueLine local storage must be rejected");
    let FinalSemanticAnalysisError::DialogueLineEscape { escape_span } = &error else {
        panic!("unexpected DialogueLine escape result: {error:?}")
    };
    assert_eq!(error.diagnostic_code(), "AW-CD-017");
    assert_eq!(
        &fixture.root_document.text()[escape_span.range().start()..escape_span.range().end()],
        "escaped"
    );
}

#[test]
fn entry_entity_reference_reads_exact_final_hir_item_owner() {
    let fixture = fixture(
        r"
flow @flow.references references {
    let selected = @entry.agent.main
}

entry agent @entry.agent.main {}
",
        None,
    );
    let report = analyze(&fixture).expect("typed Entry reference analysis");
    let (checked, entry) = report
        .expressions()
        .find_map(|(_, checked)| {
            let CheckedExpressionResolution::Value(CheckedValueResolution::Entry(entry)) =
                checked.resolution()
            else {
                return None;
            };
            Some((checked, entry))
        })
        .expect("exact Entry reference fact");
    assert_eq!(checked.ty(), &TypeKind::entity_ref(EntityKind::Entry));
    assert_eq!(entry.public_id().as_str(), "entry.agent.main");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .find_map(|(_, module)| (module.resolve_item(entry.owner()).is_ok()).then_some(module))
        .expect("Entry owner module");
    assert!(matches!(
        module
            .resolve_item(entry.owner())
            .expect("Entry owner")
            .kind(),
        HirItemKind::Entry(_)
    ));
}

#[test]
fn generic_substitutions_are_candidate_local_and_specialize_result() {
    let generic = |owner| {
        TypeKind::GenericParam(GenericTypeParameterId::new(
            GenericTypeOwnerId::Detached(DetachedTypeOwnerId::new(owner)),
            0,
        ))
    };
    let first = generic(41);
    let second = generic(42);
    let fixture = typed_overload_fixture(
        "fn caller() { choose(1i64, 2i64); }\n",
        "choose",
        vec![
            TestCallableOverload::strict([first.clone(), first.clone()], first.clone()),
            TestCallableOverload::strict([second.clone(), second.clone()], second.clone()),
        ],
    );
    let report = analyze(&fixture).expect("generic overload analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 2)
        .expect("generic call facts");
    assert!(matches!(
        call.target(),
        CallTargetFact::Ambiguous { candidates, .. } if candidates.len() == 2
    ));
    assert_eq!(call.result(), Some(&TypeKind::I64));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 4);
    assert_eq!(physical[0].expected(), &CandidateExpectedType::Exact(first));
    assert_eq!(
        physical[1].expected(),
        &CandidateExpectedType::Exact(TypeKind::I64)
    );
    assert_eq!(
        physical[2].expected(),
        &CandidateExpectedType::Exact(second)
    );
    assert_eq!(
        physical[3].expected(),
        &CandidateExpectedType::Exact(TypeKind::I64)
    );
}

#[test]
fn typed_rest_spread_checks_one_container_slot_per_candidate_pass() {
    let fixture = typed_overload_fixture(
        "fn caller(values: Vec<i64>) { choose(values...); }\n",
        "choose",
        vec![TestCallableOverload::typed_rest(
            TypeKind::I64,
            TypeKind::Unit,
        )],
    );
    let report = analyze(&fixture).expect("typed-rest spread analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("typed-rest call facts");
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 1);
    assert_eq!(physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(
        physical[0].kind(),
        PhysicalArgumentEvaluationKind::TypedRestSpread
    );
    assert_eq!(physical[0].expected(), &CandidateExpectedType::Unchecked);
    assert_eq!(call.retained_argument_inference_facts().count(), 1);
    assert_eq!(
        call.arguments()[0].slots()[0].inferred(),
        Some(&TypeKind::Vec(Box::new(TypeKind::I64)))
    );
}

#[test]
fn fixed_literal_spread_counts_each_logical_slot_in_every_probe_and_replay() {
    let fixture = typed_overload_fixture(
        "fn caller() { choose([1i64, 2i64]...); }\n",
        "choose",
        vec![
            TestCallableOverload::fixed_literal([TypeKind::I64, TypeKind::I64], TypeKind::I64),
            TestCallableOverload::fixed_literal([TypeKind::U64, TypeKind::U64], TypeKind::U64),
        ],
    );
    let report = analyze(&fixture).expect("overloaded fixed-spread analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 1)
        .expect("fixed-spread call facts");
    assert!(matches!(
        call.target(),
        CallTargetFact::Selected { considered, .. } if considered.len() == 2
    ));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 6);
    assert_eq!(call.retained_argument_inference_facts().count(), 2);
    assert!(physical.iter().all(|evaluation| {
        evaluation.argument().get() == 0
            && evaluation.kind() == PhysicalArgumentEvaluationKind::FixedLiteralSpread
    }));
    assert_eq!(
        physical
            .iter()
            .map(|evaluation| evaluation.slot().get())
            .collect::<Vec<_>>(),
        vec![0, 1, 0, 1, 0, 1]
    );
    assert_eq!(physical[4].pass(), CandidateEvaluationPass::SelectedReplay);
    assert_eq!(physical[5].pass(), CandidateEvaluationPass::SelectedReplay);
}

#[test]
fn intentionally_unchecked_capacity_arguments_retain_clean_typed_facts() {
    let fixture = fixture(
        "fn caller() { String.with_capacity(size = 8, [9, 10]...); }\n",
        None,
    );
    let report = analyze(&fixture).expect("unchecked Capacity argument analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.arguments().len() == 2)
        .expect("unchecked call facts");
    assert!(matches!(
        call.target(),
        CallTargetFact::Selected { selected, considered }
            if matches!(selected.id(), CallableCandidateId::CapacityMethod(_))
                && considered.len() == 1
    ));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 2);
    assert!(
        physical
            .iter()
            .all(|evaluation| evaluation.pass() == CandidateEvaluationPass::Probe)
    );
    assert_eq!(physical[0].expected(), &CandidateExpectedType::Unmapped);
    assert_eq!(physical[1].expected(), &CandidateExpectedType::Unchecked);
    let retained = call.retained_argument_inference_facts().collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().all(|fact| fact.expected().is_none()));
    assert!(
        retained
            .iter()
            .all(|fact| fact.poison() == super::CallPoison::Clean)
    );
}

#[test]
fn nested_call_is_rechecked_inside_each_outer_candidate_pass() {
    let fixture = typed_overload_fixture(
        r"
fn identity(value: i64) -> i64 { value }
fn caller() { choose(identity(1i64)); }
",
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([TypeKind::U64], TypeKind::U64),
        ],
    );
    let report = analyze(&fixture).expect("nested contextual call analysis");
    let outer = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| matches!(facts.target(), CallTargetFact::Selected { considered, .. } if considered.len() == 2))
        .expect("outer overloaded call");
    let inner = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| facts.expression() != outer.expression())
        .expect("nested project call");
    let outer_physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == outer.expression())
        .collect::<Vec<_>>();
    let inner_physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == inner.expression())
        .collect::<Vec<_>>();
    assert_eq!(outer_physical.len(), 3);
    // The outer I64 probe and selected replay each probe the singleton inner
    // call once.  The outer U64 probe rejects the inner call's I64 result, so
    // that singleton performs its required deterministic recovery replay.
    assert_eq!(inner_physical.len(), 4);
    assert_eq!(
        outer_physical[2].pass(),
        CandidateEvaluationPass::SelectedReplay
    );
    assert_eq!(inner_physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(inner_physical[1].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(
        inner_physical[2].pass(),
        CandidateEvaluationPass::RejectedRecoveryReplay
    );
    assert_eq!(inner_physical[3].pass(), CandidateEvaluationPass::Probe);
}

#[test]
fn cancellation_before_first_candidate_slot_retains_no_physical_prefix() {
    let fixture = fixture(
        "fn choose(value: i64) -> i64 { value }\nfn caller() { choose(1i64); }\n",
        None,
    );
    let cancellation = AtomicBool::new(true);
    let (result, physical) = super::analyzer::analyze_final_project_with_physical_trace_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    );
    assert!(matches!(result, Err(FinalSemanticAnalysisError::Cancelled)));
    assert!(physical.is_empty());
}

#[test]
fn cancellation_after_one_completed_candidate_slot_retains_only_the_physical_prefix() {
    let fixture = fixture(
        concat!(
            "fn combine(left: i64, right: i64) -> i64 { left + right }\n",
            "fn caller() { combine(1i64, 2i64); }\n",
        ),
        None,
    );
    let cancellation = AtomicBool::new(false);
    let remaining = std::cell::Cell::new(1);
    let control = FinalSemanticAnalysisControl::new(&cancellation)
        .with_cancellation_after_completed_physical_slots(&remaining);
    let (result, physical) = super::analyzer::analyze_final_project_with_physical_trace_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        control,
    );

    assert!(matches!(result, Err(FinalSemanticAnalysisError::Cancelled)));
    assert!(cancellation.load(std::sync::atomic::Ordering::Acquire));
    assert_eq!(remaining.get(), 0);
    assert_eq!(physical.len(), 1);
    assert_eq!(physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(physical[0].argument().get(), 0);
    assert_eq!(physical[0].slot().get(), 0);
}

#[test]
fn missing_target_never_emits_candidate_physical_or_retained_facts() {
    let fixture = fixture("fn caller() { absent(1i64); }\n", None);
    let cancellation = AtomicBool::new(false);
    let (result, physical) = super::analyzer::analyze_final_project_with_physical_trace_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    );
    assert!(matches!(
        result,
        Err(FinalSemanticAnalysisError::CallResolutionFailed { .. }
            | FinalSemanticAnalysisError::ValueResolutionFailed { .. }
            | FinalSemanticAnalysisError::UnknownCallTarget { .. })
    ));
    assert!(physical.is_empty());
}

#[test]
fn nested_call_depth_limit_is_inclusive_and_one_over_is_terminal() {
    let nested_source = |depth: usize| {
        let mut expression = "1i64".to_owned();
        for _ in 0..depth {
            expression = format!("identity({expression})");
        }
        format!("fn identity(value: i64) -> i64 {{ value }}\nfn caller() {{ {expression}; }}\n")
    };
    let maximum = 3;
    let limits = CallableLimits::for_test(
        PRODUCTION_CALLABLE_LIMITS.max_path_segments(),
        PRODUCTION_CALLABLE_LIMITS.max_groups_per_callable(),
        PRODUCTION_CALLABLE_LIMITS.max_parameters_per_callable(),
        PRODUCTION_CALLABLE_LIMITS.max_overloads_per_key(),
        PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call(),
        maximum,
        PRODUCTION_CALLABLE_LIMITS.max_recovery_nodes(),
        PRODUCTION_CALLABLE_LIMITS.max_diagnostics(),
        PRODUCTION_CALLABLE_LIMITS.max_catalog_build_work(),
        PRODUCTION_CALLABLE_LIMITS.max_query_work(),
    );
    let exact = fixture(&nested_source(maximum), None);
    analyze_with_callable_limits(&exact, limits).expect("inclusive nested-call depth");

    let one_over = fixture(&nested_source(maximum + 1), None);
    assert!(matches!(
        analyze_with_callable_limits(&one_over, limits),
        Err(FinalSemanticAnalysisError::CallResolutionFailed { .. })
    ));
}

#[test]
fn production_analyzer_unifies_capacity_spelling_matrix_under_one_family() {
    let fixture = fixture(
        r"
fn string_call() { String.with_capacity(1); }
fn bytes_call() { Bytes.with_capacity(2); }
fn vec_dot() { Vec<i32>.with_capacity(3); }
fn vec_path() { Vec<i32>::with_capacity(4); }
fn vec_turbofish_dot() { Vec::<i32>.with_capacity(5); }
fn vec_turbofish_path() { Vec::<i32>::with_capacity(6); }
",
        None,
    );
    let report = analyze(&fixture).expect("typed Capacity spelling matrix");
    let calls = report.calls().map(|(_, facts)| facts).collect::<Vec<_>>();
    assert_eq!(calls.len(), 6);
    assert!(calls.iter().all(|facts| matches!(
        facts.target(),
        CallTargetFact::Selected { selected, considered }
            if matches!(selected.id(), CallableCandidateId::CapacityMethod(_))
                && considered.len() == 1
    )));
    assert_eq!(report.work().logical_argument_checks(), 6);
    assert_eq!(report.work().resolver_invocations(), 6);
    assert_eq!(report.work().candidate_argument_probes(), 6);
    assert_eq!(report.work().selected_replay_argument_visits(), 0);
    assert_eq!(report.work().retained_argument_fact_publications(), 6);
}

#[test]
fn bare_vec_capacity_retains_candidate_neutral_arguments_without_resolver_entry() {
    let fixture = fixture("fn caller() { Vec.with_capacity(1, 2, 3); }\n", None);
    let report = analyze(&fixture).expect("bare generic associated recovery");
    let (owner, call) = report.calls().next().expect("one retained Call fact");
    let Some(CallCalleeClassificationFact::AssociatedType { receiver, .. }) = call.callee() else {
        panic!("bare generic recovery retains its typed associated receiver")
    };
    assert!(matches!(
        call.target(),
        CallTargetFact::Missing {
            kind: UnknownCallKind::AssociatedType
        }
    ));
    assert_eq!(call.poison(), super::CallPoison::Recovered);
    assert!(call.result().is_some_and(TypeKind::contains_nominal_poison));
    assert_eq!(
        report.expression(owner).map(CheckedExpression::ty),
        call.result()
    );
    assert_eq!(report.ty(receiver), call.result());

    let type_report = report
        .type_resolution(receiver)
        .expect("wrong-arity receiver report");
    assert!(matches!(
        type_report.outcome(),
        ResolvedTypeRefOutcome::Poisoned(_)
    ));
    assert!(type_report.outcome().product().nodes().iter().any(|node| {
        node.node() == receiver
            && matches!(
                node.outcome(),
                TypeNameResolution::Failed(TypeResolutionFailure::WrongArity { actual: 0, .. })
            )
    }));

    assert_eq!(call.arguments().len(), 3);
    for (index, argument) in call.arguments().iter().enumerate() {
        assert_eq!(usize::from(argument.argument().get()), index);
        assert_eq!(argument.poison(), super::CallPoison::Clean);
        let [slot] = argument.slots() else {
            panic!("candidate-neutral recovery retains one authored expression slot")
        };
        assert_eq!(slot.slot().get(), 0);
        assert_eq!(slot.mapped(), None);
        assert_eq!(slot.expected(), None);
        assert_eq!(slot.inferred(), Some(&TypeKind::I32));
        assert_eq!(slot.poison(), super::CallPoison::Clean);
    }

    let accounting = call.accounting();
    assert_eq!(accounting.logical_argument_checks(), 3);
    assert_eq!(accounting.resolver_invocations(), 0);
    assert_eq!(accounting.candidate_argument_probes(), 0);
    assert_eq!(accounting.selected_replay_argument_visits(), 0);
    assert_eq!(accounting.retained_argument_fact_publications(), 3);
    assert_eq!(report.work().logical_argument_checks(), 3);
    assert_eq!(report.work().resolver_invocations(), 0);
    assert_eq!(report.work().candidate_argument_probes(), 0);
    assert_eq!(report.work().selected_replay_argument_visits(), 0);
    assert_eq!(report.work().retained_argument_fact_publications(), 3);
    assert_eq!(report.work().call_facts(), 1);
    assert_eq!(
        report
            .physical_candidate_argument_evaluations()
            .filter(|evaluation| evaluation.call_expression() == owner)
            .count(),
        0
    );
}

#[test]
fn unknown_associated_receiver_does_not_enter_candidate_neutral_arity_recovery() {
    let fixture = fixture("fn caller() { Unknown.with_capacity(1, 2); }\n", None);
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::TypeResolutionFailed { .. })
    ));
}

#[test]
fn extern_capability_member_call_uses_the_project_callable_catalog() {
    let fixture = fixture(
        r"
extern capability fixture_agent {
    fn observe() -> Unit effects { agent.observe }
}

fn load_story() -> Unit effects { agent.observe } {
    fixture_agent.observe()
    ()
}
",
        None,
    );
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "observe")
        .expect("extern capability callable symbol")
        .declaration();
    assert!(
        fixture
            .registered
            .environment()
            .callable_catalog()
            .project_record(declaration)
            .is_some(),
        "the accepted callable catalog must contain the exact declaration published by the symbol table"
    );

    let report =
        analyze(&fixture).expect("extern capability member call uses typed callable facts");
    assert!(report.calls().any(|(_, call)| matches!(
        call.target(),
        CallTargetFact::Selected { selected, .. }
            if matches!(selected.id(), CallableCandidateId::Project(owner) if owner == declaration)
    )));
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (call_owner, value_receiver, nominal_receiver) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            let HirCallCallee::UnresolvedDot {
                value_receiver,
                nominal_receiver,
                ..
            } = call.callee()
            else {
                return None;
            };
            Some((owner, *value_receiver, nominal_receiver.type_id()?))
        })
        .expect("extern capability member Call retains both source-backed candidates");
    assert!(matches!(
        report.call(call_owner).and_then(CallTargetFacts::callee),
        Some(CallCalleeClassificationFact::Value { expression })
            if expression == value_receiver
    ));
    assert_eq!(report.ty(nominal_receiver), None);
    assert_eq!(report.type_resolution(nominal_receiver), None);
}

#[test]
fn agent_intrinsic_call_retains_an_admissible_result_type() {
    let fixture = fixture(
        r"
fn run_smoke() -> Result<Unit, AgentError>
effects { agent.observe }
{
    observe()
    return Ok(())
}
",
        None,
    );

    let result = analyze(&fixture);
    if let Err(error) = &result {
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        panic!(
            "Agent intrinsic call must retain a final type: {error:?}\nexpressions: {:#?}",
            module.expressions().collect::<Vec<_>>()
        );
    }
    let report = result.expect("Agent intrinsic analysis");
    let function = function_owner(&fixture, "run_smoke");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == function)
        .expect("ordinary function symbol")
        .declaration();
    let call = report
        .calls()
        .map(|(_, call)| call)
        .find(|call| {
            matches!(
                call.target(),
                CallTargetFact::Selected { selected, .. }
                    if matches!(selected.id(), CallableCandidateId::Agent(_))
            )
        })
        .expect("Agent intrinsic call fact");
    assert_eq!(call.enclosing_callable(), Some(declaration));
}

#[test]
fn agent_composite_wait_retains_typed_predicate_calls() {
    let fixture = fixture(
        r"
fn composite_wait() -> Result<Unit, AgentError>
effects { agent.observe, agent.wait }
{
    wait(
        all(exists(signal(@signal.ready)), not(signal(@signal.ready).eq(false))),
        timeout = 5s,
        stable_frames = 1u32,
        poll_frames = 1u32,
    )
    return Ok(())
}
signal ready: bool
",
        None,
    );
    let report = analyze(&fixture).expect("Agent composite wait analysis");
    assert_eq!(
        report
            .calls()
            .filter(|(_, call)| matches!(
                call.target(),
                CallTargetFact::Selected { selected, .. }
                    if matches!(selected.id(), CallableCandidateId::Agent(_))
            ))
            .count(),
        6
    );
    assert_eq!(
        report
            .calls()
            .filter(|(_, call)| matches!(
                call.target(),
                CallTargetFact::Selected { selected, .. }
                    if matches!(
                        selected.id(),
                        CallableCandidateId::DomainMethod(DomainMethodId::ProbeCompare { .. })
                    )
            ))
            .count(),
        1
    );
}

#[test]
fn agent_action_result_field_retains_its_protocol_type() {
    let fixture = fixture(
        r"
fn run_smoke() -> Result<Unit, AgentError>
effects { agent.act.physical }
{
    let click_result = try pointer.click(
        viewport_point(12u32, 34u32),
        button = .secondary,
    )
    expect(click_result.accepted)
    return Ok(())
}
",
        None,
    );
    let report = analyze(&fixture).expect("Agent action result field analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let accepted = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Select(select)
                if matches!(select.member(), HirSelectedMember::Name(name) if name.as_str() == "accepted") =>
            {
                Some(owner)
            }
            _ => None,
        })
        .expect("accepted field expression");
    assert_eq!(
        report.expression(accepted).map(CheckedExpression::ty),
        Some(&TypeKind::Bool)
    );
}

#[test]
fn closure_agent_intrinsic_retains_its_lexical_ordinary_function_owner() {
    let fixture = fixture(
        r"
fn run_smoke() -> Result<Unit, AgentError>
effects {}
{
    let hidden = || { observe() }
    return Ok(())
}
",
        None,
    );
    let report = analyze(&fixture).expect("closure Agent intrinsic analysis");
    let function = function_owner(&fixture, "run_smoke");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.source_item() == function)
        .expect("ordinary function symbol")
        .declaration();
    let call = report
        .calls()
        .map(|(_, call)| call)
        .find(|call| {
            matches!(
                call.target(),
                CallTargetFact::Selected { selected, .. }
                    if matches!(selected.id(), CallableCandidateId::Agent(_))
            )
        })
        .expect("closure Agent intrinsic call fact");
    assert_eq!(call.enclosing_callable(), Some(declaration));
}

#[test]
fn direct_return_operand_must_match_the_ordinary_function_result() {
    let fixture = fixture(
        r"
fn run_smoke() -> Result<Unit, AgentError> {
    return 1
}
",
        None,
    );

    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. })
    ));
}

#[test]
fn extern_capability_parameter_locals_receive_their_declared_types() {
    let fixture = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
    fn read_metadata(path: String) -> String effects { fs.read }
    fn read_dormant(path: String) -> String effects { fs.read }
}

fn unrelated_read() -> String effects { fs.read } {
    fs.read_metadata(path = "unrelated.arcw")
}

fn unused_factory() -> ((Unit) -> String effects { fs.read }) {
    |_unit: Unit| -> String { fs.read_dormant(path = "dormant.arcw") }
}

fn load_story() -> String effects { fs.read } {
    fs.read_text(path = "story.arcw")
}
"#,
        None,
    );

    analyze(&fixture).expect("extern capability parameter types belong to final semantic facts");
}

#[test]
fn flow_effect_and_no_effect_operands_share_the_typed_effect_projection() {
    let fixture = fixture(
        r"
flow observed()
effects { agent.observe }
ensures no_effect network.request
{}
",
        None,
    );
    let report = analyze(&fixture)
        .expect("Flow effect/no_effect identities are seeded by the HIR inventory");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let flow = module
        .source_ordered_items()
        .iter()
        .copied()
        .find(|owner| {
            module
                .resolve_item(*owner)
                .is_ok_and(|item| matches!(item.kind(), HirItemKind::Flow(_)))
        })
        .expect("Flow owner");
    assert_eq!(
        report
            .item(flow)
            .expect("checked Flow item")
            .effects()
            .to_labels(),
        ["agent.observe"],
        "the exposed Flow row includes effects operands but not no_effect prohibitions"
    );
}

#[test]
fn explicit_empty_flow_effect_bound_rejects_a_function_value_effect() {
    let fixture = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn make_loader(
    load: (String) -> String effects { fs.read }
) -> ((Unit) -> String effects { fs.read }) {
    |_unit: Unit| -> String { load("story.arcw") }
}

flow @flow.returned_closure_callback_call returned_closure_callback_call
effects { }
{
    let loader = make_loader(|path: String| -> String {
        fs.read_text(path = path)
    })
    let body = loader(())
}
"#,
        None,
    );

    let error = analyze(&fixture).expect_err("the authored empty Flow row is an upper bound");
    let FinalSemanticAnalysisError::EffectUpperBoundExceeded {
        callable,
        missing,
        trace_notes,
        ..
    } = error
    else {
        panic!("expected a Flow effect-bound rejection, got {error:?}")
    };
    assert_eq!(callable, "flow.returned_closure_callback_call");
    assert_eq!(missing.to_labels(), ["fs.read"]);
    let trace = trace_notes.join("\n");
    assert!(trace.contains("function value call `loader`"), "{trace}");
    assert!(
        trace.contains("returned function value from `make_loader`"),
        "{trace}"
    );
    assert!(
        trace.contains("higher-order argument `load` captured by returned closure"),
        "{trace}"
    );
    assert!(trace.contains("call `fs.read_text`"), "{trace}");
    assert!(!trace.contains("fs.read_metadata"), "{trace}");
    assert!(!trace.contains("fs.read_dormant"), "{trace}");
}

#[test]
fn report_rejects_a_foreign_hir_or_symbol_generation() {
    let accepted = fixture("fn root() {}\n", None);
    let input = complete_input(&accepted);
    let checked_callables = checked_callables(&accepted, &input);
    let report = FinalSemanticAnalysis::try_new(
        accepted.project.executable_view().expect("accepted HIR"),
        &accepted.symbols,
        checked_callables,
        input,
    )
    .expect("accepted report");
    let foreign = fixture("fn other() {}\n", None);

    assert!(matches!(
        report.validate_generation(
            foreign.project.executable_view().expect("foreign HIR"),
            &foreign.symbols,
        ),
        Err(FinalSemanticAnalysisError::GenerationMismatch
            | FinalSemanticAnalysisError::SymbolGenerationMismatch
            | FinalSemanticAnalysisError::CatalogGenerationMismatch)
    ));
}

#[test]
fn project_index_preserves_same_named_module_scoped_flows() {
    let fixture = fixture(
        "flow opening {\n    goto @flow.opening\n}\n",
        Some("flow opening {\n    goto @flow.opening\n}\n"),
    );
    let analysis = analyze(&fixture).expect("same-named module Flow analysis");
    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("same-named-module-flow"),
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        &analysis,
        &CheckedEntryCatalog::default(),
    )
    .expect("module-preserving project index");

    let flows = index
        .entities()
        .keys()
        .filter_map(|identity| match identity {
            ProjectEntityId::StructuralFlow(declaration) => Some((identity, declaration)),
            ProjectEntityId::Public(_) => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(flows.len(), 2);
    assert!(
        flows
            .iter()
            .all(|(_, declaration)| declaration.public_id().as_str() == "flow.opening")
    );
    assert_ne!(flows[0].1.module(), flows[1].1.module());
    assert!(flows.iter().all(|(identity, _)| {
        index
            .flow_control_summary(identity)
            .is_some_and(|summary| summary.static_goto_count() == 1)
    }));

    assert_eq!(index.relations().len(), 2);
    assert!(index.relations().iter().all(|relation| {
        relation.from() == relation.to()
            && matches!(relation.from(), ProjectEntityId::StructuralFlow(_))
    }));
}
