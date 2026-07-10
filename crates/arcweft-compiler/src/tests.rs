use arcweft_agent_protocol::artifact::EffectCapability as AgentEffectCapability;
use arcweft_bundle::BundleKind;
use arcweft_core::{
    engine::{Engine, FlowExit, FlowFiberStatus},
    pattern::RuntimePattern,
    plan::{
        FlowOp, RuntimeBuiltinIteratorEvidence, RuntimeIteratorEvidence,
        RuntimeIteratorWitnessExecutable,
    },
    source::{SourceHandlerPlan, SourceOp},
    step::{RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions},
    stream::StreamOp,
    value::{DenseSeq, RuntimeExpr, RuntimeSeq, RuntimeValue},
};
use arcweft_id::PublicId;
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::env::{FunctionParam, FunctionSignature, TypeCheckEnv};
use arcweft_lang_sema::project_index::{
    AgentActionParam, AgentActionSignature, DebugQuerySymbol, EntitySymbol, ProgramHash,
    ProjectCallableKind, ProjectCallableSymbol, ProjectGraphDependencyRelation,
    ProjectGraphDependencyRelationKind, ProjectGraphRelation, ProjectGraphRelationKind,
    ProjectGraphSymbolRef, ProjectSemanticIndex, QualifiedName, SemanticHash,
    project_semantic_index_from_hir,
};
use arcweft_lang_sema::types::{EntityKind, EntityType, TypeKind};
use arcweft_lang_syntax::parser::parse_source;
use arcweft_render_text::{RichTextColor, RichTextStyle};
use arcweft_runtime_plan::flow::RuntimePlanLowerOptions;
use arcweft_source::{SourceAnchor, SourceName};

use crate::{
    agent::{
        compile_agent_bundle_with_project, compile_agent_source, compile_agent_source_with_project,
    },
    agent_project::agent_project_graph_from_project,
    error::CompileAgentError,
    hir::{lower_source_tree, validate_hir_with_env},
    lower::{
        lower_source_runtime_plan_with_stats_and_options,
        lower_source_runtime_plan_with_typecheck_stats_and_options,
        lower_source_text_pure_helper_candidates,
    },
    parse::parse_source_text,
    source::compile_source,
    types::{TextPureHelperCandidateError, TextPureHelperKind},
};

fn public_id(value: &str) -> PublicId {
    PublicId::try_new(value).expect("valid public id")
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

fn project_with_entity(id: &str, kind: EntityKind) -> ProjectSemanticIndex {
    project_with_typed_entity(id, kind, None)
}

fn project_with_typed_entity(
    id: &str,
    kind: EntityKind,
    value: Option<TypeKind>,
) -> ProjectSemanticIndex {
    ProjectSemanticIndex::new(ProgramHash::new("program-test")).with_entity(EntitySymbol::new(
        public_id(id),
        EntityType::new(kind, value),
        SourceAnchor::generated(),
        SemanticHash::new(format!("shape.{id}.v1")),
    ))
}

#[test]
fn agent_project_graph_snapshot_preserves_project_relations() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"))
        .with_entity(EntitySymbol::new(
            public_id("entry.main"),
            EntityType::new(EntityKind::Entry, None),
            SourceAnchor::generated(),
            SemanticHash::new("shape.entry.main.v1"),
        ))
        .with_entity(EntitySymbol::new(
            public_id("flow.opening"),
            EntityType::new(EntityKind::Flow, None),
            SourceAnchor::generated(),
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
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"))
        .with_project_callable(
            QualifiedName::new("update_route"),
            ProjectCallableSymbol::new(
                ProjectCallableKind::Reducer,
                FunctionSignature::new(
                    TypeKind::Named("GameState".to_owned()),
                    [
                        FunctionParam::required("state", TypeKind::Named("GameState".to_owned())),
                        FunctionParam::required("event", TypeKind::Named("GameEvent".to_owned())),
                    ],
                ),
                SourceAnchor::generated(),
                SemanticHash::new("hir:callable:reducer:update_route:(state: GameState)"),
            ),
        )
        .with_project_callable(
            QualifiedName::new("current_route"),
            ProjectCallableSymbol::new(
                ProjectCallableKind::View,
                FunctionSignature::new(TypeKind::entity_ref(EntityKind::Flow), []),
                SourceAnchor::generated(),
                SemanticHash::new("hir:callable:view:current_route:(state: GameState)"),
            ),
        )
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
        symbol.symbol_id == "project:callable:update_route"
            && symbol.qualified_name.as_deref() == Some("update_route")
            && symbol.kind == "project_reducer"
    }));
    assert!(graph.symbols.iter().any(|symbol| {
        symbol.symbol_id == "project:callable:current_route"
            && symbol.qualified_name.as_deref() == Some("current_route")
            && symbol.kind == "project_view"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.from_symbol_id == "project:summary"
            && edge.to_symbol_id == "project:callable:update_route"
            && edge.edge_kind == "contains_callable"
    }));
    assert!(graph.edges.iter().any(|edge| {
        edge.from_symbol_id == "project:callable:update_route"
            && edge.to_symbol_id == "project:callable:current_route"
            && edge.edge_kind == "calls_callable"
    }));
    assert_eq!(summary.project_callable_count, 2);
    assert_eq!(summary.dependency_edge_count, 1);
}

