use crate::{
    effect::*, engine::*, frame::*, line_task::*, pattern::*, plan::*, source::*, stream::*,
    task::*, value::*,
};

#[test]
fn normalizes_task_events_by_replay_stable_keys() {
    let events = vec![
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("b".to_owned()),
            sequence: TaskSequence(0),
            kind: TaskEventKind::Ready("b".to_owned()),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(0),
            task_id: TaskId("z".to_owned()),
            sequence: TaskSequence(9),
            kind: TaskEventKind::Ready("z".to_owned()),
        },
        TaskEvent {
            logical_epoch: LogicalEpoch(1),
            task_id: TaskId("a".to_owned()),
            sequence: TaskSequence(1),
            kind: TaskEventKind::Ready("a".to_owned()),
        },
    ];

    let normalized = normalize_task_events(events);
    let keys: Vec<_> = normalized
        .iter()
        .map(|event| {
            (
                event.logical_epoch,
                event.task_id.0.as_str(),
                event.sequence,
            )
        })
        .collect();
    assert_eq!(
        keys,
        vec![
            (LogicalEpoch(0), "z", TaskSequence(9)),
            (LogicalEpoch(1), "a", TaskSequence(1)),
            (LogicalEpoch(1), "b", TaskSequence(0)),
        ]
    );
}

#[test]
fn source_policy_is_pure_data() {
    let policy = SourcePolicy {
        backpressure: BackpressurePolicy::BoundedQueue {
            capacity: 8,
            on_overflow: OverflowPolicy::Coalesce,
        },
        replay: ReplayPolicy::HashOnly,
        privacy: PrivacyPolicy::Transient,
        max_queue: 8,
    };

    assert!(matches!(
        policy.backpressure,
        BackpressurePolicy::BoundedQueue {
            capacity: 8,
            on_overflow: OverflowPolicy::Coalesce,
        }
    ));
    assert_eq!(policy.replay, ReplayPolicy::HashOnly);
    assert_eq!(policy.privacy, PrivacyPolicy::Transient);
}

#[test]
fn normalizes_source_events_by_source_and_sequence() {
    let events: Vec<SourceEvent<String, String>> = vec![
        SourceEvent {
            source: SourceId("source.b".to_owned()),
            sequence: TaskSequence(2),
            kind: SourceEventKind::Item("b2".to_owned()),
        },
        SourceEvent {
            source: SourceId("source.a".to_owned()),
            sequence: TaskSequence(9),
            kind: SourceEventKind::Item("a9".to_owned()),
        },
        SourceEvent {
            source: SourceId("source.a".to_owned()),
            sequence: TaskSequence(1),
            kind: SourceEventKind::Item("a1".to_owned()),
        },
    ];

    let normalized = normalize_source_events(events);
    let keys = normalized
        .iter()
        .map(|event| (event.source.0.as_str(), event.sequence))
        .collect::<Vec<_>>();
    assert_eq!(
        keys,
        vec![
            ("source.a", TaskSequence(1)),
            ("source.a", TaskSequence(9)),
            ("source.b", TaskSequence(2)),
        ]
    );
}

#[test]
fn source_runtime_latest_backpressure_keeps_latest_item() {
    let mut state = SourceRuntimeState::new(
        SourceId("source.camera".to_owned()),
        SourcePolicy {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        },
    );

    state.apply_event(SourceEvent {
        source: SourceId("source.camera".to_owned()),
        sequence: TaskSequence(0),
        kind: SourceEventKind::Item("old".to_owned()),
    });
    state.apply_event(SourceEvent {
        source: SourceId("source.camera".to_owned()),
        sequence: TaskSequence(1),
        kind: SourceEventKind::Item("new".to_owned()),
    });

    assert_eq!(state.queue.into_iter().collect::<Vec<_>>(), vec!["new"]);
}

