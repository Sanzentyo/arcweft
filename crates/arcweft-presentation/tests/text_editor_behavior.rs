use arcweft_id::PublicId;
use arcweft_presentation::hit::HitRect;
use arcweft_presentation::input::InteractionTarget;
use arcweft_presentation::text_editor::{
    TextEditorClipboard, TextEditorError, TextEditorGlyphGeometry, TextEditorLayout,
    TextEditorLayoutParts, TextEditorLayoutSource, TextEditorOutput, TextEditorState,
};
use arcweft_presentation::text_index::TextIndexSnapshot;
use arcweft_presentation::text_input::{
    CompositionEndReason, PlatformTextSelection, TextByteOffset, TextCommit, TextCompositionUpdate,
    TextEditCommand, TextInput, TextInputOperation, TextInputOptions, TextInputPrivacy,
    TextInputSecurityPolicy, TextInputSerial, TextInputSessionId, TextRange, TextRevision,
    TextSelectionAffinity, TextWritingMode,
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
    let layout = TextEditorLayout::monospaced_fixture(HitRect::new(5.0, 7.0, 160.0, 24.0))
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

#[test]
fn renderer_backed_geometry_reports_mixed_cluster_bounds_and_range_rects() {
    let mut editor = editor("A日本e\u{301}🦀", false);
    let mut clipboard = TextEditorClipboard::default();
    editor
        .apply_text_input(
            &input(TextInputOperation::SetSelection(
                PlatformTextSelection::new(
                    TextRange::new(TextByteOffset(1), TextByteOffset(10)),
                    TextSelectionAffinity::Downstream,
                ),
            )),
            &mut clipboard,
        )
        .unwrap();
    let layout = TextEditorLayout::from_renderer_parts_for_text(
        editor.text(),
        TextEditorLayoutParts {
            source: TextEditorLayoutSource::Renderer,
            text_local_control_rect: HitRect::new(0.0, 0.0, 160.0, 28.0),
            glyphs: vec![
                glyph(0, 1, 4.0, 8.0),
                glyph(1, 4, 12.0, 18.0),
                glyph(4, 7, 30.0, 18.0),
                glyph(7, 10, 48.0, 12.0),
                glyph(10, 14, 60.0, 22.0),
            ],
            caret_width: 1.0,
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_to_viewport:
                arcweft_presentation::text_input::TextGeometryTransform::identity(),
            viewport_to_screen:
                arcweft_presentation::text_input::TextGeometryTransform::translation(100.0, 200.0),
        },
    )
    .unwrap();

    let geometry = editor.geometry_snapshot(&layout).unwrap();

    assert!(layout.is_renderer_backed());
    assert_eq!(geometry.screen_character_bounds().len(), 5);
    assert_eq!(geometry.screen_selection_rects().len(), 3);
    assert!(
        geometry.screen_character_bounds()[1].bounds.width
            > geometry.screen_character_bounds()[0].bounds.width
    );
}

#[test]
fn renderer_backed_hit_testing_uses_glyph_midpoints() {
    let mut editor = editor("a日本", false);
    let layout = TextEditorLayout::from_renderer_parts_for_text(
        editor.text(),
        TextEditorLayoutParts {
            source: TextEditorLayoutSource::Renderer,
            text_local_control_rect: HitRect::new(0.0, 0.0, 96.0, 24.0),
            glyphs: vec![
                glyph(0, 1, 0.0, 7.0),
                glyph(1, 4, 7.0, 20.0),
                glyph(4, 7, 27.0, 20.0),
            ],
            caret_width: 1.0,
            writing_mode: TextWritingMode::HorizontalTb,
            text_local_to_viewport:
                arcweft_presentation::text_input::TextGeometryTransform::identity(),
            viewport_to_screen: arcweft_presentation::text_input::TextGeometryTransform::identity(),
        },
    )
    .unwrap();

    editor
        .set_caret_from_text_local_point(&layout, 18.0, 8.0, false)
        .unwrap();

    assert_eq!(editor.caret(), TextByteOffset(4));
}

fn glyph(start: u32, end: u32, x: f32, width: f32) -> TextEditorGlyphGeometry {
    TextEditorGlyphGeometry::new(
        TextRange::new(TextByteOffset(start), TextByteOffset(end)),
        HitRect::new(x, 4.0, width, 18.0),
    )
}
