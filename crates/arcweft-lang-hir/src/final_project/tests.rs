use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use arcweft_id::dialogue::DialogueLineId;
use arcweft_lang_syntax::ast::module_path::{CanonicalModulePath, ModuleSegment};
use arcweft_lang_syntax::incremental::{ParsedSource, SyntaxDatabase};
use arcweft_source::identity::SourceSnapshotId;
use arcweft_source::{SourceDocument, SourceDocumentId, SourceEdit, SourceName, SourceRange};

use super::{
    HirDeclarationBodyRootRole, HirExecutableProjectView, HirPackageModuleKey, HirProject,
    HirProjectBuildError, HirProjectBuilder, HirProjectExecutionError, HirProjectModule,
    HirProjectModuleError, HirRuntimeCallCalleeDisposition, HirRuntimeEmissionMode,
    HirRuntimeExecutableOwner, HirRuntimeExpressionTypeDisposition, HirRuntimeReachabilityEdge,
    HirRuntimeReachabilityError, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeSemanticReachability, HirRuntimeSemanticReachabilityInput,
    HirSelectedExpressionInventoryError, HirSemanticPathError, HirSemanticPathStep, exported_parts,
    styles,
};
use crate::body_edges::HirBodyChild;
use crate::database::HirDatabase;
use crate::dialogue_application::HirPostfixBracketCandidates;
use crate::expr::{HirExprKind, HirExpressionOwnedChild, HirThreadFlowItem};
use crate::final_lowering::stage_unpublished_module_for_invariant_test;
use crate::identity::ExprId;
use crate::item::{HirDeclarationMemberKind, HirItemKind};
use crate::line_identity::{DialogueLineDiagnostic, DialogueLineIdOrigin, DialogueTextKeyOrigin};
use crate::lowering::{HirModuleKey, LoweringRequest};
use crate::module::HirModule;
use crate::source_index::HirCallableSourceOwner;
use crate::symbol::{
    CallableDeclarationId, CallableDeclarationKey, CallableDeclarationOwner, CallablePackageId,
    ProjectExternalDeclarations, ProjectSymbolRevision, ProjectSymbolTable, ProjectSymbolWorldId,
};

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

fn build_project(
    database: &HirDatabase,
    package: CallablePackageId,
    modules: impl IntoIterator<Item = HirProjectModule>,
) -> Result<HirProject, HirProjectBuildError> {
    let mut builder = HirProjectBuilder::new(database, package);
    for module in modules {
        builder.insert_module(module)?;
    }
    builder.finish()
}

fn build_project_with_limit(
    database: &HirDatabase,
    package: CallablePackageId,
    modules: impl IntoIterator<Item = HirProjectModule>,
    maximum: usize,
) -> Result<HirProject, HirProjectBuildError> {
    let mut builder = HirProjectBuilder::new(database, package).with_module_limit_for_test(maximum);
    for module in modules {
        builder.insert_module(module)?;
    }
    builder.finish()
}

fn runtime_reachability(
    executable: HirExecutableProjectView<'_>,
    selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
    call_disposition: impl FnMut(ExprId) -> HirRuntimeExpressionTypeDisposition,
) -> Result<HirRuntimeSemanticReachability<'_>, HirRuntimeReachabilityError> {
    let root_document = executable
        .modules()
        .next()
        .expect("test project has one module")
        .1
        .provenance()
        .source_identity()
        .id()
        .clone();
    let world = ProjectSymbolWorldId::try_new(
        executable.package().clone(),
        root_document,
        "final-project-tests",
    )
    .unwrap();
    let revision = ProjectSymbolRevision::try_for_documents(
        executable
            .modules()
            .map(|(_, module)| module.provenance().source_identity()),
    )
    .unwrap();
    let roots = executable
        .items()
        .filter_map(|item| {
            let kind = match item.item().kind() {
                HirItemKind::Flow(_) => HirRuntimeReachabilityRootKind::CheckedFlow,
                HirItemKind::Entry(_) => HirRuntimeReachabilityRootKind::CheckedEntry,
                _ => return None,
            };
            Some(HirRuntimeReachabilityRoot::new(
                kind,
                HirRuntimeExecutableOwner::Item(item.id()),
            ))
        })
        .collect();
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        HirRuntimeEmissionMode::CheckAll,
        world,
        revision,
        roots,
        Vec::new(),
    )?;
    executable.runtime_semantic_reachability(input, selected_postfix, call_disposition)
}

pub(super) fn root_module_fixture(
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
        "fn accepted() { let value = 1 }\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    (database, package, root_path, module)
}

fn symbols_for_project(
    project: &HirProject,
    root_document: &SourceDocument,
    profile: &str,
) -> ProjectSymbolTable {
    let world = ProjectSymbolWorldId::try_new(
        project.package().clone(),
        root_document.identity().id().clone(),
        profile,
    )
    .expect("symbol world");
    let revision = ProjectSymbolRevision::try_for_documents(
        project
            .view()
            .modules()
            .map(|(_, module)| module.provenance().source_identity()),
    )
    .expect("symbol revision");
    let externals = ProjectExternalDeclarations::try_new(world, revision, Vec::new())
        .expect("empty external declarations");
    ProjectSymbolTable::link(project.view(), &externals)
        .expect("linked project symbols")
        .into_table()
}

fn assert_expression_owned_edges_have_semantic_paths(
    module: &HirModule,
    paths: &super::HirDeclarationSemanticPathIndex,
    owner: ExprId,
) {
    let expression = module.resolve_expr(owner).expect("owned expression");
    let edges = expression
        .kind()
        .expression_owned_child_edges()
        .expect("bounded owned topology");
    assert!(!edges.is_empty(), "fixture must contain owned roots");
    for edge in edges {
        let path = match edge.child() {
            HirExpressionOwnedChild::Pattern(owner) => paths.pattern(owner),
            HirExpressionOwnedChild::Statement(owner) => paths.statement(owner),
            HirExpressionOwnedChild::Body(body) => match body.child() {
                HirBodyChild::Expression(owner) => paths.expression(owner),
                HirBodyChild::Statement(owner) => paths.statement(owner),
            },
        }
        .expect("owned child semantic path");
        assert!(path.contains(&HirSemanticPathStep::ExpressionOwned(edge.role().clone())));
    }
}

