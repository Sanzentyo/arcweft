use super::call;
use crate::{
    engine::*, line_task::*, pattern::*, plan::*, step::*, task::*, time::LogicalDuration, value::*,
};

#[test]
fn engine_steps_flow_ops_and_applies_goto() {
    let line = super::line_id("say.opening.001");
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("opening_line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![
            RuntimeFlow {
                id: super::flow_id("flow.opening"),
                ops: vec![
                    FlowOp::Dialogue {
                        line: line.clone(),
                        task_group: 0,
                    },
                    FlowOp::Goto(super::flow_id("flow.next")),
                ],
            },
            RuntimeFlow {
                id: super::flow_id("flow.next"),
                ops: vec![FlowOp::Return("Ok(FlowExit::Done)".to_owned())],
            },
        ],
        vec![group],
    )
    .expect("flow plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let first = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(first.effects.line, vec![call("opening_line")]);
    assert!(matches!(
        first.flow_events.as_slice(),
        [FlowEvent::DialogueLine { .. }]
    ));

    let blocked = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert!(blocked.flow_events.is_empty());
    assert!(blocked.effects.line.is_empty());
    assert!(matches!(
        engine.fiber().status,
        FlowFiberStatus::Dialogue(_)
    ));

    let resumed = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            input_events: vec![super::dialogue_advance(&line)],
            ..RuntimeStepInput::default()
        },
    );
    assert!(resumed.flow_events.is_empty());
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Running));

    let goto = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        goto.flow_events,
        vec![FlowEvent::Goto {
            target: super::flow_id("flow.next")
        }]
    );

    let returned = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        returned.flow_events,
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
fn scoped_cleanup_effects_emit_on_scope_exit_in_lifo_order() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.cleanup")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.cleanup"),
            ops: vec![
                FlowOp::Scope(vec![
                    FlowOp::RegisterCleanup {
                        key: "panel.one".to_owned(),
                        effect: super::call("cleanup.one"),
                    },
                    FlowOp::RegisterCleanup {
                        key: "panel.two".to_owned(),
                        effect: super::call("cleanup.two"),
                    },
                ]),
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("cleanup plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let output = engine
        .step(RuntimeStepInput::default(), drain_step_options(16))
        .output;

    assert_eq!(
        output.effects.line,
        vec![super::call("cleanup.two"), super::call("cleanup.one")]
    );
}

#[test]
fn root_cleanup_effects_drain_on_return_unless_cancelled() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.cleanup")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.cleanup"),
            ops: vec![
                FlowOp::RegisterCleanup {
                    key: "panel".to_owned(),
                    effect: super::call("cleanup.panel"),
                },
                FlowOp::CancelCleanup {
                    key: "panel".to_owned(),
                },
                FlowOp::RegisterCleanup {
                    key: "toast".to_owned(),
                    effect: super::call("cleanup.toast"),
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        Vec::new(),
    )
    .expect("cleanup plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let output = engine
        .step(RuntimeStepInput::default(), drain_step_options(16))
        .output;

    assert_eq!(output.effects.line, vec![super::call("cleanup.toast")]);
}

#[test]
fn scoped_overlay_cleanup_drains_on_goto_scene_transition() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.scene_a")),
        vec![
            RuntimeFlow {
                id: super::flow_id("flow.scene_a"),
                ops: vec![FlowOp::Scope(vec![
                    FlowOp::RegisterCleanup {
                        key: "handle.flow.scene_a.overlay".to_owned(),
                        effect: super::call("presentation.handle.dispose.overlay"),
                    },
                    FlowOp::Goto(super::flow_id("flow.scene_b")),
                ])],
            },
            RuntimeFlow {
                id: super::flow_id("flow.scene_b"),
                ops: vec![FlowOp::Return("done".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("cleanup plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let output = engine
        .step(RuntimeStepInput::default(), drain_step_options(16))
        .output;

    assert_eq!(
        output.effects.line,
        vec![super::call("presentation.handle.dispose.overlay")]
    );
    let expected_target = super::flow_id("flow.scene_b");
    assert!(
        output
            .flow_events
            .iter()
            .any(|event| matches!(event, FlowEvent::Goto { target } if target == &expected_target)),
        "{:#?}",
        output.flow_events
    );
}

fn drain_step_options(max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode: RuntimeStepMode::Drain,
        budget: RuntimeStepBudget { max_ops },
    }
}

#[test]
fn engine_executes_runtime_pure_call_from_flow() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::i64(3)),
                    RuntimeExpr::Value(RuntimeValue::i64(4)),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::If {
            condition: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Ge,
                rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(3))),
            }),
            then_expr: Box::new(RuntimeExpr::Binary {
                lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
                op: RuntimeBinaryOp::Mul,
                rhs: Box::new(RuntimeExpr::Call {
                    callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                    args: vec![
                        RuntimeExpr::Local("bonus".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(2)),
                    ],
                }),
            }),
            else_expr: Box::new(RuntimeExpr::Value(RuntimeValue::i64(0))),
        },
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "18"
    ));
    assert_eq!(result.stats.pure.pure_calls, 1);
    assert_eq!(result.stats.pure.vm_calls, 1);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        2 * std::mem::size_of::<i64>()
    );
}

