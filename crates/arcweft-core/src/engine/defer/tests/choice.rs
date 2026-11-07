use super::*;
use crate::effect::LineEffectRequest;
use crate::engine::{ChoiceState, Engine, FlowEvent, FlowExit, FlowFiberStatus};
use crate::step::{RuntimeStepInput, RuntimeStepOptions};

fn selection(choice: &str, option: &str) -> RuntimeStepInput {
    use arcweft_interaction_model::{
        id::Identifier,
        input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
        payload::InteractionPayload,
    };
    RuntimeStepInput {
        input_events: vec![
            RoutedInputEvent::new(
                InputEpoch::new(0),
                InputSequence::new(0),
                InteractionTarget::new("choice").unwrap(),
                InputEventKind::Custom {
                    name: Identifier::new(format!("choice:{choice}")).unwrap(),
                },
            )
            .with_payload(InteractionPayload::Text(option.into())),
        ],
        ..RuntimeStepInput::default()
    }
}

#[test]
fn native_choice_target_drains_callee_and_caller_scopes_once_across_restore() {
    for case in [
        GotoCase::Choice,
        GotoCase::ChoiceFailure,
        GotoCase::ChoiceContinue,
    ] {
        let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(case));
        let owner = RuntimeProgramOwner::Plan(engine.program_plan());
        let mut logs = Vec::new();
        let mut selections = 0;
        let mut transfers = 0;
        let mut reached_choice = false;
        let mut reached_exit = false;
        let mut finished = false;
        for _ in 0..100 {
            let input = if matches!(engine.fiber.frame.status, FlowFiberStatus::Choice(_)) {
                reached_choice = true;
                selection(
                    "test.choice",
                    if case == GotoCase::ChoiceContinue {
                        "Next"
                    } else {
                        "next"
                    },
                )
            } else {
                RuntimeStepInput::default()
            };
            let step = engine.step(input, RuntimeStepOptions::default());
            logs.extend(
                step.output
                    .effects
                    .line
                    .iter()
                    .filter_map(|effect| match effect {
                        LineEffectRequest::Log(row) => Some(row.message.clone()),
                        _ => None,
                    }),
            );
            for event in &step.output.flow_events {
                match event {
                    FlowEvent::ChoiceSelected { id, option } => {
                        assert_eq!(id.as_deref(), Some("test.choice"));
                        assert_eq!(option, "next");
                        selections += 1;
                    }
                    FlowEvent::Goto { target } => {
                        assert_eq!(case, GotoCase::Choice);
                        assert_eq!(target.canonical_label(), "test.goto_destination");
                        assert_eq!(
                            logs,
                            [
                                "inside",
                                "selected",
                                "inner cleanup",
                                "completed cleanup",
                                "always cleanup"
                            ]
                        );
                        transfers += 1;
                    }
                    _ => {}
                }
            }
            let image = engine.inert_rollback_image().unwrap();
            if let FlowFiberStatus::Choice(state) = &engine.fiber.frame.status {
                let mut forged = image.clone();
                // Public projection is display data, even with exactly the same rows.
                forged.fiber.frame.status = FlowFiberStatus::Choice(ChoiceState::observed(
                    state.id().map(str::to_owned),
                    state.options().to_vec(),
                ));
                assert!(Engine::from_rollback_image(forged, &owner).is_err());
            }
            if let Some(exit) = &image.fiber.frame.lexical_exit
                && let NativeLexicalExitAction::Goto {
                    source: NativeGotoOrigin::Choice { .. },
                    ..
                } = &exit.action
            {
                reached_exit = true;
                for ordinal in [1, usize::MAX] {
                    let mut forged = image.clone();
                    let NativeLexicalExitAction::Goto {
                        source: NativeGotoOrigin::Choice { option, .. },
                        ..
                    } = &mut forged.fiber.frame.lexical_exit.as_mut().unwrap().action
                    else {
                        unreachable!()
                    };
                    *option = ordinal;
                    assert!(Engine::from_rollback_image(forged, &owner).is_err());
                }
            }
            engine = Engine::from_rollback_image(image, &owner).unwrap();
            match step.fiber_status {
                FlowFiberStatus::Done(FlowExit::Return(value)) => {
                    assert!(
                        step.output.diagnostics.is_empty(),
                        "{:?}",
                        step.output.diagnostics
                    );
                    if case == GotoCase::ChoiceContinue {
                        assert_eq!(value, "true");
                        assert_eq!(
                            logs,
                            [
                                "inside",
                                "selected",
                                "inner cleanup",
                                "caller resumed",
                                "completed cleanup",
                                "always cleanup"
                            ]
                        );
                    } else {
                        assert_eq!(value, "true");
                        assert_eq!(logs.last().map(String::as_str), Some("destination started"));
                    }
                    finished = true;
                    break;
                }
                FlowFiberStatus::Failed(message) => {
                    assert_eq!(case, GotoCase::ChoiceFailure);
                    assert_eq!(message, "inner cleanup failed");
                    assert_eq!(
                        logs,
                        [
                            "inside",
                            "selected",
                            "inner cleanup",
                            "failed cleanup",
                            "always cleanup"
                        ]
                    );
                    finished = true;
                    break;
                }
                _ => assert!(
                    step.output.diagnostics.is_empty(),
                    "{:?}",
                    step.output.diagnostics
                ),
            }
        }
        assert!(finished && reached_choice);
        assert_eq!(selections, 1);
        assert_eq!(transfers, usize::from(case == GotoCase::Choice));
        assert_eq!(reached_exit, case != GotoCase::ChoiceContinue);
    }
}

