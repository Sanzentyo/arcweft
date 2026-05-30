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
fn engine_executes_runtime_pure_call_from_flow() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::Int(3)),
                    RuntimeExpr::Value(RuntimeValue::Int(4)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::Int(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Call {
                    callee: "add".to_owned(),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::Int(2)),
                    ],
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::Int(0))),
        },
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = Engine::new(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "18"
    ));
    assert_eq!(result.stats.pure.pure_calls, 1);
    assert_eq!(result.stats.pure.vm_calls, 1);
    assert_eq!(result.stats.pure.arg_stack_packs, 1);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
}

#[test]
fn engine_batches_bracket_sequence_pure_calls() {
    let pure_call = |base, bonus| RuntimeExpr::PureCall {
        helper: RuntimePureHelperId(0),
        args: vec![
            RuntimeExpr::Value(RuntimeValue::Int(base)),
            RuntimeExpr::Value(RuntimeValue::Int(bonus)),
        ],
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::BracketSeq(vec![
                pure_call(3, 4),
                pure_call(5, 6),
                pure_call(7, 8),
            ]))],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = Engine::new(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "bracket_seq/3"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.pure_calls, 3);
    assert_eq!(result.stats.pure.vm_calls, 3);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(
        result.stats.pure.result_bytes_copied,
        3 * std::mem::size_of::<i64>()
    );
}

#[test]
fn engine_fuses_bracket_sequence_pure_batch_sum() {
    let pure_call = |base, bonus| RuntimeExpr::PureCall {
        helper: RuntimePureHelperId(0),
        args: vec![
            RuntimeExpr::Value(RuntimeValue::Int(base)),
            RuntimeExpr::Value(RuntimeValue::Int(bonus)),
        ],
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::BracketSeq(vec![
                    pure_call(3, 4),
                    pure_call(5, 6),
                    pure_call(7, 8),
                ])),
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = Engine::new(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "98"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.pure_calls, 3);
    assert_eq!(result.stats.pure.vm_calls, 3);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
}

#[test]
fn engine_batches_map_closure_pure_calls() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Map {
                source: Box::new(RuntimeExpr::BracketSeq(vec![
                    RuntimeExpr::Value(RuntimeValue::Int(3)),
                    RuntimeExpr::Value(RuntimeValue::Int(5)),
                    RuntimeExpr::Value(RuntimeValue::Int(7)),
                ])),
                param: "base".to_owned(),
                body: Box::new(RuntimeExpr::PureCall {
                    helper: RuntimePureHelperId(0),
                    args: vec![
                        RuntimeExpr::Local("base".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::Int(4)),
                    ],
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = Engine::new(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "bracket_seq/3"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.pure_calls, 3);
    assert_eq!(result.stats.pure.vm_calls, 3);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
}

#[test]
fn engine_fuses_map_closure_pure_batch_sum() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::BracketSeq(vec![
                        RuntimeExpr::Value(RuntimeValue::Int(3)),
                        RuntimeExpr::Value(RuntimeValue::Int(5)),
                        RuntimeExpr::Value(RuntimeValue::Int(7)),
                    ])),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::Int(4)),
                        ],
                    }),
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = Engine::new(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "60"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.pure_calls, 3);
    assert_eq!(result.stats.pure.vm_calls, 3);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(
        result.stats.pure.result_bytes_copied,
        3 * std::mem::size_of::<i64>()
    );
}

