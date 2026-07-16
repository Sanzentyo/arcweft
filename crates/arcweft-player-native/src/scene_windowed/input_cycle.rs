//! Pointer, keyboard, IME, clipboard, and dialogue input handling.

use super::{
    ButtonSource, CompositionEndReason, DialogueProgress, ElementState, Ime, ImeEnableRequest,
    ImeRequest, ImeRequestData, ImeRequestError, InputOutcome, Key, KeyEvent, KeyPhase,
    ModifiersState, MouseButton, MouseScrollDelta, NamedKey, NativeSceneState,
    NativeSceneWindowError, NativeTextInputFocusReason, PhysicalPosition, PointerId, PreparedFrame,
    PreparedTextInputTarget, TextCommit, TextCompositionUpdate, TextDeleteUnit, TextEditCommand,
    TextInput, TextInputKeyDisposition, TextInputOperation, TextInputPrivacy, TextInputSerial,
    ToPrimitive, ViewportPoint, WheelDelta, WheelNormalizationPolicy, focused_text_input_control,
    key_label, left_arrow_text_command, pointer_id, right_arrow_text_command,
    shortcut_command_from_key, shortcut_modifier_active, text_input_commit_from_key_text,
    window_ime_capabilities_for_request, window_ime_composition_selection, window_ime_request_data,
};
use arcweft_runtime_host::clipboard_host::SyncTextClipboardHostAdapter;

impl NativeSceneState {
    pub(super) fn pointer_move(&mut self, position: PhysicalPosition<f64>) {
        if let Some(frame) = self.prepared.clone() {
            self.input
                .pointer_move(&frame, PointerId(0), self.logical_position(position));
            self.window.request_redraw();
        }
    }

