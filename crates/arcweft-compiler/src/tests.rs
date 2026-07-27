use std::sync::Arc;

use arcweft_core::{
    engine::{Engine, FlowExit, FlowFiberStatus},
    pattern::RuntimePattern,
    plan::{
        FlowOp, RuntimeBuiltinIteratorEvidence, RuntimeIteratorEvidence,
        RuntimeIteratorWitnessExecutable,
    },
    source::{SourceHandlerPlan, SourceOp},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    value::{DenseSeq, RuntimeExpr, RuntimeSeq, RuntimeValue},
};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision, InlineFailurePolicy};
use arcweft_id::PublicId;
use arcweft_lang_hir::symbol::{
    CallableDeclarationId, CallableDeclarationOwner, CallablePackageId, ProjectSymbolWorldId,
};
use arcweft_lang_hir::{
    lower::lower_document_to_hir,
    project::{HirProject, HirProjectModule},
};
use arcweft_lang_sema::project_index::{
    EntitySymbol, ProgramHash, ProjectCallableSymbol, ProjectGraphDependencyRelation,
    ProjectGraphDependencyRelationKind, ProjectGraphRelation, ProjectGraphRelationKind,
    ProjectGraphSymbolRef, ProjectSemanticIndex, QualifiedName, SemanticHash,
    project_semantic_index_from_hir,
};
use arcweft_lang_sema::types::{EntityKind, EntityType, TypeKind};
use arcweft_lang_sema::{
    check::analyze_registered_project_types,
    env::{FunctionParam, FunctionSignature, TypeCheckEnv},
    registration::{CharacterRegistrar, CharacterRegistrationRequest, ProjectRegistrationFacts},
};
use arcweft_lang_syntax::ast::module_path::CanonicalModulePath;
use arcweft_lang_syntax::parser::{ParseOptions, parse_document_with_source};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::flow::{AdmittedRuntimePlanLowerOptions, RuntimePlanLowerOptions};
use arcweft_source::{
    SourceAnchor, SourceDocument, SourceDocumentId, SourceName, SourceRange, SourceSetRevision,
};
use arcweft_view::{AcceptedViewProgramRevision, ViewId, ViewProgramId, ViewStyleSheetId};

use crate::{
    agent_project::agent_project_graph_from_project,
    hir::validate_hir_with_env,
    lower::{
        lower_source_runtime_plan_with_stats_and_options,
        lower_source_runtime_plan_with_typecheck_stats_and_options,
    },
    source::compile_source,
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).expect("valid public id")
}

fn test_source_document(path: &str, source_len: usize) -> SourceDocument {
    SourceDocument::try_new(
        SourceDocumentId::try_new(path).expect("test document id"),
        SourceName::path(path),
        " ".repeat(source_len),
    )
    .expect("test source document")
}

fn parse_runtime_plan_fixture(
    source: impl Into<Arc<str>>,
) -> arcweft_lang_syntax::source::ParsedSource {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            source,
        )
        .expect("runtime-plan fixture source document"),
    );
    parse_document_with_source(document, ParseOptions::default())
}

fn test_source_anchor() -> SourceAnchor {
    let document = test_source_document("generated://arcweft/compiler-test", 0);
    SourceAnchor::from_span(
        document
            .span(SourceRange::new(0, 0))
            .expect("empty test span"),
    )
}

fn test_dialogue_presentation() -> DialoguePresentationProfile {
    DialoguePresentationProfile::engine_default()
}

fn test_dialogue_revision() -> DialogueProfileRevision {
    let manifest = test_source_document("generated://arcweft/compiler-dialogue-profile", 1);
    let sources =
        SourceSetRevision::try_for_identities([manifest.identity()]).expect("test source revision");
    DialogueProfileRevision::from_admitted_parts(
        manifest.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.compiler-test").expect("test View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x5a; 32])
            .expect("test View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn admitted_options() -> AdmittedRuntimePlanLowerOptions {
    RuntimePlanLowerOptions::default()
        .with_dialogue_profile(test_dialogue_presentation(), test_dialogue_revision())
}

fn runtime_apply_arg_counts(expr: &RuntimeExpr) -> Vec<usize> {
    let mut counts = Vec::new();
    collect_runtime_apply_arg_counts(expr, &mut counts);
    counts
}

fn collect_runtime_apply_arg_counts(expr: &RuntimeExpr, counts: &mut Vec<usize>) {
    if let RuntimeExpr::Apply { callee, args } = expr {
        collect_runtime_apply_arg_counts(callee, counts);
        counts.push(args.len());
    }
}

/// Returns the initializer, scoped binding name, and body of a lowered pipe.
///
/// Pipe bindings are deliberately outside the source identifier alphabet, so
/// the runtime plan can share one evaluated LHS across every RHS use without
/// colliding with an authored local.
fn runtime_pipe_let(expr: &RuntimeExpr) -> Option<(&RuntimeExpr, &str, &RuntimeExpr)> {
    let RuntimeExpr::Let { name, expr, body } = expr else {
        return None;
    };
    name.starts_with('\0')
        .then_some((expr.as_ref(), name.as_str(), body.as_ref()))
}

/// Returns the LHS initializer and RHS callable of a placeholder-free pipe.
///
/// The final `Apply` is intentionally kept separate from any applications
/// already present in the RHS, preserving `lhs |> f(a)` as `f(a)(lhs)`.
fn runtime_staged_pipe(expr: &RuntimeExpr) -> Option<(&RuntimeExpr, &RuntimeExpr)> {
    let (initializer, binding, body) = runtime_pipe_let(expr)?;
    let RuntimeExpr::Apply { callee, args } = body else {
        return None;
    };
    matches!(args.as_slice(), [RuntimeExpr::Local(name)] if name == binding)
        .then_some((initializer, callee.as_ref()))
}

/// Splits the separate receiver stage from a data-last method fallback.
fn runtime_data_last_stages(expr: &RuntimeExpr) -> Option<(&RuntimeExpr, &RuntimeExpr)> {
    let RuntimeExpr::Apply { callee, args } = expr else {
        return None;
    };
    let [receiver] = args.as_slice() else {
        return None;
    };
    Some((callee.as_ref(), receiver))
}

#[test]
fn agent_project_graph_snapshot_preserves_project_relations() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"))
        .with_entity(EntitySymbol::new(
            public_id("entry.main"),
            EntityType::new(EntityKind::Entry, None),
            test_source_anchor(),
            SemanticHash::new("shape.entry.main.v1"),
        ))
        .with_entity(EntitySymbol::new(
            public_id("flow.opening"),
            EntityType::new(EntityKind::Flow, None),
            test_source_anchor(),
            SemanticHash::new("shape.flow.opening.v1"),
        ))
        .with_relation(ProjectGraphRelation::new(
            public_id("entry.main"),
            public_id("flow.opening"),
            ProjectGraphRelationKind::EntryGoto,
        ));

    let graph = agent_project_graph_from_project(&project).expect("graph snapshot builds");
    let summary = graph
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "project_summary")
        .and_then(|symbol| symbol.project_summary)
        .expect("project summary graph symbol carries typed counts");

    assert!(graph.symbols.iter().any(|symbol| {
        symbol
            .public_id
            .as_ref()
            .is_some_and(|id| id.as_str() == "entry.main")
    }));
    assert_eq!(summary.entity_count, 2);
    assert_eq!(summary.relation_count, 1);
    assert!(graph.edges.iter().any(|edge| {
        edge.from_symbol_id == "project:entity:entry.main"
            && edge.to_symbol_id == "project:entity:flow.opening"
            && edge.edge_kind == "entry_goto"
    }));
}

