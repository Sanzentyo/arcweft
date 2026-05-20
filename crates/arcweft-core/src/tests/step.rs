use crate::step::{InputEvent, RuntimeDiagnostic, RuntimeStepInput, RuntimeStepOutput};
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