fn self_field(field: &str) -> RuntimeExpr {
    RuntimeExpr::Field {
        target: Box::new(RuntimeExpr::Local("self".to_owned())),
        field: field.to_owned(),
    }
}

fn counter_trait_identity(id: usize, method_name: &str) -> RuntimeTraitMethodIdentity {
    RuntimeTraitMethodIdentity {
        impl_id: id,
        trait_id: Some(id),
        witness: Some(id),
        trait_name: Some(
            if method_name == "next" {
                "Iterator"
            } else {
                "IntoIterator"
            }
            .to_owned(),
        ),
        self_type: "CounterIter".to_owned(),
        method_name: method_name.to_owned(),
        monomorph_label: format!("CounterIter::{method_name}"),
    }
}

fn counter_state() -> RuntimeValue {
    RuntimeValue::Record(vec![
        RuntimeFieldValue {
            name: "current".to_owned(),
            value: RuntimeValue::i64(0),
        },
        RuntimeFieldValue {
            name: "end".to_owned(),
            value: RuntimeValue::i64(1),
        },
    ])
}

fn counter_next_body() -> RuntimeExpr {
    RuntimeExpr::If {
        condition: Box::new(RuntimeExpr::Binary {
            lhs: Box::new(self_field("current")),
            op: RuntimeBinaryOp::Lt,
            rhs: Box::new(self_field("end")),
        }),
        then_expr: Box::new(RuntimeExpr::Let {
            name: "value".to_owned(),
            expr: Box::new(self_field("current")),
            body: Box::new(RuntimeExpr::AssignField {
                target: Box::new(RuntimeExpr::Local("self".to_owned())),
                field: "current".to_owned(),
                expr: Box::new(RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Local("value".to_owned())),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
                }),
                body: Box::new(RuntimeExpr::Variant {
                    path: None,
                    name: "Some".to_owned(),
                    payload: Some(Box::new(RuntimeExpr::Local("value".to_owned()))),
                }),
            }),
        }),
        else_expr: Box::new(RuntimeExpr::Variant {
            path: None,
            name: "None".to_owned(),
            payload: None,
        }),
    }
}

fn counter_trait_methods() -> Vec<RuntimeTraitMethod> {
    vec![
        RuntimeTraitMethod {
            id: RuntimeTraitMethodId(0),
            identity: counter_trait_identity(0, "into_iter"),
            receiver: RuntimeReceiverMode::Owned,
            input_names: vec!["self".to_owned()],
            input_types: vec![RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            body: RuntimeExpr::Local("self".to_owned()),
        },
        RuntimeTraitMethod {
            id: RuntimeTraitMethodId(1),
            identity: counter_trait_identity(1, "next"),
            receiver: RuntimeReceiverMode::MutRef,
            input_names: vec!["self".to_owned()],
            input_types: vec![RuntimePureInputType::Value],
            output_type: RuntimePureOutputType::Value,
            body: counter_next_body(),
        },
    ]
}

fn counter_witness_plan() -> RuntimePlan {
    super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::For {
                pattern: RuntimePattern::Ident("item".to_owned()),
                source: RuntimeExpr::Value(counter_state()),
                evidence: RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                    item_type: "i64".to_owned(),
                    into_iter_type: "CounterIter".to_owned(),
                    executable: RuntimeIteratorWitnessExecutable::TraitCalls(
                        RuntimeIteratorWitnessCalls {
                            into_iter: RuntimeTraitMethodId(0),
                            next: RuntimeTraitMethodId(1),
                        },
                    ),
                }),
                body: vec![FlowOp::ReturnExpr(RuntimeExpr::Local("item".to_owned()))],
            }],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_trait_methods(counter_trait_methods())
}

fn counter_identity_trait_methods() -> Vec<RuntimeTraitMethod> {
    vec![RuntimeTraitMethod {
        id: RuntimeTraitMethodId(0),
        identity: counter_trait_identity(0, "next"),
        receiver: RuntimeReceiverMode::MutRef,
        input_names: vec!["self".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
        body: counter_next_body(),
    }]
}

fn counter_identity_witness_plan() -> RuntimePlan {
    super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::For {
                pattern: RuntimePattern::Ident("item".to_owned()),
                source: RuntimeExpr::Value(counter_state()),
                evidence: RuntimeIteratorEvidence::Witness(RuntimeIteratorWitnessEvidence {
                    item_type: "i64".to_owned(),
                    into_iter_type: "Counter".to_owned(),
                    executable: RuntimeIteratorWitnessExecutable::IdentityIntoIterator(
                        RuntimeIteratorIdentityWitnessCalls {
                            next: RuntimeTraitMethodId(0),
                        },
                    ),
                }),
                body: vec![FlowOp::ReturnExpr(RuntimeExpr::Local("item".to_owned()))],
            }],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_trait_methods(counter_identity_trait_methods())
}

