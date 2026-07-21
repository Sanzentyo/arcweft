use arcweft_core::{
    effect::{
        LineEffectRequest, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall,
        RuntimeEffectExpr, RuntimeLog, RuntimeWaitTarget,
    },
    line_task::{LineChildTask, LineOutRequest, LineTaskNode, LineTaskTrigger},
    pattern::RuntimePattern,
    plan::{FlowOp, FlowRuntimeId, RuntimeEntryTarget, RuntimeIteratorEvidence},
    source::{SourceHandlerPlan, SourceOp},
    stream::StreamOp,
    time::LogicalDuration,
    value::{RuntimeCallTarget, RuntimeExpr, RuntimeValue},
};
use arcweft_dialogue::{DialoguePresentationProfile, DialogueProfileRevision};
use arcweft_lang_hir::{
    lower::{lower_document_to_hir, lower_to_hir},
    model::HirModule,
};
use arcweft_lang_sema::{
    check::{
        ForIterationEvidenceFamily, StandardIteratorFamily, analyze_types, validate_typecheck_ready,
    },
    env::TypeCheckEnv,
};
use arcweft_lang_syntax::{
    ast::items::TypedSyntaxTree,
    expr::{Expr, parse_expr},
    parser::parse_source,
};
use arcweft_resource_model::registry::ResourceTypeRegistry;
use arcweft_runtime_plan::{
    assertion::RuntimeAssertionBuildProfile,
    errors::{RuntimeHostRequestArgument, RuntimePlanLowerContext, RuntimePlanLowerErrorKind},
    flow::{
        RuntimePlanLowerOptions, lower_runtime_plan as lower_runtime_plan_admitted,
        lower_runtime_plan_with_stats as lower_runtime_plan_with_stats_admitted,
    },
    line_task::lower_line_task_groups,
};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName, SourceSetRevision};
use arcweft_view::{AcceptedViewProgramRevision, ViewProgramId};

fn admitted_options(
    base: RuntimePlanLowerOptions,
) -> arcweft_runtime_plan::flow::AdmittedRuntimePlanLowerOptions {
    base.with_dialogue_profile(
        DialoguePresentationProfile::engine_default(),
        test_dialogue_revision(),
    )
}

fn lower_runtime_plan(
    hir: &HirModule,
) -> Result<arcweft_core::plan::RuntimePlan, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>>
{
    lower_runtime_plan_admitted(hir, &admitted_options(RuntimePlanLowerOptions::default()))
}

fn lower_runtime_plan_with_options(
    hir: &HirModule,
    options: &RuntimePlanLowerOptions,
) -> Result<arcweft_core::plan::RuntimePlan, Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>>
{
    lower_runtime_plan_admitted(hir, &admitted_options(options.clone()))
}

fn lower_runtime_plan_with_stats(
    hir: &HirModule,
) -> Result<
    arcweft_runtime_plan::flow::RuntimePlanLowerReport,
    Vec<arcweft_runtime_plan::errors::RuntimePlanLowerError>,
> {
    lower_runtime_plan_with_stats_admitted(
        hir,
        &admitted_options(RuntimePlanLowerOptions::default()),
    )
}

fn test_dialogue_revision() -> DialogueProfileRevision {
    let source = SourceDocument::try_new(
        SourceDocumentId::try_new("runtime-plan-integration-test-revision").expect("source ID"),
        SourceName::Memory,
        "test manifest",
    )
    .expect("source document");
    let sources =
        SourceSetRevision::try_for_identities([source.identity()]).expect("source revision");
    DialogueProfileRevision::from_admitted_parts(
        source.identity().clone(),
        sources,
        sources,
        ViewProgramId::try_new("view_program.runtime-plan-integration-test")
            .expect("View program ID"),
        AcceptedViewProgramRevision::try_from_bytes([0x29; 32]).expect("View program revision"),
        ResourceTypeRegistry::empty().digest(),
    )
}

fn parse_ok(source: impl Into<String>) -> TypedSyntaxTree {
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

fn lower_bound(source: &str) -> HirModule {
    let tree = parse_ok(source);
    let document = SourceDocument::try_new(
        SourceDocumentId::try_new("arcweft-test://runtime-plan/source").expect("document ID"),
        SourceName::Generated,
        source,
    )
    .expect("source document");
    lower_document_to_hir(&document, &tree).expect("revision-bound source lowers to HIR")
}

fn call(callee: &str, args: &[&str]) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: callee.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    })
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn seq(node: &LineTaskNode) -> &[LineTaskNode] {
    match node {
        LineTaskNode::Seq(nodes) => nodes,
        other => panic!("expected seq node, got {other:?}"),
    }
}

fn direct_effects(nodes: &[LineTaskNode]) -> Vec<&LineEffectRequest> {
    nodes
        .iter()
        .filter_map(|node| match node {
            LineTaskNode::Effect(effect) => Some(effect),
            _ => None,
        })
        .collect()
}

fn direct_children(nodes: &[LineTaskNode]) -> Vec<&LineChildTask> {
    nodes
        .iter()
        .filter_map(|node| match node {
            LineTaskNode::Child(task) => Some(task),
            _ => None,
        })
        .collect()
}

