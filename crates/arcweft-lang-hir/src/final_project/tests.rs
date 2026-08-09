use std::sync::Arc;

use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use super::{
    HirProject, HirProjectError, HirProjectExecutionError, HirProjectModule, HirProjectModuleError,
    exported_parts, styles,
};
use crate::database::HirDatabase;
use crate::final_lowering::stage_unpublished_module_for_invariant_test;
use crate::item::{HirDeclarationMemberKind, HirItemKind};
use crate::lowering::{HirModuleKey, LoweringRequest};
use crate::module::HirModule;
use crate::symbol::CallablePackageId;

fn package() -> CallablePackageId {
    CallablePackageId::try_new("proof-final-project-tests").unwrap()
}

fn source_document(id: &str, path: &str, source: &str) -> Arc<SourceDocument> {
    Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).unwrap(),
            SourceName::path(path),
            source,
        )
        .unwrap(),
    )
}

fn parse_initial(syntax: &mut SyntaxDatabase, id: &str, path: &str, source: &str) -> ParsedSource {
    let name = SourceName::path(path);
    syntax
        .parse_initial(
            SourceSnapshotId::initial(name),
            source_document(id, path, source),
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap()
}

fn lower(
    database: &mut HirDatabase,
    parsed: &ParsedSource,
    package: &CallablePackageId,
    path: &CanonicalModulePath,
) -> Arc<HirModule> {
    let key = HirModuleKey::new(
        package.clone(),
        path.clone(),
        parsed.document().identity().clone(),
    );
    let mut transaction = stage_unpublished_module_for_invariant_test(
        database,
        LoweringRequest::try_new(key, parsed).unwrap(),
        crate::lowering::HirLoweringControl::new(),
    )
    .unwrap();
    transaction.lower_parsed_source_items(parsed).unwrap();
    transaction.finish(database).unwrap().into_module()
}

fn bind(
    database: &HirDatabase,
    package: &CallablePackageId,
    path: &CanonicalModulePath,
    module: Arc<HirModule>,
) -> HirProjectModule {
    let source_identity = module.provenance().source_identity().clone();
    HirProjectModule::try_new(database, package, path, &source_identity, module).unwrap()
}

fn root_module_fixture(
    label: &str,
) -> (
    HirDatabase,
    CallablePackageId,
    CanonicalModulePath,
    Arc<HirModule>,
) {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        &format!("arcweft-test://proof/final-project/{label}"),
        &format!("{label}.arcw"),
        "fn accepted() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    (database, package, root_path, module)
}

#[test]
fn project_module_rejects_package_mismatch() {
    let (database, package, root_path, module) = root_module_fixture("wrong-package");
    let wrong_package = CallablePackageId::try_new("another-package").unwrap();
    let retained = Arc::clone(&module);
    assert!(matches!(
        HirProjectModule::try_new(
            &database,
            &wrong_package,
            &root_path,
            module.provenance().source_identity(),
            Arc::clone(&module),
        ),
        Err(HirProjectModuleError::WrongPackage {
            expected,
            actual,
        }) if expected == wrong_package && actual == package
    ));
    assert!(Arc::ptr_eq(&module, &retained));
}

#[test]
fn project_module_rejects_path_mismatch() {
    let (database, package, root_path, module) = root_module_fixture("wrong-path");
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let retained = Arc::clone(&module);
    assert!(matches!(
        HirProjectModule::try_new(
            &database,
            &package,
            &child_path,
            module.provenance().source_identity(),
            Arc::clone(&module),
        ),
        Err(HirProjectModuleError::WrongPath {
            expected,
            actual,
        }) if expected == child_path && actual == root_path
    ));
    assert!(Arc::ptr_eq(&module, &retained));
}

#[test]
fn project_module_rejects_source_mismatch() {
    let (database, package, root_path, module) = root_module_fixture("wrong-source");
    let expected = source_document(
        "arcweft-test://proof/final-project/wrong-source",
        "wrong-source.arcw",
        "fn changed() {}\n",
    )
    .identity()
    .clone();
    let actual = module.provenance().source_identity().clone();
    let retained = Arc::clone(&module);
    assert_eq!(
        HirProjectModule::try_new(
            &database,
            &package,
            &root_path,
            &expected,
            Arc::clone(&module),
        )
        .err(),
        Some(HirProjectModuleError::WrongSource {
            module: root_path,
            expected,
            actual,
        })
    );
    assert!(Arc::ptr_eq(&module, &retained));
}

#[test]
fn project_requires_canonical_root_module() {
    let (mut database, package, root_path, _) = root_module_fixture("missing-root-seed");
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/missing-root-child",
        "missing-root-child.arcw",
        "fn child() {}\n",
    );
    let child = lower(&mut database, &child_source, &package, &child_path);
    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [bind(&database, &package, &child_path, child)],
        )
        .err(),
        Some(HirProjectError::MissingRootModule)
    );
}