#[test]
fn engine_executes_for_loop_through_trait_method_witness_calls() {
    let mut engine = super::engine_for_test_plan(counter_witness_plan());

    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 8 },
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
fn engine_executes_for_loop_through_iterator_identity_witness() {
    let mut engine = super::engine_for_test_plan(counter_identity_witness_plan());

    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 8 },
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
fn engine_routes_non_i64_pure_call_to_value_backend() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![RuntimeExpr::Value(RuntimeValue::String("ready".to_owned()))],
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "echo".to_owned(),
        input_names: vec!["label".to_owned()],
        input_types: vec![RuntimePureInputType::Value],
        output_type: RuntimePureOutputType::Value,
        expr: RuntimeExpr::Local("label".to_owned()),
        scalar_eval_supported: false,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "ready"
    ));
    assert_eq!(result.stats.pure.pure_calls, 1);
    assert_eq!(result.stats.pure.vm_calls, 1);
    assert_eq!(result.stats.pure.arg_stack_packs, 0);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(result.stats.pure.arg_bytes_copied, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        std::mem::size_of::<RuntimeValue>()
    );
}

#[test]
fn engine_batches_bracket_sequence_pure_calls() {
    let pure_call = |base, bonus| RuntimeExpr::PureCall {
        helper: RuntimePureHelperId(0),
        args: vec![
            RuntimeExpr::Value(RuntimeValue::i64(base)),
            RuntimeExpr::Value(RuntimeValue::i64(bonus)),
        ],
    };
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "seq/i64/3"
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
            RuntimeExpr::Value(RuntimeValue::i64(base)),
            RuntimeExpr::Value(RuntimeValue::i64(bonus)),
        ],
    };
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Map {
                source: Box::new(RuntimeExpr::BracketSeq(vec![
                    RuntimeExpr::Value(RuntimeValue::i64(3)),
                    RuntimeExpr::Value(RuntimeValue::i64(5)),
                    RuntimeExpr::Value(RuntimeValue::i64(7)),
                ])),
                param: "base".to_owned(),
                body: Box::new(RuntimeExpr::PureCall {
                    helper: RuntimePureHelperId(0),
                    args: vec![
                        RuntimeExpr::Local("base".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::i64(4)),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "seq/i64/3"
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
fn engine_fuses_map_closure_pure_batch_sum() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::BracketSeq(vec![
                        RuntimeExpr::Value(RuntimeValue::i64(3)),
                        RuntimeExpr::Value(RuntimeValue::i64(5)),
                        RuntimeExpr::Value(RuntimeValue::i64(7)),
                    ])),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::i64(4)),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

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
    assert_eq!(result.stats.pure.result_bytes_copied, 0);
}

#[test]
fn engine_fuses_local_map_closure_pure_batch_sum() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("values".to_owned()),
                    expr: RuntimeExpr::Value(runtime_sequence_values(vec![
                        RuntimeValue::i64(3),
                        RuntimeValue::i64(5),
                        RuntimeValue::i64(7),
                    ])),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Sum {
                    source: Box::new(RuntimeExpr::Map {
                        source: Box::new(RuntimeExpr::Local("values".to_owned())),
                        param: "base".to_owned(),
                        body: Box::new(RuntimeExpr::PureCall {
                            helper: RuntimePureHelperId(0),
                            args: vec![
                                RuntimeExpr::Local("base".to_owned()),
                                RuntimeExpr::Value(RuntimeValue::i64(4)),
                            ],
                        }),
                    }),
                }),
            ],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 8 },
        },
    );

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "60"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.pure_calls, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
}

fn assert_dense_i64_map_sum_uses_flat_batch(source: RuntimeValue, expected: &str) {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::Value(source)),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::i64(4)),
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
        input_types: vec![RuntimePureInputType::I64, RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == expected
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i64>()
    );
    assert_eq!(result.stats.pure.result_bytes_copied, 0);
}

#[test]
fn engine_batches_dense_i64_map_without_value_materialization() {
    assert_dense_i64_map_sum_uses_flat_batch(runtime_sequence_dense_i64(vec![3, 5, 7]), "60");
}

#[test]
fn engine_batches_dense_i32_map_without_widening_flat_inputs() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::Value(runtime_sequence_dense_i32(vec![
                        3, 5, 7,
                    ]))),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::i32(4)),
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
        name: "score_i32".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::I32, RuntimePureInputType::I32],
        output_type: RuntimePureOutputType::I32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "60"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<i32>()
    );
    assert_eq!(result.stats.pure.result_bytes_copied, 0);
}