#[test]
fn native_choice_retains_accepted_input_while_joined_child_runs() {
    let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(GotoCase::Choice));
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    let mut sent = false;
    let mut pending = 0;
    let mut selections = 0;
    let mut transfers = 0;
    let mut logs = Vec::new();
    for _ in 0..100 {
        let input = if !sent && matches!(engine.fiber.frame.status, FlowFiberStatus::Choice(_)) {
            let flow = FlowRuntimeId::canonical("test.goto_alternate").unwrap();
            let site = engine
                .plan
                .validate_flow_invocation(&flow, &[])
                .unwrap()
                .site();
            let position = NativeCodeCursor::root(
                &engine.plan,
                crate::plan::code::RuntimeCodeOwner::Function(site),
            )
            .unwrap();
            let work = engine.prepare_task_root_work(position, false).unwrap();
            let (child, next) = engine.prepare_child_fiber(work, &[]).unwrap();
            engine.next_fiber_id = next;
            engine.child_fibers.push_back(child);
            engine.run_child_next = false;
            sent = true;
            selection("test.choice", "next")
        } else {
            RuntimeStepInput::default()
        };
        let step = engine.step(input, RuntimeStepOptions::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        selections += step
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::ChoiceSelected { .. }))
            .count();
        transfers += step
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::Goto { .. }))
            .count();
        logs.extend(
            step.output
                .effects
                .line
                .into_iter()
                .filter_map(|effect| match effect {
                    LineEffectRequest::Log(row) => Some(row.message),
                    _ => None,
                }),
        );
        if let FlowFiberStatus::Choice(state) = &engine.fiber.frame.status
            && state.selected().is_some()
        {
            assert_eq!(selections, 0);
            assert_eq!(transfers, 0);
            assert_eq!(step.fiber_status, FlowFiberStatus::Running);
            assert!(engine.fiber.frame.lexical_exit.is_none());
            if engine.has_joined_work_for(engine.fiber.id) {
                assert!(!engine.main_fiber_can_attempt_runtime_op());
            } else {
                assert_ne!(
                    step.stop_reason,
                    crate::step::RuntimeStepStopReason::Blocked
                );
            }
            pending += 1;
        }
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
        if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
            assert_eq!(value, "true");
            assert_eq!(selections, 1);
            assert_eq!(transfers, 1);
            assert!(sent && pending > 0);
            assert_eq!(
                logs,
                [
                    "inside",
                    "wrong destination",
                    "selected",
                    "inner cleanup",
                    "completed cleanup",
                    "always cleanup",
                    "destination started"
                ]
            );
            return;
        }
        assert!(
            !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
            "{:?}",
            step.fiber_status
        );
    }
    panic!("accepted selection must resume without replaying input: {logs:?}");
}

#[test]
fn native_choice_rejects_stale_and_invalid_input_without_losing_original_options() {
    let mut engine = ordinary_flow_defer_engine(OrdinaryFlowExit::Goto(GotoCase::Choice));
    for _ in 0..30 {
        engine.step(RuntimeStepInput::default(), RuntimeStepOptions::default());
        if matches!(engine.fiber.frame.status, FlowFiberStatus::Choice(_)) {
            break;
        }
    }
    let FlowFiberStatus::Choice(state) = &engine.fiber.frame.status else {
        panic!("original choice")
    };
    let options = state.options().as_ptr();
    let owner = RuntimeProgramOwner::Plan(engine.program_plan());
    for (choice, option) in [("stale", "next"), ("test.choice", "missing")] {
        let step = engine.step(selection(choice, option), RuntimeStepOptions::default());
        assert_eq!(step.output.diagnostics.len(), 1);
        assert!(step.output.effects.line.is_empty());
        assert!(step.output.flow_events.is_empty());
        let FlowFiberStatus::Choice(state) = &step.fiber_status else {
            panic!("invalid input keeps the original Choice")
        };
        assert_eq!(state.options().as_ptr(), options);
        assert_eq!(state.id(), Some("test.choice"));
        engine =
            Engine::from_rollback_image(engine.inert_rollback_image().unwrap(), &owner).unwrap();
    }
}