#[test]
fn project_rejects_duplicate_path_and_source() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/duplicate-owner",
        "duplicate-owner.arcw",
        "fn shared() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &parsed, &package, &root_path);
    let child = lower(&mut database, &parsed, &package, &child_path);

    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &root_path, Arc::clone(&root)),
            ],
        )
        .err(),
        Some(HirProjectError::DuplicateModule {
            module: root_path.clone(),
        })
    );
    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, root),
                bind(&database, &package, &child_path, child),
            ],
        )
        .err(),
        Some(HirProjectError::DuplicateSourceDocument {
            document: parsed.document().identity().id().clone(),
            first: root_path,
            second: child_path,
        })
    );
}

#[test]
fn ordered_project_iteration_preserves_module_ids() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/root",
        "root.arcw",
        "fn root_first() {}\nfn root_second() {}\n",
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/child",
        "child.arcw",
        "fn child() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    let root_items = root.source_ordered_items().to_vec();
    assert_eq!(root_items.len(), 2);
    let child_item = child.source_ordered_items()[0];

    let project = HirProject::try_new(
        &database,
        package.clone(),
        [
            bind(&database, &package, &child_path, Arc::clone(&child)),
            bind(&database, &package, &root_path, Arc::clone(&root)),
        ],
    )
    .unwrap();

    assert_eq!(project.package(), &package);
    assert_eq!(project.database_id(), database.database_id());
    assert!(Arc::ptr_eq(
        project.module(&root_path).unwrap().module(),
        &root
    ));
    assert!(Arc::ptr_eq(
        project.module(&child_path).unwrap().module(),
        &child
    ));
    assert_eq!(
        project
            .view()
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [root_path.clone(), child_path.clone()]
    );
    assert_eq!(
        project
            .view()
            .items()
            .map(|item| (item.module_path().clone(), item.id()))
            .collect::<Vec<_>>(),
        [
            (root_path.clone(), root_items[0]),
            (root_path.clone(), root_items[1]),
            (child_path, child_item),
        ]
    );
    for (projected, expected) in project.view().items().take(2).zip(root_items) {
        assert_eq!(projected.id(), expected);
        assert!(std::ptr::eq(
            projected.item(),
            root.resolve_item(expected).unwrap(),
        ));
    }

    let executable = project.executable_view().unwrap();
    assert_eq!(executable.modules().len(), 2);
    assert_eq!(executable.items().count(), 3);
}