#[test]
fn agent_project_graph_snapshot_preserves_project_callables() {
    let update_declaration = CallableDeclarationId::try_new(
        CallablePackageId::try_new("compiler-test").expect("package"),
        CanonicalModulePath::crate_root(),
        CallableDeclarationOwner::Function,
        "update_route",
    )
    .expect("callable declaration");
    let current_declaration = CallableDeclarationId::try_new(
        CallablePackageId::try_new("compiler-test").expect("package"),
        CanonicalModulePath::crate_root(),
        CallableDeclarationOwner::Function,
        "current_route",
    )
    .expect("callable declaration");
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"))
        .with_project_callable(ProjectCallableSymbol::function(
            update_declaration,
            FunctionSignature::new(
                TypeKind::Named("GameState".to_owned()),
                [
                    FunctionParam::required("state", TypeKind::Named("GameState".to_owned())),
                    FunctionParam::required("event", TypeKind::Named("GameEvent".to_owned())),
                ],
            ),
            test_source_anchor(),
            SemanticHash::new("hir:callable:function:update_route:(state: GameState)"),
        ))
        .with_project_callable(ProjectCallableSymbol::function(
            current_declaration,
            FunctionSignature::new(TypeKind::entity_ref(EntityKind::Flow), []),
            test_source_anchor(),
            SemanticHash::new("hir:callable:function:current_route:()"),
        ))
        .with_dependency_relation(ProjectGraphDependencyRelation::new(
            ProjectGraphSymbolRef::Callable(QualifiedName::new("update_route")),
            ProjectGraphSymbolRef::Callable(QualifiedName::new("current_route")),
            ProjectGraphDependencyRelationKind::CallsCallable,
        ));

    let graph = agent_project_graph_from_project(&project).expect("graph snapshot builds");
    let summary = graph
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "project_summary")
        .and_then(|symbol| symbol.project_summary)
        .expect("project summary graph symbol carries callable counts");

    assert!(graph.symbols.iter().any(|symbol| {
        symbol.symbol_id == "project:callable:function:update_route"
            && symbol.qualified_name.as_deref() == Some("update_route")
            && symbol.kind == "project_function"
    }));
    assert!(graph.symbols.iter().any(|symbol| {
        symbol.symbol_id == "project:callable:function:current_route"
            && symbol.qualified_name.as_deref() == Some("current_route")
            && symbol.kind == "project_function"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.from_symbol_id == "project:summary"
            && edge.to_symbol_id == "project:callable:function:update_route"
            && edge.edge_kind == "contains_callable"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.from_symbol_id == "project:callable:function:update_route"
            && edge.to_symbol_id == "project:callable:function:current_route"
            && edge.edge_kind == "calls_callable"
    }));
    assert_eq!(summary.project_callable_count, 2);
    assert_eq!(summary.dependency_edge_count, 1);
}

#[test]
fn agent_project_graph_snapshot_preserves_flow_control_summary() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/agent-project-graph.arcw")
                .expect("agent project graph fixture source ID"),
            SourceName::path("compiler/agent-project-graph.arcw"),
            r#"
pub fn current_route() -> Ref<Flow> {
return @flow.done
}

flow @flow.opening opening {
let route = current_route()
goto @flow.done
goto route
}

flow @flow.done done() -> String {
return "done"
}
"#,
        )
        .expect("agent project graph fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("source lowers to HIR");
    let project =
        project_semantic_index_from_hir(&hir, ProgramHash::new("program-test"), parsed.document())
            .expect("project indexes flow control");

    let graph = agent_project_graph_from_project(&project).expect("graph snapshot builds");
    let project_summary = graph
        .symbols
        .iter()
        .find(|symbol| symbol.kind == "project_summary")
        .and_then(|symbol| symbol.project_summary)
        .expect("project summary graph symbol carries flow-control counts");
    let flow_symbol = graph
        .symbols
        .iter()
        .find(|symbol| {
            symbol
                .public_id
                .as_ref()
                .is_some_and(|id| id.as_str() == "flow.opening")
        })
        .expect("flow symbol exists");
    let summary = flow_symbol.flow_control.expect("flow control summary");

    assert!(summary.has_dynamic_control);
    assert_eq!(summary.static_goto_count, 1);
    assert_eq!(summary.dynamic_goto_count, 1);
    assert_eq!(project_summary.dynamic_control_flow_count, 1);
}

#[test]
fn compiles_dialogue_source_to_plan_and_display_catalog() {
    let source = r"
character @character.alice Alice as alice {}

entry cli @entry.main {
    goto @flow.main
}

flow @flow.main main {
alice: Hello
}
";

    let compiled = compile_source(source).expect("source compiles");

    assert!(!compiled.plan.entries.is_empty());
    assert!(!compiled.display.lines().is_empty());
}

#[test]
fn compiles_for_loop_with_trait_resolved_iterator_evidence() {
    let compiled = compile_source(
        r"
flow @flow.main main {
    for i in 0i32..3i32 {
        let _ = i
    }
}
",
    )
    .expect("for loop source compiles");

    let FlowOp::For { evidence, .. } = &compiled.plan.flows[0].ops[0] else {
        panic!("expected first runtime op to be for loop");
    };
    assert_eq!(
        evidence,
        &RuntimeIteratorEvidence::Builtin(RuntimeBuiltinIteratorEvidence::Range)
    );
}

#[test]
fn lowers_user_defined_into_iterator_to_executable_trait_calls() {
    let source = r"
struct Counter { start: i64, end: i64 }
struct CounterIter { current: i64, end: i64 }

impl IntoIterator for Counter {
    type Item = i64
    type IntoIter = CounterIter

    fn into_iter(self) -> CounterIter {
        CounterIter { current: self.start, end: self.end }
    }
}

impl Iterator for CounterIter {
    type Item = i64

    fn next(&mut self) -> Option<i64> {
        None
    }
}

flow @flow.main main {
    let source = Counter { start: 0i64, end: 3i64 }
    for value in source {
        let copy = value
    }
}
";
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("memory:///compiler-iterator-witness.arcw")
                .expect("source ID"),
            SourceName::path("memory:///compiler-iterator-witness.arcw"),
            source,
        )
        .expect("source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    assert!(parsed.errors().is_empty(), "{:?}", parsed.errors());
    let hir = lower_document_to_hir(&document, parsed.typed_tree()).expect("fixture lowers");
    let package = CallablePackageId::try_new("compiler-iterator-witness").expect("package ID");
    let project = HirProject::new(
        package.as_str(),
        [HirProjectModule::try_new(
            CanonicalModulePath::crate_root(),
            document.identity().clone(),
            hir,
        )
        .expect("root module")],
    )
    .expect("HIR project");
    let world = ProjectSymbolWorldId::try_new(
        package,
        document.identity().id().clone(),
        "compiler-iterator-witness",
    )
    .expect("symbol world");
    let registration = ProjectRegistrationFacts::try_new(
        world,
        vec![Arc::clone(&document)],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .expect("registration facts");
    let registered = CharacterRegistrar::register(CharacterRegistrationRequest::new(
        Arc::new(TypeCheckEnv::standard()),
        &project,
        &registration,
        None,
    ))
    .expect("registered semantic world");
    let hir = project.linked_module();
    let typecheck = analyze_registered_project_types(&hir, &registered);
    assert!(
        typecheck.diagnostics.is_empty(),
        "{:?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers with executable witness trait calls");

    assert_eq!(report.plan.trait_methods.len(), 2);
    let evidence = report
        .plan
        .flows
        .first()
        .and_then(|flow| {
            flow.ops.iter().find_map(|op| match op {
                FlowOp::For { evidence, .. } => Some(evidence),
                _ => None,
            })
        })
        .expect("for loop carries iterator evidence");

    let RuntimeIteratorEvidence::Witness(witness) = evidence else {
        panic!("user-defined source must use witness iterator evidence");
    };
    let RuntimeIteratorWitnessExecutable::TraitCalls(calls) = witness.executable else {
        panic!("user-defined witness must lower to executable trait calls");
    };
    assert_eq!(calls.into_iter.0, 0);
    assert_eq!(calls.next.0, 1);
}

#[test]
fn runtime_plan_uses_typecheck_evidence_for_function_value_calls() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
flow @flow.main main() -> String {
    let ok: bool = f(1i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(
        &hir,
        &TypeCheckEnv::standard()
            .with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool)),
    );
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers with function-value evidence");
    let FlowOp::Let { expr, .. } = &report.plan.flows[0].ops[0] else {
        panic!("expected first op to bind the function value call");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                && args.len() == 1
    ));

    let plain_report = lower_source_runtime_plan_with_stats_and_options(&hir, &admitted_options())
        .expect("runtime plan lowers without typecheck evidence");
    let FlowOp::Let { expr, .. } = &plain_report.plan.flows[0].ops[0] else {
        panic!("expected first plain op to bind the call");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "f" && args.len() == 1
    ));
}

#[test]
fn checked_runtime_plan_reports_missing_typed_lowering_evidence() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
flow @flow.main main {
    let ok: bool = f(1i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(
        &hir,
        &TypeCheckEnv::standard()
            .with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool)),
    );
    assert!(
        !typecheck.typed_lowering_evidence.is_empty(),
        "fixture must produce typed function-call evidence"
    );

    let errors = lower_source_runtime_plan_with_stats_and_options(
        &hir,
        &admitted_options()
            .with_required_typed_lowering_evidence_len(typecheck.typed_lowering_evidence.len()),
    )
    .expect_err("checked runtime lowering must reject missing typed evidence");

    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("checked runtime lowering expected")
            && error.message().contains("typed lowering evidence")
    }));
}

