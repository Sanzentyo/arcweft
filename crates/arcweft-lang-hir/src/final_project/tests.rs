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
    HirDeclarationBodyRootRole, HirDeclarationContractRootRole, HirDeclarationParameterRoot,
    HirDeclarationParameterRootRole, HirExecutableProjectView, HirPackageModuleKey, HirProject,
    HirProjectBuildError, HirProjectBuilder, HirProjectExecutionError, HirProjectModule,
    HirProjectModuleError, HirRuntimeCallCalleeDisposition, HirRuntimeEmissionMode,
    HirRuntimeExecutableOwner, HirRuntimeExpressionTypeDisposition, HirRuntimeReachabilityEdge,
    HirRuntimeReachabilityError, HirRuntimeReachabilityRoot, HirRuntimeReachabilityRootKind,
    HirRuntimeSemanticReachability, HirRuntimeSemanticReachabilityInput,
    HirSelectedExpressionInventoryError, HirSemanticOwnerPath, HirSemanticPathStep, exported_parts,
    styles,
};
use crate::body_edges::{HirBodyChild, HirBodyKind};
use crate::database::HirDatabase;
use crate::dialogue_application::{
    HirDialogueApplicationMetadataProjectionError, HirPostfixBracketCandidates,
};
use crate::expr::{
    HirExprKind, HirExpressionChildOwnership, HirExpressionChildRole, HirExpressionOwnedChild,
    HirThreadFlowItem,
};
use crate::final_lowering::stage_unpublished_module_for_invariant_test;
use crate::identity::ExprId;
use crate::item::{HirDeclarationMemberKind, HirItemKind};
use crate::line_identity::{DialogueLineDiagnostic, DialogueLineIdOrigin, DialogueTextKeyOrigin};
use crate::lowering::{HirModuleKey, LoweringRequest};
use crate::module::HirModule;
use crate::source_index::HirCallableSourceOwner;
use crate::stmt::HirStmtKind;
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

fn runtime_reachability<'project>(
    executable: HirExecutableProjectView<'project>,
    topology: &super::HirProjectEvaluationTopology,
    selected_postfix: impl FnMut(ExprId) -> Option<ExprId>,
    call_disposition: impl FnMut(ExprId) -> HirRuntimeExpressionTypeDisposition,
) -> Result<HirRuntimeSemanticReachability<'project>, HirRuntimeReachabilityError> {
    let world = topology.generation().symbol_world().clone();
    let revision = topology.generation().symbol_revision();
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
    executable.runtime_semantic_reachability(input, topology, selected_postfix, call_disposition)
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