fn assert_source_expression_owned_paths(
    label: &str,
    source: &str,
    select: impl Fn(&HirExprKind) -> bool,
) {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        &format!("arcweft-test://proof/final-project/{label}"),
        &format!("{label}.arcw"),
        source,
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let retained_module = Arc::clone(&module);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .expect("executable project");
    let symbols = symbols_for_project(&project, parsed.document(), label);
    let declaration = symbols
        .callable_symbols()
        .find(|symbol| symbol.source_owner() == HirCallableSourceOwner::Item)
        .expect("fixture callable")
        .declaration()
        .clone();
    let paths = project
        .executable_view()
        .expect("executable HIR")
        .declaration_semantic_paths(&symbols, &declaration)
        .expect("semantic paths");
    let owner = retained_module
        .expressions()
        .find_map(|(owner, expression)| select(expression.kind()).then_some(owner))
        .expect("selected owned expression");
    assert_expression_owned_edges_have_semantic_paths(&retained_module, &paths, owner);
}

#[test]
fn semantic_paths_consume_await_owned_roots() {
    assert_source_expression_owned_paths(
        "await-owned-paths",
        concat!(
            "flow line_handles {\n",
            "    try await task with {\n",
            "        pending progress => { let value = 1 }\n",
            "    }\n",
            "}\n",
        ),
        |kind| matches!(kind, HirExprKind::Await(_)),
    );
}

#[test]
fn semantic_paths_consume_dialogue_owned_roots() {
    assert_source_expression_owned_paths(
        "dialogue-owned-paths",
        concat!(
            "pub character alice { display_name = \"Alice\" }\n",
            "flow line_handles() -> String {\n",
            "    let (_, cue) = alice(voice=auto)[聞いて。[p]]\n",
            "    with:\n",
            "        let actor = alice.stage.acquire(scope=line)\n",
            "        let cue = at(0.42s):\n",
            "            actor.look(.worried, crossfade=120ms)\n",
            "        let voice = line.voice_handle()\n",
            "        out (voice, cue)\n",
            "    log.info(\"cue kept\", cue = cue)\n",
            "    return \"done\"\n",
            "}\n",
        ),
        |kind| matches!(kind, HirExprKind::DialogueContentApplication(_)),
    );
}

#[test]
fn view_semantic_paths_cover_parameters_and_source_ordered_values() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/view-semantic-paths",
        "view-semantic-paths.arcw",
        "view Main(count: u32 = 1) {\n    Panel {}\n    Text(count)\n}\n",
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let retained_module = Arc::clone(&module);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .unwrap();
    let symbols = symbols_for_project(&project, parsed.document(), "view-semantic-paths");
    let symbol = symbols
        .callable_symbols()
        .find(|symbol| symbol.source_owner() == HirCallableSourceOwner::ViewItem)
        .expect("View callable symbol");
    let declaration = symbol.declaration().clone();
    let item = retained_module.resolve_item(symbol.source_item()).unwrap();
    let HirItemKind::View(view) = item.kind() else {
        panic!("View item");
    };
    let parameter = &view.parameters()[0];
    let values = view.values();

    let paths = project
        .executable_view()
        .unwrap()
        .declaration_semantic_paths(&symbols, &declaration)
        .expect("View has executable semantic roots");
    assert_eq!(paths.declaration(), &declaration);
    assert_eq!(
        paths.pattern(parameter.pattern()),
        Some(
            [HirSemanticPathStep::ParameterPattern {
                group: 0,
                parameter: 0,
            }]
            .as_slice()
        )
    );
    assert_eq!(
        paths.expression(parameter.default().expect("default")),
        Some(
            [HirSemanticPathStep::ParameterDefault {
                group: 0,
                parameter: 0,
            }]
            .as_slice()
        )
    );
    assert_eq!(values.len(), 2);
    for (ordinal, value) in values.iter().copied().enumerate() {
        assert_eq!(
            paths.expression(value),
            Some(
                [HirSemanticPathStep::DeclarationBody(
                    HirDeclarationBodyRootRole::ViewValue {
                        ordinal: u32::try_from(ordinal).unwrap(),
                    },
                )]
                .as_slice()
            )
        );
    }
    assert_eq!(
        paths,
        project
            .executable_view()
            .unwrap()
            .declaration_semantic_paths(&symbols, &declaration)
            .expect("deterministic View paths")
    );
}

#[test]
fn empty_view_has_an_empty_but_valid_semantic_path_index() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/empty-view-semantic-paths",
        "empty-view-semantic-paths.arcw",
        "view Empty() {}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .expect("empty View project");
    let symbols = symbols_for_project(&project, parsed.document(), "empty-view-semantic-paths");
    let declaration = symbols
        .callable_symbols()
        .find(|symbol| symbol.source_owner() == HirCallableSourceOwner::ViewItem)
        .expect("empty View callable")
        .declaration();
    let paths = project
        .executable_view()
        .expect("executable empty View")
        .declaration_semantic_paths(&symbols, declaration)
        .expect("empty View path index");
    assert_eq!(paths.declaration(), declaration);
}

#[test]
fn poisoned_view_callable_row_is_not_executable() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/poisoned-view-callable",
        "poisoned-view-callable.arcw",
        concat!(
            "view Broken() {\n",
            "    Panel {}\n",
            "    export late\n",
            "}\n",
        ),
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(!module.diagnostics().is_empty());
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .expect("recovered View project remains inspectable");
    let symbols = symbols_for_project(&project, parsed.document(), "poisoned-view-callable");
    let symbol = symbols
        .callable_symbols()
        .find(|symbol| symbol.source_owner() == HirCallableSourceOwner::ViewItem)
        .expect("poisoned View callable row");
    assert!(!symbol.is_executable());
}

