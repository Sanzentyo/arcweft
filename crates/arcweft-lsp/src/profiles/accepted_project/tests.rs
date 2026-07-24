use super::*;
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::HirProjectModule,
    symbol::{CallablePackageId, ProjectSymbolWorldId},
};
use arcweft_lang_sema::{
    env::TypeCheckEnv,
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
};
use arcweft_lang_syntax::{ast::module_path::ModuleSegment, parser::parse_source};
use arcweft_source::SourceName;

mod production_limits;

fn document(id: &str, text: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("document ID"),
            SourceName::path(id),
            text,
        )
        .expect("source document"),
    )
}

fn module(path: CanonicalModulePath, document: &Arc<SourceDocument>) -> HirProjectModule {
    let parsed = parse_source(document.text());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(document, parsed.typed_tree()).expect("lowered HIR");
    HirProjectModule::try_new(path, document.identity().clone(), hir).expect("source-bound module")
}

fn hir_project(modules: &[(CanonicalModulePath, Arc<SourceDocument>)]) -> Arc<HirProject> {
    Arc::new(
        HirProject::new(
            "accepted-project-tests",
            modules
                .iter()
                .map(|(path, document)| module(path.clone(), document)),
        )
        .expect("HIR project"),
    )
}

fn registered_world(
    project: &HirProject,
    root: &SourceDocument,
    documents: Vec<Arc<SourceDocument>>,
) -> Arc<RegisteredSemanticWorld> {
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("accepted-project-tests").expect("package"),
        root.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts =
        ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new(), Vec::new())
            .expect("registration facts");
    Arc::new(
        CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            project,
            &facts,
            None,
        ))
        .expect("registered world"),
    )
}

fn project_and_world(
    modules: &[(CanonicalModulePath, Arc<SourceDocument>)],
) -> (Arc<HirProject>, Arc<RegisteredSemanticWorld>) {
    let root = Arc::clone(&modules[0].1);
    let documents = modules
        .iter()
        .map(|(_, document)| Arc::clone(document))
        .collect::<Vec<_>>();
    let project = hir_project(modules);
    let registered = registered_world(project.as_ref(), root.as_ref(), documents);
    (project, registered)
}

fn seed(document: Arc<SourceDocument>, uri: &str) -> AcceptedSourceDocumentSeed {
    AcceptedSourceDocumentSeed::new(
        document,
        AcceptedSourceLocator::Uri {
            uri: uri.parse::<Uri>().expect("URI"),
        },
        AcceptedSourceOwnership::Workspace,
        AcceptedSourceAccess::Writable,
    )
}

#[test]
fn exact_root_dependency_and_declaration_free_hir_are_retained() {
    let root = document(
        "arcweft-project://accepted/root.arcw",
        "flow @flow.main main() -> String { return \"ok\" }\n",
    );
    let dependency = document("arcweft-project://accepted/empty.arcw", "\n");
    let dependency_path = CanonicalModulePath::from_segments([
        ModuleSegment::new("dependency").expect("dependency segment")
    ]);
    let (hir, world) = project_and_world(&[
        (CanonicalModulePath::crate_root(), Arc::clone(&root)),
        (dependency_path.clone(), Arc::clone(&dependency)),
    ]);
    let snapshot = AcceptedProjectSnapshot::try_new(
        Arc::clone(&hir),
        world.as_ref(),
        vec![
            seed(Arc::clone(&root), "file:///accepted/root.arcw"),
            AcceptedSourceDocumentSeed::new(
                Arc::clone(&dependency),
                AcceptedSourceLocator::Uri {
                    uri: "arcweft-dependency:///empty.arcw"
                        .parse::<Uri>()
                        .expect("dependency URI"),
                },
                AcceptedSourceOwnership::Dependency,
                AcceptedSourceAccess::ReadOnly,
            ),
        ],
    )
    .expect("accepted snapshot");

    assert!(Arc::ptr_eq(snapshot.hir_project(), &hir));
    let root_key = snapshot
        .module_key(root.identity())
        .expect("root module key");
    assert_eq!(root_key.module(), &CanonicalModulePath::crate_root());
    assert_eq!(
        snapshot.hir(&root_key).expect("root HIR").source_document(),
        Some(root.as_ref())
    );
    let dependency_key = snapshot
        .module_key(dependency.identity())
        .expect("dependency module key");
    assert_eq!(dependency_key.module(), &dependency_path);
    assert_eq!(
        snapshot
            .source(dependency.identity())
            .expect("dependency source")
            .ownership(),
        AcceptedSourceOwnership::Dependency
    );
    assert_eq!(
        snapshot
            .source(dependency.identity())
            .expect("dependency source")
            .access(),
        AcceptedSourceAccess::ReadOnly
    );
    assert_eq!(snapshot.footprint().documents(), 2);
    assert_eq!(snapshot.footprint().modules(), 2);
    assert_eq!(
        snapshot.footprint().source_bytes(),
        (root.text().len() + dependency.text().len()) as u64
    );
}