#[test]
fn engine_batches_dense_u32_map_without_widening_flat_inputs() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::Value(runtime_sequence_dense_u32(vec![
                        3, 5, 7,
                    ]))),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(RuntimeValue::u32(4)),
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
        name: "score_u32".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::U32, RuntimePureInputType::U32],
        output_type: RuntimePureOutputType::U32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "60"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        6 * std::mem::size_of::<u32>()
    );
    assert_eq!(result.stats.pure.result_bytes_copied, 0);
}

#[test]
fn engine_batches_dense_i16_and_u8_map_without_widening_flat_inputs() {
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_i16(vec![3, 5, 7]),
        RuntimeValue::i16(4),
        RuntimePureInputType::I16,
        RuntimePureOutputType::I16,
        6 * std::mem::size_of::<i16>(),
    );
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_u8(vec![3, 5, 7]),
        RuntimeValue::u8(4),
        RuntimePureInputType::U8,
        RuntimePureOutputType::U8,
        6 * std::mem::size_of::<u8>(),
    );
}

#[test]
fn engine_batches_dense_exact_int_map_outputs_without_widening() {
    assert_dense_exact_int_map_output_uses_flat_batch(
        runtime_sequence_dense_u16(vec![3, 5, 7]),
        RuntimeValue::u16(4),
        RuntimePureInputType::U16,
        RuntimePureOutputType::U16,
        "seq/u16/3",
        6 * std::mem::size_of::<u16>(),
        3 * std::mem::size_of::<u16>(),
    );
    assert_dense_exact_int_map_output_uses_flat_batch(
        runtime_sequence_dense_i8(vec![3, 5, 7]),
        RuntimeValue::i8(4),
        RuntimePureInputType::I8,
        RuntimePureOutputType::I8,
        "seq/i8/3",
        6 * std::mem::size_of::<i8>(),
        3 * std::mem::size_of::<i8>(),
    );
}

#[test]
fn engine_calls_exact_int_pure_helpers_without_value_fallback() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::u16(7)),
                    RuntimeExpr::Value(RuntimeValue::u16(5)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_u16".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![RuntimePureInputType::U16, RuntimePureInputType::U16],
        output_type: RuntimePureOutputType::U16,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "12"
    ));
    assert_eq!(result.stats.pure.pure_calls, 1);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(result.stats.pure.arg_bytes_copied, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        2 * std::mem::size_of::<u16>()
    );
    assert_eq!(result.stats.pure.fallbacks, 0);
}

#[test]
fn engine_calls_typed_float_pure_helpers_without_arg_vec_allocation() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![
                    RuntimeExpr::Value(RuntimeValue::F32(1.5)),
                    RuntimeExpr::Value(RuntimeValue::F32(2.0)),
                ],
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_f32".to_owned(),
        input_names: vec!["base".to_owned(), "gain".to_owned()],
        input_types: vec![RuntimePureInputType::F32, RuntimePureInputType::F32],
        output_type: RuntimePureOutputType::F32,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("gain".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "3"
    ));
    assert_eq!(result.stats.pure.pure_calls, 1);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(result.stats.pure.arg_bytes_copied, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        2 * std::mem::size_of::<f32>()
    );
}

#[test]
fn engine_batches_dense_float_map_outputs_without_value_materialization() {
    assert_dense_float_map_output_uses_flat_batch(
        runtime_sequence_dense_f32(vec![1.5, 2.0, 2.5]),
        RuntimeValue::F32(2.0),
        RuntimePureInputType::F32,
        RuntimePureOutputType::F32,
        "seq/f32/3",
        6 * std::mem::size_of::<f32>(),
        3 * std::mem::size_of::<f32>(),
    );
    assert_dense_float_map_output_uses_flat_batch(
        runtime_sequence_dense_f64(vec![1.5, 2.0, 2.5]),
        RuntimeValue::F64(2.0),
        RuntimePureInputType::F64,
        RuntimePureOutputType::F64,
        "seq/f64/3",
        6 * std::mem::size_of::<f64>(),
        3 * std::mem::size_of::<f64>(),
    );
}

#[test]
fn engine_batches_dense_u64_map_without_widening_flat_inputs() {
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_u64(vec![3, 5, 7]),
        RuntimeValue::u64(4),
        RuntimePureInputType::U64,
        RuntimePureOutputType::U64,
        6 * std::mem::size_of::<u64>(),
    );
}

#[test]
fn engine_batches_dense_i128_and_u128_map_without_widening_flat_inputs() {
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_i128(vec![3, 5, 7]),
        RuntimeValue::i128(4),
        RuntimePureInputType::I128,
        RuntimePureOutputType::I128,
        6 * std::mem::size_of::<i128>(),
    );
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_u128(vec![3, 5, 7]),
        RuntimeValue::u128(4),
        RuntimePureInputType::U128,
        RuntimePureOutputType::U128,
        6 * std::mem::size_of::<u128>(),
    );
}

