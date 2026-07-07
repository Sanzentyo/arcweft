use arcweft_core::audio::RuntimeAudioCommand;
use arcweft_core::effect::{
    LineEffectRequest, RuntimeAssertion, RuntimeAssertionProfile, RuntimeAssignment, RuntimeCall,
    RuntimeEvent, RuntimeField, RuntimeLog, RuntimeWaitTarget,
};
use arcweft_core::engine::{FlowFiber, FlowFiberStatus};
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::line_task::{LineOutRequest, LineTaskGroup, LineTaskNode, LineTaskScope};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{
    ChoiceRuntimeOption, FlowEvent, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimeHostCallTarget,
    RuntimeIteratorEvidence, RuntimeLineId, RuntimePlan, RuntimePureHelper, RuntimePureHelperId,
    RuntimePureHelperOrigin, RuntimePureInputType, RuntimePureOutputType,
};
use arcweft_core::source::{
    RuntimeSourceEvent, SourceEventKind, SourceHandlerPlan, SourceId, SourceOp, SourcePlan,
    SourcePolicy,
};
use arcweft_core::step::{
    RuntimeHostCallId, RuntimeHostCallMode, RuntimeHostCallResult, RuntimeStepBudget,
    RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
    RuntimeStepStopReason,
};
use arcweft_core::stream::{StreamOp, StreamPlan, StreamRuntimeId};
use arcweft_core::task::{
    AwaitManyTarget, AwaitTarget, HostCapabilityId, HostTaskArgTemplate, HostTaskRequestTemplate,
    LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId, TaskSequence,
};
use arcweft_core::time::LogicalDuration;
use arcweft_core::value::{
    RuntimeBinaryOp, RuntimeBinding, RuntimeCallTarget, RuntimeEnv, RuntimeExpr, RuntimePayload,
    RuntimeSeq, RuntimeValue,
};
use arcweft_interaction_model::{
    audio::{AudioCommand, AudioDispatchId, AudioMillis, GainDbMilli},
    id::Identifier,
    input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
    payload::InteractionPayload,
};
use arcweft_render_text::LineDisplayCatalog;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[derive(Debug)]
struct ParityStep {
    structured: RuntimeStepResult,
    awbc: RuntimeStepResult,
    structured_fiber: FlowFiber,
    awbc_fiber: FlowFiber,
}

fn flow_id(value: &str) -> FlowRuntimeId {
    FlowRuntimeId::from_runtime_target_value(value).expect("test flow ID is valid")
}

fn line_id(value: &str) -> RuntimeLineId {
    RuntimeLineId::from_runtime_line_value(value).expect("test line ID is valid")
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
                structured_fiber: structured.fiber().clone(),
                awbc_fiber: awbc.fiber().clone(),
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
        step.awbc_fiber.source_states, step.structured_fiber.source_states,
        "source state mismatch"
    );
    assert_facade_fiber_eq(step);
}

fn assert_facade_fiber_eq(step: &ParityStep) {
    if !matches!(
        step.structured.stop_reason,
        RuntimeStepStopReason::Done | RuntimeStepStopReason::Failed | RuntimeStepStopReason::Output
    ) {
        return;
    }
    assert_runtime_env_bindings_eq(
        &step.awbc_fiber.env,
        &step.structured_fiber.env,
        "facade environment mismatch",
    );
    assert_eq!(
        step.awbc_fiber.observations, step.structured_fiber.observations,
        "facade observations mismatch"
    );
    assert_eq!(
        step.awbc_fiber.stream_states, step.structured_fiber.stream_states,
        "facade stream state mismatch"
    );
    assert_eq!(
        normalized_status(&step.awbc_fiber.status),
        normalized_status(&step.structured_fiber.status),
        "facade status mismatch"
    );
}

fn assert_runtime_env_bindings_eq(left: &RuntimeEnv, right: &RuntimeEnv, message: &str) {
    let mut left_bindings = left.bindings_snapshot();
    let mut right_bindings = right.bindings_snapshot();
    left_bindings.sort_by(|left, right| left.name.cmp(&right.name));
    right_bindings.sort_by(|left, right| left.name.cmp(&right.name));
    assert_eq!(left_bindings, right_bindings, "{message}");
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
        FlowFiberStatus::HostCall(mut state) => {
            state.resume = None;
            state.binding = None;
            state.target.args.clear();
            FlowFiberStatus::HostCall(state)
        }
        status => status,
    }
}

fn flow(ops: Vec<FlowOp>) -> RuntimePlan {
    flows(
        "flow.main",
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops,
        }],
    )
}