#[test]
fn duplicate_identity_and_uri_are_rejected_without_overwrite() {
    let root = document(
        "arcweft-project://accepted/duplicate.arcw",
        "flow @flow.main main {}\n",
    );
    let (hir, world) = project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let duplicate = AcceptedProjectSnapshot::try_new(
        Arc::clone(&hir),
        world.as_ref(),
        vec![
            seed(Arc::clone(&root), "file:///accepted/duplicate.arcw"),
            seed(Arc::clone(&root), "file:///accepted/duplicate-again.arcw"),
        ],
    );
    assert!(matches!(
        duplicate,
        Err(AcceptedProjectSnapshotError::DuplicateSourceIdentity { .. })
    ));

    let extra = document("arcweft-generated://accepted/extra.arcw", "\n");
    let duplicate_uri = AcceptedProjectSnapshot::try_new(
        hir,
        world.as_ref(),
        vec![
            seed(root, "file:///accepted/shared.arcw"),
            AcceptedSourceDocumentSeed::new(
                extra,
                AcceptedSourceLocator::Uri {
                    uri: "file:///accepted/shared.arcw".parse::<Uri>().expect("URI"),
                },
                AcceptedSourceOwnership::Generated,
                AcceptedSourceAccess::ReadOnly,
            ),
        ],
    );
    assert!(matches!(
        duplicate_uri,
        Err(AcceptedProjectSnapshotError::DuplicateUri { .. })
    ));
}

#[test]
fn conflicting_source_id_reports_both_exact_revisions() {
    let first = document("arcweft-project://accepted/conflicting.arcw", "\n");
    let conflicting = document(
        "arcweft-project://accepted/conflicting.arcw",
        "// another revision\n",
    );
    assert_ne!(first.identity(), conflicting.identity());
    assert_ne!(
        first.identity().source_len(),
        conflicting.identity().source_len()
    );
    let (hir, world) =
        project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&first))]);

    let result = AcceptedProjectSnapshot::try_new(
        hir,
        world.as_ref(),
        vec![
            seed(Arc::clone(&first), "file:///accepted/conflicting.arcw"),
            seed(
                Arc::clone(&conflicting),
                "file:///accepted/conflicting-again.arcw",
            ),
        ],
    );

    let Err(AcceptedProjectSnapshotError::ConflictingSourceId {
        id,
        first: actual_first,
        conflicting: actual_conflicting,
    }) = result
    else {
        panic!("expected a typed conflicting-source-ID rejection");
    };
    assert_eq!(&id, first.identity().id());
    assert_eq!(&actual_first, first.identity());
    assert_eq!(&actual_conflicting, conflicting.identity());
}

#[test]
fn one_source_bound_to_two_modules_reports_conflicting_mapping() {
    let shared = document("arcweft-project://accepted/shared.arcw", "\n");
    let child = CanonicalModulePath::from_segments([
        ModuleSegment::new("child").expect("child module segment")
    ]);
    let project = hir_project(&[
        (CanonicalModulePath::crate_root(), Arc::clone(&shared)),
        (child.clone(), Arc::clone(&shared)),
    ]);
    let world = registered_world(project.as_ref(), shared.as_ref(), vec![Arc::clone(&shared)]);

    let result = AcceptedProjectSnapshot::try_new(
        project,
        world.as_ref(),
        vec![seed(Arc::clone(&shared), "file:///accepted/shared.arcw")],
    );

    let Err(AcceptedProjectSnapshotError::ConflictingModuleMapping {
        source,
        first,
        conflicting,
    }) = result
    else {
        panic!("expected a typed conflicting-module-mapping rejection");
    };
    assert_eq!(&source, shared.identity());
    assert_eq!(first, CanonicalModulePath::crate_root());
    assert_eq!(conflicting, child);
}