#[test]
fn engine_batches_dense_isize_map_without_widening_flat_inputs() {
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_isize(vec![3, 5, 7]),
        RuntimeValue::isize(4),
        RuntimePureInputType::ISize,
        RuntimePureOutputType::ISize,
        6 * std::mem::size_of::<i64>(),
    );
    assert_dense_exact_int_map_output_uses_flat_batch(
        runtime_sequence_dense_isize(vec![3, 5, 7]),
        RuntimeValue::isize(4),
        RuntimePureInputType::ISize,
        RuntimePureOutputType::ISize,
        "seq/isize/3",
        6 * std::mem::size_of::<i64>(),
        3 * std::mem::size_of::<i64>(),
    );
}

#[test]
fn engine_batches_dense_usize_map_without_widening_flat_inputs() {
    assert_dense_exact_int_map_sum_uses_flat_batch(
        runtime_sequence_dense_usize(vec![3, 5, 7]),
        RuntimeValue::usize(4),
        RuntimePureInputType::USize,
        RuntimePureOutputType::USize,
        6 * std::mem::size_of::<u64>(),
    );
    assert_dense_exact_int_map_output_uses_flat_batch(
        runtime_sequence_dense_usize(vec![3, 5, 7]),
        RuntimeValue::usize(4),
        RuntimePureInputType::USize,
        RuntimePureOutputType::USize,
        "seq/usize/3",
        6 * std::mem::size_of::<u64>(),
        3 * std::mem::size_of::<u64>(),
    );
}

fn assert_dense_exact_int_map_sum_uses_flat_batch(
    source: RuntimeValue,
    bonus: RuntimeValue,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    expected_borrowed_bytes: usize,
) {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Sum {
                source: Box::new(RuntimeExpr::Map {
                    source: Box::new(RuntimeExpr::Value(source)),
                    param: "base".to_owned(),
                    body: Box::new(RuntimeExpr::PureCall {
                        helper: RuntimePureHelperId(0),
                        args: vec![
                            RuntimeExpr::Local("base".to_owned()),
                            RuntimeExpr::Value(bonus),
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
        name: "score_exact".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![input_type, input_type],
        output_type,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "60"
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        expected_borrowed_bytes
    );
    assert_eq!(result.stats.pure.result_bytes_copied, 0);
}

fn assert_dense_exact_int_map_output_uses_flat_batch(
    source: RuntimeValue,
    bonus: RuntimeValue,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    expected_return: &str,
    expected_borrowed_bytes: usize,
    expected_result_bytes: usize,
) {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Map {
                source: Box::new(RuntimeExpr::Value(source)),
                param: "base".to_owned(),
                body: Box::new(RuntimeExpr::PureCall {
                    helper: RuntimePureHelperId(0),
                    args: vec![
                        RuntimeExpr::Local("base".to_owned()),
                        RuntimeExpr::Value(bonus),
                    ],
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_exact".to_owned(),
        input_names: vec!["base".to_owned(), "bonus".to_owned()],
        input_types: vec![input_type, input_type],
        output_type,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("bonus".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == expected_return
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        expected_borrowed_bytes
    );
    assert_eq!(result.stats.pure.result_bytes_copied, expected_result_bytes);
}

fn assert_dense_float_map_output_uses_flat_batch(
    source: RuntimeValue,
    gain: RuntimeValue,
    input_type: RuntimePureInputType,
    output_type: RuntimePureOutputType,
    expected_return: &str,
    expected_borrowed_bytes: usize,
    expected_result_bytes: usize,
) {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Map {
                source: Box::new(RuntimeExpr::Value(source)),
                param: "base".to_owned(),
                body: Box::new(RuntimeExpr::PureCall {
                    helper: RuntimePureHelperId(0),
                    args: vec![
                        RuntimeExpr::Local("base".to_owned()),
                        RuntimeExpr::Value(gain),
                    ],
                }),
            })],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid")
    .with_pure_helpers(vec![RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "score_float".to_owned(),
        input_names: vec!["base".to_owned(), "gain".to_owned()],
        input_types: vec![input_type, input_type],
        output_type,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("base".to_owned())),
            op: RuntimeBinaryOp::Mul,
            rhs: Box::new(RuntimeExpr::Local("gain".to_owned())),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Annotated,
    }]);
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == expected_return
    ));
    assert_eq!(result.stats.pure.batch_calls, 1);
    assert_eq!(result.stats.pure.batch_items, 3);
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
    assert_eq!(
        result.stats.pure.arg_bytes_borrowed,
        expected_borrowed_bytes
    );
    assert_eq!(result.stats.pure.result_bytes_copied, expected_result_bytes);
}

#[test]
fn engine_keeps_dynamic_homogeneous_textual_sequences_dense() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("label".to_owned()),
                    expr: RuntimeExpr::Value(RuntimeValue::String("alpha".to_owned())),
                },
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("target".to_owned()),
                    expr: RuntimeExpr::EntityRef("character.alice".to_owned()),
                },
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("names".to_owned()),
                    expr: RuntimeExpr::BracketSeq(vec![
                        RuntimeExpr::Local("label".to_owned()),
                        RuntimeExpr::Value(RuntimeValue::String("beta".to_owned())),
                    ]),
                },
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("targets".to_owned()),
                    expr: RuntimeExpr::BracketSeq(vec![
                        RuntimeExpr::Local("target".to_owned()),
                        RuntimeExpr::EntityRef("character.bob".to_owned()),
                    ]),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Tuple(vec![
                    RuntimeExpr::MethodCall {
                        receiver: Box::new(RuntimeExpr::Local("names".to_owned())),
                        method: "len".to_owned(),
                        args: Vec::new(),
                    },
                    RuntimeExpr::MethodCall {
                        receiver: Box::new(RuntimeExpr::Local("targets".to_owned())),
                        method: "len".to_owned(),
                        args: Vec::new(),
                    },
                ])),
            ],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 16 },
        },
    );

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "tuple/2"
    ));
    assert!(matches!(
        engine.fiber().env.get("names"),
        Some(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::Strings(values))))
            if values.as_slice() == ["alpha".to_owned(), "beta".to_owned()].as_slice()
    ));
    assert!(matches!(
        engine.fiber().env.get("targets"),
        Some(RuntimeValue::Seq(RuntimeSeq::Dense(DenseSeq::EntityRefs(values))))
            if values.as_slice() == ["character.alice".to_owned(), "character.bob".to_owned()].as_slice()
    ));
    assert_eq!(result.stats.pure.arg_vec_allocations, 0);
}

