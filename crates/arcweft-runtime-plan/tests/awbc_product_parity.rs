use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::line_task::{LineTaskGroup, LineTaskNode, LineTaskScope};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan, RuntimePureHelper,
    RuntimePureHelperId, RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::source::{
    RuntimeSourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan,
    SourcePolicy, SourceRuntimeState,
};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::stream::{StreamOp, StreamPlan, StreamRuntimeId};
use arcweft_core::task::{
    AwaitManyTarget, AwaitTarget, HostCapabilityId, HostTaskArgTemplate, HostTaskRequestTemplate,
    LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId, TaskSequence,
};
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeExpr, RuntimePayload, RuntimeSeq,
    RuntimeValue,
};
use arcweft_interaction_model::{
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;
use std::collections::BTreeMap;

#[derive(Debug)]
struct ParityStep {
    structured: RuntimeStepResult,
    awbc: RuntimeStepResult,
    structured_sources: BTreeMap<SourceId, SourceRuntimeState>,
    awbc_sources: BTreeMap<SourceId, SourceRuntimeState>,
}

fn run_parity(plan: RuntimePlan, inputs: Vec<RuntimeStepInput>) -> Vec<ParityStep> {
    run_parity_with_options(
        plan,
        &[],
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
        },
        inputs,
    )
}

fn run_parity_with_root_bindings(
    plan: RuntimePlan,
    root_bindings: &[RuntimeBinding],
    inputs: Vec<RuntimeStepInput>,
) -> Vec<ParityStep> {
    run_parity_with_options(
        plan,
        root_bindings,
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 128 },
        },
        inputs,
    )
}

fn run_parity_with_options(
    plan: RuntimePlan,
    root_bindings: &[RuntimeBinding],
    options: RuntimeStepOptions,
    inputs: Vec<RuntimeStepInput>,
) -> Vec<ParityStep> {
    let display = LineDisplayCatalog::default();
    let awbc = AwbcLowerer::new(&plan, &display, "awbc_product_parity.arcw")
        .lower()
        .expect("runtime plan lowers to AWBC")
        .program;
    assert!(
        awbc.product_step_parity_blockers().is_empty(),
        "ordinary lowered fixture must have no static product blockers"
    );
    let mut structured =
        ArcweftRuntimeExecutor::from_runtime_plan(plan, ArcweftExecutionTier::StructuredVm);
    let mut awbc =
        ArcweftRuntimeExecutor::from_awbc_product(awbc, arcweft_core::awbc::schema::AwbcEntryId(0))
            .expect("AWBC product executor builds");
    let mut structured_backend = arcweft_core::pure::VmRuntimePureCallBackend::default();
    let mut awbc_backend = arcweft_core::pure::VmRuntimePureCallBackend::default();

    inputs
        .into_iter()
        .map(|input| {
            let structured_result = structured.step_with_root_bindings_and_pure_backend(
                input.clone(),
                root_bindings,
                options,
                &mut structured_backend,
            );
            let awbc_result = awbc.step_with_root_bindings_and_pure_backend(
                input,
                root_bindings,
                options,
                &mut awbc_backend,
            );
            ParityStep {
                structured: structured_result,
                awbc: awbc_result,
                structured_sources: structured.fiber().source_states.clone(),
                awbc_sources: awbc.fiber().source_states.clone(),
            }
        })
        .collect()
}

fn assert_step_boundary_eq(step: &ParityStep) {
    assert_eq!(
        step.awbc.output, step.structured.output,
        "RuntimeStepOutput mismatch"
    );
    assert_eq!(
        step.awbc.stop_reason, step.structured.stop_reason,
        "RuntimeStepStopReason mismatch"
    );
    assert_eq!(
        normalized_status(&step.awbc.fiber_status),
        normalized_status(&step.structured.fiber_status),
        "FlowFiberStatus mismatch"
    );
    assert_eq!(
        step.awbc.stats.diagnostics, step.structured.stats.diagnostics,
        "diagnostic stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.line_effects, step.structured.stats.line_effects,
        "line-effect stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.task_events_in, step.structured.stats.task_events_in,
        "task-events-in stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.source_events_in, step.structured.stats.source_events_in,
        "source-events-in stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.source_events_emitted, step.structured.stats.source_events_emitted,
        "source-events-emitted stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.stream_events_emitted, step.structured.stats.stream_events_emitted,
        "stream-events-emitted stat mismatch"
    );
    assert_eq!(
        step.awbc.stats.audio_commands, step.structured.stats.audio_commands,
        "audio-command stat mismatch"
    );
    assert_eq!(
        step.awbc_sources, step.structured_sources,
        "source state mismatch"
    );
}

