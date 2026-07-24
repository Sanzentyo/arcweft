use std::sync::Arc;

use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
    symbol::{
        ProjectExternalDeclarations, ProjectSymbolRevision, ProjectSymbolTable,
        ProjectSymbolWorldId,
    },
};
use arcweft_lang_syntax::{
    ast::module_path::{CanonicalModulePath, ModuleSegment},
    parser::parse_source,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

use crate::{
    callable::{CallableRecord, RegisteredCallableCatalog, RegisteredProjectModuleCallables},
    effect_row::EffectRow,
    registration::{AcceptedNominalWorld, EnvironmentManifestDigest},
    types::TypeKind,
};

use super::{
    AdapterPackageId, CallableArgumentPolicy, CallableCandidateId, CallableDocumentation,
    CallableEffectSchema, CallableGroupIndex, CallableGroupKind, CallableLookupKey,
    CallableOverloadIndex, CallableParameterGroup, CallableSignatureSchema, CallableValidator,
    EnvironmentCallableId, EnvironmentCallableKind, EnvironmentCallableOwner,
    EnvironmentCallablePublication, EnvironmentCallablePublicationRecord,
    EnvironmentDeclarationOrdinal, PRODUCTION_CALLABLE_LIMITS, RegisteredCallableCatalogBuilder,
    SpreadArgumentPolicy, StandardEnvironmentId, UnknownNamedArgumentPolicy,
    accepted_nominal_world, path,
};

const PACKAGE: &str = "callable-determinism-tests";

#[test]
fn reversed_project_module_input_is_canonicalized_by_hir_order() {
    let root_document = source_document(
        "arcweft-project://callable-determinism-tests/src/main.arcw",
        "fn root_value(value: i32) -> i32 { value }\n",
    );
    let child_document = source_document(
        "arcweft-project://callable-determinism-tests/src/child.arcw",
        "fn child_value(value: String) -> String { value }\n",
    );
    let root = project_module(CanonicalModulePath::crate_root(), &root_document);
    let child_path = CanonicalModulePath::crate_root()
        .join(ModuleSegment::new("child").expect("child module segment"));
    let child = project_module(child_path.clone(), &child_document);

    let forward_project =
        HirProject::new(PACKAGE, [root.clone(), child.clone()]).expect("forward HIR project");
    let reversed_project =
        HirProject::new(PACKAGE, [child, root]).expect("reversed HIR project input");
    let documents = [root_document, child_document];

    let forward = project_catalog(&forward_project, &documents);
    let reversed = project_catalog(&reversed_project, &documents);

    assert_eq!(forward.project().modules(), reversed.project().modules());
    assert_eq!(project_records(&forward), project_records(&reversed));
    assert_eq!(forward.project(), reversed.project());
    assert_eq!(forward.digest(), reversed.digest());

    let module_rows = forward.project().modules();
    assert_eq!(module_rows.len(), 2);
    assert_eq!(module_rows[0].module(), &CanonicalModulePath::crate_root());
    assert_eq!(module_rows[1].module(), &child_path);
    assert_eq!(module_rows[0].declarations().len(), 1);
    assert_eq!(module_rows[1].declarations().len(), 1);
}

#[test]
fn reversed_environment_publications_same_catalog() {
    let (_, symbols) = super::external_binding_project([]);
    let world = accepted_nominal_world(&symbols);
    let key = CallableLookupKey::Free(path(&["stable_lookup"]));
    let overload = CallableOverloadIndex::try_from_usize(0).expect("overload index");

    let standard_owner = EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core);
    let adapter_owner = EnvironmentCallableOwner::Adapter(
        AdapterPackageId::try_new("adapter.determinism").expect("adapter owner"),
    );
    let standard = publication(
        &world,
        standard_owner.clone(),
        0x11,
        vec![publication_record(
            key.clone(),
            TypeKind::String,
            overload,
            0,
        )],
    );
    let adapter = publication(
        &world,
        adapter_owner.clone(),
        0x22,
        vec![publication_record(key.clone(), TypeKind::I64, overload, 0)],
    );

    let forward = environment_catalog(&world, [standard.clone(), adapter.clone()]);
    let reversed = environment_catalog(&world, [adapter, standard]);
    let lookup = path(&["stable_lookup"]);
    let forward_set = forward.free(&lookup).expect("forward lookup set");
    let reversed_set = reversed.free(&lookup).expect("reversed lookup set");

    assert_eq!(forward_set, reversed_set);
    assert_eq!(forward.digest(), reversed.digest());

    let expected = [
        CallableCandidateId::Environment(EnvironmentCallableId::new(
            standard_owner,
            EnvironmentCallableKind::Function,
            key.clone(),
            overload,
        )),
        CallableCandidateId::Environment(EnvironmentCallableId::new(
            adapter_owner,
            EnvironmentCallableKind::Function,
            key,
            overload,
        )),
    ];
    assert_eq!(primary_ids(forward_set), expected);
    assert_eq!(primary_ids(reversed_set), expected);

    for candidate in expected {
        let CallableCandidateId::Environment(id) = candidate else {
            unreachable!("environment publication produces an environment candidate")
        };
        assert_eq!(
            forward.environment_record(&id),
            reversed.environment_record(&id),
            "by-ID lookup must not observe publication insertion order"
        );
    }
}