#[test]
fn hir_and_symbol_module_inventory_difference_is_exact() {
    let root = document("arcweft-project://accepted/inventory-root.arcw", "\n");
    let child_document = document("arcweft-project://accepted/inventory-child.arcw", "\n");
    let child = CanonicalModulePath::from_segments([
        ModuleSegment::new("child").expect("child module segment")
    ]);
    let hir = hir_project(&[
        (CanonicalModulePath::crate_root(), Arc::clone(&root)),
        (child.clone(), Arc::clone(&child_document)),
    ]);
    let symbol_project = hir_project(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let world = registered_world(
        symbol_project.as_ref(),
        root.as_ref(),
        vec![Arc::clone(&root)],
    );

    let result = AcceptedProjectSnapshot::try_new(
        hir,
        world.as_ref(),
        vec![
            seed(Arc::clone(&root), "file:///accepted/inventory-root.arcw"),
            seed(child_document, "file:///accepted/inventory-child.arcw"),
        ],
    );

    let Err(AcceptedProjectSnapshotError::ModuleInventoryMismatch {
        hir_only,
        symbol_only,
    }) = result
    else {
        panic!("expected a typed module-inventory rejection");
    };
    assert_eq!(&*hir_only, &[child]);
    assert!(symbol_only.is_empty());
}

#[test]
fn project_hir_and_symbol_source_identity_difference_is_exact() {
    let project_document = document("arcweft-project://accepted/source-mismatch.arcw", "\n");
    let symbol_document = document(
        "arcweft-project://accepted/source-mismatch.arcw",
        "// symbol revision\n",
    );
    let hir = hir_project(&[(
        CanonicalModulePath::crate_root(),
        Arc::clone(&project_document),
    )]);
    let symbol_project = hir_project(&[(
        CanonicalModulePath::crate_root(),
        Arc::clone(&symbol_document),
    )]);
    let world = registered_world(
        symbol_project.as_ref(),
        symbol_document.as_ref(),
        vec![Arc::clone(&symbol_document)],
    );

    let result = AcceptedProjectSnapshot::try_new(
        hir,
        world.as_ref(),
        vec![seed(
            Arc::clone(&project_document),
            "file:///accepted/source-mismatch.arcw",
        )],
    );

    let Err(AcceptedProjectSnapshotError::ModuleSourceMismatch {
        module,
        project,
        hir,
        symbols,
    }) = result
    else {
        panic!("expected a typed module-source rejection");
    };
    assert_eq!(module, CanonicalModulePath::crate_root());
    assert_eq!(&project, project_document.identity());
    assert_eq!(&hir, project_document.identity());
    assert_eq!(&symbols, symbol_document.identity());
}

#[test]
fn equal_identity_with_unequal_hir_text_is_rejected_explicitly() {
    let source = document(
        "arcweft-project://accepted/text-collision.arcw",
        "accepted\n",
    );
    let module = CanonicalModulePath::crate_root();

    let result = validate_bound_hir_source(
        &module,
        source.identity(),
        source.identity(),
        "different\n",
        source.text(),
    );

    let Err(AcceptedProjectSnapshotError::HirTextMismatch {
        module: actual_module,
        source: actual_source,
    }) = result
    else {
        panic!("expected a typed HIR-text rejection");
    };
    assert_eq!(actual_module, module);
    assert_eq!(&actual_source, source.identity());
}

#[test]
fn accepted_generated_source_without_module_is_not_forged_into_hir() {
    let root = document(
        "arcweft-project://accepted/main.arcw",
        "flow @flow.main main {}\n",
    );
    let generated = document("arcweft-generated://accepted/index.arcw", "\n");
    let (hir, world) = project_and_world(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let snapshot = AcceptedProjectSnapshot::try_new(
        hir,
        world.as_ref(),
        vec![
            seed(root, "file:///accepted/main.arcw"),
            AcceptedSourceDocumentSeed::new(
                Arc::clone(&generated),
                AcceptedSourceLocator::Uri {
                    uri: "arcweft-generated:///index.arcw"
                        .parse::<Uri>()
                        .expect("generated URI"),
                },
                AcceptedSourceOwnership::Generated,
                AcceptedSourceAccess::ReadOnly,
            ),
        ],
    )
    .expect("accepted generated source");
    assert!(snapshot.source(generated.identity()).is_some());
    assert!(snapshot.module_key(generated.identity()).is_none());
    assert_eq!(snapshot.sources().documents().len(), 2);
}
