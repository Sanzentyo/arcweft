use arcweft_core::effect::{LineEffectRequest, RuntimeCall};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_core::executor::{ArcweftExecutionTier, ArcweftRuntimeExecutor, RuntimeExecutor};
use arcweft_core::line_task::{LineTaskGroup, LineTaskNode, LineTaskScope};
use arcweft_core::pattern::RuntimePattern;
use arcweft_core::plan::{ChoiceRuntimeOption, FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use arcweft_core::step::{
    RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions, RuntimeStepResult,
};
use arcweft_core::task::{
    AwaitManyTarget, AwaitTarget, HostCapabilityId, HostTaskArgTemplate, HostTaskRequestTemplate,
    LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId, TaskSequence,
};
use arcweft_core::value::{RuntimeExpr, RuntimePayload, RuntimeSeq, RuntimeValue};
use arcweft_interaction_model::{
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
}

fn run_parity(plan: RuntimePlan, inputs: Vec<RuntimeStepInput>) -> Vec<ParityStep> {
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

    inputs
        .into_iter()
        .map(|input| {
            let options = RuntimeStepOptions {
                mode: RuntimeStepMode::Drain,
                budget: RuntimeStepBudget { max_ops: 128 },
            };
            ParityStep {
                structured: structured.step(input.clone(), options),
                awbc: awbc.step(input, options),
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
            source_events: vec![arcweft_core::source::RuntimeSourceEvent {
                source: arcweft_core::source::SourceId("source.test".to_owned()),
                sequence: TaskSequence(0),
                kind: arcweft_core::source::SourceEventKind::Item(RuntimePayload(
                    RuntimeValue::String("source-item".to_owned()),
                )),
            }],
            ..RuntimeStepInput::default()
        }],
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
    let target = AwaitManyTarget {
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
    };
    let steps = run_parity(
        flow(vec![
            FlowOp::AwaitMany {
                binding: Some(RuntimePattern::Ident("items".to_owned())),
                target,
                pending: Vec::new(),
            },
            FlowOp::Return("done".to_owned()),
        ]),
        vec![RuntimeStepInput::default()],
    );

    assert_step_boundary_eq(&steps[0]);
}
