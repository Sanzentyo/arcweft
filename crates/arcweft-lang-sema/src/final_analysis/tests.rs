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
use arcweft_core::{
    entry::TypeLayoutHash,
    pattern::{RuntimeCheckedType, RuntimeOpaqueTypeProducerId},
    value::{RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeValue},
};
use arcweft_interaction_model::dialogue::CharacterDialogueCustomFieldId;
use arcweft_lang_hir::{
    database::HirDatabase,
    dialogue_application::HirPostfixBracketCandidates,
    expr::{HirCallCallee, HirExprKind, HirSelectedMember},
    item::{HirFunctionBody, HirItemKind},
    lowering::{HirModuleKey, LoweringRequest},
    module::HirModule,
    pattern::HirPatternKind,
    project::{HirProject, HirProjectBuilder, HirProjectModule, HirSemanticPathStep},
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
    types::TypePath,
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceName, SourceRange, identity::SourceSnapshotId,
};

use super::match_coverage::{
    CheckedCoverageWitness, CheckedMatchBuildError, CheckedMatchLimitKind, CheckedUnreachableReason,
};
use super::semantic_transcript::SemanticTranscriptError;
use super::{
    CallAnalysisOutcome, CallTargetFacts, CandidateEvaluationPass, CandidateExpectedType,
    CharacterDialogueFieldCoordinate, CheckedAssertionDisposition, CheckedBinding,
    CheckedCallableJoinError, CheckedCharacterDialogueTarget, CheckedDropFade, CheckedDropPolicy,
    CheckedEvaluatedEffect, CheckedExpression, CheckedExpressionEdgeError,
    CheckedExpressionResolution, CheckedFunctionExecution, CheckedItem, CheckedItemRole,
    CheckedIteration, CheckedIteratorFamily, CheckedMatchLimits, CheckedPatchOperation,
    CheckedPattern, CheckedPatternResolution, CheckedSelectResolution, CheckedStatement,
    CheckedStatementRole, CheckedSuspensionRole, CheckedSuspensionStatement, CheckedTryBoundary,
    CheckedTryCarrier, CheckedTypeSelection, CheckedValueResolution, CheckedVariantOwner,
    FinalCallSealLocation, FinalSemanticAnalysis, FinalSemanticAnalysisControl,
    FinalSemanticAnalysisError, FinalSemanticAnalysisInput, FinalSemanticCatalogs,
    PhysicalArgumentEvaluationKind, PostfixBracketResolution, RegisteredSemanticValueId,
    SemanticFactFamily, analyze_final_project,
};
#[path = "tests/match_coverage.rs"]
mod match_coverage;
use crate::{
    CheckedNeedProducerAdmissionError,
    assertion::{AssertionBuildProfile, AssertionContext, AssertionRuntimePolicy},
    callable::{
        AdapterPackageId, AgentIntrinsicSignatureId, CallConstraintInvariant, CallPoison,
        CallableAccess, CallableArgumentPolicy, CallableAuthorityRank, CallableCandidateId,
        CallableDocumentation, CallableEffectSchema, CallableGenericParameterIssuer,
        CallableGroupIndex, CallableGroupKind, CallableLimits, CallableLookupKey, CallableName,
        CallableOverloadIndex, CallableParameter, CallableParameterAdmission,
        CallableParameterGroup, CallableParameterIndex, CallableParameterPassing,
        CallableParameterPresence, CallablePath, CallableProviderId, CallableReceiverMode,
        CallableRecord, CallableSignatureSchema, CallableValidator, CatalogCallableEntry,
        CheckedCallArgumentSlotSource, CheckedCallExecutionSource, CheckedClosureId,
        DialogueCallableId, DomainMethodId, DropCallableId, EffectContractOrigin,
        EnvironmentCallableCatalog, EnvironmentCallableId, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentCallablePublicationDigest,
        EnvironmentDeclarationOrdinal, LineContextMethodId, LineScheduleCallableId,
        NonEmptyCallableSet, PRODUCTION_CALLABLE_LIMITS, PresentationCallableId,
        ProjectCallablePath, RegisteredCallableCatalog, SemanticSignatureSurface,
        SpreadArgumentPolicy, StageMethodId, UnknownCallKind, UnknownNamedArgumentPolicy,
        ViewModifierId,
    },
    character_dialogue::CharacterDialogueCustomFieldBinding,
    effect_row::EffectRow,
    effects::{EffectId, EffectSet},
    env::{
        TypeCheckEnv,
        nominal::{
            AcceptedNominalId, AcceptedNominalOrigin, AcceptedNominalOwnerId,
            AcceptedNominalRecord, AcceptedNominalSemantics, OpenNominalArity, OpenNominalPattern,
            OpenNominalRule, OpenNominalRuleId, OpenNominalScope,
        },
    },
    nominal::TypeNameResolution,
    ownership::{
        CheckedOwnershipCertificate, CheckedOwnershipError, CheckedOwnershipLimits,
        RetainedValueDisposition, RuntimeOwnershipProjection, RuntimeOwnershipRejection,
        RuntimeProducerArgumentClassifier,
    },
    project_index::{ProgramHash, ProjectCallableKind, ProjectEntityId, ProjectSemanticIndex},
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
    semantic_coordinate::{
        CheckedExpressionChildRole, SemanticCoordinateIndex, StableCheckedValueCoordinate,
    },
    signature::{
        SignatureQuery, SignatureQueryControl, SignatureQueryError, SignatureQueryOutcome,
        SignatureQueryStep, query_signature,
    },
    types::{
        AgentBuiltinType, EntityKind, GenericParameterOwnerId, GenericTypeParameterId,
        StageActorHandleType, TypeGenericUseCollector, TypeKind,
    },
};

pub(super) struct Fixture {
    pub(super) project: HirProject,
    pub(super) symbols: Arc<ProjectSymbolTable>,
    pub(super) registered: RegisteredSemanticWorld,
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

pub(super) fn fixture(root_source: &str, child_source: Option<&str>) -> Fixture {
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

pub(super) fn environment_overload_fixture(root_source: &str) -> Fixture {
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
                CallableGenericParameterIssuer::empty(),
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

fn character_registration(
    document_id: &str,
    path: &str,
    source: &str,
) -> (
    Arc<SourceDocument>,
    SourceBackedCharacterCatalog,
    ExternalRegistrationFact,
) {
    let manifest_document = source_document(document_id, path, source);
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

fn akane_character_registration() -> (
    Arc<SourceDocument>,
    SourceBackedCharacterCatalog,
    ExternalRegistrationFact,
) {
    character_registration(
        "arcweft-test://sema/final/character-akane",
        "character-akane.json",
        AKANE_CHARACTER_MANIFEST,
    )
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
    admission: CallableParameterAdmission,
    passing: CallableParameterPassing,
    presence: CallableParameterPresence,
}

impl TestCallableParameter {
    fn exact(ty: TypeKind) -> Self {
        Self {
            admission: CallableParameterAdmission::checked(ty),
            passing: CallableParameterPassing::PositionalOrNamed,
            presence: CallableParameterPresence::Required,
        }
    }

    fn typed_rest(ty: TypeKind) -> Self {
        Self {
            admission: CallableParameterAdmission::checked(ty),
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
    generic_issuer: CallableGenericParameterIssuer,
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
            generic_issuer: CallableGenericParameterIssuer::empty(),
        }
    }

    fn with_generic_issuer(mut self, generic_issuer: CallableGenericParameterIssuer) -> Self {
        self.generic_issuer = generic_issuer;
        self
    }

    fn typed_rest(item: TypeKind, result: TypeKind) -> Self {
        Self {
            parameters: vec![TestCallableParameter::typed_rest(item)],
            result,
            effects: EffectSet::new(),
            spread: SpreadArgumentPolicy::TypedRest,
            generic_issuer: CallableGenericParameterIssuer::empty(),
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
            generic_issuer: CallableGenericParameterIssuer::empty(),
        }
    }

    fn effectful(result: TypeKind, effect: &str) -> Self {
        Self {
            parameters: Vec::new(),
            result,
            effects: EffectSet::from_labels([effect]).expect("valid test effect"),
            spread: SpreadArgumentPolicy::Reject,
            generic_issuer: CallableGenericParameterIssuer::empty(),
        }
    }
}

/// Replaces only the accepted environment callable catalog while preserving
/// the source-backed project, nominal generation, and symbol authority. This
/// lets the matrix exercise typed schemas that the environment input codec
/// intentionally cannot author (notably function-value and generic-parameter types).
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
    let TestCallableOverload {
        parameters,
        result,
        effects,
        spread,
        generic_issuer,
    } = overload;
    let parameters = parameters
        .into_iter()
        .enumerate()
        .map(|(index, parameter)| {
            CallableParameter::try_new(
                CallableParameterIndex::try_from_usize(index).expect("parameter index"),
                Some(CallableName::try_new(format!("value{index}")).expect("parameter name")),
                parameter.admission,
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
            result,
            CallableEffectSchema::Fixed(EffectRow::closed(effects)),
            CallableArgumentPolicy::new(UnknownNamedArgumentPolicy::Reject, spread),
            CallableValidator::Ordinary,
            generic_issuer,
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
) -> (
    Arc<arcweft_lang_hir::project::HirProjectEvaluationTopology>,
    Arc<crate::callable::CheckedCallableCatalog>,
) {
    super::analyzer::freeze_checked_callables_for_test(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        super::FinalSemanticCatalogs::production(&fixture.registered),
        input,
    )
    .expect("checked callable catalog")
}

pub(super) fn analyze(
    fixture: &Fixture,
) -> Result<FinalSemanticAnalysis, FinalSemanticAnalysisError> {
    let cancellation = AtomicBool::new(false);
    analyze_final_project(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        FinalSemanticCatalogs::production(&fixture.registered),
        FinalSemanticAnalysisControl::new(&cancellation),
    )
    .map_err(super::FinalSemanticProjectError::into_semantic_fixture_error)
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
    .map_err(super::FinalSemanticProjectError::into_semantic_fixture_error)
}

#[test]
fn dialogue_line_plan_bindings_are_inferred_in_source_order() {
    let fixture = character_nominal_fixture(concat!(
        "pub character @character.akane Akane as akane {}\n",
        "flow line_handles() -> String {\n",
        "    let (_, cue) = akane(voice=auto)[聞いて。[p]]\n",
        "    with:\n",
        "        let actor = akane.stage.acquire(scope=line)\n",
        "        let cue = at(0.42s):\n",
        "            actor.look(.normal, crossfade=120ms)\n",
        "        let voice = line.voice_handle()\n",
        "        out (voice, cue)\n",
        "    return \"done\"\n",
        "}\n",
    ));
    let report = analyze(&fixture).expect("typed Dialogue line-plan bindings");
    let character = CharacterId::try_new("character.akane").expect("Character identity");
    let acquire = report
        .calls()
        .find(|(_, call)| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::StageMethod(StageMethodId::Acquire)
            })
        })
        .map(|(_, call)| call)
        .expect("exact stage acquire call");
    assert_eq!(
        selected_application(acquire).result().ty(),
        &TypeKind::StageActorHandle(StageActorHandleType::Exact(character.clone()))
    );
    let look = report
        .calls()
        .find(|(_, call)| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::StageMethod(StageMethodId::Look)
            })
        })
        .map(|(_, call)| call)
        .expect("exact stage look call");
    assert_eq!(
        selected_application(look).result().ty(),
        &TypeKind::CueHandle
    );
    assert!(report.calls().any(|(_, call)| {
        call.selected_application().is_some_and(|application| {
            application.core().candidates().selected().id()
                == &CallableCandidateId::LineContextMethod(LineContextMethodId::VoiceHandle)
                && application.result().ty() == &TypeKind::VoiceHandle
        })
    }));
    assert!(report.calls().any(|(_, call)| {
        call.selected_application().is_some_and(|application| {
            application.core().candidates().selected().id()
                == &CallableCandidateId::LineSchedule(LineScheduleCallableId::At)
        })
    }));
    assert!(!report.calls().any(|(_, call)| {
        call.selected_application().is_some_and(|application| {
            matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::CapacityMethod(_)
            ) && application.result().ty() == &TypeKind::VoiceHandle
        })
    }));
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let dialogue_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::DialogueContentApplication(_)
            )
            .then_some(owner)
        })
        .expect("typed Dialogue application owner");
    assert_dialogue_application_result_authority(
        &report,
        dialogue_owner,
        &TypeKind::DialogueLine(Box::new(TypeKind::Tuple(vec![
            TypeKind::VoiceHandle,
            TypeKind::CueHandle,
        ]))),
    );
    assert_dialogue_line_plan_edges(&report, module);
}

fn assert_dialogue_application_result_authority(
    report: &FinalSemanticAnalysis,
    owner: arcweft_lang_hir::identity::ExprId,
    expected: &TypeKind,
) {
    let expression = report
        .expression(owner)
        .expect("Dialogue application expression fact");
    let call = report.call(owner).expect("Dialogue application call fact");
    let application = selected_application(call);
    assert_eq!(expression.ty(), expected);
    assert_eq!(application.result().ty(), expected);
    assert_eq!(
        application.core().candidates().selected().schema().result(),
        expected
    );
}

fn assert_dialogue_line_plan_edges(report: &FinalSemanticAnalysis, module: &HirModule) {
    let (dialogue_owner, _) = module
        .expressions()
        .find(|(_, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::DialogueContentApplication(_)
            )
        })
        .expect("DialogueContentApplication owner");
    let dialogue_edges = report
        .checked_child_edges(dialogue_owner)
        .expect("Dialogue line-plan child edges have checked evidence");
    assert!(dialogue_edges.iter().any(|(_, role)| {
        matches!(
            role,
            CheckedExpressionChildRole::LinePlanOptionValue { .. }
                | CheckedExpressionChildRole::LinePlanLetValue { .. }
                | CheckedExpressionChildRole::LinePlanOut { .. }
                | CheckedExpressionChildRole::LinePlanTimelineAssert { .. }
                | CheckedExpressionChildRole::LinePlanExpression { .. }
                | CheckedExpressionChildRole::LinePlanTimedCueAnchor { .. }
                | CheckedExpressionChildRole::LinePlanTimedCueBody { .. }
        )
    }));
}

#[test]
fn let_else_publishes_success_bindings_after_a_diverging_failure_body() {
    let fixture = fixture(
        concat!(
            "flow main() -> String {\n",
            "    let Some(route) = Some(@flow.done) else {\n",
            "        return \"missing\"\n",
            "    }\n",
            "    goto route\n",
            "}\n",
            "flow done() -> String { return \"done\" }\n",
        ),
        None,
    );
    analyze(&fixture).expect("typed LetElse with diverging failure body");
}