#[test]
fn semantic_paths_reject_symbols_from_a_foreign_snapshot() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let first = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/foreign-semantic-path-snapshot",
        "foreign-semantic-path-snapshot.arcw",
        "view Main() {\n    Panel {}\n}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let first_module = lower(&mut database, &first, &package, &root_path);
    let first_project = build_project(
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
    let first_symbols = symbols_for_project(&first_project, first.document(), "foreign-snapshot");
    let declaration = first_symbols
        .callable_symbols()
        .find(|symbol| symbol.source_owner() == HirCallableSourceOwner::ViewItem)
        .expect("View callable symbol")
        .declaration()
        .clone();

    let second = syntax
        .reparse(
            &first,
            &[SourceEdit::new(
                first.document().span(SourceRange::new(0, 0)).unwrap(),
                "/// revised\n",
            )],
            arcweft_lang_syntax::parser::ParseOptions::default(),
        )
        .unwrap();
    let second_module = lower(&mut database, &second, &package, &root_path);
    assert_eq!(first_module.module_id(), second_module.module_id());
    assert_ne!(first_module.snapshot_id(), second_module.snapshot_id());
    let second_project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, second_module)],
    )
    .unwrap();

    assert_eq!(
        second_project
            .executable_view()
            .unwrap()
            .declaration_semantic_paths(&first_symbols, &declaration),
        Err(HirSemanticPathError::ForeignSnapshot)
    );
}

#[test]
fn semantic_paths_keep_extern_and_trait_requirements_bodyless() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/bodyless-semantic-paths",
        "bodyless-semantic-paths.arcw",
        concat!(
            "extern capability host { fn read() -> Unit }\n",
            "trait Readable { fn read(&self) -> Self }\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .expect("bodyless project");
    let symbols = symbols_for_project(&project, parsed.document(), "bodyless-semantic-paths");
    let bodyless = symbols
        .callable_symbols()
        .filter(|symbol| {
            matches!(
                symbol.source_owner(),
                HirCallableSourceOwner::ExternCapabilityFunction { .. }
                    | HirCallableSourceOwner::TraitFunction { .. }
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(bodyless.len(), 2);
    for symbol in bodyless {
        assert_eq!(
            project
                .executable_view()
                .expect("executable HIR")
                .declaration_semantic_paths(&symbols, symbol.declaration()),
            Err(HirSemanticPathError::MissingBody)
        );
    }
}

#[test]
fn declaration_root_matrix_is_exact_and_deterministic() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/declaration-root-matrix",
        "declaration-root-matrix.arcw",
        concat!(
            "fn ordinary(value: Int = 1) { value }\n",
            "predicate logical(value: Bool) = value\n",
            "proof evidence(value: Int) = ()\n",
            "flow directed(value: Int) { value }\n",
            "struct Target {}\n",
            "trait Base { fn run(self) -> Int }\n",
            "impl Base for Target { fn run(self) -> Int { 1 } }\n",
            "impl Target { fn own(self) -> Int { 2 } }\n",
            "view Screen(value: Int = 1) { Text(value) }\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let retained_module = Arc::clone(&module);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .expect("declaration-root matrix project");
    let symbols = symbols_for_project(&project, parsed.document(), "declaration-root-matrix");
    let expected = [
        (
            CallableDeclarationOwner::Function,
            HirDeclarationBodyRootRole::FunctionBody,
            1_usize,
        ),
        (
            CallableDeclarationOwner::Predicate,
            HirDeclarationBodyRootRole::PredicateBody,
            1,
        ),
        (
            CallableDeclarationOwner::Proof,
            HirDeclarationBodyRootRole::ProofBody,
            1,
        ),
        (
            CallableDeclarationOwner::Flow,
            HirDeclarationBodyRootRole::FlowBody,
            1,
        ),
        (
            CallableDeclarationOwner::TraitImplementation,
            HirDeclarationBodyRootRole::ImplFunctionBody,
            1,
        ),
        (
            CallableDeclarationOwner::InherentMethod,
            HirDeclarationBodyRootRole::ImplFunctionBody,
            1,
        ),
        (
            CallableDeclarationOwner::View,
            HirDeclarationBodyRootRole::ViewValue { ordinal: 0 },
            1,
        ),
    ];
    for (owner, root, expected_count) in expected {
        let rows = symbols
            .callable_symbols()
            .filter(|symbol| symbol.owner() == owner)
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), expected_count, "{owner:?}");
        for symbol in rows {
            let paths = project
                .executable_view()
                .expect("executable HIR")
                .declaration_semantic_paths(&symbols, symbol.declaration())
                .expect("declaration semantic paths");
            let rooted = retained_module.expressions().any(|(expression, _)| {
                paths.expression(expression).is_some_and(|path| {
                    path.first() == Some(&HirSemanticPathStep::DeclarationBody(root))
                })
            });
            assert!(rooted, "missing {owner:?} declaration root");
            assert!(retained_module.patterns().any(|(pattern, _)| matches!(
                paths.pattern(pattern),
                Some([HirSemanticPathStep::ParameterPattern {
                    group: 0,
                    parameter: 0
                }])
            )));
            let has_default = retained_module.expressions().any(|(expression, _)| {
                matches!(
                    paths.expression(expression),
                    Some([HirSemanticPathStep::ParameterDefault {
                        group: 0,
                        parameter: 0
                    }])
                )
            });
            assert_eq!(
                has_default,
                matches!(
                    owner,
                    CallableDeclarationOwner::Function | CallableDeclarationOwner::View
                ),
                "{owner:?} default path policy"
            );
            assert_eq!(
                paths,
                project
                    .executable_view()
                    .unwrap()
                    .declaration_semantic_paths(&symbols, symbol.declaration())
                    .expect("deterministic declaration semantic paths")
            );
        }
    }
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
        build_project(
            &database,
            package.clone(),
            [bind(&database, &package, &child_path, child)],
        )
        .err(),
        Some(HirProjectBuildError::MissingRootModule {
            package: package.clone(),
        })
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
        build_project(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &root_path, Arc::clone(&root)),
            ],
        )
        .err(),
        Some(HirProjectBuildError::DuplicateModule {
            key: HirPackageModuleKey::new(package.clone(), root_path.clone()),
        })
    );
    assert_eq!(
        build_project(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, root),
                bind(&database, &package, &child_path, child),
            ],
        )
        .err(),
        Some(HirProjectBuildError::DuplicateSourceDocument {
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

    let project = build_project(
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
fn selected_expression_inventory_validates_and_projects_one_postfix_graph() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/selected-expression-root",
        "selected-expression-root.arcw",
        "flow root(items: Vec<i64>, key: i64) { items[key] }\n",
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/selected-expression-child",
        "selected-expression-child.arcw",
        "fn foreign() { true }\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    let foreign = child
        .expressions()
        .next()
        .map(|(owner, _)| owner)
        .expect("foreign expression fixture");
    let (owner, target, index, dialogue, index_children, dialogue_children) = root
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::PostfixBracket(postfix) = expression.kind() else {
                return None;
            };
            let HirPostfixBracketCandidates::Ambiguous { index, dialogue } = postfix.candidates()
            else {
                return None;
            };
            let index_children = root
                .resolve_expr(*index)
                .expect("index candidate")
                .kind()
                .direct_expression_children();
            let dialogue_children = root
                .resolve_expr(*dialogue)
                .expect("dialogue candidate")
                .kind()
                .direct_expression_children();
            Some((
                owner,
                postfix.target(),
                *index,
                *dialogue,
                index_children,
                dialogue_children,
            ))
        })
        .expect("ambiguous postfix fixture");
    let project = build_project(
        &database,
        package.clone(),
        [
            bind(&database, &package, &child_path, child),
            bind(&database, &package, &root_path, root),
        ],
    )
    .unwrap();
    let executable = project.executable_view().unwrap();

    assert_eq!(
        executable.selected_expression_owners(|_| None),
        Err(HirSelectedExpressionInventoryError::MissingPostfixSelection { expression: owner })
    );
    assert_eq!(
        executable.selected_expression_owners(|candidate_owner| {
            (candidate_owner == owner).then_some(foreign)
        }),
        Err(
            HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                expression: owner,
                candidate: foreign,
            }
        )
    );

    let selected = executable
        .selected_expression_owners(|candidate_owner| (candidate_owner == owner).then_some(index))
        .expect("selected index graph");
    assert!(selected.contains(&owner));
    assert!(selected.contains(&target));
    assert!(selected.contains(&index));
    assert!(!selected.contains(&dialogue));
    assert!(
        index_children.iter().all(|child| selected.contains(child)),
        "the complete selected candidate graph remains reachable"
    );

    assert_runtime_postfix_expression_type_inventory(
        executable,
        owner,
        target,
        index,
        dialogue,
        &index_children,
        &dialogue_children,
    );
}