#[test]
fn receive_action_lowers_to_view_action_host_call() {
    let tree = parse_ok(
        r"
pub action feedback.submit(value: String)

flow test {
  let event = receive action(@action:.feedback.submit)
  return event.value
}
",
    );
    let hir = lower_to_hir(&tree).expect("HIR lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let flow = &plan.flows[0];

    let FlowOp::HostCall {
        binding: Some(RuntimePattern::Ident(binding)),
        target,
    } = &flow.ops[0]
    else {
        panic!("expected receive action host call");
    };

    assert_eq!(binding, "event");
    assert_eq!(target.public_id, "view.action.await");
    assert_eq!(target.capability, "view.action");
    assert_eq!(target.operation, "await");
    assert_eq!(
        target.args,
        vec![RuntimeExpr::EntityRef("action.feedback.submit".to_owned())]
    );
}

#[test]
fn receive_action_inside_let_block_lowers_to_scope_value() {
    let tree = parse_ok(
        r"
pub action feedback.submit(value: String)

flow test {
  let submitted = {
    let event = receive action(@action:.feedback.submit)
    event.value
  }
  return submitted
}
",
    );
    let hir = lower_to_hir(&tree).expect("HIR lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let flow = &plan.flows[0];

    let FlowOp::LetScope {
        pattern,
        ops,
        value,
    } = &flow.ops[0]
    else {
        panic!("expected block let to lower to a flow scope");
    };

    assert_eq!(pattern, &RuntimePattern::Ident("submitted".to_owned()));
    let FlowOp::HostCall {
        binding: Some(RuntimePattern::Ident(binding)),
        target,
    } = &ops[0]
    else {
        panic!("expected receive action inside scope body");
    };
    assert_eq!(binding, "event");
    assert_eq!(target.public_id, "view.action.await");
    assert!(matches!(
        value,
        RuntimeExpr::Field { target, field }
            if field == "value"
                && matches!(target.as_ref(), RuntimeExpr::Local(name) if name == "event")
    ));
    assert!(matches!(
        &flow.ops[1],
        FlowOp::ReturnExpr(RuntimeExpr::Local(name)) if name == "submitted"
    ));
}

#[test]
fn receive_action_inside_computation_let_block_lowers_to_scope_value() {
    let tree = parse_ok(
        r"
pub action feedback.submit(value: String)

flow test {
  let submitted = result {
    let event = receive action(@action:.feedback.submit)
    event.value
  }
  return submitted
}
",
    );
    let hir = lower_to_hir(&tree).expect("HIR lowers");
    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");
    let flow = &plan.flows[0];

    let FlowOp::LetScope {
        pattern,
        ops,
        value,
    } = &flow.ops[0]
    else {
        panic!("expected computation block let to lower to a flow scope");
    };

    assert_eq!(pattern, &RuntimePattern::Ident("submitted".to_owned()));
    assert!(matches!(
        &ops[0],
        FlowOp::HostCall {
            binding: Some(RuntimePattern::Ident(binding)),
            target,
        } if binding == "event" && target.public_id == "view.action.await"
    ));
    assert!(matches!(
        value,
        RuntimeExpr::Field { target, field }
            if field == "value"
                && matches!(target.as_ref(), RuntimeExpr::Local(name) if name == "event")
    ));
}

#[test]
fn canonical_log_signal_metric_are_ordinary_calls() {
    assert!(matches!(
        parse_expr(r#"log.info("selected {id:?}", id = selected.id)"#)
            .expect("log.info parses as ordinary expression"),
        Expr::Call(_)
    ));
    assert!(matches!(
        parse_expr("signal.set(@signal.current_flow, @flow.opening)")
            .expect("signal.set parses as ordinary expression"),
        Expr::Call(_)
    ));
    assert!(matches!(
        parse_expr("metric.set(@metric.frame_time_ms, frame_time.ms())")
            .expect("metric.set parses as ordinary expression"),
        Expr::Call(_)
    ));
}

#[test]
fn runtime_plan_lowers_pure_function_call_from_compile_gap_fixture() {
    let tree = parse_ok(
        r#"
#[pure]
fn add(a: i32, b: i32) -> i32 { a + b }

flow @flow.main main {
    let n = add(1, 2)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("pure function call lowers to HIR");

    lower_runtime_plan(&hir).expect("pure function call lowers to runtime plan");
}

#[test]
fn entry_selects_runtime_goto_flow_from_compile_gap_fixture() {
    let tree = parse_ok(
        r#"
entry cli @entry.main { goto @flow.second }
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry lowers");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers with explicit entry");
    assert!(matches!(
        &plan.entries[0].target,
        RuntimeEntryTarget::Flow(id) if id.public_label().as_str() == "flow.second"
    ));
}

#[test]
fn entry_goto_selects_runtime_flow_from_final_syntax() {
    let tree = parse_ok(
        r#"
entry cli @entry.runtime_plan {
    goto @flow.second
}
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry goto lowers");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers with goto entry");
    assert!(matches!(
        &plan.entries[0].target,
        RuntimeEntryTarget::Flow(id) if id.public_label().as_str() == "flow.second"
    ));
}

#[test]
fn lowers_dialogue_line_plan_to_core_task_group() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice(focus=.soft)[待って。[mark .release_focus][p]]
    with:
        init:
            'line.focus.main <- acquire_focus()
            defer { cleanup_init_probe() }
        thread motion:
            wait(mark(.release_focus))
            wait(0.35s)
            tick_motion()
            defer { cleanup_motion() }
        at(0.42s): alice.stage.face(worried)
        on mark(.release_focus):
            'line.focus.main |> drop
            defer { cleanup_handler() }
        defer { cleanup_line_scope() }
        defer on completed:
            cleanup_line()
            defer { cleanup_completed_probe() }
}
",
    );
    let hir = lower_to_hir(&tree).expect("runtime plan fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("runtime plan fixture is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("line task group lowers");
    assert_eq!(groups.len(), 1);
    let group = groups[0].group();
    let root = seq(&group.root.node);
    let root_effects = direct_effects(root);

    assert_eq!(
        root_effects,
        vec![&LineEffectRequest::RegisterHandle {
            key: "'line.focus.main".to_owned(),
            handle: "acquire_focus()".to_owned(),
        }]
    );
    assert_eq!(
        group.root.defer_stack,
        vec![
            vec![call("cleanup_init_probe", &[])],
            vec![call("cleanup_line_scope", &[])]
        ]
    );
    assert_eq!(
        group.root.completed_defer_stack,
        vec![vec![
            call("cleanup_line", &[]),
            call("cleanup_completed_probe", &[])
        ]]
    );

    let children = direct_children(root);
    assert_eq!(children.len(), 3);
    assert_eq!(children[0].name.as_deref(), Some("motion"));
    assert!(matches!(children[0].trigger, LineTaskTrigger::Immediate));
    assert_eq!(
        direct_effects(seq(&children[0].scope.node)),
        vec![
            &LineEffectRequest::Wait(RuntimeWaitTarget::Mark(".release_focus".to_owned())),
            &LineEffectRequest::Wait(RuntimeWaitTarget::Duration(LogicalDuration::from_nanos(
                350_000_000
            ))),
            &call("tick_motion", &[]),
        ]
    );
    assert_eq!(
        children[0].scope.defer_stack,
        vec![vec![call("cleanup_motion", &[])]]
    );
    assert_eq!(children[1].name.as_deref(), Some("at(0.42s)"));
    assert_eq!(
        children[1].trigger,
        LineTaskTrigger::Delay(LogicalDuration::from_nanos(420_000_000))
    );
    assert_eq!(
        direct_effects(seq(&children[1].scope.node)),
        vec![&call("alice.stage.face", &["worried"])]
    );
    assert_eq!(children[2].name.as_deref(), Some(".release_focus"));
    assert_eq!(
        children[2].trigger,
        LineTaskTrigger::Mark(".release_focus".to_owned())
    );
    assert_eq!(
        direct_effects(seq(&children[2].scope.node)),
        vec![&LineEffectRequest::DropHandle {
            key: "'line.focus.main".to_owned(),
        }]
    );
    assert_eq!(
        children[2].scope.defer_stack,
        vec![vec![call("cleanup_handler", &[])]]
    );
}

#[test]
fn line_plan_parser_rejects_items_outside_the_current_grammar() {
    let parsed = parse_source(
        r"
flow @flow.raw raw {
    alice[待って。[p]]
    with:
        @bad raw item
}
",
    );

    assert!(parsed.errors().iter().any(|error| {
        error
            .message()
            .contains("unexpected token after expression")
    }));
}

#[test]
fn line_plan_runtime_lowering_rejects_unlowered_semantic_items() {
    let tree = parse_ok(
        r"
flow @flow.unsupported unsupported {
    alice[待って。[p]]
    with:
        voice = auto
}
",
    );
    let hir = lower_to_hir(&tree).expect("unsupported semantic item fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("unsupported semantic item fixture is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("line option lowers to runtime IR");
    assert_eq!(groups[0].group().options[0].name, "voice");
    assert_eq!(groups[0].group().options[0].value, "auto");
}

#[test]
fn line_plan_runtime_lowering_lowers_nested_group_expressions() {
    let tree = parse_ok(
        r"
flow @flow.grouped grouped {
    alice[待って。[p]]
    with {
        start {
            together {
                cue_start()
                cue_next()
            }
        }
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("group fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("group fixture is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("grouped expressions lower");
    let [LineTaskNode::Start(start_children)] = seq(&groups[0].group().root.node) else {
        panic!("root should preserve start group");
    };
    let [
        LineTaskNode::Parallel {
            policy: _,
            children,
        },
    ] = start_children.as_slice()
    else {
        panic!("start group should contain one parallel together node");
    };
    assert_eq!(
        direct_effects(children),
        vec![&call("cue_start", &[]), &call("cue_next", &[])]
    );
}

#[test]
fn lowers_structured_log_signal_metric_and_event_effects() {
    let tree = parse_ok(
        r#"
flow @flow.effects effects {
    alice[待って。[p]]
    with:
        log.info("selected {id:?}", id = selected.id)
        signal.set(@signal.current_flow, @flow.effects)
        metric.set(@metric.frame_time_ms, frame_time.ms())
        event.emit(GameEvent.ChoiceSelected, id = @choice.opening.listen)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("effect fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("effect fixture is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("structured effects lower");
    let root = seq(&groups[0].group().root.node);
    let init = direct_effects(root);
    assert!(matches!(
        init[0],
        LineEffectRequest::Log(RuntimeLog { level, message, fields })
            if level == "info" && message == "selected {id:?}" && fields.len() == 1
    ));
    assert!(matches!(
        init[1],
        LineEffectRequest::SignalWrite(RuntimeAssignment { target, value })
            if target == "@signal.current_flow" && value == "@flow.effects"
    ));
    assert!(matches!(
        init[2],
        LineEffectRequest::MetricWrite(RuntimeAssignment { target, value })
            if target == "@metric.frame_time_ms" && value == "frame_time.ms()"
    ));
    assert!(matches!(init[3], LineEffectRequest::EmitEvent(event) if event.fields.len() == 1));
}

#[test]
fn lowers_line_plan_semantic_items_to_runtime_ir() {
    let tree = parse_ok(
        r"
flow @flow.semantic semantic {
    alice[待って。[p]]
    with:
        voice = auto
        let actor = alice.stage_handle()
        memo(.line_handles, scope=line)
        assert(actor.ready())
        cancel on input(.SkipLine) { out 'line .Skipped }
        out .Done
}
",
    );
    let hir = lower_to_hir(&tree).expect("semantic line plan lowers to HIR");
    validate_typecheck_ready(&hir).expect("semantic line plan is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("semantic line plan items lower");
    let group = groups[0].group();
    assert_eq!(group.options[0].name, "voice");
    assert_eq!(group.options[0].value, "auto");
    assert_eq!(group.bindings[0].value, "alice.stage_handle()");
    assert_eq!(group.memo[0].name, "line_handles");
    assert_eq!(group.memo[0].options[0].name, "scope");
    assert_eq!(group.memo[0].options[0].value, "line");
    assert_eq!(group.assertions[0].expr, "actor.ready()");
    assert_eq!(group.cancel_rules[0].trigger, "input .SkipLine");
    assert_eq!(
        group.cancel_rules[0].action,
        vec![LineEffectRequest::Out(LineOutRequest {
            label: Some("line".to_owned()),
            value: ".Skipped".to_owned(),
        })]
    );
    assert_eq!(
        group.out,
        vec![LineOutRequest {
            label: None,
            value: ".Done".to_owned(),
        }]
    );
}

#[test]
fn together_group_rejects_conflicting_resource_writes() {
    let tree = parse_ok(
        r"
flow @flow.conflict conflict {
    alice[待って。[p]]
    with {
        together {
            signal.set(@signal.current_flow, @flow.a)
            signal.set(@signal.current_flow, @flow.b)
        }
    }
}
",
    );
    let hir = lower_to_hir(&tree).expect("conflict fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("conflict fixture is typecheck-ready");

    let errors = lower_line_task_groups(&hir).expect_err("conflicting writes are rejected");
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("parallel resource conflict"))
    );
}

#[test]
fn together_group_allows_append_only_effects() {
    let tree = parse_ok(
        r#"
flow @flow.append append {
    alice[待って。[p]]
    with {
        together {
            log.info("left")
            event.emit(GameEvent.Left)
        }
    }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("append fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("append fixture is typecheck-ready");

    lower_line_task_groups(&hir).expect("append-only effects do not conflict");
}

#[test]
fn lowers_flow_dialogue_goto_return_to_runtime_plan() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[待って。[p]]
    with:
        out .Done
    goto @flow.next
}

flow @flow.next next {
    return Ok(FlowExit.Done)
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");

    assert!(plan.entries.is_empty());
    assert_eq!(plan.line_task_groups.len(), 1);
    assert_eq!(plan.flows.len(), 2);
    assert!(matches!(
        &plan.flows[0].ops[0],
        FlowOp::Dialogue { task_group: 0, .. }
    ));
    assert!(matches!(&plan.flows[0].ops[1], FlowOp::GotoExpr(_)));
    assert!(matches!(&plan.flows[1].ops[0], FlowOp::ReturnExpr(_)));
}

#[test]
fn lowers_dialogue_result_let_and_bound_timed_cue() {
    let tree = parse_ok(
        r#"
flow @flow.line_handles line_handles {
    let (_, cue) = alice(voice=auto)[聞いて。[p]]
    with:
        let actor = alice.stage.acquire(scope=line)
        let cue = at(0.42s):
            actor.look(.worried, crossfade=120ms)
        let voice = line.voice_handle()
        out (voice, cue)

    log.info("cue kept", cue = cue)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("dialogue result let fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("dialogue result let fixture is typecheck-ready");

    let source_groups =
        lower_line_task_groups(&hir).expect("dialogue result let line task group lowers");
    assert_eq!(source_groups.len(), 1);

    let plan = lower_runtime_plan(&hir).expect("dialogue result let lowers to runtime plan");

    assert_eq!(plan.line_task_groups.len(), 1);
    assert!(matches!(
        &plan.flows[0].ops[0],
        FlowOp::Dialogue {
            line,
            task_group: 0,
            ..
        } if line.canonical_label() == "line_handles.dialogue.0"
    ));
    assert!(matches!(
        &plan.flows[0].ops[1],
        FlowOp::Let {
            expr: RuntimeExpr::Tuple(items),
            ..
        } if items.len() == 2
    ));
    let children = direct_children(seq(&plan.line_task_groups[0].root.node));
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].name.as_deref(), Some("at(0.42s)"));
    assert_eq!(
        children[0].trigger,
        LineTaskTrigger::Delay(LogicalDuration::from_nanos(420_000_000))
    );
}

#[test]
fn runtime_plan_lowers_stream_and_source_plans_separately_from_flow_ops() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base + bonus
}

stream fn rms_level(frames: Stream<i64, String>) -> Stream<i64, String> {
    for frame in frames {
        yield score(frame, 2i64)
    }
}

pub source @source.player_mic_frames: Source<i64, String> {
    from capture.microphone(@capture.player_microphone)
    backpressure = bounded(capacity = 8, overflow = drop_oldest)
    replay = hash_only
    privacy = transient

    on item frame => yield score(frame, 2i64)
}

flow @flow.opening opening {
    return Ok(FlowExit.Done)
}
",
    );
    let hir = lower_to_hir(&tree).expect("stream/source fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("stream/source runtime plan lowers");

    assert_eq!(plan.stream_plans.len(), 1);
    assert_eq!(plan.stream_plans[0].id.canonical_label(), "rms_level");
    assert!(matches!(
        plan.stream_plans[0].ops.as_slice(),
        [StreamOp::ForNext { body, .. }]
            if matches!(body.as_slice(), [StreamOp::Yield { expr }]
                if matches!(expr, RuntimeExpr::PureCall { helper, .. } if helper.0 == 0))
    ));
    assert_eq!(plan.source_plans.len(), 1);
    assert_eq!(plan.source_plans[0].id.0, "source.player_mic_frames");
    assert!(matches!(
        plan.source_plans[0].handlers.as_slice(),
        [SourceHandlerPlan::Item {
            pattern: RuntimePattern::Ident(name),
            ops,
        }]
            if name == "frame"
                && matches!(ops.as_slice(), [SourceOp::Yield(expr)]
                    if matches!(expr, RuntimeExpr::PureCall { helper, .. } if helper.0 == 0))
    ));
}

#[test]
fn runtime_plan_rejects_unsupported_stream_statement_instead_of_noop() {
    let tree = parse_ok(
        r#"
stream fn invalid(values: Stream<i64, String>) -> Stream<i64, String> {
    log.info("not executable here")
    yield 1i64
}

flow @flow.main main {
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsupported stream fixture lowers to HIR");

    let errors = lower_runtime_plan(&hir).expect_err("unsupported stream statement is rejected");
    assert!(errors.iter().any(|error| {
        matches!(
            error.context(),
            Some(RuntimePlanLowerContext::Statement {
                owner, path, kind, ..
            })
                if owner == "stream function `invalid`"
                    && path == &["0".to_owned()]
                    && kind == "expression"
        )
    }));
}

#[test]
fn runtime_plan_rejects_discarded_stream_final_value() {
    let tree = parse_ok(
        r#"
stream fn invalid(values: Stream<i64, String>) -> Stream<i64, String> {
    log.info("must not disappear")
}

flow @flow.main main {
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("stream final-value fixture lowers to HIR");

    let errors = lower_runtime_plan(&hir).expect_err("stream final value is rejected");
    assert!(errors.iter().any(|error| {
        matches!(
            error.context(),
            Some(RuntimePlanLowerContext::Expression {
                owner,
                path,
                statement_kind,
                role,
                ..
            }) if owner == "stream function `invalid`"
                && path == &["body.value".to_owned()]
                && statement_kind == "body"
                && role == "final value"
        ) && error
            .reason()
            .contains("cannot end with a value expression")
    }));
}

#[test]
fn runtime_plan_rejects_unsupported_source_statement_instead_of_noop() {
    let tree = parse_ok(
        r#"
pub source @source.invalid: Source<i64, String> {
    from capture.microphone(@capture.player_microphone)
    backpressure = latest
    replay = none
    privacy = transient

    on item value => { let copy = value }
}

flow @flow.main main {
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsupported source fixture lowers to HIR");

    let errors = lower_runtime_plan(&hir).expect_err("unsupported source statement is rejected");
    assert!(errors.iter().any(|error| {
        matches!(
            error.context(),
            Some(RuntimePlanLowerContext::Statement {
                owner, path, kind, ..
            })
                if owner == "source `source.invalid` handler `item`"
                    && path == &["0".to_owned()]
                    && kind == "let"
        )
    }));
}

#[test]
fn stream_expression_failure_preserves_role_and_authored_range() {
    let source = r#"
stream fn invalid(values: Stream<i64, String>) -> Stream<i64, String> {
    yield await next
}

flow @flow.main main {
    return "done"
}
"#;
    let tree = parse_ok(source);
    let hir = lower_to_hir(&tree).expect("stream expression fixture lowers to HIR");

    let errors = lower_runtime_plan(&hir).expect_err("stream await expression is rejected");
    let error = errors
        .iter()
        .find(|error| {
            matches!(
                error.context(),
                Some(RuntimePlanLowerContext::Expression {
                    owner,
                    path,
                    statement_kind,
                    role,
                    ..
                }) if owner == "stream function `invalid`"
                    && path == &["0".to_owned()]
                    && statement_kind == "yield"
                    && role == "value"
            )
        })
        .expect("yield expression error retains structured context");
    let range = error
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("yield expression retains authored range");
    assert_eq!(&source[range.as_range()], "await next");
    assert!(error.reason().contains("suspension-aware"));
}

#[test]
fn source_header_expression_failure_is_not_stringified() {
    let source = r#"
pub source @source.invalid: Source<i64, String> {
    from _
    backpressure = latest
    replay = none
    privacy = transient

    on item value => yield value
}

flow @flow.main main {
    return "done"
}
"#;
    let hir = lower_bound(source);

    let errors = lower_runtime_plan(&hir).expect_err("unsupported source header is rejected");
    let error = errors
        .iter()
        .find(|error| {
            matches!(
                error.context(),
                Some(RuntimePlanLowerContext::Expression {
                    owner,
                    path,
                    statement_kind,
                    role,
                    ..
                }) if owner == "source `source.invalid`"
                    && path == &["header.from".to_owned()]
                    && statement_kind == "header"
                    && role == "from"
            ) && error
                .reason()
                .contains("partial placeholder is outside a runtime binding scope")
        })
        .expect("source from error retains structured context");
    let range = error
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("source from error retains authored range");
    assert_eq!(&source[range.as_range()], "_");
    assert_eq!(
        error
            .diagnostic()
            .span()
            .expect("diagnostic projects the authored range")
            .range()
            .as_range(),
        range.as_range()
    );
}

#[test]
fn source_handler_expression_failure_retains_absolute_authored_range() {
    let source = r#"
pub source @source.invalid: Source<i64, String> {
    from capture.values()
    backpressure = latest
    replay = none
    privacy = transient

    on item value => {
        yield await next
    }
}

flow @flow.main main {
    return "done"
}
"#;
    let hir = lower_bound(source);

    let errors = lower_runtime_plan(&hir).expect_err("source handler await is rejected");
    let error = errors
        .iter()
        .find(|error| {
            matches!(
                error.context(),
                Some(RuntimePlanLowerContext::Expression {
                    owner,
                    path,
                    statement_kind,
                    role,
                    ..
                }) if owner == "source `source.invalid` handler `item`"
                    && path == &["0".to_owned()]
                    && statement_kind == "yield"
                    && role == "value"
            )
        })
        .expect("source handler failure retains structured context");
    let range = error
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("source handler failure retains authored range");
    assert_eq!(&source[range.as_range()], "await next");
    assert_eq!(
        error
            .diagnostic()
            .span()
            .expect("diagnostic projects the handler range")
            .range()
            .as_range(),
        range.as_range()
    );
}

#[test]
fn source_bounded_policy_distinguishes_missing_and_unknown_overflow() {
    let missing_source = r#"
pub source @source.missing: Source<i64, String> {
    from capture.values()
    backpressure = bounded(capacity = 8)
    replay = none
    privacy = transient
}

flow @flow.main main { return "done" }
"#;
    let missing_hir = lower_to_hir(&parse_ok(missing_source)).expect("missing fixture lowers");
    let missing_errors =
        lower_runtime_plan(&missing_hir).expect_err("missing overflow is rejected");
    let missing = missing_errors
        .iter()
        .find(|error| error.reason().contains("requires an `overflow` option"))
        .expect("missing overflow has a dedicated diagnostic");
    let missing_range = missing
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("missing overflow anchors to the bounded policy");
    assert_eq!(
        &missing_source[missing_range.as_range()],
        "bounded(capacity = 8)"
    );

    let unknown_source = r#"
pub source @source.unknown: Source<i64, String> {
    from capture.values()
    backpressure = bounded(capacity = 8, overflow = legacy)
    replay = none
    privacy = transient
}

flow @flow.main main { return "done" }
"#;
    let unknown_hir = lower_to_hir(&parse_ok(unknown_source)).expect("unknown fixture lowers");
    let unknown_errors =
        lower_runtime_plan(&unknown_hir).expect_err("unknown overflow is rejected");
    let unknown = unknown_errors
        .iter()
        .find(|error| {
            error
                .reason()
                .contains("unknown source overflow policy `legacy`")
        })
        .expect("unknown overflow has a spelling diagnostic");
    let unknown_range = unknown
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("unknown overflow retains the authored value range");
    assert_eq!(&unknown_source[unknown_range.as_range()], "legacy");
}

#[test]
fn source_duplicate_header_is_rejected_at_second_value() {
    let source = r#"
pub source @source.duplicate: Source<i64, String> {
    from capture.values()
    backpressure = latest
    replay = none
    privacy = transient
    privacy = recordable
}

flow @flow.main main { return "done" }
"#;
    let hir = lower_to_hir(&parse_ok(source)).expect("duplicate source fixture lowers");

    let errors = lower_runtime_plan(&hir).expect_err("duplicate header is rejected");
    let duplicate = errors
        .iter()
        .find(|error| {
            error
                .reason()
                .contains("source header `privacy` may appear only once")
        })
        .expect("duplicate header has a structured diagnostic");
    let range = duplicate
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("duplicate header retains the second value range");
    assert_eq!(&source[range.as_range()], "recordable");
}

#[test]
fn source_runtime_policy_rejects_private_full_replay() {
    let source = r#"
pub source @source.private: Source<i64, String> {
    from capture.values()
    backpressure = latest
    replay = full
    privacy = private
}

flow @flow.main main { return "done" }
"#;
    let hir = lower_to_hir(&parse_ok(source)).expect("private source fixture lowers");

    let errors = lower_runtime_plan(&hir).expect_err("private full replay is rejected");
    let incompatible = errors
        .iter()
        .find(|error| {
            error
                .reason()
                .contains("`privacy = private` is incompatible with `replay = full`")
        })
        .expect("runtime policy boundary rechecks privacy/replay compatibility");
    let range = incompatible
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("incompatibility anchors to the privacy policy");
    assert_eq!(&source[range.as_range()], "private");
}

#[test]
fn stream_unit_return_remains_executable() {
    let tree = parse_ok(
        r#"
stream fn finite(values: Stream<i64, String>) -> Stream<i64, String> {
    return ()
}

flow @flow.main main {
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("stream return fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("unit stream return lowers");
    assert!(matches!(
        plan.stream_plans[0].ops.as_slice(),
        [StreamOp::Return]
    ));
}

#[test]
fn runtime_plan_lowers_range_for_source_as_runtime_range_expr() {
    let tree = parse_ok(
        r"
flow @flow.range range {
    let a = 2
    for i in 0..a {
        log.info(i)
    }
    return a
}
",
    );
    let hir = lower_to_hir(&tree).expect("range for fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("range for fixture is typecheck-ready");
    let typecheck = analyze_types(&hir, &TypeCheckEnv::standard());
    assert!(
        typecheck.diagnostics.is_empty(),
        "{:?}",
        typecheck.diagnostics
    );
    assert_eq!(
        typecheck
            .for_iteration_evidence
            .first()
            .map(|evidence| &evidence.family),
        Some(&ForIterationEvidenceFamily::Builtin(
            StandardIteratorFamily::Range
        ))
    );

    let plan = lower_runtime_plan_with_options(
        &hir,
        &RuntimePlanLowerOptions::default()
            .with_for_iteration_evidence([RuntimeIteratorEvidence::builtin_range()]),
    )
    .expect("range for runtime plan lowers");

    let FlowOp::For { source, .. } = &plan.flows[0].ops[1] else {
        panic!(
            "expected second op to be for, got {:?}",
            plan.flows[0].ops[1]
        );
    };
    let RuntimeExpr::Range {
        start,
        end,
        inclusive,
    } = source
    else {
        panic!("expected range source, got {source:?}");
    };
    assert!(!inclusive);
    assert!(matches!(
        start.as_deref(),
        Some(RuntimeExpr::Value(RuntimeValue::Int(value))) if value.exact_i32() == Some(0)
    ));
    assert!(matches!(end.as_deref(), Some(RuntimeExpr::Local(name)) if name == "a"));
}

#[test]
fn line_plan_runtime_lowering_rejects_yield_effect() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    alice[待って。[p]]
    with:
        yield .Done
}
",
    );
    let hir = lower_to_hir(&tree).expect("line-plan yield fixture lowers to HIR");

    let errors = lower_line_task_groups(&hir).expect_err("line-plan yield is not a core effect");
    assert!(errors.iter().any(|error| {
        error
            .message()
            .contains("`yield` cannot be lowered from a dialogue line plan")
    }));
}

#[test]
fn lowers_choice_and_await_to_runtime_plan() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    try await load_opening_assets() with { pending p => progress.set(p.ratio) }
    choice @choice.opening.first {
        @choice.opening.listen "聞いてみる" -> @flow.alice_intro
    }
}

flow @flow.alice_intro alice_intro {
    return Ok(FlowExit.Done)
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("choice-await fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("choice-await runtime plan lowers");

    assert!(matches!(&plan.flows[0].ops[0], FlowOp::Await { pending, .. } if pending.len() == 1));
    let FlowOp::Choice { id, options } = &plan.flows[0].ops[1] else {
        panic!("expected choice op");
    };
    assert_eq!(id.as_deref(), Some("choice.opening.first"));
    assert_eq!(options[0].id.as_deref(), Some("choice.opening.listen"));
    assert_eq!(options[0].target, Some(flow_id("flow.alice_intro")));
}

#[test]
fn runtime_plan_lowers_await_calls_to_typed_host_requests() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    try await fs.read_text("game/config.arcw") with { pending p => progress.set(p.ratio) }
    try await http.fetch("https://example.invalid/api", method = "POST", body = "payload") with { pending p => progress.set(p.ratio) }
    try await asset.image(@asset:.bg.room) with { pending p => progress.set(p.ratio) }
    try await shader.compile(@shader.fade, entry = "main") with { pending p => progress.set(p.ratio) }
    try await audio.decode(@voice.alice.opening) with { pending p => progress.set(p.ratio) }
    try await tts.synthesize("hello", voice = "alice") with { pending p => progress.set(p.ratio) }
    try await process.run("arcw", args = ["check", "game.arcw"]) with { pending p => progress.set(p.ratio) }
    try await wasm.call("module", "function", 1) with { pending p => progress.set(p.ratio) }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("typed host request fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("typed host requests lower");
    let requests = plan.flows[0]
        .ops
        .iter()
        .map(|op| match op {
            FlowOp::Await { target, .. } => &target.request,
            other => panic!("expected await op, got {other:?}"),
        })
        .collect::<Vec<_>>();

    let expected = [
        ("fs", "read_text", 1),
        ("http", "fetch", 3),
        ("asset", "image", 1),
        ("shader", "compile", 2),
        ("audio", "decode", 1),
        ("tts", "synthesize", 2),
        ("process", "run", 2),
        ("wasm", "call", 3),
    ];
    for (request, (capability, operation, args)) in requests.iter().zip(expected) {
        assert_eq!(request.capability.0, capability);
        assert_eq!(request.operation, operation);
        assert_eq!(request.args.len(), args);
    }
    assert_eq!(requests[1].args[1].name(), Some("method"));
    assert_eq!(requests[1].args[2].name(), Some("body"));
    assert_eq!(requests[6].args[1].name(), Some("args"));
}

#[test]
fn runtime_plan_normalizes_family_relative_asset_call_args() {
    let tree = parse_ok(
        r"
flow @flow.opening opening {
    let room = bg(@asset:.room)
}
",
    );
    let hir = lower_to_hir(&tree).expect("family-relative asset call lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("family-relative asset call lowers");
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected let op");
    };
    let RuntimeExpr::Call { args, .. } = expr else {
        panic!("expected runtime call");
    };

    assert_eq!(args, &[RuntimeExpr::EntityRef("asset.room".to_owned())]);
}

#[test]
fn runtime_plan_preserves_host_request_spread_args() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    let args = ["save/out.txt", "hello"]
    try await fs.write(args...) with { pending p => progress.set(p.ratio) }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("spread host request fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("spread host request lowers");
    let FlowOp::Await { target, .. } = &plan.flows[0].ops[1] else {
        panic!("expected await op");
    };

    assert_eq!(target.request.capability.0, "fs");
    assert_eq!(target.request.operation, "write");
    assert_eq!(target.request.args.len(), 1);
    assert!(target.request.args[0].is_spread());
}

#[test]
fn host_request_failure_retains_flow_owner_path_and_await_target_range() {
    let source = r"
flow @flow.loading loading {
    try await fs.read_text(try next) with { pending p => progress.set(p.ratio) }
}
";
    let hir = lower_bound(source);

    let errors = lower_runtime_plan(&hir).expect_err("try argument is not a pure host value");
    let error = errors
        .iter()
        .find(|error| {
            matches!(
                error.context(),
                Some(RuntimePlanLowerContext::HostRequestArgument {
                    owner,
                    path,
                    capability,
                    operation,
                    argument: RuntimeHostRequestArgument::Positional(0),
                    ..
                }) if owner == "flow `loading`"
                    && path == &["0".to_owned()]
                    && capability == "fs"
                    && operation == "read_text"
            )
        })
        .expect("host argument error retains structured flow context");
    let range = error
        .context()
        .and_then(RuntimePlanLowerContext::source_range)
        .expect("host argument error retains the authored await target range");
    assert_eq!(&source[range.as_range()], "fs.read_text(try next)");
    assert_eq!(
        error
            .diagnostic()
            .span()
            .expect("host diagnostic projects the authored range")
            .range()
            .as_range(),
        range.as_range()
    );
}

#[test]
fn let_await_failure_retains_binding_rhs_range() {
    let source = r"
flow @flow.loading loading {
    let contents = try await fs.read_text(try next) with { pending p => progress.set(p.ratio) }
}
";
    let hir = lower_to_hir(&parse_ok(source)).expect("invalid let-await fixture lowers");

    let errors = lower_runtime_plan(&hir).expect_err("try argument is not a pure host value");
    let range = errors
        .iter()
        .find_map(|error| match error.context() {
            Some(RuntimePlanLowerContext::HostRequestArgument {
                source_range: Some(range),
                ..
            }) => Some(*range),
            _ => None,
        })
        .expect("let-await host failure retains its binding RHS range");
    assert_eq!(&source[range.as_range()], "fs.read_text(try next)");
}

#[test]
fn runtime_plan_lowers_traverse_parallel_to_await_many() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    let paths = [path.save("a.txt"), path.save("b.txt"), path.save("c.txt")]
    let values = try await paths.traverse(fs.read_text).parallel(limit = 2) with { pending p => progress.set(p.ratio) }
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("traverse parallel fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("traverse parallel runtime plan lowers");
    let FlowOp::AwaitMany {
        binding,
        target,
        pending,
    } = &plan.flows[0].ops[1]
    else {
        panic!("expected await many op");
    };

    assert!(binding.is_some());
    assert_eq!(target.limit, 2);
    assert_eq!(target.request.capability.0, "fs");
    assert_eq!(target.request.operation, "read_text");
    assert_eq!(target.request.args.len(), 1);
    assert_eq!(pending.len(), 1);
}

#[test]
fn runtime_plan_lowering_preserves_let_and_dynamic_goto() {
    let tree = parse_ok(
        r"
flow @flow.typed typed {
    let route = @flow.next
    goto route
}

flow @flow.next next {
    return Ok(FlowExit.Done)
}
",
    );
    let hir = lower_to_hir(&tree).expect("runtime fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("typed runtime fixture lowers");

    assert!(matches!(&plan.flows[0].ops[0], FlowOp::Let { .. }));
    assert!(matches!(&plan.flows[0].ops[1], FlowOp::GotoExpr(_)));
}

#[test]
fn runtime_plan_rewrites_pure_function_calls_to_typed_pure_call() {
    let tree = parse_ok(
        r#"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    if base >= 3 { base * add(bonus, 2) } else { 0 }
}

fn inferred(base: i64) -> i64 {
    base + 1
}

flow @flow.main main {
    let explicit = score(3, 4)
    let auto = inferred(explicit)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("pure call fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("pure call runtime plan lowers");

    assert_eq!(plan.pure_helpers.len(), 2);
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected explicit pure call let");
    };
    assert!(matches!(expr, RuntimeExpr::PureCall { helper, .. } if helper.0 == 0));
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[1] else {
        panic!("expected inferred pure call let");
    };
    assert!(matches!(expr, RuntimeExpr::PureCall { helper, .. } if helper.0 == 1));
}

#[test]
fn runtime_plan_lowers_expected_partial_placeholder_function_let() {
    let tree = parse_ok(
        r#"
flow @flow.main main {
    let high: i64 -> bool = _ > 80i64
    let high_grouped: i64 -> bool = (_ > 80i64)
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("partial function fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("partial function runtime plan lowers");

    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected partial function let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(
                            lhs.as_ref(),
                            RuntimeExpr::Local(name) if name == "__arcweft_partial"
                        )
                )
    ));
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[1] else {
        panic!("expected grouped partial function let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Function { params, body }
            if params.as_slice() == ["__arcweft_partial"]
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Binary { lhs, .. }
                        if matches!(
                            lhs.as_ref(),
                            RuntimeExpr::Local(name) if name == "__arcweft_partial"
                        )
                )
    ));
}

#[test]
fn runtime_plan_lowers_local_function_value_calls_to_apply() {
    let tree = parse_ok(
        r"
#[pure]
fn add(lhs: i64, rhs: i64) -> i64 {
    lhs + rhs
}

flow @flow.main main {
    let f = add
    let add_two = f(2i64)
    let seven = add_two(5i64)
    return seven
}
",
    );
    let hir = lower_to_hir(&tree).expect("local function fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("local function runtime plan lowers");

    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected function alias let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Function { params, .. } if params.as_slice() == ["lhs", "rhs"]
    ));
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[1] else {
        panic!("expected partial apply let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                && args.len() == 1
    ));
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[2] else {
        panic!("expected second apply let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "add_two")
                && args.len() == 1
    ));
}

#[test]
fn runtime_plan_lowers_closure_return_statement_to_function_body() {
    let tree = parse_ok(
        r"
flow @flow.main main {
    let f = || -> i64 {
        let value = 7i64
        return value
    }
    let result = f()
    return result
}
",
    );
    let hir = lower_to_hir(&tree).expect("closure return fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("closure return runtime plan lowers");

    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected closure binding");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Function { params, body }
            if params.is_empty()
                && matches!(
                    body.as_ref(),
                    RuntimeExpr::Let { name, body, .. }
                        if name == "value"
                            && matches!(
                                body.as_ref(),
                                RuntimeExpr::Local(returned) if returned == "value"
                            )
                )
    ));
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[1] else {
        panic!("expected closure call binding");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Apply { callee, args }
            if matches!(callee.as_ref(), RuntimeExpr::Local(name) if name == "f")
                && args.is_empty()
    ));
}

#[test]
fn runtime_plan_lowers_data_format_path_to_enum_variant() {
    let tree = parse_ok(
        r#"
flow @flow.main main {
    let bytes = data.encode(["hello"], .Json)
    return bytes
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("data format fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("data format runtime plan lowers");

    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected data encode let");
    };
    let RuntimeExpr::Call { callee, args } = expr else {
        panic!("expected data encode call");
    };
    assert_eq!(callee, &RuntimeCallTarget::Named("data.encode".to_owned()));
    assert!(matches!(
        args.as_slice(),
        [
            _,
            RuntimeExpr::Variant {
                path: None,
                name,
                payload: None
            }
        ] if name == "Json"
    ));
}

#[test]
fn runtime_plan_lowers_user_enum_shorthand_payloads_to_variants() {
    let tree = parse_ok(
        r#"
enum Mood {
    Alert,
    WithScore(i64),
    WithMeta { label: String },
}

flow @flow.main main {
    let mood: Mood = .Alert
    let scored: Mood = .WithScore(7i64)
    let meta: Mood = WithMeta { label = "ready" }
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("user enum shorthand fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("user enum shorthand runtime plan lowers");

    let [
        FlowOp::Let { expr: mood, .. },
        FlowOp::Let { expr: scored, .. },
        FlowOp::Let { expr: meta, .. },
    ] = plan.flows[0].ops.as_slice()
    else {
        panic!(
            "expected three enum constructor bindings, got {:?}",
            plan.flows[0].ops
        );
    };
    assert!(matches!(
        mood,
        RuntimeExpr::Variant {
            path: None,
            name,
            payload: None
        } if name == "Alert"
    ));
    assert!(matches!(
        scored,
        RuntimeExpr::Variant {
            path: None,
            name,
            payload: Some(payload)
        } if name == "WithScore"
            && matches!(
                payload.as_ref(),
                RuntimeExpr::Value(RuntimeValue::Int(value)) if value.exact_i64() == Some(7)
            )
    ));
    assert!(matches!(
        meta,
        RuntimeExpr::Variant {
            path: None,
            name,
            payload: Some(payload)
        } if name == "WithMeta"
            && matches!(
                payload.as_ref(),
                RuntimeExpr::Record(fields)
                    if fields.len() == 1
                        && fields[0].name == "label"
                        && matches!(
                            &fields[0].value,
                            RuntimeExpr::Value(RuntimeValue::String(value)) if value == "ready"
                        )
            )
    ));
}

#[test]
fn runtime_plan_fuses_unused_map_binding_into_following_sum() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.main main {
    let values: Array<i64, 4> = [1i64; 4]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    let total: i64 = scores.sum()
    return total
}
",
    );
    let hir = lower_to_hir(&tree).expect("map sum fusion fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("map sum fusion runtime plan lowers");

    assert_eq!(plan.flows[0].ops.len(), 2);
    let FlowOp::Let { expr, .. } = &plan.flows[0].ops[0] else {
        panic!("expected fused total let");
    };
    assert!(matches!(
        expr,
        RuntimeExpr::Sum { source }
            if matches!(source.as_ref(), RuntimeExpr::Map { body, .. }
                if matches!(body.as_ref(), RuntimeExpr::PureCall { helper, .. } if helper.0 == 0))
    ));
    assert!(matches!(
        expr,
        RuntimeExpr::Sum { source }
            if matches!(source.as_ref(), RuntimeExpr::Map { source, .. }
                if matches!(source.as_ref(), RuntimeExpr::RepeatSeq { len: 4, .. }))
    ));
}

#[test]
fn runtime_plan_lowering_reports_pure_and_map_sum_optimization_work() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.main main {
    let values: Array<i64, 4> = [1i64; 4]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    let total: i64 = scores.sum()
    return total
}
",
    );
    let hir = lower_to_hir(&tree).expect("map sum stats fixture lowers to HIR");

    let report = lower_runtime_plan_with_stats(&hir).expect("map sum stats runtime plan lowers");

    assert_eq!(report.stats.pure_helpers, 1);
    assert_eq!(report.stats.pure_candidate_functions_seen, 1);
    assert_eq!(report.stats.pure_candidate_lower_attempts, 1);
    assert_eq!(report.stats.pure_candidate_lower_failures_inferred, 0);
    assert!(report.stats.pure_expr_lowered_nodes >= 1);
    assert_eq!(
        report.stats.pure_expr_cloned_nodes,
        report.stats.pure_expr_lowered_nodes
    );
    assert_eq!(report.stats.pure_rewrite_expr_visits, 0);
    assert_eq!(report.stats.optimized_flows, 1);
    assert!(report.stats.optimized_op_slices >= 1);
    assert!(report.stats.local_use_tail_scans >= 1);
    assert!(report.stats.local_use_scan_ops >= 1);
    assert_eq!(report.stats.sequence_map_sum_fusions, 1);
    assert_eq!(report.stats.map_sum_fusions, 0);
    assert_eq!(report.stats.sequence_source_inlines, 0);
    assert_eq!(report.stats.pure_call_exprs, 1);
}

#[test]
fn runtime_plan_keeps_sequence_binding_when_used_after_fused_map_sum() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.main main {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    let total: i64 = scores.sum()
    let again: i64 = values.sum()
    return total + again
}
",
    );
    let hir = lower_to_hir(&tree).expect("map sum sequence reuse fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("map sum sequence reuse runtime plan lowers");

    assert_eq!(plan.flows[0].ops.len(), 4);
    assert!(matches!(
        &plan.flows[0].ops[0],
        FlowOp::Let { expr, .. }
            if matches!(
                expr,
                RuntimeExpr::RepeatSeq { len: 4, .. }
                    | RuntimeExpr::Value(RuntimeValue::Seq(_))
            )
    ));
}

#[test]
fn runtime_plan_keeps_sequence_binding_when_map_body_uses_it() {
    let tree = parse_ok(
        r"
flow @flow.main main {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| item + values.sum())
    let total: i64 = scores.sum()
    return total
}
",
    );
    let hir = lower_to_hir(&tree).expect("map body sequence use fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("map body sequence use runtime plan lowers");

    assert_eq!(plan.flows[0].ops.len(), 3);
    assert!(matches!(
        &plan.flows[0].ops[0],
        FlowOp::Let {
            expr: RuntimeExpr::Value(RuntimeValue::Seq(_)),
            ..
        }
    ));
    assert!(matches!(
        &plan.flows[0].ops[1],
        FlowOp::Let {
            expr: RuntimeExpr::Sum { source },
            ..
        } if matches!(source.as_ref(), RuntimeExpr::Map { source, .. }
            if matches!(source.as_ref(), RuntimeExpr::Local(name) if name == "values"))
    ));
}

#[test]
fn runtime_plan_keeps_map_binding_when_used_after_sum() {
    let tree = parse_ok(
        r"
#[pure]
fn score(base: i64, bonus: i64) -> i64 {
    return base * (bonus + 2i64)
}

flow @flow.main main {
    let values: Vec<i64> = [1i64, 2i64, 3i64, 4i64]
    let scores: Vec<i64> = values.map(|item| score(item, 2i64))
    let total: i64 = scores.sum()
    let again: i64 = scores.sum()
    return total + again
}
",
    );
    let hir = lower_to_hir(&tree).expect("map sum non-fusion fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("map sum non-fusion runtime plan lowers");

    assert_eq!(plan.flows[0].ops.len(), 5);
    assert!(matches!(
        &plan.flows[0].ops[1],
        FlowOp::Let {
            expr: RuntimeExpr::Map { .. },
            ..
        }
    ));
}

#[test]
fn runtime_plan_lowering_preserves_structured_if_and_match() {
    let tree = parse_ok(
        r#"
flow @flow.structured structured {
    if ready {
        goto @flow.ready
    }

    match route {
        @flow.ready => return "ready"
        _ => return "fallback"
    }
}

flow @flow.ready ready {
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("structured runtime fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("structured runtime plan lowers");

    assert!(matches!(&plan.flows[0].ops[0], FlowOp::If { .. }));
    assert!(matches!(&plan.flows[0].ops[1], FlowOp::Match { .. }));
}

#[test]
fn runtime_plan_lowering_preserves_assertion_profiles() {
    let tree = parse_ok(
        r#"
flow @flow.assertions assertions {
    ensure(route.is_some(), "route missing")
    assert(state.ready(), "state must be ready")
    debug_assert(cache.consistent())
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("assertion runtime fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("assertion fixture is typecheck-ready");

    let plan = lower_runtime_plan(&hir).expect("assertion runtime plan lowers");
    let [
        FlowOp::EvaluatedEffect(RuntimeEffectExpr::Ensure {
            condition: ensure_condition,
            message: ensure_message,
        }),
        FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
            condition: assert_condition,
            message: assert_message,
            profile: RuntimeAssertionProfile::Always,
        }),
        FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
            condition: debug_condition,
            message: debug_message,
            profile: RuntimeAssertionProfile::DebugOnly,
        }),
    ] = plan.flows[0].ops.as_slice()
    else {
        panic!("expected three assertion-related effects");
    };

    assert!(matches!(
        ensure_condition,
        RuntimeExpr::Call { .. } | RuntimeExpr::MethodCall { .. }
    ));
    assert_eq!(
        ensure_message,
        &RuntimeExpr::Value(RuntimeValue::String("route missing".to_owned()))
    );
    assert!(matches!(
        assert_condition,
        RuntimeExpr::Call { .. } | RuntimeExpr::MethodCall { .. }
    ));
    assert_eq!(
        assert_message,
        &RuntimeExpr::Value(RuntimeValue::String("state must be ready".to_owned()))
    );
    assert!(matches!(
        debug_condition,
        RuntimeExpr::Call { .. } | RuntimeExpr::MethodCall { .. }
    ));
    assert_eq!(
        debug_message,
        &RuntimeExpr::Value(RuntimeValue::String("assertion failed".to_owned()))
    );
}

#[test]
fn typed_check_assertions_lower_conditions_in_authored_order() {
    let tree = parse_ok(
        r"
flow assertions {
    assert.check(true, false, true)
}
",
    );
    let hir = lower_to_hir(&tree).expect("typed assertion fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("check assertion lowers to runtime guards");
    let profiles_and_conditions = plan.flows[0]
        .ops
        .iter()
        .map(|op| match op {
            FlowOp::EvaluatedEffect(RuntimeEffectExpr::Assert {
                condition, profile, ..
            }) => (*profile, condition),
            other => panic!("expected typed assertion guard, got {other:?}"),
        })
        .collect::<Vec<_>>();

    assert_eq!(profiles_and_conditions.len(), 3);
    assert!(
        profiles_and_conditions
            .iter()
            .all(|(profile, _)| *profile == RuntimeAssertionProfile::Always)
    );
    assert_eq!(
        profiles_and_conditions
            .iter()
            .map(|(_, condition)| *condition)
            .collect::<Vec<_>>(),
        vec![
            &RuntimeExpr::Value(RuntimeValue::Bool(true)),
            &RuntimeExpr::Value(RuntimeValue::Bool(false)),
            &RuntimeExpr::Value(RuntimeValue::Bool(true)),
        ]
    );
}

#[test]
fn release_profile_omits_typed_debug_assertion_and_condition_evaluation() {
    let tree = parse_ok(
        r#"
flow assertions {
    assert.debug(unknown.condition())
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("typed debug assertion fixture lowers to HIR");
    let options = RuntimePlanLowerOptions::new()
        .with_assertion_build_profile(RuntimeAssertionBuildProfile::Release);

    let plan = lower_runtime_plan_with_options(&hir, &options)
        .expect("release plan omits the complete debug assertion");

    assert_eq!(
        plan.flows[0].ops,
        [FlowOp::ReturnExpr(RuntimeExpr::Value(
            RuntimeValue::String("done".to_owned())
        ))]
    );
}

#[test]
fn unresolved_typed_prove_assertion_blocks_runtime_plan_with_stable_code() {
    let tree = parse_ok(
        r"
flow assertions {
    assert.prove(true)
}
",
    );
    let hir = lower_to_hir(&tree).expect("typed prove assertion fixture lowers to HIR");

    let errors = lower_runtime_plan(&hir).expect_err("unresolved prove blocks code generation");

    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].kind(), RuntimePlanLowerErrorKind::UnresolvedProof);
    assert_eq!(
        errors[0]
            .diagnostic()
            .code()
            .expect("diagnostic has a stable code")
            .as_str(),
        "verify.proof.unresolved"
    );
}

#[test]
fn audited_unsafe_lifetime_region_lowers_as_a_lexical_runtime_scope() {
    let tree = parse_ok(
        r#"
flow @flow.audit audit {
    unsafe lifetime @unsafe.cache reason = "owned clone" {
        /// SAFETY: the value is owned before promotion.
        let summary = promote_unchecked('flow)
    }
    return "done"
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("unsafe lifetime fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("audited lifetime region lowers");

    assert!(matches!(
        plan.flows[0].ops.as_slice(),
        [FlowOp::Scope(scope), FlowOp::ReturnExpr(_)]
            if matches!(scope.as_slice(), [FlowOp::Let { .. }])
    ));
}

#[test]
fn runtime_plan_lowers_audio_call_to_typed_audio_effect() {
    let tree = parse_ok(
        r"
flow @flow.audio audio {
    audio.play(@voice.opening, @asset:.voice.opening, @bus.master, fade_in_millis = 120u64)
}
",
    );
    let hir = lower_to_hir(&tree).expect("audio call fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("audio call lowers to runtime plan");
    let [FlowOp::Effect(LineEffectRequest::Audio(command))] = plan.flows[0].ops.as_slice() else {
        panic!("expected typed audio effect, got {:?}", plan.flows[0].ops);
    };

    assert_eq!(command.operation_name(), "play");
}