#[test]
fn module_binding_rejects_identity_mismatch_and_stale_arc() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let first = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/stale",
        "stale.arcw",
        "fn first() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let first_module = lower(&mut database, &first, &package, &root_path);
    let first_snapshot = first_module.snapshot_id();

    let wrong_package = CallablePackageId::try_new("another-package").unwrap();
    assert!(matches!(
        HirProjectModule::try_new(
            &database,
            &wrong_package,
            &root_path,
            first_module.provenance().source_identity(),
            Arc::clone(&first_module),
        ),
        Err(HirProjectModuleError::WrongPackage { .. })
    ));
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    assert!(matches!(
        HirProjectModule::try_new(
            &database,
            &package,
            &child_path,
            first_module.provenance().source_identity(),
            Arc::clone(&first_module),
        ),
        Err(HirProjectModuleError::WrongPath { .. })
    ));
    let different_revision = source_document(
        "arcweft-test://proof/final-project/stale",
        "stale.arcw",
        "fn first_with_different_text() {}\n",
    );
    let expected_source = different_revision.identity().clone();
    let actual_source = first_module.provenance().source_identity().clone();
    assert_eq!(
        HirProjectModule::try_new(
            &database,
            &package,
            &root_path,
            &expected_source,
            Arc::clone(&first_module),
        )
        .err(),
        Some(HirProjectModuleError::WrongSource {
            module: root_path.clone(),
            expected: expected_source,
            actual: actual_source,
        })
    );
    let foreign_database = HirDatabase::try_new().unwrap();
    assert!(matches!(
        HirProjectModule::try_new(
            &foreign_database,
            &package,
            &root_path,
            first_module.provenance().source_identity(),
            Arc::clone(&first_module),
        ),
        Err(HirProjectModuleError::WrongDatabase { .. })
    ));

    let edited = syntax
        .reparse(
            &first,
            &[SourceEdit::new(
                first.document().span(SourceRange::new(3, 8)).unwrap(),
                "second",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_module = lower(&mut database, &edited, &package, &root_path);
    assert!(!Arc::ptr_eq(&first_module, &second_module));
    let second_snapshot = second_module.snapshot_id();
    let first_source_identity = first_module.provenance().source_identity().clone();
    assert_eq!(
        HirProjectModule::try_new(
            &database,
            &package,
            &root_path,
            &first_source_identity,
            first_module,
        )
        .err(),
        Some(HirProjectModuleError::StaleModuleLease {
            module: root_path,
            current: second_snapshot,
            supplied: first_snapshot,
        })
    );
}

#[test]
fn accepted_project_generation_remains_bound_to_its_original_exact_arc() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let first = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/immutable",
        "immutable.arcw",
        "fn first() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let first_module = lower(&mut database, &first, &package, &root_path);
    let first_snapshot = first_module.snapshot_id();
    let future_stale = bind(&database, &package, &root_path, Arc::clone(&first_module));
    let project = HirProject::try_new(
        &database,
        package.clone(),
        [bind(
            &database,
            &package,
            &root_path,
            Arc::clone(&first_module),
        )],
    )
    .unwrap();

    let edited = syntax
        .reparse(
            &first,
            &[SourceEdit::new(
                first.document().span(SourceRange::new(3, 8)).unwrap(),
                "second",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_module = lower(&mut database, &edited, &package, &root_path);
    let second_snapshot = second_module.snapshot_id();

    let retained = project.module(&root_path).unwrap().module();
    assert!(Arc::ptr_eq(retained, &first_module));
    assert!(!Arc::ptr_eq(retained, &second_module));
    assert_eq!(retained.snapshot_id(), first_snapshot);
    assert_eq!(project.executable_view().unwrap().modules().len(), 1);
    assert_eq!(
        HirProject::try_new(&database, package, [future_stale]).err(),
        Some(HirProjectError::StaleModuleLease {
            module: root_path,
            current: second_snapshot,
            supplied: first_snapshot,
        })
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one project-admission matrix covers duplicates, limits, stale leases, and mixed databases"
)]
fn project_rejects_duplicates_limit_and_mixed_database() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/shared-source",
        "shared.arcw",
        "fn shared() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &parsed, &package, &root_path);
    let child = lower(&mut database, &parsed, &package, &child_path);

    assert!(matches!(
        HirProject::try_new(
            &database,
            CallablePackageId::try_new("wrong-project-package").unwrap(),
            [bind(&database, &package, &root_path, Arc::clone(&root),)],
        ),
        Err(HirProjectError::WrongPackage { .. })
    ));
    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [bind(&database, &package, &child_path, Arc::clone(&child),)],
        )
        .err(),
        Some(HirProjectError::MissingRootModule)
    );

    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &root_path, Arc::clone(&root)),
            ],
        )
        .err(),
        Some(HirProjectError::DuplicateModule {
            module: root_path.clone(),
        })
    );
    assert_eq!(
        HirProject::try_new(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &child_path, Arc::clone(&child)),
            ],
        )
        .err(),
        Some(HirProjectError::DuplicateSourceDocument {
            document: parsed.document().identity().id().clone(),
            first: root_path.clone(),
            second: child_path.clone(),
        })
    );

    let mut distinct_syntax = SyntaxDatabase::try_new().unwrap();
    let distinct_parsed = parse_initial(
        &mut distinct_syntax,
        "arcweft-test://proof/final-project/distinct-child",
        "distinct-child.arcw",
        "fn distinct_child() {}\n",
    );
    let distinct_child = lower(&mut database, &distinct_parsed, &package, &child_path);
    assert!(
        HirProject::try_new_with_limit(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(
                    &database,
                    &package,
                    &child_path,
                    Arc::clone(&distinct_child),
                ),
            ],
            2,
        )
        .is_ok()
    );
    assert_eq!(
        HirProject::try_new_with_limit(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(
                    &database,
                    &package,
                    &child_path,
                    Arc::clone(&distinct_child),
                ),
            ],
            1,
        )
        .err(),
        Some(HirProjectError::ModuleLimit {
            observed: 2,
            maximum: 1
        })
    );

    let mut foreign_syntax = SyntaxDatabase::try_new().unwrap();
    let foreign_parsed = parse_initial(
        &mut foreign_syntax,
        "arcweft-test://proof/final-project/foreign",
        "foreign.arcw",
        "fn foreign() {}\n",
    );
    let mut foreign_database = HirDatabase::try_new().unwrap();
    let foreign = lower(
        &mut foreign_database,
        &foreign_parsed,
        &package,
        &child_path,
    );
    assert!(matches!(
        HirProject::try_new(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, root),
                bind(&foreign_database, &package, &child_path, foreign),
            ],
        ),
        Err(HirProjectError::WrongDatabase { .. })
    ));
}