fn flows(entry: &str, flows: Vec<RuntimeFlow>) -> RuntimePlan {
    RuntimePlan::new(Some(flow_id(entry)), flows, Vec::new()).expect("runtime plan builds")
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

fn audio_stop_all() -> LineEffectRequest {
    LineEffectRequest::Audio(Box::new(RuntimeAudioCommand::StopAll {
        fade_out_millis: RuntimeExpr::Value(RuntimeValue::u32(125)),
    }))
}

fn audio_set_bus_gain() -> LineEffectRequest {
    LineEffectRequest::Audio(Box::new(RuntimeAudioCommand::SetBusGain {
        bus: RuntimeExpr::Value(RuntimeValue::String("bus.music".to_owned())),
        gain_db_milli: RuntimeExpr::Local("music_gain".to_owned()),
        transition_millis: RuntimeExpr::Value(RuntimeValue::u32(250)),
    }))
}

fn non_control_effect_table() -> Vec<LineEffectRequest> {
    vec![
        LineEffectRequest::RegisterHandle {
            key: "handle.ui".to_owned(),
            handle: "h-001".to_owned(),
        },
        LineEffectRequest::DropHandle {
            key: "handle.ui".to_owned(),
        },
        LineEffectRequest::Wait(RuntimeWaitTarget::Duration(LogicalDuration::from_nanos(17))),
        LineEffectRequest::Call(RuntimeCall {
            callee: "effect.probe".to_owned(),
            args: vec!["left".to_owned(), "right".to_owned()],
        }),
        LineEffectRequest::Log(RuntimeLog {
            level: "info".to_owned(),
            message: "hello".to_owned(),
            fields: vec![RuntimeField {
                name: "scene".to_owned(),
                value: "opening".to_owned(),
            }],
        }),
        LineEffectRequest::SignalWrite(RuntimeAssignment {
            target: "signal.ready".to_owned(),
            value: "true".to_owned(),
        }),
        LineEffectRequest::MetricWrite(RuntimeAssignment {
            target: "metric.score".to_owned(),
            value: "42".to_owned(),
        }),
        LineEffectRequest::EmitEvent(RuntimeEvent {
            event: "event.chapter".to_owned(),
            fields: vec![RuntimeField {
                name: "chapter".to_owned(),
                value: "01".to_owned(),
            }],
        }),
        LineEffectRequest::Out(LineOutRequest {
            label: Some("speaker".to_owned()),
            value: "line-out".to_owned(),
        }),
        LineEffectRequest::Ensure {
            condition: "ready".to_owned(),
            message: "must be ready".to_owned(),
        },
        LineEffectRequest::Assert(RuntimeAssertion {
            condition: "debug_flag".to_owned(),
            message: "asserted".to_owned(),
            profile: RuntimeAssertionProfile::Always,
        }),
        LineEffectRequest::Close("panel.main".to_owned()),
        LineEffectRequest::Select("choice.primary".to_owned()),
        LineEffectRequest::Break {
            label: Some("loop.main".to_owned()),
            value: Some("break-value".to_owned()),
        },
        LineEffectRequest::Continue {
            label: Some("loop.main".to_owned()),
        },
    ]
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

fn i32_range_expr(start: i32, end: i32) -> RuntimeExpr {
    RuntimeExpr::Range {
        start: Some(Box::new(RuntimeExpr::Value(RuntimeValue::i32(start)))),
        end: Some(Box::new(RuntimeExpr::Value(RuntimeValue::i32(end)))),
        inclusive: false,
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
fn awbc_product_parity_for_range_effects() {
    let steps = run_parity(
        flow(vec![
            FlowOp::For {
                pattern: RuntimePattern::Ident("i".to_owned()),
                source: i32_range_expr(0, 3),
                evidence: RuntimeIteratorEvidence::builtin_range(),
                body: vec![FlowOp::Effect(call("effect.loop"))],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(steps[0].structured.output.effects.line.len(), 3);
    assert_eq!(
        steps[0].awbc.output.effects.line,
        steps[0].structured.output.effects.line
    );
}

#[test]
fn awbc_product_parity_for_empty_range_skips_body() {
    let steps = run_parity(
        flow(vec![
            FlowOp::For {
                pattern: RuntimePattern::Ident("i".to_owned()),
                source: i32_range_expr(0, 0),
                evidence: RuntimeIteratorEvidence::builtin_range(),
                body: vec![FlowOp::Effect(call("effect.loop"))],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert!(steps[0].structured.output.effects.line.is_empty());
    assert!(steps[0].awbc.output.effects.line.is_empty());
}

#[test]
fn awbc_product_parity_for_empty_range_skips_return_body() {
    let steps = run_parity(
        flow(vec![
            FlowOp::For {
                pattern: RuntimePattern::Ident("i".to_owned()),
                source: i32_range_expr(0, 0),
                evidence: RuntimeIteratorEvidence::builtin_range(),
                body: vec![FlowOp::Return("loop".to_owned())],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.flow_events,
        vec![FlowEvent::Return {
            value: "done".to_owned()
        }]
    );
}

#[test]
fn awbc_product_parity_for_non_empty_range_can_return_from_body() {
    let steps = run_parity(
        flow(vec![
            FlowOp::For {
                pattern: RuntimePattern::Ident("i".to_owned()),
                source: i32_range_expr(0, 1),
                evidence: RuntimeIteratorEvidence::builtin_range(),
                body: vec![FlowOp::Return("loop".to_owned())],
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.flow_events,
        vec![FlowEvent::Return {
            value: "loop".to_owned()
        }]
    );
}

#[test]
fn awbc_product_parity_for_range_dialogue_body_outputs() {
    let group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("loop_line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = RuntimePlan::new(
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![
                FlowOp::For {
                    pattern: RuntimePattern::Ident("i".to_owned()),
                    source: i32_range_expr(0, 2),
                    evidence: RuntimeIteratorEvidence::builtin_range(),
                    body: vec![FlowOp::Dialogue {
                        line: line_id("line.loop.001"),
                        task_group: 0,
                    }],
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        vec![group],
    )
    .expect("runtime plan builds");
    let steps = run_parity(plan, vec![RuntimeStepInput::default()]);

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
        Some(flow_id("flow.main")),
        vec![RuntimeFlow {
            id: flow_id("flow.main"),
            ops: vec![
                FlowOp::Dialogue {
                    line: line_id("line.opening.001"),
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
fn awbc_product_parity_direct_host_call_suspend_result() {
    let steps = run_parity(
        flow(vec![
            FlowOp::HostCall {
                binding: Some(RuntimePattern::Ident("hosted".to_owned())),
                target: RuntimeHostCallTarget::new(
                    "host.probe",
                    "probe",
                    "read",
                    [RuntimeExpr::Value(RuntimeValue::String("arg0".to_owned()))],
                    RuntimeHostCallMode::Suspend,
                    true,
                ),
            },
            FlowOp::ReturnExpr(RuntimeExpr::Local("hosted".to_owned())),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                host_call_results: vec![RuntimeHostCallResult {
                    id: RuntimeHostCallId("host.probe".to_owned()),
                    outcome: Ok(RuntimePayload(RuntimeValue::String("host-ok".to_owned()))),
                }],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
    let request = steps[0]
        .structured
        .output
        .requests
        .host_calls
        .first()
        .expect("host call request emitted");
    assert_eq!(request.id, RuntimeHostCallId("host.probe".to_owned()));
    assert_eq!(request.public_id, "host.probe");
    assert_eq!(request.capability, "probe");
    assert_eq!(request.operation, "read");
    assert_eq!(
        request.args,
        vec![RuntimePayload(RuntimeValue::String("arg0".to_owned()))]
    );
    assert_eq!(
        steps[1].structured.output.flow_events,
        vec![FlowEvent::Return {
            value: "host-ok".to_owned()
        }]
    );
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
fn awbc_product_parity_scope_cleanup_and_cancel() {
    let steps = run_parity(
        flow(vec![
            FlowOp::Scope(vec![
                FlowOp::RegisterCleanup {
                    key: "panel".to_owned(),
                    effect: call("cleanup.panel"),
                },
                FlowOp::CancelCleanup {
                    key: "panel".to_owned(),
                },
                FlowOp::RegisterCleanup {
                    key: "toast".to_owned(),
                    effect: call("cleanup.toast"),
                },
            ]),
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].awbc.output.effects.line,
        vec![call("cleanup.toast")]
    );
}

#[test]
fn awbc_product_parity_audio_stop_all() {
    let steps = run_parity(
        flow(vec![
            FlowOp::Effect(audio_stop_all()),
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(steps[0].structured.output.requests.audio.len(), 1);
    assert_eq!(
        steps[0].awbc.output.requests.audio,
        steps[0].structured.output.requests.audio
    );
    assert_eq!(steps[0].awbc.stats.audio_commands, 1);
    assert_eq!(steps[0].structured.stats.audio_commands, 1);
    assert_eq!(
        steps[0].structured.output.requests.audio[0].dispatch,
        AudioDispatchId::new(0, 0)
    );
    assert!(matches!(
        steps[0].structured.output.requests.audio[0].command,
        AudioCommand::StopAll { fade_out } if fade_out == AudioMillis::new(125)
    ));
}

#[test]
fn awbc_product_parity_audio_expression_bus_gain() {
    let steps = run_parity_with_root_bindings(
        flow(vec![
            FlowOp::Effect(audio_set_bus_gain()),
            FlowOp::Return("done".to_owned()),
        ]),
        &[binding("music_gain", RuntimeValue::i32(-4500))],
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(steps[0].structured.output.requests.audio.len(), 1);
    assert_eq!(
        steps[0].awbc.output.requests.audio,
        steps[0].structured.output.requests.audio
    );
    assert_eq!(steps[0].awbc.stats.audio_commands, 1);
    assert!(matches!(
        &steps[0].structured.output.requests.audio[0].command,
        AudioCommand::SetBusGain { bus, gain, transition }
            if bus.as_str() == "bus.music"
                && *gain == GainDbMilli::new(-4500).expect("valid gain")
                && *transition == AudioMillis::new(250)
    ));
}

#[test]
fn awbc_product_parity_line_effect_kind_table_non_control() {
    let effects = non_control_effect_table();
    let steps = run_parity(
        flow(
            effects
                .iter()
                .cloned()
                .map(FlowOp::Effect)
                .chain(std::iter::once(FlowOp::Return("done".to_owned())))
                .collect(),
        ),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(steps[0].structured.output.effects.line, effects);
    assert_eq!(steps[0].awbc.output.effects.line, effects);
}

#[test]
fn awbc_product_parity_control_effect_return() {
    let effect = LineEffectRequest::Return("effect-return".to_owned());
    let steps = run_parity(
        flow(vec![
            FlowOp::Effect(effect.clone()),
            FlowOp::Return("unreachable".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.effects.line,
        vec![effect.clone()]
    );
    assert_eq!(
        steps[0].structured.output.flow_events,
        vec![FlowEvent::Return {
            value: "effect-return".to_owned(),
        }]
    );
    assert_eq!(
        normalized_status(&steps[0].structured.fiber_status),
        FlowFiberStatus::Done(arcweft_core::engine::FlowExit::Return(
            "effect-return".to_owned(),
        ))
    );
}

#[test]
fn awbc_product_parity_control_effect_goto() {
    let effect = LineEffectRequest::Goto("flow.next".to_owned());
    let steps = run_parity(
        flows(
            "flow.main",
            vec![
                RuntimeFlow {
                    id: flow_id("flow.main"),
                    ops: vec![
                        FlowOp::Effect(effect.clone()),
                        FlowOp::Return("unreachable".to_owned()),
                    ],
                },
                RuntimeFlow {
                    id: flow_id("flow.next"),
                    ops: vec![FlowOp::Return("next-done".to_owned())],
                },
            ],
        ),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.effects.line,
        vec![effect.clone()]
    );
    assert_eq!(
        steps[0].structured.output.flow_events,
        vec![
            FlowEvent::Goto {
                target: flow_id("flow.next"),
            },
            FlowEvent::Return {
                value: "next-done".to_owned(),
            },
        ],
    );
}

#[test]
fn awbc_product_parity_dynamic_goto() {
    let steps = run_parity(
        flows(
            "flow.main",
            vec![
                RuntimeFlow {
                    id: flow_id("flow.main"),
                    ops: vec![
                        FlowOp::GotoExpr(RuntimeExpr::Value(RuntimeValue::String(
                            "flow.next".to_owned(),
                        ))),
                        FlowOp::Return("unreachable".to_owned()),
                    ],
                },
                RuntimeFlow {
                    id: flow_id("flow.next"),
                    ops: vec![FlowOp::Return("next-done".to_owned())],
                },
            ],
        ),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
    assert_eq!(
        steps[0].structured.output.flow_events,
        vec![
            FlowEvent::Goto {
                target: flow_id("flow.next"),
            },
            FlowEvent::Return {
                value: "next-done".to_owned(),
            },
        ],
    );
}

#[test]
fn awbc_product_parity_control_effect_failures() {
    for effect in [
        LineEffectRequest::Panic("panic-effect".to_owned()),
        LineEffectRequest::Fail("fail-effect".to_owned()),
        LineEffectRequest::Bail("bail-effect".to_owned()),
    ] {
        let steps = run_parity(
            flow(vec![
                FlowOp::Effect(effect.clone()),
                FlowOp::Return("unreachable".to_owned()),
            ]),
            vec![RuntimeStepInput::default()],
        );

        assert_step_boundary_eq(&steps[0]);
        assert_eq!(
            steps[0].structured.output.effects.line,
            vec![effect.clone()]
        );
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
fn awbc_product_parity_stream_for_next_binds_source_item() {
    let plan = flow(vec![FlowOp::Return("done".to_owned())]).with_generation_plans(
        vec![StreamPlan {
            id: StreamRuntimeId("passthrough".to_owned()),
            item_ty: "IteratorItem".to_owned(),
            error_ty: "CaptureError".to_owned(),
            ops: vec![StreamOp::ForNext {
                pattern: RuntimePattern::Ident("frame".to_owned()),
                source: RuntimeExpr::Local("frames".to_owned()),
                body: vec![StreamOp::Yield {
                    expr: RuntimeExpr::Local("frame".to_owned()),
                }],
            }],
        }],
        Vec::new(),
    );
    let awbc = AwbcLowerer::new(
        &plan,
        &LineDisplayCatalog::default(),
        "stream-for-next.arcw",
    )
    .lower()
    .expect("stream for-next plan lowers")
    .program;

    assert!(
        awbc.product_step_parity_blockers().is_empty(),
        "stream for-next binding should produce verified product AWBC"
    );
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
        .structured_fiber
        .source_states
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
        .structured_fiber
        .source_states
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
        .structured_fiber
        .source_states
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

#[test]
fn awbc_product_parity_await_many_out_of_order_progress_ready_three_items() {
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target: await_many_target_with_items(&["a", "b", "c"], 2),
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![
            RuntimeStepInput::default(),
            RuntimeStepInput {
                task_events: vec![
                    task_ready("task.many.1", 1, "b-ok"),
                    task_ready("task.many.0", 1, "a-ok"),
                    task_progress("task.many.1", 0, "b-half"),
                    task_progress("task.many.0", 0, "a-half"),
                ],
                ..RuntimeStepInput::default()
            },
            RuntimeStepInput {
                task_events: vec![
                    task_ready("task.many.2", 1, "c-ok"),
                    task_progress("task.many.2", 0, "c-half"),
                ],
                ..RuntimeStepInput::default()
            },
        ],
    );

    for step in &steps {
        assert_step_boundary_eq(step);
    }
    assert!(
        steps[1]
            .structured
            .output
            .flow_events
            .iter()
            .any(|event| matches!(
                event,
                FlowEvent::AwaitStarted { task, .. } if task.0 == "task.many.2"
            )),
        "third await-many task should be started after the first two ready events"
    );
    assert!(
        steps[2]
            .structured
            .output
            .flow_events
            .iter()
            .any(|event| matches!(
                event,
                FlowEvent::AwaitReady { need, value }
                    if need.0 == "need.many"
                        && *value == RuntimePayload(RuntimeValue::Seq(RuntimeSeq::values(vec![
                            RuntimeValue::String("a-ok".to_owned()),
                            RuntimeValue::String("b-ok".to_owned()),
                            RuntimeValue::String("c-ok".to_owned()),
                        ])))
            )),
        "await-many aggregate result should preserve source item order"
    );
}

fn await_many_target() -> AwaitManyTarget {
    await_many_target_with_items(&["a", "b"], 2)
}

fn await_many_target_with_items(items: &[&str], limit: usize) -> AwaitManyTarget {
    AwaitManyTarget {
        need: NeedId("need.many".to_owned()),
        task: TaskId("task.many".to_owned()),
        source: RuntimeExpr::Value(RuntimeValue::Seq(RuntimeSeq::values(
            items
                .iter()
                .map(|item| RuntimeValue::String((*item).to_owned()))
                .collect(),
        ))),
        item_binding: "item".to_owned(),
        limit,
        request: HostTaskRequestTemplate {
            capability: HostCapabilityId("probe".to_owned()),
            operation: "read".to_owned(),
            args: vec![HostTaskArgTemplate::Positional(RuntimeExpr::Local(
                "item".to_owned(),
            ))],
        },
    }
}

fn task_ready(task_id: &str, sequence: u64, value: &str) -> TaskEvent {
    TaskEvent {
        logical_epoch: LogicalEpoch::default(),
        task_id: TaskId(task_id.to_owned()),
        sequence: TaskSequence(sequence),
        kind: TaskEventKind::Ready(RuntimePayload(RuntimeValue::String(value.to_owned()))),
    }
}

fn task_progress(task_id: &str, sequence: u64, value: &str) -> TaskEvent {
    TaskEvent {
        logical_epoch: LogicalEpoch::default(),
        task_id: TaskId(task_id.to_owned()),
        sequence: TaskSequence(sequence),
        kind: TaskEventKind::Progress(RuntimePayload(RuntimeValue::String(value.to_owned()))),
    }
}