#[test]
fn runtime_plan_uses_expected_function_evidence_for_placeholder_args() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
flow @flow.main main() -> String {
    let accepted: bool = accept(_ > 80i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let predicate = TypeKind::function([TypeKind::I64], TypeKind::Bool);
    let typecheck = arcweft_lang_sema::check::analyze_types(
        &hir,
        &TypeCheckEnv::standard().with_function_signature(
            "accept",
            FunctionSignature::new(
                TypeKind::Bool,
                [FunctionParam::required("predicate", predicate)],
            ),
        ),
    );
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers with expected-function evidence");
    let FlowOp::Let { expr, .. } = &report.plan.flows[0].ops[0] else {
        panic!("expected first op to bind the call");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Call { callee, args }
            if callee.as_label() == "accept"
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Function { params, body }]
                        if params.as_slice() == ["__arcweft_partial"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Binary { lhs, .. }
                                    if matches!(
                                        lhs.as_ref(),
                                        RuntimeExpr::Local(name)
                                            if name == "__arcweft_partial"
                                    )
                            )
                )
    ));
}

#[test]
fn runtime_plan_report_carries_closure_capture_metadata() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
flow @flow.main main() -> String {
    let limit: i64 = 80i64
    let is_high = |score: i64| -> bool {
        score >= limit
    }
    let ok: bool = is_high(81i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());
    assert!(
        typecheck.closure_captures.iter().any(|inventory| inventory
            .captures
            .iter()
            .any(|capture| capture.name == "limit")),
        "fixture must produce sema closure capture metadata"
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers with capture metadata");
    let capture_inventory = report
        .closure_captures
        .iter()
        .find(|inventory| {
            inventory
                .captures
                .iter()
                .any(|capture| capture.name == "limit")
        })
        .expect("runtime-plan report carries closure capture inventory");
    assert!(
        capture_inventory
            .captures
            .iter()
            .any(|capture| capture.name == "limit" && capture.type_label == "i64"),
        "expected `limit: i64` capture metadata, got {:?}",
        capture_inventory.captures
    );
}

#[test]
fn runtime_plan_lowers_inferred_partial_placeholder_functions() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main() -> String {
    let high = _ > 80i64
    let high_grouped = (_ > 80i64)
    let add_one = add(_, 1i64)
    let double = add(_, _)
    let add_to_one = add(right = _, left = 1i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers inferred partial functions");
    let [
        FlowOp::Let { expr: high, .. },
        FlowOp::Let {
            expr: high_grouped, ..
        },
        FlowOp::Let { expr: add_one, .. },
        FlowOp::Let { expr: double, .. },
        FlowOp::Let {
            expr: add_to_one, ..
        },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected inferred function lets");
    };
    assert!(matches!(
        high,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(body.as_ref(), RuntimeExpr::Binary { .. })
    ));
    assert!(matches!(
        high_grouped,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(body.as_ref(), RuntimeExpr::Binary { .. })
    ));
    assert!(matches!(
        add_one,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(body.as_ref(), RuntimeExpr::PureCall { .. })
    ));
    assert!(matches!(
        double,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::PureCall { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(left), RuntimeExpr::Local(right)]
                                if left == "__arcweft_partial"
                                    && right == "__arcweft_partial"
                        )
                )
    ));
    assert!(matches!(
        add_to_one,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::PureCall { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Value(left), RuntimeExpr::Local(right)]
                                if left == &RuntimeValue::i64(1)
                                    && right == "__arcweft_partial"
                        )
                )
    ));
}

#[test]
fn runtime_plan_lowers_named_missing_inferred_helper_input() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main() -> i64 {
    let named_missing = add(right = 1i64)
    return named_missing(2i64)
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers named missing input");
    let FlowOp::Let { expr, .. } = &report.plan.flows[0].ops[0] else {
        panic!("expected named missing input binding");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["left"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::PureCall { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(left), RuntimeExpr::Value(right)]
                                if left == "left" && right == &RuntimeValue::i64(1)
                        )
                )
    ));
}

#[test]
fn runtime_plan_lowers_typed_data_last_method_fallback() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow @flow.main main() -> String {
    let score = 90i64
    let ok = score.above(80i64)
    let named = score.above(min = 80i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers data-last method fallback");
    let [
        FlowOp::Let { .. },
        FlowOp::Let { expr: ok, .. },
        FlowOp::Let { expr: named, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected score, ok, and named lets");
    };
    let (positional_stage, positional_receiver) =
        runtime_data_last_stages(ok).expect("positional fallback keeps a receiver stage");
    assert!(matches!(
        positional_receiver,
        RuntimeExpr::Local(name) if name == "score"
    ));
    assert!(matches!(
        positional_stage,
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Function { params, .. }
                    if params.as_slice() == ["min", "value"]
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(80)
            )
    ));

    let (named_stage, named_receiver) =
        runtime_data_last_stages(named).expect("named fallback keeps a receiver stage");
    assert!(matches!(
        named_receiver,
        RuntimeExpr::Local(name) if name == "score"
    ));
    assert!(matches!(
        named_stage,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["value"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::PureCall { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Value(min), RuntimeExpr::Local(value)]
                                if min == &RuntimeValue::i64(80) && value == "value"
                        )
                )
    ));
}

#[test]
fn runtime_plan_keeps_curried_source_and_local_method_fallback_staged() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn above(min: i64)(value: i64) -> bool {
    return value > min
}

fn trim(prefix: String)(value: String) -> String {
    return value
}

flow @flow.main main() -> String {
    let compare = above
    let score = 90i64
    let source = score.above(80i64)
    let local = score.compare(80i64)
    let text = " padded "
    let inherent = text.trim()
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers curried source and local method fallback");
    let [
        FlowOp::Let { .. },
        FlowOp::Let { .. },
        FlowOp::Let { expr: source, .. },
        FlowOp::Let { expr: local, .. },
        FlowOp::Let { .. },
        FlowOp::Let { expr: inherent, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected compare, score, source, local, text, and inherent lets");
    };

    let (source_stage, source_receiver) =
        runtime_data_last_stages(source).expect("source fallback keeps its receiver stage");
    assert!(matches!(source_receiver, RuntimeExpr::Local(name) if name == "score"));
    assert_eq!(
        runtime_apply_arg_counts(source_stage),
        [1],
        "source method arguments must complete the first group before the receiver: {source:#?}"
    );

    let (local_stage, local_receiver) =
        runtime_data_last_stages(local).expect("local fallback keeps its receiver stage");
    assert!(matches!(local_receiver, RuntimeExpr::Local(name) if name == "score"));
    assert!(matches!(
        local_stage,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "compare")
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(80)
                )
    ));

    assert!(
        matches!(
            inherent,
            RuntimeExpr::MethodCall {
                receiver,
                method,
                args,
            } if matches!(receiver.as_ref(), RuntimeExpr::Local(name) if name == "text")
                && method == "trim"
                && args.is_empty()
        ),
        "inherent method must win over data-last fallback: {inherent:#?}"
    );
}

#[test]
fn runtime_plan_lowers_fixed_literal_spread_data_last_method_fallback() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn between(min: i64, max: i64, value: i64) -> bool {
    return value > min
}

flow @flow.main main() -> String {
    let score = 75i64
    let direct = score.between([60i64, 90i64]...)
    let mixed = score.between([60i64]..., max = 90i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers fixed spread data-last method fallback");
    let [
        FlowOp::Let { .. },
        FlowOp::Let { expr: direct, .. },
        FlowOp::Let { expr: mixed, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected score, direct, and mixed lets");
    };
    assert_direct_spread_data_last_stage(direct);
    assert_mixed_spread_data_last_stage(mixed);
}

fn assert_direct_spread_data_last_stage(expr: &RuntimeExpr) {
    let (stage, receiver) =
        runtime_data_last_stages(expr).expect("spread fallback keeps a receiver stage");
    assert!(matches!(receiver, RuntimeExpr::Local(score) if score == "score"));
    let RuntimeExpr::Apply { callee, args } = stage else {
        panic!("expected direct spread arguments in the first stage, got {stage:#?}");
    };
    assert!(matches!(
        callee.as_ref(),
        RuntimeExpr::Function { params, .. }
            if params.as_slice() == ["min", "max", "value"]
    ));
    assert!(
        matches!(
            args.as_slice(),
            [RuntimeExpr::SpreadArg(value)]
                if matches!(
                    value.as_ref(),
                    RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::Dense(
                        DenseSeq::I64(values)
                    ))) if matches!(values.as_slice(), [60, 90])
                )
        ),
        "expected direct fallback to preserve one spread arg in its first stage, got {expr:#?}"
    );
}