#[test]
fn project_view_allows_recovered_but_executable_view_rejects_first_canonical() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let first_recovered_path = root_path.join(ModuleSegment::new("a_recovered").unwrap());
    let last_recovered_path = root_path.join(ModuleSegment::new("z_recovered").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/recovered-root",
        "recovered-root.arcw",
        "fn clean() {}\n",
    );
    let mut first_syntax = SyntaxDatabase::try_new().unwrap();
    let first_source = parse_initial(
        &mut first_syntax,
        "arcweft-test://proof/final-project/recovered-first",
        "recovered-first.arcw",
        "fn first_missing()\n",
    );
    let mut last_syntax = SyntaxDatabase::try_new().unwrap();
    let last_source = parse_initial(
        &mut last_syntax,
        "arcweft-test://proof/final-project/recovered-last",
        "recovered-last.arcw",
        "fn last_missing()\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let first_recovered = lower(
        &mut database,
        &first_source,
        &package,
        &first_recovered_path,
    );
    let last_recovered = lower(&mut database, &last_source, &package, &last_recovered_path);
    assert!(root.is_executable());
    assert!(!first_recovered.is_executable());
    assert!(!last_recovered.is_executable());
    let first_snapshot = first_recovered.snapshot_id();
    let project = HirProject::try_new(
        &database,
        package.clone(),
        [
            bind(&database, &package, &last_recovered_path, last_recovered),
            bind(&database, &package, &root_path, root),
            bind(&database, &package, &first_recovered_path, first_recovered),
        ],
    )
    .unwrap();

    assert_eq!(
        project
            .view()
            .modules()
            .map(|(path, _)| path.clone())
            .collect::<Vec<_>>(),
        [root_path, first_recovered_path.clone(), last_recovered_path,]
    );
    assert_eq!(project.view().items().count(), 3);
    assert_eq!(
        project.executable_view().err(),
        Some(HirProjectExecutionError::RecoveredModule {
            module: first_recovered_path,
            snapshot: first_snapshot,
        })
    );
}

fn project_with_view_exports_and_styles() -> (
    HirProject,
    Arc<HirModule>,
    Arc<HirModule>,
    CanonicalModulePath,
    CanonicalModulePath,
) {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/projections-root",
        "root.arcw",
        concat!(
            "pub view Root() {\n",
            "    export part panel as public.panel\n",
            "}\n",
            "pub style root_theme {}\n",
        ),
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/projections-child",
        "child.arcw",
        concat!(
            "pub view Child() {\n",
            "    export part content as public.content\n",
            "}\n",
            "pub style child_theme {}\n",
        ),
    );
    assert!(root_source.diagnostics().is_empty());
    assert!(child_source.diagnostics().is_empty());

    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    let project = HirProject::try_new(
        &database,
        package.clone(),
        [
            bind(&database, &package, &child_path, Arc::clone(&child)),
            bind(&database, &package, &root_path, Arc::clone(&root)),
        ],
    )
    .unwrap();
    (project, root, child, root_path, child_path)
}

#[test]
fn exported_parts_iterate_without_flattening() {
    let (project, root, child, root_path, child_path) = project_with_view_exports_and_styles();
    let root_item = root.source_ordered_items()[0];
    let child_item = child.source_ordered_items()[0];
    let projected = exported_parts(project.view()).collect::<Vec<_>>();

    assert_eq!(projected.len(), 2);
    assert_eq!(projected[0].module_path(), &root_path);
    assert_eq!(projected[0].item(), root_item);
    assert_eq!(projected[0].member().item(), root_item);
    assert_eq!(projected[0].member().ordinal(), 0);
    assert_eq!(projected[1].module_path(), &child_path);
    assert_eq!(projected[1].item(), child_item);
    assert_eq!(projected[1].member().item(), child_item);
    assert_eq!(projected[1].member().ordinal(), 0);

    for (module, part) in [(&root, projected[0]), (&child, projected[1])] {
        let member = module.declaration_members().resolve(part.member()).unwrap();
        let HirDeclarationMemberKind::ViewExport(expected) = member.kind() else {
            panic!("projected View member changed family")
        };
        assert!(std::ptr::eq(part.part(), expected));
    }
}

#[test]
fn styles_iterate_without_flattening() {
    let (project, root, child, root_path, child_path) = project_with_view_exports_and_styles();
    let root_item = root.source_ordered_items()[1];
    let child_item = child.source_ordered_items()[1];
    let projected = styles(project.view()).collect::<Vec<_>>();

    assert_eq!(projected.len(), 2);
    assert_eq!(projected[0].module_path(), &root_path);
    assert_eq!(projected[0].item(), root_item);
    assert_eq!(projected[1].module_path(), &child_path);
    assert_eq!(projected[1].item(), child_item);

    for (module, style) in [(&root, projected[0]), (&child, projected[1])] {
        let item = module.resolve_item(style.item()).unwrap();
        let HirItemKind::Style(expected) = item.kind() else {
            panic!("projected Style item changed family")
        };
        assert!(std::ptr::eq(style.style(), expected));
    }
}