#[test]
fn engine_records_source_events_without_running_adapters() {
    let source = SourcePlan {
        id: SourceId("source.camera".to_owned()),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy::default(),
        handlers: Vec::new(),
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(Vec::new(), vec![source]);
    let mut engine = Engine::new(plan);

    let output = engine.step(FrameInput {
        source_events: vec![SourceEvent {
            source: SourceId("source.camera".to_owned()),
            sequence: TaskSequence(0),
            kind: SourceEventKind::Item("frame0".to_owned()),
        }],
        ..FrameInput::default()
    });

    assert_eq!(output.source_events.len(), 1);
    assert_eq!(
        engine
            .fiber()
            .source_states
            .get(&SourceId("source.camera".to_owned()))
            .expect("source state exists")
            .queue
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}

#[test]
fn source_handler_yield_controls_source_queue() {
    let source_id = SourceId("source.camera".to_owned());
    let source = SourcePlan {
        id: source_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy {
            backpressure: BackpressurePolicy::LatestOnly,
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 1,
        },
        handlers: vec![SourceHandlerPlan::Item {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            ops: vec![SourceOp::Yield(RuntimeExpr::Local("frame".to_owned()))],
        }],
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(Vec::new(), vec![source]);
    let mut engine = Engine::new(plan);

    engine.step(FrameInput {
        source_events: vec![SourceEvent {
            source: source_id.clone(),
            sequence: TaskSequence(0),
            kind: SourceEventKind::Item("frame0".to_owned()),
        }],
        ..FrameInput::default()
    });

    assert_eq!(
        engine
            .fiber()
            .source_states
            .get(&source_id)
            .expect("source state exists")
            .queue
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}

#[test]
fn stream_plan_drains_source_queue_and_emits_stream_items() {
    let source_id = SourceId("source.camera".to_owned());
    let stream_id = StreamRuntimeId("stream.rms".to_owned());
    let source = SourcePlan {
        id: source_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        from: RuntimeExpr::Local("camera".to_owned()),
        policy: SourcePolicy {
            backpressure: BackpressurePolicy::BoundedQueue {
                capacity: 4,
                on_overflow: OverflowPolicy::Error,
            },
            replay: ReplayPolicy::HashOnly,
            privacy: PrivacyPolicy::Transient,
            max_queue: 4,
        },
        handlers: vec![SourceHandlerPlan::Item {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            ops: vec![SourceOp::Yield(RuntimeExpr::Local("frame".to_owned()))],
        }],
    };
    let stream = StreamPlan {
        id: stream_id.clone(),
        item_ty: "Frame".to_owned(),
        error_ty: "CaptureError".to_owned(),
        ops: vec![StreamOp::ForNext {
            pattern: RuntimePattern::Ident("frame".to_owned()),
            source: RuntimeExpr::EntityRef(source_id.0.clone()),
            body: vec![StreamOp::Yield {
                expr: RuntimeExpr::Local("frame".to_owned()),
            }],
        }],
    };
    let plan = RuntimePlan::new(None, Vec::new(), Vec::new())
        .expect("empty plan is valid")
        .with_generation_plans(vec![stream], vec![source]);
    let mut engine = Engine::new(plan);

    let output = engine.step(FrameInput {
        source_events: vec![SourceEvent {
            source: source_id.clone(),
            sequence: TaskSequence(0),
            kind: SourceEventKind::Item("frame0".to_owned()),
        }],
        ..FrameInput::default()
    });

    assert_eq!(
        output.stream_events,
        vec![StreamEvent {
            stream: stream_id.clone(),
            sequence: TaskSequence(0),
            kind: SourceEventKind::Item("frame0".to_owned()),
        }]
    );
    assert!(
        engine
            .fiber()
            .source_states
            .get(&source_id)
            .expect("source state exists")
            .queue
            .is_empty()
    );
    assert_eq!(
        engine
            .fiber()
            .stream_states
            .get(&stream_id)
            .expect("stream state exists")
            .queue
            .iter()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["frame0".to_owned()]
    );
}

fn call(name: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: name.to_owned(),
        args: Vec::new(),
    })
}

#[test]
fn runtime_records_log_signal_metric_and_event_observations() {
    let effects = vec![
        LineEffectRequest::Log(RuntimeLog {
            level: "info".to_owned(),
            message: "entered".to_owned(),
            fields: Vec::new(),
        }),
        LineEffectRequest::SignalWrite(RuntimeAssignment {
            target: "signal.current_flow".to_owned(),
            value: "flow.opening".to_owned(),
        }),
        LineEffectRequest::MetricWrite(RuntimeAssignment {
            target: "metric.frame_time_ms".to_owned(),
            value: "16".to_owned(),
        }),
        LineEffectRequest::EmitEvent(RuntimeEvent {
            event: "event.flow_entered".to_owned(),
            fields: Vec::new(),
        }),
    ];
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.opening".to_owned()),
            ops: effects.into_iter().map(FlowOp::Effect).collect(),
        }],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = Engine::new(plan);

    for _ in 0..4 {
        engine.step(FrameInput::default());
    }

    let observations = &engine.fiber().observations;
    assert_eq!(observations.logs.len(), 1);
    assert_eq!(
        observations.signals.get("signal.current_flow"),
        Some(&"flow.opening".to_owned())
    );
    assert_eq!(
        observations.metrics.get("metric.frame_time_ms"),
        Some(&"16".to_owned())
    );
    assert_eq!(observations.events.len(), 1);
}