#[test]
fn compact_choice_goto_owns_canonical_ids_and_exact_flow_target() {
    let fixture = fixture(
        r#"
flow main {
    scope dream {
        choice @.first {
            @.next "Next" -> @flow.done
        }
    }
}

flow done() -> String {
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("static Choice goto analysis");
    let choice = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Choice(choice) => Some((expression, choice)),
            _ => None,
        })
        .expect("one checked Choice expression");
    assert_eq!(choice.0.ty(), &TypeKind::Never);
    assert_eq!(
        choice.1.public_id().map(arcweft_id::PublicId::as_str),
        Some("choice.main.dream.first")
    );
    assert_eq!(
        choice
            .1
            .option_ids()
            .iter()
            .map(arcweft_id::PublicId::as_str)
            .collect::<Vec<_>>(),
        ["choice.main.dream.first.next"]
    );
    let [target] = choice.1.gotos() else {
        panic!("one checked Choice goto")
    };
    assert_eq!(target.arm(), 0);
    assert_eq!(target.target().public_id().as_str(), "flow.done");
}

#[test]
fn assignment_semantics_admit_only_one_direct_local_nominal_field() {
    let fixture = fixture(
        concat!(
            "struct Point { x: i64, active: bool }\n",
            "fn update(point: Point) -> bool {\n",
            "    point.active = true\n",
            "    point.active\n",
            "}\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("direct local nominal field assignment is accepted");
    let assignment = report
        .statements()
        .find_map(|(_, statement)| match statement.role() {
            CheckedStatementRole::Assignment(assignment) => Some(assignment),
            _ => None,
        })
        .expect("assignment statement retains one checked place");

    assert_eq!(assignment.place().field().declaration_ordinal(), 1);
    assert_eq!(assignment.place().field_type(), &TypeKind::Bool);
    assert_eq!(assignment.value_type(), &TypeKind::Bool);
    assert_eq!(
        assignment.place().nominal().declaration().name().as_str(),
        "Point"
    );
    assert!(matches!(
        report
            .local(assignment.place().local())
            .expect("assignment base is one accepted local")
            .ty(),
        TypeKind::ProjectNominal(nominal)
            if nominal.declaration() == assignment.place().nominal().declaration()
    ));
}

#[test]
fn assignment_semantics_reject_non_direct_or_non_nominal_places_and_type_mismatch() {
    let parser_rejected_cases = [
        ("bare-local", "fn invalid(value: i64) { value = 1i64 }\n"),
        (
            "index-place",
            "fn invalid(values: Vec<i64>) { values[0] = 1i64 }\n",
        ),
        (
            "dereference-place",
            "fn invalid(value: &mut i64) { *value = 1i64 }\n",
        ),
    ];
    for (label, source) in parser_rejected_cases {
        let fixture = fixture(source, None);
        assert!(
            fixture.project.executable_view().is_err(),
            "{label} must be rejected before final semantic publication",
        );
    }

    let semantic_cases = [
        (
            "nested-field",
            concat!(
                "struct Point { x: i64 }\n",
                "struct Wrapper { point: Point }\n",
                "fn invalid(wrapper: Wrapper) { wrapper.point.x = 1i64 }\n",
            ),
        ),
        (
            "entity-field",
            concat!(
                "fn controller() -> Result<Unit, AgentError> effects {} { Ok(()) }\n",
                "entry agent @entry.agent.main { controller = controller }\n",
                "fn invalid() { @entry.agent.main.name = \"changed\" }\n",
            ),
        ),
        (
            "rhs-type-mismatch",
            concat!(
                "struct Point { x: i64 }\n",
                "fn invalid(point: Point) { point.x = true }\n",
            ),
        ),
    ];

    for (label, source) in semantic_cases {
        let fixture = fixture(source, None);
        if fixture.project.executable_view().is_err() {
            continue;
        }
        assert!(
            matches!(
                analyze(&fixture),
                Err(FinalSemanticAnalysisError::WrongPayloadFamily)
            ),
            "{label} must be rejected by the checked assignment authority",
        );
    }
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
    .map_err(super::FinalSemanticProjectError::into_semantic_fixture_error)
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
    let complete = fact.complete().expect("complete expression fixture");
    *fact = CheckedExpression::new(
        complete.ty().clone(),
        complete.type_selection(),
        effects.clone(),
        complete.resolution().clone(),
    )
    .into();
    effects
}

fn input_from_report(report: &FinalSemanticAnalysis) -> FinalSemanticAnalysisInput {
    let mut input = FinalSemanticAnalysisInput::new();
    let mut callable_joins = BTreeMap::new();
    for (owner, fact) in report.types() {
        input.push_type(owner, fact.clone());
    }
    for (owner, fact) in report.locals() {
        input.push_local(owner, fact.clone());
    }
    for (owner, fact) in report.captures() {
        input.push_capture(owner, fact.clone());
    }
    for (owner, fact) in report.expressions() {
        input.push_expression(owner, fact.clone());
    }
    for (owner, fact) in report.patterns() {
        input.push_pattern(owner, fact.clone());
    }
    for (owner, fact) in report.statements() {
        input.push_statement(owner, fact.clone());
    }
    for (owner, fact) in report.items() {
        input.push_item(owner, fact.clone());
    }
    for (owner, fact) in report.calls() {
        input.push_call(fact.clone());
        let join = match report.checked_callable_join(owner) {
            Ok(join) => Ok(join.clone()),
            Err(CheckedExpressionEdgeError::Callable(error)) => Err(error),
            Err(CheckedExpressionEdgeError::Child(_)) => Err(CheckedCallableJoinError::NotSelected),
        };
        callable_joins.insert(owner, join);
    }
    input.set_callable_joins(callable_joins);
    input
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
            HirStmtKind::Wait { .. } => {
                CheckedStatementRole::Suspension(Box::new(CheckedSuspensionStatement::Wait))
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
    let (topology, checked_callables) = checked_callables(&fixture, &input);
    let report = FinalSemanticAnalysis::try_new(
        executable,
        &fixture.symbols,
        topology,
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
fn nested(need: Need<Result<i64, String>>) -> Result<i64, String> {
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
fn prefix_try_unwraps_the_result_of_await_inside_a_matching_result_boundary() {
    let fixture = fixture(
        r"
fn nested(need: Need<Result<i64, String>>) -> Result<i64, String> {
    Ok(try await need)
}
",
        None,
    );
    let report = analyze(&fixture).expect("Try of Await final analysis");
    assert!(report.expressions().any(|(_, expression)| {
        expression.ty() == &TypeKind::I64
            && expression
                .effects()
                .iter()
                .any(|effect| effect.as_str() == "control.suspend")
    }));
    assert!(report.expressions().any(|(_, expression)| {
        matches!(
            expression.resolution(),
            CheckedExpressionResolution::Try(tried)
                if matches!(tried.carrier(), CheckedTryCarrier::Result {
                    success: TypeKind::I64,
                    residual,
                } if matches!(residual.as_ref(), TypeKind::String))
                    && matches!(tried.boundary(), CheckedTryBoundary::Callable(_))
        )
    }));
}

#[test]
fn standard_zero_argument_need_callable_uses_accepted_nominal_results() {
    let fixture = fixture(
        r"
type ArcResult<T> = Result<T, ArcError>

fn load_opening_assets() -> ArcResult<ImageHandle> {
    let bg = try await load_bg()
    Ok(bg)
}
",
        None,
    );
    let report = analyze(&fixture).unwrap_or_else(|error| {
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        panic!(
            "standard load_bg final analysis failed: {error:?}\nexpressions: {:#?}",
            module.expressions().collect::<Vec<_>>()
        )
    });
    let awaited = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Await(awaited) => Some(awaited),
            _ => None,
        })
        .expect("fixture retains one Await fact");
    let operand = report
        .expression(awaited.operand())
        .expect("Await operand has one checked expression");
    let TypeKind::Need(result) = operand.ty() else {
        panic!("load_bg result is one unary Need")
    };
    let TypeKind::Result { ok, error } = result.as_ref() else {
        panic!("load_bg Need payload is one Result")
    };
    assert!(matches!(
        ok.as_ref(),
        TypeKind::AcceptedNominal(nominal)
            if fixture
                .registered
                .environment()
                .nominal_world()
                .nominal_catalog()
                .exact(nominal.declaration().canonical_path())
                .is_some_and(|record| matches!(
                    record.semantics(),
                    AcceptedNominalSemantics::Opaque(carrier)
                        if carrier.producer().as_str() == "std.image_handle"
                ))
    ));
    assert!(matches!(
        error.as_ref(),
        TypeKind::AcceptedNominal(nominal)
            if fixture
                .registered
                .environment()
                .nominal_world()
                .nominal_catalog()
                .exact(nominal.declaration().canonical_path())
                .is_some_and(|record| matches!(
                    record.semantics(),
                    AcceptedNominalSemantics::Opaque(carrier)
                        if carrier.producer().as_str() == "std.arc_error"
                ))
    ));

    let classifier = RuntimeProducerArgumentClassifier::try_new(&report, &fixture.registered)
        .expect("final analysis and accepted world share one symbol lease");
    let public = fixture
        .registered
        .checked_ownership(&report, ok, CheckedOwnershipLimits::PRODUCTION)
        .expect("public ownership summary uses the exact accepted opaque row");
    assert_eq!(
        public.disposition(),
        RetainedValueDisposition::SnapshotClone
    );
    let admission = classifier
        .classify(ok)
        .expect("the exact accepted opaque catalog row admits ImageHandle");
    let RuntimeOwnershipProjection::Checked(RuntimeCheckedType::Opaque { owner }) =
        admission.projection()
    else {
        panic!("accepted ImageHandle uses the exact core opaque owner")
    };
    let value = owner
        .try_wrap(RuntimeValue::Unit)
        .expect("an exact opaque owner constructs its value");
    admission
        .validate_live_value(&value)
        .expect("the exact accepted opaque carrier accepts its live value");
    admission
        .try_digest(&value, 1_024)
        .expect("the exact accepted opaque value has canonical identity");
    let restored = admission
        .try_snapshot(&value)
        .expect("the exact accepted opaque value snapshots")
        .into_runtime_value()
        .expect("the core snapshot restores the opaque value");
    admission
        .validate_live_value(&restored)
        .expect("restored opaque evidence remains exact");

    let foreign = self::fixture("fn foreign() {}", None);
    let stale = RuntimeProducerArgumentClassifier::try_new(&report, &foreign.registered)
        .expect_err("a foreign registered world cannot reuse ownership evidence");
    assert_eq!(
        stale.rejection(),
        Some(RuntimeOwnershipRejection::StaleAuthority)
    );
}

fn ownership_test_type_path(name: &str) -> TypePath {
    TypePath::from(
        ProjectSymbolPath::new(
            ModulePathRoot::ImplicitCrate,
            [ProjectSymbolSegment::try_new(name).expect("ownership test type segment")],
        )
        .expect("ownership test type path"),
    )
}

fn generic_test_owner(owner: u64) -> AcceptedNominalId {
    AcceptedNominalId::new(
        AcceptedNominalOwnerId::Standard,
        ownership_test_type_path(&format!("GenericOwner{owner}")),
    )
}

fn image_handle_ownership_certificate(base: TypeCheckEnv) -> CheckedOwnershipCertificate {
    let fixture = fixture_with_base_environment("fn stable() {}", None, base);
    let report = analyze(&fixture).expect("ownership fixture final analysis");
    let image = fixture
        .registered
        .environment()
        .nominal_world()
        .nominal_catalog()
        .exact(&ownership_test_type_path("ImageHandle"))
        .expect("standard ImageHandle row")
        .try_instantiate([])
        .expect("zero-arity ImageHandle");
    fixture
        .registered
        .checked_ownership(&report, &image, CheckedOwnershipLimits::PRODUCTION)
        .expect("ImageHandle ownership")
}

#[test]
fn ownership_evidence_ignores_unrelated_accepted_catalog_rows() {
    let baseline = image_handle_ownership_certificate(TypeCheckEnv::standard());
    let unrelated = AcceptedNominalRecord::try_new_opaque(
        AcceptedNominalId::new(
            AcceptedNominalOwnerId::Standard,
            ownership_test_type_path("UnrelatedOpaque"),
        ),
        0,
        RuntimeOpaqueTypeProducerId::try_new("test.unrelated")
            .expect("unrelated producer identity"),
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::SnapshotOnly,
        AcceptedNominalOrigin::Test,
        None,
    )
    .expect("unrelated accepted opaque row");
    let with_unrelated = image_handle_ownership_certificate(
        TypeCheckEnv::standard()
            .try_with_nominal_record(unrelated)
            .expect("extend accepted catalog"),
    );

    assert_eq!(baseline.evidence(), with_unrelated.evidence());
}

#[test]
fn public_ownership_summary_keeps_need_fail_closed_until_live_carrier_cut() {
    let fixture = fixture("fn pending() {}", None);
    let report = analyze(&fixture).expect("Need payload has an exact semantic identity");
    let need = TypeKind::Need(Box::new(TypeKind::Stream {
        item: Box::new(TypeKind::I64),
        error: Box::new(TypeKind::String),
    }));

    fixture
        .registered
        .checked_ownership(&report, &need, CheckedOwnershipLimits::PRODUCTION)
        .expect_err("public Need ownership remains unavailable before Cut 5");
}

#[test]
fn public_ownership_type_node_limit_is_exact_and_fails_one_over() {
    let fixture = fixture("fn stable() {}", None);
    let report = analyze(&fixture).expect("ownership limits fixture final analysis");
    let ty = TypeKind::Tuple(vec![TypeKind::I32, TypeKind::Bool]);
    let exact = CheckedOwnershipLimits {
        max_type_nodes: 3,
        max_recursion_depth: 1,
        ..CheckedOwnershipLimits::PRODUCTION
    };

    fixture
        .registered
        .checked_ownership(&report, &ty, exact)
        .expect("the root and two children exactly consume three type nodes");
    assert_eq!(
        fixture.registered.checked_ownership(
            &report,
            &ty,
            CheckedOwnershipLimits {
                max_type_nodes: 2,
                ..exact
            },
        ),
        Err(CheckedOwnershipError::WorkLimit)
    );
}

#[test]
fn public_ownership_recursion_limit_is_exact_and_fails_one_over() {
    let fixture = fixture("fn stable() {}", None);
    let report = analyze(&fixture).expect("ownership limits fixture final analysis");
    let ty = TypeKind::Tuple(vec![TypeKind::Option(Box::new(TypeKind::I32))]);
    let exact = CheckedOwnershipLimits {
        max_recursion_depth: 2,
        ..CheckedOwnershipLimits::PRODUCTION
    };

    fixture
        .registered
        .checked_ownership(&report, &ty, exact)
        .expect("the grandchild exactly consumes recursion depth two");
    assert_eq!(
        fixture.registered.checked_ownership(
            &report,
            &ty,
            CheckedOwnershipLimits {
                max_recursion_depth: 1,
                ..exact
            },
        ),
        Err(CheckedOwnershipError::WorkLimit)
    );
}

pub(super) fn project_nominal_expression_type(
    report: &FinalSemanticAnalysis,
    name: &str,
) -> TypeKind {
    report
        .expressions()
        .find_map(|(_, expression)| match expression.ty() {
            TypeKind::ProjectNominal(nominal) if nominal.declaration().name().as_str() == name => {
                Some(expression.ty().clone())
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing checked expression of project nominal `{name}`"))
}

#[test]
fn project_nominal_schema_rejects_an_affine_struct_field() {
    let fixture = fixture(
        concat!(
            "struct Retained { stable: i64, handle: CueHandle }\n",
            "fn retain(value: Retained) -> Retained { value }\n",
        ),
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::NominalSchemaProjection(
            super::NominalSchemaProjectionError::UnsupportedLeaf { ty, .. }
        )) if *ty == TypeKind::CueHandle
    ));
}

#[test]
fn project_nominal_schema_reports_the_first_rejected_field_in_declaration_order() {
    let fixture = fixture(
        concat!(
            "struct Ordered { first: CueHandle, second: Stream<i64, String> }\n",
            "fn retain(value: Ordered) -> Ordered { value }\n",
        ),
        None,
    );
    let Err(FinalSemanticAnalysisError::NominalSchemaProjection(
        super::NominalSchemaProjectionError::UnsupportedLeaf { path, ty },
    )) = analyze(&fixture)
    else {
        panic!("the first non-persistent field must fail at the nominal schema owner")
    };
    assert_eq!(*ty, TypeKind::CueHandle);
    assert_eq!(
        path.steps(),
        &[super::NominalSchemaPathStep::Field {
            ordinal: 0,
            name: ModuleSegment::new("first").expect("field name"),
        }]
    );
}

#[test]
fn project_nominal_schema_classifies_variant_payloads_in_declaration_order() {
    let fixture = fixture(
        concat!(
            "enum RetainedChoice { Stable i64, Live CueHandle }\n",
            "fn retain(value: RetainedChoice) -> RetainedChoice { value }\n",
        ),
        None,
    );
    let Err(FinalSemanticAnalysisError::NominalSchemaProjection(
        super::NominalSchemaProjectionError::UnsupportedLeaf { path, ty },
    )) = analyze(&fixture)
    else {
        panic!("the affine variant payload must fail at the nominal schema owner")
    };
    assert_eq!(*ty, TypeKind::CueHandle);
    assert_eq!(
        path.steps(),
        &[super::NominalSchemaPathStep::VariantPayload {
            ordinal: 1,
            name: ModuleSegment::new("Live").expect("variant name"),
        }]
    );
}

#[test]
fn need_payload_project_nominal_requires_a_closed_snapshot_schema() {
    let fixture = fixture(
        concat!(
            "struct DeferredPayload { stream: Stream<i64, String> }\n",
            "fn retain(value: DeferredPayload) -> DeferredPayload { value }\n",
        ),
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::NominalSchemaProjection(
            super::NominalSchemaProjectionError::UnsupportedLeaf { ty, .. }
        )) if matches!(*ty, TypeKind::Stream { .. })
    ));
}

#[test]
fn public_ownership_nominal_edge_and_active_depth_limits_are_exact_and_fail_one_over() {
    let fixture = fixture(
        concat!(
            "struct StableRecord { value: i64 }\n",
            "fn retain(value: StableRecord) -> StableRecord { value }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("nominal work-limit fixture final analysis");
    let ty = project_nominal_expression_type(&report, "StableRecord");
    let exact = CheckedOwnershipLimits {
        max_nominal_edges: 1,
        max_active_nominal_depth: 1,
        ..CheckedOwnershipLimits::PRODUCTION
    };

    fixture
        .registered
        .checked_ownership(&report, &ty, exact)
        .expect("one project nominal exactly consumes one edge and active depth");
    assert_eq!(
        fixture.registered.checked_ownership(
            &report,
            &ty,
            CheckedOwnershipLimits {
                max_nominal_edges: 0,
                ..exact
            },
        ),
        Err(CheckedOwnershipError::WorkLimit)
    );
    assert_eq!(
        fixture.registered.checked_ownership(
            &report,
            &ty,
            CheckedOwnershipLimits {
                max_active_nominal_depth: 0,
                ..exact
            },
        ),
        Err(CheckedOwnershipError::WorkLimit)
    );
}

#[test]
fn public_ownership_evidence_row_limit_is_exact_and_fails_one_over() {
    let fixture = fixture("fn stable() {}", None);
    let report = analyze(&fixture).expect("evidence work-limit fixture final analysis");
    let image = fixture
        .registered
        .environment()
        .nominal_world()
        .nominal_catalog()
        .exact(&ownership_test_type_path("ImageHandle"))
        .expect("standard ImageHandle row")
        .try_instantiate([])
        .expect("zero-arity ImageHandle");
    let exact = CheckedOwnershipLimits {
        max_evidence_rows: 1,
        ..CheckedOwnershipLimits::PRODUCTION
    };

    fixture
        .registered
        .checked_ownership(&report, &image, exact)
        .expect("one consulted opaque row exactly consumes one evidence row");
    assert_eq!(
        fixture.registered.checked_ownership(
            &report,
            &image,
            CheckedOwnershipLimits {
                max_evidence_rows: 0,
                ..exact
            },
        ),
        Err(CheckedOwnershipError::WorkLimit)
    );
}

fn selected_application(call: &CallTargetFacts) -> &crate::callable::CheckedCallApplication {
    call.selected_application()
        .expect("fixture retains a selected call application")
}

fn selected_candidate(call: &CallTargetFacts) -> &Arc<crate::callable::ResolvedCallable> {
    selected_application(call).core().candidates().selected()
}

fn selected_candidates(call: &CallTargetFacts) -> &[Arc<crate::callable::ResolvedCallable>] {
    selected_application(call).core().candidates().candidates()
}

fn selected_execution_arguments(
    call: &CallTargetFacts,
) -> &[crate::callable::CheckedCallExecutionArgument] {
    selected_application(call).core().execution().arguments()
}

fn selected_call_owner(report: &FinalSemanticAnalysis) -> arcweft_lang_hir::identity::ExprId {
    report
        .calls()
        .find_map(|(owner, facts)| facts.selected_application().is_some().then_some(owner))
        .expect("fixture retains one selected call")
}

#[test]
fn selected_direct_call_derives_source_order_producer_admission_without_caller_rows() {
    let build = |source: &str| {
        let fixture = fixture(source, None);
        let report = analyze(&fixture).expect("selected producer call final analysis");
        let project = fixture.project.executable_view().expect("executable HIR");
        report
            .checked_need_producer_admission_for_call(
                project,
                &fixture.symbols,
                &fixture.registered,
                selected_call_owner(&report),
                CheckedOwnershipLimits::PRODUCTION,
            )
            .expect("direct selected call has an exact expression-backed argument inventory")
    };

    let source = concat!(
        "fn consume(number: i64, label: String) -> i64 { number }\n",
        "fn root() -> i64 { consume(1i64, \"stable\") }\n",
    );
    let first = build(source);
    let reallocated = build(source);
    let changed_type = build(concat!(
        "fn consume(number: i64, label: bool) -> i64 { number }\n",
        "fn root() -> i64 { consume(1i64, true) }\n",
    ));

    assert_eq!(first.arguments().len(), 2);
    assert_eq!(
        first.arguments()[0].disposition(),
        RetainedValueDisposition::Copy
    );
    assert_eq!(
        first.arguments()[1].disposition(),
        RetainedValueDisposition::SnapshotClone
    );
    assert_eq!(first.digest(), reallocated.digest());
    assert_ne!(first.digest(), changed_type.digest());
}

#[test]
fn producer_admission_fails_closed_for_need_and_argument_limit() {
    let fixture = fixture(
        concat!(
            "fn consume(value: Need<i64>) {}\n",
            "fn root(value: Need<i64>) { consume(value) }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("Need producer call final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let owner = selected_call_owner(&report);

    assert!(matches!(
        report.checked_need_producer_admission_for_call(
            project,
            &fixture.symbols,
            &fixture.registered,
            owner,
            CheckedOwnershipLimits::PRODUCTION,
        ),
        Err(CheckedNeedProducerAdmissionError::Ownership(
            CheckedOwnershipError::Rejected
        ))
    ));
    assert_eq!(
        report.checked_need_producer_admission_for_call(
            project,
            &fixture.symbols,
            &fixture.registered,
            owner,
            CheckedOwnershipLimits {
                max_producer_arguments: 0,
                ..CheckedOwnershipLimits::PRODUCTION
            },
        ),
        Err(CheckedNeedProducerAdmissionError::WorkLimit)
    );
}

#[test]
fn producer_admission_rejects_an_explicit_extension_receiver_capture() {
    let fixture = fixture(
        concat!(
            "fn normalize(self: String, suffix: String) -> String { self }\n",
            "fn dotted(value: String) -> String { value.normalize(\"!\") }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("extension receiver final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = report
        .calls()
        .find_map(|(owner, _)| {
            module
                .resolve_expr(owner)
                .ok()
                .is_some_and(|expression| {
                    matches!(
                        expression.kind(),
                        HirExprKind::Call(call)
                            if matches!(call.callee(), HirCallCallee::UnresolvedDot { .. })
                    )
                })
                .then_some(owner)
        })
        .expect("dotted extension call");
    assert_eq!(
        report.checked_need_producer_admission_for_call(
            project,
            &fixture.symbols,
            &fixture.registered,
            owner,
            CheckedOwnershipLimits::PRODUCTION,
        ),
        Err(CheckedNeedProducerAdmissionError::UnsupportedCapture)
    );
}

#[test]
fn producer_admission_rejects_compact_spread_slots() {
    let fixture = fixture(
        concat!(
            "fn add(left: i64, right: i64) -> i64 { left + right }\n",
            "fn root() -> i64 { add([1i64, 2i64]...) }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("compact spread final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    assert_eq!(
        report.checked_need_producer_admission_for_call(
            project,
            &fixture.symbols,
            &fixture.registered,
            selected_call_owner(&report),
            CheckedOwnershipLimits::PRODUCTION,
        ),
        Err(CheckedNeedProducerAdmissionError::UnsupportedArgumentInventory)
    );
}

#[test]
fn type_level_function_argument_fails_at_the_exact_call_projection() {
    let fixture = fixture(
        concat!(
            "fn consume(callback: i64 -> i64) {}\n",
            "fn root(callback: i64 -> i64) { consume(callback) }\n",
        ),
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::CallConstraintFailure(_))
    ));
}

#[test]
fn pending_observer_uses_the_standard_progress_field_owner() {
    let fixture = fixture(
        r"
fn observe(need: Need<i64>) -> i64 {
    await need with {
        pending progress => {
            let ratio = progress.ratio
            let label = progress.label
        }
    }
}
",
        None,
    );
    let report = analyze(&fixture).expect("Pending Progress field analysis");
    let mut fields = report.expressions().filter_map(|(_, expression)| {
        let CheckedExpressionResolution::Select(CheckedSelectResolution::ProgressField { field }) =
            expression.resolution()
        else {
            return None;
        };
        Some((*field, expression.ty().clone()))
    });
    assert_eq!(
        fields.next(),
        Some((crate::types::ProgressField::Ratio, TypeKind::F32))
    );
    assert_eq!(
        fields.next(),
        Some((
            crate::types::ProgressField::Label,
            TypeKind::Option(Box::new(TypeKind::String)),
        ))
    );
    assert_eq!(fields.next(), None);
}

#[test]
fn prefix_try_uses_the_checked_implicit_callable_as_its_propagation_boundary() {
    let carrier = TypeKind::Result {
        ok: Box::new(TypeKind::I64),
        error: Box::new(TypeKind::String),
    };
    let callback = TypeKind::function_with_effects(
        [carrier.clone()],
        carrier,
        EffectRow::closed(EffectSet::new()),
    );
    let fixture = typed_overload_fixture(
        "fn caller() { choose(try _); }\n",
        "choose",
        vec![TestCallableOverload::strict(
            [callback.clone()],
            TypeKind::Unit,
        )],
    );
    let report = analyze(&fixture).expect("Try implicit-callable analysis");
    let (_, callable) = report
        .expressions()
        .find(|(_, expression)| {
            matches!(
                expression.resolution(),
                CheckedExpressionResolution::ImplicitCallable(_)
            )
        })
        .expect("checked implicit callable");
    assert_eq!(callable.ty(), &callback);
    let CheckedExpressionResolution::ImplicitCallable(callable) = callable.resolution() else {
        unreachable!("matched above")
    };
    assert!(matches!(
        callable.body_resolution(),
        CheckedExpressionResolution::Try(tried)
            if matches!(tried.boundary(), CheckedTryBoundary::FunctionSite(_))
    ));
}

#[test]
fn pipe_left_uses_one_checked_pipe_owner_without_creating_a_callable() {
    let fixture = fixture(
        r"
fn pipeline(input: Result<i64, String>) -> Result<i64, String> {
    Ok(input |> try ^)
}
",
        None,
    );
    let report = analyze(&fixture).expect("checked pipe Try analysis");
    let (owner, pipe) = report
        .expressions()
        .find_map(|(owner, expression)| match expression.resolution() {
            CheckedExpressionResolution::Pipe(pipe) => Some((owner, pipe)),
            _ => None,
        })
        .expect("checked pipe fact");
    assert_eq!(pipe.placeholders().len(), 1);
    assert!(matches!(
        report
            .expression(pipe.placeholders()[0])
            .expect("pipe-left placeholder fact")
            .resolution(),
        CheckedExpressionResolution::PipeLeft { pipe } if *pipe == owner
    ));
    assert!(!report.expressions().any(|(_, expression)| matches!(
        expression.resolution(),
        CheckedExpressionResolution::ImplicitCallable(_)
    )));
}

#[test]
fn drop_policy_overload_is_checked_for_free_pipe_and_dot_surfaces() {
    let fixture = fixture(
        r"
fn dispose(value: i64) {
    drop(value);
    drop(stop_now)(value);
    value |> drop(stop_now);
    value.drop(stop_now);
    let retained = on_drop(stop_now)(value);
    retained;
}
",
        None,
    );
    let report = analyze(&fixture).expect("typed drop policy overload analysis");
    let drops = report
        .statements()
        .filter_map(|(_, statement)| match statement.role() {
            CheckedStatementRole::EvaluatedEffect(effect) => match effect.as_ref() {
                CheckedEvaluatedEffect::Drop {
                    operation,
                    policy_source,
                    policy,
                    ..
                } => Some((*operation, *policy_source, policy)),
                _ => None,
            },
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(drops.len(), 4);
    assert!(matches!(
        drops[0],
        (DropCallableId::Drop, None, CheckedDropPolicy::Default)
    ));
    for (operation, policy_source, policy) in &drops[1..] {
        assert_eq!(*operation, DropCallableId::DropWithPolicy);
        assert!(policy_source.is_some());
        assert!(matches!(
            policy,
            CheckedDropPolicy::Stop {
                fade: CheckedDropFade::ConstantNanos(0)
            }
        ));
    }
}

#[test]
fn carrier_blocks_are_the_nearest_checked_try_boundaries() {
    let fixture = fixture(
        r"
fn retain_result(input: Result<i64, String>) -> Result<i64, String> {
    result {
        let value = try input
        value
    }
}

fn retain_option(input: Option<i64>) -> Option<i64> {
    option {
        let value = try input
        value
    }
}
",
        None,
    );
    let report = analyze(&fixture).expect("carrier block Try analysis");
    let boundaries = report
        .expressions()
        .filter_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::Try(tried) => Some(tried.boundary()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(boundaries.len(), 2);
    assert!(
        boundaries
            .iter()
            .all(|boundary| matches!(boundary, CheckedTryBoundary::CarrierBlock(_)))
    );
    assert!(report.expressions().any(|(_, expression)| {
        matches!(
            expression.ty(),
            TypeKind::Result { ok, error }
                if **ok == TypeKind::I64 && **error == TypeKind::String
        )
    }));
    assert!(report.expressions().any(|(_, expression)| {
        matches!(expression.ty(), TypeKind::Option(item) if **item == TypeKind::I64)
    }));
}

#[test]
fn option_block_wraps_its_tail_without_constructing_a_need() {
    let fixture = fixture(
        r"
fn selected() -> Option<i64> {
    option { 7i64 }
}
",
        None,
    );
    let report = analyze(&fixture).expect("Option carrier block final analysis");
    assert!(report.expressions().any(|(_, expression)| {
        matches!(
            expression.ty(),
            TypeKind::Option(value) if **value == TypeKind::I64
        )
    }));
    assert!(
        !report
            .expressions()
            .any(|(_, expression)| matches!(expression.ty(), TypeKind::Need(_)))
    );
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
    let (_, catalog) = checked_callables(&fixture, &input);
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
fn checked_catalog_closure_rows_use_project_callee_exposed_effect_contract() {
    let fixture = fixture(
        r#"
fn bounded() -> i64 effects { fs.read } {
    1i64
}

fn root() {
    let callback = || bounded();
    ()
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("project-call closure effect analysis");
    let executable = fixture.project.executable_view().expect("executable HIR");
    let (_, module) = executable.modules().next().expect("root module");
    let closure_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Closure(_)).then_some(owner)
        })
        .expect("closure expression");
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

    assert_eq!(
        report
            .checked_callables()
            .closure_at_source(closure_source)
            .expect("source-indexed checked closure row")
            .concrete()
            .to_labels(),
        ["fs.read"]
    );
}

#[test]
fn incomplete_or_duplicate_fact_sets_never_publish() {
    let fixture = fixture("fn root() {}\n", None);
    let executable = fixture.project.executable_view().expect("executable HIR");
    let mut missing = complete_input(&fixture);
    missing.expressions.pop();
    let (missing_topology, missing_catalog) = checked_callables(&fixture, &missing);
    assert!(matches!(
        FinalSemanticAnalysis::try_new(
            executable,
            &fixture.symbols,
            missing_topology,
            missing_catalog,
            missing
        ),
        Err(FinalSemanticAnalysisError::MissingFact { .. })
    ));

    let mut duplicate = complete_input(&fixture);
    let expression = duplicate.expressions[0].clone();
    duplicate.expressions.push(expression);
    let (duplicate_topology, duplicate_catalog) = checked_callables(&fixture, &duplicate);
    assert!(matches!(
        FinalSemanticAnalysis::try_new(
            executable,
            &fixture.symbols,
            duplicate_topology,
            duplicate_catalog,
            duplicate
        ),
        Err(FinalSemanticAnalysisError::DuplicateFact { .. })
    ));
}

#[test]
fn cancellation_is_terminal_before_any_report_is_observable() {
    let fixture = fixture("fn root() {}\n", None);
    let cancellation = AtomicBool::new(true);
    let input = complete_input(&fixture);
    let (topology, checked_callables) = checked_callables(&fixture, &input);
    let result = FinalSemanticAnalysis::try_new_with_control(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        topology,
        checked_callables,
        input,
        FinalSemanticAnalysisControl::new(&cancellation),
    );
    assert!(matches!(result, Err(FinalSemanticAnalysisError::Cancelled)));
}

#[test]
fn every_call_expression_requires_one_sealed_shared_resolver_fact() {
    let fixture = fixture("fn target() {}\nfn caller() { target(); }\n", None);
    let call_owner = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .flat_map(|(_, module)| module.expressions())
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(owner)
        })
        .expect("fixture call owner");
    let input = complete_input(&fixture);
    let (topology, checked_callables) = checked_callables(&fixture, &input);
    let result = FinalSemanticAnalysis::try_new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        topology,
        checked_callables,
        input,
    );
    let Err(FinalSemanticAnalysisError::CallSeal(failure)) = result else {
        panic!("missing sealed resolver graph result: {result:?}");
    };
    assert_eq!(
        failure.location(),
        FinalCallSealLocation::Site(crate::callable::CheckedCallSite::HirCall(call_owner))
    );
    assert_eq!(
        failure.typed_failure_for_test(),
        &CallConstraintInvariant::MissingOrStalePreparedNode
    );
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
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);
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
fn checked_callable_join_uses_the_current_catalog_row_and_digest() {
    let fixture = fixture("fn target() {}\nfn caller() { target(); }\n", None);
    let report = analyze(&fixture).expect("project call final analysis");
    let (owner, _) = report.calls().next().expect("one call fact");
    let join = report
        .checked_callable_join(owner)
        .expect("selected project callable joins the current catalog");
    let id = join
        .checked_id()
        .expect("project call has a checked callable ID");
    assert_eq!(join.digest(), Some(id.semantic_digest()));
    assert_ne!(join.semantic_digest().as_bytes(), &[0; 32]);
}

#[test]
fn intrinsic_callable_join_keeps_typed_candidate_authority_without_a_catalog_row() {
    let fixture = fixture("fn caller() { String.with_capacity(8); }\n", None);
    let report = analyze(&fixture).expect("typed intrinsic call final analysis");
    let (owner, _) = report.calls().next().expect("one call fact");
    let join = report
        .checked_callable_join(owner)
        .expect("typed intrinsic joins without fabricating a checked catalog row");
    assert!(join.checked_id().is_none());
    assert!(join.digest().is_none());
    assert_ne!(join.semantic_digest().as_bytes(), &[0; 32]);
}

#[test]
fn checked_callable_join_rejects_missing_call_evidence() {
    let fixture = fixture("fn root() {}\n", None);
    let report = analyze(&fixture).expect("root final analysis");
    let owner = report
        .expressions()
        .next()
        .map(|(owner, _)| owner)
        .expect("root expression fact");
    assert_eq!(
        report.checked_callable_join(owner),
        Err(super::CheckedExpressionEdgeError::Callable(
            super::CheckedCallableJoinError::NotSelected,
        ))
    );
}

#[test]
fn checked_child_edges_preserve_hir_order_and_role_ordinals() {
    let fixture = fixture("fn root() -> (i64, i64) { (1i64, 2i64) }\n", None);
    let report = analyze(&fixture).expect("tuple final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, expression) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Tuple(_)))
        .expect("tuple expression");
    let expected = expression.kind().direct_expression_children();
    let checked = report
        .checked_child_edges(owner)
        .expect("tuple children have checked facts");
    assert_eq!(
        checked.iter().map(|(child, _)| *child).collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        checked
            .iter()
            .map(|(_, role)| role.semantic_tag())
            .collect::<Vec<_>>(),
        [0x1000, 0x1000]
    );
}

#[test]
fn checked_record_fields_use_declaration_ordinals_not_authored_order() {
    let fixture = fixture(
        concat!(
            "struct Pair { first: i64, second: bool }\n",
            "fn root() -> Pair { Pair { second = true, first = 1i64 } }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("record literal final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, expression) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Record(_)))
        .expect("record expression");
    let expected = expression.kind().direct_expression_children();
    let edges = report
        .checked_child_edges(owner)
        .expect("record fields have checked evidence");
    let edge_fact = report
        .checked_expression_edge_fact(owner)
        .expect("record field fact");
    assert_eq!(
        edges.iter().map(|(child, _)| *child).collect::<Vec<_>>(),
        expected
    );
    let accepted_ordinals = edges
        .iter()
        .map(|(_, role)| match role {
            CheckedExpressionChildRole::RecordField {
                source_ordinal,
                accepted_field,
            } => {
                let field = edge_fact
                    .record_fields()
                    .iter()
                    .find(|field| field.source_ordinal() == *source_ordinal)
                    .expect("accepted record field row");
                assert_eq!(*accepted_field, field.semantic_id());
                (*source_ordinal, field.declaration_ordinal())
            }
            other => panic!("unexpected record child role: {other:?}"),
        })
        .collect::<Vec<_>>();
    assert_eq!(accepted_ordinals, [(0, 1), (1, 0)]);
}

fn checked_match_reference(
    report: &FinalSemanticAnalysis,
    module: &HirModule,
    symbols: &ProjectSymbolTable,
    owner: arcweft_lang_hir::identity::ExprId,
) -> super::CheckedMatchRef {
    report
        .checked_match_ref(module, symbols, owner)
        .expect("Match reference belongs to the exact accepted module snapshot")
}

#[test]
fn checked_match_reference_rejects_a_foreign_snapshot_before_transcription() {
    let source = concat!(
        "fn root(flag: bool) -> i64 {\n",
        "    match flag {\n",
        "        true => 1i64\n",
        "        false => 2i64\n",
        "    }\n",
        "}\n",
    );
    let fixture = fixture(source, None);
    let report = analyze(&fixture).expect("checked Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Match expression");
    let foreign = self::fixture(source, None);
    let foreign_project = foreign.project.executable_view().expect("foreign HIR");
    let foreign_snapshot = foreign_project
        .module(&CanonicalModulePath::crate_root())
        .expect("foreign root HIR module")
        .snapshot_id();
    let stale = super::CheckedMatchRef::new(foreign_snapshot, owner);

    assert_eq!(
        report.build_checked_match_for_ref(
            project,
            &fixture.symbols,
            stale,
            CheckedMatchLimits::PRODUCTION,
        ),
        Err(SemanticTranscriptError::StaleMatchReference)
    );
}

#[test]
fn checked_match_fact_and_edges_retain_exact_guard_presence_and_children() {
    let fixture = fixture(
        concat!(
            "fn root(flag: bool) -> i64 {\n",
            "    match flag {\n",
            "        true when true => 1i64\n",
            "        _ => 2i64\n",
            "    }\n",
            "}\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("ordinary Match final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, expression) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Match(_)))
        .expect("ordinary Match expression");
    let HirExprKind::Match(authored) = expression.kind() else {
        unreachable!("filtered Match expression")
    };
    let checked = report.expression(owner).expect("checked Match owner");
    let match_fact = checked.match_fact().expect("checked Match fact");
    assert_eq!(match_fact.scrutinee(), authored.scrutinee());
    assert_eq!(match_fact.arms().len(), authored.arms().len());
    for (authored, accepted) in authored.arms().iter().zip(match_fact.arms()) {
        assert_eq!(accepted.guard(), authored.guard());
        assert_eq!(accepted.value(), authored.value());
    }
    let edges = report
        .checked_child_edges(owner)
        .expect("Match child edges have complete checked evidence");
    assert_eq!(
        edges.iter().map(|(child, _)| *child).collect::<Vec<_>>(),
        expression.kind().direct_expression_children()
    );
    assert!(
        edges
            .iter()
            .any(|(_, role)| matches!(role, CheckedExpressionChildRole::Guard { arm: 0 }))
    );
    assert!(
        edges
            .iter()
            .any(|(_, role)| matches!(role, CheckedExpressionChildRole::ArmValue { arm: 1 }))
    );
    let product = report
        .build_checked_match_for_ref(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            super::CheckedMatchLimits::PRODUCTION,
        )
        .expect("generic Match semantic product");
    assert_eq!(product.arms().len(), 2);
    assert!(product.coverage().exhaustive());
    assert_ne!(product.semantic_digest().as_bytes(), &[0; 32]);
}

#[test]
fn checked_match_semantic_path_crosses_the_typed_statement_root() {
    let fixture = fixture(
        concat!(
            "fn root(flag: bool) -> i64 {\n",
            "    let selected = match flag {\n",
            "        true => 1i64\n",
            "        false => 2i64\n",
            "    }\n",
            "    selected\n",
            "}\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("statement-root Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("statement initializer Match");
    let product = report
        .build_checked_match_for_ref(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            super::CheckedMatchLimits::PRODUCTION,
        )
        .expect("statement-origin path is HIR-owned and semantically enriched");
    assert!(product.coverage().exhaustive());
    assert_eq!(product.arms().len(), 2);
}

#[test]
fn checked_match_transcript_retains_stable_option_binding_rows() {
    let fixture = fixture(
        r#"
fn root(value: Option<i64>) -> i64 {
    match value {
        .Some(item) => item
        .None => 0i64
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("Option Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("Option Match expression");
    let product = report
        .build_checked_match_for_ref(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("Option Match semantic product");
    assert!(product.coverage().exhaustive());
    assert!(product.coverage().unreachable().is_empty());
    let some = product.arms().first().expect("Some arm");
    assert_eq!(some.bindings().len(), 1);
    assert_ne!(some.bindings()[0].ty().as_bytes(), &[0; 32]);
    assert!(matches!(
        some.bindings()[0].coordinate(),
        StableCheckedValueCoordinate::Binding(_)
    ));
}

#[test]
fn checked_match_transcript_rejects_non_exhaustive_and_enforces_limits() {
    let fixture = fixture(
        "fn root(flag: bool) -> i64 { match flag { true => 1i64 } }\n",
        None,
    );
    let report = analyze(&fixture).expect("non-exhaustive Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("non-exhaustive Match expression");
    let non_exhaustive = report.build_checked_match_for_ref(
        project,
        &fixture.symbols,
        checked_match_reference(&report, module, &fixture.symbols, owner),
        CheckedMatchLimits::PRODUCTION,
    );
    assert!(matches!(
        non_exhaustive,
        Err(SemanticTranscriptError::NonExhaustive {
            witness: CheckedCoverageWitness::Bool(false)
        })
    ));

    let byte_limited = report.build_checked_match_for_ref(
        project,
        &fixture.symbols,
        checked_match_reference(&report, module, &fixture.symbols, owner),
        CheckedMatchLimits::PRODUCTION.with_limit(CheckedMatchLimitKind::TranscriptBytes, 0),
    );
    assert!(matches!(
        byte_limited,
        Err(SemanticTranscriptError::MatchBuild(
            CheckedMatchBuildError::LimitExceeded {
                kind: CheckedMatchLimitKind::TranscriptBytes,
                limit: 0,
                ..
            }
        ))
    ));
}

#[test]
fn checked_match_coverage_reports_guarded_rows_redundant_after_prior_coverage() {
    let fixture = fixture(
        r#"
fn root(flag: bool, ready: bool) -> i64 {
    match flag {
        true => 1i64
        true when ready => 2i64
        false => 3i64
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("guarded Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("guarded Match expression");
    let product = report
        .build_checked_match_for_ref(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("guarded Match semantic product");
    assert!(product.coverage().exhaustive());
    let unreachable = product.coverage().unreachable();
    assert_eq!(unreachable.len(), 1);
    assert_eq!(unreachable[0].arm().ordinal(), 1);
    assert_eq!(
        unreachable[0].reason(),
        CheckedUnreachableReason::CoveredByPriorUsefulArms
    );
}

#[test]
fn declaration_paths_retain_nested_thread_body_coordinates() {
    let fixture = fixture(
        r#"
flow main(flag: bool) {
    thread {
        if flag {
            match flag {
                true => {}
                false => {}
            }
        }
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("nested Thread path final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::Flow)
        .expect("root flow callable")
        .declaration();
    let topology = project
        .accept_symbol_generation(&fixture.symbols)
        .expect("accepted HIR symbol generation")
        .into_evaluation_topology()
        .expect("project evaluation topology");
    let paths = topology
        .declaration_semantic_paths(declaration)
        .expect("nested Thread semantic paths");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_scrutinee = module
        .statements()
        .find_map(|(_, statement)| match statement.kind() {
            HirStmtKind::Match(matched) => Some(matched.scrutinee()),
            _ => None,
        })
        .expect("Thread Match scrutinee path expression");
    let path = paths
        .expression(match_scrutinee)
        .expect("Thread Match scrutinee semantic path");
    assert!(
        path.steps()
            .iter()
            .any(|step| matches!(step, HirSemanticPathStep::ThreadBody(_)))
    );
    assert!(path.steps().iter().any(|step| {
        matches!(
            step,
            HirSemanticPathStep::Body(
                arcweft_lang_hir::body_edges::HirBodyChildRole::ThreadItem { .. }
            )
        )
    }));
    assert!(
        report
            .expressions()
            .any(|(owner, _)| paths.expression(owner).is_some())
    );
}

#[test]
fn semantic_coordinate_index_issues_only_from_the_exact_declaration_local_path() {
    let fixture = fixture(
        r#"
flow root {
    let value: i64 = 1i64
}
flow other {}
"#,
        None,
    );
    let report = analyze(&fixture).expect("coordinate edge authority");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let local = module
        .locals()
        .map(|(owner, _)| owner)
        .find(|owner| {
            report
                .accepted_root_catalog()
                .semantic_path((*owner).into())
                .expect("sealed local lookup")
                .is_some()
        })
        .expect("root binding local");
    let index = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let statement = module
        .statements()
        .map(|(owner, _)| owner)
        .find(|owner| {
            report
                .accepted_root_catalog()
                .semantic_path((*owner).into())
                .expect("sealed statement lookup")
                .is_some()
        })
        .expect("root statement");
    let pattern = module
        .patterns()
        .map(|(owner, _)| owner)
        .find(|owner| {
            report
                .accepted_root_catalog()
                .semantic_path((*owner).into())
                .expect("sealed pattern lookup")
                .is_some()
        })
        .expect("root pattern");
    let statement_evidence = index
        .statement_evidence(statement)
        .expect("stable statement coordinate evidence");
    assert_eq!(statement_evidence.owner(), statement);
    let statement_coordinate = statement_evidence.into_coordinate();
    assert_eq!(
        statement_coordinate.canonical_bytes().unwrap(),
        statement_coordinate.path().canonical_bytes().unwrap()
    );
    let pattern_evidence = index
        .pattern_evidence(pattern)
        .expect("stable pattern coordinate evidence");
    assert_eq!(pattern_evidence.owner(), pattern);
    let pattern_coordinate = pattern_evidence.into_coordinate();
    assert_eq!(
        pattern_coordinate.canonical_bytes().unwrap(),
        pattern_coordinate.path().canonical_bytes().unwrap()
    );
    let binding = index
        .binding_evidence(local)
        .expect("stable binding coordinate evidence");
    assert_eq!(binding.owner(), local);
    let binding = binding.into_coordinate();
    assert_eq!(
        binding.canonical_bytes().unwrap(),
        binding.path().canonical_bytes().unwrap()
    );
    assert!(binding.path().steps().iter().all(|step| !matches!(
        step,
        crate::semantic_coordinate::CheckedSemanticPathStep::Expression(_)
    )));

    let foreign_fixture =
        crate::final_analysis::tests::fixture("fn foreign() -> i64 { 0i64 }\n", None);
    let foreign_project = foreign_fixture
        .project
        .executable_view()
        .expect("foreign executable HIR");
    let foreign_owner = foreign_project
        .module(&CanonicalModulePath::crate_root())
        .expect("foreign root module")
        .expressions()
        .next()
        .expect("foreign expression")
        .0;
    assert_eq!(
        index.expression(foreign_owner),
        Err(
            crate::semantic_coordinate::SemanticCoordinateIndexError::MissingOwner {
                owner: foreign_owner.into()
            }
        )
    );
}

#[test]
fn semantic_coordinate_index_resolves_expression_hops_from_checked_edges() {
    let fixture = fixture("fn root() -> i64 { 1i64 + 2i64 }\n", None);
    let report = analyze(&fixture).expect("expression edge authority");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root module");
    let owner = module
        .expressions()
        .map(|(owner, _)| owner)
        .find(|owner| {
            report
                .accepted_root_catalog()
                .semantic_path((*owner).into())
                .expect("sealed expression lookup")
                .is_some_and(|location| {
                    location
                        .path()
                        .steps()
                        .iter()
                        .any(|step| matches!(step, HirSemanticPathStep::Expression(_)))
                })
        })
        .expect("expression reached through a checked hop");
    let index = SemanticCoordinateIndex::new(report.accepted_root_catalog(), &report);
    let checked = index
        .expression_evidence(owner)
        .expect("checked expression hop coordinate evidence");
    assert_eq!(checked.owner(), owner);
    let checked = checked.into_coordinate();
    assert!(checked.steps().iter().any(|step| {
        matches!(
            step,
            crate::semantic_coordinate::CheckedSemanticPathStep::Expression(_)
        )
    }));

    let matching = CheckedCallExecutionSource::seal(
        CheckedCallArgumentSlotSource::Expression(owner),
        index
            .expression_evidence(owner)
            .expect("matching expression coordinate evidence"),
    )
    .expect("execution source consumes exact owner evidence");
    assert_eq!(
        matching.coordinate(),
        &StableCheckedValueCoordinate::Expression(checked)
    );

    let other = module
        .expressions()
        .map(|(candidate, _)| candidate)
        .find(|candidate| *candidate != owner)
        .expect("distinct expression owner");
    assert_eq!(
        CheckedCallExecutionSource::seal(
            CheckedCallArgumentSlotSource::Expression(other),
            index
                .expression_evidence(owner)
                .expect("mismatched expression coordinate evidence"),
        ),
        Err(CallConstraintInvariant::PreparedCallSiteMismatch)
    );
}

#[test]
fn checked_match_project_enum_consumes_layout_free_semantic_cases() {
    let fixture = fixture(
        r#"
enum Route {
    Opening,
    Closing,
}

fn root(route: Route) -> i64 {
    match route {
        .Opening => 1i64
        .Closing => 2i64
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("project enum Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, authored_match) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Match(authored_match) = expression.kind() else {
                return None;
            };
            Some((owner, authored_match))
        })
        .expect("project enum Match expression");
    let semantic_type = report
        .expression(authored_match.scrutinee())
        .expect("checked enum scrutinee")
        .ty()
        .semantic_identity_digest();
    let definition = report
        .project_nominal_semantic(semantic_type)
        .expect("layout-free project nominal semantics");
    let cases = definition.cases().expect("project enum semantic cases");
    assert_eq!(cases.len(), 2);
    for (ordinal, arm) in authored_match.arms().iter().enumerate() {
        let checked = report.pattern(arm.pattern()).expect("checked enum pattern");
        let CheckedPatternResolution::Variant(resolution) = checked.resolution() else {
            panic!("enum arm must retain a checked variant case");
        };
        assert_eq!(
            resolution.selected().semantic_id(),
            cases[ordinal].semantic_id()
        );
    }
    let product = report
        .build_checked_match_for_ref(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("project enum Match semantic product");
    assert!(product.coverage().exhaustive());
    assert_ne!(product.coverage().domain_digest().as_bytes(), &[0; 32]);
    assert_ne!(
        product.arms()[0].pattern().as_bytes(),
        product.arms()[1].pattern().as_bytes()
    );
}

#[test]
fn checked_match_transcript_changes_when_source_arm_order_changes() {
    let build = |source: &str| {
        let fixture = fixture(source, None);
        let report = analyze(&fixture).expect("ordered Match final analysis");
        let project = fixture.project.executable_view().expect("executable HIR");
        let module = project
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("Match expression");
        let product = report
            .build_checked_match_for_ref(
                project,
                &fixture.symbols,
                checked_match_reference(&report, module, &fixture.symbols, owner),
                CheckedMatchLimits::PRODUCTION,
            )
            .expect("ordered Match semantic product");
        *product.semantic_digest().as_bytes()
    };

    let first = build(
        r#"
fn root(flag: bool) -> i64 {
    match flag {
        true => 1i64
        false => 2i64
    }
}
"#,
    );
    let reordered = build(
        r#"
fn root(flag: bool) -> i64 {
    match flag {
        false => 2i64
        true => 1i64
    }
}
"#,
    );
    assert_ne!(first, reordered);
}

#[test]
fn checked_match_transcript_commits_checked_callable_contract() {
    let build = |effect: &str| {
        let source = format!(
            r#"
fn callee() -> i64 effects {{ {effect} }} {{
    1i64
}}

fn root(flag: bool) -> i64 {{
    match flag {{
        true => callee()
        false => 0i64
    }}
}}
"#
        );
        let fixture = fixture(&source, None);
        let report = analyze(&fixture).expect("call-contract Match final analysis");
        let project = fixture.project.executable_view().expect("executable HIR");
        let module = project
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        let owner = module
            .expressions()
            .find_map(|(owner, expression)| {
                matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
            })
            .expect("Match expression");
        let product = report
            .build_checked_match_for_ref(
                project,
                &fixture.symbols,
                checked_match_reference(&report, module, &fixture.symbols, owner),
                CheckedMatchLimits::PRODUCTION,
            )
            .expect("call-contract Match semantic product");
        *product.semantic_digest().as_bytes()
    };

    assert_ne!(build("fs.read"), build("fs.write"));
}

#[test]
fn checked_match_transcript_accepts_nested_product_coverage() {
    let fixture = fixture(
        r#"
fn root(pair: (bool, bool)) -> i64 {
    match pair {
        (true, true) => 1i64
        _ => 0i64
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("tuple Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("tuple Match expression");
    let product = report
        .build_checked_match_for_ref(
            project,
            &fixture.symbols,
            checked_match_reference(&report, module, &fixture.symbols, owner),
            CheckedMatchLimits::PRODUCTION,
        )
        .expect("tuple coverage uses the generic product matrix");
    assert!(product.coverage().exhaustive());
}

#[test]
fn declaration_paths_retain_for_body_and_closure_match_coordinates() {
    let fixture = fixture(
        r#"
flow root {
    let values: Vec<bool> = [true, false]
    for value in values {
        let handler = |item: bool| -> i64 {
            match item {
                true => 1i64
                false => 2i64
            }
        }
    }
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("For/closure Match final analysis");
    let project = fixture.project.executable_view().expect("executable HIR");
    let declaration = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "root")
        .expect("root flow callable")
        .declaration();
    let topology = project
        .accept_symbol_generation(&fixture.symbols)
        .expect("accepted HIR symbol generation")
        .into_evaluation_topology()
        .expect("project evaluation topology");
    let paths = topology
        .declaration_semantic_paths(declaration)
        .expect("For/closure semantic paths");
    let module = project
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let match_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Match(_)).then_some(owner)
        })
        .expect("closure Match expression");
    let path = paths
        .expression(match_owner)
        .expect("closure Match semantic path");
    assert!(path.steps().iter().any(|step| {
        matches!(
            step,
            HirSemanticPathStep::ThreadBody(arcweft_lang_hir::stmt::HirStatementBodyRole::For)
        )
    }));
    assert!(path.steps().iter().any(|step| {
        matches!(
            step,
            HirSemanticPathStep::Expression(
                arcweft_lang_hir::expr::HirExpressionChildRole::ClosureBody
            )
        )
    }));
    assert!(report.expression(match_owner).is_some());
}

#[test]
fn checked_choice_path_evidence_is_published_with_hir_child_order() {
    let fixture = fixture(
        r#"
flow main {
    choice @.first {
        @.next "Next" -> @flow.done
    }
}

flow done() -> String {
    return "done"
}
"#,
        None,
    );
    let report = analyze(&fixture).expect("Choice path final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, expression) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Choice(_)))
        .expect("Choice expression");
    let checked = report.expression(owner).expect("checked Choice owner");
    assert!(checked.nested_path_evidence().is_some_and(Result::is_ok));
    let edges = report
        .checked_child_edges(owner)
        .expect("Choice nested path edges have accepted evidence");
    assert_eq!(
        edges.iter().map(|(child, _)| *child).collect::<Vec<_>>(),
        expression.kind().direct_expression_children()
    );
}

#[test]
fn missing_checker_owned_choice_path_evidence_rejects_only_the_edge_fact() {
    let fixture = fixture(
        r#"
flow main {
    choice @.first {
        @.next "Next" -> @flow.done
    }
}

flow done() -> String {
    return "done"
}
"#,
        None,
    );
    let accepted = analyze(&fixture).expect("Choice path final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, _) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Choice(_)))
        .expect("Choice expression");
    let mut input = input_from_report(&accepted);
    let (_, checked) = input
        .expressions
        .iter_mut()
        .find(|(candidate, _)| *candidate == owner)
        .expect("checked Choice input fact");
    let complete = checked.complete().expect("complete Choice fixture");
    *checked = CheckedExpression::new(
        complete.ty().clone(),
        complete.type_selection(),
        complete.effects().clone(),
        complete.resolution().clone(),
    )
    .into();
    let report = FinalSemanticAnalysis::try_new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        Arc::clone(accepted.hir_topology()),
        accepted.checked_callables().clone(),
        input,
    )
    .expect("missing nested evidence remains a recoverable owner fact");
    assert_eq!(
        report.checked_child_edges(owner),
        Err(super::CheckedExpressionEdgeError::Child(
            super::CheckedChildEdgeError::MissingNestedPath,
        ))
    );
    assert!(report.checked_expression_edge_fact(owner).is_err());
    assert_eq!(
        report.checked_callable_join(owner),
        Err(super::CheckedExpressionEdgeError::Child(
            super::CheckedChildEdgeError::MissingNestedPath,
        ))
    );
}

#[test]
fn checker_nested_path_error_is_retained_as_the_publication_edge_error() {
    let fixture = fixture(
        r#"
flow main {
    choice @.first {
        @.next "Next" -> @flow.done
    }
}

flow done() -> String {
    return "done"
}
"#,
        None,
    );
    let accepted = analyze(&fixture).expect("Choice path final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, _) = module
        .expressions()
        .find(|(_, expression)| matches!(expression.kind(), HirExprKind::Choice(_)))
        .expect("Choice expression");
    let mut input = input_from_report(&accepted);
    let (_, checked) = input
        .expressions
        .iter_mut()
        .find(|(candidate, _)| *candidate == owner)
        .expect("checked Choice input fact");
    let complete = checked.complete().expect("complete Choice fixture");
    *checked = CheckedExpression::new(
        complete.ty().clone(),
        complete.type_selection(),
        complete.effects().clone(),
        complete.resolution().clone(),
    )
    .with_nested_path_evidence(Err(super::CheckedChildEdgeError::StaleNestedPath))
    .into();
    let report = FinalSemanticAnalysis::try_new(
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        Arc::clone(accepted.hir_topology()),
        accepted.checked_callables().clone(),
        input,
    )
    .expect("checker error remains a recoverable owner fact");
    assert_eq!(
        report.checked_child_edges(owner),
        Err(super::CheckedExpressionEdgeError::Child(
            super::CheckedChildEdgeError::StaleNestedPath,
        ))
    );
}

#[test]
fn production_analyzer_routes_capacity_through_typed_associated_authority() {
    let fixture = fixture("fn caller() { String.with_capacity(8); }\n", None);
    let report = analyze(&fixture).expect("typed Capacity final analysis");
    let calls = report.calls().collect::<Vec<_>>();
    assert_eq!(calls.len(), 1);
    let (_, call) = calls[0];
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);
    assert!(matches!(
        selected.id(),
        CallableCandidateId::CapacityMethod(_)
    ));
    assert_eq!(considered.len(), 1);
    assert_eq!(selected_application(call).result().ty(), &TypeKind::String);
    assert_eq!(selected_execution_arguments(call).len(), 1);
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
    let selected = selected_candidate(call);
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
        assert!(matches!(
            signature.origin(),
            crate::callable::SignatureOrigin::Language { .. }
        ));
        assert_eq!(signature.result(), selected.schema().result());
        assert_eq!(
            signature.effects(),
            selected
                .schema()
                .effects()
                .fixed_row()
                .expect("Capacity has fixed effects")
        );
        assert_eq!(signature.poison(), CallPoison::Clean);
        assert_eq!(signature.groups().len(), selected.schema().groups().len());
        assert_eq!(
            selected.schema().argument_policy(),
            CallableArgumentPolicy::new(
                UnknownNamedArgumentPolicy::OpenSupply,
                SpreadArgumentPolicy::Unchecked,
            )
        );
        let [group] = signature.groups() else {
            panic!("one Capacity group")
        };
        let [parameter] = group.parameters() else {
            panic!("one unchecked rest parameter")
        };
        assert!(parameter.admission().is_unchecked());
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
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);
    assert_eq!(considered.len(), 1);
    assert_eq!(
        selected.id(),
        &CallableCandidateId::Presentation(PresentationCallableId::Show)
    );
    let crate::callable::ResolvedCallableBaseInstantiation::Character { owner } =
        selected.instantiation()
    else {
        panic!("Character presentation specialization must precede candidate preparation")
    };
    assert_eq!(owner.character().as_str(), "character.akane");
    let expected_look =
        TypeKind::character_look(CharacterId::try_new("character.akane").expect("Character ID"));
    let look = report
        .expressions()
        .find_map(|(_, expression)| match expression.resolution() {
            CheckedExpressionResolution::StageLook(look) => Some(look),
            _ => None,
        })
        .expect("accepted manifest-joined Character look fact");
    assert_eq!(
        CheckedExpressionResolution::StageLook(look.clone()).semantic_tag(),
        0x0206
    );
    assert_eq!(
        look.character(),
        expected_look
            .character_nominal()
            .expect("Character nominal type")
            .character()
    );
    assert_eq!(
        look.character_nominal(),
        expected_look.semantic_identity_digest()
    );
    assert_eq!(look.diagnostic_name().as_str(), "normal");
    assert_ne!(look.look().as_bytes(), &[0; 32]);
    assert!(report.expressions().all(|(_, expression)| !matches!(
        expression.resolution(),
        CheckedExpressionResolution::Variant(variant)
            if matches!(variant.owner(), CheckedVariantOwner::CharacterNominal { nominal, .. }
                if nominal.family() == crate::types::CharacterNominalFamily::Look)
    )));
    assert_character_signature_projection(&fixture, &report, SOURCE, selected, &expected_look);
}

#[test]
fn character_stage_look_rejects_names_absent_from_the_exact_registered_manifest() {
    let fixture = character_nominal_fixture(concat!(
        "pub character @character.akane Akane as akane {}\n",
        "fn caller() { show(@character.akane, look = .missing); }\n",
    ));

    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::ExpressionTypeUnavailable { .. })
    ));
}

#[test]
fn character_any_show_rejects_reserved_look_instead_of_open_supply() {
    let fixture = fixture(
        "fn supply(speaker: Ref<Character>) { show(speaker, look = 1i64); }\n",
        None,
    );
    let report = analyze(&fixture).expect("Character-Any call keeps rejected evidence");
    let (_, call) = report.calls().next().expect("one Show call");
    assert!(matches!(call.outcome(), CallAnalysisOutcome::Rejected(_)));
    assert!(call.selected_application().is_none());
}

#[test]
fn character_any_show_rejects_reserved_look_clear() {
    let fixture = fixture(
        "fn clear(speaker: Ref<Character>) { show(speaker, look = None); }\n",
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::ValueResolutionFailed { .. })
    ));
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
    assert_eq!(item.value_type(), checked.ty().semantic_identity_digest());
    assert!(item.has_valid_semantic_identity());
    assert_ne!(item.semantic_id().as_bytes(), &[0; 32]);
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
    selected: &Arc<crate::callable::ResolvedCallable>,
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
    assert_eq!(selected_look.declared_type(), Some(expected_look));
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
    assert!(matches!(
        signature.origin(),
        crate::callable::SignatureOrigin::Language { .. }
    ));
    assert_eq!(signature.result(), selected.schema().result());
    let [group] = signature.groups() else {
        panic!("one Character presentation group")
    };
    let signature_look = group
        .parameters()
        .get(1)
        .expect("projected Character look parameter");
    assert_eq!(signature_look.admission(), selected_look.admission());
    assert_eq!(signature_look.declared_type(), Some(expected_look));
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
            selected_execution_arguments(facts)
                .iter()
                .any(|argument| argument.slots().len() == 2)
        })
        .expect("expanded two-slot call facts");
    let slots = selected_execution_arguments(call)[0].slots();

    assert_eq!(slots.len(), 2);
    for (ordinal, slot) in slots.iter().enumerate() {
        assert!(matches!(
            slot.source().raw(),
            CheckedCallArgumentSlotSource::CompactNumericElement {
                ordinal: actual,
                ..
            } if actual == u32::try_from(ordinal).unwrap()
        ));
        assert_eq!(slot.inferred(), &TypeKind::I64);
        assert_eq!(slot.expected(), Some(&TypeKind::I64));
    }
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(
        selected_execution_arguments(call)
            .iter()
            .flat_map(|argument| argument.slots())
            .count(),
        2
    );
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
        .find(|facts| selected_execution_arguments(facts).len() == 1)
        .expect("overloaded call facts");
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);
    assert_eq!(considered.len(), 2);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 2);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 1);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
    assert_eq!(
        selected_execution_arguments(call)
            .iter()
            .flat_map(|argument| argument.slots())
            .count(),
        1
    );
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

fn assert_index_postfix_transaction(fixture: &Fixture) {
    let report = analyze(fixture).expect("postfix expression final analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, postfix) = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::PostfixBracket(postfix) => Some((owner, postfix)),
            _ => None,
        })
        .expect("postfix bracket expression");
    let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates() else {
        panic!("postfix fixture retains both typed candidates");
    };
    assert!(matches!(
        report.expression(owner).map(CheckedExpression::resolution),
        Some(CheckedExpressionResolution::PostfixBracket(
            PostfixBracketResolution::Index { candidate }
        )) if candidate == index
    ));
    assert!(report.expression(*index).is_some());
    assert!(
        report.expression(*dialogue).is_none(),
        "the rejected dialogue candidate must not leak a semantic fact"
    );
    let edges = report
        .checked_child_edges(owner)
        .expect("selected postfix edge graph");
    assert!(edges.iter().any(|(child, role)| {
        *child == *index && matches!(role, CheckedExpressionChildRole::PostfixIndexCandidate)
    }));
    assert!(
        !edges.iter().any(|(_, role)| {
            matches!(role, CheckedExpressionChildRole::PostfixDialogueCandidate)
        })
    );
}

#[test]
fn top_level_postfix_winner_applies_inside_expression_transaction() {
    let fixture = fixture(
        "fn caller(items: Seq<i64>, key: usize) { items[key]; }\n",
        None,
    );
    assert_index_postfix_transaction(&fixture);
}

#[test]
fn selected_postfix_child_missing_from_facts_fails_closed() {
    let fixture = fixture(
        "fn caller(items: Seq<i64>, key: usize) { items[key]; }\n",
        None,
    );
    let accepted = analyze(&fixture).expect("selected index analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let missing = module
        .expressions()
        .find_map(|(_, expression)| {
            let HirExprKind::Index(index) = expression.kind() else {
                return None;
            };
            Some(index.index())
        })
        .expect("selected index operand");
    let mut input = input_from_report(&accepted);
    input.expressions.retain(|(owner, _)| *owner != missing);
    assert!(matches!(
        FinalSemanticAnalysis::try_new(
            fixture.project.executable_view().expect("executable HIR"),
            &fixture.symbols,
            Arc::clone(accepted.hir_topology()),
            accepted.checked_callables().clone(),
            input,
        ),
        Err(FinalSemanticAnalysisError::MissingFact {
            family: SemanticFactFamily::Expression,
        })
    ));
}

#[test]
fn singleton_call_postfix_argument_commits_with_selected_publication() {
    let fixture = fixture(
        concat!(
            "fn consume(value: i64) -> i64 { value }\n",
            "fn caller(items: Seq<i64>, key: usize) { consume(items[key]); }\n",
        ),
        None,
    );
    assert_index_postfix_transaction(&fixture);
}

#[test]
fn multi_candidate_call_postfix_argument_survives_selected_replay() {
    let fixture = typed_overload_fixture(
        "fn caller(items: Seq<i64>, key: usize) { choose(items[key]); }\n",
        "choose",
        vec![
            TestCallableOverload::strict([TypeKind::I64], TypeKind::I64),
            TestCallableOverload::strict([TypeKind::Bool], TypeKind::Bool),
        ],
    );
    assert_index_postfix_transaction(&fixture);
}

#[test]
fn call_adj_a_013_three_candidate_semantic_facts_remain_complete() {
    let fixture = candidate_boundary_fixture(3);
    let report = analyze(&fixture).expect("three-candidate final analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| selected_execution_arguments(facts).len() == 1)
        .expect("three-candidate call facts");
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);

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
    let selected = selected_candidate(call);
    let considered = selected_candidates(call);

    assert_eq!(considered.len(), candidate_count);
    assert_eq!(selected_application(call).result().ty(), &TypeKind::I64);
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
        .find(|facts| matches!(facts.outcome(), CallAnalysisOutcome::Ambiguous(_)))
        .expect("ambiguous call facts");
    let CallAnalysisOutcome::Ambiguous(evidence) = call.outcome() else {
        panic!("the two numeric candidates remain tied");
    };
    let candidates = evidence.candidates();
    let considered = evidence.considered();

    assert_eq!(candidates.len(), 2);
    assert_eq!(considered.len(), 3);
    assert_eq!(call.accounting().logical_argument_checks(), 1);
    assert_eq!(call.accounting().resolver_invocations(), 1);
    assert_eq!(call.accounting().candidate_argument_probes(), 3);
    assert_eq!(call.accounting().selected_replay_argument_visits(), 0);
    assert_eq!(call.accounting().retained_argument_fact_publications(), 1);
}

#[test]
fn work_failure_is_terminal_without_recovery_replay() {
    let fixture = fixture(
        r"
fn combine(left: i64, right: i64) -> i64 { left + right }
fn caller() { combine(1i64, 2i64); }
",
        None,
    );
    let (failed, physical) = analyze_with_query_work(&fixture, 1);
    assert!(failed.is_err());
    assert!(physical.is_empty());
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
        let report = analyze(&fixture).unwrap_or_else(|error| {
            panic!("candidate-contextual argument analysis for `{source}`: {error:?}")
        });
        let call = report
            .calls()
            .map(|(_, facts)| facts)
            .find(|facts| matches!(facts.outcome(), CallAnalysisOutcome::Ambiguous(_)))
            .expect("contextual call facts");
        assert!(matches!(
            call.outcome(),
            CallAnalysisOutcome::Ambiguous(evidence) if evidence.candidates().len() == 2
        ));
        let physical = report
            .physical_candidate_argument_evaluations()
            .filter(|evaluation| evaluation.call_expression() == call.expression())
            .collect::<Vec<_>>();
        assert_eq!(physical.len(), 2);
        for (evaluation, expected) in physical.iter().zip(&expected) {
            assert_eq!(evaluation.pass(), CandidateEvaluationPass::Probe);
            let CandidateExpectedType::Exact(actual) = evaluation.expected() else {
                panic!("contextual candidate owns an exact expected type");
            };
            match (actual, expected) {
                (
                    TypeKind::Function {
                        params: actual_params,
                        return_type: actual_return,
                        effects: actual_effects,
                    },
                    TypeKind::Function {
                        params: expected_params,
                        return_type: expected_return,
                        effects: expected_effects,
                    },
                ) => {
                    assert_eq!(actual_params, expected_params);
                    assert_eq!(actual_return, expected_return);
                    assert_eq!(actual_effects.concrete(), expected_effects.concrete());
                    assert!(matches!(
                        actual_effects.tail(),
                        crate::effect_row::EffectRowTail::Variable(_)
                    ));
                }
                _ => assert_eq!(actual, expected),
            }
        }
        let primary_source = physical[0].source();
        let CandidateExpectedType::Exact(primary_expected) = physical[0].expected() else {
            panic!("primary candidate owns an exact expected type");
        };
        let CheckedCallArgumentSlotSource::Expression(primary_owner) = primary_source else {
            panic!("contextual shorthand/placeholder is expression-backed");
        };
        let published = report
            .expression(primary_owner)
            .expect("primary contextual projection is published")
            .ty();
        match (published, primary_expected) {
            (
                TypeKind::Function {
                    params: published_params,
                    return_type: published_return,
                    effects: published_effects,
                },
                TypeKind::Function {
                    params: expected_params,
                    return_type: expected_return,
                    effects: expected_effects,
                },
            ) => {
                assert_eq!(published_params, expected_params);
                assert_eq!(published_return, expected_return);
                assert_eq!(published_effects.concrete(), expected_effects.concrete());
                assert_eq!(
                    published_effects.tail(),
                    crate::effect_row::EffectRowTail::Closed
                );
                assert!(matches!(
                    expected_effects.tail(),
                    crate::effect_row::EffectRowTail::Variable(_)
                ));
            }
            _ => assert_eq!(published, primary_expected),
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
        assert_eq!(variant.selected().diagnostic_name(), Some("Json"));
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
            .map(|case| case.diagnostic_name().expect("builtin diagnostic name"))
            .collect::<Vec<_>>(),
        arcweft_data::DataFormat::ALL
            .map(arcweft_data::DataFormat::variant_name)
            .into_iter()
            .collect::<Vec<_>>()
    );
    assert!(cases.iter().all(|case| case.payload().is_unit()));
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
        &TypeKind::function_with_effects(
            [TypeKind::I64],
            TypeKind::Bool,
            EffectRow::closed(EffectSet::new()),
        )
    );
    assert_eq!(
        report
            .expression(initializers[3])
            .expect("zero-argument closure initializer fact")
            .ty(),
        &TypeKind::function_with_effects([], TypeKind::I32, EffectRow::closed(EffectSet::new()))
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
    let typed = module
        .patterns()
        .find_map(|(owner, pattern)| {
            matches!(pattern.kind(), HirPatternKind::TypedBinding { .. }).then_some(owner)
        })
        .expect("one typed-binding pattern");
    let checked = report.pattern(typed).expect("typed-binding fact");
    let CheckedPatternResolution::TypedBinding(binding) = checked.resolution() else {
        panic!("typed binding must not collapse into Structural")
    };
    assert_eq!(binding.annotation(), &TypeKind::U64);
    assert_eq!(
        binding.annotation_digest(),
        TypeKind::U64.semantic_identity_digest()
    );
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
            error: Box::new(crate::env::nominal::standard_agent_error_type()),
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
    assert_dialogue_application_result_authority(
        &report,
        application,
        &TypeKind::DialogueLine(Box::new(TypeKind::Unit)),
    );
    let (postfix_owner, rejected_index) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::PostfixBracket(postfix) = expression.kind() else {
                return None;
            };
            let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
            else {
                return None;
            };
            (*dialogue == application).then_some((owner, *index))
        })
        .expect("outer Dialogue postfix selection");
    let edges = report
        .checked_child_edges(postfix_owner)
        .expect("selected Dialogue postfix edges");
    assert!(edges.iter().any(|(child, role)| {
        *child == application
            && matches!(role, CheckedExpressionChildRole::PostfixDialogueCandidate)
    }));
    assert!(
        !edges
            .iter()
            .any(|(_, role)| { matches!(role, CheckedExpressionChildRole::PostfixIndexCandidate) })
    );
    assert!(report.expression(rejected_index).is_none());
    let dormant_content_target = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Path(path) = expression.kind() else {
                return None;
            };
            (path.as_resolved().and_then(|path| path.lexical_name()) == Some("Hello"))
                .then_some(owner)
        })
        .expect("dormant index-alternative content target");
    assert!(report.expression(dormant_content_target).is_none());
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
    let selected = selected_candidate(call);
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
    let selected = selected_candidate(application_call);
    let considered = selected_candidates(application_call);
    assert_eq!(considered.len(), 1);
    assert_eq!(
        selected.id(),
        &CallableCandidateId::Dialogue(DialogueCallableId::ContentApplication)
    );
    assert_eq!(
        selected.schema().validator(),
        &CallableValidator::Dialogue(DialogueCallableId::ContentApplication)
    );
    assert!(selected_execution_arguments(application_call).is_empty());
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
    let CheckedPatchOperation::Set {
        value: custom_value,
        ty: TypeKind::String,
    } = custom.operation()
    else {
        panic!("custom field must retain one typed Set source")
    };
    let selected = selected_candidate(report.call(factory_owner).expect("custom field call fact"));
    let mood = selected.schema().groups()[0]
        .parameters()
        .iter()
        .find(|parameter| parameter.name().is_some_and(|name| name.as_str() == "mood"))
        .expect("accepted custom binding is part of the shared signature schema");
    assert_eq!(mood.declared_type(), Some(&TypeKind::String));
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|row| {
            row.call_expression() == factory_owner
                && row.source() == CheckedCallArgumentSlotSource::Expression(*custom_value)
        })
        .collect::<Vec<_>>();
    assert!(
        !physical.is_empty(),
        "custom source must have a physical candidate observation"
    );
    assert!(
        physical
            .iter()
            .all(|row| row.pass() == CandidateEvaluationPass::Probe
                && row.kind() == PhysicalArgumentEvaluationKind::Authored)
    );
    for (index, row) in physical.iter().enumerate() {
        assert!(
            physical[..index]
                .iter()
                .all(|previous| !previous.same_candidate_slot(row))
        );
    }
    assert_eq!(
        report
            .call(factory_owner)
            .expect("custom field call fact")
            .accounting()
            .retained_argument_fact_publications(),
        1
    );
    assert_eq!(
        report
            .expressions()
            .filter(|(owner, _)| owner == custom_value)
            .count(),
        1
    );
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
        Some(TypeKind::function_with_effects(
            Vec::new(),
            any_dialogue.clone(),
            EffectRow::closed(EffectSet::new()),
        ))
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
fn generic_collector_traverses_project_accepted_and_open_nominal_arguments() {
    let project_fixture = fixture(
        r"
struct GenericProject<T> { value: T }

fn identity<T>(value: GenericProject<T>) -> GenericProject<T> { value }
",
        None,
    );
    let project_report = analyze(&project_fixture).expect("project nominal generic analysis");
    let project_type = project_report
        .types()
        .find_map(|(_, ty)| match ty {
            TypeKind::ProjectNominal(nominal)
                if nominal
                    .arguments()
                    .iter()
                    .any(|argument| matches!(argument, TypeKind::GenericParam(_))) =>
            {
                Some(ty.clone())
            }
            _ => None,
        })
        .expect("generic project nominal type");
    let project_declaration = project_fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "identity")
        .expect("project generic declaration")
        .declaration()
        .clone();
    let project_record = project_fixture
        .registered
        .environment()
        .callable_catalog()
        .project_record(&project_declaration)
        .expect("project generic callable record");
    assert_eq!(
        project_record
            .schema()
            .generic_inventory()
            .types()
            .iter()
            .filter(|entry| entry.role() == crate::callable::CallableSchemaGenericRole::Candidate)
            .count(),
        1,
        "project declaration issuer supplies its exact candidate",
    );

    let accepted_id = AcceptedNominalId::new(
        AcceptedNominalOwnerId::Standard,
        ownership_test_type_path("GenericAccepted"),
    );
    let accepted_record = AcceptedNominalRecord::try_new_opaque(
        accepted_id,
        1,
        RuntimeOpaqueTypeProducerId::try_new("test.generic-accepted").expect("accepted producer"),
        RuntimeOpaqueValueClass::Plain,
        RuntimeOpaquePersistence::SnapshotOnly,
        AcceptedNominalOrigin::Test,
        None,
    )
    .expect("accepted generic nominal record");
    let accepted_base = TypeCheckEnv::standard()
        .try_with_nominal_record(accepted_record)
        .expect("accepted generic nominal environment");
    let accepted_fixture = fixture_with_base_environment(
        "fn identity<T>(value: GenericAccepted<T>) -> GenericAccepted<T> { value }\n",
        None,
        accepted_base,
    );
    let accepted_report = analyze(&accepted_fixture).expect("accepted nominal generic analysis");
    let accepted_type = accepted_report
        .types()
        .find_map(|(_, ty)| match ty {
            TypeKind::AcceptedNominal(nominal)
                if nominal
                    .arguments()
                    .iter()
                    .any(|argument| matches!(argument, TypeKind::GenericParam(_))) =>
            {
                Some(ty.clone())
            }
            _ => None,
        })
        .expect("generic accepted nominal type");

    let open_rule = OpenNominalRule::try_new(
        OpenNominalRuleId::new(
            crate::env::identity::EnvironmentBindingId::try_new("generic-open")
                .expect("open rule owner"),
            0,
        ),
        OpenNominalScope::AcceptedWorld,
        OpenNominalPattern::Exact(ownership_test_type_path("GenericOpen")),
        OpenNominalArity::Exact(1),
        None,
    )
    .expect("open generic nominal rule");
    let open_base = TypeCheckEnv::standard()
        .try_with_open_nominal_rule(open_rule)
        .expect("open generic nominal environment");
    let open_fixture = fixture_with_base_environment(
        "fn identity<T>(value: GenericOpen<T>) -> GenericOpen<T> { value }\n",
        None,
        open_base,
    );
    let open_report = analyze(&open_fixture).expect("open nominal generic analysis");
    let open_type = open_report
        .types()
        .find_map(|(_, ty)| match ty {
            TypeKind::OpenNominal(nominal)
                if nominal
                    .arguments()
                    .iter()
                    .any(|argument| matches!(argument, TypeKind::GenericParam(_))) =>
            {
                Some(ty.clone())
            }
            _ => None,
        })
        .expect("generic open nominal type");

    for (label, ty) in [
        ("project", project_type),
        ("accepted", accepted_type),
        ("open", open_type),
    ] {
        let inventory = TypeGenericUseCollector::collect(&ty)
            .unwrap_or_else(|error| panic!("{label} generic collection: {error}"));
        assert_eq!(inventory.types().len(), 1, "{label} generic type count");
    }
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
        group.parameters()[1].declared_type(),
        Some(&TypeKind::Named("DialogueContent".to_owned()))
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

fn controller() -> Result<Unit, AgentError> effects {} {
    Ok(())
}

entry agent @entry.agent.main {
    controller = controller
}
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
    assert_eq!(entry.diagnostic_public_id().as_str(), "entry.agent.main");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .modules()
        .find_map(|(_, module)| {
            (module.resolve_item(entry.lookup_owner()).is_ok()).then_some(module)
        })
        .expect("Entry owner module");
    assert!(matches!(
        module
            .resolve_item(entry.lookup_owner())
            .expect("Entry owner")
            .kind(),
        HirItemKind::Entry(_)
    ));
}

#[test]
fn generic_substitutions_are_candidate_local_and_specialize_result() {
    let generic = |owner| {
        TypeKind::GenericParam(GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(generic_test_owner(owner)),
            0,
        ))
    };
    let first = generic(41);
    let second = generic(42);
    let first_issuer =
        CallableGenericParameterIssuer::accepted_nominal(generic_test_owner(41), 1, 0)
            .expect("first accepted nominal generic issuer");
    let second_issuer =
        CallableGenericParameterIssuer::accepted_nominal(generic_test_owner(42), 1, 0)
            .expect("second accepted nominal generic issuer");
    let fixture = typed_overload_fixture(
        "fn caller() { choose(1i64, 2i64); }\n",
        "choose",
        vec![
            TestCallableOverload::strict([first.clone(), first.clone()], first.clone())
                .with_generic_issuer(first_issuer),
            TestCallableOverload::strict([second.clone(), second.clone()], second.clone())
                .with_generic_issuer(second_issuer),
        ],
    );
    let choose_path = CallablePath::try_new([CallableName::try_new("choose").expect("path")])
        .expect("choose path");
    let choose = fixture
        .registered
        .environment()
        .callable_catalog()
        .free(&choose_path)
        .expect("generic overload set");
    assert_eq!(choose.as_slice().len(), 2);
    let expected_parameters = [
        GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(generic_test_owner(41)),
            0,
        ),
        GenericTypeParameterId::new(
            GenericParameterOwnerId::AcceptedNominal(generic_test_owner(42)),
            0,
        ),
    ];
    for (entry, expected_parameter) in choose.as_slice().iter().zip(expected_parameters) {
        let rows = entry.primary().schema().generic_inventory().types();
        assert_eq!(rows.len(), 1, "one generic candidate row");
        assert_eq!(rows[0].parameter(), &expected_parameter);
        assert!(
            rows.iter()
                .all(|row| { row.role() == crate::callable::CallableSchemaGenericRole::Candidate })
        );
    }
    let report = analyze(&fixture).expect("generic overload analysis");
    let call = report
        .calls()
        .map(|(_, facts)| facts)
        .find(|facts| matches!(facts.outcome(), CallAnalysisOutcome::Ambiguous(_)))
        .expect("generic call facts");
    assert!(matches!(
        call.outcome(),
        CallAnalysisOutcome::Ambiguous(evidence) if evidence.candidates().len() == 2
    ));
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
        .find(|facts| selected_execution_arguments(facts).len() == 1)
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
    assert_eq!(
        selected_execution_arguments(call)
            .iter()
            .flat_map(|argument| argument.slots())
            .count(),
        1
    );
    assert_eq!(
        selected_execution_arguments(call)[0].slots()[0].inferred(),
        &TypeKind::Vec(Box::new(TypeKind::I64))
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
        .find(|facts| selected_execution_arguments(facts).len() == 1)
        .expect("fixed-spread call facts");
    assert_eq!(selected_candidates(call).len(), 2);
    let physical = report
        .physical_candidate_argument_evaluations()
        .filter(|evaluation| evaluation.call_expression() == call.expression())
        .collect::<Vec<_>>();
    assert_eq!(physical.len(), 6);
    assert_eq!(
        selected_execution_arguments(call)
            .iter()
            .flat_map(|argument| argument.slots())
            .count(),
        2
    );
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
        .find(|facts| selected_execution_arguments(facts).len() == 2)
        .expect("unchecked call facts");
    assert!(matches!(
        selected_candidate(call).id(),
        CallableCandidateId::CapacityMethod(_)
    ));
    assert_eq!(selected_candidates(call).len(), 1);
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
    let retained = selected_execution_arguments(call)
        .iter()
        .flat_map(|argument| argument.slots())
        .collect::<Vec<_>>();
    assert_eq!(retained.len(), 2);
    assert!(retained.iter().all(|fact| fact.expected().is_none()));
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
        .find(|facts| {
            facts
                .selected_application()
                .is_some_and(|application| application.core().candidates().candidates().len() == 2)
        })
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
    // Each exact outer attempt evaluates the singleton inner call once: the
    // I64 probe, the U64 probe, and the selected outer replay. The inner call
    // itself remains a singleton probe in all three attempts.
    assert_eq!(inner_physical.len(), 3);
    assert_eq!(
        outer_physical[2].pass(),
        CandidateEvaluationPass::SelectedReplay
    );
    assert_eq!(inner_physical[0].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(inner_physical[1].pass(), CandidateEvaluationPass::Probe);
    assert_eq!(inner_physical[2].pass(), CandidateEvaluationPass::Probe);
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
    assert!(calls.iter().all(|facts| {
        facts.selected_application().is_some_and(|application| {
            matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::CapacityMethod(_)
            ) && application.core().candidates().candidates().len() == 1
        })
    }));
    assert_eq!(report.work().logical_argument_checks(), 6);
    assert_eq!(report.work().resolver_invocations(), 6);
    assert_eq!(report.work().candidate_argument_probes(), 6);
    assert_eq!(report.work().selected_replay_argument_visits(), 0);
    assert_eq!(report.work().retained_argument_fact_publications(), 6);
}

#[test]
fn production_analyzer_routes_string_preserving_value_methods_through_capacity_family() {
    let fixture = fixture(
        "fn normalize(name: String) -> String { name.trim().to_string() }\n",
        None,
    );
    let report = analyze(&fixture).expect("typed String value-method analysis");
    let calls = report.calls().map(|(_, facts)| facts).collect::<Vec<_>>();
    assert_eq!(calls.len(), 2);
    assert!(calls.iter().all(|facts| {
        facts.selected_application().is_some_and(|application| {
            matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::CapacityMethod(_)
            ) && application.core().candidates().candidates().len() == 1
        })
    }));
    assert!(calls.iter().all(|facts| {
        facts
            .selected_application()
            .is_some_and(|application| application.result().ty() == &TypeKind::String)
    }));
}

#[test]
fn production_analyzer_routes_single_string_preserving_value_method() {
    let fixture = fixture(
        "fn normalize(name: String) -> String { name.trim() }\n",
        None,
    );
    let report = analyze(&fixture).expect("typed String value-method analysis");
    let (_, call) = report.calls().next().expect("one method call");
    assert!(matches!(
        call.selected_application(),
        Some(application)
            if matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::CapacityMethod(_)
            ) && application.core().candidates().candidates().len() == 1
    ));
    assert_eq!(selected_application(call).result().ty(), &TypeKind::String);
}

#[test]
fn explicit_extension_receiver_unifies_free_and_dot_callable_identity() {
    let fixture = fixture(
        concat!(
            "fn normalize(self: String, suffix: String) -> String { self }\n",
            "fn direct(value: String) -> String { normalize(value, \"!\") }\n",
            "fn dotted(value: String) -> String { value.normalize(\"!\") }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("typed extension receiver analysis");
    let selected = report
        .calls()
        .map(|(_, facts)| selected_candidate(facts))
        .collect::<Vec<_>>();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].id(), selected[1].id());
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let mut direct_join = None;
    let mut dotted_join = None;
    for (owner, _) in report.calls() {
        let expression = module.resolve_expr(owner).expect("extension call owner");
        let HirExprKind::Call(call) = expression.kind() else {
            panic!("call inventory owner must be a HIR Call")
        };
        match call.callee() {
            HirCallCallee::Value { .. } => {
                direct_join = Some(report.checked_callable_join(owner));
            }
            HirCallCallee::UnresolvedDot { .. } => {
                dotted_join = Some(report.checked_callable_join(owner));
            }
            HirCallCallee::Associated { .. } => panic!("unexpected associated extension call"),
        }
    }
    let direct_join = direct_join
        .expect("direct extension call")
        .expect("direct extension join");
    assert!(matches!(
        direct_join,
        &super::CheckedCallableJoin::Catalog {
            receiver: CallableReceiverMode::None,
            ..
        }
    ));
    let dotted_join = dotted_join
        .expect("dotted extension call")
        .expect("dotted extension join");
    assert!(matches!(
        dotted_join,
        &super::CheckedCallableJoin::Catalog {
            receiver: CallableReceiverMode::Extension {
                receiver: TypeKind::String,
                group,
                parameter,
            },
            ..
        } if group.get() == 0 && parameter.get() == 0
    ));
    assert!(matches!(
        selected[0].instantiation(),
        crate::callable::ResolvedCallableBaseInstantiation::None
    ));
    assert!(matches!(
        selected[1].instantiation(),
        crate::callable::ResolvedCallableBaseInstantiation::Extension { group, parameter, .. }
            if group.get() == 0 && parameter.get() == 0
    ));
}

#[test]
fn data_last_extension_receiver_consumes_the_final_receiver_group() {
    let fixture = fixture(
        concat!(
            "fn append(suffix: String)(self: String) -> String { self }\n",
            "fn dotted(value: String) -> String { value.append(\"!\") }\n",
        ),
        None,
    );
    let report = analyze(&fixture).expect("typed data-last extension analysis");
    let (_, facts) = report.calls().next().expect("one extension call");
    let selected = selected_candidate(facts);
    assert!(matches!(
        selected.instantiation(),
        crate::callable::ResolvedCallableBaseInstantiation::Extension { group, parameter, .. }
            if group.get() == 1 && parameter.get() == 0
    ));
    assert_eq!(selected_application(facts).result().ty(), &TypeKind::String);
    assert!(matches!(
        selected_application(facts).result(),
        crate::callable::CheckedCallResult::Value(_)
    ));
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
    assert!(report.calls().any(|(_, call)| {
        call.selected_application().is_some_and(|application| {
            matches!(
                application.core().candidates().selected().id(),
                CallableCandidateId::Project(owner) if owner == declaration
            )
        })
    }));
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (_call_owner, _value_receiver, nominal_receiver) = module
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

entry agent @entry.agent.main { controller = run_smoke }
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
            call.selected_application().is_some_and(|application| {
                matches!(
                    application.core().candidates().selected().id(),
                    CallableCandidateId::Agent(_)
                )
            })
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
    let count_probe = metric(@metric.count)
    wait(
        all(exists(signal(@signal.ready)), not(signal(@signal.ready).eq(false))),
        timeout = 5s,
        stable_frames = 1u32,
        poll_frames = 1u32,
    )
    return Ok(())
}

signal ready: bool
metric counter count: u64 {}

entry agent @entry.agent.main { controller = composite_wait }
",
        None,
    );
    let report = analyze(&fixture).unwrap_or_else(|error| {
        let module = fixture
            .project
            .executable_view()
            .expect("executable HIR")
            .module(&CanonicalModulePath::crate_root())
            .expect("root HIR module");
        panic!(
            "Agent composite wait analysis failed: {error:?}\nexpressions: {:#?}",
            module.expressions().collect::<Vec<_>>()
        )
    });
    assert_eq!(
        report
            .calls()
            .filter(|(_, call)| {
                call.selected_application().is_some_and(|application| {
                    matches!(
                        application.core().candidates().selected().id(),
                        CallableCandidateId::Agent(_)
                    )
                })
            })
            .count(),
        7,
        "calls: {:#?}",
        report.calls().collect::<Vec<_>>()
    );
    assert_eq!(
        report
            .calls()
            .filter(|(_, call)| {
                call.selected_application().is_some_and(|application| {
                    matches!(
                        application.core().candidates().selected().id(),
                        CallableCandidateId::DomainMethod(DomainMethodId::ProbeCompare { .. })
                    )
                })
            })
            .count(),
        1
    );
    let (signal_owner, signal) = report
        .calls()
        .find(|(_, call)| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::Agent(AgentIntrinsicSignatureId::Signal)
            })
        })
        .expect("typed signal probe call");
    assert_eq!(
        selected_application(signal).result().ty(),
        &TypeKind::Probe(Box::new(TypeKind::Bool))
    );
    assert_eq!(
        report.expression(signal_owner).map(CheckedExpression::ty),
        Some(&TypeKind::Probe(Box::new(TypeKind::Bool)))
    );
    let (metric_owner, metric) = report
        .calls()
        .find(|(_, call)| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::Agent(AgentIntrinsicSignatureId::Metric)
            })
        })
        .expect("typed metric probe call");
    assert_eq!(
        selected_application(metric).result().ty(),
        &TypeKind::Probe(Box::new(TypeKind::U64))
    );
    assert_eq!(
        report.expression(metric_owner).map(CheckedExpression::ty),
        Some(&TypeKind::Probe(Box::new(TypeKind::U64)))
    );
}

#[test]
fn agent_signal_payload_closes_before_the_exists_parent_scope() {
    let fixture = fixture(
        r"
fn local_wait() -> Result<Unit, AgentError>
effects { agent.observe, agent.wait }
{
    let ready = signal(@signal.ready)
    wait(exists(ready), timeout = 5s)
    return Ok(())
}
signal ready: bool

entry agent @entry.agent.main { controller = local_wait }
",
        None,
    );
    let report = analyze(&fixture)
        .unwrap_or_else(|error| panic!("Agent local signal projection: {error:?}"));
    let signal = report
        .calls()
        .map(|(_, call)| call)
        .find(|call| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::Agent(AgentIntrinsicSignatureId::Signal)
            })
        })
        .expect("selected signal call");
    assert_eq!(
        selected_application(signal).result().ty(),
        &TypeKind::Probe(Box::new(TypeKind::Bool))
    );
    let exists = report
        .calls()
        .map(|(_, call)| call)
        .find(|call| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::Agent(AgentIntrinsicSignatureId::Exists)
            })
        })
        .expect("selected exists call");
    assert_eq!(
        selected_application(exists).result().ty(),
        &TypeKind::Predicate
    );
    assert!(report.expressions().all(|(_, expression)| {
        crate::types::TypeGenericUseCollector::collect(expression.ty()).is_ok_and(|inventory| {
            inventory.types().iter().all(|parameter| {
                parameter.owner()
                    != &GenericParameterOwnerId::LanguageIntrinsic(
                        crate::types::LanguageIntrinsicGenericOwner::AgentSignal,
                    )
            })
        })
    }));
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

