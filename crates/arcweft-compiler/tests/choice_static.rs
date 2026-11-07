use arcweft_compiler::source::compile_source;
use arcweft_core::plan::FlowOp;
use arcweft_runtime_plan::awbc_lower::AwbcLowerer;

#[test]
fn compact_choice_goto_preserves_checked_ids_and_flow_target_through_awbc() {
    let compiled = compile_source(
        r#"
entry cli @entry.main { goto @flow.main }

flow main {
    choice @.first {
        @.next "Next" -> @flow.done
    }
}

flow done() -> String {
    return "done"
}
"#,
    )
    .expect("static Choice goto compiles");

    let (id, options) = compiled
        .plan
        .flows()
        .iter()
        .flat_map(|flow| flow.body().ops().iter())
        .find_map(|op| match op {
            FlowOp::Scope { body: ops, .. } => ops.iter().find_map(|op| match op {
                FlowOp::Choice { id, options } => Some((id, options)),
                _ => None,
            }),
            FlowOp::Choice { id, options } => Some((id, options)),
            _ => None,
        })
        .expect("one lowered Choice operation");
    assert_eq!(id.as_deref(), Some("choice.main.first"));
    let [option] = options.as_slice() else {
        panic!("one lowered Choice option")
    };
    assert_eq!(option.id.as_deref(), Some("choice.main.first.next"));
    assert_eq!(option.label, "Next");
    let target = option.target.as_ref().expect("checked Flow target");
    assert_eq!(target.public_label().as_str(), "flow.done");

    AwbcLowerer::new(
        &compiled.plan,
        &compiled.dialogue_content,
        "choice_static.arcw",
    )
    .lower()
    .expect("static Choice goto lowers to verified AWBC");

    use arcweft_core::engine::{Engine, FlowExit, FlowFiberStatus};
    use arcweft_core::plan::FlowEvent;
    use arcweft_core::step::{RuntimeStepInput, RuntimeStepOptions};
    use arcweft_interaction_model::{
        id::Identifier,
        input::{InputEpoch, InputEventKind, InputSequence, InteractionTarget, RoutedInputEvent},
        payload::InteractionPayload,
    };
    let main = compiled.plan.flows()[0].id.clone();
    let mut engine = Engine::for_flow(compiled.plan.clone(), &main).unwrap();
    let mut sent = false;
    let mut selected = 0;
    let mut transferred = 0;
    for _ in 0..40 {
        let input = if let FlowFiberStatus::Choice(state) = &engine.fiber().frame.status {
            assert!(!sent, "one selection input is sufficient");
            assert_eq!(state.id(), Some("choice.main.first"));
            let option = &state.options()[0];
            sent = true;
            RuntimeStepInput {
                input_events: vec![
                    RoutedInputEvent::new(
                        InputEpoch::new(0),
                        InputSequence::new(0),
                        InteractionTarget::new("choice").unwrap(),
                        InputEventKind::Custom {
                            name: Identifier::new("choice:choice.main.first").unwrap(),
                        },
                    )
                    .with_payload(InteractionPayload::Text(option.id.clone().unwrap())),
                ],
                ..RuntimeStepInput::default()
            }
        } else {
            RuntimeStepInput::default()
        };
        let step = engine.step(input, RuntimeStepOptions::default());
        assert!(
            step.output.diagnostics.is_empty(),
            "{:?}",
            step.output.diagnostics
        );
        selected += step
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::ChoiceSelected { .. }))
            .count();
        transferred += step
            .output
            .flow_events
            .iter()
            .filter(|event| matches!(event, FlowEvent::Goto { .. }))
            .count();
        if let FlowFiberStatus::Done(FlowExit::Return(value)) = step.fiber_status {
            assert_eq!(value, "done");
            assert!(sent);
            assert_eq!(selected, 1);
            assert_eq!(transferred, 1);
            return;
        }
        assert!(
            !matches!(step.fiber_status, FlowFiberStatus::Failed(_)),
            "{:?}",
            step.fiber_status
        );
    }
    panic!("source-lowered Choice must reach its selected Flow");
}