#[test]
fn engine_sums_local_i64_sequence_by_borrow() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
            ops: vec![
                FlowOp::Let {
                    pattern: RuntimePattern::Ident("scores".to_owned()),
                    expr: RuntimeExpr::BracketSeq(vec![
                        RuntimeExpr::Value(RuntimeValue::i64(18)),
                        RuntimeExpr::Value(RuntimeValue::i64(15)),
                        RuntimeExpr::Value(RuntimeValue::i64(20)),
                    ]),
                },
                FlowOp::ReturnExpr(RuntimeExpr::Sum {
                    source: Box::new(RuntimeExpr::Local("scores".to_owned())),
                }),
            ],
        }],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let result = engine.step(
        RuntimeStepInput::default(),
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 8 },
        },
    );

    assert!(matches!(
        result.fiber_status,
        FlowFiberStatus::Done(FlowExit::Return(ref value)) if value == "53"
    ));
    assert_eq!(result.stats.pure.pure_calls, 0);
}

#[test]
fn engine_runs_flow_thread_body_as_child_fiber() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
        target: Some(super::flow_id("flow.listen")),
        out: None,
        effects: Vec::new(),
    };
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![
            RuntimeFlow {
                id: super::flow_id("flow.opening"),
                ops: vec![FlowOp::Choice {
                    id: Some("choice.opening".to_owned()),
                    options: vec![option.clone()],
                }],
            },
            RuntimeFlow {
                id: super::flow_id("flow.listen"),
                ops: vec![FlowOp::Return("listen".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("choice plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let presented = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        presented.flow_events,
        vec![FlowEvent::ChoicePresented {
            id: Some("choice.opening".to_owned()),
            options: vec![option],
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Choice(_)));

    let selected = super::runtime_step(
        &mut engine,
        RuntimeStepInput {
            input_events: vec![super::input_event("choice", Some("choice.listen"))],
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
                target: super::flow_id("flow.listen")
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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.opening"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
                kind: TaskEventKind::Ready(RuntimePayload::from("bg_handle")),
            }],
            ..RuntimeStepInput::default()
        },
    );
    assert!(ready.flow_events.iter().any(|event| matches!(
        event,
        FlowEvent::AwaitReady { value, .. } if value.label() == "bg_handle"
    )));
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Running));
}

#[test]
fn engine_runs_bounded_await_many_tasks_in_source_order() {
    let mut engine = super::engine_for_test_plan(await_many_read_plan());

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
            value: RuntimePayload::new(runtime_sequence_values(vec![
                RuntimeValue::String("A".to_owned()),
                RuntimeValue::String("B".to_owned()),
                RuntimeValue::String("C".to_owned()),
            ])),
        })
    );

    let returned = super::runtime_step(&mut engine, RuntimeStepInput::default());
    assert_eq!(
        returned.flow_events,
        vec![FlowEvent::Return {
            value: "seq/values/3".to_owned()
        }]
    );
}

