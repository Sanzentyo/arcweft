use super::call;
use crate::{engine::*, frame::*, line_task::*, pattern::*, plan::*, task::*, value::*};

#[test]
fn engine_steps_flow_ops_and_applies_goto() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("opening_line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.opening".to_owned()),
                ops: vec![
                    FlowOp::Dialogue {
                        line: RuntimeLineId("say.opening.001".to_owned()),
                        task_group: 0,
                    },
                    FlowOp::Goto(FlowRuntimeId("flow.next".to_owned())),
                ],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.next".to_owned()),
                ops: vec![FlowOp::Return("Ok(FlowExit::Done)".to_owned())],
            },
        ],
        vec![group],
    )
    .expect("flow plan is valid");
    let mut engine = Engine::new(plan);

    let first = engine.step(FrameInput::default());
    assert_eq!(first.line_effects, vec![call("opening_line")]);
    assert!(matches!(
        first.flow_events.as_slice(),
        [FlowEvent::DialogueLine { .. }]
    ));

    let second = engine.step(FrameInput::default());
    assert_eq!(
        second.flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.next".to_owned())
        }]
    );

    let third = engine.step(FrameInput::default());
    assert_eq!(
        third.flow_events,
        vec![FlowEvent::Return {
            value: "Ok(FlowExit::Done)".to_owned()
        }]
    );
    assert!(matches!(
        engine.fiber().status,
        FlowFiberStatus::Done(FlowExit::Return(_))
    ));
}