#[test]
fn non_application_expression_cannot_issue_dialogue_metadata_projection() {
    let (_, _, _, module) = root_module_fixture("metadata-projection");
    let owner = module.expressions().next().expect("fixture expression").0;
    assert_eq!(
        module.dialogue_application_metadata_projection(owner),
        Err(HirDialogueApplicationMetadataProjectionError::NotDialogueApplication)
    );
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

fn evaluation_topology(
    project: &HirProject,
    symbols: &ProjectSymbolTable,
) -> Arc<super::HirProjectEvaluationTopology> {
    project
        .executable_view()
        .expect("executable project")
        .accept_symbol_generation(symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("project evaluation topology")
}

fn declaration_paths_for_source(
    label: &str,
    source: &str,
    declaration_name: &str,
) -> (
    Arc<HirModule>,
    Arc<super::HirProjectEvaluationTopology>,
    CallableDeclarationKey,
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
        .find(|symbol| {
            symbol.source_owner() == HirCallableSourceOwner::Item
                && symbol.declaration().name() == declaration_name
        })
        .expect("item callable")
        .declaration()
        .clone();
    let topology = evaluation_topology(&project, &symbols);
    (retained_module, topology, declaration)
}

#[test]
fn runtime_reachability_rejects_a_foreign_topology_generation() {
    let (database, package, root_path, module) = root_module_fixture("foreign-runtime-topology");
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .expect("project");
    let symbols_for = |profile: &str| {
        let world = ProjectSymbolWorldId::try_new(
            package.clone(),
            module.provenance().source_identity().id().clone(),
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
            .expect("external declarations");
        ProjectSymbolTable::link(project.view(), &externals)
            .expect("linked symbols")
            .into_table()
    };
    let accepted_symbols = symbols_for("accepted-runtime-topology");
    let foreign_symbols = symbols_for("foreign-runtime-topology");
    let executable = project.executable_view().expect("executable project");
    let accepted = evaluation_topology(&project, &accepted_symbols);
    let foreign = evaluation_topology(&project, &foreign_symbols);
    let input = HirRuntimeSemanticReachabilityInput::try_new(
        HirRuntimeEmissionMode::CheckAll,
        accepted.generation().symbol_world().clone(),
        accepted.generation().symbol_revision(),
        Vec::new(),
        Vec::new(),
    )
    .expect("reachability input");

    assert!(matches!(
        executable.runtime_semantic_reachability(
            input,
            foreign.as_ref(),
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        ),
        Err(HirRuntimeReachabilityError::TopologyGenerationMismatch)
    ));
}

fn assert_expression_owned_edges_have_semantic_paths(
    module: &HirModule,
    paths: &super::HirSemanticPathIndex,
    owner: ExprId,
) {
    let expression = module.resolve_expr(owner).expect("owned expression");
    let edges = expression
        .kind()
        .expression_owned_child_edges()
        .expect("bounded owned topology");
    assert!(!edges.is_empty(), "fixture must contain owned roots");
    let owner_hops = paths
        .expression(owner)
        .expect("owned expression path")
        .hops();
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
        assert!(
            path.steps()
                .contains(&HirSemanticPathStep::ExpressionOwned(edge.role().clone()))
        );
        assert_eq!(path.hops(), owner_hops);
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
    let topology = evaluation_topology(&project, &symbols);
    let paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("semantic paths");
    let owner = retained_module
        .expressions()
        .find_map(|(owner, expression)| select(expression.kind()).then_some(owner))
        .expect("selected owned expression");
    assert_expression_owned_edges_have_semantic_paths(&retained_module, paths, owner);
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
fn semantic_body_rows_retain_empty_flow_item_and_direct_thread_bodies() {
    let (module, topology, declaration) = declaration_paths_for_source(
        "semantic-empty-body-rows",
        concat!(
            "flow empty_flow {}\n",
            "test @test.empty empty {}\n",
            "fn nested() { let worker = thread {} }\n",
        ),
        "nested",
    );
    let nested_paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("nested declaration paths");
    let thread_row = nested_paths
        .body_rows()
        .iter()
        .find(|row| {
            row.owner().expression_owner().is_some_and(|owner| {
                row.owner().expression_owned_role().is_none()
                    && module
                        .resolve_expr(owner)
                        .is_ok_and(|expression| matches!(expression.kind(), HirExprKind::Thread(_)))
            })
        })
        .expect("direct empty Thread body row");
    assert_eq!(thread_row.kind(), HirBodyKind::Thread);
    assert!(thread_row.children().is_empty());

    let flow_declaration = topology.modules()[0]
        .entries()
        .iter()
        .filter_map(|entry| entry.body())
        .find(|body| {
            module
                .resolve_item(body.source_item())
                .is_ok_and(|item| matches!(item.kind(), HirItemKind::Flow(_)))
        })
        .expect("empty Flow declaration")
        .declaration()
        .clone();
    let flow_paths = topology
        .declaration_semantic_paths(&flow_declaration)
        .expect("empty Flow paths");
    assert!(flow_paths.body_rows().iter().any(|row| {
        row.owner().declaration_role() == Some(HirDeclarationBodyRootRole::FlowBody)
            && row.kind() == HirBodyKind::Thread
            && row.children().is_empty()
    }));

    let item_row = topology.modules()[0]
        .entries()
        .iter()
        .find(|entry| {
            entry
                .roots()
                .iter()
                .any(|root| matches!(root.role(), super::HirDeclarationItemRootRole::TestBody))
        })
        .expect("empty test item entry")
        .paths()
        .body_rows()
        .iter()
        .find(|row| row.owner().item_role() == Some(&super::HirDeclarationItemRootRole::TestBody))
        .expect("empty item body row");
    assert_eq!(item_row.kind(), HirBodyKind::Ordinary);
    assert!(item_row.children().is_empty());
}

#[test]
fn semantic_body_rows_retain_empty_await_and_choice_thread_bodies_without_conceptual_rows() {
    let (module, topology, declaration) = declaration_paths_for_source(
        "semantic-empty-nested-body-rows",
        concat!(
            "flow nested() {\n",
            "    try await task with { pending progress => {} }\n",
            "    choice @choice.opening {\n",
            "        @.listen \"Listen\" -> @flow.listen\n",
            "    } with {\n",
            "        timeout 10s {}\n",
            "        cancel on input(.BackToTitle) {}\n",
            "        on select selected {}\n",
            "    }\n",
            "}\n",
        ),
        "nested",
    );
    let paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("nested declaration paths");
    let await_rows = paths
        .body_rows()
        .iter()
        .filter(|row| {
            matches!(
                row.owner().expression_owned_role(),
                Some(crate::expr::HirExpressionOwnedBodyRole::AwaitBranchBody { .. })
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(await_rows.len(), 1);
    assert!(await_rows[0].children().is_empty());

    let choice_rows = paths
        .body_rows()
        .iter()
        .filter(|row| {
            matches!(
                row.owner().expression_owned_role(),
                Some(
                    crate::expr::HirExpressionOwnedBodyRole::ChoiceOptionSelectBody { .. }
                        | crate::expr::HirExpressionOwnedBodyRole::ChoicePlanTimeoutBody { .. }
                        | crate::expr::HirExpressionOwnedBodyRole::ChoicePlanCancelBody { .. }
                        | crate::expr::HirExpressionOwnedBodyRole::ChoicePlanOnSelectBody { .. }
                )
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(choice_rows.len(), 3);
    assert!(
        choice_rows
            .iter()
            .all(|row| { row.kind() == HirBodyKind::Thread && row.children().is_empty() })
    );
    assert!(paths.body_rows().iter().all(|row| {
        !matches!(
            row.owner().expression_owned_role(),
            Some(
                crate::expr::HirExpressionOwnedBodyRole::ChoiceLetStatement { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::ChoiceForPattern { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::ChoiceMatchArmPattern { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::ChoiceOptionForPattern { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::ChoiceOptionLetStatement { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::DialogueLinePlanStatement { .. }
                    | crate::expr::HirExpressionOwnedBodyRole::DialogueLinePlanLet { .. }
            )
        )
    }));
    assert!(
        module
            .expressions()
            .any(|(_, expression)| matches!(expression.kind(), HirExprKind::Choice(_)))
    );
}

#[test]
fn semantic_body_rows_retain_ordinary_and_thread_statement_bodies_and_match_expression_wrappers() {
    let (module, topology, declaration) = declaration_paths_for_source(
        "semantic-statement-body-rows",
        concat!(
            "fn ordinary() {\n",
            "    if true {}\n",
            "    match true { true => 1 }\n",
            "    let done = 0\n",
            "}\n",
            "flow threaded() {\n",
            "    if true {}\n",
            "    match true { true => 1 }\n",
            "}\n",
        ),
        "ordinary",
    );
    let ordinary_paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("ordinary declaration paths");
    let if_statement = module
        .statements()
        .find_map(|(owner, value)| {
            (matches!(value.kind(), HirStmtKind::If(_))
                && ordinary_paths.statement(owner).is_some())
            .then_some(owner)
        })
        .expect("ordinary if statement");
    assert!(ordinary_paths.body_rows().iter().any(|row| {
        row.owner().statement_owner() == Some(if_statement)
            && row.owner().statement_role() == Some(crate::stmt::HirStatementBodyRole::Then)
            && row.kind() == HirBodyKind::Ordinary
            && row.children().is_empty()
    }));
    let match_statement = module
        .statements()
        .find_map(|(owner, value)| {
            (matches!(value.kind(), HirStmtKind::Match(_))
                && ordinary_paths.statement(owner).is_some())
            .then_some(owner)
        })
        .expect("ordinary match statement");
    let match_wrapper = ordinary_paths
        .body_rows()
        .iter()
        .find(|row| {
            row.owner().statement_owner() == Some(match_statement)
                && row.owner().statement_role()
                    == Some(crate::stmt::HirStatementBodyRole::MatchArm { arm: 0 })
        })
        .expect("expression match-arm wrapper");
    assert_eq!(match_wrapper.kind(), HirBodyKind::Expression);
    assert_eq!(match_wrapper.children().len(), 1);

    let threaded_declaration = topology.modules()[0]
        .entries()
        .iter()
        .filter_map(|entry| entry.body())
        .find(|body| body.declaration().name() == "threaded")
        .expect("threaded declaration");
    assert!(threaded_declaration.paths().body_rows().iter().any(|row| {
        row.owner().statement_role() == Some(crate::stmt::HirStatementBodyRole::Then)
            && row.kind() == HirBodyKind::Thread
            && row.children().is_empty()
    }));
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
fn nested_postfix_dialogue_candidates_publish_each_expression_once() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/nested-postfix-dialogue-paths",
        "nested-postfix-dialogue-paths.arcw",
        concat!(
            "pub character alice { display_name = \"Alice\" }\n",
            "flow opening {\n",
            "    alice[Hello[p]]\n",
            "}\n",
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
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .expect("executable project");
    let symbols = symbols_for_project(&project, parsed.document(), "nested-postfix-dialogue-paths");
    let topology = evaluation_topology(&project, &symbols);
    let module_topology = topology
        .module(module.module_id())
        .expect("module evaluation topology");
    let expected = module
        .expressions()
        .map(|(owner, _)| owner)
        .collect::<BTreeSet<_>>();
    let actual = module_topology.expression_owners().collect::<BTreeSet<_>>();
    assert_eq!(actual, expected);
    assert_eq!(
        module_topology.expression_uses().rows().len(),
        expected.len()
    );

    for (owner, expression) in module.expressions() {
        let target_role = match expression.kind() {
            HirExprKind::PostfixBracket(_) | HirExprKind::Index(_) => {
                Some(HirExpressionChildRole::Target)
            }
            HirExprKind::DialogueContentApplication(_) => {
                Some(HirExpressionChildRole::DialogueTarget)
            }
            _ => None,
        };
        let Some(target_role) = target_role else {
            continue;
        };
        let expected_ownership = match expression.kind() {
            HirExprKind::PostfixBracket(_) => HirExpressionChildOwnership::Owning,
            HirExprKind::Index(_) | HirExprKind::DialogueContentApplication(_) => {
                HirExpressionChildOwnership::ReferenceOnly
            }
            _ => unreachable!(),
        };
        assert!(module_topology.expression_edges(owner).iter().any(|edge| {
            matches!(
                edge,
                super::HirExpressionEvaluationEdge::Expression {
                    role,
                    ownership,
                    ..
                } if role == &target_role && *ownership == expected_ownership
            )
        }));
    }
}

#[test]
fn semantic_owner_hops_survive_closure_block_statement_and_initializer_walkers() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/semantic-owner-hops",
        "semantic-owner-hops.arcw",
        concat!(
            "fn accepted() -> Unit {\n",
            "    let callback: (Unit) -> Unit = |_unit: Unit| -> Unit {\n",
            "        let inner = ()\n",
            "        inner\n",
            "    }\n",
            "    ()\n",
            "}\n",
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
    .expect("semantic-owner-hop project");
    let symbols = symbols_for_project(&project, parsed.document(), "semantic-owner-hops");
    let declaration = symbols
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "accepted")
        .expect("accepted declaration")
        .declaration()
        .clone();
    let topology = evaluation_topology(&project, &symbols);
    let paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("semantic paths");
    let (closure, closure_expression) = retained_module
        .expressions()
        .find_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Closure(closure) => Some((owner, closure)),
            _ => None,
        })
        .expect("closure expression");
    let block = closure_expression.body();
    let HirExprKind::Block(block_expression) = retained_module
        .resolve_expr(block)
        .expect("closure body")
        .kind()
    else {
        panic!("closure body is a block");
    };
    let [statement] = block_expression.statements() else {
        panic!("closure block has one initializer statement");
    };
    let initializer = match retained_module.resolve_stmt(*statement).unwrap().kind() {
        HirStmtKind::Let { initializer, .. } => *initializer,
        kind => panic!("unexpected closure statement: {kind:?}"),
    };
    let body_path = paths.expression(block).expect("closure block path");
    let parameter_pattern = closure_expression
        .parameters()
        .first()
        .expect("closure parameter")
        .pattern();
    let parameter_path = paths
        .pattern(parameter_pattern)
        .expect("closure parameter pattern path");
    assert!(parameter_path.steps().iter().any(|step| matches!(
        step,
        HirSemanticPathStep::ExpressionOwned(
            crate::expr::HirExpressionOwnedBodyRole::ClosureParameterPattern { parameter: 0 }
        )
    )));
    let statement_path = paths.statement(*statement).expect("closure statement path");
    let initializer_path = paths.expression(initializer).expect("initializer path");
    assert_eq!(statement_path.hops(), body_path.hops());
    assert_eq!(initializer_path.hops(), body_path.hops());
    assert_eq!(body_path.hops().len(), 1);
    let [hop] = body_path.hops() else {
        panic!("one closure-to-block expression hop");
    };
    assert_eq!(hop.parent(), closure);
    assert_eq!(hop.child(), block);
}

#[test]
fn declaration_body_topology_keeps_root_matrix_and_unified_path_index_parity() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/declaration-body-topology",
        "declaration-body-topology.arcw",
        concat!(
            "fn accepted(value: Unit = ()) effects { agent.observe } {\n",
            "    value\n",
            "}\n",
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
    .expect("topology project");
    let symbols = symbols_for_project(&project, parsed.document(), "declaration-body-topology");
    let symbol = symbols
        .callable_symbols()
        .find(|symbol| symbol.declaration().name() == "accepted")
        .expect("accepted declaration");
    let declaration = symbol.declaration().clone();
    let project_topology = evaluation_topology(&project, &symbols);
    let declaration_view = project_topology
        .declaration(&declaration)
        .expect("declaration topology");
    let topology = declaration_view.body();
    assert_eq!(topology.declaration(), &declaration);
    assert_eq!(topology.source_item(), symbol.source_item());
    assert_eq!(topology.source_owner(), symbol.source_owner());
    assert_eq!(topology.snapshot(), retained_module.snapshot_id());
    assert!(matches!(
        topology.paths().root(),
        super::HirSemanticPathRoot::Declaration(value) if value == &declaration
    ));
    assert_eq!(topology.paths().snapshot(), topology.snapshot());
    assert_eq!(
        topology
            .parameter_roots()
            .iter()
            .map(HirDeclarationParameterRoot::role)
            .collect::<Vec<_>>(),
        vec![
            HirDeclarationParameterRootRole::Pattern {
                group: 0,
                parameter: 0,
            },
            HirDeclarationParameterRootRole::Default {
                group: 0,
                parameter: 0,
            },
        ]
    );
    let contracts = topology.contract_roots();
    assert_eq!(contracts.len(), 1);
    assert_eq!(
        contracts[0].role(),
        HirDeclarationContractRootRole::EffectOperand {
            clause: 0,
            family: super::HirFlowContractRootFamily::Effects,
            operand: 0
        }
    );
    let roots = topology.roots();
    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].projection().kind(), HirBodyKind::Ordinary);
    assert_eq!(roots[0].role(), HirDeclarationBodyRootRole::FunctionBody);
    let delegated_paths = project_topology
        .declaration_semantic_paths(&declaration)
        .expect("delegated semantic path index");
    assert_eq!(topology.paths(), delegated_paths);
    assert_eq!(
        declaration_view,
        project_topology
            .declaration(&declaration)
            .expect("deterministic declaration topology")
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the project seal test checks paths, locals, captures, and deterministic reconstruction"
)]
fn project_evaluation_topology_seals_source_order_and_local_origins() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/project-evaluation-topology",
        "project-evaluation-topology.arcw",
        concat!(
            "fn accepted(input: Unit) -> Unit {\n",
            "    let value = input\n",
            "    value\n",
            "}\n",
        ),
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
    .expect("evaluation topology project");
    let symbols = symbols_for_project(&project, parsed.document(), "project-evaluation-topology");
    let executable = project.executable_view().expect("executable project");
    let witness = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation");
    let project_generation = Arc::clone(witness.generation());
    let topology = witness
        .into_evaluation_topology()
        .expect("project evaluation topology");
    assert_eq!(topology.modules().len(), 1);
    let module_topology = &topology.modules()[0];
    assert!(Arc::ptr_eq(topology.generation(), &project_generation));
    assert!(Arc::ptr_eq(
        module_topology.generation(),
        topology
            .generation()
            .module(&CanonicalModulePath::crate_root())
            .expect("root module generation")
    ));
    assert_eq!(module_topology.entries().len(), 1);
    assert_eq!(module_topology.snapshot(), retained_module.snapshot_id());
    let item = retained_module.source_ordered_items()[0];
    let item_entry = &module_topology.entries()[0];
    assert_eq!(item_entry.item(), item);
    assert_eq!(
        item_entry.family().family(),
        retained_module
            .resolve_item(item)
            .expect("accepted item")
            .family()
    );
    assert_eq!(item_entry.entry_ordinal(), 0);
    assert_eq!(
        item_entry.paths().root(),
        &super::HirSemanticPathRoot::Item {
            item,
            entry_ordinal: 0,
            role: super::HirItemEvaluationEntryRole::Item,
        }
    );
    assert_eq!(
        module_topology.expression_owners().collect::<BTreeSet<_>>(),
        retained_module
            .expressions()
            .map(|(expression, _)| expression)
            .collect::<BTreeSet<_>>()
    );
    assert!(matches!(
        module_topology.entries()[0].role(),
        super::HirItemEvaluationEntryRole::Item
    ));
    assert!(module_topology.entries()[0].body().is_some());
    let value = retained_module
        .locals()
        .find(|(_, local)| local.name().as_str() == "value")
        .expect("value local")
        .0;
    let initializer = retained_module
        .statements()
        .find_map(|(_, statement)| match statement.kind() {
            HirStmtKind::Let {
                initializer,
                locals,
                ..
            } if locals.as_ref() == [value].as_slice() => Some(*initializer),
            _ => None,
        })
        .expect("value initializer");
    assert_eq!(
        module_topology.local_origins().origin(value),
        Some(super::HirLocalValueOrigin::DirectInitializer(initializer))
    );
    let binding = module_topology
        .local_origins()
        .binding(value)
        .expect("binding origin site");
    assert_eq!(binding.local(), value);
    assert!(binding.statement().is_some());
    assert_eq!(binding.value(), Some(initializer));
    assert!(binding.pattern().is_some());
    assert_eq!(
        binding.statement_role(),
        Some(super::HirLocalBindingStatementRole::Let)
    );
    assert_eq!(
        topology,
        executable
            .accept_symbol_generation(&symbols)
            .expect("accepted symbol generation")
            .into_evaluation_topology()
            .expect("deterministic project topology")
    );
}

#[test]
fn capture_and_expression_use_indexes_preserve_regions_order_and_access() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().expect("syntax database");
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/capture-use-index",
        "capture-use-index.arcw",
        concat!(
            "fn accepted(input: i64, other: i64) -> Unit {\n",
            "    let mut target = input\n",
            "    target = other\n",
            "    let callback = || -> i64 { other + input }\n",
            "    let empty = || -> Unit { () }\n",
            "    let abstraction = {\n",
            "        let internal = input\n",
            "        let nested = || { _ + input }\n",
            "        consume(_)\n",
            "        _ + internal\n",
            "    }\n",
            "    ()\n",
            "}\n",
        ),
    );
    assert!(
        parsed.diagnostics().is_empty(),
        "{:?}",
        parsed.diagnostics()
    );
    let mut database = HirDatabase::try_new().expect("HIR database");
    let module = lower(&mut database, &parsed, &package, &root_path);
    assert!(
        module.diagnostics().is_empty(),
        "{:?}",
        module.diagnostics()
    );
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .expect("project");
    let symbols = symbols_for_project(&project, parsed.document(), "capture-use-index");
    let topology = evaluation_topology(&project, &symbols);
    let module_topology = topology
        .module(module.module_id())
        .expect("module topology");

    let assignment = module
        .statements()
        .find_map(|(_, statement)| match statement.kind() {
            HirStmtKind::Assign { target, value } => Some((*target, *value)),
            _ => None,
        })
        .expect("assignment");
    assert_eq!(
        module_topology
            .expression_uses()
            .row(assignment.0)
            .expect("assignment target use")
            .capture_access(),
        crate::scope::CaptureAccess::Reassign,
    );
    assert_eq!(
        module_topology
            .expression_uses()
            .row(assignment.1)
            .expect("assignment value use")
            .capture_access(),
        crate::scope::CaptureAccess::Read,
    );

    assert_capture_and_region_indexes(&module, module_topology);
}

fn assert_capture_and_region_indexes(
    module: &HirModule,
    module_topology: &super::HirModuleEvaluationTopology,
) {
    let closures = module
        .expressions()
        .filter_map(|(owner, expression)| match expression.kind() {
            HirExprKind::Closure(closure) => Some((owner, closure)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert!(closures.len() >= 3, "fixture retains all explicit closures");
    for (owner, closure) in &closures {
        let rows = module_topology
            .captures()
            .captures_for_closure(*owner)
            .expect("every closure, including an empty closure, owns one range");
        assert_eq!(
            rows.iter()
                .map(super::semantic_paths::HirCaptureEvaluationRow::capture)
                .collect::<Vec<_>>(),
            closure.captures(),
        );
    }
    assert!(closures.iter().any(|(owner, closure)| {
        closure.captures().is_empty()
            && module_topology.captures().captures_for_closure(*owner) == Some(&[])
    }));

    let local = |name: &str| {
        module
            .locals()
            .find_map(|(owner, local)| (local.name().as_str() == name).then_some(owner))
            .unwrap_or_else(|| panic!("local `{name}`"))
    };
    let internal = module_topology
        .local_origins()
        .binding(local("internal"))
        .expect("internal binding origin");
    let region_root = internal
        .binding_expression()
        .expect("statement binding retains its expression region");
    let region = module_topology
        .expression_uses()
        .implicit_callable_region(
            region_root,
            crate::expr::HirPlaceholderKind::PartialApplication,
        )
        .expect("implicit callable region");
    assert!(region.contains_binding(internal));
    assert!(
        !region.contains_binding(
            module_topology
                .local_origins()
                .binding(local("input"))
                .expect("parameter binding"),
        )
    );
    let all_placeholders = module
        .expressions()
        .filter_map(|(owner, expression)| {
            matches!(
                expression.kind(),
                HirExprKind::Placeholder(crate::expr::HirPlaceholderKind::PartialApplication)
            )
            .then_some(owner)
        })
        .collect::<BTreeSet<_>>();
    let region_placeholders = region.placeholders().collect::<BTreeSet<_>>();
    assert!(all_placeholders.len() >= 3);
    assert_eq!(region_placeholders.len(), 1);
    assert!(region_placeholders.is_subset(&all_placeholders));
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the item-root matrix keeps every non-callable and inline-member role visible"
)]
fn project_item_entries_retain_rooted_paths_in_source_order() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/item-entry-paths",
        "item-entry-paths.arcw",
        concat!(
            "#[tool.fixture(1)]\n",
            "pub res @image.room room: std.presentation.Image {\n",
            "    asset = @asset.bg.room\n",
            "    visible = true\n",
            "}\n",
            "#[launch(primary)]\n",
            "entry server @entry.http {\n",
            "    budget = policy(1 + 2)\n",
            "}\n",
            "#[tool.fixture(1)]\n",
            "test @test.scenario scenario {\n",
            "    true\n",
            "}\n",
            "bench @bench.score {\n",
            "    setup { true }\n",
            "    measure { false }\n",
            "    report { true }\n",
            "}\n",
            "#[tool.flag(1)]\n",
            "style Theme {\n",
            "    token color.text: Color = white\n",
            "}\n",
            "trait Base {\n",
            "    #[member.flag(2)]\n",
            "    fn run(self) -> Int\n",
            "}\n",
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
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .expect("item-root topology project");
    let symbols = symbols_for_project(&project, parsed.document(), "item-entry-paths");
    let topology = project
        .executable_view()
        .expect("executable item-root project")
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("item-root topology");
    let entries = topology.modules()[0].entries();
    assert_eq!(entries.len(), 7);
    for (ordinal, entry) in entries.iter().enumerate() {
        assert_eq!(entry.entry_ordinal(), u32::try_from(ordinal).unwrap());
        assert_eq!(
            entry.paths().root(),
            &super::HirSemanticPathRoot::Item {
                item: entry.item(),
                entry_ordinal: u32::try_from(ordinal).unwrap(),
                role: entry.role(),
            }
        );
        assert!(matches!(
            entry.paths().root(),
            super::HirSemanticPathRoot::Item { .. }
        ));
        for root in entry.roots() {
            for edge in root.projection().children() {
                match edge.child() {
                    HirBodyChild::Expression(expression) => {
                        assert!(entry.paths().expression(expression).is_some());
                        let location = topology
                            .semantic_path(expression.into())
                            .expect("unique body expression path")
                            .expect("body expression path lookup");
                        assert_eq!(location.root(), entry.paths().root());
                    }
                    HirBodyChild::Statement(statement) => {
                        assert!(entry.paths().statement(statement).is_some());
                    }
                }
            }
        }
    }
    assert!(module.statements().next().is_some());
    for (statement, _) in module.statements() {
        let location = topology
            .semantic_path(statement.into())
            .expect("unique statement path")
            .expect("statement path lookup");
        assert!(topology.modules()[0].entries().iter().any(|entry| {
            (entry.paths().root() == location.root()
                && entry.paths().statement(statement) == Some(location.path()))
                || entry.body().is_some_and(|body| {
                    body.paths().root() == location.root()
                        && body.paths().statement(statement) == Some(location.path())
                })
        }));
    }
    assert!(module.patterns().next().is_some());
    for (pattern, _) in module.patterns() {
        let location = topology
            .semantic_path(pattern.into())
            .expect("unique pattern path")
            .expect("pattern path lookup");
        assert!(topology.modules()[0].entries().iter().any(|entry| {
            (entry.paths().root() == location.root()
                && entry.paths().pattern(pattern) == Some(location.path()))
                || entry.body().is_some_and(|body| {
                    body.paths().root() == location.root()
                        && body.paths().pattern(pattern) == Some(location.path())
                })
        }));
    }
    assert!(entries[0].roots().iter().any(|root| matches!(
        root.role(),
        super::HirDeclarationItemRootRole::ResourceField { field: 0 }
    )));
    assert!(entries[0].roots().iter().any(|root| matches!(
        root.role(),
        super::HirDeclarationItemRootRole::AttributeArgument { .. }
    )));
    assert!(entries[1].roots().iter().any(|root| matches!(
        root.role(),
        super::HirDeclarationItemRootRole::EntryOption { .. }
    )));
    assert!(
        entries[2]
            .roots()
            .iter()
            .any(|root| matches!(root.role(), super::HirDeclarationItemRootRole::TestBody))
    );
    assert!(
        entries[3]
            .roots()
            .iter()
            .any(|root| matches!(root.role(), super::HirDeclarationItemRootRole::BenchBody))
    );
    assert!(
        entries[4]
            .roots()
            .iter()
            .any(|root| matches!(root.role(), super::HirDeclarationItemRootRole::Style { .. }))
    );
    assert!(entries[6].roots().iter().any(|root| matches!(
        root.role(),
        super::HirDeclarationItemRootRole::AttributeArgument {
            owner: super::HirItemAttributeOwner::InlineMember { member: 0 },
            ..
        }
    )));
    assert!(entries[5].body().is_none());
    assert!(entries[6].body().is_some());
}

#[test]
#[allow(
    clippy::match_same_arms,
    clippy::too_many_lines,
    reason = "the activity path test checks all member-local ownership boundaries"
)]
fn activity_member_bindings_belong_only_to_the_primary_item_path_index() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let parsed = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/activity-item-paths",
        "activity-item-paths.arcw",
        concat!(
            "pub activity TruckGame {\n",
            "    mode = checkpointed_realtime\n",
            "    lifecycle = snapshot\n",
            "    input {\n",
            "        controls: Stream<InputEvent, InputError>\n",
            "        seed: u64\n",
            "    }\n",
            "    output {\n",
            "        result: TruckResult\n",
            "    }\n",
            "}\n",
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
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .expect("activity item topology project");
    let symbols = symbols_for_project(&project, parsed.document(), "activity-item-paths");
    let topology = project
        .executable_view()
        .expect("executable activity project")
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("activity item topology");
    let activity_item = module
        .source_ordered_items()
        .iter()
        .copied()
        .find(|item| {
            matches!(
                module.resolve_item(*item).unwrap().kind(),
                HirItemKind::Activity(_)
            )
        })
        .expect("activity item");
    let activity = module.resolve_item(activity_item).unwrap();
    let member_locals = activity
        .members()
        .iter()
        .copied()
        .filter_map(|member| {
            let member = module.declaration_members().resolve(member).unwrap();
            match member.kind() {
                HirDeclarationMemberKind::ActivityInput(value) => value.local(),
                HirDeclarationMemberKind::ActivityOutput(value) => value.local(),
                _ => None,
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(member_locals.len(), 3);
    let entries = topology.modules()[0].entries();
    let primary = entries
        .iter()
        .find(|entry| entry.item() == activity_item)
        .expect("primary activity entry");
    for local in member_locals {
        let path = primary
            .paths()
            .local(local)
            .expect("activity member local item path");
        let location = topology
            .semantic_path(local.into())
            .expect("unique activity local path")
            .expect("activity local path lookup");
        assert_eq!(location.root(), primary.paths().root());
        assert_eq!(location.path(), path);
        assert!(matches!(
            path.steps().first(),
            Some(HirSemanticPathStep::DeclarationMember { .. })
        ));
        assert!(matches!(
            primary.paths().root(),
            super::HirSemanticPathRoot::Item { .. }
        ));
        for entry in entries {
            if entry.item() != activity_item {
                assert!(entry.paths().local(local).is_none());
                assert!(
                    entry
                        .body()
                        .is_none_or(|body| body.paths().local(local).is_none())
                );
            }
        }
    }
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

    let topology = evaluation_topology(&project, &symbols);
    let paths = topology
        .declaration_semantic_paths(&declaration)
        .expect("View has executable semantic roots");
    assert!(matches!(
        paths.root(),
        super::HirSemanticPathRoot::Declaration(value) if value == &declaration
    ));
    assert_eq!(
        paths
            .pattern(parameter.pattern())
            .map(HirSemanticOwnerPath::steps),
        Some(
            [HirSemanticPathStep::ParameterPattern {
                group: 0,
                parameter: 0,
            }]
            .as_slice()
        )
    );
    assert_eq!(
        paths
            .expression(parameter.default().expect("default"))
            .map(HirSemanticOwnerPath::steps),
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
            paths.expression(value).map(HirSemanticOwnerPath::steps),
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
        topology
            .declaration_semantic_paths(&declaration)
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
    let topology = evaluation_topology(&project, &symbols);
    let paths = topology
        .declaration_semantic_paths(declaration)
        .expect("empty View path index");
    assert!(matches!(
        paths.root(),
        super::HirSemanticPathRoot::Declaration(value) if value == declaration
    ));
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

    let result = second_project
        .executable_view()
        .unwrap()
        .accept_symbol_generation(&first_symbols);
    assert!(matches!(
        result,
        Err(super::AcceptedHirProjectSymbolGenerationError::SourceIdentityMismatch { .. })
    ));
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
    let project_topology = evaluation_topology(&project, &symbols);
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
        let topology = project_topology
            .declaration(symbol.declaration())
            .expect("bodyless declaration topology");
        assert!(topology.body().roots().is_empty());
        assert!(matches!(
            topology.body().paths().root(),
            super::HirSemanticPathRoot::Declaration(value) if value == symbol.declaration()
        ));
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
    let project_topology = evaluation_topology(&project, &symbols);
    assert_declaration_root_matrix(&retained_module, &project_topology, &symbols);
}

fn assert_declaration_root_matrix(
    retained_module: &HirModule,
    project_topology: &super::HirProjectEvaluationTopology,
    symbols: &ProjectSymbolTable,
) {
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
            let paths = project_topology
                .declaration_semantic_paths(symbol.declaration())
                .expect("declaration semantic paths");
            let rooted = retained_module.expressions().any(|(expression, _)| {
                paths.expression(expression).is_some_and(|path| {
                    path.steps().first() == Some(&HirSemanticPathStep::DeclarationBody(root))
                })
            });
            assert!(rooted, "missing {owner:?} declaration root");
            assert!(retained_module.patterns().any(|(pattern, _)| matches!(
                paths.pattern(pattern).map(HirSemanticOwnerPath::steps),
                Some([HirSemanticPathStep::ParameterPattern {
                    group: 0,
                    parameter: 0
                }])
            )));
            let has_default = retained_module.expressions().any(|(expression, _)| {
                matches!(
                    paths
                        .expression(expression)
                        .map(HirSemanticOwnerPath::steps),
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
                project_topology
                    .declaration_semantic_paths(symbol.declaration())
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
    let symbols = symbols_for_project(&project, root_source.document(), "selected-expression");
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("selected expression topology");

    assert_selected_graph_rejections(executable, &topology, owner, foreign);

    assert_selected_index_graph(executable, &topology, owner, target, index, &index_children);
    assert_selected_dialogue_graph(
        executable,
        &topology,
        owner,
        target,
        index,
        dialogue,
        &dialogue_children,
    );

    assert_runtime_postfix_expression_type_inventory(RuntimePostfixExpressionTypeInventory {
        executable,
        topology: &topology,
        owner,
        target,
        index,
        dialogue,
        index_children: index_children.into_boxed_slice(),
        dialogue_children: dialogue_children.into_boxed_slice(),
    });
}

fn assert_selected_graph_rejections(
    executable: HirExecutableProjectView<'_>,
    topology: &Arc<super::HirProjectEvaluationTopology>,
    owner: ExprId,
    foreign: ExprId,
) {
    assert_eq!(
        executable.selected_expression_graph(topology, |_| None, |_| None),
        Err(HirSelectedExpressionInventoryError::MissingPostfixSelection { expression: owner })
    );
    assert_eq!(
        executable.selected_expression_graph(
            topology,
            |candidate_owner| (candidate_owner == owner).then_some(foreign),
            |_| None,
        ),
        Err(
            HirSelectedExpressionInventoryError::InvalidPostfixSelection {
                expression: owner,
                candidate: foreign,
            }
        )
    );
}

fn assert_selected_index_graph(
    executable: HirExecutableProjectView<'_>,
    topology: &Arc<super::HirProjectEvaluationTopology>,
    owner: ExprId,
    target: ExprId,
    index: ExprId,
    index_children: &[ExprId],
) {
    let selected_graph = executable
        .selected_expression_graph(
            topology,
            |candidate_owner| (candidate_owner == owner).then_some(index),
            |_| None,
        )
        .expect("selected index graph");
    assert!(selected_graph.expression_edges(owner).iter().any(|edge| {
        matches!(
            edge,
            super::HirExpressionEvaluationEdge::Expression {
                role: crate::expr::HirExpressionChildRole::PostfixIndexCandidate,
                ownership: crate::expr::HirExpressionChildOwnership::Owning,
                child,
            } if *child == index
        )
    }));
    assert!(!selected_graph.expression_edges(owner).iter().any(|edge| {
        matches!(
            edge,
            super::HirExpressionEvaluationEdge::Expression {
                role: crate::expr::HirExpressionChildRole::PostfixDialogueCandidate,
                ..
            }
        )
    }));
    assert!(!selected_graph.expression_edges(index).iter().any(|edge| {
        matches!(
            edge,
            super::HirExpressionEvaluationEdge::Expression {
                ownership: crate::expr::HirExpressionChildOwnership::ReferenceOnly,
                ..
            }
        )
    }));
    let selected = selected_graph.expression_owners().collect::<BTreeSet<_>>();
    assert!(selected.contains(&owner));
    assert!(selected.contains(&target));
    assert!(selected.contains(&index));
    assert!(index_children.iter().all(|child| selected.contains(child)));
}

fn assert_selected_dialogue_graph(
    executable: HirExecutableProjectView<'_>,
    topology: &Arc<super::HirProjectEvaluationTopology>,
    owner: ExprId,
    target: ExprId,
    index: ExprId,
    dialogue: ExprId,
    dialogue_children: &[ExprId],
) {
    let dialogue_graph = executable
        .selected_expression_graph(
            topology,
            |candidate_owner| (candidate_owner == owner).then_some(dialogue),
            |_| None,
        )
        .expect("selected dialogue graph");
    assert!(dialogue_graph.expression_edges(owner).iter().any(|edge| {
        matches!(
            edge,
            super::HirExpressionEvaluationEdge::Expression {
                role: crate::expr::HirExpressionChildRole::PostfixDialogueCandidate,
                ownership: crate::expr::HirExpressionChildOwnership::Owning,
                child,
            } if *child == dialogue
        )
    }));
    assert!(!dialogue_graph.expression_edges(owner).iter().any(|edge| {
        matches!(
            edge,
            super::HirExpressionEvaluationEdge::Expression {
                role: crate::expr::HirExpressionChildRole::PostfixIndexCandidate,
                ..
            }
        )
    }));
    assert!(
        !dialogue_graph
            .expression_edges(dialogue)
            .iter()
            .any(|edge| {
                matches!(
                    edge,
                    super::HirExpressionEvaluationEdge::Expression {
                        ownership: crate::expr::HirExpressionChildOwnership::ReferenceOnly,
                        ..
                    }
                )
            })
    );
    let selected = dialogue_graph.expression_owners().collect::<BTreeSet<_>>();
    assert!(selected.contains(&target));
    assert!(selected.contains(&dialogue));
    assert!(!selected.contains(&index));
    assert!(
        dialogue_children
            .iter()
            .filter(|child| **child != target)
            .all(|child| selected.contains(child))
    );
}

#[test]
fn selected_expression_inventory_rejects_a_foreign_project_topology() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut first_syntax = SyntaxDatabase::try_new().unwrap();
    let first_source = parse_initial(
        &mut first_syntax,
        "arcweft-test://proof/final-project/selection-foreign-first",
        "selection-foreign-first.arcw",
        "fn first() { 1 }\n",
    );
    let mut first_database = HirDatabase::try_new().unwrap();
    let first_module = lower(&mut first_database, &first_source, &package, &root_path);
    let first_project = build_project(
        &first_database,
        package.clone(),
        [bind(&first_database, &package, &root_path, first_module)],
    )
    .unwrap();
    let first_executable = first_project.executable_view().unwrap();

    let mut second_syntax = SyntaxDatabase::try_new().unwrap();
    let second_source = parse_initial(
        &mut second_syntax,
        "arcweft-test://proof/final-project/selection-foreign-second",
        "selection-foreign-second.arcw",
        "fn second() { 2 }\n",
    );
    let mut second_database = HirDatabase::try_new().unwrap();
    let second_module = lower(&mut second_database, &second_source, &package, &root_path);
    let second_project = build_project(
        &second_database,
        package.clone(),
        [bind(&second_database, &package, &root_path, second_module)],
    )
    .unwrap();
    let second_symbols = symbols_for_project(
        &second_project,
        second_source.document(),
        "selection-foreign-second",
    );
    let second_topology = second_project
        .executable_view()
        .unwrap()
        .accept_symbol_generation(&second_symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .unwrap();

    assert_eq!(
        first_executable.selected_expression_graph(&second_topology, |_| None, |_| None),
        Err(HirSelectedExpressionInventoryError::TopologyMismatch)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the lowered integration fixture checks all nested statement/body transitions together"
)]
fn selected_topology_uses_statement_plan_order_for_nested_control_bodies() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut syntax = SyntaxDatabase::try_new().unwrap();
    let source = parse_initial(
        &mut syntax,
        "arcweft-test://proof/final-project/selected-statement-plan-order",
        "selected-statement-plan-order.arcw",
        concat!(
            "flow mixed() {\n",
            "    if true { 1 } else if false { 2 } else { 3 }\n",
            "    match true {\n",
            "        true => thread worker { 4 },\n",
            "        false => { 5 },\n",
            "    }\n",
            "    select {\n",
            "        value = true => { 6 }\n",
            "    }\n",
            "}\n",
        ),
    );
    let mut database = HirDatabase::try_new().unwrap();
    let module = lower(&mut database, &source, &package, &root_path);
    assert!(module.is_executable(), "{:?}", module.diagnostics());
    let project = build_project(
        &database,
        package.clone(),
        [bind(&database, &package, &root_path, Arc::clone(&module))],
    )
    .unwrap();
    let executable = project.executable_view().unwrap();
    let symbols = symbols_for_project(&project, source.document(), "selected-statement-plan-order");
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("statement-plan topology");
    let module_topology = &topology.modules()[0];
    let all_expressions = module
        .expressions()
        .map(|(owner, _)| owner)
        .collect::<BTreeSet<_>>();
    let selected = executable
        .selected_expression_graph(&topology, |_| None, |_| None)
        .expect("selected statement-plan graph")
        .expression_owners()
        .collect::<BTreeSet<_>>();
    assert_eq!(selected, all_expressions);

    let if_statement = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::If(_)).then_some(owner)
        })
        .expect("nested if statement");
    let match_statement = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Match(_)).then_some(owner)
        })
        .expect("mixed match statement");
    let select_statement = module
        .statements()
        .find_map(|(owner, statement)| {
            matches!(statement.kind(), HirStmtKind::Select(_)).then_some(owner)
        })
        .expect("select statement");
    let if_condition = match module.resolve_stmt(if_statement).unwrap().kind() {
        HirStmtKind::If(value) => value.condition(),
        _ => unreachable!(),
    };
    let match_scrutinee = match module.resolve_stmt(match_statement).unwrap().kind() {
        HirStmtKind::Match(value) => value.scrutinee(),
        _ => unreachable!(),
    };
    let select_source = match module.resolve_stmt(select_statement).unwrap().kind() {
        HirStmtKind::Select(crate::stmt::HirSelectStmt::Branches { branches, .. }) => {
            match branches[0].head() {
                crate::stmt::HirSelectBranchHead::Bind { source, .. } => *source,
                _ => panic!("select source binding"),
            }
        }
        _ => panic!("select branches"),
    };
    let roots = module_topology.selection_roots();
    let position = |owner| {
        roots
            .iter()
            .position(|candidate| *candidate == owner)
            .unwrap()
    };
    assert!(position(if_condition) < position(match_scrutinee));
    assert!(position(match_scrutinee) < position(select_source));

    let thread_owner = module
        .expressions()
        .find_map(|(owner, expression)| {
            matches!(expression.kind(), HirExprKind::Thread(_)).then_some(owner)
        })
        .expect("thread arm expression");
    let thread_edges = module_topology.expression_edges(thread_owner);
    assert!(!thread_edges.is_empty());
    assert!(
        matches!(
            thread_edges[0],
            super::HirExpressionEvaluationEdge::Statement { .. }
        ),
        "thread edges: {thread_edges:?}"
    );
}

struct RuntimePostfixExpressionTypeInventory<'project, 'topology> {
    executable: HirExecutableProjectView<'project>,
    topology: &'topology Arc<super::HirProjectEvaluationTopology>,
    owner: ExprId,
    target: ExprId,
    index: ExprId,
    dialogue: ExprId,
    index_children: Box<[ExprId]>,
    dialogue_children: Box<[ExprId]>,
}

fn assert_runtime_postfix_expression_type_inventory(
    inventory: RuntimePostfixExpressionTypeInventory<'_, '_>,
) {
    let RuntimePostfixExpressionTypeInventory {
        executable,
        topology,
        owner,
        target,
        index,
        dialogue,
        index_children,
        dialogue_children,
    } = inventory;
    assert!(matches!(
        runtime_reachability(
            executable,
            topology,
            |_| None,
            |_| HirRuntimeExpressionTypeDisposition::Retain,
        ),
        Err(HirRuntimeReachabilityError::SelectedExpressions(
            HirSelectedExpressionInventoryError::MissingPostfixSelection { expression }
        )) if expression == owner
    ));
    let runtime_index = runtime_reachability(
        executable,
        topology,
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
        topology,
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
    let symbols = symbols_for_project(&project, parsed.document(), "runtime-expression-types");
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("runtime expression topology");

    let semantic = executable
        .selected_expression_graph(&topology, |_| None, |_| None)
        .expect("postfix-free semantic inventory");
    let semantic = semantic.expression_owners().collect::<BTreeSet<_>>();
    assert!(semantic.contains(&effect_root));
    assert!(
        effect_children.iter().all(|child| semantic.contains(child)),
        "semantic analysis retains the complete effect expression subtree"
    );

    let runtime_owners = runtime_reachability(
        executable,
        &topology,
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
    let (project, call, callee, argument, _, _, _, _, topology) = runtime_call_inventory_fixture();
    let executable = project.executable_view().unwrap();
    let retained = runtime_reachability(
        executable,
        &topology,
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
        &topology,
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
        &topology,
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
        &topology,
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
        &topology,
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
            &topology,
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
    let (project, _, _, _, call, callee, receiver, argument, topology) =
        runtime_call_inventory_fixture();
    let executable = project.executable_view().expect("executable fixture");
    let retained = runtime_reachability(
        executable,
        &topology,
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
    Arc<super::HirProjectEvaluationTopology>,
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
    let symbols = symbols_for_project(&project, parsed.document(), "runtime-expression-carrier");
    let topology = project
        .executable_view()
        .unwrap()
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("runtime carrier topology");
    (
        project,
        call,
        callee,
        argument,
        member_call,
        member_callee,
        receiver,
        member_argument,
        topology,
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
    let symbols = symbols_for_project(&project, parsed.document(), "runtime-owner-domain");
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("runtime owner topology");
    let inventory = runtime_reachability(
        executable,
        &topology,
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
    let symbols = symbols_for_project(&project, parsed.document(), "runtime-reachability-order");
    let topology = executable
        .accept_symbol_generation(&symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("runtime reachability topology");
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
    let world = topology.generation().symbol_world().clone();
    let revision = topology.generation().symbol_revision();
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
                &topology,
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
    let forward_symbols =
        symbols_for_project(&forward, root_source.document(), "selected-roots-forward");
    let reverse_symbols =
        symbols_for_project(&reverse, root_source.document(), "selected-roots-reverse");
    let forward_topology = forward
        .executable_view()
        .unwrap()
        .accept_symbol_generation(&forward_symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("forward selected roots topology");
    let reverse_topology = reverse
        .executable_view()
        .unwrap()
        .accept_symbol_generation(&reverse_symbols)
        .expect("accepted symbol generation")
        .into_evaluation_topology()
        .expect("reverse selected roots topology");

    let forward = forward
        .executable_view()
        .unwrap()
        .selected_expression_graph(&forward_topology, |_| None, |_| None)
        .expect("postfix-free forward inventory")
        .expression_owners()
        .collect::<BTreeSet<_>>();
    let reverse = reverse
        .executable_view()
        .unwrap()
        .selected_expression_graph(&reverse_topology, |_| None, |_| None)
        .expect("postfix-free reverse inventory")
        .expression_owners()
        .collect::<BTreeSet<_>>();
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
fn accepted_symbol_generation_witness_joins_non_callable_sources_exactly() {
    let package = package();
    let root_path = CanonicalModulePath::crate_root();
    let mut first_syntax = SyntaxDatabase::try_new().unwrap();
    let first = parse_initial(
        &mut first_syntax,
        "arcweft-test://proof/final-project/witness-first",
        "witness.arcw",
        "struct First {}\n",
    );
    let mut first_database = HirDatabase::try_new().unwrap();
    let first_module = lower(&mut first_database, &first, &package, &root_path);
    let first_project = build_project(
        &first_database,
        package.clone(),
        [bind(&first_database, &package, &root_path, first_module)],
    )
    .unwrap();
    let first_symbols = symbols_for_project(
        &first_project,
        first.document(),
        "accepted-symbol-generation-witness",
    );
    let first_executable = first_project.executable_view().unwrap();
    let witness = first_executable
        .accept_symbol_generation(&first_symbols)
        .unwrap();
    assert_eq!(witness.project().modules().len(), 1);
    assert_eq!(witness.symbols().modules().len(), 1);

    let mut second_syntax = SyntaxDatabase::try_new().unwrap();
    let second = parse_initial(
        &mut second_syntax,
        "arcweft-test://proof/final-project/witness-second",
        "witness.arcw",
        "struct Second {}\n",
    );
    let mut second_database = HirDatabase::try_new().unwrap();
    let second_module = lower(&mut second_database, &second, &package, &root_path);
    let second_project = build_project(
        &second_database,
        package.clone(),
        [bind(&second_database, &package, &root_path, second_module)],
    )
    .unwrap();
    let second_symbols = symbols_for_project(
        &second_project,
        second.document(),
        "accepted-symbol-generation-foreign",
    );
    assert!(matches!(
        first_executable.accept_symbol_generation(&second_symbols),
        Err(super::AcceptedHirProjectSymbolGenerationError::SourceIdentityMismatch { .. })
    ));
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
