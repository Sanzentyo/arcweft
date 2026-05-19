use crate::frame::{FrameInput, FrameOutput, InputEvent, RuntimeDiagnostic};
use crate::time::{LogicalDuration, TickId};

#[test]
fn frame_input_view_borrows_adapter_owned_events() {
    let input = FrameInput {
        tick: TickId(7),
        dt: LogicalDuration::from_nanos(16_000_000),
        input_events: vec![InputEvent {
            kind: "advance".to_owned(),
            payload: None,
        }],
        ..FrameInput::default()
    };

    let view = input.as_view();

    assert_eq!(view.tick(), TickId(7));
    assert_eq!(view.dt(), LogicalDuration::from_nanos(16_000_000));
    assert_eq!(view.input_events()[0].kind, "advance");
    assert!(view.external_values().is_empty());
}

#[test]
fn frame_output_writer_scopes_mutation_without_taking_output() {
    let mut output = FrameOutput::default();
    {
        let mut writer = output.writer();
        writer.push_diagnostic("first");
        writer.merge(FrameOutput {
            diagnostics: vec![RuntimeDiagnostic {
                message: "second".to_owned(),
            }],
            ..FrameOutput::default()
        });
    }

    let messages = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert_eq!(messages, ["first", "second"]);
}