fn assert_mixed_spread_data_last_stage(expr: &RuntimeExpr) {
    let (stage, receiver) =
        runtime_data_last_stages(expr).expect("mixed fallback keeps a receiver stage");
    assert!(matches!(receiver, RuntimeExpr::Local(score) if score == "score"));
    let RuntimeExpr::Function { params, body } = stage else {
        panic!("expected mixed first stage to leave only its receiver parameter, got {stage:#?}");
    };
    assert_eq!(params, &["value"]);
    let RuntimeExpr::PureCall { args, .. } = body.as_ref() else {
        panic!("expected mixed first stage to call its helper, got {body:#?}");
    };
    assert!(
        matches!(
            args.as_slice(),
            [
                RuntimeExpr::SpreadArg(value),
                RuntimeExpr::Value(max),
                RuntimeExpr::Local(value_param),
            ] if matches!(
                value.as_ref(),
                RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::Dense(
                    DenseSeq::I64(values)
                ))) if matches!(values.as_slice(), [60])
            ) && max == &RuntimeValue::i64(90) && value_param == "value"
        ),
        "expected mixed fallback to preserve spread and named args before its receiver stage, got {expr:#?}"
    );
}

#[test]
fn runtime_plan_lowers_data_last_pipe_call_with_typecheck() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main() -> String {
    let partial = 2i64 |> add
    let positional = 2i64 |> add(1i64)
    let named = 2i64 |> add(lhs = 1i64)
    let named_rhs = 2i64 |> add(rhs = 1i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers data-last pipe calls");
    let [
        FlowOp::Let { expr: partial, .. },
        FlowOp::Let {
            expr: positional, ..
        },
        FlowOp::Let { expr: named, .. },
        FlowOp::Let {
            expr: named_rhs, ..
        },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected partial, positional, and named pipe lets");
    };
    let (partial_lhs, partial_callee) =
        runtime_staged_pipe(partial).expect("bare callable pipe keeps a final apply stage");
    assert!(matches!(
        partial_lhs,
        RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
    ));
    assert!(matches!(
        partial_callee,
        RuntimeExpr::Function { params, .. } if params.as_slice() == ["lhs", "rhs"]
    ));

    for (label, expr) in [
        ("positional", positional),
        ("named-left", named),
        ("named-right", named_rhs),
    ] {
        let (lhs, rhs_callable) = runtime_staged_pipe(expr)
            .unwrap_or_else(|| panic!("{label} pipe must keep rhs(lhs) as a separate stage"));
        assert!(
            matches!(lhs, RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)),
            "unexpected {label} pipe initializer: {lhs:#?}"
        );
        assert!(
            matches!(
                rhs_callable,
                RuntimeExpr::Function { params, .. } if params.len() == 1
            ) || matches!(
                rhs_callable,
                RuntimeExpr::Apply { args, .. } if args.len() == 1
            ),
            "{label} RHS must be the one-argument callable produced by add(1), got {rhs_callable:#?}"
        );
    }
}

#[test]
fn runtime_plan_binds_pipe_left_once_inside_if_let_expression() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
flow @flow.main main() -> i64 {
    let maybe = Some(7i64)
    let selected: i64 = maybe |> if let .Some(value) = ^ when value > 1i64 {
        value
    } else {
        1i64
    }
    return selected
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers pipe-left placeholder inside if-let expression");
    let [
        FlowOp::Let { expr: maybe, .. },
        FlowOp::Let { expr: selected, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected maybe and selected lets");
    };
    assert!(matches!(maybe, RuntimeExpr::Variant { name, .. } if name == "Some"));
    let (initializer, binding, body) =
        runtime_pipe_let(selected).expect("pipe-left lowers through one lexical binding");
    assert!(matches!(initializer, RuntimeExpr::Local(name) if name == "maybe"));
    assert!(matches!(
        body,
        RuntimeExpr::IfLet {
            pattern:
                RuntimePattern::Variant {
                    name,
                    payload: Some(payload),
                    ..
                },
            expr,
            guard: Some(_),
            then_expr,
            else_expr,
        } if name == "Some"
            && matches!(
                payload.as_ref(),
                RuntimePattern::Tuple(items)
                    if matches!(items.as_slice(), [RuntimePattern::Ident(value)] if value == "value")
            )
            && matches!(expr.as_ref(), RuntimeExpr::Local(name) if name == binding)
            && matches!(then_expr.as_ref(), RuntimeExpr::Local(name) if name == "value")
            && matches!(else_expr.as_ref(), RuntimeExpr::Value(value) if value == &RuntimeValue::i64(1))
    ));
}

#[test]
fn runtime_plan_binds_pipe_left_once_inside_match_expression() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
flow @flow.main main() -> i64 {
    let ready = true
    let selected: i64 = ready |> match ^ {
        true => 7i64
        false => 1i64
    }
    return selected
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers pipe-left placeholder inside match expression");
    let [
        FlowOp::Let { expr: ready, .. },
        FlowOp::Let { expr: selected, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected ready and selected lets");
    };
    assert!(matches!(
        ready,
        RuntimeExpr::Value(RuntimeValue::Bool(true))
    ));
    let (initializer, binding, body) =
        runtime_pipe_let(selected).expect("pipe-left lowers through one lexical binding");
    assert!(matches!(initializer, RuntimeExpr::Local(name) if name == "ready"));
    assert!(matches!(
        body,
        RuntimeExpr::Match { scrutinee, arms }
            if matches!(scrutinee.as_ref(), RuntimeExpr::Local(name) if name == binding)
                && matches!(
                    arms.as_slice(),
                    [
                        arcweft_core::value::RuntimeExprMatchArm {
                            pattern: RuntimePattern::Literal(RuntimeValue::Bool(true)),
                            guard: None,
                            value: RuntimeExpr::Value(first),
                        },
                        arcweft_core::value::RuntimeExprMatchArm {
                            pattern: RuntimePattern::Literal(RuntimeValue::Bool(false)),
                            guard: None,
                            value: RuntimeExpr::Value(second),
                        },
                    ] if first == &RuntimeValue::i64(7) && second == &RuntimeValue::i64(1)
                )
    ));
}

#[test]
fn runtime_plan_lowers_non_annotated_function_prefix_partial_with_typecheck() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main() -> String {
    let add_two = add(2i64)
    let seven: i64 = add_two(5i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers non-annotated function prefix partial");
    let [
        FlowOp::Let { expr: add_two, .. },
        FlowOp::Let { expr: seven, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected partial and second apply lets");
    };
    assert!(matches!(
        add_two,
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Function { params, .. } if params.as_slice() == ["lhs", "rhs"]
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(2)
            )
    ));
    assert!(matches!(
        seven,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "add_two")
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(5)
                )
    ));
}

#[test]
fn runtime_plan_lowers_source_function_named_data_last_pipe_to_apply() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn choose(left: String, right: String) -> (String, String) {
    return (left, right)
}

flow @flow.main main() -> String {
    let via_right: (String, String) = "pipe-left" |> choose(right = "named-right")
    let via_left: (String, String) = "pipe-right" |> choose(left = "named-left")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers source function named data-last pipe");
    let [
        FlowOp::Let {
            expr: via_right, ..
        },
        FlowOp::Let { expr: via_left, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected named data-last pipe lets");
    };
    let (right_initializer, right_callable) =
        runtime_staged_pipe(via_right).expect("named-right pipe keeps a final receiver stage");
    assert!(matches!(
        right_initializer,
        RuntimeExpr::Value(RuntimeValue::String(value)) if value == "pipe-left"
    ));
    assert!(matches!(
        right_callable,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["left"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(left), RuntimeExpr::Value(RuntimeValue::String(right))]
                                if left == "left" && right == "named-right"
                        )
                )
    ));

    let (left_initializer, left_callable) =
        runtime_staged_pipe(via_left).expect("named-left pipe keeps a final receiver stage");
    assert!(matches!(
        left_initializer,
        RuntimeExpr::Value(RuntimeValue::String(value)) if value == "pipe-right"
    ));
    assert!(matches!(
        left_callable,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["right"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Value(RuntimeValue::String(left)), RuntimeExpr::Local(right)]
                                if left == "named-left" && right == "right"
                        )
                )
    ));
}

#[test]
fn runtime_plan_lowers_destructured_closure_parameter_application() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
flow @flow.main main() -> String {
    let choose = |(left, right): (String, String)| right
    let value: String = choose(("head", "tail"))
    return value
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers destructured closure parameter");
    let [
        FlowOp::Let { expr: choose, .. },
        FlowOp::Let { expr: value, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected choose and value lets");
    };
    assert!(matches!(
        choose,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["$arcweft.closure.arg.0"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Match { scrutinee, arms }
                        if matches!(
                            scrutinee.as_ref(),
                            RuntimeExpr::Local(name) if name == "$arcweft.closure.arg.0"
                        ) && arms.len() == 1
                )
    ));
    assert!(matches!(
        value,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "choose")
                && matches!(args.as_slice(), [RuntimeExpr::Tuple(items)] if items.len() == 2)
    ));
}

#[test]
fn checked_runtime_plan_materializes_named_missing_source_function_partial_call() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn choose(left: String, right: String) -> String {
    return right
}