fn assert_runtime_postfix_expression_type_inventory(
    executable: HirExecutableProjectView<'_>,
    owner: ExprId,
    target: ExprId,
    index: ExprId,
    dialogue: ExprId,
    index_children: &[ExprId],
    dialogue_children: &[ExprId],
) {
    assert!(matches!(
        runtime_reachability(
            executable,
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        ),
        Err(HirRuntimeReachabilityError::SelectedExpressions(
            HirSelectedExpressionInventoryError::MissingPostfixSelection { expression }
        )) if expression == owner
    ));
    let runtime_index = runtime_reachability(
        executable,
        |candidate_owner| (candidate_owner == owner).then_some(index),
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
    .expect("selected runtime index reachability")
    .selected_expression_type_owners()
    .expect("selected runtime index type graph");
    assert!(runtime_index.contains(&owner));
    assert!(runtime_index.contains(&target));
    assert!(runtime_index.contains(&index));
    assert!(!runtime_index.contains(&dialogue));
    assert!(
        index_children
            .iter()
            .all(|child| runtime_index.contains(child)),
        "the complete runtime index graph remains reachable"
    );

    let runtime_dialogue = runtime_reachability(
        executable,
        |candidate_owner| (candidate_owner == owner).then_some(dialogue),
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
    .expect("selected runtime dialogue reachability")
    .selected_expression_type_owners()
    .expect("selected runtime dialogue operand graph");
    assert!(!runtime_dialogue.contains(&owner));
    assert!(!runtime_dialogue.contains(&dialogue));
    assert!(!runtime_dialogue.contains(&index));
    assert!(runtime_dialogue.contains(&target));
    assert!(
        dialogue_children
            .iter()
            .all(|child| runtime_dialogue.contains(child)),
        "dialogue operands remain runtime type owners without carrier types"
    );
}

#[test]
fn runtime_expression_type_inventory_excludes_effect_metadata_subtrees() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/runtime-expression-types",
        "runtime-expression-types.arcw",
        "flow root() effects { fs.read } { true }\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let effect_root = module
        .items()
        .find_map(|(_, item)| item.kind().effect_expression_roots().into_iter().next())
        .expect("fixture effect expression root");
    let effect_children = module
        .resolve_expr(effect_root)
        .expect("effect root resolves")
        .kind()
        .direct_expression_children();
    let body_literal = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Literal(_)).then_some(owner)
        })
        .expect("ordinary body literal");
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .unwrap();
    let executable = project.executable_view().unwrap();

    let semantic = executable
        .selected_expression_owners(|_| None)
        .expect("postfix-free semantic inventory");
    assert!(semantic.contains(&effect_root));
    assert!(
        effect_children.iter().all(|child| semantic.contains(child)),
        "semantic analysis retains the complete effect expression subtree"
    );

    let runtime_owners = runtime_reachability(
        executable,
        |_| None,
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
    .expect("runtime semantic reachability");
    let runtime = runtime_owners
        .selected_expression_type_owners()
        .expect("postfix-free runtime type inventory");
    assert!(!runtime.contains(&effect_root));
    assert!(
        effect_children.iter().all(|child| !runtime.contains(child)),
        "effect metadata descendants do not publish runtime types"
    );
    assert!(runtime.contains(&body_literal));
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the fixture asserts every non-value call-carrier disposition in one matrix"
)]
fn runtime_expression_type_inventory_applies_typed_non_value_call_carriers() {
    let (project, call, callee, argument, _, _, _, _) = runtime_call_inventory_fixture();
    let executable = project.executable_view().unwrap();
    let retained = runtime_reachability(
        executable,
        |_| None,
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
    .expect("retained reachability")
    .selected_expression_type_owners()
    .expect("postfix-free retained call inventory");
    assert!(retained.contains(&call));
    assert!(retained.contains(&callee));
    assert!(retained.contains(&argument));

    let retained_static_call = runtime_reachability(
        executable,
        |_| None,
        |owner| {
            if owner == call {
                HirRuntimeExpressionTypeDisposition::RetainedCallResult {
                    callee: HirRuntimeCallCalleeDisposition::Static,
                }
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            }
        },
    )
    .expect("retained static-call reachability")
    .selected_expression_type_owners()
    .expect("retained static call result inventory");
    assert!(retained_static_call.contains(&call));
    assert!(!retained_static_call.contains(&callee));
    assert!(retained_static_call.contains(&argument));

    let retained_receiver_call = runtime_reachability(
        executable,
        |_| None,
        |owner| {
            if owner == call {
                HirRuntimeExpressionTypeDisposition::RetainedCallResult {
                    callee: HirRuntimeCallCalleeDisposition::RuntimeReceiver,
                }
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            }
        },
    )
    .expect("retained receiver-call reachability")
    .selected_expression_type_owners()
    .expect("retained runtime-receiver call result inventory");
    assert!(retained_receiver_call.contains(&call));
    assert!(retained_receiver_call.contains(&callee));
    assert!(retained_receiver_call.contains(&argument));

    let carrier = runtime_reachability(
        executable,
        |_| None,
        |owner| {
            if owner == call {
                HirRuntimeExpressionTypeDisposition::NonValueCallCarrier {
                    callee: HirRuntimeCallCalleeDisposition::Static,
                }
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            }
        },
    )
    .expect("static carrier reachability")
    .selected_expression_type_owners()
    .expect("postfix-free carrier inventory");
    assert!(!carrier.contains(&call));
    assert!(!carrier.contains(&callee));
    assert!(carrier.contains(&argument));

    let receiver = runtime_reachability(
        executable,
        |_| None,
        |owner| {
            if owner == call {
                HirRuntimeExpressionTypeDisposition::NonValueCallCarrier {
                    callee: HirRuntimeCallCalleeDisposition::RuntimeReceiver,
                }
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            }
        },
    )
    .expect("receiver carrier reachability")
    .selected_expression_type_owners()
    .expect("runtime-receiver call carrier inventory");
    assert!(!receiver.contains(&call));
    assert!(receiver.contains(&callee));
    assert!(receiver.contains(&argument));

    assert!(matches!(
        runtime_reachability(
            executable,
            |_| None,
            |owner| {
                if owner == argument {
                    HirRuntimeExpressionTypeDisposition::NonValueCallCarrier {
                        callee: HirRuntimeCallCalleeDisposition::Static,
                    }
                } else {
                    HirRuntimeExpressionTypeDisposition::Retain
                }
            },
        ),
        Err(HirRuntimeReachabilityError::SelectedExpressions(
            HirSelectedExpressionInventoryError::InvalidRuntimeCallDisposition {
                expression,
            }
        )) if expression == argument
    ));
}

#[test]
fn retained_member_call_result_keeps_receiver_and_omits_select_callee() {
    let (project, _, _, _, call, callee, receiver, argument) = runtime_call_inventory_fixture();
    let executable = project.executable_view().expect("executable fixture");
    let retained = runtime_reachability(
        executable,
        |_| None,
        |owner| {
            if owner == call {
                HirRuntimeExpressionTypeDisposition::RetainedCallResult {
                    callee: HirRuntimeCallCalleeDisposition::RuntimeReceiver,
                }
            } else {
                HirRuntimeExpressionTypeDisposition::Retain
            }
        },
    )
    .expect("retained member-call reachability")
    .selected_expression_type_owners()
    .expect("retained member-call result inventory");
    assert!(retained.contains(&call));
    assert!(!retained.contains(&callee));
    assert!(retained.contains(&receiver));
    assert!(retained.contains(&argument));
}

fn runtime_call_inventory_fixture() -> (
    HirProject,
    ExprId,
    ExprId,
    ExprId,
    ExprId,
    ExprId,
    ExprId,
    ExprId,
) {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/runtime-expression-carrier",
        "runtime-expression-carrier.arcw",
        concat!(
            "fn helper(value: bool) -> bool { value }\n",
            "flow root(value: bool) { helper(value) }\n",
            "flow compare(value: bool) { (value.eq)(false) }\n",
        ),
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let (call, callee, argument) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            let callee = call.callee().value_expression()?;
            (!matches!(
                module.resolve_expr(callee).ok()?.kind(),
                HirExprKind::Select(_)
            ))
            .then_some((owner, callee, call.arguments()[0].value()))
        })
        .expect("fixture call expression");
    let (member_call, member_callee, receiver, member_argument) = module
        .expressions()
        .find_map(|(owner, expression)| {
            let HirExprKind::Call(call) = expression.kind() else {
                return None;
            };
            let callee = call.callee().value_expression()?;
            let HirExprKind::Select(select) = module.resolve_expr(callee).ok()?.kind() else {
                return None;
            };
            Some((owner, callee, select.target(), call.arguments()[0].value()))
        })
        .expect("fixture member call expression");
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .unwrap();
    (
        project,
        call,
        callee,
        argument,
        member_call,
        member_callee,
        receiver,
        member_argument,
    )
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "one mixed View, Style, and runtime fixture proves the complete owner-domain boundary and canonical local filtering"
)]
fn runtime_semantic_reachability_excludes_presentation_and_unreachable_functions() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/runtime-owner-domain",
        "runtime-owner-domain.arcw",
        concat!(
            "flow runtime(first: bool) {\n",
            "    let second: bool = first\n",
            "    second\n",
            "}\n",
            "#[tool.flag(1)]\n",
            "view Card(dialogue: DialogueView, count: i64 = 1i64) {\n",
            "    Text(\"x\")\n",
            "}\n",
            "#[tool.flag(2)]\n",
            "style Theme {\n",
            "    token color.text: Color = white\n",
            "    Button { color = rgba(10, 20, 30, 255) }\n",
            "    when environment(color-scheme == dark) {\n",
            "        Button { color = red }\n",
            "    }\n",
            "}\n",
            "fn runtime_after(third: bool) { third }\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .unwrap();
    let executable = project.executable_view().unwrap();
    let inventory = runtime_reachability(
        executable,
        |_| None,
        |_| HirRuntimeExpressionTypeDisposition::Retain,
    )
    .expect("runtime semantic reachability");

    let mut presentation_locals = BTreeSet::new();
    for (_, item) in module.items() {
        match item.kind() {
            HirItemKind::View(view) => {
                for parameter in view.parameters() {
                    assert!(!inventory.contains_pattern(parameter.pattern()));
                    assert!(!inventory.contains_type(parameter.ty()));
                    assert!(
                        parameter
                            .default()
                            .is_none_or(|owner| !inventory.contains_expression(owner))
                    );
                    for local in parameter.locals() {
                        presentation_locals.insert(*local);
                        assert!(!inventory.contains_local(*local));
                    }
                }
                assert!(
                    view.values()
                        .iter()
                        .all(|owner| !inventory.contains_expression(*owner))
                );
            }
            HirItemKind::Style(style) => {
                assert!(
                    style
                        .value_expression_roots()
                        .iter()
                        .all(|owner| !inventory.contains_expression(*owner)),
                    "Style values stay with the Style product"
                );
                assert!(style.value_expression_roots().len() >= 4);
            }
            HirItemKind::Flow(flow) => {
                for parameter in flow.parameters() {
                    assert!(inventory.contains_pattern(parameter.pattern()));
                    assert!(inventory.contains_type(parameter.ty()));
                    assert!(
                        parameter
                            .locals()
                            .iter()
                            .all(|local| inventory.contains_local(*local))
                    );
                }
                assert!(flow.body().items().iter().all(|item| match item {
                    HirThreadFlowItem::DialogueApplication(expression) => {
                        inventory.contains_expression(*expression)
                    }
                    HirThreadFlowItem::Statement(statement)
                    | HirThreadFlowItem::Choice(statement)
                    | HirThreadFlowItem::If(statement)
                    | HirThreadFlowItem::IfLet(statement)
                    | HirThreadFlowItem::Match(statement)
                    | HirThreadFlowItem::While(statement)
                    | HirThreadFlowItem::WhileLet(statement)
                    | HirThreadFlowItem::For(statement)
                    | HirThreadFlowItem::Select(statement)
                    | HirThreadFlowItem::SourceLocale(statement)
                    | HirThreadFlowItem::Scope(statement)
                    | HirThreadFlowItem::Include(statement)
                    | HirThreadFlowItem::Error(statement) => {
                        inventory.contains_statement(*statement)
                    }
                }));
            }
            HirItemKind::Function(function) => {
                for parameter in function
                    .parameter_groups()
                    .iter()
                    .flat_map(crate::item::HirFunctionParameterGroup::parameters)
                {
                    assert!(!inventory.contains_pattern(parameter.pattern()));
                    assert!(!inventory.contains_type(parameter.ty()));
                    assert!(
                        parameter
                            .locals()
                            .iter()
                            .all(|local| !inventory.contains_local(*local))
                    );
                }
                let crate::item::HirFunctionBody::Block {
                    statements, tail, ..
                } = function.body()
                else {
                    panic!("unreachable function fixture has a block body")
                };
                assert!(
                    statements
                        .iter()
                        .all(|statement| !inventory.contains_statement(*statement))
                );
                assert!(!inventory.contains_expression(*tail));
            }
            HirItemKind::Module(_)
            | HirItemKind::Use(_)
            | HirItemKind::Predicate(_)
            | HirItemKind::Proof(_)
            | HirItemKind::Trait(_)
            | HirItemKind::Impl(_)
            | HirItemKind::Enum(_)
            | HirItemKind::Struct(_)
            | HirItemKind::TypeAlias(_)
            | HirItemKind::Resource(_)
            | HirItemKind::Character(_)
            | HirItemKind::Action(_)
            | HirItemKind::Activity(_)
            | HirItemKind::Signal(_)
            | HirItemKind::Metric(_)
            | HirItemKind::Layer(_)
            | HirItemKind::Entry(_)
            | HirItemKind::ExternCapability(_)
            | HirItemKind::Test(_)
            | HirItemKind::Bench(_)
            | HirItemKind::Error(_) => panic!("unexpected fixture item family"),
        }
    }

    assert!(!presentation_locals.is_empty());
    assert_eq!(inventory.locals().count(), 2);
    let selected = inventory
        .selected_expression_type_owners()
        .expect("postfix-free runtime type inventory");
    assert!(
        selected
            .iter()
            .all(|owner| inventory.contains_expression(*owner))
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the determinism proof constructs and compares both complete edge orderings"
)]
fn runtime_reachability_is_edge_order_independent_and_records_shortest_paths() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/runtime-reachability-order",
        "runtime-reachability-order.arcw",
        concat!(
            "fn first() -> bool { true }\n",
            "fn second() -> bool { false }\n",
            "flow root() { first() }\n",
        ),
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .unwrap();
    let executable = project.executable_view().unwrap();
    let flow = executable
        .items()
        .find(|item| matches!(item.item().kind(), HirItemKind::Flow(_)))
        .map(super::HirProjectItemRef::id)
        .unwrap();
    let functions = executable
        .items()
        .filter_map(|item| {
            let HirItemKind::Function(function) = item.item().kind() else {
                return None;
            };
            Some((function.name().resolved()?.as_str().to_owned(), item.id()))
        })
        .collect::<BTreeMap<_, _>>();
    let call = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Call(_)).then_some(owner)
        })
        .unwrap();
    let root = HirRuntimeReachabilityRoot::new(
        HirRuntimeReachabilityRootKind::CheckedFlow,
        HirRuntimeExecutableOwner::Item(flow),
    );
    let edges = functions
        .iter()
        .map(|(name, owner)| {
            let declaration = CallableDeclarationKey::Existing(
                CallableDeclarationId::try_new(
                    package.clone(),
                    root_path.clone(),
                    CallableDeclarationOwner::Function,
                    name,
                )
                .unwrap(),
            );
            HirRuntimeReachabilityEdge::new(
                super::HirRuntimeReachabilitySite::Expression(call),
                HirRuntimeExecutableOwner::Item(*owner),
                super::HirRuntimeReachabilityEdgeKind::CheckedProjectCall { call, declaration },
            )
        })
        .collect::<Vec<_>>();
    let world = ProjectSymbolWorldId::try_new(
        package,
        parsed.document().identity().id().clone(),
        "final-project-tests",
    )
    .unwrap();
    let revision =
        ProjectSymbolRevision::try_for_documents([parsed.document().identity()]).unwrap();
    let build = |edges: Vec<HirRuntimeReachabilityEdge>| {
        let input = HirRuntimeSemanticReachabilityInput::try_new(
            HirRuntimeEmissionMode::CheckAll,
            world.clone(),
            revision,
            vec![root.clone()],
            edges,
        )
        .unwrap();
        executable
            .runtime_semantic_reachability(
                input,
                |_| None,
                |owner| {
                    if owner == call {
                        HirRuntimeExpressionTypeDisposition::RetainedCallResult {
                            callee: HirRuntimeCallCalleeDisposition::Static,
                        }
                    } else {
                        HirRuntimeExpressionTypeDisposition::Retain
                    }
                },
            )
            .unwrap()
    };
    let forward = build(edges.clone());
    let reverse = build(edges.into_iter().rev().collect());
    assert_eq!(forward.identity().digest(), reverse.identity().digest());
    assert_eq!(forward.reachable_executables().count(), 3);
    for owner in functions.values().copied() {
        let executable = HirRuntimeExecutableOwner::Item(owner);
        assert_eq!(forward.first_path(&executable).unwrap().steps().len(), 1);
    }
}