fn normalized_status(status: &FlowFiberStatus) -> FlowFiberStatus {
    match status.clone() {
        FlowFiberStatus::Dialogue(mut state) => {
            state.resume = None;
            FlowFiberStatus::Dialogue(state)
        }
        FlowFiberStatus::Choice(mut state) => {
            state.resume = None;
            FlowFiberStatus::Choice(state)
        }
        FlowFiberStatus::Waiting(mut state) => {
            state.resume = None;
            state.binding = None;
            state.target.request = host_template("normalized", "await");
            FlowFiberStatus::Waiting(state)
        }
        FlowFiberStatus::WaitingMany(mut state) => {
            state.resume = None;
            state.binding = None;
            state.target.source = RuntimeExpr::Value(RuntimeValue::Unit);
            state.target.request = host_template("normalized", "await_many");
            FlowFiberStatus::WaitingMany(state)
        }
        status => status,
    }
}

fn flow(ops: Vec<FlowOp>) -> RuntimePlan {
    RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops,
        }],
        Vec::new(),
    )
    .expect("runtime plan builds")
}

fn input_event(kind: &str, payload: Option<&str>) -> RoutedInputEvent {
    let mut event = RoutedInputEvent::new(
        InputEpoch::default(),
        InputSequence::default(),
        InteractionTarget::new("runtime").expect("runtime target"),
        InputEventKind::Custom {
            name: Identifier::new(kind).expect("input kind"),
        },
    );
    if let Some(payload) = payload {
        event = event.with_payload(InteractionPayload::Text(payload.to_owned()));
    }
    event
}

fn call(name: &str) -> LineEffectRequest {
    LineEffectRequest::Call(RuntimeCall {
        callee: name.to_owned(),
        args: Vec::new(),
    })
}

fn binding(name: &str, value: RuntimeValue) -> RuntimeBinding {
    RuntimeBinding {
        name: name.to_owned(),
        value,
    }
}

fn host_template(capability: &str, operation: &str) -> HostTaskRequestTemplate {
    HostTaskRequestTemplate {
        capability: HostCapabilityId(capability.to_owned()),
        operation: operation.to_owned(),
        args: Vec::new(),
    }
}