flow @flow.main main() -> String {
    let choose_right = choose(right = "tail")
    let value: String = choose_right("head")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes non-helper source function partial");

    let [
        FlowOp::Let {
            expr: choose_right, ..
        },
        FlowOp::Let { expr: value, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected choose_right and value lets");
    };
    assert!(matches!(
        choose_right,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["left"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { callee, args }
                        if matches!(
                            callee.as_ref(),
                            RuntimeExpr::Function { params, body }
                                if params.as_slice() == ["left", "right"]
                                    && matches!(body.as_ref(), RuntimeExpr::Local(name) if name == "right")
                        ) && matches!(
                            args.as_slice(),
                            [RuntimeExpr::Local(name), RuntimeExpr::Value(value)]
                                if name == "left" && value == &RuntimeValue::String("tail".to_owned())
                        )
                )
    ));
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "choose_right")
                    && matches!(
                        args.as_slice(),
                        [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("head".to_owned())
                    )
        ),
        "expected value let to call choose_right, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_lowers_signature_fixed_literal_spread_apply() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main() -> String {
    let add_one = add([1i64]...)
    let exact: i64 = add([1i64]..., 2i64)
    let value: i64 = add_one(2i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan lowers fixed literal spread signature calls");

    let [
        FlowOp::Let { expr: add_one, .. },
        FlowOp::Let { expr: exact, .. },
        FlowOp::Let { expr: value, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected add_one, exact, and value lets");
    };
    assert!(
        matches!(
            add_one,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, .. } if params.as_slice() == ["left", "right"]
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::SpreadArg(value)]
                        if matches!(
                            value.as_ref(),
                            RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::Dense(
                                DenseSeq::I64(values)
                            ))) if matches!(values.as_slice(), [1])
                        )
                )
        ),
        "expected add_one to partially apply add with fixed literal spread, got {add_one:#?}"
    );
    assert!(
        matches!(
            exact,
            RuntimeExpr::PureCall { args, .. }
                if matches!(
                    args.as_slice(),
                    [RuntimeExpr::SpreadArg(value), RuntimeExpr::Value(right)]
                        if matches!(
                            value.as_ref(),
                            RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::Dense(
                                DenseSeq::I64(values)
                            ))) if matches!(values.as_slice(), [1])
                        ) && right == &RuntimeValue::i64(2)
                )
        ),
        "expected exact to apply add with fixed literal spread and positional arg, got {exact:#?}"
    );
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "add_one")
                    && matches!(
                        args.as_slice(),
                        [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(2)
                    )
        ),
        "expected value let to call add_one, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_curried_source_function_value() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn pair(left: String)(right: String) -> (String, String) {
    return (left, right)
}

flow @flow.main main() -> String {
    let with_left = pair("left")
    let tupled: (String, String) = with_left("right")
    let direct: (String, String) = pair("x")("y")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes curried source function");

    let [
        FlowOp::Let {
            expr: with_left, ..
        },
        FlowOp::Let { expr: tupled, .. },
        FlowOp::Let { expr: direct, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected with_left, tupled, and direct lets");
    };
    assert!(
        matches!(
            with_left,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["left"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Function { params, body }
                                    if params.as_slice() == ["right"]
                                        && matches!(body.as_ref(), RuntimeExpr::Tuple(items) if items.len() == 2)
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("left".to_owned())
                )
        ),
        "expected with_left to apply materialized curried function, got {with_left:#?}"
    );
    assert!(matches!(
        tupled,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "with_left")
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("right".to_owned())
                )
    ));
    assert!(matches!(
        direct,
        RuntimeExpr::Apply { callee, args }
            if matches!(
                callee.as_ref(),
                RuntimeExpr::Apply { callee, args }
                    if matches!(
                        callee.as_ref(),
                        RuntimeExpr::Function { params, .. } if params.as_slice() == ["left"]
                    ) && matches!(
                        args.as_slice(),
                        [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("x".to_owned())
                    )
            ) && matches!(
                args.as_slice(),
                [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("y".to_owned())
            )
    ));
}

#[test]
fn checked_runtime_plan_lowers_function_value_fixed_literal_spread_apply() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.main main() -> String {
    let add_one = add(1i64)
    let ok: i64 = add_one([2i64]...)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan lowers fixed literal spread function-value apply");

    let [
        FlowOp::Let { expr: add_one, .. },
        FlowOp::Let { expr: ok, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected add_one and ok lets");
    };
    assert!(
        matches!(
            add_one,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, .. } if params.as_slice() == ["a", "b"]
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(1)
                )
        ),
        "expected add_one to apply materialized curried function, got {add_one:#?}"
    );
    assert!(
        matches!(
            ok,
            RuntimeExpr::Apply { callee, args }
                if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "add_one")
                    && matches!(
                        args.as_slice(),
                        [RuntimeExpr::SpreadArg(value)]
                            if matches!(
                                value.as_ref(),
                                RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::Dense(
                                    DenseSeq::I64(values)
                                )))
                                    if matches!(
                                        values.as_slice(),
                                        [2]
                                    )
                            )
                    )
        ),
        "expected ok to apply add_one with fixed literal spread arg, got {ok:#?}"
    );
}

#[test]
fn function_value_numeric_spread_keeps_following_typed_evidence_aligned() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn sum(a: i64, b: i64) -> i64 {
    return a + b
}

flow main() -> String {
    let callback: (i64, i64) -> i64 = sum
    let total: i64 = callback([1i64, 2i64]...)
    let wide: u128 = 340282366920938463463374607431768211455
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "{:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("spread container and following numeric evidence stay aligned");
    let FlowOp::Let { expr: wide, .. } = &report.plan.flows[0].ops[2] else {
        panic!("expected wide let after function-value spread");
    };
    assert_eq!(wide, &RuntimeExpr::Value(RuntimeValue::u128(u128::MAX)));
}

#[test]
fn checked_runtime_plan_materializes_source_function_returned_closure() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn pairer(left: String) -> String -> (String, String) {
    return |right: String| (left, right)
}

flow @flow.main main() -> String {
    let with_left = pairer("left")
    let tupled: (String, String) = with_left("right")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function returned closure");

    let [
        FlowOp::Let {
            expr: with_left, ..
        },
        FlowOp::Let { expr: tupled, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected with_left and tupled lets");
    };
    assert!(
        matches!(
            with_left,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["left"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Function { params, body }
                                    if params.as_slice() == ["right"]
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Tuple(items)
                                                if matches!(
                                                    items.as_slice(),
                                                    [RuntimeExpr::Local(left), RuntimeExpr::Local(right)]
                                                        if left == "left" && right == "right"
                                                )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("left".to_owned())
                )
        ),
        "expected with_left to apply returned closure source function, got {with_left:#?}"
    );
    assert!(matches!(
        tupled,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "with_left")
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::String("right".to_owned())
                )
    ));
}

#[test]
fn checked_runtime_plan_materializes_source_function_destructured_closure_let() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn choose_right(pair: (String, String)) -> String {
    let choose = |(left, right): (String, String)| right
    return choose(pair)
}

flow @flow.main main() -> String {
    let value: String = choose_right(("head", "tail"))
    return value
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function destructured closure let");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["pair"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "choose"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::Function { params, body }
                                                if params.as_slice() == ["$arcweft.closure.arg.0"]
                                                    && matches!(
                                                        body.as_ref(),
                                                        RuntimeExpr::Match { scrutinee, arms }
                                                            if matches!(
                                                                scrutinee.as_ref(),
                                                                RuntimeExpr::Local(name)
                                                                    if name == "$arcweft.closure.arg.0"
                                                            ) && matches!(
                                                                arms.as_slice(),
                                                                [arcweft_core::value::RuntimeExprMatchArm {
                                                                    pattern: RuntimePattern::Tuple(items),
                                                                    guard: None,
                                                                    value,
                                                                }] if matches!(
                                                                    items.as_slice(),
                                                                    [
                                                                        RuntimePattern::Ident(left),
                                                                        RuntimePattern::Ident(right),
                                                                    ] if left == "left" && right == "right"
                                                                ) && matches!(
                                                                    value,
                                                                    RuntimeExpr::Local(name) if name == "right"
                                                                )
                                                            )
                                                    )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Apply { callee, args }
                                                if matches!(
                                                    callee.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "choose"
                                                ) && matches!(
                                                    args.as_slice(),
                                                    [RuntimeExpr::Local(name)] if name == "pair"
                                                )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Tuple(items)] if items.len() == 2
                )
        ),
        "expected value to apply source function containing destructured closure let, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_pure_helper_call_body() {
    let parsed = parse_runtime_plan_fixture(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    left + right
}

fn finish_with_tail(value: i64, id: i64 -> i64) -> i64 {
    let finish = |item: i64| add(right = 5i64, left = item)
    return finish(value)
}