#[test]
fn selected_expression_inventory_is_deterministic_across_module_input_order() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("child").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/selected-roots-root",
        "selected-roots-root.arcw",
        "fn root() { 1 }\n",
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/selected-roots-child",
        "selected-roots-child.arcw",
        "fn child() { true }\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    let expected = root
        .expressions()
        .chain(child.expressions())
        .map(|(owner, _)| owner)
        .collect::<BTreeSet<_>>();
    let root = bind(&database, &package, &root_path, root);
    let child = bind(&database, &package, &child_path, child);
    let forward = build_project(&database, package.clone(), [root.clone(), child.clone()]).unwrap();
    let reverse = build_project(&database, package, [child, root]).unwrap();

    let forward = forward
        .executable_view()
        .unwrap()
        .selected_expression_owners(|_| None)
        .expect("postfix-free forward inventory");
    let reverse = reverse
        .executable_view()
        .unwrap()
        .selected_expression_owners(|_| None)
        .expect("postfix-free reverse inventory");
    assert_eq!(forward, expected);
    assert_eq!(reverse, expected);
}

#[test]
fn project_publishes_generated_dialogue_identity_from_typed_callable_owner() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/dialogue-generated",
        "dialogue-generated.arcw",
        "fn opening() {\n    let line = alice[前[strong]強調[/strong]後]\n}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(module.is_executable(), "{:?}", module.diagnostics());
    assert_eq!(
        module.dialogue_line_candidates().records().len(),
        1,
        "expression inventory: {:#?}",
        module
            .expressions()
            .map(|(id, expression)| (id, expression.kind()))
            .collect::<Vec<_>>()
    );
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, module)],
    )
    .unwrap();

    let records = project.dialogue_lines().records();
    assert_eq!(records.len(), 1);
    let line = &records[0];
    assert_eq!(
        line.id().as_str(),
        "say.fn.proof-final-project-tests.function.opening.001"
    );
    assert_eq!(
        line.text_key().as_str(),
        "text.fn.proof-final-project-tests.function.opening.001"
    );
    assert_eq!(line.id_origin(), DialogueLineIdOrigin::Generated);
    assert_eq!(line.text_key_origin(), DialogueTextKeyOrigin::Derived);
    assert_eq!(project.dialogue_lines().get(line.id()), Some(line));
    assert_eq!(
        project
            .dialogue_lines()
            .for_expr(line.source().application()),
        Some(line)
    );
    assert_eq!(
        project
            .dialogue_lines()
            .source_ordered()
            .collect::<Vec<_>>(),
        [line]
    );
}