#[test]
fn agent_project_graph_snapshot_preserves_flow_control_summary() {
    let tree = parse_source(
        r#"
pub reducer current_route() -> Ref<Flow> {
return @flow.done
}

flow @flow.opening opening {
let route = current_route()
goto @flow.done
goto route
}

flow @flow.done done {
return "done"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let project = project_semantic_index_from_hir(
        &hir,
        ProgramHash::new("program-test"),
        &SourceName::path("game.arcw"),
    )
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

fn project_with_agent_action(
    id: &str,
    kind: EntityKind,
    action: &str,
    params: impl IntoIterator<Item = AgentActionParam>,
) -> ProjectSemanticIndex {
    ProjectSemanticIndex::new(ProgramHash::new("program-test")).with_entity(
        EntitySymbol::new(
            public_id(id),
            EntityType::new(kind, None),
            SourceAnchor::generated(),
            SemanticHash::new(format!("shape.{id}.v1")),
        )
        .with_agent_action(AgentActionSignature::new(
            QualifiedName::new(action),
            params,
            TypeKind::ActionResult,
        )),
    )
}

fn project_with_typed_debug_paths() -> ProjectSemanticIndex {
    ProjectSemanticIndex::new(ProgramHash::new("program-test"))
        .with_debug_query(
            QualifiedName::new("state.route.phase"),
            DebugQuerySymbol::new(FunctionSignature::return_only(TypeKind::String)),
        )
        .with_debug_query(
            QualifiedName::new("observation.tick"),
            DebugQuerySymbol::new(FunctionSignature::return_only(TypeKind::U64)),
        )
}

#[test]
fn compiles_dialogue_source_to_plan_and_display_catalog() {
    let source = r"
character @character.alice Alice as alice {}

entry game @entry.main {
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
    let parsed = parse_source_text(
        r"
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
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
flow @flow.main main {
    let ok: bool = f(1i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(
        &hir,
        &TypeCheckEnv::standard()
            .with_symbol("f", TypeKind::function([TypeKind::I64], TypeKind::Bool)),
    );
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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

    let plain_report =
        lower_source_runtime_plan_with_stats_and_options(&hir, &RuntimePlanLowerOptions::default())
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
    let parsed = parse_source_text(
        r#"
flow @flow.main main {
    let ok: bool = f(1i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
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
        &RuntimePlanLowerOptions::default()
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
    let parsed = parse_source_text(
        r#"
flow @flow.main main {
    let accepted: bool = accept(_ > 80i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
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
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
flow @flow.main main {
    let limit: i64 = 80i64
    let is_high = |score: i64| -> bool {
        score >= limit
    }
    let ok: bool = is_high(81i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
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
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main {
    let high = _ > 80i64
    let high_grouped = (_ > 80i64)
    let add_one = add(_, 1i64)
    let double = add(_, _)
    let add_to_one = add(right = _, left = 1i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main {
    let named_missing = add(right = 1i64)
    return named_missing(2i64)
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn above(min: i64, value: i64) -> bool {
    return value > min
}

flow @flow.main main {
    let score = 90i64
    let ok = score.above(80i64)
    let named = score.above(min = 80i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn above(min: i64)(value: i64) -> bool {
    return value > min
}

fn trim(prefix: String)(value: String) -> String {
    return value
}

flow @flow.main main {
    let compare = above
    let score = 90i64
    let source = score.above(80i64)
    let local = score.compare(80i64)
    let text = " padded "
    let inherent = text.trim()
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn between(min: i64, max: i64, value: i64) -> bool {
    return value > min
}

flow @flow.main main {
    let score = 75i64
    let direct = score.between([60i64, 90i64]...)
    let mixed = score.between([60i64]..., max = 90i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main {
    let partial = 2i64 |> add
    let positional = 2i64 |> add(1i64)
    let named = 2i64 |> add(lhs = 1i64)
    let named_rhs = 2i64 |> add(rhs = 1i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
flow @flow.main main {
    let maybe = Some(7i64)
    let selected: i64 = maybe |> if let .Some(value) = ^ when value > 1i64 {
        value
    } else {
        1i64
    }
    return selected
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
flow @flow.main main {
    let ready = true
    let selected: i64 = ready |> match ^ {
        true => 7i64
        false => 1i64
    }
    return selected
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main {
    let add_two = add(2i64)
    let seven: i64 = add_two(5i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn choose(left: String, right: String) -> (String, String) {
    return (left, right)
}

flow @flow.main main {
    let via_right: (String, String) = "pipe-left" |> choose(right = "named-right")
    let via_left: (String, String) = "pipe-right" |> choose(left = "named-left")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
flow @flow.main main {
    let choose = |(left, right): (String, String)| right
    let value: String = choose(("head", "tail"))
    return value
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn choose(left: String, right: String) -> String {
    return right
}

flow @flow.main main {
    let choose_right = choose(right = "tail")
    let value: String = choose_right("head")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn add(left: i64, right: i64) -> i64 {
    return left + right
}

flow @flow.main main {
    let add_one = add([1i64]...)
    let exact: i64 = add([1i64]..., 2i64)
    let value: i64 = add_one(2i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn pair(left: String)(right: String) -> (String, String) {
    return (left, right)
}

flow @flow.main main {
    let with_left = pair("left")
    let tupled: (String, String) = with_left("right")
    let direct: (String, String) = pair("x")("y")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn add(a: i64)(b: i64) -> i64 {
    return a + b
}

flow @flow.main main {
    let add_one = add(1i64)
    let ok: i64 = add_one([2i64]...)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn sum(a: i64, b: i64) -> i64 {
    return a + b
}

flow main {
    let callback: (i64, i64) -> i64 = sum
    let total: i64 = callback([1i64, 2i64]...)
    let wide: u128 = 340282366920938463463374607431768211455
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "{:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect("spread container and following numeric evidence stay aligned");
    let FlowOp::Let { expr: wide, .. } = &report.plan.flows[0].ops[2] else {
        panic!("expected wide let after function-value spread");
    };
    assert_eq!(wide, &RuntimeExpr::Value(RuntimeValue::u128(u128::MAX)));
}

#[test]
fn checked_runtime_plan_materializes_source_function_returned_closure() {
    let parsed = parse_source_text(
        r#"
fn pairer(left: String) -> String -> (String, String) {
    return |right: String| (left, right)
}

flow @flow.main main {
    let with_left = pairer("left")
    let tupled: (String, String) = with_left("right")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn choose_right(pair: (String, String)) -> String {
    let choose = |(left, right): (String, String)| right
    return choose(pair)
}

flow @flow.main main {
    let value: String = choose_right(("head", "tail"))
    return value
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
#[pure]
fn add(left: i64, right: i64) -> i64 {
    left + right
}

fn finish_with_tail(value: i64, id: i64 -> i64) -> i64 {
    let finish = |item: i64| add(right = 5i64, left = item)
    return finish(value)
}

flow @flow.main main {
    let id = |item: i64| item
    let value: i64 = finish_with_tail(7i64, id)
    return value
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
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

flow @flow.main main {
    let value: (String, i64) = finish_with_alias("score", 7i64)
    return value
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    return pair(right = tail, left = "head")
}

flow @flow.main main {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    let make_pair = pair
    return make_pair("head", tail)
}

flow @flow.main main {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
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

flow @flow.main main {
    let value: (String, i64, i64) = finish_with_pipe("score", 7i64)
    return value
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn pair(left: String, right: String) -> (String, String) {
    return (left, right)
}

fn tail_pair(tail: String) -> (String, String) {
    return tail |> pair(left = "head")
}

flow @flow.main main {
    let value: (String, String) = tail_pair("tail")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(typecheck.diagnostics.is_empty());

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
fn choose_score(value: i64, ready: bool) -> i64 {
    let boosted = if ready { value + 10i64 } else { value }
    return match ready {
        true when boosted > 10i64 => boosted
        false => value
        _ => 0i64
    }
}

flow @flow.main main {
    let value: i64 = choose_score(3i64, true)
    return value
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
fn choose_optional(maybe: Option<i64>, fallback: i64) -> i64 {
    let selected = if let .Some(value) = maybe when value > fallback {
        value
    } else {
        fallback
    }
    return selected
}

flow @flow.main main {
    let value: i64 = choose_optional(Some(7i64), 1i64)
    return value
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn use_loader(path: String, load: String -> String) -> String {
    return load(path)
}

flow @flow.main main {
    let load = |path: String| path
    let body: String = use_loader("story.arcw", load)
    return body
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn apply_suffix(prefix: String, combine: String -> String -> String, suffix: String) -> String {
    let with_prefix = combine(prefix)
    return with_prefix(suffix)
}

flow @flow.main main {
    let combine = |left: String| -> String -> String {
        return |right: String| left
    }
    let body: String = apply_suffix("story.arcw", combine, "tail")
    return body
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim_tail = trim_right(right = " tail ")
    let value: String = trim_tail("head")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main {
    let normalize_tail = normalize(right = " tail ")
    let value: String = normalize_tail("head")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim_head = trim_right("head")
    let value: String = trim_head(" tail ")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim = trim_right
    let value: String = trim("head", " tail ")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main {
    let normalize_value = normalize
    let value: String = normalize_value("head", " tail ")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
fn checked_runtime_plan_rejects_bare_task_function_value() {
    let parsed = parse_source_text(
        r#"
task fn load_label(name: String) -> String {
    return name
}

flow @flow.main main {
    let loader = load_label
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported bare task function values");

    assert!(
        errors.iter().any(|error| {
            error.message().contains(
                "unsupported callable family `source_function_value_without_runtime_candidate`",
            ) && error
                .message()
                .contains("function `load_label` cannot be referenced as a runtime function value")
        }),
        "expected unsupported bare task function value diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_bare_dialogue_function_value() {
    let parsed = parse_source_text(
        r#"
dialogue fn format_line(name: String) -> String {
    return name
}

flow @flow.main main {
    let formatter = format_line
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported bare dialogue function values");

    assert!(
        errors.iter().any(|error| {
            error.message().contains(
                "unsupported callable family `source_function_value_without_runtime_candidate`",
            ) && error
                .message()
                .contains("function `format_line` cannot be referenced as a runtime function value")
        }),
        "expected unsupported bare dialogue function value diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_bare_stream_function_value() {
    let parsed = parse_source_text(
        r#"
stream fn passthrough(frames: Stream<i64, String>) -> Stream<i64, String> {
    for frame in frames {
        yield frame
    }
}

flow @flow.main main {
    let transform = passthrough
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported bare stream function values");

    assert!(
        errors.iter().any(|error| {
            error.message().contains(
                "unsupported callable family `source_function_value_without_runtime_candidate`",
            ) && error
                .message()
                .contains("function `passthrough` cannot be referenced as a runtime function value")
        }),
        "expected unsupported bare stream function value diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_data_last_source_function_partial_when_body_calls() {
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

flow @flow.main main {
    let trim_tail: String -> String = "head" |> trim_right
    let value: String = trim_tail(" tail ")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
fn trim_right(left: String, right: String) -> String {
    return right.trim()
}

fn normalize(left: String, right: String) -> String {
    return trim_right(left, right)
}

flow @flow.main main {
    let normalize_tail: String -> String = "head" |> normalize
    let value: String = normalize_tail(" tail ")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
fn checked_runtime_plan_rejects_data_last_task_function_partial() {
    let parsed = parse_source_text(
        r#"
task fn load_label(prefix: String, name: String) -> String {
    return name
}

flow @flow.main main {
    let load_named: String -> String = "Ada" |> load_label
    let value: String = load_named("prefix")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported data-last task partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `load_label` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper task data-last partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_data_last_dialogue_function_partial() {
    let parsed = parse_source_text(
        r#"
dialogue fn format_line(prefix: String, name: String) -> String {
    return name
}

flow @flow.main main {
    let format_named: String -> String = "Ada" |> format_line
    let value: String = format_named("prefix")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported data-last dialogue partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `format_line` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper dialogue data-last partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn checked_runtime_plan_rejects_data_last_stream_function_partial() {
    let parsed = parse_source_text(
        r#"
stream fn tag_frame(prefix: String, name: String) -> Stream<String, String> {
    yield name
}

flow @flow.main main {
    let tag_named: String -> Stream<String, String> = "Ada" |> tag_frame
    let values: Stream<String, String> = tag_named("prefix")
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let errors = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
    )
    .expect_err("checked runtime plan rejects unsupported data-last stream partials");

    assert!(
        errors.iter().any(|error| {
            error
                .message()
                .contains("unsupported callable family `signature_partial_without_helper`")
                && error.message().contains(
                    "function `tag_frame` partial application requires executable helper lowering",
                )
        }),
        "expected non-helper stream data-last partial diagnostic, got {errors:#?}"
    );
}

#[test]
fn runtime_plan_lowers_local_function_data_last_pipe_to_apply() {
    let parsed = parse_source_text(
        r#"
fn add(lhs: i64, rhs: i64) -> i64 {
    return lhs + rhs
}

flow @flow.main main {
    let f = add
    let partial = 2i64 |> f
    let exact = 2i64 |> f(1i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r"
#[pure]
fn add(left: i64)(right: i64) -> i64 {
    return left + right
}

flow @flow.main main {
    let sum: i64 = 2i64 |> add(40i64)
    return sum
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
    let parsed = parse_source_text(
        r#"
#[pure]
fn tuple_tail(a: i64, b: i64)(c: i64) -> (i64, i64, i64) {
    return (a, b, c)
}

#[pure]
fn chain(a: i64)(b: i64)(c: i64, d: i64) -> i64 {
    return a + b + c + d
}

flow @flow.main main {
    let tupled = tuple_tail(1i64, 2i64)(3i64)
    let sum = chain(1i64)(2i64)(3i64, 4i64)
    return "done"
}
"#,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    let typecheck = arcweft_lang_sema::check::analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "unexpected type errors: {:#?}",
        typecheck.diagnostics
    );

    let report = lower_source_runtime_plan_with_typecheck_stats_and_options(
        &hir,
        &typecheck,
        &RuntimePlanLowerOptions::default(),
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
fn runtime_plan_uses_typecheck_evidence_across_stream_and_source_exprs() {
    let parsed = parse_source_text(
        r"
flow @flow.main main {
    let warmup = 1i64
    return warmup
}

stream fn relay(values: Stream<i64, String>) -> Stream<i64, String> {
    for value in values {
        yield f(value)
    }
}

pub source @source.values: Source<i64, String> {
    from input
    backpressure = latest
    replay = none
    privacy = transient

    on item value => yield f(value)
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
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
        &RuntimePlanLowerOptions::default(),
    )
    .expect("runtime plan lowers with shared typed evidence cursor");

    assert!(matches!(
        report.plan.stream_plans[0].ops.as_slice(),
        [StreamOp::ForNext { body, .. }]
            if matches!(body.as_slice(), [StreamOp::Yield { expr }]
                if matches!(
                    expr,
                    RuntimeExpr::Apply { callee, args }
                        if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                            && args.len() == 1
                ))
    ));
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

flow @flow.main main -> i32 {
    let source = Hoge { current: 0i32, end: 3i32 }
    for value in source {
        return value
    }
    return -1i32
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

    let mut engine = Engine::new(compiled.plan);
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
fn compiles_agent_source_through_agent_dialect() {
    let compiled = compile_agent_source(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
observe()
}
",
    )
    .expect("agent source compiles");

    assert_eq!(compiled.hir.agents().len(), 1);
    assert_eq!(compiled.hir.agents()[0].item().name(), "opening_smoke");
}

#[test]
fn compile_agent_source_rejects_removed_line_commands() {
    let error = compile_agent_source("observe\n").expect_err("legacy command fails");

    assert!(matches!(error, CompileAgentError::Parse(_)));
}

#[test]
fn compile_agent_source_with_project_checks_choose_intrinsic() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
try choose(@choice.opening.listen)
}
",
        &project,
    )
    .expect("agent source typechecks against project index");

    assert_eq!(compiled.hir.agents().len(), 1);
    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionResult)
    );
}

#[test]
fn compile_agent_source_with_project_rejects_choose_family_mismatch() {
    let project = project_with_entity("flow.main", EntityKind::Flow);
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
choose(@flow.main)
}
",
        &project,
    )
    .expect_err("flow is not a choice option");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_checks_entity_ref_metadata_fields() {
    let project = project_with_entity("flow.opening", EntityKind::Flow);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.project_ref_metadata project_ref_metadata()
effects { }
{
let route_id = (@flow.opening).id
let route_family = (@flow.opening).family
let route_name = (@flow.opening).name
expect(route_id == "flow.opening", "id field")
expect(route_family == "flow", "family field")
expect(route_name == "opening", "name field")
}
"#,
        &project,
    )
    .expect("project entity ref metadata fields typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .filter(|judgment| judgment.ty == TypeKind::String)
            .count()
            >= 3
    );
}

#[test]
fn compile_agent_source_with_project_checks_entity_meta_intrinsic() {
    let project = project_with_entity("flow.opening", EntityKind::Flow);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.project_entity_meta project_entity_meta()
effects { debug.read }
{
let meta = try entity_meta(@flow.opening)
let hash = meta.semantic_hash
let path = meta.source.path
let has_source = meta.source.has_source
expect(meta.id == "flow.opening", "metadata id")
expect(meta.kind == "flow", "metadata kind")
expect(hash == "shape.flow.opening.v1", "semantic hash")
expect(path == "", "generated source path")
expect(has_source == false, "generated source flag")
}
"#,
        &project,
    )
    .expect("project entity metadata intrinsic typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentEntityMetadata)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentSourceAnchor)
    );
}

#[test]
fn compile_agent_source_with_project_checks_project_neighbors_intrinsic() {
    let project = project_with_entity("flow.opening", EntityKind::Flow);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.project_neighbors project_neighbors_smoke()
effects { debug.read }
{
let graph = try project_neighbors(@flow.opening, depth = 1u32)
expect(graph.root == "flow.opening")
expect(graph.node_count > 0u32)
expect(graph.edge_count > 0u32)
let symbol = graph.symbols[0]
let edge = graph.edges[0]
expect(symbol.kind != "")
expect(symbol.has_project_summary)
expect(symbol.entity_count > 0u32)
expect(symbol.relation_count > 0u32)
expect(symbol.has_flow_control == false)
expect(symbol.dynamic_goto_count >= 0u32)
expect(edge.kind != "")
}
"#,
        &project,
    )
    .expect("project graph neighborhood intrinsic typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentProjectGraphNeighborhood)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentProjectGraphSymbol)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentProjectGraphEdge)
    );
}

#[test]
fn compile_agent_source_with_project_checks_advance_text_intrinsic() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.advance_text advance_text()
effects { agent.act.semantic }
{
try advance_text()
}
",
        &project,
    )
    .expect("advance_text intrinsic typechecks with semantic action effect");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionResult)
    );
}

#[test]
fn compile_agent_source_with_project_rejects_advance_text_without_effect() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.advance_text advance_text()
effects { agent.observe }
{
advance_text()
}
",
        &project,
    )
    .expect_err("advance_text requires semantic action effect");

    assert!(error.to_string().contains("agent.act.semantic"));
}

#[test]
fn compile_agent_source_with_project_checks_pointer_click_intrinsic() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.pointer_click pointer_click()
effects { agent.act.physical }
{
try pointer.click(viewport_point(12u32, 34u32), button = .primary)
}
",
        &project,
    )
    .expect("pointer.click intrinsic typechecks with physical action effect");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionResult)
    );
}

#[test]
fn compile_agent_source_with_project_rejects_pointer_click_without_effect() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.pointer_click pointer_click()
effects { agent.act.semantic }
{
pointer.click(viewport_point(12u32, 34u32))
}
",
        &project,
    )
    .expect_err("pointer.click requires physical action effect");

    assert!(error.to_string().contains("agent.act.physical"));
}

#[test]
fn compile_agent_source_with_project_checks_typed_debug_paths() {
    let project = project_with_typed_debug_paths();
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_path debug_path()
effects { debug.read, agent.observe, agent.wait }
{
try wait(
    all(
        state("route.phase").eq("opening"),
        observation("tick").ge(1u64),
    ),
    timeout = 5ms,
)
}
"#,
        &project,
    )
    .expect("typed debug paths typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Probe(Box::new(TypeKind::String)))
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Probe(Box::new(TypeKind::U64)))
    );
}

#[test]
fn compile_agent_source_with_project_checks_typed_debug_path_constructors() {
    let project = project_with_typed_debug_paths();
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_path debug_path()
effects { debug.read, agent.observe, agent.wait }
{
let route = state_path("route.phase")
let tick = observation_path("tick")
try wait(
    all(
        state(route).eq("opening"),
        observation(tick).ge(1u64),
    ),
    timeout = 5ms,
)
}
"#,
        &project,
    )
    .expect("typed debug path constructors typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::DebugStatePath)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ObservationFieldPath)
    );
}

#[test]
fn compile_agent_source_with_project_rejects_empty_debug_path_constructor() {
    let project = project_with_typed_debug_paths();
    let error = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_path debug_path()
effects { debug.read }
{
state_path("")
}
"#,
        &project,
    )
    .expect_err("empty typed debug path is rejected");

    assert!(error.to_string().contains("debug state path"));
    assert!(error.to_string().contains("must not be empty"));
}

#[test]
fn compile_agent_source_with_project_rejects_debug_path_value_mismatch() {
    let project = project_with_typed_debug_paths();
    let error = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_path debug_path()
effects { debug.read }
{
state("route.phase").eq(1u64)
}
"#,
        &project,
    )
    .expect_err("typed debug path rejects mismatched comparison value");

    assert!(error.to_string().contains("Probe.eq expected value"));
    assert!(error.to_string().contains("String"));
    assert!(error.to_string().contains("u64"));
}

#[test]
fn compile_agent_source_with_project_rejects_unresolved_project_entity() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-test"));
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.act.semantic }
{
choose(@choice.opening.listen)
}
",
        &project,
    )
    .expect_err("missing project entity");

    assert!(matches!(error, CompileAgentError::Resolve(_)));
}

#[test]
fn compile_agent_source_with_project_checks_signal_probe_wait() {
    let project =
        project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
try wait(signal(@signal.ready).eq(true), timeout = 5s)
}
",
        &project,
    )
    .expect("signal probe and wait typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Predicate)
    );
}

#[test]
fn compile_agent_source_with_project_checks_statement_wait_entity_probe() {
    let project = project_with_typed_entity(
        "signal.current_flow",
        EntityKind::Signal,
        Some(TypeKind::entity_ref(EntityKind::Flow)),
    )
    .with_entity(EntitySymbol::new(
        public_id("flow.opening"),
        EntityType::new(EntityKind::Flow, None),
        SourceAnchor::generated(),
        SemanticHash::new("shape.flow.opening.v1"),
    ));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
try wait(signal(@signal.current_flow).eq(@flow.opening), timeout = 5s, stable_frames = 1u32, poll_frames = 1u32)
}
",
        &project,
    )
    .expect("statement-form wait lowers to typed Agent intrinsic");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Predicate)
    );
}

#[test]
fn compile_agent_source_with_project_checks_action_enabled_predicate() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.action_wait action_wait()
effects { agent.observe, agent.wait }
{
let listen = choice_action(@choice.opening.listen)
try wait(action_enabled(listen), timeout = 5s)
}
",
        &project,
    )
    .expect("action_enabled predicate typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Predicate)
    );
}

#[test]
fn compile_agent_source_with_project_rejects_signal_payload_mismatch() {
    let project =
        project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
    let error = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe }
{
signal(@signal.ready).eq("yes")
}
"#,
        &project,
    )
    .expect_err("signal bool payload rejects string comparison");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_rejects_wait_without_timeout() {
    let project =
        project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
wait(signal(@signal.ready).eq(true))
}
",
        &project,
    )
    .expect_err("wait requires timeout");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_checks_metric_probe() {
    let project = project_with_typed_entity("metric.fps", EntityKind::Metric, Some(TypeKind::F32));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.perf_watch perf_watch()
effects { agent.observe }
{
metric(@metric.fps).eq(60.0f32)
}
",
        &project,
    )
    .expect("metric probe typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
}

#[test]
fn compile_agent_source_with_project_checks_composite_predicates() {
    let project =
        project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool))
            .with_entity(EntitySymbol::new(
                public_id("metric.fps"),
                EntityType::new(EntityKind::Metric, Some(TypeKind::F32)),
                SourceAnchor::generated(),
                SemanticHash::new("shape.metric.fps.v1"),
            ));
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.composite_wait composite_wait()
effects { agent.observe, agent.wait }
{
try wait(all(signal(@signal.ready).eq(true), not(metric(@metric.fps).lt(30.0f32))), timeout = 5s)
}
",
        &project,
    )
    .expect("composite predicate typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .filter(|judgment| judgment.ty == TypeKind::Predicate)
            .count()
            >= 3
    );
}

#[test]
fn compile_agent_source_with_project_checks_state_and_observation_probes() {
    let project = ProjectSemanticIndex::new(ProgramHash::new("program-a"));
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_state debug_state()
effects { agent.observe, agent.wait, debug.read }
{
try wait(
    all(state("route.phase").eq("opening"), observation("tick").ge(1i64)),
    timeout = 5s,
)
}
"#,
        &project,
    )
    .expect("state and observation probes typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Probe(Box::new(TypeKind::AgentValue)))
    );
}

#[test]
fn compile_agent_source_with_project_rejects_wait_zero_stable() {
    let project =
        project_with_typed_entity("signal.ready", EntityKind::Signal, Some(TypeKind::Bool));
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.opening_smoke opening_smoke()
effects { agent.observe, agent.wait }
{
wait(signal(@signal.ready).eq(true), timeout = 5s, stable_frames = 0u32)
}
",
        &project,
    )
    .expect_err("wait stable must be positive");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_checks_observation_action_contains() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.action_probe action_probe()
effects { agent.observe }
{
let frame = try observe()
expect(frame.actions.contains(choice_action(@choice.opening.listen)))
}
",
        &project,
    )
    .expect("observation action contains expression typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionTarget)
    );
}

#[test]
fn compile_agent_source_with_project_checks_action_target_fields() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.action_fields action_fields()
effects { agent.observe }
{
let frame = try observe()
let expected = choice_action(@choice.opening.listen)
expect(expected.id == "action.select_choice.choice.opening.listen")
expect(expected.target == "choice.opening.listen")
expect(expected.action == "select_choice")
expect(expected.kind == "semantic")
expect(expected.enabled)
expect(frame.actions[0].enabled)
expect(frame.actions[0].target == expected.target)
}
"#,
        &project,
    )
    .expect("ActionTarget field projections typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Bool)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::String)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionName)
    );
}

#[test]
fn compile_agent_source_with_project_checks_observed_object_fields_and_capture_target() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.visual_dialogue visual_dialogue()
effects { agent.observe, agent.capture }
{
let frame = try observe()
let textbox = try frame.objects.require_role("dialogue_textbox")
let color = try capture(
    object(textbox.id),
    format = .png,
    kind = .color,
    name = "dialogue-textbox-color",
)
expect(textbox.role == "dialogue_textbox")
expect(textbox.bbox.width > 0u32)
expect(textbox.bbox.height > 0u32)
return color.uri
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("ObservedObject fields and object capture target typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ObservedObject)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentBBox)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::CaptureRef)
    );
}

#[test]
fn compile_agent_source_with_project_checks_opening_smoke_try_surface() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption)
        .with_entity(EntitySymbol::new(
            public_id("signal.current_flow"),
            EntityType::new(
                EntityKind::Signal,
                Some(TypeKind::entity_ref(EntityKind::Flow)),
            ),
            SourceAnchor::generated(),
            SemanticHash::new("shape.signal.current_flow.v1"),
        ))
        .with_entity(EntitySymbol::new(
            public_id("flow.alice_intro"),
            EntityType::new(EntityKind::Flow, None),
            SourceAnchor::generated(),
            SemanticHash::new("shape.flow.alice_intro.v1"),
        ));
    let source = r#"
#[agent(version = 1)]
agent @agent.opening.listen opening_listen()
effects {
agent.observe,
agent.act.semantic,
agent.wait,
agent.capture,
debug.record,
}
{
let first = try observe()
expect(
    first.actions.contains(choice_action(@choice.opening.listen)),
    message = "opening choice is not currently selectable",
)

scope choose_listen {
    let action = try choose(@choice.opening.listen)
    expect(action.accepted)

    let next = try wait(
        signal(@signal.current_flow).eq(@flow.alice_intro),
        timeout = 5s,
        stable_frames = 2u32,
        poll_frames = 1u32,
    )
    expect(next.signals.get(@signal.current_flow) == @flow.alice_intro)
}

let image = try capture(
    viewport(),
    format = .png,
    name = "after-choice",
)
attach(image)
checkpoint("opening-listen-complete")

Ok(())
}
"#;
    let compiled = compile_agent_source_with_project(source, &project)
        .expect("opening smoke try surface typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Observation)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionResult)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::CaptureRef)
    );
}

#[test]
fn compile_agent_source_with_project_checks_failure_investigation_surface() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption)
        .with_entity(EntitySymbol::new(
            public_id("signal.current_flow"),
            EntityType::new(
                EntityKind::Signal,
                Some(TypeKind::entity_ref(EntityKind::Flow)),
            ),
            SourceAnchor::generated(),
            SemanticHash::new("shape.signal.current_flow.v1"),
        ))
        .with_entity(EntitySymbol::new(
            public_id("flow.alice_intro"),
            EntityType::new(EntityKind::Flow, None),
            SourceAnchor::generated(),
            SemanticHash::new("shape.flow.alice_intro.v1"),
        ));
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug.opening_route investigate_opening_route()
effects {
agent.observe,
agent.act.semantic,
agent.wait,
agent.capture,
debug.record,
rag.query,
}
{
let before = try observe()
note(fmt("initial tick={before.tick} state={before.state_hash}"))

let result = try choose(@choice.opening.listen)
checkpoint("choice-dispatched")

let after = try wait(
    any([
        signal(@signal.current_flow).eq(@flow.alice_intro),
        diagnostics().has_error(),
    ]),
    timeout = 8s,
)

if after.signals.get(@signal.current_flow) != @flow.alice_intro {
    let context = try rag.query(
        "opening listen choice did not reach alice_intro",
        roots = [@choice.opening.listen, @flow.alice_intro],
        graph_depth = 2u32,
        limit = 12usize,
    )
    note(context.summary())

    let image = try capture(viewport(), name = "route-failure")
    attach(image)

    expect(false, message = "opening route failed; investigation context attached")
}

expect(result.accepted)
Ok(())
}
"#,
        &project,
    )
    .expect("failure investigation surface typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::Predicate)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::RagContextPack)
    );
}

#[test]
fn compile_agent_source_with_project_checks_invoke_intrinsic() {
    let project = project_with_agent_action(
        "activity.inventory",
        EntityKind::Activity,
        "open",
        [AgentActionParam::required("label", TypeKind::String)],
    );
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
try invoke(@activity.inventory, .open, { label = "main" })
}
"#,
        &project,
    )
    .expect("invoke intrinsic typechecks against project action signature");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::ActionResult)
    );
}

#[test]
fn compile_agent_source_with_project_checks_projected_image_invoke_action() {
    let project_source = parse_source(
        r#"
asset bg.pulse {
file = "bg/pulse.gif"
kind = image
}

flow @flow.opening opening {
let pulse = image(asset = @asset:.bg.pulse, target = "target.sample.pulse", layer = "layer.foreground", x = 96px, y = 72px, width = 360px, height = 180px, action = "action.inspect.pulse")
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&project_source).expect("project source lowers");
    let project = project_semantic_index_from_hir(
        &hir,
        ProgramHash::new("program-test"),
        &arcweft_source::SourceName::path("game.arcw"),
    )
    .expect("project source indexes image action");
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.inspect_pulse inspect_pulse()
effects { agent.act.semantic }
{
try invoke(@target.sample.pulse, "action.inspect.pulse")
}
"#,
        &project,
    )
    .expect("projected image action typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
}

#[test]
fn compile_agent_source_with_project_rejects_unknown_invoke_action() {
    let project = project_with_agent_action("activity.inventory", EntityKind::Activity, "open", []);
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
invoke(@activity.inventory, .close)
}
",
        &project,
    )
    .expect_err("unknown action rejects");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_rejects_unknown_invoke_arg() {
    let project = project_with_agent_action(
        "activity.inventory",
        EntityKind::Activity,
        "open",
        [AgentActionParam::required("label", TypeKind::String)],
    );
    let error = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
invoke(@activity.inventory, .open, { title = "main" })
}
"#,
        &project,
    )
    .expect_err("unknown invoke arg rejects");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_rejects_missing_invoke_arg() {
    let project = project_with_agent_action(
        "activity.inventory",
        EntityKind::Activity,
        "open",
        [AgentActionParam::required("label", TypeKind::String)],
    );
    let error = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
invoke(@activity.inventory, .open)
}
",
        &project,
    )
    .expect_err("missing required invoke arg rejects");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_rejects_invoke_arg_type_mismatch() {
    let project = project_with_agent_action(
        "activity.inventory",
        EntityKind::Activity,
        "open",
        [AgentActionParam::required("index", TypeKind::U32)],
    );
    let error = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.open_inventory open_inventory()
effects { agent.act.semantic }
{
invoke(@activity.inventory, .open, { index = "main" })
}
"#,
        &project,
    )
    .expect_err("invoke arg type mismatch rejects");

    assert!(matches!(error, CompileAgentError::Type(_)));
}

#[test]
fn compile_agent_source_with_project_checks_capture_and_debug_record() {
    let project = project_with_entity("layer.hud", EntityKind::Layer);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.capture_hud capture_hud()
effects { agent.capture, debug.record }
{
let shot = try capture(layer(@layer.hud), format = .png, name = "hud")
attach(shot)
checkpoint("after-capture")
note(fmt("captured"))
}
"#,
        &project,
    )
    .expect("capture and debug record intrinsics typecheck");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
}

#[test]
fn compile_agent_source_with_project_infers_capture_without_source_bound() {
    let compiled = compile_agent_source_with_project(
        r"
#[agent(version = 1)]
agent @agent.capture_view capture_view()
{
capture(viewport())
}
",
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("capture effect is inferred without a source upper bound");

    let summary = compiled
        .typecheck_report
        .effects
        .summary(&arcweft_lang_sema::effect_model::CallableId::new(
            "agent.capture_view",
        ))
        .expect("agent effect summary");
    assert!(summary.declared().is_none());
    assert!(summary.inferred().contains(
        &arcweft_lang_sema::effects::EffectId::parse("agent.capture").expect("valid effect")
    ));
}

#[test]
fn compile_agent_source_with_project_checks_read_resource() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.read_resource read_resource_smoke()
effects { agent.resource.read }
{
let resource = try read_resource("arcweft://session/cli/observation/latest.json")
return resource.body.json
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("read_resource intrinsic typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentResource)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::String)
    );
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentResourceBody)
    );
}

#[test]
fn compile_agent_source_with_project_checks_read_resource_body_value() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.read_resource_value read_resource_value_smoke()
effects { agent.resource.read }
{
let resource = try read_resource("arcweft://session/cli/observation/latest.json")
return resource.body.value
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("read_resource body.value typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::AgentValue)
    );
}

#[test]
fn compile_agent_source_with_project_checks_attach_resource() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.attach_resource attach_resource_smoke()
effects { agent.resource.read, debug.record }
{
let resource = try read_resource("arcweft://session/cli/observation/latest.json")
attach(resource)
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("attach accepts AgentResource values");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
}

#[test]
fn compile_agent_source_with_project_infers_read_resource_without_source_bound() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.read_resource read_resource_smoke()
{
read_resource(uri = "arcweft://session/cli/observation/latest.json")
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("read_resource effect is inferred without a source upper bound");

    let summary = compiled
        .typecheck_report
        .effects
        .summary(&arcweft_lang_sema::effect_model::CallableId::new(
            "agent.read_resource_smoke",
        ))
        .expect("agent effect summary");
    assert!(summary.declared().is_none());
    assert!(summary.inferred().contains(
        &arcweft_lang_sema::effects::EffectId::parse("agent.resource.read").expect("valid effect")
    ));
}

#[test]
fn compile_agent_source_with_project_checks_rag_query() {
    let project = project_with_entity("choice.opening.listen", EntityKind::ChoiceOption);
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_context debug_context()
effects { rag.query }
{
try rag.query(
    "opening choice recent failures",
    roots = [@choice.opening.listen],
    graph_depth = 2u32,
    limit = 8usize,
)
}
"#,
        &project,
    )
    .expect("rag.query intrinsic typechecks");

    assert!(compiled.typecheck_report.diagnostics.is_empty());
    assert!(
        compiled
            .typecheck_report
            .judgments
            .iter()
            .any(|judgment| judgment.ty == TypeKind::RagContextPack)
    );
}

#[test]
fn compile_agent_source_with_project_infers_rag_query_effect_without_source_bound() {
    let compiled = compile_agent_source_with_project(
        r#"
#[agent(version = 1)]
agent @agent.debug_context debug_context()
{
rag.query("opening choice recent failures")
}
"#,
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("rag.query effect is inferred without a source upper bound");

    let summary = compiled
        .typecheck_report
        .effects
        .summary(&arcweft_lang_sema::effect_model::CallableId::new(
            "agent.debug_context",
        ))
        .expect("agent effect summary");
    assert!(summary.declared().is_none());
    assert!(summary.inferred().contains(
        &arcweft_lang_sema::effects::EffectId::parse("rag.query").expect("valid effect")
    ));
}

#[test]
fn compile_agent_bundle_with_project_builds_agent_controller_bundle() {
    let compiled = compile_agent_bundle_with_project(
        r"
#[agent(version = 1)]
agent @agent.observe_smoke observe_smoke()
effects { agent.observe }
{
observe()
}
",
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("agent bundle compiles");

    assert_eq!(compiled.bundle.bundle_kind, BundleKind::AgentController);
    assert_eq!(compiled.manifest.agent_id.as_str(), "agent.observe_smoke");
    assert_eq!(
        compiled
            .manifest
            .declared_effects
            .iter()
            .map(AgentEffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.observe"]
    );
    assert_eq!(compiled.manifest.verified_effects.analysis_version, 1);
    assert_eq!(
        compiled
            .manifest
            .verified_effects
            .declared
            .iter()
            .map(AgentEffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.observe"]
    );
    assert_eq!(
        compiled
            .manifest
            .verified_effects
            .inferred
            .iter()
            .map(AgentEffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.observe"]
    );
    assert!(
        compiled
            .manifest
            .verified_effects
            .digest
            .as_str()
            .starts_with("blake3:")
    );
    assert_eq!(compiled.bundle.bytecode.program.flows.len(), 1);
    assert!(
        compiled.bundle.manifest.runtime.bytecode_instructions > 0,
        "Agent body should lower into bytecode operations"
    );

    let bytes = compiled.bundle.to_json_bytes().expect("bundle encodes");
    let decoded = arcweft_bundle::ArcweftBundle::from_json_slice(&bytes).expect("bundle decodes");

    assert_eq!(decoded.bundle_kind, BundleKind::AgentController);
    assert_eq!(
        decoded.agent.as_ref().map(|agent| agent.agent_id.as_str()),
        Some("agent.observe_smoke")
    );
    assert_eq!(
        decoded.agent.as_ref().map(|agent| &agent.verified_effects),
        Some(&compiled.manifest.verified_effects)
    );
}

#[test]
fn compile_agent_bundle_lowers_inferred_effects_not_unused_source_upper_bound() {
    let compiled = compile_agent_bundle_with_project(
        r"
#[agent(version = 1)]
agent @agent.observe_smoke observe_smoke()
effects { agent.observe, agent.capture }
{
    observe()
}
",
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("agent bundle compiles");

    let manifest_effects = compiled
        .manifest
        .declared_effects
        .iter()
        .map(AgentEffectCapability::as_str)
        .collect::<Vec<_>>();
    assert_eq!(manifest_effects, vec!["agent.observe"]);
    assert_eq!(
        compiled
            .manifest
            .verified_effects
            .declared
            .iter()
            .map(AgentEffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.observe"]
    );
    assert_eq!(
        compiled
            .manifest
            .verified_effects
            .inferred
            .iter()
            .map(AgentEffectCapability::as_str)
            .collect::<Vec<_>>(),
        vec!["agent.observe"]
    );
}

#[test]
fn compile_agent_bundle_with_project_records_required_entity_source_anchors() {
    let tree = parse_source(
        r#"
signal @signal.current_flow: Watch<Ref<Flow>>
flow @flow.opening opening {
return "ok"
}
"#,
    )
    .into_typed_tree();
    let hir = lower_to_hir(&tree).expect("source lowers to HIR");
    let project = project_semantic_index_from_hir(
        &hir,
        ProgramHash::new("program-test"),
        &SourceName::path("game.arcw"),
    )
    .expect("project indexes HIR entities");
    let compiled = compile_agent_bundle_with_project(
        r"
#[agent(version = 1)]
agent @agent.observe_smoke observe_smoke()
effects { agent.observe }
{
observe()
}
",
        &project,
    )
    .expect("agent bundle compiles with project entities");

    let flow = compiled
        .manifest
        .project_binding
        .required_entities
        .iter()
        .find(|entity| entity.public_id.as_str() == "flow.opening")
        .expect("flow entity is recorded in project binding");
    let source_anchor = flow
        .source_anchor
        .as_ref()
        .expect("HIR-derived entity carries a source anchor");

    assert_eq!(flow.semantic_hash.as_str(), "hir:flow:flow.opening:_");
    assert_eq!(source_anchor.path, "game.arcw");
    assert!(
        source_anchor.start_byte < source_anchor.end_byte,
        "entity source anchor should preserve a non-empty byte range"
    );
}

#[test]
fn compile_agent_bundle_with_project_preserves_budget_attribute() {
    let compiled = compile_agent_bundle_with_project(
        r"
#[agent(version = 1)]
#[budget(
timeout = 20s,
steps = 96usize,
host_calls = 9usize,
observations = 8usize,
captures = 3usize,
rag_queries = 4usize,
stored_bytes = 12_345u64,
context_bytes = 4_096u64,
)]
agent @agent.budget_smoke budget_smoke()
effects { agent.observe }
{
observe()
}
",
        &ProjectSemanticIndex::new(ProgramHash::new("program-test")),
    )
    .expect("agent bundle compiles");

    assert_eq!(compiled.manifest.budget.logical_timeout_millis, 20_000);
    assert_eq!(compiled.manifest.budget.max_vm_steps, 96);
    assert_eq!(compiled.manifest.budget.max_host_calls, 9);
    assert_eq!(compiled.manifest.budget.max_observations, 8);
    assert_eq!(compiled.manifest.budget.max_captures, 3);
    assert_eq!(compiled.manifest.budget.max_rag_queries, 4);
    assert_eq!(compiled.manifest.budget.max_capture_bytes, 12_345);
    assert_eq!(compiled.manifest.budget.max_context_bytes, 4_096);
}

#[test]
fn lower_source_runtime_plan_with_options_applies_dialogue_defaults_profile() {
    let parsed = parse_source_text(
        r##"
pub dialogue defaults @dialogue.defaults {
text_color = rgb("#101112")
}

pub dialogue defaults @dialogue:.defaults.mobile {
text_color = rgb("#202122")
}

character @character.alice Alice as alice {}

flow @flow.main main {
alice: Hello[p]
}
"##,
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");
    validate_hir_with_env(&hir, &TypeCheckEnv::standard()).expect("fixture typechecks");

    let report = lower_source_runtime_plan_with_stats_and_options(
        &hir,
        &RuntimePlanLowerOptions::default().with_dialogue_defaults("dialogue.defaults.mobile"),
    )
    .expect("runtime plan lowers with selected dialogue defaults");
    let spec = report
        .line_display_catalog
        .lines()
        .first()
        .expect("line display spec");

    assert_eq!(
        spec.base_styles,
        vec![RichTextStyle::Color {
            value: RichTextColor::Rgb {
                red: 32,
                green: 33,
                blue: 34
            }
        }]
    );
}

#[test]
fn lower_source_text_pure_helper_candidates_classifies_renderer_extensions() {
    let parsed = parse_source_text(
        r"
#[text_shader]
#[pure]
fn glow(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph + seed
}

#[text_effect]
#[pure]
fn jitter(t: f32, glyph: f32, seed: f32) -> f32 {
return t - glyph + seed
}

#[text_motion]
#[pure]
fn orbit(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph * seed
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");

    let report =
        lower_source_text_pure_helper_candidates(&hir).expect("text renderer helpers lower");

    assert_eq!(report.shaders[0].name(), "glow");
    assert_eq!(report.effects[0].name(), "jitter");
    assert_eq!(report.motions[0].name(), "orbit");
}

#[test]
fn lower_source_text_pure_helper_candidates_rejects_unpure_exports() {
    let parsed = parse_source_text(
        r"
#[text_effect]
fn drift(t: f32, glyph: f32, seed: f32) -> f32 {
return t + glyph + seed
}
",
    );
    let hir = lower_source_tree(parsed.typed_tree()).expect("fixture lowers");

    assert_eq!(
        lower_source_text_pure_helper_candidates(&hir),
        Err(vec![TextPureHelperCandidateError::MissingPureAttribute {
            kind: TextPureHelperKind::Effect,
            name: "drift".to_owned(),
        }])
    );
}