#[test]
fn hashmap_seed_does_not_change_result() {
    let (_, symbols) = super::external_binding_project([]);
    let world = accepted_nominal_world(&symbols);
    let keys = (0..12)
        .map(|index| format!("seed_stable_{index:02}"))
        .collect::<Vec<_>>();
    let mut baseline = None;

    for attempt in 0..32 {
        let mut standard_records = deterministic_records(&keys, &TypeKind::String);
        let mut adapter_records = deterministic_records(&keys, &TypeKind::String);
        if attempt % 2 == 1 {
            standard_records.reverse();
        }
        if attempt % 3 == 1 {
            let rotation = attempt % adapter_records.len();
            adapter_records.rotate_left(rotation);
        } else if attempt % 3 == 2 {
            adapter_records.reverse();
        }

        let standard = publication(
            &world,
            EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core),
            0x31,
            standard_records,
        );
        let adapter = publication(
            &world,
            EnvironmentCallableOwner::Adapter(
                AdapterPackageId::try_new("adapter.seed-stability").expect("adapter owner"),
            ),
            0x32,
            adapter_records,
        );
        let catalog = if attempt % 2 == 0 {
            environment_catalog(&world, [standard, adapter])
        } else {
            environment_catalog(&world, [adapter, standard])
        };
        let snapshot = (
            catalog.digest(),
            keys.iter()
                .map(|key| lookup_snapshot(&catalog, key))
                .collect::<Vec<_>>(),
        );

        if let Some(expected) = &baseline {
            assert_eq!(
                &snapshot, expected,
                "fresh HashMap state and input permutations must not affect typed lookup slices"
            );
        } else {
            baseline = Some(snapshot);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LookupSnapshot {
    selected: CallableCandidateId,
    equivalent: Vec<CallableCandidateId>,
    selected_record: CallableRecord,
    equivalent_records: Vec<CallableRecord>,
}

fn source_document(id: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source document ID"),
            SourceName::path(id),
            source,
        )
        .expect("source document"),
    )
}

fn project_module(module: CanonicalModulePath, document: &SourceDocument) -> HirProjectModule {
    let parsed = parse_source(document.text());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(document, parsed.typed_tree()).expect("lowered HIR module");
    HirProjectModule::try_new(module, document.identity().clone(), hir)
        .expect("source-bound HIR module")
}

fn project_catalog(
    project: &HirProject,
    documents: &[Arc<SourceDocument>],
) -> RegisteredCallableCatalog {
    let root = project
        .source(&CanonicalModulePath::crate_root())
        .expect("root source identity");
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        root.id().clone(),
        "canonical-module-order",
    )
    .expect("project symbol world");
    let revision = ProjectSymbolRevision::try_for_documents(
        documents.iter().map(|document| document.identity()),
    )
    .expect("project symbol revision");
    let externals =
        ProjectExternalDeclarations::try_new(world, revision, Vec::new()).expect("empty externals");
    let symbols = ProjectSymbolTable::link(project, &externals)
        .expect("linked project symbols")
        .into_table();
    let nominal_world = accepted_nominal_world(&symbols);
    let mut builder = RegisteredCallableCatalogBuilder::for_nominal_world(
        &nominal_world,
        PRODUCTION_CALLABLE_LIMITS,
    );
    builder
        .add_project(project, &symbols, &nominal_world)
        .expect("project callable publication");
    builder
        .add_project_bindings(project, &symbols, |_| {
            Some(TypeKind::Named("Module".to_owned()))
        })
        .expect("project binding publication");
    builder.finish().expect("registered project catalog")
}