flow @flow.main main() -> i64 {
    let id = |item: i64| item
    let value: i64 = finish_with_tail(7i64, id)
    return value
}
",
    );
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing pure helper call");

    assert_eq!(report.plan.pure_helpers.len(), 1);
    assert_eq!(report.plan.pure_helpers[0].name, "add");

    let [
        FlowOp::Let { expr: id, .. },
        FlowOp::Let { expr: value, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected id and value lets");
    };
    assert!(matches!(
        id,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["item"]
                && matches!(body.as_ref(), RuntimeExpr::Local(name) if name == "item")
    ));
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["value", "id"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "finish"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::Function { params, body }
                                                if params.as_slice() == ["item"]
                                                    && matches!(
                                                        body.as_ref(),
                                                        RuntimeExpr::PureCall { args, .. }
                                                            if matches!(
                                                                args.as_slice(),
                                                                [
                                                                    RuntimeExpr::Local(left),
                                                                    RuntimeExpr::Value(right),
                                                                ] if left == "item" && right == &RuntimeValue::i64(5)
                                                            )
                                                    )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Apply { callee, args }
                                                if matches!(
                                                    callee.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "finish"
                                                ) && matches!(
                                                    args.as_slice(),
                                                    [RuntimeExpr::Local(name)] if name == "value"
                                                )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value), RuntimeExpr::Local(id)]
                        if value == &RuntimeValue::i64(7) && id == "id"
                )
        ),
        "expected value to apply source function containing exact pure-helper call, got {value:#?}"
    );
    assert_source_fn_pure_helper_alias_body_lowers();
}

fn assert_source_fn_pure_helper_alias_body_lowers() {
    let parsed = parse_runtime_plan_fixture(
        r#"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    left + right
}

fn finish_with_alias(label: String, value: i64) -> (String, i64) {
    let op = add
    let add_label = op(value)
    return (label, add_label(5i64))
}

flow @flow.main main() -> (String, i64) {
    let value: (String, i64) = finish_with_alias("score", 7i64)
    return value
}
"#,
    );
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing pure-helper alias");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["label", "value"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "op"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::Function { params, body }
                                                if params.as_slice() == ["left", "right"]
                                                    && matches!(
                                                        body.as_ref(),
                                                        RuntimeExpr::Binary { .. }
                                                    )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Let { name, expr, body }
                                                if name == "add_label"
                                                    && matches!(
                                                        expr.as_ref(),
                                                        RuntimeExpr::Apply { callee, args }
                                                            if matches!(
                                                                callee.as_ref(),
                                                                RuntimeExpr::Local(name) if name == "op"
                                                            ) && matches!(
                                                                args.as_slice(),
                                                                [RuntimeExpr::Local(left)]
                                                                    if left == "value"
                                                            )
                                                    )
                                                    && matches!(
                                                        body.as_ref(),
                                                        RuntimeExpr::Tuple(items)
                                                            if matches!(
                                                                items.as_slice(),
                                                                [
                                                                    RuntimeExpr::Local(label),
                                                                    RuntimeExpr::Apply { callee, args },
                                                                ] if label == "label"
                                                                    && matches!(
                                                                        callee.as_ref(),
                                                                        RuntimeExpr::Local(name) if name == "add_label"
                                                                    )
                                                                    && matches!(
                                                                        args.as_slice(),
                                                                        [RuntimeExpr::Value(right)]
                                                                            if right == &RuntimeValue::i64(5)
                                                                    )
                                                            )
                                                    )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(label), RuntimeExpr::Value(value)]
                        if label == &RuntimeValue::String("score".to_owned())
                            && value == &RuntimeValue::i64(7)
                )
        ),
        "expected value to apply source function containing pure-helper alias, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_exact_source_call_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    return pair(right = tail, left = "head")
}

flow @flow.main main() -> String {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing exact source call");

    assert!(
        report.plan.pure_helpers.is_empty(),
        "String-valued source functions should not be materialized through pure helpers"
    );

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["tail"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Apply { callee, args }
                                    if matches!(
                                        callee.as_ref(),
                                        RuntimeExpr::Function { params, body }
                                            if params.as_slice() == ["left", "right"]
                                                && matches!(
                                                    body.as_ref(),
                                                    RuntimeExpr::Tuple(items)
                                                        if matches!(
                                                            items.as_slice(),
                                                            [
                                                                RuntimeExpr::Local(left),
                                                                RuntimeExpr::Local(right),
                                                            ] if left == "left" && right == "right"
                                                        )
                                                )
                                    ) && matches!(
                                        args.as_slice(),
                                        [RuntimeExpr::Value(left), RuntimeExpr::Local(right)]
                                            if left == &RuntimeValue::String("head".to_owned())
                                                && right == "tail"
                                    )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)]
                        if value == &RuntimeValue::String("tail".to_owned())
                )
        ),
        "expected value to apply source function containing exact source function call, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_exact_source_alias_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    let make_pair = pair
    return make_pair("head", tail)
}

flow @flow.main main() -> String {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing source-function alias");

    assert!(
        report.plan.pure_helpers.is_empty(),
        "String-valued source functions should not be materialized through pure helpers"
    );

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["tail"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "make_pair"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::Function { params, body }
                                                if params.as_slice() == ["left", "right"]
                                                    && matches!(
                                                        body.as_ref(),
                                                        RuntimeExpr::Tuple(items)
                                                            if items.len() == 2
                                                    )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Apply { callee, args }
                                                if matches!(
                                                    callee.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "make_pair"
                                                ) && matches!(
                                                    args.as_slice(),
                                                    [
                                                        RuntimeExpr::Value(left),
                                                        RuntimeExpr::Local(right),
                                                    ] if left == &RuntimeValue::String("head".to_owned())
                                                        && right == "tail"
                                                )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)]
                        if value == &RuntimeValue::String("tail".to_owned())
                )
        ),
        "expected value to apply source function containing source-function alias, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_pure_helper_pipe_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    left + right
}

fn finish_with_pipe(label: String, value: i64) -> (String, i64, i64) {
    let add_label = value |> add
    let exact = value |> add(^, 5i64)
    return (label, add_label(5i64), exact)
}

flow @flow.main main() -> (String, i64, i64) {
    let value: (String, i64, i64) = finish_with_pipe("score", 7i64)
    return value
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing pure-helper pipes");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["label", "value"]
                            && runtime_pipe_body_has_partial_and_exact_helper_apply(body)
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(label), RuntimeExpr::Value(value)]
                        if label == &RuntimeValue::String("score".to_owned())
                            && value == &RuntimeValue::i64(7)
                )
        ),
        "expected source function body to preserve pure-helper pipe applies, got {value:#?}"
    );
}

fn runtime_pipe_body_has_partial_and_exact_helper_apply(expr: &RuntimeExpr) -> bool {
    let RuntimeExpr::Let { name, expr, body } = expr else {
        return false;
    };
    if name != "add_label" {
        return false;
    }
    let Some((partial_initializer, partial_callable)) = runtime_staged_pipe(expr) else {
        return false;
    };
    if !matches!(partial_initializer, RuntimeExpr::Local(name) if name == "value")
        || !matches!(
            partial_callable,
            RuntimeExpr::Function { params, .. } if params.as_slice() == ["left", "right"]
        )
    {
        return false;
    }

    let RuntimeExpr::Let {
        name: exact_name,
        expr: exact_expr,
        body,
    } = body.as_ref()
    else {
        return false;
    };
    if exact_name != "exact" {
        return false;
    }
    let Some((exact_initializer, exact_binding, exact_body)) = runtime_pipe_let(exact_expr) else {
        return false;
    };
    matches!(exact_initializer, RuntimeExpr::Local(name) if name == "value")
        && matches!(
            exact_body,
            RuntimeExpr::PureCall { args, .. }
                if matches!(
                    args.as_slice(),
                    [RuntimeExpr::Local(left), RuntimeExpr::Value(right)]
                        if left == exact_binding && right == &RuntimeValue::i64(5)
                )
        )
        && matches!(
            body.as_ref(),
            RuntimeExpr::Tuple(items)
                if matches!(
                    items.as_slice(),
                    [
                        RuntimeExpr::Local(label),
                        RuntimeExpr::Apply { callee, args },
                        RuntimeExpr::Local(exact),
                    ] if label == "label"
                        && exact == "exact"
                        && matches!(
                            callee.as_ref(),
                            RuntimeExpr::Local(name) if name == "add_label"
                        )
                        && matches!(
                            args.as_slice(),
                            [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(5)
                        )
                )
        )
}

#[test]
fn checked_runtime_plan_materializes_source_function_named_source_pipe_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    return tail |> pair(left = "head")
}

