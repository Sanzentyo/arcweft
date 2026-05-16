use super::support::*;

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
        vec![vec![LineEffectRequest::EmitSignal(
            "cleanup_init_probe()".to_owned()
        )]]
    );
    assert_eq!(
        group.defer_stack,
        vec![vec![LineEffectRequest::EmitSignal(
            "cleanup_line_scope()".to_owned()
        )]]
    );
    assert_eq!(group.children.len(), 3);
    assert_eq!(group.children[0].name.as_deref(), Some("motion"));
    assert_eq!(
        group.children[0].body,
        vec![
            LineEffectRequest::WaitMark(".release_focus".to_owned()),
            LineEffectRequest::Wait(arcweft_core::LogicalDuration::from_nanos(350_000_000)),
            LineEffectRequest::EmitSignal("tick_motion()".to_owned()),
        ]
    );
    assert_eq!(
        group.children[0].defer_stack,
        vec![vec![LineEffectRequest::EmitSignal(
            "cleanup_motion()".to_owned()
        )]]
    );
    assert_eq!(group.children[1].name.as_deref(), Some("at(0.42s)"));
    assert_eq!(
        group.children[1].body,
        vec![
            LineEffectRequest::Wait(arcweft_core::LogicalDuration::from_nanos(420_000_000)),
            LineEffectRequest::EmitSignal("alice.stage.face(worried)".to_owned()),
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
        vec![vec![LineEffectRequest::EmitSignal(
            "cleanup_handler()".to_owned()
        )]]
    );
    assert_eq!(
        group.finally,
        vec![
            LineEffectRequest::EmitSignal("cleanup_line()".to_owned()),
            LineEffectRequest::EmitSignal("cleanup_finally_probe()".to_owned()),
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

    let errors = lower_line_task_groups(&hir).expect_err("line option is not silently dropped");

    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("line-plan option `voice`"))
    );
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
        vec![
            LineEffectRequest::EmitSignal("cue_start()".to_owned()),
            LineEffectRequest::EmitSignal("cue_next()".to_owned()),
        ]
    );
}
