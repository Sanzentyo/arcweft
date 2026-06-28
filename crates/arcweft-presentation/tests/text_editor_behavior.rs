use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_editor::{
    TextEditorClipboard, TextEditorError, TextEditorLayout, TextEditorOutput, TextEditorState,
};
use arcweft_presentation::text_index::TextIndexSnapshot;
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextSelection, TextByteOffset, TextCommit, TextCompositionUpdate,
    TextEditCommand, TextInput, TextInputOperation, TextInputOptions, TextInputPrivacy,
    TextInputSecurityPolicy, TextInputSerial, TextInputSessionId, TextRange, TextRevision,
    TextSelectionAffinity,
};

fn target() -> InteractionTarget {
    InteractionTarget::new(PublicId::try_new("target.text.editor").unwrap())
}

fn editor(text: &str, secure: bool) -> TextEditorState {
    TextEditorState::new(
        TextInputSessionId(42),
        target(),
        text,
        TextInputOptions::default().secure(secure),
    )
    .unwrap()
}

fn input(operation: TextInputOperation) -> TextInput {
    TextInput::single(TextInputSessionId(42), TextInputSerial(1), operation)
}

#[test]
fn backspace_uses_shared_grapheme_boundary_for_combining_sequence() {
    let mut editor = editor("e\u{301}x", false);
    let mut clipboard = TextEditorClipboard::default();
    let accent_end = TextIndexSnapshot::new("e\u{301}x").grapheme_boundaries()[1];
    editor
        .apply_text_input(
            &input(TextInputOperation::SetSelection(
                PlatformTextSelection::new(
                    TextRange::new(accent_end, accent_end),
                    TextSelectionAffinity::Downstream,
                ),
            )),
            &mut clipboard,
        )
        .unwrap();

    editor
        .apply_text_input(
            &input(TextInputOperation::Command(TextEditCommand::Backspace)),
            &mut clipboard,
        )
        .unwrap();

    assert_eq!(editor.text(), "x");
    assert_eq!(editor.caret(), TextByteOffset(0));
}

#[test]
fn selected_text_replacement_is_deterministic() {
    let mut editor = editor("abc日本語", false);
    let mut clipboard = TextEditorClipboard::default();
    let index = TextIndexSnapshot::new("abc日本語");
    let replacement = TextRange::new(
        index
            .byte_offset_for_utf16(arcweft_presentation::text_input::TextUtf16Offset(1))
            .unwrap(),
        index
            .byte_offset_for_utf16(arcweft_presentation::text_input::TextUtf16Offset(5))
            .unwrap(),
    );

    editor
        .apply_text_input(
            &input(TextInputOperation::SetSelection(
                PlatformTextSelection::new(replacement, TextSelectionAffinity::Downstream),
            )),
            &mut clipboard,
        )
        .unwrap();
    editor
        .apply_text_input(
            &input(TextInputOperation::Commit(TextCommit::new("X"))),
            &mut clipboard,
        )
        .unwrap();

    assert_eq!(editor.text(), "aX語");
}

#[test]
fn composition_cancel_restores_original_replacement_text() {
    let mut editor = editor("かな", false);
    let mut clipboard = TextEditorClipboard::default();
    let all = TextRange::new(
        TextByteOffset(0),
        TextByteOffset(u32::try_from("かな".len()).unwrap()),
    );
    editor
        .apply_text_input(
            &input(TextInputOperation::SetSelection(
                PlatformTextSelection::new(all, TextSelectionAffinity::Downstream),
            )),
            &mut clipboard,
        )
        .unwrap();
    editor
        .apply_text_input(&input(TextInputOperation::StartComposition), &mut clipboard)
        .unwrap();
    editor
        .apply_text_input(
            &input(TextInputOperation::SetComposition(
                TextCompositionUpdate::new(
                    "仮名",
                    TextRange::new(TextByteOffset(6), TextByteOffset(6)),
                )
                .with_replacement(all),
            )),
            &mut clipboard,
        )
        .unwrap();
    let outputs = editor
        .apply_text_input(
            &input(TextInputOperation::EndComposition {
                reason: CompositionEndReason::Cancelled,
            }),
            &mut clipboard,
        )
        .unwrap();

    assert_eq!(editor.text(), "かな");
    assert_eq!(outputs, vec![TextEditorOutput::CancelledComposition]);
}

#[test]
fn secure_clipboard_commands_are_rejected_before_text_leaks() {
    let mut editor = editor("secret", true);
    let mut clipboard = TextEditorClipboard::default();
    editor
        .apply_text_input(
            &input(TextInputOperation::Command(TextEditCommand::SelectAll)),
            &mut clipboard,
        )
        .unwrap();

    let error = editor
        .apply_text_input(
            &input(TextInputOperation::Command(TextEditCommand::Copy))
                .with_privacy(TextInputPrivacy::Sensitive),
            &mut clipboard,
        )
        .unwrap_err();

    assert_eq!(
        error,
        TextEditorError::SecureClipboardCommand(TextEditCommand::Copy)
    );
    assert_eq!(clipboard.read(), None);
}

#[test]
fn geometry_snapshot_anchors_candidate_away_from_origin_after_transform() {
    let editor = editor("日本語", false);
    let layout = TextEditorLayout::new(HitRect::new(5.0, 7.0, 160.0, 24.0))
        .with_text_origin(12.0, 11.0)
        .with_grapheme_advance(16.0)
        .with_viewport_to_screen(
            arcweft_presentation::text_input::TextGeometryTransform::translation(100.0, 200.0),
        );

    let snapshots = editor.snapshots(&layout).unwrap();

    assert_eq!(snapshots.client().revision(), TextRevision(0));
    assert!(snapshots.geometry().candidate_anchor_rect().x >= 100.0);
    assert!(snapshots.geometry().candidate_anchor_rect().y >= 200.0);
    assert!(!snapshots.geometry().screen_character_bounds().is_empty());
}

#[test]
fn secure_snapshot_redacts_text_and_character_bounds() {
    let editor = editor("secret", true);
    let snapshots = editor.snapshots(&TextEditorLayout::default()).unwrap();
    let policy = TextInputSecurityPolicy::from_options(editor.options());
    let redacted = policy.redact_snapshot(snapshots.client());
    let redacted_geometry = policy.redact_geometry(snapshots.geometry());

    assert_eq!(redacted.surrounding_text(), "");
    assert!(redacted.character_bounds().is_empty());
    assert!(redacted_geometry.screen_character_bounds().is_empty());
}