flow @flow.main main() -> String {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing source-function pipe");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    let RuntimeExpr::Apply { callee, args } = value else {
        panic!("expected tail_pair application, got {value:#?}");
    };
    assert!(matches!(
        args.as_slice(),
        [RuntimeExpr::Value(RuntimeValue::String(value))] if value == "tail"
    ));
    let RuntimeExpr::Function { params, body } = callee.as_ref() else {
        panic!("expected materialized tail_pair function, got {callee:#?}");
    };
    assert_eq!(params, &["tail"]);
    let (initializer, pair_with_left) = runtime_staged_pipe(body).unwrap_or_else(|| {
        panic!("source function pipe must keep pair(left = head)(tail), got {body:#?}")
    });
    assert!(matches!(initializer, RuntimeExpr::Local(name) if name == "tail"));
    assert!(matches!(
        pair_with_left,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["right"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Apply { args, .. }
                        if matches!(
                            args.as_slice(),
                            [RuntimeExpr::Value(RuntimeValue::String(left)), RuntimeExpr::Local(right)]
                                if left == "head" && right == "right"
                        )
                )
    ));
}

#[test]
fn checked_runtime_plan_materializes_source_function_control_expression_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
fn choose_score(value: i64, ready: bool) -> i64 {
    let boosted = if ready { value + 10i64 } else { value }
    return match ready {
        true when boosted > 10i64 => boosted
        false => value
        _ => 0i64
    }
}

flow @flow.main main() -> i64 {
    let value: i64 = choose_score(3i64, true)
    return value
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing control expressions");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["value", "ready"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "boosted"
                                        && matches!(expr.as_ref(), RuntimeExpr::If { .. })
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Match { scrutinee, arms }
                                                if matches!(
                                                    scrutinee.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "ready"
                                                ) && matches!(
                                                    arms.as_slice(),
                                                    [
                                                        arcweft_core::value::RuntimeExprMatchArm {
                                                            pattern: RuntimePattern::Literal(RuntimeValue::Bool(true)),
                                                            guard: Some(_),
                                                            value: RuntimeExpr::Local(first),
                                                        },
                                                        arcweft_core::value::RuntimeExprMatchArm {
                                                            pattern: RuntimePattern::Literal(RuntimeValue::Bool(false)),
                                                            guard: None,
                                                            value: RuntimeExpr::Local(second),
                                                        },
                                                        arcweft_core::value::RuntimeExprMatchArm {
                                                            pattern: RuntimePattern::Discard,
                                                            guard: None,
                                                            value: RuntimeExpr::Value(fallback),
                                                        },
                                                    ] if first == "boosted"
                                                        && second == "value"
                                                        && fallback == &RuntimeValue::i64(0)
                                                )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value), RuntimeExpr::Value(ready)]
                        if value == &RuntimeValue::i64(3) && ready == &RuntimeValue::Bool(true)
                )
        ),
        "expected value to apply source function containing if/match control expressions, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_if_let_expression_body() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
fn choose_optional(maybe: Option<i64>, fallback: i64) -> i64 {
    let selected = if let .Some(value) = maybe when value > fallback {
        value
    } else {
        fallback
    }
    return selected
}

flow @flow.main main() -> i64 {
    let value: i64 = choose_optional(Some(7i64), 1i64)
    return value
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function containing if-let expression");

    let [FlowOp::Let { expr: value, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected value let");
    };
    assert!(
        matches!(
            value,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["maybe", "fallback"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "selected"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::IfLet {
                                                pattern: RuntimePattern::Variant {
                                                    path: None,
                                                    name,
                                                    payload: Some(payload),
                                                },
                                                expr,
                                                guard: Some(_),
                                                then_expr,
                                                else_expr,
                                                } if name == "Some"
                                                && matches!(
                                                    payload.as_ref(),
                                                    RuntimePattern::Tuple(items)
                                                        if matches!(
                                                            items.as_slice(),
                                                            [RuntimePattern::Ident(value)] if value == "value"
                                                        )
                                                )
                                                && matches!(
                                                    expr.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "maybe"
                                                )
                                                && matches!(
                                                    then_expr.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "value"
                                                )
                                                && matches!(
                                                    else_expr.as_ref(),
                                                    RuntimeExpr::Local(name) if name == "fallback"
                                                )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Local(name) if name == "selected"
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Variant { name, .. }, RuntimeExpr::Value(fallback)]
                        if name == "Some" && fallback == &RuntimeValue::i64(1)
                )
        ),
        "expected value to apply source function containing if-let expression, got {value:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_callback_param_call() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn use_loader(path: String, load: String -> String) -> String {
    return load(path)
}

flow @flow.main main() -> String {
    let load = |path: String| path
    let body: String = use_loader("story.arcw", load)
    return body
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes source function callback call");

    let [
        FlowOp::Let { expr: load, .. },
        FlowOp::Let { expr: body, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected load and body lets");
    };
    assert!(matches!(
        load,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["path"]
                && matches!(body.as_ref(), RuntimeExpr::Local(name) if name == "path")
    ));
    assert!(
        matches!(
            body,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["path", "load"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Apply { callee, args }
                                    if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "load")
                                        && matches!(
                                            args.as_slice(),
                                            [RuntimeExpr::Local(name)] if name == "path"
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value), RuntimeExpr::Local(name)]
                        if value == &RuntimeValue::String("story.arcw".to_owned())
                            && name == "load"
                )
        ),
        "expected body to apply source function whose body applies callback param, got {body:#?}"
    );
}

#[test]
fn checked_runtime_plan_materializes_source_function_callback_partial_let() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn apply_suffix(prefix: String, combine: String -> String -> String, suffix: String) -> String {
    let with_prefix = combine(prefix)
    return with_prefix(suffix)
}