    pub(super) fn pointer_button(
        &mut self,
        button: &ButtonSource,
        element_state: ElementState,
        position: PhysicalPosition<f64>,
    ) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let pointer = pointer_id(button);
        let position = self.logical_position(position);
        let modifiers = arcweft_player_scene::input::InputPointerModifiers::new(
            self.keyboard_modifiers.shift_key(),
        );
        let outcome = match element_state {
            ElementState::Pressed if button.clone().mouse_button() == Some(MouseButton::Right) => {
                self.input
                    .pointer_context_menu(&frame, pointer, position, modifiers)
            }
            ElementState::Pressed => self
                .input
                .pointer_down(&frame, pointer, position, modifiers),
            ElementState::Released => self.input.pointer_up(&frame, pointer, position, modifiers),
        };
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::Pointer)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    pub(super) fn wheel(&mut self, delta: MouseScrollDelta) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let delta = match delta {
            MouseScrollDelta::LineDelta(x, y) => WheelDelta::lines(f64::from(x), f64::from(y)),
            MouseScrollDelta::PixelDelta(position) => WheelDelta::from_physical_pixels(
                position.x,
                position.y,
                self.window.scale_factor(),
            )?,
        };
        let delta = WheelNormalizationPolicy::default().normalize(delta)?;
        let outcome = self
            .input
            .precision_scroll(&frame, delta.horizontal(), delta.vertical());
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::RedrawRefresh)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    pub(super) fn keyboard(&mut self, event: &KeyEvent) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let phase = match event.state {
            ElementState::Pressed => KeyPhase::Down,
            ElementState::Released => KeyPhase::Up,
        };
        if phase == KeyPhase::Down
            && let Some(operation) =
                self.text_input_operation_from_key_event(event, self.keyboard_modifiers)
        {
            self.apply_window_ime_operations(vec![operation])?;
            return Ok(());
        }
        let label = key_label(&event.logical_key);
        let disposition = self.text_input.backend_key_disposition(&label);
        let player_disposition = if self.text_input.shortcuts_allowed(disposition) {
            disposition
        } else {
            TextInputKeyDisposition::ImeConsumed
        };
        let outcome = self.input.keyboard_with_modifiers_and_ime(
            &frame,
            &label,
            phase,
            self.keyboard_modifiers.shift_key(),
            player_disposition,
        );
        self.apply_outcome(outcome)?;
        self.window.request_redraw();
        Ok(())
    }

    fn text_input_operation_from_key_event(
        &self,
        event: &KeyEvent,
        modifiers: ModifiersState,
    ) -> Option<TextInputOperation> {
        let editor = self.input.focused_text_editor()?;
        let selecting = modifiers.shift_key() && editor.options().selection_enabled();
        if editor.options().shortcuts_enabled()
            && let Some(command) = shortcut_command_from_key(&event.logical_key, modifiers)
        {
            return Some(TextInputOperation::Command(command));
        }
        match &event.logical_key {
            Key::Named(NamedKey::Backspace) if modifiers.control_key() || modifiers.alt_key() => {
                Some(TextInputOperation::Command(TextEditCommand::DeleteWordLeft))
            }
            Key::Named(NamedKey::Backspace) => {
                Some(TextInputOperation::Command(TextEditCommand::Backspace))
            }
            Key::Named(NamedKey::Delete) if modifiers.control_key() || modifiers.alt_key() => Some(
                TextInputOperation::Command(TextEditCommand::DeleteWordRight),
            ),
            Key::Named(NamedKey::Delete) => {
                Some(TextInputOperation::Command(TextEditCommand::Delete))
            }
            Key::Named(NamedKey::ArrowLeft) => Some(TextInputOperation::Command(
                left_arrow_text_command(modifiers, selecting),
            )),
            Key::Named(NamedKey::ArrowRight) => Some(TextInputOperation::Command(
                right_arrow_text_command(modifiers, selecting),
            )),
            Key::Named(NamedKey::ArrowUp) => {
                Some(TextInputOperation::Command(TextEditCommand::MoveUp {
                    selecting,
                }))
            }
            Key::Named(NamedKey::ArrowDown) => {
                Some(TextInputOperation::Command(TextEditCommand::MoveDown {
                    selecting,
                }))
            }
            Key::Named(NamedKey::PageUp) => {
                Some(TextInputOperation::Command(TextEditCommand::MovePageUp {
                    selecting,
                }))
            }
            Key::Named(NamedKey::PageDown) => {
                Some(TextInputOperation::Command(TextEditCommand::MovePageDown {
                    selecting,
                }))
            }
            Key::Named(NamedKey::Home) => {
                let command = if modifiers.control_key() || modifiers.meta_key() {
                    TextEditCommand::MoveDocumentStart { selecting }
                } else {
                    TextEditCommand::MoveLineStart { selecting }
                };
                Some(TextInputOperation::Command(command))
            }
            Key::Named(NamedKey::End) => {
                let command = if modifiers.control_key() || modifiers.meta_key() {
                    TextEditCommand::MoveDocumentEnd { selecting }
                } else {
                    TextEditCommand::MoveLineEnd { selecting }
                };
                Some(TextInputOperation::Command(command))
            }
            Key::Named(NamedKey::Tab) if editor.options().tab_inserts_text() => {
                Some(TextInputOperation::Commit(TextCommit::new("\t")))
            }
            Key::Named(NamedKey::Enter) => {
                if editor.options().is_multiline() {
                    Some(TextInputOperation::Commit(TextCommit::new("\n")))
                } else {
                    Some(TextInputOperation::Command(TextEditCommand::Submit))
                }
            }
            Key::Named(NamedKey::Escape) => {
                Some(TextInputOperation::Command(TextEditCommand::Cancel))
            }
            _ if shortcut_modifier_active(modifiers) => None,
            _ => event
                .text
                .as_ref()
                .and_then(|text| text_input_commit_from_key_text(text.as_str())),
        }
    }

    pub(super) fn ime(&mut self, event: Ime) -> Result<(), NativeSceneWindowError> {
        match event {
            Ime::Enabled => {
                self.window_ime_supported = true;
                self.window_ime_enabled = true;
                if self.input.window_focused()
                    && let Some(frame) = self.prepared.clone()
                {
                    self.sync_window_ime(&frame);
                }
                Ok(())
            }
            Ime::Preedit(preedit, selection) => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                let selection = window_ime_composition_selection(&preedit, selection);
                let update = TextCompositionUpdate::new(preedit, selection);
                self.apply_window_ime_operations(vec![TextInputOperation::SetComposition(update)])
            }
            Ime::Commit(text) => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::Commit(TextCommit::new(
                    text,
                ))])
            }
            Ime::DeleteSurrounding {
                before_bytes,
                after_bytes,
            } => {
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::DeleteSurrounding {
                    before: u32::try_from(before_bytes).unwrap_or(u32::MAX),
                    after: u32::try_from(after_bytes).unwrap_or(u32::MAX),
                    unit: TextDeleteUnit::Utf8Byte,
                }])
            }
            Ime::Disabled => {
                self.window_ime_enabled = false;
                if !self.input.window_focused() {
                    return Ok(());
                }
                self.apply_window_ime_operations(vec![TextInputOperation::EndComposition {
                    reason: CompositionEndReason::PlatformDisabled,
                }])
            }
        }
    }

    pub(super) fn focus_changed(&mut self, focused: bool) -> Result<(), NativeSceneWindowError> {
        let outcome = self.input.focus_changed(focused);
        self.apply_outcome(outcome)?;
        if !focused {
            self.text_input.blur_active();
            self.disable_window_ime();
        }
        Ok(())
    }

    fn apply_window_ime_operations(
        &mut self,
        operations: Vec<TextInputOperation>,
    ) -> Result<(), NativeSceneWindowError> {
        if operations.is_empty() {
            return Ok(());
        }
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        let Some(editor) = self.input.focused_text_editor() else {
            return Ok(());
        };
        let session = editor.session();
        let privacy = if editor.options().is_secure() {
            TextInputPrivacy::Sensitive
        } else {
            TextInputPrivacy::Plain
        };
        let input = TextInput::new(session, self.next_window_ime_serial(), operations)
            .with_privacy(privacy);
        self.text_input
            .record_window_ime_text_input(&input, TextInputKeyDisposition::ImeConsumed);
        let outcome = self.input.text_input(&frame, input)?;
        self.apply_outcome(outcome)?;
        let prepared = self.prepare_frame()?;
        self.sync_text_input_bridge(&prepared.frame, NativeTextInputFocusReason::RedrawRefresh)?;
        self.sync_window_ime(&prepared.frame);
        self.prepared = Some(prepared.frame);
        self.window.request_redraw();
        Ok(())
    }

    fn next_window_ime_serial(&mut self) -> TextInputSerial {
        let serial = TextInputSerial(self.next_window_ime_serial);
        self.next_window_ime_serial = self.next_window_ime_serial.saturating_add(1);
        serial
    }

    pub(super) fn sync_text_input_bridge(
        &mut self,
        frame: &PreparedFrame,
        reason: NativeTextInputFocusReason,
    ) -> Result<(), NativeSceneWindowError> {
        self.text_input
            .sync_focus(focused_text_input_control(frame, reason))?;
        Ok(())
    }

    pub(super) fn sync_window_ime(&mut self, frame: &PreparedFrame) {
        if !self.window_ime_supported || !self.input.window_focused() {
            return;
        }
        let Some(PreparedTextInputTarget { snapshot, geometry }) =
            frame.focused_text_input_target()
        else {
            self.disable_window_ime();
            return;
        };
        let request = window_ime_request_data(&snapshot, &geometry);
        if self.window_ime_enabled {
            self.update_window_ime(request);
        } else {
            self.enable_window_ime(request);
        }
    }

    fn enable_window_ime(&mut self, request: ImeRequestData) {
        let capabilities = window_ime_capabilities_for_request(&request);
        let Some(enable) = ImeEnableRequest::new(capabilities, request.clone()) else {
            self.window_ime_supported = false;
            return;
        };
        match self.window.request_ime_update(ImeRequest::Enable(enable)) {
            Ok(()) | Err(ImeRequestError::AlreadyEnabled) => {
                self.window_ime_enabled = true;
                self.update_window_ime(request);
            }
            Err(ImeRequestError::NotEnabled) => self.window_ime_enabled = false,
            Err(_) => self.mark_window_ime_unsupported(),
        }
    }

    fn update_window_ime(&mut self, request: ImeRequestData) {
        match self
            .window
            .request_ime_update(ImeRequest::Update(request.clone()))
        {
            Ok(()) | Err(ImeRequestError::AlreadyEnabled) => self.window_ime_enabled = true,
            Err(ImeRequestError::NotEnabled) => {
                self.window_ime_enabled = false;
                self.enable_window_ime(request);
            }
            Err(_) => self.mark_window_ime_unsupported(),
        }
    }

    fn disable_window_ime(&mut self) {
        if self.window_ime_enabled {
            let _ = self.window.request_ime_update(ImeRequest::Disable);
        }
        self.window_ime_enabled = false;
    }

    fn mark_window_ime_unsupported(&mut self) {
        self.window_ime_supported = false;
        self.window_ime_enabled = false;
    }

    fn apply_outcome(&mut self, outcome: InputOutcome) -> Result<(), NativeSceneWindowError> {
        let InputOutcome {
            actions,
            text_control_write_backs,
            clipboard_requests,
            diagnostics: _,
            dialogue_progress,
            cancel: _,
            redraw: _,
        } = outcome;
        self.apply_dialogue_progress(dialogue_progress);
        for action in actions {
            self.runtime.session_mut().queue_semantic_action(&action)?;
        }
        self.text_input
            .record_runtime_write_backs(&text_control_write_backs);
        self.runtime
            .session_mut()
            .queue_text_control_write_backs(text_control_write_backs)?;
        self.apply_clipboard_requests(clipboard_requests)?;
        Ok(())
    }

    fn apply_clipboard_requests(
        &mut self,
        clipboard_requests: Vec<arcweft_presentation::clipboard::TextClipboardRequest>,
    ) -> Result<(), NativeSceneWindowError> {
        let Some(frame) = self.prepared.clone() else {
            return Ok(());
        };
        for request in clipboard_requests {
            let host_outcome = self.clipboard.apply_clipboard_request_sync(request);
            let outcome = self.input.apply_clipboard_outcome(&frame, host_outcome)?;
            let InputOutcome {
                actions,
                text_control_write_backs,
                clipboard_requests,
                diagnostics: _,
                dialogue_progress,
                cancel: _,
                redraw: _,
            } = outcome;
            self.apply_dialogue_progress(dialogue_progress);
            for action in actions {
                self.runtime.session_mut().queue_semantic_action(&action)?;
            }
            self.text_input
                .record_runtime_write_backs(&text_control_write_backs);
            self.runtime
                .session_mut()
                .queue_text_control_write_backs(text_control_write_backs)?;
            if !clipboard_requests.is_empty() {
                self.apply_clipboard_requests(clipboard_requests)?;
            }
        }
        Ok(())
    }

    fn apply_dialogue_progress(&mut self, progress: DialogueProgress) {
        match progress {
            DialogueProgress::None => {}
            DialogueProgress::Reveal => self.dialogue_visual_clock.complete_current_stage(),
            DialogueProgress::Advance { target } => {
                self.runtime.session_mut().queue_dialogue_advance(target);
            }
        }
    }

    fn logical_position(&self, position: PhysicalPosition<f64>) -> ViewportPoint {
        ViewportPoint::new(
            (position.x / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
            (position.y / self.window.scale_factor())
                .to_f32()
                .unwrap_or(0.0),
        )
    }
}