#[test]
fn engine_waits_for_choice_input() {
    let option = ChoiceRuntimeOption {
        id: Some("choice.listen".to_owned()),
        label: "Listen".to_owned(),
        target: Some(FlowRuntimeId("flow.listen".to_owned())),
        out: None,
        effects: Vec::new(),
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.opening".to_owned()),
                ops: vec![FlowOp::Choice {
                    id: Some("choice.opening".to_owned()),
                    options: vec![option],
                }],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.listen".to_owned()),
                ops: vec![FlowOp::Return("listen".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("choice plan is valid");
    let mut engine = Engine::new(plan);

    let presented = engine.step(FrameInput::default());
    assert_eq!(
        presented.flow_events,
        vec![FlowEvent::ChoicePresented {
            id: Some("choice.opening".to_owned())
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Choice(_)));

    let selected = engine.step(FrameInput {
        input_events: vec![InputEvent {
            kind: "choice".to_owned(),
            payload: Some("choice.listen".to_owned()),
        }],
        ..FrameInput::default()
    });
    assert_eq!(
        selected.flow_events,
        vec![
            FlowEvent::ChoiceSelected {
                id: Some("choice.opening".to_owned()),
                option: "choice.listen".to_owned()
            },
            FlowEvent::Goto {
                target: FlowRuntimeId("flow.listen".to_owned())
            }
        ]
    );
}

#[test]
fn engine_waits_for_await_task_event() {
    let target = AwaitTarget {
        need: NeedId("need.bg".to_owned()),
        task: TaskId("task.bg".to_owned()),
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.opening".to_owned()),
            ops: vec![
                FlowOp::Await {
                    target: target.clone(),
                    pending: vec![call("show_loading")],
                },
                FlowOp::Return("ready".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("await plan is valid");
    let mut engine = Engine::new(plan);

    let waiting = engine.step(FrameInput::default());
    assert_eq!(waiting.line_effects, vec![call("show_loading")]);
    assert_eq!(waiting.task_requests[0].id, target.task);
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Waiting(_)));

    let ready = engine.step(FrameInput {
        task_events: vec![TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("task.bg".to_owned()),
            sequence: TaskSequence(0),
            kind: TaskEventKind::Ready("bg_handle".to_owned()),
        }],
        ..FrameInput::default()
    });
    assert!(ready.flow_events.iter().any(|event| matches!(
        event,
        FlowEvent::AwaitReady { value, .. } if value == "bg_handle"
    )));
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Running));
}

#[test]
fn engine_binds_runtime_values_and_gotos_entity_refs() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.opening".to_owned()),
                ops: vec![
                    FlowOp::Let {
                        pattern: RuntimePattern::Ident("route".to_owned()),
                        expr: RuntimeExpr::EntityRef("flow.next".to_owned()),
                    },
                    FlowOp::GotoExpr(RuntimeExpr::Local("route".to_owned())),
                ],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.next".to_owned()),
                ops: vec![FlowOp::Return("done".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = Engine::new(plan);

    assert!(engine.step(FrameInput::default()).flow_events.is_empty());
    let goto = engine.step(FrameInput::default());

    assert_eq!(
        goto.flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.next".to_owned())
        }]
    );
}

#[test]
fn engine_runs_if_and_match_blocks_from_runtime_values() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![
            RuntimeFlow {
                id: FlowRuntimeId("flow.opening".to_owned()),
                ops: vec![FlowOp::If {
                    condition: RuntimeExpr::Local("ready".to_owned()),
                    then_ops: vec![FlowOp::Goto(FlowRuntimeId("flow.match".to_owned()))],
                    else_ops: vec![FlowOp::Return("wait".to_owned())],
                }],
            },
            RuntimeFlow {
                id: FlowRuntimeId("flow.match".to_owned()),
                ops: vec![FlowOp::Match {
                    scrutinee: RuntimeExpr::Local("route".to_owned()),
                    arms: vec![
                        RuntimeMatchArm {
                            pattern: RuntimePattern::Entity("flow.ready".to_owned()),
                            guard: None,
                            ops: vec![FlowOp::Return("ready".to_owned())],
                        },
                        RuntimeMatchArm {
                            pattern: RuntimePattern::Discard,
                            guard: None,
                            ops: vec![FlowOp::Return("fallback".to_owned())],
                        },
                    ],
                }],
            },
        ],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = Engine::new(plan);

    let first = engine.step(FrameInput {
        external_values: vec![
            RuntimeBinding {
                name: "ready".to_owned(),
                value: RuntimeValue::Bool(true),
            },
            RuntimeBinding {
                name: "route".to_owned(),
                value: RuntimeValue::EntityRef("flow.ready".to_owned()),
            },
        ],
        ..FrameInput::default()
    });
    assert!(first.flow_events.is_empty());
    assert!(engine.step(FrameInput::default()).flow_events.is_empty());
    assert_eq!(
        engine.step(FrameInput::default()).flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.match".to_owned())
        }]
    );
    let mut matched = FrameOutput::default();
    for _ in 0..6 {
        matched = engine.step(FrameInput::default());
        if matched
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::Return { .. }))
        {
            break;
        }
    }

    assert_eq!(
        matched.flow_events,
        vec![FlowEvent::Return {
            value: "ready".to_owned()
        }]
    );
}

#[test]
fn loop_break_exits_to_next_flow_op_without_running_remaining_body() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.loop".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.loop".to_owned()),
            ops: vec![
                FlowOp::Loop {
                    body: vec![
                        FlowOp::Break(None),
                        FlowOp::Return("unreachable".to_owned()),
                    ],
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("loop plan is valid");
    let mut engine = Engine::new(plan);

    for _ in 0..3 {
        engine.step(FrameInput::default());
    }
    let output = engine.step(FrameInput::default());

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "done".to_owned()
        }]
    );
}

#[test]
fn while_continue_reruns_condition_and_skips_remaining_body() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.while".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.while".to_owned()),
            ops: vec![
                FlowOp::While {
                    condition: RuntimeExpr::Local("keep".to_owned()),
                    body: vec![FlowOp::Continue, FlowOp::Return("unreachable".to_owned())],
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("while plan is valid");
    let mut engine = Engine::new(plan);
    let keep_true = RuntimeBinding {
        name: "keep".to_owned(),
        value: RuntimeValue::Bool(true),
    };
    let keep_false = RuntimeBinding {
        name: "keep".to_owned(),
        value: RuntimeValue::Bool(false),
    };

    engine.step(FrameInput {
        external_values: vec![keep_true],
        ..FrameInput::default()
    });
    engine.step(FrameInput::default());
    engine.step(FrameInput {
        external_values: vec![keep_false],
        ..FrameInput::default()
    });
    let mut output = FrameOutput::default();
    for _ in 0..6 {
        output = engine.step(FrameInput::default());
        if output
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::Return { .. }))
        {
            break;
        }
    }

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "done".to_owned()
        }]
    );
}

