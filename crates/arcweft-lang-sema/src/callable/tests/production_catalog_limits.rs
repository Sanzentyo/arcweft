use std::{ops::Range, sync::Arc};

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        CallableDeclarationId, CallableDeclarationOwner, CallablePackageId,
        ProjectExternalDeclarations, ProjectSymbolRevision, ProjectSymbolTable,
        ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    parser::{ParseOptions, parse_document_with_source},
};
use arcweft_source::{
    SourceDocument, SourceDocumentId, SourceDocumentIdentity, SourceName, SourceRange,
};

use crate::{
    callable::{
        AdapterPackageId, CallableArgumentPolicy, CallableAuthorityRank, CallableBuildLimitError,
        CallableCandidateId, CallableCatalogBuildError, CallableDiagnostic, CallableDiagnosticCode,
        CallableDiagnosticSeverity, CallableDiagnosticSubject, CallableDocumentation,
        CallableEffectSchema, CallableGroupIndex, CallableGroupKind, CallableInstantiation,
        CallableLookupKey, CallableName, CallableOverloadIndex, CallableParameterGroup,
        CallablePath, CallablePathError, CallableQueryLimitError, CallableSchemaError,
        CallableSignatureSchema, CallableValidator, EnvironmentCallableKind,
        EnvironmentCallableOwner, EnvironmentCallablePublication,
        EnvironmentCallablePublicationRecord, EnvironmentDeclarationOrdinal,
        NonEmptyResolvedCandidates, PRODUCTION_CALLABLE_LIMITS, ProjectCallablePath,
        RegisteredCallableCatalogBuilder, ResolveCallError, ResolvedCallable,
        SemanticSignatureError, SemanticSignatureHelp, SemanticSignatureIndex,
        SemanticSignatureRecovery, SignatureOrigin, SignatureWorkReport, SpreadArgumentPolicy,
        StandardEnvironmentId, UnknownNamedArgumentPolicy,
    },
    checker::TypeExpressionId,
    effect_row::EffectRow,
    registration::{AcceptedNominalWorld, EnvironmentManifestDigest},
    types::TypeKind,
};

use super::{
    accepted_nominal_world, external_binding_project, group, semantic_signature,
    signature_query_work_report,
};
use crate::callable::limits::{CatalogBuildWork, ResolverWork};

const PRODUCTION_RECORDS: usize = 262_144;
const DUPLICATE_KEYS: usize = 87_380;
const UNIQUE_KEYS_PER_PUBLICATION: usize = 43_692;

#[test]
fn production_path_and_group_limits_accept_exact_and_reject_one_over() {
    let exact_path = CallablePath::try_new(
        (0..PRODUCTION_CALLABLE_LIMITS.max_path_segments())
            .map(|index| callable_name(format!("segment_{index}"))),
    )
    .expect("the exact production path boundary is accepted");
    assert_eq!(
        exact_path.len(),
        PRODUCTION_CALLABLE_LIMITS.max_path_segments()
    );
    assert_eq!(
        CallablePath::try_new(
            (0..=PRODUCTION_CALLABLE_LIMITS.max_path_segments())
                .map(|index| callable_name(format!("segment_{index}"))),
        ),
        Err(CallablePathError::TooManySegments {
            actual: 33,
            limit: 32,
        })
    );

    let exact_schema = production_schema(PRODUCTION_CALLABLE_LIMITS.max_groups_per_callable())
        .expect("the exact production group boundary is accepted");
    assert_eq!(
        exact_schema.groups().len(),
        PRODUCTION_CALLABLE_LIMITS.max_groups_per_callable()
    );
    assert_eq!(
        production_schema(PRODUCTION_CALLABLE_LIMITS.max_groups_per_callable() + 1),
        Err(CallableSchemaError::GroupLimit {
            actual: 17,
            limit: 16,
        })
    );
}

