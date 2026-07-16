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

fn project_and_world(
    modules: &[(CanonicalModulePath, Arc<SourceDocument>)],
) -> (Arc<HirProject>, Arc<RegisteredSemanticWorld>) {
    let root = Arc::clone(&modules[0].1);
    let documents = modules
        .iter()
        .map(|(_, document)| Arc::clone(document))
        .collect::<Vec<_>>();
    let project = Arc::new(
        HirProject::new(
            "accepted-project-tests",
            modules
                .iter()
                .map(|(path, document)| module(path.clone(), document)),
        )
        .expect("HIR project"),
    );
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("accepted-project-tests").expect("package"),
        root.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(world, documents, Vec::new(), Vec::new())
        .expect("registration facts");
    let registered = Arc::new(
        CharacterRegistrar::register(CharacterRegistrationRequest::new(
            Arc::new(TypeCheckEnv::standard()),
            project.as_ref(),
            &facts,
            None,
        ))
        .expect("registered world"),
    );
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
        "flow @flow.main main { return \"ok\" }\n",
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
