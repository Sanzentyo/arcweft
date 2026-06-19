use arcweft_core::{
    effect::{
        LineEffectRequest, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall, RuntimeLog,
        RuntimeWaitTarget,
    },
    line_task::{LineChildTask, LineOutRequest, LineTaskNode, LineTaskTrigger},
    plan::{FlowOp, FlowRuntimeId, RuntimeEntryKind, RuntimeEntryTarget},
    source::{SourceHandlerPlan, SourceOp},
    stream::StreamOp,
    time::LogicalDuration,
    value::{RuntimeExpr, RuntimeValue},
};
use arcweft_lang_hir::lower::lower_to_hir;
use arcweft_lang_sema::check::validate_typecheck_ready;
use arcweft_lang_syntax::{
    ast::items::TypedSyntaxTree,
    expr::{Expr, parse_expr},
    parser::{ParseOptions, SourceDialect, parse_document, parse_source},
};
use arcweft_runtime_plan::{
    flow::{
        lower_agent_controller_plan_with_stats, lower_runtime_plan, lower_runtime_plan_with_stats,
    },
    line_task::lower_line_task_groups,
};

fn parse_ok(source: impl Into<String>) -> TypedSyntaxTree {
    let parsed = parse_source(source);
    assert!(
        parsed.errors().is_empty(),
        "expected source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

fn parse_agent_ok(source: impl Into<String>) -> TypedSyntaxTree {
    let parsed = parse_document(
        source,
        ParseOptions {
            source_dialect: SourceDialect::Agent,
        },
    );
    assert!(
        parsed.errors().is_empty(),
        "expected agent source to parse without errors, got {:?}",
        parsed.errors()
    );
    parsed.into_typed_tree()
}

fn call(callee: &str, args: &[&str]) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: callee.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    })
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
fn canonical_log_signal_metric_are_ordinary_calls() {
    assert!(matches!(
        parse_expr(r#"log.info("selected {id:?}", id = selected.id)"#)
            .expect("log.info parses as ordinary expression"),
        Expr::MethodCall { .. }
    ));
    assert!(matches!(
        parse_expr("signal.set(@signal.current_flow, @flow.opening)")
            .expect("signal.set parses as ordinary expression"),
        Expr::MethodCall { .. }
    ));
    assert!(matches!(
        parse_expr("metric.set(@metric.frame_time_ms, frame_time.ms())")
            .expect("metric.set parses as ordinary expression"),
        Expr::MethodCall { .. }
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
fn entry_selects_runtime_start_flow_from_compile_gap_fixture() {
    let tree = parse_ok(
        r#"
entry game @entry.main { start(@flow.second) }
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry lowers");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers with explicit entry");
    assert!(
        plan.entry_flow
            .as_ref()
            .is_some_and(|id| id.0 == "flow.second")
    );
}

#[test]
fn entry_accepts_bare_start_flow_target_from_compile_gap_fixture() {
    let tree = parse_ok(
        r#"
entry game {
    start @flow.second
}
flow @flow.first first { return "wrong" }
flow @flow.second second { return "right" }
"#,
    );
    let hir = lower_to_hir(&tree).expect("entry lowers");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers with bare start entry");
    assert!(
        plan.entry_flow
            .as_ref()
            .is_some_and(|id| id.0 == "flow.second")
    );
}

#[test]
fn agent_controller_plan_lowers_body_to_entry_flow() {
    let tree = parse_agent_ok(
        r"
#[agent(version = 1)]
agent @agent.observe_smoke observe_smoke()
effects { agent.observe }
{
    observe()
}
",
    );
    let hir = lower_to_hir(&tree).expect("agent lowers to HIR");
    let agent = hir.agents().first().expect("agent item lowers");

    let report =
        lower_agent_controller_plan_with_stats(&hir, agent).expect("agent controller lowers");

    assert_eq!(
        report.plan.entry_flow.as_ref().map(|id| id.0.as_str()),
        Some("agent.observe_smoke")
    );
    assert_eq!(report.plan.flows.len(), 1);
    assert_eq!(report.plan.flows[0].id.0, "agent.observe_smoke");
    assert!(!report.plan.flows[0].ops.is_empty());
    assert_eq!(report.plan.entries.len(), 1);
    assert_eq!(report.plan.entries[0].id.0, "entry.agent.observe_smoke");
    assert_eq!(
        report.plan.entries[0].kind,
        RuntimeEntryKind::Custom("agent_controller".to_owned())
    );
    assert_eq!(
        report.plan.entries[0].target,
        RuntimeEntryTarget::Flow(FlowRuntimeId("agent.observe_smoke".to_owned()))
    );
}

#[test]
fn agent_controller_plan_lowers_host_call_let_to_await() {
    let tree = parse_agent_ok(
        r#"
#[agent(version = 1)]
agent @agent.capture_smoke capture_smoke()
effects { agent.capture }
{
    let shot = capture(viewport(), format = .png, name = "hud")
    return shot.uri
}
"#,
    );
    let hir = lower_to_hir(&tree).expect("agent lowers to HIR");
    let agent = hir.agents().first().expect("agent item lowers");

    let report =
        lower_agent_controller_plan_with_stats(&hir, agent).expect("agent controller lowers");

    let FlowOp::Await {
        binding, target, ..
    } = &report.plan.flows[0].ops[0]
    else {
        panic!(
            "expected capture let to lower to Await, got {:?}",
            report.plan.flows[0].ops[0]
        );
    };
    assert!(binding.is_some());
    assert_eq!(target.request.capability.0, "agent");
    assert_eq!(target.request.operation, "capture");
    assert_eq!(target.request.args.len(), 2);
}

#[test]
fn agent_controller_plan_lowers_wait_predicate_to_host_task() {
    let tree = parse_agent_ok(
        r"
#[agent(version = 1)]
agent @agent.wait_smoke wait_smoke()
effects { agent.wait, agent.observe }
{
    let obs = wait(signal(@signal.ready).eq(true), timeout = 5ms, stable_frames = 2u32, poll_frames = 1u32)
    return obs.tick
}
",
    );
    let hir = lower_to_hir(&tree).expect("agent lowers to HIR");
    let agent = hir.agents().first().expect("agent item lowers");

    let report =
        lower_agent_controller_plan_with_stats(&hir, agent).expect("agent controller lowers");

    let FlowOp::Await { target, .. } = &report.plan.flows[0].ops[0] else {
        panic!(
            "expected wait let to lower to Await, got {:?}",
            report.plan.flows[0].ops[0]
        );
    };
    assert_eq!(target.request.capability.0, "agent");
    assert_eq!(target.request.operation, "wait");
    assert_eq!(target.request.args.len(), 2);
}

#[test]
fn agent_controller_plan_lowers_composite_wait_predicates_to_host_task() {
    let tree = parse_agent_ok(
        r"
#[agent(version = 1)]
agent @agent.wait_composite wait_composite()
effects { agent.wait, agent.observe }
{
    let obs = wait(any(exists(signal(@signal.ready)), metric(@metric.fps).ge(55.0f32)), timeout = 5ms)
    return obs.tick
}
",
    );
    let hir = lower_to_hir(&tree).expect("agent lowers to HIR");
    let agent = hir.agents().first().expect("agent item lowers");

    let report =
        lower_agent_controller_plan_with_stats(&hir, agent).expect("agent controller lowers");

    let FlowOp::Await { target, .. } = &report.plan.flows[0].ops[0] else {
        panic!(
            "expected wait let to lower to Await, got {:?}",
            report.plan.flows[0].ops[0]
        );
    };
    assert_eq!(target.request.capability.0, "agent");
    assert_eq!(target.request.operation, "wait");
    let predicate = target.request.args[0].value();
    let RuntimeExpr::Record(fields) = predicate else {
        panic!("expected predicate record, got {predicate:?}");
    };
    assert!(fields.iter().any(|field| {
        field.name == "kind"
            && matches!(&field.value, RuntimeExpr::Value(RuntimeValue::String(value)) if value == "any")
    }));
    assert!(format!("{predicate:?}").contains("greater_or_equal"));
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
fn line_plan_runtime_lowering_rejects_raw_items() {
    let tree = parse_ok(
        r"
flow @flow.raw raw {
    alice[待って。[p]]
    with:
        @bad raw item
}
",
    );
    let hir = lower_to_hir(&tree).expect("raw line plan fixture lowers to HIR");
    let errors = lower_line_task_groups(&hir).expect_err("raw line plan item is rejected");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("raw line-plan item"))
    );
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
        event.emit(GameEvent::ChoiceSelected, id = @choice.opening.listen)
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
            event.emit(GameEvent::Left)
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
    return Ok(FlowExit::Done)
}
",
    );
    let hir = lower_to_hir(&tree).expect("flow fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("runtime plan lowers");

    assert_eq!(
        plan.entry_flow,
        Some(FlowRuntimeId("flow.opening".to_owned()))
    );
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
    return Ok(FlowExit::Done)
}
",
    );
    let hir = lower_to_hir(&tree).expect("stream/source fixture lowers to HIR");

    let plan = lower_runtime_plan(&hir).expect("stream/source runtime plan lowers");

    assert_eq!(plan.stream_plans.len(), 1);
    assert_eq!(plan.stream_plans[0].id.0, "rms_level");
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
        [SourceHandlerPlan::Item { ops, .. }]
            if matches!(ops.as_slice(), [SourceOp::Yield(expr)]
                if matches!(expr, RuntimeExpr::PureCall { helper, .. } if helper.0 == 0))
    ));
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
    return Ok(FlowExit::Done)
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
    assert_eq!(
        options[0].target,
        Some(FlowRuntimeId("flow.alice_intro".to_owned()))
    );
}