#[test]
fn branch_pattern_bindings_do_not_leak_after_branch_scope() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.branch".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.branch".to_owned()),
            ops: vec![
                FlowOp::IfLet {
                    pattern: RuntimePattern::Variant {
                        path: None,
                        name: "Some".to_owned(),
                        payload: Some(Box::new(RuntimePattern::Ident("route".to_owned()))),
                    },
                    expr: RuntimeExpr::Local("opt".to_owned()),
                    guard: None,
                    then_ops: vec![FlowOp::Noop],
                    else_ops: Vec::new(),
                },
                FlowOp::GotoExpr(RuntimeExpr::Local("route".to_owned())),
            ],
        }],
        Vec::new(),
    )
    .expect("branch plan is valid");
    let mut engine = Engine::new(plan);

    engine.step(FrameInput {
        external_values: vec![RuntimeBinding {
            name: "opt".to_owned(),
            value: RuntimeValue::Variant {
                path: None,
                name: "Some".to_owned(),
                payload: Some(Box::new(RuntimeValue::EntityRef("flow.next".to_owned()))),
            },
        }],
        ..FrameInput::default()
    });
    for _ in 0..4 {
        engine.step(FrameInput::default());
    }
    let output = engine.step(FrameInput::default());

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("unknown runtime binding `route`")
    }));
}

#[test]
fn duplicate_pattern_bindings_fail_before_env_mutation() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.dup".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.dup".to_owned()),
            ops: vec![FlowOp::Let {
                pattern: RuntimePattern::Tuple(vec![
                    RuntimePattern::Ident("x".to_owned()),
                    RuntimePattern::Ident("x".to_owned()),
                ]),
                expr: RuntimeExpr::Tuple(vec![
                    RuntimeExpr::Value(RuntimeValue::Int(1)),
                    RuntimeExpr::Value(RuntimeValue::Int(2)),
                ]),
            }],
        }],
        Vec::new(),
    )
    .expect("duplicate pattern plan is valid");
    let mut engine = Engine::new(plan);

    let output = engine.step(FrameInput::default());

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("pattern binds `x` more than once")
    }));
    assert!(engine.fiber().env.get("x").is_none());
}

#[test]
fn if_let_expression_binds_only_success_branch() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.if_let".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.if_let".to_owned()),
            ops: vec![
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("target".to_owned()),
                    expr: RuntimeExpr::IfLet {
                        pattern: RuntimePattern::Variant {
                            path: None,
                            name: "Some".to_owned(),
                            payload: Some(Box::new(RuntimePattern::Ident("route".to_owned()))),
                        },
                        expr: Box::new(RuntimeExpr::Local("opt".to_owned())),
                        guard: None,
                        then_expr: Box::new(RuntimeExpr::Local("route".to_owned())),
                        else_expr: Box::new(RuntimeExpr::EntityRef("flow.fallback".to_owned())),
                    },
                },
                FlowOp::GotoExpr(RuntimeExpr::Local("target".to_owned())),
            ],
        }],
        Vec::new(),
    )
    .expect("if-let runtime plan is valid");
    let mut engine = Engine::new(plan);

    engine.step(FrameInput {
        external_values: vec![RuntimeBinding {
            name: "opt".to_owned(),
            value: RuntimeValue::Variant {
                path: None,
                name: "Some".to_owned(),
                payload: Some(Box::new(RuntimeValue::EntityRef("flow.next".to_owned()))),
            },
        }],
        ..FrameInput::default()
    });
    let output = engine.step(FrameInput::default());

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.next".to_owned())
        }]
    );
    assert!(engine.fiber().env.get("route").is_none());
}