#[test]
fn production_candidate_owner_accepts_exact_and_rejects_one_over_atomically() {
    let exact = NonEmptyResolvedCandidates::try_new(
        resolved_candidates(PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call()),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("the exact production candidate boundary is accepted");
    assert_eq!(
        exact.len().get() as usize,
        PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call()
    );

    assert_eq!(
        NonEmptyResolvedCandidates::try_new(
            resolved_candidates(PRODUCTION_CALLABLE_LIMITS.max_candidates_per_call() + 1),
            &PRODUCTION_CALLABLE_LIMITS,
        ),
        Err(ResolveCallError::CandidateLimit {
            actual: 257,
            limit: 256,
        })
    );
}

#[test]
fn production_recovery_owner_accepts_exact_and_rejects_one_over() {
    let document = signature_document("production-recovery-limits");
    let call_span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let exact_work = SignatureWorkReport::try_new(
        0,
        0,
        0,
        PRODUCTION_CALLABLE_LIMITS.max_recovery_nodes(),
        0,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("exact recovery work report");
    let exact = semantic_help(
        &document,
        call_span.clone(),
        SemanticSignatureRecovery::Recovered {
            missing_close_delimiter: true,
            nodes: PRODUCTION_CALLABLE_LIMITS.max_recovery_nodes(),
        },
        Vec::new(),
        exact_work,
    )
    .expect("the exact production recovery boundary is accepted");
    assert_eq!(
        exact.recovery(),
        SemanticSignatureRecovery::Recovered {
            missing_close_delimiter: true,
            nodes: 256,
        }
    );

    assert_eq!(
        semantic_help(
            &document,
            call_span,
            SemanticSignatureRecovery::Recovered {
                missing_close_delimiter: true,
                nodes: PRODUCTION_CALLABLE_LIMITS.max_recovery_nodes() + 1,
            },
            Vec::new(),
            exact_work,
        ),
        Err(SemanticSignatureError::Limit(
            CallableQueryLimitError::RecoveryNodes {
                actual: 257,
                limit: 256,
            }
        ))
    );
}

#[test]
fn production_diagnostic_owner_accepts_exact_and_rejects_one_over() {
    let document = signature_document("production-diagnostic-limits");
    let call_span = document
        .span(SourceRange::new(0, document.text().len()))
        .expect("call span");
    let diagnostic = CallableDiagnostic::try_new(
        CallableDiagnosticCode::ResourceExhausted,
        CallableDiagnosticSeverity::Information,
        None,
        CallableDiagnosticSubject::None,
        Vec::new(),
        Some(document.identity()),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("typed callable diagnostic");
    let exact_work = SignatureWorkReport::try_new(
        0,
        0,
        0,
        0,
        PRODUCTION_CALLABLE_LIMITS.max_diagnostics(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("exact diagnostic work report");
    let exact = semantic_help(
        &document,
        call_span.clone(),
        SemanticSignatureRecovery::Complete,
        vec![diagnostic.clone(); PRODUCTION_CALLABLE_LIMITS.max_diagnostics()],
        exact_work,
    )
    .expect("the exact production diagnostic boundary is accepted");
    assert_eq!(
        exact.diagnostics().len(),
        PRODUCTION_CALLABLE_LIMITS.max_diagnostics()
    );

    assert_eq!(
        semantic_help(
            &document,
            call_span,
            SemanticSignatureRecovery::Complete,
            vec![diagnostic; PRODUCTION_CALLABLE_LIMITS.max_diagnostics() + 1],
            exact_work,
        ),
        Err(SemanticSignatureError::Limit(
            CallableQueryLimitError::Diagnostics {
                actual: 129,
                limit: 128,
            }
        ))
    );
}

#[test]
fn production_module_builder_accepts_exact_and_rejects_one_over_without_staging() {
    let (exact_project, exact_symbols) = empty_module_project(4_096, "exact");
    let exact_world = accepted_nominal_world(&exact_symbols);
    let mut exact_builder = RegisteredCallableCatalogBuilder::for_nominal_world(
        &exact_world,
        PRODUCTION_CALLABLE_LIMITS,
    );
    exact_builder
        .add_project(&exact_project, &exact_symbols, &exact_world)
        .expect("the exact production module boundary is staged");
    let exact_catalog = exact_builder
        .finish()
        .expect("the exact production module boundary freezes");
    assert_eq!(exact_catalog.project().modules().len(), 4_096);

    let (one_over_project, one_over_symbols) = empty_module_project(4_097, "one-over");
    let one_over_world = accepted_nominal_world(&one_over_symbols);
    let mut one_over_builder = RegisteredCallableCatalogBuilder::for_nominal_world(
        &one_over_world,
        PRODUCTION_CALLABLE_LIMITS,
    );
    assert_eq!(
        one_over_builder.add_project(&one_over_project, &one_over_symbols, &one_over_world),
        Err(CallableCatalogBuildError::Limit(
            CallableBuildLimitError::Modules {
                actual: 4_097,
                limit: 4_096,
            }
        ))
    );
    assert!(
        one_over_builder
            .finish()
            .expect("a rejected project stages no modules")
            .project()
            .modules()
            .is_empty()
    );
}

#[test]
fn production_record_and_build_work_limits_freeze_at_the_exact_boundary() {
    // Four publications, 262,144 records, and 87,380 standard/adapter
    // equivalents consume exactly 1,048,576 production build-work units:
    // publications + three record/key passes + three duplicate-comparison
    // units per equivalent pair.
    assert_eq!(
        4 + 3 * PRODUCTION_RECORDS + 3 * DUPLICATE_KEYS,
        usize::try_from(PRODUCTION_CALLABLE_LIMITS.max_catalog_build_work())
            .expect("production catalog work fits usize")
    );
    assert_eq!(
        2 * DUPLICATE_KEYS + 2 * UNIQUE_KEYS_PER_PUBLICATION,
        PRODUCTION_RECORDS
    );

    let (_, symbols) = external_binding_project([]);
    let world = accepted_nominal_world(&symbols);
    let schema = production_schema(1).expect("production record schema");
    let mut builder =
        RegisteredCallableCatalogBuilder::for_nominal_world(&world, PRODUCTION_CALLABLE_LIMITS);

    builder
        .add_environment(production_publication(
            &world,
            EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core),
            publication_records("shared", 0..DUPLICATE_KEYS, &schema),
            0x10,
        ))
        .expect("standard half of equivalent record pairs");
    builder
        .add_environment(production_publication(
            &world,
            adapter_owner("adapter.production-records-equivalent"),
            publication_records("shared", 0..DUPLICATE_KEYS, &schema),
            0x20,
        ))
        .expect("adapter half of equivalent record pairs");
    builder
        .add_environment(production_publication(
            &world,
            adapter_owner("adapter.production-records-unique-a"),
            publication_records("unique_a", 0..UNIQUE_KEYS_PER_PUBLICATION, &schema),
            0x30,
        ))
        .expect("first unique record partition");
    builder
        .add_environment(production_publication(
            &world,
            adapter_owner("adapter.production-records-unique-b"),
            publication_records("unique_b", 0..UNIQUE_KEYS_PER_PUBLICATION, &schema),
            0x40,
        ))
        .expect("second unique record partition");

    let extra = production_publication(
        &world,
        adapter_owner("adapter.production-records-one-over"),
        publication_records("one_over", 0..1, &schema),
        0x50,
    );
    assert_eq!(
        builder.add_environment(extra),
        Err(CallableCatalogBuildError::Limit(
            CallableBuildLimitError::Records {
                actual: 262_145,
                limit: 262_144,
            }
        ))
    );

    let catalog = builder
        .finish()
        .expect("the exact record and catalog-build-work boundaries freeze");
    assert_eq!(
        catalog
            .free(&callable_path("shared", 0))
            .expect("coalesced exact-boundary key")
            .len()
            .get(),
        1
    );
    assert!(
        catalog
            .free(&callable_path("unique_b", UNIQUE_KEYS_PER_PUBLICATION - 1))
            .is_some()
    );
}

#[test]
fn production_work_owners_accept_exact_and_reject_one_over_without_mutation() {
    let build_limit = PRODUCTION_CALLABLE_LIMITS.max_catalog_build_work();
    let mut build = CatalogBuildWork::new(build_limit);
    build
        .charge(build_limit)
        .expect("the exact catalog-build-work boundary is accepted");
    assert_eq!(
        build.charge(1),
        Err(CallableCatalogBuildError::Limit(
            CallableBuildLimitError::Work {
                requested: 1,
                consumed: 1_048_576,
                limit: 1_048_576,
            }
        ))
    );
    assert_eq!(build.consumed(), build_limit);

    let query_limit = PRODUCTION_CALLABLE_LIMITS.max_query_work();
    let mut query = ResolverWork::new(query_limit);
    query
        .charge(query_limit)
        .expect("the exact resolver-query-work boundary is accepted");
    assert_eq!(
        query
            .signature_report(0, 0, &PRODUCTION_CALLABLE_LIMITS)
            .expect("exact resolver work projects a typed report")
            .total_work(),
        Ok(query_limit)
    );
    assert_eq!(
        query.charge(1),
        Err(CallableQueryLimitError::Work {
            requested: 1,
            consumed: 4_096,
            limit: 4_096,
        })
    );
    assert_eq!(query.consumed(), query_limit);
}

fn callable_name(value: impl Into<Arc<str>>) -> CallableName {
    CallableName::try_new(value).expect("valid production callable name")
}

fn callable_path(prefix: &str, index: usize) -> CallablePath {
    CallablePath::try_new([callable_name(format!("{prefix}_{index}"))])
        .expect("single-segment production callable path")
}

fn production_schema(group_count: usize) -> Result<CallableSignatureSchema, CallableSchemaError> {
    let groups = (0..group_count)
        .map(|index| {
            CallableParameterGroup::try_new(
                CallableGroupIndex::try_from_usize(index).expect("group index"),
                if index == 0 {
                    CallableGroupKind::Initial
                } else {
                    CallableGroupKind::Curried
                },
                Vec::new(),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("empty production parameter group")
        })
        .collect();
    CallableSignatureSchema::try_new(
        groups,
        TypeKind::Unit,
        CallableEffectSchema::fixed(EffectRow::default()),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
}

fn resolved_candidates(count: usize) -> Vec<ResolvedCallable> {
    let package = CallablePackageId::try_new("production-candidate-limits").expect("package");
    let module = CanonicalModulePath::crate_root();
    let schema = Arc::new(production_schema(1).expect("candidate schema"));
    (0..count)
        .map(|index| {
            let label = format!("candidate_{index}");
            let declaration = CallableDeclarationId::try_new(
                package.clone(),
                module.clone(),
                CallableDeclarationOwner::Function,
                label.clone(),
            )
            .expect("candidate declaration");
            ResolvedCallable::try_new(
                CallableCandidateId::Project(declaration.clone()),
                SignatureOrigin::Project {
                    declaration,
                    path: ProjectCallablePath::new(
                        package.clone(),
                        module.clone(),
                        CallablePath::try_new([callable_name(label)]).expect("candidate path"),
                    ),
                },
                Arc::clone(&schema),
                CallableInstantiation::None,
                Vec::new(),
                Some(CallableAuthorityRank::Project),
                &PRODUCTION_CALLABLE_LIMITS,
            )
            .expect("resolved production candidate")
        })
        .collect()
}

fn signature_document(id: &str) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(format!("arcweft-memory://{id}.arcw")).expect("document id"),
        SourceName::Memory,
        "panic(value)",
    )
    .expect("signature document")
}

fn semantic_help(
    document: &SourceDocument,
    call_span: arcweft_source::SourceSpan,
    recovery: SemanticSignatureRecovery,
    diagnostics: Vec<CallableDiagnostic>,
    work: SignatureWorkReport,
) -> Result<SemanticSignatureHelp, SemanticSignatureError> {
    SemanticSignatureHelp::try_new(
        document.identity().clone(),
        call_span.clone(),
        call_span,
        TypeExpressionId::from_index(1),
        vec![semantic_signature(None)],
        SemanticSignatureIndex::try_from_usize(0).expect("active signature"),
        None,
        group(0),
        None,
        recovery,
        diagnostics,
        0,
        work,
        signature_query_work_report(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
}

fn empty_module_project(module_count: usize, profile: &str) -> (HirProject, ProjectSymbolTable) {
    let source = " ";
    let mut identities = Vec::<SourceDocumentIdentity>::with_capacity(module_count);
    let mut modules = Vec::with_capacity(module_count);
    for index in 0..module_count {
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(format!(
                    "arcweft-project://production-module-limits/{profile}/{index}.arcw"
                ))
                .expect("module document id"),
                SourceName::path(format!("src/{profile}/{index}.arcw")),
                source,
            )
            .expect("module source document"),
        );
        let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
        assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
        let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("empty module HIR");
        let module = if index == 0 {
            CanonicalModulePath::crate_root()
        } else {
            CanonicalModulePath::from_segments([
                ModuleSegment::new(format!("module_{index}")).expect("module segment")
            ])
        };
        identities.push(document.identity().clone());
        modules.push(
            HirProjectModule::try_new(module, document.identity().clone(), hir)
                .expect("bound empty HIR module"),
        );
    }

    let package_name = format!("production-module-limits-{profile}");
    let project = HirProject::new(package_name.clone(), modules).expect("module-limit project");
    let package = CallablePackageId::try_new(package_name).expect("callable package");
    let root = identities.first().expect("root source identity");
    let world = ProjectSymbolWorldId::try_new(package, root.id().clone(), profile)
        .expect("project symbol world");
    let revision = ProjectSymbolRevision::try_for_documents(identities.iter())
        .expect("project source revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("empty external declarations");
    let symbols = ProjectSymbolTable::link(&project, &externals)
        .expect("empty modules link")
        .into_table();
    (project, symbols)
}

fn production_publication(
    world: &AcceptedNominalWorld,
    owner: EnvironmentCallableOwner,
    records: Vec<EnvironmentCallablePublicationRecord>,
    digest_byte: u8,
) -> EnvironmentCallablePublication {
    EnvironmentCallablePublication::try_new_projected(
        owner,
        world.stamp(),
        EnvironmentManifestDigest::from_bytes([digest_byte; 32]),
        records,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("production environment publication")
}

fn publication_records(
    prefix: &str,
    indices: Range<usize>,
    schema: &CallableSignatureSchema,
) -> Vec<EnvironmentCallablePublicationRecord> {
    indices
        .map(|index| {
            EnvironmentCallablePublicationRecord::try_new(
                EnvironmentCallableKind::Function,
                CallableLookupKey::Free(callable_path(prefix, index)),
                CallableOverloadIndex::try_from_usize(0).expect("overload index"),
                schema.clone(),
                CallableDocumentation::missing(),
                None,
                None,
                EnvironmentDeclarationOrdinal::try_from_usize(index).expect("declaration ordinal"),
            )
            .expect("production environment record")
        })
        .collect()
}

fn adapter_owner(package: &str) -> EnvironmentCallableOwner {
    EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new(package).expect("adapter package identity"),
    )
}