fn await_many_read_plan() -> RuntimePlan {
    super::runtime_plan(
        Some(super::flow_id("flow.main")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.main"),
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
        kind: TaskEventKind::Ready(RuntimePayload::from(value)),
    }
}

#[test]
fn engine_binds_runtime_values_and_gotos_entity_refs() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![
            RuntimeFlow {
                id: super::flow_id("flow.opening"),
                ops: vec![
                    FlowOp::Let {
                        pattern: RuntimePattern::Ident("route".to_owned()),
                        expr: RuntimeExpr::EntityRef("flow.next".to_owned()),
                    },
                    FlowOp::GotoExpr(RuntimeExpr::Local("route".to_owned())),
                ],
            },
            RuntimeFlow {
                id: super::flow_id("flow.next"),
                ops: vec![FlowOp::Return("done".to_owned())],
            },
        ],
        Vec::new(),
    )
    .expect("flow plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    assert!(
        super::runtime_step(&mut engine, RuntimeStepInput::default())
            .flow_events
            .is_empty()
    );
    let goto = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert_eq!(
        goto.flow_events,
        vec![FlowEvent::Goto {
            target: super::flow_id("flow.next")
        }]
    );
}

#[test]
fn engine_runs_if_and_match_blocks_from_runtime_values() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![
            RuntimeFlow {
                id: super::flow_id("flow.opening"),
                ops: vec![FlowOp::If {
                    condition: RuntimeExpr::Local("ready".to_owned()),
                    then_ops: vec![FlowOp::Goto(super::flow_id("flow.match"))],
                    else_ops: vec![FlowOp::Return("wait".to_owned())],
                }],
            },
            RuntimeFlow {
                id: super::flow_id("flow.match"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
            target: super::flow_id("flow.match")
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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.loop")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.loop"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.while")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.while"),
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
    let mut engine = super::engine_for_test_plan(plan);
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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.for")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.for"),
            ops: vec![FlowOp::For {
                pattern: RuntimePattern::Ident("item".to_owned()),
                source: RuntimeExpr::BracketSeq(vec![
                    RuntimeExpr::Value(RuntimeValue::i64(1)),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
                    RuntimeExpr::Value(RuntimeValue::i64(3)),
                    RuntimeExpr::Value(RuntimeValue::i64(4)),
                ]),
                evidence: crate::plan::RuntimeIteratorEvidence::builtin_seq(),
                body: vec![FlowOp::Effect(call("observe.item"))],
            }],
        }],
        Vec::new(),
    )
    .expect("for plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let first = engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());

    assert_eq!(first.stats.executed_ops, 1);
    assert_eq!(first.stats.pending_ops_after, 3);
}

#[test]
fn branch_pattern_bindings_do_not_leak_after_branch_scope() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.branch")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.branch"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.dup")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.dup"),
            ops: vec![FlowOp::Let {
                pattern: RuntimePattern::Tuple(vec![
                    RuntimePattern::Ident("x".to_owned()),
                    RuntimePattern::Ident("x".to_owned()),
                ]),
                expr: RuntimeExpr::Tuple(vec![
                    RuntimeExpr::Value(RuntimeValue::i64(1)),
                    RuntimeExpr::Value(RuntimeValue::i64(2)),
                ]),
            }],
        }],
        Vec::new(),
    )
    .expect("duplicate pattern plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());

    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message
            .contains("pattern binds `x` more than once")
    }));
    assert!(engine.fiber().env.get("x").is_none());
}

#[test]
fn runtime_pattern_binding_capacity_counts_nested_bindings() {
    let pattern = RuntimePattern::Tuple(vec![
        RuntimePattern::Ident("head".to_owned()),
        RuntimePattern::BracketSeq {
            items: vec![
                RuntimePattern::Discard,
                RuntimePattern::MutIdent("item".to_owned()),
            ],
            rest: Some("tail".to_owned()),
        },
        RuntimePattern::Whole {
            name: "whole".to_owned(),
            pattern: Box::new(RuntimePattern::Variant {
                path: None,
                name: "Some".to_owned(),
                payload: Some(Box::new(RuntimePattern::Typed {
                    name: "payload".to_owned(),
                    ty: "String".to_owned(),
                })),
            }),
        },
    ]);

    assert_eq!(pattern_binding_capacity(&pattern), 5);
}