entry agent @entry.agent.main { controller = run_smoke }
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
fn agent_diagnostics_method_uses_the_typed_builtin_receiver() {
    let fixture = fixture(
        r"
fn inspect() -> Result<Unit, AgentError> effects { agent.observe } {
    let result = diagnostics().has_error()
    return Ok(())
}

entry agent @entry.agent.main { controller = inspect }
",
        None,
    );
    let report = analyze(&fixture).expect("typed Agent diagnostics analysis");
    let (owner, call) = report
        .calls()
        .find(|(_, call)| {
            call.selected_application().is_some_and(|application| {
                application.core().candidates().selected().id()
                    == &CallableCandidateId::DomainMethod(DomainMethodId::DiagnosticsHasError)
            })
        })
        .expect("typed diagnostics method call");
    assert_eq!(
        selected_application(call).result().ty(),
        &TypeKind::Predicate
    );

    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let HirExprKind::Call(hir_call) = module
        .resolve_expr(owner)
        .expect("diagnostics method call resolves")
        .kind()
    else {
        panic!("diagnostics method fact must belong to one Call expression")
    };
    let callee = hir_call
        .callee()
        .value_expression()
        .expect("diagnostics method has a value callee");
    let HirExprKind::Select(select) = module
        .resolve_expr(callee)
        .expect("diagnostics method select resolves")
        .kind()
    else {
        panic!("diagnostics method callee must be a member selection")
    };
    assert_eq!(
        report
            .expression(select.target())
            .map(CheckedExpression::ty),
        Some(&TypeKind::AgentBuiltin(AgentBuiltinType::Diagnostics))
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

entry agent @entry.agent.main { controller = run_smoke }
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
            call.selected_application().is_some_and(|application| {
                matches!(
                    application.core().candidates().selected().id(),
                    CallableCandidateId::Agent(_)
                )
            })
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
fn scoped_flow_effect_bound_covers_the_same_unscoped_runtime_operation() {
    let fixture = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

flow main() effects { fs.read(save) } {
    let text = fs.read_text(path = "profile.json")
}
"#,
        None,
    );

    analyze(&fixture).expect("a scoped Flow effect bound covers its unscoped operation");
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
    let (topology, checked_callables) = checked_callables(&accepted, &input);
    let report = FinalSemanticAnalysis::try_new(
        accepted.project.executable_view().expect("accepted HIR"),
        &accepted.symbols,
        topology,
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

#[test]
fn view_has_checked_callable_and_project_index_rows_without_a_call_binding() {
    let fixture = fixture("view Main(count: u32 = 1) {\n    Text(count)\n}\n", None);
    let analysis = analyze(&fixture).expect("View checked callable analysis");
    let symbol = fixture
        .symbols
        .callable_symbols()
        .find(|symbol| symbol.owner() == arcweft_lang_hir::symbol::CallableDeclarationOwner::View)
        .expect("View callable symbol");
    let facts = analysis
        .checked_callables()
        .project_callable(symbol.declaration())
        .expect("View checked callable facts");
    assert_eq!(facts.record().schema().result(), &TypeKind::ViewValue);
    assert_eq!(facts.record().schema().groups().len(), 1);
    assert_eq!(facts.record().schema().groups()[0].parameters().len(), 1);

    let index = ProjectSemanticIndex::try_from_final_project(
        ProgramHash::new("checked-view-callable"),
        fixture.project.executable_view().expect("executable HIR"),
        &fixture.symbols,
        &analysis,
    )
    .expect("View project index");
    let indexed = index
        .project_callable_by_declaration(symbol.declaration())
        .expect("View callable index row");
    assert_eq!(indexed.kind(), ProjectCallableKind::View);
    assert_eq!(indexed.checked(), facts.id());
}

#[test]
fn view_modifier_without_an_accepted_catalog_fails_at_the_call_owner() {
    let fixture = fixture("view Main() {\n    Text(\"hello\").x(10px)\n}\n", None);
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Call(call) => match call.callee() {
                HirCallCallee::Value { value }
                    if module
                        .resolve_expr(*value)
                        .is_ok_and(|callee| matches!(callee.kind(), HirExprKind::Select(_))) =>
                {
                    Some(owner)
                }
                _ => None,
            },
            _ => None,
        })
        .expect("well-formed selected-member View call");

    let result = analyze(&fixture);
    assert!(
        matches!(
            &result,
            Err(FinalSemanticAnalysisError::UnknownCallTarget {
                owner: rejected,
                kind: UnknownCallKind::Method,
                name,
                ..
            }) if rejected == &owner && name == "x"
        ),
        "{result:?}"
    );
}

#[test]
fn registered_on_click_selects_the_typed_modifier_and_exact_handler_contract() {
    let fixture = fixture(
        "view Main(dialogue: DialogueView) {\n    Button().on_click { dialogue.primary_action }\n}\n",
        None,
    );
    let analysis = analyze(&fixture).expect("registered on_click View modifier analysis");
    let module = fixture
        .project
        .executable_view()
        .expect("executable HIR")
        .module(&CanonicalModulePath::crate_root())
        .expect("root HIR module");
    let (owner, callee, handler) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            let selected = analysis.call(owner)?.selected_application()?;
            if selected.core().candidates().selected().schema().validator()
                != &CallableValidator::ViewModifier(ViewModifierId::OnActivate)
            {
                return None;
            }
            let HirCallCallee::Value { value: callee } = call.callee() else {
                return None;
            };
            let [argument] = call.arguments() else {
                return None;
            };
            Some((owner, *callee, argument.value()))
        })
        .expect("typed on_click application");

    assert_eq!(
        analysis.expression(owner).expect("modifier result").ty(),
        &TypeKind::ViewValue
    );
    assert!(matches!(
        analysis
            .expression(callee)
            .expect("modifier callee")
            .resolution(),
        CheckedExpressionResolution::Select(CheckedSelectResolution::Method(_))
    ));
    assert_eq!(
        analysis.expression(handler).expect("handler closure").ty(),
        ViewModifierId::OnActivate.signature().params()[0].ty()
    );
}

