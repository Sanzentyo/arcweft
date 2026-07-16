use super::call;
use crate::effect::{LineEffectRequest, RuntimeLog};
use crate::engine::FlowFiberStatus;
use crate::line_task::{LineTaskGroup, LineTaskNode, LineTaskScope};
use crate::plan::{FlowOp, RuntimeFlow, RuntimePlan};
use crate::step::{
    RuntimeDiagnostic, RuntimeStepBudget, RuntimeStepInput, RuntimeStepMode, RuntimeStepOptions,
    RuntimeStepOutput, RuntimeStepStopReason,
};
use crate::time::{LogicalDuration, TickId};

#[test]
fn runtime_step_input_ref_borrows_adapter_owned_events() {
    let input = RuntimeStepInput {
        tick: TickId(7),
        dt: LogicalDuration::from_nanos(16_000_000),
        input_events: vec![super::input_event("advance", None)],
        ..RuntimeStepInput::default()
    };

    let view = input.as_view();

    assert_eq!(view.tick(), TickId(7));
    assert_eq!(view.dt(), LogicalDuration::from_nanos(16_000_000));
    assert_eq!(
        crate::step::input_event_trigger_name(&view.input_events()[0]),
        Some("advance")
    );
    assert!(view.bindings().is_empty());
}

#[test]
fn dialogue_advance_requires_the_exact_public_line_target() {
    let line = super::line_id("say.opening.001");
    let untargeted = RuntimeStepInput {
        input_events: vec![super::input_event("dialogue.advance", None)],
        ..RuntimeStepInput::default()
    };
    let wrong_line = RuntimeStepInput {
        input_events: vec![super::input_event("dialogue.advance", Some("say.other"))],
        ..RuntimeStepInput::default()
    };
    let targeted = RuntimeStepInput {
        input_events: vec![super::dialogue_advance(&line)],
        ..RuntimeStepInput::default()
    };
    let mut wrong_input_target_event = super::dialogue_advance(&line);
    wrong_input_target_event.target =
        arcweft_interaction_model::input::InteractionTarget::new("dialogue-widget")
            .expect("test target");
    let wrong_input_target = RuntimeStepInput {
        input_events: vec![wrong_input_target_event],
        ..RuntimeStepInput::default()
    };
    let removed_alias = RuntimeStepInput {
        input_events: vec![super::input_event(
            "advance",
            Some(line.public_label().as_str()),
        )],
        ..RuntimeStepInput::default()
    };

    assert!(!untargeted.advances_dialogue(&line));
    assert!(!wrong_line.advances_dialogue(&line));
    assert!(!wrong_input_target.advances_dialogue(&line));
    assert!(!removed_alias.advances_dialogue(&line));
    assert!(targeted.advances_dialogue(&line));
}

#[test]
fn runtime_step_output_sink_scopes_mutation_without_taking_output() {
    let mut output = RuntimeStepOutput::default();
    {
        let mut writer = output.writer();
        writer.push_diagnostic("first");
        writer.merge(RuntimeStepOutput {
            diagnostics: vec![RuntimeDiagnostic::new("second".to_owned())],
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
    let mut drain = super::engine_for_test_plan(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let drained = drain.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Drain, 8),
    );
    assert_eq!(drained.stop_reason, RuntimeStepStopReason::Done);
    assert_eq!(drained.stats.executed_ops, 3);
    assert_eq!(drained.stats.pending_ops_before, 0);
    assert!(matches!(drain.fiber().status, FlowFiberStatus::Done(_)));

    let mut budgeted = super::engine_for_test_plan(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let result = budgeted.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Drain, 2),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::BudgetExhausted);
    assert_eq!(result.stats.executed_ops, 2);
    assert!(matches!(budgeted.fiber().status, FlowFiberStatus::Running));

    let mut one_op = super::engine_for_test_plan(linear_plan(vec![
        FlowOp::Noop,
        FlowOp::Return("done".to_owned()),
    ]));
    let result = one_op.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::OneOp, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::OneOp);
    assert_eq!(result.stats.executed_ops, 1);
    assert!(matches!(one_op.fiber().status, FlowFiberStatus::Running));
}

#[test]
fn game_mode_stops_on_visible_output_but_server_mode_drains() {
    let line = super::line_id("say.opening.001");
    let line_group = LineTaskGroup {
        root: LineTaskScope {
            node: LineTaskNode::Seq(vec![LineTaskNode::Effect(call("show.line"))]),
            ..LineTaskScope::default()
        },
        ..LineTaskGroup::default()
    };
    let plan = super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.opening"),
            ops: vec![
                FlowOp::Dialogue {
                    line: line.clone(),
                    task_group: 0,
                },
                FlowOp::Return("done".to_owned()),
            ],
        }],
        vec![line_group],
    )
    .expect("plan is valid");

    let mut game = super::engine_for_test_plan(plan.clone());
    let result = game.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Game, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);
    assert!(matches!(game.fiber().status, FlowFiberStatus::Dialogue(_)));

    let resumed = game.step(
        RuntimeStepInput {
            input_events: vec![super::dialogue_advance(&line)],
            ..RuntimeStepInput::default()
        },
        options(RuntimeStepMode::Game, 8),
    );
    assert_eq!(resumed.stop_reason, RuntimeStepStopReason::Done);
    assert!(matches!(game.fiber().status, FlowFiberStatus::Done(_)));

    let mut server = super::engine_for_test_plan(plan);
    let result = server.step(
        RuntimeStepInput::default(),
        options(RuntimeStepMode::Server, 8),
    );
    assert_eq!(result.stop_reason, RuntimeStepStopReason::Output);
    assert!(matches!(
        server.fiber().status,
        FlowFiberStatus::Dialogue(_)
    ));

    let resumed = server.step(
        RuntimeStepInput {
            input_events: vec![super::dialogue_advance(&line)],
            ..RuntimeStepInput::default()
        },
        options(RuntimeStepMode::Server, 8),
    );
    assert_eq!(resumed.stop_reason, RuntimeStepStopReason::Done);
    assert!(matches!(server.fiber().status, FlowFiberStatus::Done(_)));
}

#[test]
fn game_mode_does_not_stop_for_pure_observations() {
    let mut engine = super::engine_for_test_plan(linear_plan(vec![
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
    assert_eq!(result.stats.line_effects, 1);
    assert_eq!(engine.fiber().observations.logs.len(), 1);
}

fn linear_plan(ops: Vec<FlowOp>) -> RuntimePlan {
    super::runtime_plan(
        Some(super::flow_id("flow.opening")),
        vec![RuntimeFlow {
            id: super::flow_id("flow.opening"),
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
