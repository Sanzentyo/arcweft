use super::call;
use crate::effect::{LineEffectRequest, RuntimeLog};
use crate::engine::{Engine, FlowFiberStatus};
use crate::line_task::{LineTaskGroup, LineTaskNode, LineTaskScope};
use crate::plan::{FlowOp, FlowRuntimeId, RuntimeFlow, RuntimePlan};
use crate::step::{
    InputEvent, RuntimeDiagnostic, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode,
    RuntimeStepOptions, RuntimeStepOutput, RuntimeStepStopReason,
};
use crate::time::{LogicalDuration, TickId};

#[test]
fn runtime_step_input_ref_borrows_adapter_owned_events() {
    let input = RuntimeStepInput {
        tick: TickId(7),
        dt: LogicalDuration::from_nanos(16_000_000),
        input_events: vec![InputEvent {
            kind: "advance".to_owned(),
            payload: None,
        }],
        ..RuntimeStepInput::default()
    };

    let view = input.as_view();

    assert_eq!(view.tick(), TickId(7));
    assert_eq!(view.dt(), LogicalDuration::from_nanos(16_000_000));
    assert_eq!(view.input_events()[0].kind, "advance");
    assert!(view.bindings().is_empty());
}

#[test]
fn runtime_step_output_sink_scopes_mutation_without_taking_output() {
    let mut output = RuntimeStepOutput::default();
    {
        let mut writer = output.writer();
        writer.push_diagnostic("first");
        writer.merge(RuntimeStepOutput {
            diagnostics: vec![RuntimeDiagnostic {
                message: "second".to_owned(),
            }],
            ..RuntimeStepOutput::default()
        });
    }

    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages, ["first", "second"]);
}

#[test]
fn runtime_step_modes_apply_internal_drain_and_budget() {
    let mut drain = Engine::new(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let drained = drain.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Drain, 8),
    );
    assert_eq!(drained.stop_reason, RuntimeStepStopReason::Done);
    assert!(matches!(drain.fiber().status, FlowFiberStatus::Done(_)));

    let mut budgeted = Engine::new(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let result = budgeted.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Drain, 2),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::BudgetExhausted);
    assert!(matches!(budgeted.fiber().status, FlowFiberStatus::Running));

    let mut one_op = Engine::new(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let result = one_op.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::OneOp, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::OneOp);
    assert!(matches!(one_op.fiber().status, FlowFiberStatus::Running));
}

#[test]
fn game_mode_stops_on_visible_output_but_server_mode_drains() {
    let line_group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("show.line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.opening".to_owned()),
            ops: vec![
                FlowOp::Dialogue {
                    line: crate::plan::RuntimeLineId("say.opening.001".to_owned()),
                    task_group: 0,
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        vec![line_group],
    )
    .expect("plan is valid");

    let mut game = Engine::new(plan.clone());
    let result = game.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Game, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);
    assert!(matches!(game.fiber().status, FlowFiberStatus::Running));

    let mut server = Engine::new(plan);
    let result = server.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Server, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert!(matches!(server.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn game_mode_does_not_stop_for_pure_observations() {
    let mut engine = Engine::new(linear_plan(vec![
        FlowOp::Effect(LineEffectRequest::Log(RuntimeLog {
            level: "info".to_owned(),
            message: "tick".to_owned(),
            fields: Vec::new(),
        })),
        FlowOp::Return("done".to_owned()),
    ]));

    let result = engine.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Game, 8),
    );

    assert_eq!(result.stop_reason, RuntimeStepStopReason::Done);
    assert_eq!(engine.fiber().observations.logs.len(), 1);
}

fn linear_plan(ops: Vec<FlowOp>) -> RuntimePlan {
    RuntimePlan::new(
        Some(FlowRuntimeId("flow.opening".to_owned())),
        vec![RuntimeFlow {
            id: FlowRuntimeId("flow.opening".to_owned()),
            ops,
        }],
        Vec::new(),
    )
    .expect("plan is valid")
}

const fn options(mode: RuntimeStepMode, max_ops: usize) -> RuntimeStepOptions {
    RuntimeStepOptions {
        mode,
        budget: RuntimeStepBudget { max_ops },
    }
}