#[test]
fn registered_on_click_rejects_a_non_action_handler() {
    let fixture = fixture(
        "view Main(dialogue: DialogueView) {\n    Button().on_click { \"not an action\" }\n}\n",
        None,
    );
    assert!(matches!(
        analyze(&fixture),
        Err(FinalSemanticAnalysisError::CheckedCallableJoin(error))
            if error.as_ref() == &CheckedCallableJoinError::NotSelected
    ));
}

#[test]
fn registered_on_click_rejects_an_effectful_action_producer_at_the_call_owner() {
    let fixture = fixture(
        r#"
extern capability fs {
    fn read_text(path: String) -> String effects { fs.read }
}

fn impure(value: DialogueAction) -> DialogueAction effects { fs.read } {
    value
}

view Main(dialogue: DialogueView) {
    Button().on_click { impure(dialogue.primary_action) }
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
    let owner = module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Call(call)
                if matches!(
                    call.callee(),
                    HirCallCallee::Value { value }
                        if module.resolve_expr(*value).is_ok_and(|callee| {
                            matches!(callee.kind(), HirExprKind::Select(select) if module.resolve_expr(select.target()).is_ok_and(|target| matches!(target.kind(), HirExprKind::Call(_))))
                        })
                ) => Some(owner),
            _ => None,
        })
        .expect("outer selected-member View call");

    let result = analyze(&fixture);
    assert!(
        matches!(
            &result,
            Err(FinalSemanticAnalysisError::CallResolutionFailed { owner: rejected })
                if rejected == &owner
        ),
        "{result:?}"
    );
}