flow @flow.main main() -> String {
    let combine = |left: String| -> String -> String {
        return |right: String| left
    }
    let body: String = apply_suffix("story.arcw", combine, "tail")
    return body
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("checked runtime plan materializes callback partial let");

    let [
        FlowOp::Let { expr: combine, .. },
        FlowOp::Let { expr: body, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected combine and body lets");
    };
    assert!(matches!(
        combine,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["left"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["right"]
                            && matches!(body.as_ref(), RuntimeExpr::Local(name) if name == "left")
                )
    ));
    assert!(
        matches!(
            body,
            RuntimeExpr::Apply { callee, args }
                if matches!(
                    callee.as_ref(),
                    RuntimeExpr::Function { params, body }
                        if params.as_slice() == ["prefix", "combine", "suffix"]
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Let { name, expr, body }
                                    if name == "with_prefix"
                                        && matches!(
                                            expr.as_ref(),
                                            RuntimeExpr::Apply { callee, args }
                                                if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "combine")
                                                    && matches!(
                                                        args.as_slice(),
                                                        [RuntimeExpr::Local(name)] if name == "prefix"
                                                    )
                                        )
                                        && matches!(
                                            body.as_ref(),
                                            RuntimeExpr::Apply { callee, args }
                                                if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "with_prefix")
                                                    && matches!(
                                                        args.as_slice(),
                                                        [RuntimeExpr::Local(name)] if name == "suffix"
                                                    )
                                        )
                            )
                ) && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value), RuntimeExpr::Local(name), RuntimeExpr::Value(suffix)]
                        if value == &RuntimeValue::String("story.arcw".to_owned())
                            && name == "combine"
                            && suffix == &RuntimeValue::String("tail".to_owned())
                )
        ),
        "expected body to apply source function with callback partial let, got {body:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_source_function_partial_when_body_calls() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main() -> String {
    let trim_tail = trim_right(right = " tail ")
    let value: String = trim_tail("head")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects source function partials whose body calls");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `trim_right` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_source_function_partial_when_body_calls_unaccepted_source() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main() -> String {
    let normalize_tail = normalize(right = " tail ")
    let value: String = normalize_tail("head")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects partials through chained unsupported sources");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `normalize` partial application requires executable helper lowering",
                )
        }),
        "expected chained source partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_prefix_source_function_partial_when_body_calls() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main() -> String {
    let trim_head = trim_right("head")
    let value: String = trim_head(" tail ")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects unsupported prefix source function partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `trim_right` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper prefix partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_bare_source_function_value_when_body_calls() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main() -> String {
    let trim = trim_right
    let value: String = trim("head", " tail ")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects unsupported bare source function values");

    assert!(
        errors.iter().any(|error| {
            error.message().contains(
                "unsupported callable family `source_function_value_without_runtime_candidate`",
            ) && error
                .message()
                .contains("function `trim_right` cannot be referenced as a runtime function value")
        }),
        "expected unsupported bare source function value diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_bare_source_function_value_when_body_calls_unaccepted_source() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main() -> String {
    let normalize_value = normalize
    let value: String = normalize_value("head", " tail ")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects source functions that call unaccepted sources");

    assert!(
        errors.iter().any(|error| {
            error.message().contains(
                "unsupported callable family `source_function_value_without_runtime_candidate`",
            ) && error
                .message()
                .contains("function `normalize` cannot be referenced as a runtime function value")
        }),
        "expected unsupported chained source function diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main() -> String {
    let trim_tail: String -> String = "head" |> trim_right
    let value: String = trim_tail(" tail ")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects unsupported data-last source function partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `trim_right` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper data-last partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls_unaccepted_source()
 {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main() -> String {
    let normalize_tail: String -> String = "head" |> normalize
    let value: String = normalize_tail(" tail ")
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect_err("checked runtime plan rejects chained unsupported data-last source partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `normalize` partial application requires executable helper lowering",
                )
        }),
        "expected chained source data-last partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn runtime_plan_lowers_local_function_data_last_pipe_to_apply() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main() -> String {
    let f = add
    let partial = 2i64 |> f
    let exact = 2i64 |> f(1i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers local function data-last pipe");
    let [
        FlowOp::Let { expr: f, .. },
        FlowOp::Let { expr: partial, .. },
        FlowOp::Let { expr: exact, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected function alias, partial pipe, and exact pipe lets");
    };
    assert!(matches!(
        f,
        RuntimeExpr::Function { params, .. } if params.as_slice() == ["lhs", "rhs"]
    ));
    let (partial_initializer, partial_callee) =
        runtime_staged_pipe(partial).expect("bare local pipe keeps its final apply stage");
    assert!(matches!(
        partial_initializer,
        RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
    ));
    assert!(matches!(partial_callee, RuntimeExpr::Local(name) if name == "f"));

    let (exact_initializer, exact_callee) =
        runtime_staged_pipe(exact).expect("local call pipe keeps f(1)(lhs) staged");
    assert!(matches!(
        exact_initializer,
        RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
    ));
    assert!(matches!(
        exact_callee,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                && matches!(
                    args.as_slice(),
                    [RuntimeExpr::Value(value)] if value == &RuntimeValue::i64(1)
                )
    ));
}

#[test]
fn runtime_plan_keeps_curried_data_last_pipe_groups_staged() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
#[pure]
fn add(left: i64)(right: i64) -> i64 {
    return left + right
}

flow @flow.main main() -> i64 {
    let sum: i64 = 2i64 |> add(40i64)
    return sum
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers curried data-last pipe");
    let [FlowOp::Let { expr: sum, .. }, ..] = report.plan.flows[0].ops.as_slice() else {
        panic!("expected sum let");
    };
    let (initializer, add_left) =
        runtime_staged_pipe(sum).expect("pipeline must lower as add(40)(2)");
    assert!(matches!(
        initializer,
        RuntimeExpr::Value(value) if value == &RuntimeValue::i64(2)
    ));
    assert_eq!(
        runtime_apply_arg_counts(add_left),
        [1],
        "the authored add(40) group must remain before the pipeline receiver group: {sum:#?}"
    );
}

#[test]
fn runtime_plan_preserves_curried_call_group_application_samples() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r#"
#[pure]
fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64) {
    return (a, b, c)
}

#[pure]
fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64 {
    return a + b + c + d
}

flow @flow.main main() -> String {
    let tupled = tuple_tail(1i64, 2i64)(3i64)
    let sum = chain(1i64)(2i64)(3i64, 4i64)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers curried call-group samples");
    let [
        FlowOp::Let { expr: tupled, .. },
        FlowOp::Let { expr: sum, .. },
        ..,
    ] = report.plan.flows[0].ops.as_slice()
    else {
        panic!("expected tupled and sum lets");
    };

    assert_eq!(runtime_apply_arg_counts(tupled), [2, 1]);
    assert_eq!(runtime_apply_arg_counts(sum), [1, 1, 2]);
}

#[test]
fn runtime_plan_uses_typecheck_evidence_across_source_exprs() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
flow @flow.main main() -> i64 {
    let warmup = 1i64
    return warmup
}

pub source @source.values: Source<i64, String> {
    from input
    backpressure = latest
    replay = none
    privacy = transient

    on item value => yield f(value)
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    let function_ty = TypeKind::function([TypeKind::I64], TypeKind::I64);
    let source_ty = TypeKind::Source {
        item: Box::new(TypeKind::I64),
        error: Box::new(TypeKind::String),
    };
    let typecheck = arcweft_lang_sema::check::analyze_types(
        &hir,
        &TypeCheckEnv::standard()
            .with_symbol("f", function_ty)
            .with_symbol("input", source_ty),
    );
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime plan lowers with shared typed evidence cursor");

    assert!(report.plan.stream_plans.is_empty());
    assert!(matches!(
        report.plan.source_plans[0].handlers.as_slice(),
        [SourceHandlerPlan::Item { ops, .. }]
            if matches!(ops.as_slice(), [SourceOp::Yield(expr)]
                if matches!(
                    expr,
                    RuntimeExpr::Apply { callee, args }
                        if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                            && args.len() == 1
                ))
    ));
}

#[test]
fn runtime_plan_keeps_presentation_named_numeric_evidence_aligned() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
        r#"
flow main() -> String {
    image(asset = @asset:.bg.pulse, id = "image.pulse", x = 1px, opacity = 0.5, depth = 7, param.count = 9, visible = true)
    return "done"
}
"#,
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir = lower_document_to_hir(parsed.document(), parsed.typed_tree())
        .expect("presentation fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "presentation fixture must typecheck: {:#?}",
        typecheck.diagnostics
    );

    lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &admitted_options(),
    )
    .expect("runtime lowering must consume the same per-argument expression evidence");
}

#[test]
fn compiles_and_runs_bare_named_iterator_as_identity_into_iterator() {
    let compiled = compile_source(
        r"
struct Hoge { current: i32, end: i32 }

impl Iterator for Hoge {
    type Item = i32

    fn next(&mut self) -> Option<i32> {
        if self.current < self.end {
            let value = self.current
            self.current = self.current + 1i32
            Some(value)
        } else {
            None
        }
    }
}

flow @flow.main main() -> i32 {
    let source = Hoge { current: 0i32, end: 3i32 }
    for value in source {
        return value
    }
    return -1i32
}

entry cli @entry.main {
    goto @flow.main
}
",
    )
    .expect("bare named Iterator source compiles");

    assert_eq!(compiled.plan.trait_methods.len(), 1);
    let evidence = compiled
        .plan
        .flows
        .first()
        .and_then(|flow| {
            flow.ops.iter().find_map(|op| match op {
                FlowOp::For { evidence, .. } => Some(evidence),
                _ => None,
            })
        })
        .expect("for loop carries iterator evidence");
    let RuntimeIteratorEvidence::Witness(witness) = evidence else {
        panic!("bare named Iterator must use witness iterator evidence");
    };
    let RuntimeIteratorWitnessExecutable::IdentityIntoIterator(calls) = witness.executable else {
        panic!("bare named Iterator must lower through identity into-iterator evidence");
    };
    assert_eq!(calls.next.0, 0);

    let entry = compiled.plan.entries[0].id.clone();
    let mut engine =
        Engine::for_entry(compiled.plan, &entry).expect("explicit CLI entry starts its flow");
    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 16 },
        },
    );
    assert!(
        matches!(
            result.fiber_status,
            FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "0"
        ),
        "unexpected runtime result: {result:#?}"
    );
}

#[test]
fn runtime_plan_lowering_preserves_admitted_dialogue_profile() {
    let document = Arc::new(
        SourceDocument::try_new(
            SourceDocumentId::try_new("arcweft-test://compiler/runtime-plan/main.arcw")
                .expect("runtime-plan fixture source ID"),
            SourceName::path("compiler/runtime-plan/main.arcw"),
            r"
character @character.alice Alice as alice {}

flow @flow.main main {
alice: Hello[p]
}
",
        )
        .expect("runtime-plan fixture source document"),
    );
    let parsed = parse_document_with_source(Arc::clone(&document), ParseOptions::default());
    let hir =
        lower_document_to_hir(parsed.document(), parsed.typed_tree()).expect("fixture lowers");
    validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("fixture typechecks");

    let profile = DialoguePresentationProfile::new(
        ViewId::standard_dialogue(),
        Some(
            ViewStyleSheetId::try_new("style.dialogue.mobile")
                .expect("typed profile Style identity"),
        ),
        InlineFailurePolicy::FailLine,
    );
    let revision = test_dialogue_revision();

    let report = lower_source_runtime_plan_with_stats_and_options(
        &hir,
        &RuntimePlanLowerOptions::default()
            .with_dialogue_profile(profile.clone(), revision.clone()),
    )
    .expect("runtime plan lowers with the compiler-admitted dialogue profile");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(spec.view, *profile.view());
    assert_eq!(spec.profile_style, profile.style().cloned());
    assert_eq!(spec.dialogue_revision, revision);
    assert_eq!(spec.inline_failure, InlineFailurePolicy::FailLine);
}