#[test]
fn engine_steps_line_task_groups_as_sans_io_effects() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("line_start"))]),
            defer_stack: vec![vec![call("line_defer")]],
            completed_defer_stack: vec![vec![call("line_completed")]],
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let mut engine = Engine::new(RuntimePlan::lines_only(vec![group]));

    let output = engine.step(FrameInput::default());

    assert_eq!(
        output.line_effects,
        vec![
            call("line_start"),
            call("line_defer"),
            call("line_completed")
        ]
    );
    assert_eq!(engine.fiber().status, FlowFiberStatus::Done(FlowExit::Done));
}

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

#[test]
fn line_cancel_rule_replaces_normal_line_body() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("normal"))]),
            cancelled_defer_stack: vec![vec![call("cancel_cleanup")]],
            ..LineTaskScope::default()
        },
        cancel_rules: vec![LineCancelRuleRequest {
            trigger: "input .SkipLine".to_owned(),
            action: vec![LineEffectRequest::Out(LineOutRequest {
                label: None,
                value: ".Skipped".to_owned(),
            })],
        }],
        ..LineTaskGroup::default()
    };
    let output = run_line_task_group_for_input(
        &group,
        &FrameInput {
            input_events: vec![InputEvent {
                kind: "input".to_owned(),
                payload: Some(".SkipLine".to_owned()),
            }],
            ..FrameInput::default()
        },
    );

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::LineCancelled {
            trigger: "input .SkipLine".to_owned()
        }]
    );
    assert_eq!(
        output.line_effects,
        vec![
            LineEffectRequest::Out(LineOutRequest {
                label: None,
                value: ".Skipped".to_owned()
            }),
            call("cancel_cleanup")
        ]
    );
}

#[test]
fn child_task_triggers_emit_task_request_and_scoped_body() {
    let child = LineChildTask {
        id: TaskId("line.task.0.mark".to_owned()),
        key: Some(TaskKey("line.task.mark".to_owned())),
        name: Some("mark".to_owned()),
        trigger: LineTaskTrigger::Mark(".seen".to_owned()),
        priority: TaskPriority(7),
        join_policy: ChildJoinPolicy::Join,
        cancel_policy: ChildCancelPolicy::CancelAndJoin,
        scope: Box::new(LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("handler"))]),
            defer_stack: vec![vec![call("handler_defer")]],
            ..LineTaskScope::default()
        }),
    };
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Child(child)]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let input = FrameInput {
        input_events: vec![InputEvent {
            kind: "mark".to_owned(),
            payload: Some(".seen".to_owned()),
        }],
        ..FrameInput::default()
    };

    let output = run_line_task_group(&group, &input, ScopeExit::Completed);

    assert_eq!(output.task_requests.len(), 1);
    assert_eq!(output.task_requests[0].priority, TaskPriority(7));
    assert_eq!(
        output.line_effects,
        vec![call("handler"), call("handler_defer")]
    );
}
