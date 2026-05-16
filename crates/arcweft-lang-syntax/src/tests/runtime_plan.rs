use super::support::*;

fn call(callee: &str, args: &[&str]) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: callee.to_owned(),
        args: args.iter().map(|arg| (*arg).to_owned()).collect(),
    })
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
            wait mark .release_focus
            wait 0.35s
            tick_motion()
            defer { cleanup_motion() }
        at(0.42s): alice.stage.face(worried)
        on .release_focus:
            'line.focus.main |> drop
            defer { cleanup_handler() }
        defer { cleanup_line_scope() }
        finally:
            cleanup_line()
            defer { cleanup_finally_probe() }
}
",
    );
    let hir = lower_to_hir(&tree).expect("runtime plan fixture lowers to HIR");
    validate_typecheck_ready(&hir).expect("runtime plan fixture is typecheck-ready");

    let groups = lower_line_task_groups(&hir).expect("line task group lowers");
    assert_eq!(groups.len(), 1);
    let group = groups[0].group();

    assert_eq!(
        group.init,
        vec![LineEffectRequest::RegisterHandle {
            key: "'line.focus.main".to_owned(),
            handle: "acquire_focus()".to_owned(),
        }]
    );
    assert_eq!(
        group.init_defer_stack,
        vec![vec![call("cleanup_init_probe", &[])]]
    );
    assert_eq!(
        group.defer_stack,
        vec![vec![call("cleanup_line_scope", &[])]]
    );
    assert_eq!(group.children.len(), 3);
    assert_eq!(group.children[0].name.as_deref(), Some("motion"));
    assert_eq!(
        group.children[0].body,
        vec![
            LineEffectRequest::WaitMark(".release_focus".to_owned()),
            LineEffectRequest::Wait(arcweft_core::LogicalDuration::from_nanos(350_000_000)),
            call("tick_motion", &[]),
        ]
    );
    assert_eq!(
        group.children[0].defer_stack,
        vec![vec![call("cleanup_motion", &[])]]
    );
    assert_eq!(group.children[1].name.as_deref(), Some("at(0.42s)"));
    assert_eq!(
        group.children[1].body,
        vec![
            LineEffectRequest::Wait(arcweft_core::LogicalDuration::from_nanos(420_000_000)),
            call("alice.stage.face", &["worried"]),
        ]
    );
    assert_eq!(group.children[2].name.as_deref(), Some(".release_focus"));
    assert_eq!(
        group.children[2].body,
        vec![
            LineEffectRequest::WaitMark(".release_focus".to_owned()),
            LineEffectRequest::DropHandle {
                key: "'line.focus.main".to_owned(),
            },
        ]
    );
    assert_eq!(
        group.children[2].defer_stack,
        vec![vec![call("cleanup_handler", &[])]]
    );
    assert_eq!(
        group.finally,
        vec![
            call("cleanup_line", &[]),
            call("cleanup_finally_probe", &[])
        ]
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
    assert_eq!(
        groups[0].group().init,
        vec![call("cue_start", &[]), call("cue_next", &[])]
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
    let init = &groups[0].group().init;
    assert!(matches!(
        &init[0],
        LineEffectRequest::Log(RuntimeLog { level, message, fields })
            if level == "info" && message == "selected {id:?}" && fields.len() == 1
    ));
    assert!(matches!(
        &init[1],
        LineEffectRequest::SignalWrite(RuntimeAssignment { target, value })
            if target == "@signal.current_flow" && value == "@flow.effects"
    ));
    assert!(matches!(
        &init[2],
        LineEffectRequest::MetricWrite(RuntimeAssignment { target, value })
            if target == "@metric.frame_time_ms" && value == "frame_time.ms()"
    ));
    assert!(matches!(&init[3], LineEffectRequest::EmitEvent(event) if event.fields.len() == 1));
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
        memo line_handles(scope=line)
        assert actor.ready()
        cancel on input .SkipLine { out 'line .Skipped }
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
    assert_eq!(group.memo[0].name, "line_handles(scope=line)");
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