#[test]
fn runtime_plan_lowers_await_calls_to_typed_host_requests() {
    let tree = parse_ok(
        r#"
flow @flow.loading loading {
    try await fs.read_text("game/config.arcw") with { pending p => progress.set(p.ratio) }
    try await http.fetch("https://example.invalid/api", method = "POST", body = "payload") with { pending p => progress.set(p.ratio) }
    try await asset.image(@asset.bg.room) with { pending p => progress.set(p.ratio) }
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
    return Ok(FlowExit::Done)
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
        FlowOp::Effect(ensure),
        FlowOp::Effect(assert),
        FlowOp::Effect(debug_assert),
    ] = plan.flows[0].ops.as_slice()
    else {
        panic!("expected three assertion-related effects");
    };

    assert!(matches!(
        ensure,
        LineEffectRequest::Ensure { condition, message }
            if condition == "route.is_some()" && message == "\"route missing\""
    ));
    assert!(matches!(
        assert,
        LineEffectRequest::Assert(assertion)
            if assertion.condition == "state.ready()"
                && assertion.message == "\"state must be ready\""
                && assertion.profile == RuntimeAssertionProfile::Always
    ));
    assert!(matches!(
        debug_assert,
        LineEffectRequest::Assert(assertion)
            if assertion.condition == "cache.consistent()"
                && assertion.message == "assertion failed"
                && assertion.profile == RuntimeAssertionProfile::DebugOnly
    ));
}