#[test]
fn module_input_permutations_produce_equal_inventory_fingerprint() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("chapter").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/fingerprint-root",
        "fingerprint-root.arcw",
        "fn root_line() {\n    let line = alice[before[strong]root[/strong]after]\n}\n",
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/fingerprint-child",
        "fingerprint-child.arcw",
        "fn child_line() {\n    let line = bob[before[strong]child[/strong]after]\n}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    let root = bind(&database, &package, &root_path, root);
    let child = bind(&database, &package, &child_path, child);

    let forward = build_project(&database, package.clone(), [root.clone(), child.clone()]).unwrap();
    let reverse = build_project(&database, package, [child, root]).unwrap();

    assert_eq!(forward.dialogue_lines(), reverse.dialogue_lines());
    assert_eq!(
        forward.dialogue_lines().cache_fingerprint(),
        reverse.dialogue_lines().cache_fingerprint()
    );
    assert_ne!(
        forward.dialogue_lines().cache_fingerprint().as_bytes(),
        &[0; 32]
    );
}

#[test]
fn project_rejects_cross_module_dialogue_id_collision_with_exact_sites() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let child_path = root_path.join(ModuleSegment::new("chapter").unwrap());
    let mut root_syntax = SyntaxDatabase::try_new().unwrap();
    let root_source = parse_initial(
        &mut root_syntax,
        "arcweft-test://proof/final-project/dialogue-collision-root",
        "dialogue-collision-root.arcw",
        "fn root_line() {\n    let line = alice(id = @say.shared)[前[strong]ルート[/strong]後]\n}\n",
    );
    let mut child_syntax = SyntaxDatabase::try_new().unwrap();
    let child_source = parse_initial(
        &mut child_syntax,
        "arcweft-test://proof/final-project/dialogue-collision-child",
        "dialogue-collision-child.arcw",
        "fn child_line() {\n    let line = bob(id = @say.shared)[前[strong]チャイルド[/strong]後]\n}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let root = lower(&mut database, &root_source, &package, &root_path);
    let child = lower(&mut database, &child_source, &package, &child_path);
    assert!(root.is_executable(), "{:?}", root.diagnostics());
    assert!(child.is_executable(), "{:?}", child.diagnostics());
    let result = build_project(
        &database,
        package.clone(),
        [
            bind(&database, &package, &child_path, child),
            bind(&database, &package, &root_path, root),
        ],
    );
    let Some(HirProjectBuildError::DialogueLines(rejection)) = result.err() else {
        panic!("duplicate line ID must atomically reject the project")
    };
    let [
        DialogueLineDiagnostic::LineIdCollision {
            id,
            first,
            conflicting,
        },
    ] = rejection.diagnostics()
    else {
        panic!("one typed collision diagnostic expected")
    };
    assert_eq!(
        id,
        &DialogueLineId::try_new("say.shared").expect("checked line ID")
    );
    assert_eq!(first.module().path(), &root_path);
    assert_eq!(conflicting.module().path(), &child_path);
    assert_eq!(first.span().source(), root_source.document().identity());
    assert_eq!(
        conflicting.span().source(),
        child_source.document().identity()
    );
    let root_id_start = root_source
        .document()
        .text()
        .find("@say.shared")
        .expect("root line ID");
    let child_id_start = child_source
        .document()
        .text()
        .find("@say.shared")
        .expect("child line ID");
    assert_eq!(
        first.span().range(),
        SourceRange::new(root_id_start, root_id_start + "@say.shared".len())
    );
    assert_eq!(
        conflicting.span().range(),
        SourceRange::new(child_id_start, child_id_start + "@say.shared".len())
    );
    assert_eq!(
        rejection.diagnostics()[0]
            .to_source_diagnostic()
            .labels()
            .len(),
        2
    );
}

