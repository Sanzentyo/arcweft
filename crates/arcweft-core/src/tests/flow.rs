use super::call;
use crate::{
    engine::*, line_task::*, pattern::*, plan::*, step::*, task::*, time::LogicalDuration, value::*,
};

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

    let first = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(first.effects.line, vec![call("opening_line")]);
    assert!(matches!(
        first.flow_events.as_slice(),
        [FlowEvent::DialogueLine { .. }]
    ));

    let second = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        second.flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.next".to_owned())
        }]
    );

    let third = super::runtime_step(&mut engine, RuntimeStepInput::default());
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

    let presented = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        presented.flow_events,
        vec![FlowEvent::ChoicePresented {
            id: Some("choice.opening".to_owned())
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Choice(_)));

    let selected = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            input_events: vec![InputEvent {
                kind: "choice".to_owned(),
                payload: Some("choice.listen".to_owned()),
            }],
            ..RuntimeStepInput::default()
        },
    );
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
        request: HostTaskRequestTemplate::new(
            "asset",
            "image",
            [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                RuntimeValue::String("asset.bg.room".to_owned()),
            ))],
        ),
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.opening".to_owned()),
            ops: vec![
                FlowOp::Await {
                    binding: None,
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

    let waiting = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(waiting.effects.line, vec![call("show_loading")]);
    assert_eq!(waiting.requests.tasks[0].id, target.task);
    assert_eq!(
        waiting.requests.tasks[0].request,
        HostTaskRequest::AssetLoad(AssetRequest {
            id: "asset.bg.room".to_owned(),
            kind: "image".to_owned(),
        })
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Waiting(_)));

    let ready = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            task_events: vec![TaskEvent {
                logical_epoch: LogicalEpoch(0),
                task_id: TaskId("task.bg".to_owned()),
                sequence: TaskSequence(0),
                kind: TaskEventKind::Ready("bg_handle".to_owned()),
            }],
            ..RuntimeStepInput::default()
        },
    );
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

    assert!(
        super::runtime_step(&mut engine, RuntimeStepInput::default())
            .flow_events
            .is_empty()
    );
    let goto = super::runtime_step(&mut engine, RuntimeStepInput::default());

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

    let first = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![
                RuntimeBinding {
                    name: "ready".to_owned(),
                    value: RuntimeValue::Bool(true),
                },
                RuntimeBinding {
                    name: "route".to_owned(),
                    value: RuntimeValue::EntityRef("flow.ready".to_owned()),
                },
            ],
            ..RuntimeStepInput::default()
        },
    );
    assert!(first.flow_events.is_empty());
    assert!(
        super::runtime_step(&mut engine, RuntimeStepInput::default())
            .flow_events
            .is_empty()
    );
    assert_eq!(
        super::runtime_step(&mut engine, RuntimeStepInput::default()).flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.match".to_owned())
        }]
    );
    let mut matched = RuntimeStepOutput::default();
    for _ in 0..6 {
        matched = super::runtime_step(&mut engine, RuntimeStepInput::default());
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
        super::runtime_step(&mut engine, RuntimeStepInput::default());
    }
    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

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

    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![keep_true],
            ..RuntimeStepInput::default()
        },
    );
    super::runtime_step(&mut engine, RuntimeStepInput::default());
    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![keep_false],
            ..RuntimeStepInput::default()
        },
    );
    let mut output = RuntimeStepOutput::default();
    for _ in 0..6 {
        output = super::runtime_step(&mut engine, RuntimeStepInput::default());
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

    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![RuntimeBinding {
                name: "opt".to_owned(),
                value: RuntimeValue::Variant {
                    path: None,
                    name: "Some".to_owned(),
                    payload: Some(Box::new(RuntimeValue::EntityRef("flow.next".to_owned()))),
                },
            }],
            ..RuntimeStepInput::default()
        },
    );
    for _ in 0..4 {
        super::runtime_step(&mut engine, RuntimeStepInput::default());
    }
    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

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

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("pattern binds `x` more than once")
    }));
    assert!(engine.fiber().env.get("x").is_none());
}

#[test]
fn typed_runtime_patterns_match_value_shape() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.typed".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.typed".to_owned()),
            ops: vec![FlowOp::Match {
                scrutinee: RuntimeExpr::Local("payload".to_owned()),
                arms: vec![
                    RuntimeMatchArm {
                        pattern: RuntimePattern::Typed {
                            name: "text".to_owned(),
                            ty: "String".to_owned(),
                        },
                        guard: None,
                        ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Local("text".to_owned()))],
                    },
                    RuntimeMatchArm {
                        pattern: RuntimePattern::Typed {
                            name: "bytes".to_owned(),
                            ty: "Bytes".to_owned(),
                        },
                        guard: None,
                        ops: vec![FlowOp::Return("bytes".to_owned())],
                    },
                ],
            }],
        }],
        Vec::new(),
    )
    .expect("typed pattern plan is valid");
    let mut engine = Engine::new(plan);

    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![RuntimeBinding {
                name: "payload".to_owned(),
                value: RuntimeValue::String("hello".to_owned()),
            }],
            ..RuntimeStepInput::default()
        },
    );
    let mut output = RuntimeStepOutput::default();
    for _ in 0..4 {
        output = super::runtime_step(&mut engine, RuntimeStepInput::default());
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
            value: "hello".to_owned()
        }]
    );
}

