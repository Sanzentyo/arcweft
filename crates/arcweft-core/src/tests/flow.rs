//! Native-flow behavior exercised through the sole typed runtime-plan builder.

use crate::{
    engine::{Engine, FlowFiberStatus},
    pattern::RuntimeSemanticTypeId,
    plan::{
        FlowEvent, RuntimeAwaitPendingObserverSeed, RuntimeAwaitTargetSeed, RuntimeExprSeed,
        RuntimeExprSeedKind, RuntimeFlowOpSeed, RuntimeFlowSeed,
        RuntimeHostTaskRequestTemplateSeed, RuntimePatternSeed, RuntimePatternSeedKind,
        RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    },
    step::{RuntimeStepInput, RuntimeStepOptions},
    task::{
        HostCapabilityId, LogicalEpoch, NeedId, TaskEvent, TaskEventKind, TaskId,
        TaskOutcomeContract, TaskSequence,
    },
    value::RuntimeValue,
};
use arcweft_need::Progress;

const STRING_TYPE_MARKER: u8 = 1;

fn string_type() -> RuntimeSemanticTypeId {
    RuntimeSemanticTypeId::from_bytes([STRING_TYPE_MARKER; 32])
}

fn string_value(value: &str) -> RuntimeExprSeed {
    RuntimeExprSeed::new(
        string_type(),
        RuntimeExprSeedKind::Value(RuntimeValue::String(value.to_owned())),
    )
}

fn flow_id(value: &str) -> crate::plan::FlowRuntimeId {
    crate::plan::FlowRuntimeId::from_runtime_target_value(value).expect("valid test flow id")
}

fn finish_plan(flows: impl IntoIterator<Item = RuntimeFlowSeed>) -> crate::plan::RuntimePlan {
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [RuntimePlanTypeSeed::new(
                string_type(),
                RuntimePlanTypeProjection::String,
            )],
            [],
            [],
            [],
        )
        .expect("typed scalar admission");
    for flow in flows {
        builder.push_flow_seed(flow).expect("typed flow admission");
    }
    builder.finish().expect("valid typed runtime plan")
}

fn step(engine: &mut Engine) -> crate::step::RuntimeStepOutput {
    engine
        .step(RuntimeStepInput::default(), RuntimeStepOptions::default())
        .output
}

fn drain(engine: &mut Engine) -> crate::step::RuntimeStepOutput {
    let mut output = crate::step::RuntimeStepOutput::default();
    for _ in 0..8 {
        let next = step(engine);
        output.flow_events.extend(next.flow_events);
        output.diagnostics.extend(next.diagnostics);
        if matches!(
            engine.fiber().status,
            FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
        ) {
            break;
        }
    }
    output
}

#[test]
fn native_flow_returns_a_typed_scalar_value() {
    let entry = flow_id("flow.return");
    let plan = finish_plan([RuntimeFlowSeed::new(
        entry.clone(),
        [],
        vec![RuntimeFlowOpSeed::ReturnExpr(string_value("ready"))],
    )]);
    let mut engine = Engine::for_flow(plan, &entry).expect("entry flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "ready".to_owned(),
        }]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_goto_selects_the_targeted_typed_flow() {
    let opening = flow_id("flow.opening");
    let ending = flow_id("flow.ending");
    let plan = finish_plan([
        RuntimeFlowSeed::new(
            opening.clone(),
            [],
            vec![RuntimeFlowOpSeed::Goto(ending.clone())],
        ),
        RuntimeFlowSeed::new(
            ending.clone(),
            [],
            vec![RuntimeFlowOpSeed::ReturnExpr(string_value("finished"))],
        ),
    ]);
    let mut engine = Engine::for_flow(plan, &opening).expect("opening flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![
            FlowEvent::Goto { target: ending },
            FlowEvent::Return {
                value: "finished".to_owned(),
            },
        ]
    );
    assert!(matches!(engine.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn native_if_uses_the_admitted_bool_condition() {
    let bool_type = RuntimeSemanticTypeId::from_bytes([2; 32]);
    let entry = flow_id("flow.branch");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(string_type(), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(bool_type, RuntimePlanTypeProjection::Bool),
            ],
            [],
            [],
            [],
        )
        .expect("typed scalar admission");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            vec![RuntimeFlowOpSeed::If {
                condition: RuntimeExprSeed::new(
                    bool_type,
                    RuntimeExprSeedKind::Value(RuntimeValue::Bool(true)),
                ),
                then_ops: vec![RuntimeFlowOpSeed::ReturnExpr(string_value("then"))],
                else_ops: vec![RuntimeFlowOpSeed::ReturnExpr(string_value("else"))],
            }],
        ))
        .expect("typed if flow admission");
    let plan = builder.finish().expect("valid typed branch plan");
    let mut engine = Engine::for_flow(plan, &entry).expect("branch flow exists");

    let output = drain(&mut engine);

    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "then".to_owned(),
        }]
    );
}

#[test]
fn await_progress_runs_only_the_first_matching_observer() {
    let progress_type = RuntimeSemanticTypeId::from_bytes([3; 32]);
    let entry = flow_id("flow.await_observer");
    let mut builder = RuntimePlanBuilder::new();
    builder
        .admit_semantic_batch(
            [
                RuntimePlanTypeSeed::new(string_type(), RuntimePlanTypeProjection::String),
                RuntimePlanTypeSeed::new(progress_type, RuntimePlanTypeProjection::Progress),
            ],
            [],
            [],
            [],
        )
        .expect("Await observer types admit");
    builder
        .push_flow_seed(RuntimeFlowSeed::new(
            entry.clone(),
            [],
            vec![RuntimeFlowOpSeed::Await {
                binding: None,
                target: RuntimeAwaitTargetSeed {
                    need: NeedId("need.observe".to_owned()),
                    task: TaskId("task.observe".to_owned()),
                    outcome: TaskOutcomeContract::new(crate::pattern::RuntimeCheckedType::String),
                    request: RuntimeHostTaskRequestTemplateSeed {
                        capability: HostCapabilityId("test".to_owned()),
                        operation: "observe".to_owned(),
                        args: Vec::new(),
                    },
                },
                observers: vec![
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("first".to_owned())],
                    },
                    RuntimeAwaitPendingObserverSeed {
                        pattern: RuntimePatternSeed::new(
                            progress_type,
                            RuntimePatternSeedKind::Discard,
                        ),
                        ops: vec![RuntimeFlowOpSeed::Return("second".to_owned())],
                    },
                ],
            }],
        ))
        .expect("Await observer flow admits");
    let plan = builder.finish().expect("valid Await observer plan");
    let mut engine = Engine::for_flow(plan, &entry).expect("Await observer flow exists");
    let _started = step(&mut engine);

    let result = engine.step(
        RuntimeStepInput {
            task_events: vec![TaskEvent {
                logical_epoch: LogicalEpoch(1),
                task_id: TaskId("task.observe".to_owned()),
                sequence: TaskSequence(1),
                kind: TaskEventKind::Progress(
                    Progress::new(0.5).expect("fixture Progress is valid"),
                ),
            }],
            ..RuntimeStepInput::default()
        },
        RuntimeStepOptions::default(),
    );

    assert_eq!(
        result
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::AwaitProgress { .. }))
            .count(),
        1
    );
    let output = drain(&mut engine);
    assert_eq!(
        output.flow_events,
        vec![FlowEvent::Return {
            value: "first".to_owned(),
        }]
    );
}