#[test]
fn module_line_identity_error_publishes_no_candidate_or_project() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/dialogue-wrong-family",
        "dialogue-wrong-family.arcw",
        "fn opening() {\n    let line = alice(id = @scene.wrong)[前[strong]本文[/strong]後]\n}\n",
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(!module.is_executable());
    assert!(module.dialogue_line_candidates().records().is_empty());
    assert!(module.diagnostics().iter().any(|diagnostic| matches!(
        diagnostic,
        crate::diagnostic::HirDiagnostic::LineIdentity(
            DialogueLineDiagnostic::InvalidLineIdFamily { found, span }
        ) if found == "scene" && span.source() == parsed.document().identity()
    )));
    let bound = bind(&database, &package, &root_path, module);
    let project = build_project(&database, package, [bound]).unwrap();
    assert!(project.dialogue_lines().records().is_empty());
    assert!(project.executable_view().is_err());
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
    let project = build_project(
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
        build_project(&database, package, [future_stale]).err(),
        Some(HirProjectBuildError::StaleModuleLease {
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
        build_project(
            &database,
            CallablePackageId::try_new("wrong-project-package").unwrap(),
            [bind(&database, &package, &root_path, Arc::clone(&root),)],
        ),
        Err(HirProjectBuildError::ModulePackageMismatch { .. })
    ));
    assert_eq!(
        build_project(
            &database,
            package.clone(),
            [bind(&database, &package, &child_path, Arc::clone(&child),)],
        )
        .err(),
        Some(HirProjectBuildError::MissingRootModule {
            package: package.clone(),
        })
    );

    assert_eq!(
        build_project(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &root_path, Arc::clone(&root)),
            ],
        )
        .err(),
        Some(HirProjectBuildError::DuplicateModule {
            key: HirPackageModuleKey::new(package.clone(), root_path.clone()),
        })
    );
    assert_eq!(
        build_project(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, Arc::clone(&root)),
                bind(&database, &package, &child_path, Arc::clone(&child)),
            ],
        )
        .err(),
        Some(HirProjectBuildError::DuplicateSourceDocument {
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
        build_project_with_limit(
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
        build_project_with_limit(
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
        Some(HirProjectBuildError::ModuleLimit {
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
        build_project(
            &database,
            package.clone(),
            [
                bind(&database, &package, &root_path, root),
                bind(&foreign_database, &package, &child_path, foreign),
            ],
        ),
        Err(HirProjectBuildError::WrongDatabase { .. })
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
    let project = build_project(
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
    let project = build_project(
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