fn project_records(catalog: &RegisteredCallableCatalog) -> Vec<CallableRecord> {
    catalog
        .project()
        .modules()
        .iter()
        .flat_map(RegisteredProjectModuleCallables::declarations)
        .map(|declaration| {
            catalog
                .project_record(declaration)
                .expect("module declaration has a project record")
                .as_ref()
                .clone()
        })
        .collect()
}

fn callable_schema(result: TypeKind) -> CallableSignatureSchema {
    let group = CallableParameterGroup::try_new(
        CallableGroupIndex::ZERO,
        CallableGroupKind::Initial,
        Vec::new(),
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("empty initial parameter group");
    CallableSignatureSchema::try_new(
        vec![group],
        result,
        CallableEffectSchema::fixed(EffectRow::default()),
        CallableArgumentPolicy::new(
            UnknownNamedArgumentPolicy::Reject,
            SpreadArgumentPolicy::Reject,
        ),
        CallableValidator::Ordinary,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("determinism schema")
}

fn publication_record(
    key: CallableLookupKey,
    result: TypeKind,
    overload: CallableOverloadIndex,
    ordinal: usize,
) -> EnvironmentCallablePublicationRecord {
    EnvironmentCallablePublicationRecord::try_new(
        EnvironmentCallableKind::Function,
        key,
        overload,
        callable_schema(result),
        CallableDocumentation::missing(),
        None,
        None,
        EnvironmentDeclarationOrdinal::try_from_usize(ordinal)
            .expect("environment declaration ordinal"),
    )
    .expect("environment publication record")
}

fn deterministic_records(
    keys: &[String],
    result: &TypeKind,
) -> Vec<EnvironmentCallablePublicationRecord> {
    keys.iter()
        .enumerate()
        .map(|(ordinal, key)| {
            publication_record(
                CallableLookupKey::Free(path(&[key])),
                result.clone(),
                CallableOverloadIndex::try_from_usize(0).expect("overload index"),
                ordinal,
            )
        })
        .collect()
}

fn publication(
    world: &AcceptedNominalWorld,
    owner: EnvironmentCallableOwner,
    digest_byte: u8,
    records: Vec<EnvironmentCallablePublicationRecord>,
) -> EnvironmentCallablePublication {
    EnvironmentCallablePublication::try_new_projected(
        owner,
        world.stamp(),
        EnvironmentManifestDigest::from_bytes([digest_byte; 32]),
        records,
        &PRODUCTION_CALLABLE_LIMITS,
    )
    .expect("typed environment publication")
}

fn environment_catalog(
    world: &AcceptedNominalWorld,
    publications: impl IntoIterator<Item = EnvironmentCallablePublication>,
) -> RegisteredCallableCatalog {
    let mut builder =
        RegisteredCallableCatalogBuilder::for_nominal_world(world, PRODUCTION_CALLABLE_LIMITS);
    for publication in publications {
        builder
            .add_environment(publication)
            .expect("publication belongs to the nominal world");
    }
    builder.finish().expect("registered environment catalog")
}

fn primary_ids(set: &super::NonEmptyCallableSet) -> Vec<CallableCandidateId> {
    set.as_slice()
        .iter()
        .map(|entry| entry.primary().id().clone())
        .collect()
}

fn lookup_snapshot(catalog: &RegisteredCallableCatalog, key: &str) -> LookupSnapshot {
    let lookup = path(&[key]);
    let set = catalog.free(&lookup).expect("deterministic lookup set");
    assert_eq!(
        set.len().get(),
        1,
        "exact standard/adapter duplicates coalesce before selection"
    );
    let selected = set.first();
    assert!(matches!(
        selected.primary().id(),
        CallableCandidateId::Environment(id)
            if id.owner() == &EnvironmentCallableOwner::Standard(StandardEnvironmentId::Core)
    ));
    let equivalent = selected
        .equivalent_sources()
        .iter()
        .map(|source| source.id().clone())
        .collect::<Vec<_>>();
    assert!(matches!(
        equivalent.as_slice(),
        [CallableCandidateId::Environment(id)]
            if matches!(id.owner(), EnvironmentCallableOwner::Adapter(_))
    ));

    let selected_record = selected.primary().as_ref().clone();
    let equivalent_records = equivalent
        .iter()
        .map(|candidate| {
            let CallableCandidateId::Environment(id) = candidate else {
                unreachable!("equivalent environment source has an environment ID")
            };
            catalog
                .environment_record(id)
                .expect("equivalent by-ID record")
                .as_ref()
                .clone()
        })
        .collect();
    LookupSnapshot {
        selected: selected.primary().id().clone(),
        equivalent,
        selected_record,
        equivalent_records,
    }
}
