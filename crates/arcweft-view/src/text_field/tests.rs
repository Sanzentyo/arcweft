use super::{TextEditError, TextEditState, TextEditorPart, TextFieldMetrics, TextFieldPartRect};
use crate::text_source::ViewTextSource;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::text_input::{
    TextByteOffset, TextCommit, TextCompositionUpdate, TextInput, TextInputOperation,
    TextInputSerial, TextInputSessionId, TextRange,
};

#[test]
fn composition_visual_source_does_not_mutate_committed_document() {
    let mut state = TextEditState::new("abc");
    state.bind_session(TextInputSessionId(4));
    state.set_composition(
        TextCompositionUpdate::new("かな", TextRange::new(TextByteOffset(0), TextByteOffset(6)))
            .with_replacement(TextRange::new(TextByteOffset(1), TextByteOffset(2))),
    );

    let visual = state.visual_source();

    assert_eq!(state.document(), "abc");
    assert_eq!(state.session(), Some(TextInputSessionId(4)));
    assert_eq!(visual, ViewTextSource::plain("aかなc"));
}

#[test]
fn text_input_batches_reject_stale_sessions_without_mutation() {
    let mut state = TextEditState::new("abc");
    state.bind_session(TextInputSessionId(1));
    let input = TextInput::single(
        TextInputSessionId(2),
        TextInputSerial(1),
        TextInputOperation::Commit(TextCommit::new("x")),
    );

    let error = state
        .apply_text_input(&input)
        .expect_err("stale session rejects");

    assert_eq!(
        error,
        TextEditError::StaleTextInputSession {
            active: Some(TextInputSessionId(1)),
            incoming: TextInputSessionId(2),
        }
    );
    assert_eq!(state.document(), "abc");
}

#[test]
fn committed_text_updates_document_after_session_check() {
    let mut state = TextEditState::new("abc");
    state.bind_session(TextInputSessionId(1));
    let input = TextInput::single(
        TextInputSessionId(1),
        TextInputSerial(1),
        TextInputOperation::Commit(TextCommit::new("x")),
    );

    let outcome = state.apply_text_input(&input).expect("commit applies");

    assert!(outcome.changed());
    assert_eq!(state.document(), "xabc");
}

#[test]
fn visual_buffer_orders_selection_composition_and_caret_parts() {
    let mut state = TextEditState::new("abc");
    state.bind_session(TextInputSessionId(4));
    state.set_composition(TextCompositionUpdate::new(
        "かな",
        TextRange::new(TextByteOffset(0), TextByteOffset(6)),
    ));
    let buffer = state.visual_buffer(
        None,
        HitRect::new(0.0, 0.0, 100.0, 20.0),
        TextFieldMetrics::default(),
        true,
    );

    assert_eq!(buffer.display_text(), "•••••");
    assert!(buffer.is_secure());
    assert_eq!(
        buffer.parts().last().map(TextFieldPartRect::part),
        Some(TextEditorPart::Caret)
    );
    assert!(
        buffer
            .parts()
            .iter()
            .any(|part| part.part() == TextEditorPart::Composition)
    );
}
