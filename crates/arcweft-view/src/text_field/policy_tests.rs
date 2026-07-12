use super::{
    TextDeleteUnit, TextEditCommand, TextEditState, TextFieldBindingCommitPolicy,
    TextFieldEditPolicy, TextFieldGeometryPolicy, TextFieldMetrics, TextFieldPolicyEditError,
};
use crate::text_source::ViewTextSource;
use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_input::{
    PlatformTextSelection, TextByteOffset, TextCommit, TextCompositionUpdate,
    TextGeometryTransform, TextInput, TextInputOperation, TextInputOptions, TextInputPrivacy,
    TextInputSerial, TextInputSessionId, TextRange, TextSelectionAffinity, TextWritingMode,
};

fn target() -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new("target.textfield").unwrap())
}

#[test]
fn candidate_updates_do_not_commit_binding_before_commit() {
    let mut state = TextEditState::new("abc");
    state.bind_session(TextInputSessionId(1));
    let preedit = TextInput::single(
        TextInputSessionId(1),
        TextInputSerial(1),
        TextInputOperation::SetComposition(TextCompositionUpdate::new(
            "にほんご",
            TextRange::new(TextByteOffset(0), TextByteOffset(12)),
        )),
    );
    let outcome = state
        .apply_text_input_with_policy(&preedit, TextFieldEditPolicy::plain())
        .unwrap();

    assert!(!outcome.should_commit_binding(TextFieldBindingCommitPolicy::OnCommittedEdit));
    assert_eq!(state.document(), "abc");
    assert_eq!(state.visual_source(), ViewTextSource::plain("にほんごabc"));
}

#[test]
fn commit_updates_binding_policy_after_preedit() {
    let mut state = TextEditState::new("");
    state.bind_session(TextInputSessionId(2));
    let input = TextInput::single(
        TextInputSessionId(2),
        TextInputSerial(2),
        TextInputOperation::Commit(TextCommit::new("日本語")),
    );
    let outcome = state
        .apply_text_input_with_policy(&input, TextFieldEditPolicy::plain())
        .unwrap();

    assert!(outcome.should_commit_binding(TextFieldBindingCommitPolicy::OnCommittedEdit));
    assert_eq!(state.document(), "日本語");
}

#[test]
fn delete_surrounding_preserves_emoji_and_combining_grapheme_boundaries() {
    let mut state = TextEditState::new("a👩‍💻e\u{301}b");
    state.bind_session(TextInputSessionId(3));
    state
        .apply_text_input_with_policy(
            &TextInput::single(
                TextInputSessionId(3),
                TextInputSerial(1),
                TextInputOperation::SetSelection(PlatformTextSelection::new(
                    TextRange::new(TextByteOffset(15), TextByteOffset(15)),
                    TextSelectionAffinity::Downstream,
                )),
            ),
            TextFieldEditPolicy::plain(),
        )
        .unwrap();
    state
        .apply_text_input_with_policy(
            &TextInput::single(
                TextInputSessionId(3),
                TextInputSerial(2),
                TextInputOperation::DeleteSurrounding {
                    before: 1,
                    after: 0,
                    unit: TextDeleteUnit::GraphemeCluster,
                },
            ),
            TextFieldEditPolicy::plain(),
        )
        .unwrap();

    assert_eq!(state.document(), "a👩‍💻b");
}

#[test]
fn secure_policy_rejects_plain_batches_and_clipboard() {
    let mut state = TextEditState::new("");
    state.bind_session(TextInputSessionId(4));
    let input = TextInput::single(
        TextInputSessionId(4),
        TextInputSerial(1),
        TextInputOperation::Commit(TextCommit::new("secret")),
    );
    assert_eq!(
        state.apply_text_input_with_policy(&input, TextFieldEditPolicy::secure()),
        Err(TextFieldPolicyEditError::SecureInputNotRedacted)
    );
    let copy = TextInput::single(
        TextInputSessionId(4),
        TextInputSerial(2),
        TextInputOperation::Command(TextEditCommand::Copy),
    )
    .with_privacy(TextInputPrivacy::Sensitive);
    assert_eq!(
        state.apply_text_input_with_policy(&copy, TextFieldEditPolicy::secure()),
        Err(TextFieldPolicyEditError::SecureClipboardCommand(
            TextEditCommand::Copy
        ))
    );
}

#[test]
fn secure_snapshot_redacts_value_composition_and_character_bounds() {
    let mut state = TextEditState::new("secret");
    state.bind_session(TextInputSessionId(5));
    state.set_composition(TextCompositionUpdate::new(
        "preedit",
        TextRange::new(TextByteOffset(0), TextByteOffset(7)),
    ));
    let snapshot = state.text_input_client_snapshot(
        TextInputSessionId(5),
        target(),
        HitRect::new(0.0, 0.0, 200.0, 24.0),
        TextFieldMetrics::default(),
        TextInputOptions::default(),
        TextFieldEditPolicy::secure(),
    );

    assert!(snapshot.surrounding_text().is_empty());
    assert!(snapshot.composition().is_none());
    assert!(snapshot.character_bounds().is_empty());
    assert!(snapshot.options().is_secure());
}

#[test]
fn candidate_anchor_converts_after_scroll_transform_and_vertical_writing() {
    let mut state = TextEditState::new("abcd");
    state.bind_session(TextInputSessionId(6));
    state
        .apply_text_input_with_policy(
            &TextInput::single(
                TextInputSessionId(6),
                TextInputSerial(1),
                TextInputOperation::SetSelection(PlatformTextSelection::new(
                    TextRange::new(TextByteOffset(2), TextByteOffset(2)),
                    TextSelectionAffinity::Downstream,
                )),
            ),
            TextFieldEditPolicy::plain(),
        )
        .unwrap();
    let geometry = state.text_input_geometry_snapshot(
        TextInputSessionId(6),
        HitRect::new(5.0, 7.0, 100.0, 100.0),
        TextFieldMetrics::default(),
        TextFieldGeometryPolicy::default()
            .with_writing_mode(TextWritingMode::VerticalRl)
            .with_text_local_to_viewport(TextGeometryTransform::translation(10.0, 20.0))
            .with_viewport_to_screen(TextGeometryTransform::translation(100.0, 200.0)),
    );

    assert_eq!(geometry.writing_mode(), TextWritingMode::VerticalRl);
    assert!(geometry.candidate_anchor_rect().x >= 110.0);
    assert!(geometry.candidate_anchor_rect().y >= 220.0);
}
