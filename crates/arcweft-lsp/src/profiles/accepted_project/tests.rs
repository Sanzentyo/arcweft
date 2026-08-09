use super::*;
use arcweft_compiler::project::{
    CompiledProject, ProjectCompilationContext, ProjectCompilationSession, ProjectCompileError,
    compile_project,
};
use arcweft_lang_hir::symbol::{CallablePackageId, ProjectSymbolWorldId};
use arcweft_lang_sema::{env::TypeCheckEnv, registration::ProjectRegistrationFacts};
use arcweft_lang_syntax::{
    ast::module_path::ModuleSegment, incremental::SyntaxDatabase, parser::ParseOptions,
};
use arcweft_manifest_model::{BuildSpec, PackageId, PackageSpec, PackageVersion};
use arcweft_project::sources::{ProjectSourceFile, ProjectSources};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_source::{SourceName, identity::SourceSnapshotId};
use std::{collections::BTreeMap, path::PathBuf};

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

fn compiled_project(
    modules: &[(CanonicalModulePath, Arc<SourceDocument>)],
) -> Arc<CompiledProject> {
    Arc::new(project_compilation(modules).expect("compiled project"))
}

fn project_compilation(
    modules: &[(CanonicalModulePath, Arc<SourceDocument>)],
) -> Result<CompiledProject, ProjectCompileError> {
    let root = Arc::clone(&modules[0].1);
    let manifest = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-project://accepted/arcw.toml")
                .expect("manifest document ID"),
            SourceName::path("arcw.toml"),
            "",
        )
        .expect("manifest document"),
    );
    let sources = ProjectSources::new(
        PathBuf::from("arcw.toml"),
        PathBuf::new(),
        PackageSpec {
            id: PackageId::new("org.arcweft.accepted-project-tests").expect("package ID"),
            version: PackageVersion::new("0.0.0").expect("package version"),
        },
        BuildSpec::default(),
        manifest,
        modules.iter().map(|(module, document)| {
            ProjectSourceFile::new(
                module.clone(),
                PathBuf::from(format!("src/{module}.arcw")),
                Arc::clone(document),
                [],
            )
        }),
    )
    .expect("project sources");
    let world = ProjectSymbolWorldId::try_new(
        CallablePackageId::try_new("org.arcweft.accepted-project-tests").expect("package"),
        root.identity().id().clone(),
        "test",
    )
    .expect("world");
    let facts = ProjectRegistrationFacts::try_new(
        world,
        modules
            .iter()
            .map(|(_, document)| Arc::clone(document))
            .collect(),
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let context = ProjectCompilationContext::new(
        Arc::new(TypeCheckEnv::standard()),
        Arc::new(facts),
        Arc::new(ResourceTypeRegistry::empty()),
        None,
        None,
    );
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed_sources = modules
        .iter()
        .map(|(module, document)| {
            let parsed = syntax
                .parse_initial(
                    SourceSnapshotId::initial(document.display_name().clone()),
                    Arc::clone(document),
                    ParseOptions::default(),
                )
                .expect("attached project source");
            (module.clone(), parsed)
        })
        .collect::<BTreeMap<_, _>>();
    let mut compiler = ProjectCompilationSession::try_new().expect("HIR database");
    compile_project(&mut compiler, &sources, &parsed_sources, &context)
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
    let compiled = compiled_project(&[
        (CanonicalModulePath::crate_root(), Arc::clone(&root)),
        (dependency_path.clone(), Arc::clone(&dependency)),
    ]);
    let snapshot = AcceptedProjectSnapshot::try_new(
        Arc::clone(compiled.tooling_lease()),
        Some(compiled.as_ref()),
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

    assert!(Arc::ptr_eq(
        snapshot.tooling_lease(),
        compiled.tooling_lease()
    ));
    assert!(Arc::ptr_eq(snapshot.hir_project(), compiled.hir_project()));
    let root_key = snapshot
        .module_key(root.identity())
        .expect("root module key");
    assert_eq!(root_key.module(), &CanonicalModulePath::crate_root());
    assert!(Arc::ptr_eq(
        snapshot
            .hir(&root_key)
            .expect("root HIR")
            .provenance()
            .document(),
        &root
    ));
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
fn recovered_tooling_lease_retains_exact_source_hir_and_navigation_without_semantics() {
    let root = document("arcweft-project://accepted/recovered.arcw", "fn {\n");
    let error = project_compilation(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))])
        .expect_err("recovered source is not executable");
    let tooling = error
        .tooling_lease()
        .cloned()
        .expect("post-HIR failure retains the exact tooling lease");
    let uri_text = "file:///accepted/recovered.arcw";
    let uri = uri_text.parse::<Uri>().expect("recovered URI");
    let snapshot = AcceptedProjectSnapshot::try_new(
        Arc::clone(&tooling),
        None,
        vec![seed(Arc::clone(&root), uri_text)],
    )
    .expect("recovered tooling snapshot");

    assert!(Arc::ptr_eq(snapshot.tooling_lease(), &tooling));
    assert!(Arc::ptr_eq(snapshot.hir_project(), tooling.hir_project()));
    let key = snapshot
        .module_key(root.identity())
        .expect("recovered source keeps its canonical module");
    assert!(
        snapshot
            .parsed_source(&key)
            .is_some_and(|parsed| !parsed.diagnostics().is_empty())
    );
    assert!(Arc::ptr_eq(
        snapshot.hir(&key).expect("recovered HIR remains navigable"),
        tooling
            .hir_project()
            .view()
            .module(&CanonicalModulePath::crate_root())
            .expect("tooling HIR root"),
    ));
    assert!(snapshot.hir_for_open_document(&uri, &root).is_some());
    assert!(snapshot.entry_references().is_empty());
    assert!(snapshot.sources().character_source_revision().is_none());
}

#[test]
fn duplicate_identity_and_uri_are_rejected_without_overwrite() {
    let root = document(
        "arcweft-project://accepted/duplicate.arcw",
        "flow @flow.main main {}\n",
    );
    let compiled = compiled_project(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let duplicate = AcceptedProjectSnapshot::try_new(
        Arc::clone(compiled.tooling_lease()),
        Some(compiled.as_ref()),
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
        Arc::clone(compiled.tooling_lease()),
        Some(compiled.as_ref()),
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
    let compiled = compiled_project(&[(CanonicalModulePath::crate_root(), Arc::clone(&first))]);

    let result = AcceptedProjectSnapshot::try_new(
        Arc::clone(compiled.tooling_lease()),
        Some(compiled.as_ref()),
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
    let compiled = compiled_project(&[(CanonicalModulePath::crate_root(), Arc::clone(&root))]);
    let snapshot = AcceptedProjectSnapshot::try_new(
        Arc::clone(compiled.tooling_lease()),
        Some(compiled.as_ref()),
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