#[test]
fn fs_write_dispatches_string_and_bytes_payloads() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.write".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.write".to_owned()),
            ops: vec![
                FlowOp::Await {
                    binding: None,
                    target: AwaitTarget {
                        need: NeedId("need.text".to_owned()),
                        task: TaskId("task.text".to_owned()),
                        request: HostTaskRequestTemplate::new(
                            "fs",
                            "write",
                            [
                                HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                    RuntimeValue::String("save/out.txt".to_owned()),
                                )),
                                HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                    RuntimeValue::String("hello".to_owned()),
                                )),
                            ],
                        ),
                    },
                    pending: Vec::new(),
                },
                FlowOp::Await {
                    binding: None,
                    target: AwaitTarget {
                        need: NeedId("need.bytes".to_owned()),
                        task: TaskId("task.bytes".to_owned()),
                        request: HostTaskRequestTemplate::new(
                            "fs",
                            "write",
                            [HostTaskArgTemplate::spread(RuntimeExpr::BracketSeq(vec![
                                RuntimeExpr::Value(RuntimeValue::String("save/out.bin".to_owned())),
                                RuntimeExpr::Value(RuntimeValue::BracketSeq(vec![
                                    RuntimeValue::Int(1),
                                    RuntimeValue::Int(2),
                                ])),
                            ]))],
                        ),
                    },
                    pending: Vec::new(),
                },
            ],
        }],
        Vec::new(),
    )
    .expect("fs.write plan is valid");
    let mut engine = Engine::new(plan);

    let text = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert!(matches!(
        &text.requests.tasks[0].request,
        HostTaskRequest::FileWriteText(FileWriteTextRequest { path, text })
            if path == "save/out.txt" && text == "hello"
    ));

    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            task_events: vec![TaskEvent {
                logical_epoch: LogicalEpoch(0),
                task_id: TaskId("task.text".to_owned()),
                sequence: TaskSequence(0),
                kind: TaskEventKind::Ready("ok".to_owned()),
            }],
            ..RuntimeStepInput::default()
        },
    );
    let mut bytes = RuntimeStepOutput::default();
    for _ in 0..4 {
        bytes = super::runtime_step(&mut engine, RuntimeStepInput::default());
        if !bytes.requests.tasks.is_empty() {
            break;
        }
    }
    assert!(matches!(
        &bytes.requests.tasks[0].request,
        HostTaskRequest::FileWriteBytes(FileWriteBytesRequest { path, bytes })
            if path == "save/out.bin" && bytes == &[1, 2]
    ));
}

#[test]
fn runtime_call_spread_expands_sequence_arguments() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.spread".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.spread".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Call {
                callee: "add".to_owned(),
                args: vec![RuntimeExpr::SpreadArg(Box::new(RuntimeExpr::BracketSeq(
                    vec![
                        RuntimeExpr::Value(RuntimeValue::Int(20)),
                        RuntimeExpr::Value(RuntimeValue::Int(22)),
                    ],
                )))],
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let mut engine = Engine::new(plan);
    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "42".to_owned()
        }]
    );
}

#[test]
fn custom_host_request_spread_preserves_concrete_payload_values() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.log".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.log".to_owned()),
            ops: vec![FlowOp::Await {
                binding: None,
                target: AwaitTarget {
                    need: NeedId("need.log".to_owned()),
                    task: TaskId("task.log".to_owned()),
                    request: HostTaskRequestTemplate::new(
                        "log",
                        "emit",
                        [
                            HostTaskArgTemplate::positional(RuntimeExpr::Value(
                                RuntimeValue::String("loaded".to_owned()),
                            )),
                            HostTaskArgTemplate::spread(RuntimeExpr::BracketSeq(vec![
                                RuntimeExpr::Value(RuntimeValue::String("bg.room".to_owned())),
                                RuntimeExpr::Value(RuntimeValue::Int(3)),
                                RuntimeExpr::Value(RuntimeValue::Duration(
                                    LogicalDuration::from_nanos(120_000_000),
                                )),
                                RuntimeExpr::Value(RuntimeValue::EntityRef(
                                    "asset.bg.room".to_owned(),
                                )),
                            ])),
                        ],
                    ),
                },
                pending: Vec::new(),
            }],
        }],
        Vec::new(),
    )
    .expect("custom host request plan is valid");
    let mut engine = Engine::new(plan);

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());
    let HostTaskRequest::Custom {
        capability,
        operation,
        args,
    } = &output.requests.tasks[0].request
    else {
        panic!("expected custom host request");
    };

    assert_eq!(capability.0, "log");
    assert_eq!(operation, "emit");
    assert_eq!(
        args,
        &vec![
            RuntimePayload::from("loaded"),
            RuntimePayload::from("bg.room"),
            RuntimePayload::new(RuntimeValue::Int(3)),
            RuntimePayload::new(RuntimeValue::Duration(LogicalDuration::from_nanos(
                120_000_000
            ))),
            RuntimePayload::new(RuntimeValue::EntityRef("asset.bg.room".to_owned())),
        ]
    );
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

    super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            bindings: vec![RuntimeBinding {
                name: "opt".to_owned(),
                value: RuntimeValue::Variant {
                    path: None,
                    name: "Some".to_owned(),
                    payload: Some(Box::new(RuntimeValue::EntityRef("flow.next".to_owned()))),
                },
            }],
            ..RuntimeStepInput::default()
        },
    );
    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Goto {
            target: FlowRuntimeId("flow.next".to_owned())
        }]
    );
    assert!(engine.fiber().env.get("route").is_none());
}