#[test]
fn typed_runtime_patterns_match_value_shape() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.typed")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.typed"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
fn typed_runtime_patterns_use_canonical_primitive_labels() {
    let bool_pattern = RuntimePattern::Typed {
        name: "ok".to_owned(),
        ty: "bool".to_owned(),
    };
    let old_bool_pattern = RuntimePattern::Typed {
        name: "ok".to_owned(),
        ty: "Bool".to_owned(),
    };
    let char_pattern = RuntimePattern::Typed {
        name: "ch".to_owned(),
        ty: "char".to_owned(),
    };
    let old_char_pattern = RuntimePattern::Typed {
        name: "ch".to_owned(),
        ty: "Char".to_owned(),
    };

    assert!(
        match_runtime_pattern(&bool_pattern, &RuntimeValue::Bool(true))
            .expect("canonical bool typed pattern matches")
            .is_some()
    );
    assert!(
        match_runtime_pattern(&old_bool_pattern, &RuntimeValue::Bool(true))
            .expect("old Bool label is still a valid typed pattern")
            .is_none()
    );
    assert!(
        match_runtime_pattern(&char_pattern, &RuntimeValue::Char('a'))
            .expect("canonical char typed pattern matches")
            .is_some()
    );
    assert!(
        match_runtime_pattern(&old_char_pattern, &RuntimeValue::Char('a'))
            .expect("old Char label is still a valid typed pattern")
            .is_none()
    );
}

#[test]
fn fs_write_dispatches_string_and_bytes_payloads() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.write")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.write"),
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
                                RuntimeExpr::Value(runtime_sequence_dense_bytes(vec![1, 2])),
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
    let mut engine = super::engine_for_test_plan(plan);

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
                kind: TaskEventKind::Ready(RuntimePayload::from("ok")),
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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.spread")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.spread"),
            ops: vec![FlowOp::ReturnExpr(RuntimeExpr::Call {
                callee: RuntimeCallTarget::intrinsic(RuntimeIntrinsic::Add),
                args: vec![RuntimeExpr::SpreadArg(Box::new(RuntimeExpr::BracketSeq(
                    vec![
                        RuntimeExpr::Value(RuntimeValue::i64(20)),
                        RuntimeExpr::Value(RuntimeValue::i64(22)),
                    ],
                )))],
            })],
        }],
        Vec::new(),
    )
    .expect("runtime plan is valid");
    let mut engine = super::engine_for_test_plan(plan);
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
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.log")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.log"),
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
                                RuntimeExpr::Value(RuntimeValue::i64(3)),
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
    let mut engine = super::engine_for_test_plan(plan);

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());
    let HostTaskRequest::Custom {
        capability,
        operation,
        args,
        ..
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
            RuntimePayload::new(RuntimeValue::i64(3)),
            RuntimePayload::new(RuntimeValue::Duration(LogicalDuration::from_nanos(
                120_000_000
            ))),
            RuntimePayload::new(RuntimeValue::EntityRef("asset.bg.room".to_owned())),
        ]
    );
}

#[test]
fn custom_host_request_preserves_nested_record_variant_and_refs() {
    let nested_payload = RuntimeValue::Record(vec![
        RuntimeFieldValue {
            name: "actor".to_owned(),
            value: RuntimeValue::EntityRef("character.alice".to_owned()),
        },
        RuntimeFieldValue {
            name: "state".to_owned(),
            value: RuntimeValue::Variant {
                path: Some("Emotion".to_owned()),
                name: "Focused".to_owned(),
                payload: Some(Box::new(RuntimeValue::Tuple(vec![
                    RuntimeValue::i32(3),
                    RuntimeValue::String("line.ready".to_owned()),
                ]))),
            },
        },
        RuntimeFieldValue {
            name: "routes".to_owned(),
            value: runtime_sequence_values(vec![
                RuntimeValue::EntityRef("flow.next".to_owned()),
                RuntimeValue::EntityRef("flow.fallback".to_owned()),
            ]),
        },
    ]);
    let expected = RuntimePayload::new(nested_payload.clone());
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.inspect")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.inspect"),
            ops: vec![FlowOp::Await {
                binding: None,
                target: AwaitTarget {
                    need: NeedId("need.inspect".to_owned()),
                    task: TaskId("task.inspect".to_owned()),
                    request: HostTaskRequestTemplate::new(
                        "adapter",
                        "inspect",
                        [HostTaskArgTemplate::positional(RuntimeExpr::Value(
                            nested_payload,
                        ))],
                    ),
                },
                pending: Vec::new(),
            }],
        }],
        Vec::new(),
    )
    .expect("custom host request plan is valid");
    let mut engine = super::engine_for_test_plan(plan);

    let output = super::runtime_step(&mut engine, RuntimeStepInput::default());
    let HostTaskRequest::Custom { args, .. } = &output.requests.tasks[0].request else {
        panic!("expected custom host request");
    };

    assert_eq!(args, &vec![expected]);
}

#[test]
fn if_let_expression_binds_only_success_branch() {
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.if_let")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.if_let"),
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
    let mut engine = super::engine_for_test_plan(plan);

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
            target: super::flow_id("flow.next")
        }]
    );
    assert!(engine.fiber().env.get("route").is_none());
}
