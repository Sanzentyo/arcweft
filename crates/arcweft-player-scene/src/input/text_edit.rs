use super::{
    DialogueProgress, InputController, InputOutcome, InputRouter, PreparedFrame, RawInputKind,
    TextClipboardOutcome, TextClipboardRequest, TextControlValue, TextControlWriteBack,
    TextEditorError, TextEditorLayout, TextEditorOutput, TextInput, TextInputOperation,
    TextInputPrivacy, dialogue_progress_for_frame, focused_editor_matches_frame,
    text_control_change_writeback,
};

impl InputController {
    #[allow(
        clippy::too_many_lines,
        reason = "Text input updates share editor state, write-back, semantic action, and dialogue gating in one event path."
    )]
    pub fn text_input(
        &mut self,
        frame: &PreparedFrame,
        input: TextInput,
    ) -> Result<InputOutcome, TextEditorError> {
        let mut text_control_write_backs = Vec::new();
        let mut clipboard_requests = Vec::new();
        let mut editor_outputs = Vec::new();
        let stale = self.focused_text_editor.as_ref().is_some_and(|editor| {
            editor.session() == input.session() && !focused_editor_matches_frame(frame, editor)
        });
        if stale {
            self.deactivate_focused_text_editor();
        }
        let mut submitted_runtime_text_control = false;
        if let Some(editor) = self
            .focused_text_editor
            .as_mut()
            .filter(|editor| editor.session() == input.session())
        {
            let before_text = editor.text().to_owned();
            let before_selection = editor.selection();
            let visual_layout = frame
                .focused_text_input_target()
                .filter(|focused| {
                    focused.snapshot.session() == editor.session()
                        && focused.snapshot.target() == editor.target()
                        && editor.options().visual_line_vertical_navigation_enabled()
                })
                .and_then(|focused| {
                    TextEditorLayout::from_geometry_snapshot_for_text(
                        editor.text(),
                        &focused.geometry,
                    )
                    .ok()
                });
            let outputs = editor.apply_text_input_with_layout(
                &input,
                &mut self.text_editor_clipboard,
                visual_layout.as_ref(),
            )?;
            let submitted = input.submits_runtime_text_control()
                || outputs
                    .iter()
                    .any(|output| matches!(output, TextEditorOutput::Submitted(_)));
            submitted_runtime_text_control = submitted;
            let changed = input.commits_runtime_text_control_value()
                && (editor.text() != before_text || editor.selection() != before_selection);
            if changed || submitted {
                let privacy = if input.privacy().is_sensitive() || editor.options().is_secure() {
                    TextInputPrivacy::Sensitive
                } else {
                    TextInputPrivacy::Plain
                };
                let target = editor.target().clone();
                let session = editor.session();
                let value = TextControlValue::new(editor.text(), privacy);
                let selection = editor.selection();
                let revision = editor.revision();
                if changed {
                    text_control_write_backs.push(TextControlWriteBack::change(
                        target.clone(),
                        session,
                        value.clone(),
                        selection,
                        revision,
                    ));
                }
                if submitted {
                    text_control_write_backs.push(TextControlWriteBack::submit(
                        target, session, value, selection, revision,
                    ));
                }
            }
            editor_outputs = outputs;
        }
        clipboard_requests.extend(self.clipboard_requests_from_editor_outputs(&editor_outputs));
        self.ime_composing = input.operations().iter().fold(
            self.ime_composing,
            |active, operation| match operation {
                TextInputOperation::StartComposition | TextInputOperation::SetComposition(_) => {
                    true
                }
                TextInputOperation::Commit(_)
                | TextInputOperation::EndComposition { .. }
                | TextInputOperation::Command(
                    arcweft_presentation::text_input::TextEditCommand::Cancel
                    | arcweft_presentation::text_input::TextEditCommand::Submit,
                ) => false,
                TextInputOperation::DeleteSurrounding { .. }
                | TextInputOperation::SetSelection(_)
                | TextInputOperation::Command(_) => active,
            },
        );
        let raw = self.raw(RawInputKind::Text(input));
        let _ = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        let dialogue_progress = dialogue_progress_for_frame(
            frame,
            submitted_runtime_text_control && self.dialogue_can_advance_from_unfocused_input(frame),
        );
        Ok(InputOutcome {
            actions: Vec::new(),
            view_handler_invocations: Vec::new(),
            text_control_write_backs,
            clipboard_requests,
            diagnostics: Vec::new(),
            dialogue_progress,
            cancel: false,
            redraw: true,
        })
    }

    pub fn apply_clipboard_outcome(
        &mut self,
        frame: &PreparedFrame,
        outcome: TextClipboardOutcome,
    ) -> Result<InputOutcome, TextEditorError> {
        let Some(request) = self
            .pending_clipboard_requests
            .remove(&outcome.request_id())
        else {
            return Ok(InputOutcome::redraw(false));
        };
        let mut text_control_write_backs = Vec::new();
        let mut redraw = false;

        match outcome {
            TextClipboardOutcome::ReadCommitted { text, .. } => {
                if let Some(editor) = self.focused_text_editor.as_mut().filter(|editor| {
                    editor.session() == request.session()
                        && editor.target() == request.target()
                        && focused_editor_matches_frame(frame, editor)
                }) {
                    editor.paste_text(text.as_str())?;
                    text_control_write_backs.push(text_control_change_writeback(editor));
                    redraw = true;
                }
            }
            TextClipboardOutcome::Failed { error, .. }
                if request.operation()
                    == arcweft_presentation::clipboard::TextClipboardOperation::Paste
                    && error.kind().may_use_local_fallback() =>
            {
                if let Some(editor) = self.focused_text_editor.as_mut().filter(|editor| {
                    editor.session() == request.session()
                        && editor.target() == request.target()
                        && focused_editor_matches_frame(frame, editor)
                }) && editor
                    .paste_local_clipboard(&self.text_editor_clipboard)
                    .is_ok()
                {
                    text_control_write_backs.push(text_control_change_writeback(editor));
                    redraw = true;
                }
            }
            TextClipboardOutcome::WriteCommitted { .. }
            | TextClipboardOutcome::Cleared { .. }
            | TextClipboardOutcome::Failed { .. } => {}
        }

        Ok(InputOutcome {
            actions: Vec::new(),
            view_handler_invocations: Vec::new(),
            text_control_write_backs,
            clipboard_requests: Vec::new(),
            diagnostics: Vec::new(),
            dialogue_progress: DialogueProgress::None,
            cancel: false,
            redraw,
        })
    }

    fn clipboard_requests_from_editor_outputs(
        &mut self,
        outputs: &[TextEditorOutput],
    ) -> Vec<TextClipboardRequest> {
        outputs
            .iter()
            .filter_map(|output| match output {
                TextEditorOutput::Clipboard(intent) => {
                    let request_id = self.next_clipboard_request_id.next();
                    self.next_clipboard_request_id = request_id;
                    let request = intent.clone().into_request(request_id);
                    self.pending_clipboard_requests
                        .insert(request_id, request.clone());
                    Some(request)
                }
                TextEditorOutput::None
                | TextEditorOutput::Submitted(_)
                | TextEditorOutput::CancelledComposition => None,
            })
            .collect()
    }
}