#[test]
fn engine_runs_flow_thread_body_as_child_fiber() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Thread {
                    name: Some("worker".to_owned()),
                    body: vec![FlowOp::Effect(call("child.work"))],
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = Engine::new(plan);

    let first = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
    assert_eq!(first.output.requests.tasks.len(), 1);
    assert_eq!(first.stats.child_fibers, 1);
    assert_eq!(engine.child_fiber_count(), 1);

    let second = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 8 },
        },
    );

    assert_eq!(second.output.effects.line, vec![call("child.work")]);
    assert!(second.output.flow_events.contains(&FlowEvent::Return {
        value: "done".to_owned()
    }));
    assert_eq!(engine.child_fiber_count(), 0);
    assert!(matches!(
        engine.fiber().status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "done"
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
fn engine_runs_bounded_await_many_tasks_in_source_order() {
    let mut engine = Engine::new(await_many_read_plan());

    let first = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(first.requests.tasks.len(), 2);
    assert_eq!(
        first.requests.tasks[0].id,
        TaskId("task.read_many.0".to_owned())
    );
    assert_eq!(
        first.requests.tasks[1].id,
        TaskId("task.read_many.1".to_owned())
    );
    assert!(matches!(
        engine.fiber().status,
        FlowFiberStatus::WaitingMany(_)
    ));

    let second = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            task_events: vec![
                ready_event("task.read_many.0", 0, "A"),
                ready_event("task.read_many.1", 1, "B"),
            ],
            ..RuntimeStepInput::default()
        },
    );
    assert_eq!(second.requests.tasks.len(), 1);
    assert_eq!(
        second.requests.tasks[0].id,
        TaskId("task.read_many.2".to_owned())
    );
    assert!(matches!(
        engine.fiber().status,
        FlowFiberStatus::WaitingMany(_)
    ));

    let third = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            task_events: vec![ready_event("task.read_many.2", 2, "C")],
            ..RuntimeStepInput::default()
        },
    );
    assert_eq!(
        third.flow_events.last(),
        Some(&FlowEvent::AwaitReady {
            need: NeedId("need.read_many".to_owned()),
            value: "3 item(s)".to_owned(),
        })
    );

    let returned = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        returned.flow_events,
        vec![FlowEvent::Return {
            value: "bracket_seq/3".to_owned()
        }]
    );
}

fn await_many_read_plan() -> RuntimePlan {
    RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::AwaitMany {
                    binding: Some(RuntimePattern::Ident("values".to_owned())),
                    target: await_many_read_target(),
                    pending: Vec::new(),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Local("values".to_owned())),
            ],
        }],
        Vec::new(),
    )
    .expect("await many plan is valid")
}

fn await_many_read_target() -> AwaitManyTarget {
    AwaitManyTarget::new(
        NeedId("need.read_many".to_owned()),
        TaskId("task.read_many".to_owned()),
        RuntimeExpr::BracketSeq(vec![
            RuntimeExpr::Value(RuntimeValue::String("save/a.txt".to_owned())),
            RuntimeExpr::Value(RuntimeValue::String("save/b.txt".to_owned())),
            RuntimeExpr::Value(RuntimeValue::String("save/c.txt".to_owned())),
        ]),
        AWAIT_MANY_ITEM_BINDING,
        2,
        HostTaskRequestTemplate::new(
            "fs",
            "read_text",
            [HostTaskArgTemplate::positional(RuntimeExpr::Local(
                AWAIT_MANY_ITEM_BINDING.to_owned(),
            ))],
        ),
    )
}

fn ready_event(task_id: &str, sequence: u64, value: &str) -> TaskEvent {
    TaskEvent {
        logical_epoch: LogicalEpoch(0),
        task_id: TaskId(task_id.to_owned()),
        sequence: TaskSequence(sequence),
        kind: TaskEventKind::Ready(value.to_owned()),
    }
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
fn for_loop_expands_one_iteration_at_a_time() {
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.for".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.for".to_owned()),
            ops: vec![FlowOp::For {
                pattern: RuntimePattern::Ident("item".to_owned()),
                source: RuntimeExpr::BracketSeq(vec![
                    RuntimeExpr::Value(RuntimeValue::Int(1)),
                    RuntimeExpr::Value(RuntimeValue::Int(2)),
                    RuntimeExpr::Value(RuntimeValue::Int(3)),
                    RuntimeExpr::Value(RuntimeValue::Int(4)),
                ]),
                body: vec![FlowOp::Effect(call("observe.item"))],
            }],
        }],
        Vec::new(),
    )
    .expect("for plan is valid");
    let mut engine = Engine::new(plan);

    let first = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.stats.executed_ops, 1);
    assert_eq!(first.stats.pending_ops_after, 3);
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