#[test]
fn awbc_product_parity_entry() {
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_entry_local_bindings() {
    let steps = run_parity(
        flow(vec![
            FlowOp::Let {
                pattern: RuntimePattern::Ident("total".to_owned()),
                expr: RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(5))),
                },
            },
            FlowOp::ReturnExpr(RuntimeExpr::Local("total".to_owned())),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_entry_root_bindings_named_equivalent() {
    let plan = flow(vec![FlowOp::ReturnExpr(RuntimeExpr::Binary {
        lhs: Box::new(RuntimeExpr::Local("left".to_owned())),
        op: RuntimeBinaryOp::Add,
        rhs: Box::new(RuntimeExpr::Local("right".to_owned())),
    })]);
    let steps = run_parity_with_root_bindings(
        plan,
        &[
            binding("right", RuntimeValue::i64(5)),
            binding("left", RuntimeValue::i64(2)),
        ],
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_pure_intrinsic() {
    let helper = RuntimePureHelper {
        id: RuntimePureHelperId(0),
        name: "helper.add_one".to_owned(),
        input_names: vec!["x".to_owned()],
        input_types: vec![RuntimePureInputType::I64],
        output_type: RuntimePureOutputType::I64,
        expr: RuntimeExpr::Binary {
            lhs: Box::new(RuntimeExpr::Local("x".to_owned())),
            op: RuntimeBinaryOp::Add,
            rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(1))),
        },
        scalar_eval_supported: true,
        origin: RuntimePureHelperOrigin::Inferred,
    };
    let plan = flow(vec![
        FlowOp::Let {
            pattern: RuntimePattern::Ident("pure_value".to_owned()),
            expr: RuntimeExpr::PureCall {
                helper: RuntimePureHelperId(0),
                args: vec![RuntimeExpr::Value(RuntimeValue::i64(41))],
            },
        },
        FlowOp::ReturnExpr(RuntimeExpr::Call {
            callee: RuntimeCallTarget::intrinsic(arcweft_core::value::RuntimeIntrinsic::Add),
            args: vec![
                RuntimeExpr::Local("pure_value".to_owned()),
                RuntimeExpr::Value(RuntimeValue::i64(1)),
            ],
        }),
    ])
    .with_pure_helpers(vec![helper]);
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_dialogue() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("opening_line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.main".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.main".to_owned()),
            ops: vec![
                FlowOp::Dialogue {
                    line: "line.opening.001".into(),
                    task_group: 0,
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        vec![group],
    )
    .expect("runtime plan builds");
    let steps = run_parity(
        plan,
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput::default(),
            RuntimeStepInput {
                input_events: vec![input_event("advance", Some("line.opening.001"))],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput::default(),
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_choice() {
    let option = ChoiceRuntimeOption {
        id: Some("choice.listen".to_owned()),
        label: "Listen".to_owned(),
        target: None,
        out: None,
        effects: Vec::new(),
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::Choice {
                id: Some("choice.opening".to_owned()),
                options: vec![option],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                input_events: vec![input_event("choice", Some("choice.listen"))],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput::default(),
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_choice_invalid_then_valid() {
    let option = ChoiceRuntimeOption {
        id: Some("choice.listen".to_owned()),
        label: "Listen".to_owned(),
        target: None,
        out: None,
        effects: Vec::new(),
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::Choice {
                id: Some("choice.opening".to_owned()),
                options: vec![option],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                input_events: vec![input_event("choice", Some("choice.missing"))],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput {
                input_events: vec![input_event("choice", Some("choice.listen"))],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput::default(),
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await() {
    let target = AwaitTarget {
        need: NeedId("need.load".to_owned()),
        task: TaskId("task.load".to_owned()),
        request: host_template("probe", "load"),
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::Await {
                binding: Some(RuntimePattern::Ident("loaded".to_owned())),
                target,
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.load".to_owned()),
                    sequence: TaskSequence(0),
                    kind: TaskEventKind::Progress(RuntimePayload(RuntimeValue::String(
                        "half".to_owned(),
                    ))),
                }],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.load".to_owned()),
                    sequence: TaskSequence(1),
                    kind: TaskEventKind::Ready(RuntimePayload(RuntimeValue::String(
                        "ok".to_owned(),
                    ))),
                }],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput::default(),
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await_error() {
    let target = AwaitTarget {
        need: NeedId("need.load".to_owned()),
        task: TaskId("task.load".to_owned()),
        request: host_template("probe", "load"),
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::Await {
                binding: Some(RuntimePattern::Ident("loaded".to_owned())),
                target,
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.load".to_owned()),
                    sequence: TaskSequence(0),
                    kind: TaskEventKind::Err("host failed".to_owned()),
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await_cancel() {
    let target = AwaitTarget {
        need: NeedId("need.load".to_owned()),
        task: TaskId("task.load".to_owned()),
        request: host_template("probe", "load"),
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::Await {
                binding: Some(RuntimePattern::Ident("loaded".to_owned())),
                target,
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.load".to_owned()),
                    sequence: TaskSequence(0),
                    kind: TaskEventKind::Cancelled,
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_host() {
    awbc_product_parity_await();
}

#[test]
fn awbc_product_parity_effect() {
    let steps = run_parity(
        flow(vec![
            FlowOp::Effect(call("effect.probe")),
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default(), RuntimeStepInput::default()],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_source_stream() {
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]),
        vec![RuntimeStepInput {
            source_events: vec![RuntimeSourceEvent {
                source: SourceId("source.test".to_owned()),
                sequence: TaskSequence(0),
                kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                    "source-item".to_owned(),
                ))),
            }],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_source_duplicate_sequence_items() {
    let source = SourceId("source.test".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]),
        vec![RuntimeStepInput {
            source_events: vec![
                RuntimeSourceEvent {
                    source: source.clone(),
                    sequence: TaskSequence(0),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "first".to_owned(),
                    ))),
                },
                RuntimeSourceEvent {
                    source: source.clone(),
                    sequence: TaskSequence(0),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "second".to_owned(),
                    ))),
                },
            ],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(steps[0].structured.output.effects.source_events.len(), 2);
    let queue = steps[0]
        .structured_sources
        .get(&source)
        .expect("source state exists")
        .queue
        .iter()
        .map(RuntimePayload::label)
        .collect::<Vec<_>>();
    assert_eq!(queue, vec!["second"]);
}

#[test]
fn awbc_product_parity_source_lower_sequence_cross_step() {
    let source = SourceId("source.test".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]),
        vec![
            RuntimeStepInput {
                source_events: vec![RuntimeSourceEvent {
                    source: source.clone(),
                    sequence: TaskSequence(2),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "newer".to_owned(),
                    ))),
                }],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput {
                source_events: vec![RuntimeSourceEvent {
                    source: source.clone(),
                    sequence: TaskSequence(1),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "late".to_owned(),
                    ))),
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
    let queue = steps[1]
        .structured_sources
        .get(&source)
        .expect("source state exists")
        .queue
        .iter()
        .map(RuntimePayload::label)
        .collect::<Vec<_>>();
    assert_eq!(queue, vec!["late"]);
}

#[test]
fn awbc_product_parity_source_handler_closes_later_source() {
    let driver = SourceId("source.driver".to_owned());
    let target = SourceId("source.target".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
            Vec::new(),
            vec![
                SourcePlan {
                    id: driver.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("driver".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: vec![SourceHandlerPlan::Disconnected {
                        ops: vec![SourceOp::Close(target.clone())],
                    }],
                },
                SourcePlan {
                    id: target.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("target".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: Vec::new(),
                },
            ],
        ),
        vec![RuntimeStepInput {
            source_events: vec![RuntimeSourceEvent {
                source: driver,
                sequence: TaskSequence(0),
                kind: SourceEventKind::Disconnected,
            }],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.requests.source_close,
        vec![target]
    );
}

#[test]
fn awbc_product_parity_source_item_handler_respects_pattern() {
    let driver = SourceId("source.driver".to_owned());
    let target = SourceId("source.target".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
            Vec::new(),
            vec![
                SourcePlan {
                    id: driver.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("driver".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: vec![SourceHandlerPlan::Item {
                        pattern: RuntimePattern::Literal(RuntimeValue::String("run".to_owned())),
                        ops: vec![SourceOp::Close(target.clone())],
                    }],
                },
                SourcePlan {
                    id: target.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("target".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: Vec::new(),
                },
            ],
        ),
        vec![RuntimeStepInput {
            source_events: vec![
                RuntimeSourceEvent {
                    source: driver.clone(),
                    sequence: TaskSequence(0),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "ignore".to_owned(),
                    ))),
                },
                RuntimeSourceEvent {
                    source: driver,
                    sequence: TaskSequence(1),
                    kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                        "run".to_owned(),
                    ))),
                },
            ],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.requests.source_close,
        vec![target]
    );
}

#[test]
fn awbc_product_parity_source_progress_handler_respects_pattern() {
    let driver = SourceId("source.driver".to_owned());
    let target = SourceId("source.target".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
            Vec::new(),
            vec![
                SourcePlan {
                    id: driver.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("driver".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: vec![SourceHandlerPlan::Progress {
                        pattern: RuntimePattern::Literal(RuntimeValue::String("ready".to_owned())),
                        ops: vec![SourceOp::Close(target.clone())],
                    }],
                },
                SourcePlan {
                    id: target.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("target".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: Vec::new(),
                },
            ],
        ),
        vec![RuntimeStepInput {
            source_events: vec![
                RuntimeSourceEvent {
                    source: driver.clone(),
                    sequence: TaskSequence(0),
                    kind: SourceEventKind::Progress("warming".to_owned()),
                },
                RuntimeSourceEvent {
                    source: driver,
                    sequence: TaskSequence(1),
                    kind: SourceEventKind::Progress("ready".to_owned()),
                },
            ],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.requests.source_close,
        vec![target]
    );
}

#[test]
fn awbc_product_parity_source_error_handler_respects_pattern() {
    let driver = SourceId("source.driver".to_owned());
    let target = SourceId("source.target".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
            Vec::new(),
            vec![
                SourcePlan {
                    id: driver.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("driver".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: vec![SourceHandlerPlan::Error {
                        pattern: RuntimePattern::Literal(RuntimeValue::String(
                            "recoverable".to_owned(),
                        )),
                        ops: vec![SourceOp::Close(target.clone())],
                    }],
                },
                SourcePlan {
                    id: target.clone(),
                    item_ty: "Frame".to_owned(),
                    error_ty: "SourceError".to_owned(),
                    from: RuntimeExpr::Value(RuntimeValue::String("target".to_owned())),
                    policy: SourcePolicy::default(),
                    handlers: Vec::new(),
                },
            ],
        ),
        vec![RuntimeStepInput {
            source_events: vec![
                RuntimeSourceEvent {
                    source: driver.clone(),
                    sequence: TaskSequence(0),
                    kind: SourceEventKind::Error(RuntimePayload(RuntimeValue::String(
                        "fatal".to_owned(),
                    ))),
                },
                RuntimeSourceEvent {
                    source: driver,
                    sequence: TaskSequence(1),
                    kind: SourceEventKind::Error(RuntimePayload(RuntimeValue::String(
                        "recoverable".to_owned(),
                    ))),
                },
            ],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.requests.source_close,
        vec![target]
    );
}

#[test]
fn awbc_product_parity_source_handler_yields_to_source_queue() {
    let source = SourceId("source.driver".to_owned());
    let steps = run_parity(
        flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
            Vec::new(),
            vec![SourcePlan {
                id: source.clone(),
                item_ty: "Frame".to_owned(),
                error_ty: "SourceError".to_owned(),
                from: RuntimeExpr::Value(RuntimeValue::String("driver".to_owned())),
                policy: SourcePolicy::default(),
                handlers: vec![SourceHandlerPlan::Item {
                    pattern: RuntimePattern::Ident("frame".to_owned()),
                    ops: vec![SourceOp::Yield(RuntimeExpr::Value(RuntimeValue::String(
                        "derived-frame".to_owned(),
                    )))],
                }],
            }],
        ),
        vec![RuntimeStepInput {
            source_events: vec![RuntimeSourceEvent {
                source: source.clone(),
                sequence: TaskSequence(0),
                kind: SourceEventKind::Item(RuntimePayload(RuntimeValue::String(
                    "raw-frame".to_owned(),
                ))),
            }],
            ..RuntimeStepInput::default()
        }],
    );

    assert_step_boundary_eq(&steps[0]);
    let queue = steps[0]
        .structured_sources
        .get(&source)
        .expect("source state exists")
        .queue
        .iter()
        .map(RuntimePayload::label)
        .collect::<Vec<_>>();
    assert_eq!(queue, vec!["derived-frame"]);
}

#[test]
fn awbc_product_parity_stream_yield() {
    let plan = flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
        vec![StreamPlan {
            id: StreamRuntimeId("stream.generated".to_owned()),
            item_ty: "String".to_owned(),
            error_ty: "String".to_owned(),
            ops: vec![
                StreamOp::Yield {
                    expr: RuntimeExpr::Value(RuntimeValue::String("stream-item-0".to_owned())),
                },
                StreamOp::Yield {
                    expr: RuntimeExpr::Value(RuntimeValue::String("stream-item-1".to_owned())),
                },
            ],
        }],
        Vec::new(),
    );
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_stream_yield_then_close() {
    let plan = flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
        vec![StreamPlan {
            id: StreamRuntimeId("stream.generated".to_owned()),
            item_ty: "String".to_owned(),
            error_ty: "String".to_owned(),
            ops: vec![
                StreamOp::Yield {
                    expr: RuntimeExpr::Value(RuntimeValue::String("stream-item-0".to_owned())),
                },
                StreamOp::Yield {
                    expr: RuntimeExpr::Value(RuntimeValue::String("stream-item-1".to_owned())),
                },
                StreamOp::Close {
                    source: RuntimeExpr::Value(RuntimeValue::String("stream.generated".to_owned())),
                },
            ],
        }],
        Vec::new(),
    );
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_multi_stream_yield_and_close() {
    let plan = flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
        vec![
            StreamPlan {
                id: StreamRuntimeId("stream.alpha".to_owned()),
                item_ty: "String".to_owned(),
                error_ty: "String".to_owned(),
                ops: vec![StreamOp::Yield {
                    expr: RuntimeExpr::Value(RuntimeValue::String("alpha-item".to_owned())),
                }],
            },
            StreamPlan {
                id: StreamRuntimeId("stream.beta".to_owned()),
                item_ty: "String".to_owned(),
                error_ty: "String".to_owned(),
                ops: vec![
                    StreamOp::Yield {
                        expr: RuntimeExpr::Value(RuntimeValue::String("beta-item".to_owned())),
                    },
                    StreamOp::Close {
                        source: RuntimeExpr::Value(RuntimeValue::String("stream.beta".to_owned())),
                    },
                ],
            },
        ],
        Vec::new(),
    );
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_stream_closes_source_target() {
    let source = SourceId("source.generated".to_owned());
    let plan = flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
        vec![StreamPlan {
            id: StreamRuntimeId("stream.driver".to_owned()),
            item_ty: "String".to_owned(),
            error_ty: "String".to_owned(),
            ops: vec![StreamOp::Close {
                source: RuntimeExpr::Value(RuntimeValue::String(source.0.clone())),
            }],
        }],
        vec![SourcePlan {
            id: source.clone(),
            item_ty: "Frame".to_owned(),
            error_ty: "SourceError".to_owned(),
            from: RuntimeExpr::Value(RuntimeValue::String("source".to_owned())),
            policy: SourcePolicy::default(),
            handlers: Vec::new(),
        }],
    );
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.requests.source_close,
        vec![source]
    );
}

#[test]
fn awbc_product_parity_one_op() {
    let steps = run_parity_with_options(
        flow(vec![
            FlowOp::Let {
                pattern: RuntimePattern::Ident("total".to_owned()),
                expr: RuntimeExpr::Binary {
                    lhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(2))),
                    op: RuntimeBinaryOp::Add,
                    rhs: Box::new(RuntimeExpr::Value(RuntimeValue::i64(5))),
                },
            },
            FlowOp::ReturnExpr(RuntimeExpr::Local("total".to_owned())),
        ]),
        &[],
        RuntimeStepOptions {
            mode: RuntimeStepMode::OneOp,
            budget: RuntimeStepBudget { max_ops: 128 },
        },
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_budget_loop() {
    let steps = run_parity_with_options(
        flow(vec![FlowOp::Loop {
            body: vec![FlowOp::Noop],
        }]),
        &[],
        RuntimeStepOptions {
            mode: RuntimeStepMode::Drain,
            budget: RuntimeStepBudget { max_ops: 3 },
        },
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_budget_trap_stats() {
    let steps = run_parity(
        flow(vec![FlowOp::ReturnExpr(RuntimeExpr::Value(
            RuntimeValue::String("done".to_owned()),
        ))]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_await_many() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target(),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}

#[test]
fn awbc_product_parity_await_many_error() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target(),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.many.0".to_owned()),
                    sequence: TaskSequence(0),
                    kind: TaskEventKind::Err("host failed".to_owned()),
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await_many_cancel() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target(),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![TaskEvent {
                    logical_epoch: LogicalEpoch::default(),
                    task_id: TaskId("task.many.0".to_owned()),
                    sequence: TaskSequence(0),
                    kind: TaskEventKind::Cancelled,
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await_many_partial_then_error() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target(),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![
                    TaskEvent {
                        logical_epoch: LogicalEpoch::default(),
                        task_id: TaskId("task.many.1".to_owned()),
                        sequence: TaskSequence(0),
                        kind: TaskEventKind::Ready(RuntimePayload(RuntimeValue::String(
                            "b-ok".to_owned(),
                        ))),
                    },
                    TaskEvent {
                        logical_epoch: LogicalEpoch::default(),
                        task_id: TaskId("task.many.0".to_owned()),
                        sequence: TaskSequence(1),
                        kind: TaskEventKind::Err("host failed".to_owned()),
                    },
                ],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

#[test]
fn awbc_product_parity_await_many_partial_then_cancel() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target(),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![
                    TaskEvent {
                        logical_epoch: LogicalEpoch::default(),
                        task_id: TaskId("task.many.1".to_owned()),
                        sequence: TaskSequence(0),
                        kind: TaskEventKind::Ready(RuntimePayload(RuntimeValue::String(
                            "b-ok".to_owned(),
                        ))),
                    },
                    TaskEvent {
                        logical_epoch: LogicalEpoch::default(),
                        task_id: TaskId("task.many.0".to_owned()),
                        sequence: TaskSequence(1),
                        kind: TaskEventKind::Cancelled,
                    },
                ],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
}

fn await_many_target() -> AwaitManyTarget {
    AwaitManyTarget {
        need: NeedId("need.many".to_owned()),
        task: TaskId("task.many".to_owned()),
        source: RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::values(vec![
            RuntimeValue::String("a".to_owned()),
            RuntimeValue::String("b".to_owned()),
        ]))),
        item_binding: "item".to_owned(),
        limit: 2,
        request: HostTaskRequestTemplate {
            capability: HostCapabilityId("probe".to_owned()),
            operation: "read".to_owned(),
            args: vec![HostTaskArgTemplate::Positional(RuntimeExpr::Local(
                "item".to_owned(),
            ))],
        },
    }
}
