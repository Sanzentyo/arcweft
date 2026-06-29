use arcweft_id::PublicId;
use arcweft_presentation::input::{
    Action, InputEpoch, KeyPhase, PointerId, PointerInput, PointerPhase, RawInputEvent,
    RawInputKind, ViewportPoint,
};
use arcweft_presentation::interaction::{
    FocusState, InteractionState, PointerCapture, PressedTarget,
};
use arcweft_presentation::router::{InputRouter, RouteDecision};
use arcweft_presentation::text_editor::{
    TextEditorClipboard, TextEditorError, TextEditorOutput, TextEditorState,
};
use arcweft_presentation::text_input::{
    TextControlValue, TextControlWriteBack, TextInput, TextInputKeyDisposition, TextInputOperation,
    TextInputPrivacy,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, FramePlanError, InteractionVisualState, PreparedFrame, RenderTextInputControl,
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct DragState {
    pub pointer: PointerId,
    pub target: arcweft_presentation::input::InteractionTarget,
    pub start: ViewportPoint,
    pub current: ViewportPoint,
}

impl DragState {
    pub fn distance_squared(&self) -> f32 {
        let dx = self.current.x - self.start.x;
        let dy = self.current.y - self.start.y;
        dx.mul_add(dx, dy * dy)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputOutcome {
    pub actions: Vec<Action>,
    pub text_control_write_backs: Vec<TextControlWriteBack>,
    pub dialogue_advance: bool,
    pub redraw: bool,
}

impl InputOutcome {
    pub fn text_control_write_backs(&self) -> &[TextControlWriteBack] {
        &self.text_control_write_backs
    }

    pub fn into_text_control_write_backs(self) -> Vec<TextControlWriteBack> {
        self.text_control_write_backs
    }

    fn redraw(redraw: bool) -> Self {
        Self {
            actions: Vec::new(),
            text_control_write_backs: Vec::new(),
            dialogue_advance: false,
            redraw,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputController {
    next_epoch: u64,
    interaction: InteractionState,
    pointer_positions: BTreeMap<u64, ViewportPoint>,
    pressed: BTreeMap<u64, arcweft_presentation::input::InteractionTarget>,
    drags: BTreeMap<u64, DragState>,
    choice_scroll: ChoiceScroll,
    window_focused: bool,
    ime_composing: bool,
    focused_text_editor: Option<TextEditorState>,
    text_editor_clipboard: TextEditorClipboard,
}

impl InputController {
    pub const fn interaction(&self) -> &InteractionState {
        &self.interaction
    }

    pub const fn choice_scroll(&self) -> ChoiceScroll {
        self.choice_scroll
    }

    pub const fn window_focused(&self) -> bool {
        self.window_focused
    }

    pub const fn ime_composing(&self) -> bool {
        self.ime_composing
    }

    pub const fn focused_text_editor(&self) -> Option<&TextEditorState> {
        self.focused_text_editor.as_ref()
    }

    pub fn pointer_position(&self, pointer: PointerId) -> Option<ViewportPoint> {
        self.pointer_positions.get(&pointer.0).copied()
    }

    pub fn visual_state(&self) -> InteractionVisualState {
        InteractionVisualState {
            focused: self.interaction.focus().target().cloned(),
            hovered: self.interaction.primary_hovered_target().cloned(),
            pressed: self.interaction.primary_pressed_target().cloned(),
        }
    }

    /// Activates the player-owned editor for a declarative runtime text control.
    ///
    /// Platform bridges still receive focus exclusively from
    /// `PreparedFrame::focused_text_input_target`; this state is the live
    /// product editor that validated platform edits mutate before the next
    /// frame is planned.
    pub fn activate_text_control(
        &mut self,
        control: &RenderTextInputControl,
    ) -> Result<bool, FramePlanError> {
        let options = control.resolved_options()?;
        let already_active = self.focused_text_editor.as_ref().is_some_and(|editor| {
            editor.session() == control.session && editor.target() == &control.target
        });
        if already_active {
            return Ok(false);
        }
        self.focused_text_editor = Some(TextEditorState::from_text_control(
            control.session,
            control.target.clone(),
            control.value.clone(),
            control.selection,
            options,
        )?);
        Ok(true)
    }

    #[must_use]
    pub fn apply_live_text_control_state(
        &self,
        control: RenderTextInputControl,
    ) -> RenderTextInputControl {
        self.focused_text_editor
            .as_ref()
            .map_or(control.clone(), |editor| {
                if editor.session() == control.session && editor.target() == &control.target {
                    control
                        .with_value(editor.text())
                        .with_selection(editor.selection())
                        .with_options(editor.options().clone())
                } else {
                    control
                }
            })
    }

    pub fn ensure_choice_focus(&mut self, frame: &PreparedFrame) {
        if self.interaction.focus().target().is_none()
            && let Some(target) = frame.first_choice_target()
        {
            self.set_focus(frame, target);
        }
    }

    pub fn pointer_move(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        match InputRouter::hover_path(pointer, position, &frame.layers, &frame.hits) {
            Some(path) => {
                let _ = self.interaction.set_hover_path(path);
            }
            None => {
                let _ = self.interaction.clear_hover(pointer);
            }
        }
        if let Some(drag) = self.drags.get_mut(&pointer.0) {
            drag.current = position;
        }
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Move,
        }));
        let _ = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        InputOutcome::redraw(true)
    }

    pub fn pointer_down(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Down,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        if let RouteDecision::Routed(event) = routed.decision() {
            let target = event.target().clone();
            if let Some(node) = frame.semantics.find(&target) {
                self.interaction
                    .set_focus(FocusState::new(node.layer().clone(), target.clone()));
                self.interaction.capture_pointer(PointerCapture::new(
                    pointer,
                    node.layer().clone(),
                    target.clone(),
                ));
                self.interaction.press_pointer(PressedTarget::new(
                    pointer,
                    node.layer().clone(),
                    target.clone(),
                ));
                self.pressed.insert(pointer.0, target.clone());
                self.drags.insert(
                    pointer.0,
                    DragState {
                        pointer,
                        target,
                        start: position,
                        current: position,
                    },
                );
            }
        }
        InputOutcome::redraw(true)
    }

    pub fn pointer_up(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Up,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        let released = self.pressed.remove(&pointer.0);
        let _ = self.interaction.release_pressed(pointer);
        let _ = self.interaction.release_pointer(pointer);
        let drag = self.drags.remove(&pointer.0);
        let is_activation = drag
            .as_ref()
            .is_some_and(|drag| drag.distance_squared() <= 64.0);
        let actions = match (released, routed.decision(), is_activation) {
            (Some(pressed), RouteDecision::Routed(event), true) if &pressed == event.target() => {
                choice_action(frame, event.target()).into_iter().collect()
            }
            _ => Vec::new(),
        };
        activation_outcome(frame, actions, is_activation)
    }

    pub fn pointer_cancel(&mut self, pointer: PointerId) -> InputOutcome {
        self.pointer_positions.remove(&pointer.0);
        self.pressed.remove(&pointer.0);
        self.drags.remove(&pointer.0);
        self.interaction.clear_pointer(pointer);
        InputOutcome::redraw(true)
    }

    pub fn keyboard(&mut self, frame: &PreparedFrame, key: &str, phase: KeyPhase) -> InputOutcome {
        if phase == KeyPhase::Up {
            return InputOutcome::default();
        }
        match key {
            "ArrowUp" | "ArrowLeft" => {
                let next = frame.adjacent_choice_target(self.interaction.focus().target(), -1);
                if let Some(next) = next {
                    self.set_focus(frame, next);
                }
                InputOutcome::redraw(true)
            }
            "ArrowDown" | "ArrowRight" => {
                let next = frame.adjacent_choice_target(self.interaction.focus().target(), 1);
                if let Some(next) = next {
                    self.set_focus(frame, next);
                }
                InputOutcome::redraw(true)
            }
            "Home" => {
                if let Some(target) = frame.first_choice_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "End" => {
                if let Some(target) = frame.last_choice_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "Enter" | " " | "Space" => {
                let actions = self
                    .interaction
                    .focus()
                    .target()
                    .and_then(|target| choice_action(frame, target))
                    .into_iter()
                    .collect();
                activation_outcome(frame, actions, true)
            }
            _ => InputOutcome::default(),
        }
    }

    pub fn keyboard_with_ime(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        disposition: TextInputKeyDisposition,
    ) -> InputOutcome {
        if disposition.shortcuts_suppressed() || self.ime_composing {
            return InputOutcome {
                actions: Vec::new(),
                text_control_write_backs: Vec::new(),
                dialogue_advance: false,
                redraw: self.ime_composing,
            };
        }
        self.keyboard(frame, key, phase)
    }

    pub fn text_input(
        &mut self,
        frame: &PreparedFrame,
        input: TextInput,
    ) -> Result<InputOutcome, TextEditorError> {
        let mut text_control_write_backs = Vec::new();
        if let Some(editor) = self
            .focused_text_editor
            .as_mut()
            .filter(|editor| editor.session() == input.session())
        {
            let before_text = editor.text().to_owned();
            let before_selection = editor.selection();
            let outputs = editor.apply_text_input(&input, &mut self.text_editor_clipboard)?;
            let submitted = input.submits_runtime_text_control()
                || outputs
                    .iter()
                    .any(|output| matches!(output, TextEditorOutput::Submitted(_)));
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
        }
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
        Ok(InputOutcome {
            actions: Vec::new(),
            text_control_write_backs,
            dialogue_advance: false,
            redraw: true,
        })
    }

    pub fn wheel(&mut self, _delta_y: f32) -> InputOutcome {
        InputOutcome::redraw(true)
    }

    pub fn focus_changed(&mut self, focused: bool) -> InputOutcome {
        self.window_focused = focused;
        if !focused {
            self.interaction.clear_focus();
            self.pointer_positions.clear();
            self.pressed.clear();
            self.drags.clear();
            self.focused_text_editor = None;
            self.interaction.clear_pointer_state();
        }
        InputOutcome::redraw(true)
    }

    fn set_focus(
        &mut self,
        frame: &PreparedFrame,
        target: arcweft_presentation::input::InteractionTarget,
    ) {
        if let Some(node) = frame.semantics.find(&target) {
            self.interaction
                .set_focus(FocusState::new(node.layer().clone(), target));
        }
    }

    fn raw(&mut self, kind: RawInputKind) -> RawInputEvent {
        let epoch = InputEpoch(self.next_epoch);
        self.next_epoch = self.next_epoch.saturating_add(1);
        RawInputEvent::new(epoch, kind)
    }
}

fn activation_outcome(
    frame: &PreparedFrame,
    actions: Vec<Action>,
    is_activation: bool,
) -> InputOutcome {
    let dialogue_advance =
        is_activation && actions.is_empty() && frame.has_dialogue() && frame.choices.is_empty();
    InputOutcome {
        actions,
        text_control_write_backs: Vec::new(),
        dialogue_advance,
        redraw: true,
    }
}

fn choice_action(
    frame: &PreparedFrame,
    target: &arcweft_presentation::input::InteractionTarget,
) -> Option<Action> {
    let choice = frame.choice_for_target(target)?;
    let kind = PublicId::try_new("action.choice.select").ok()?;
    frame
        .semantics
        .lower_action(target, &kind)
        .ok()
        .map(|action| action.with_payload(choice.option_id.clone()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_presentation::hit::HitRect;
    use arcweft_presentation::semantic::SemanticRole;
    use arcweft_presentation::text_input::{
        TextByteOffset, TextCompositionUpdate, TextControlWriteBackKind, TextEditCommand,
        TextInputOperation, TextInputOptions, TextInputPrivacy, TextInputSerial,
        TextInputSessionId, TextRange,
    };
    use arcweft_render_wgpu::geometry::{
        RenderPreferences, RenderScene, RenderViewport, SharedFramePlanner,
    };

    fn target(name: &str) -> arcweft_presentation::input::InteractionTarget {
        arcweft_presentation::input::InteractionTarget::new(
            PublicId::try_new(format!("target.{name}")).unwrap(),
        )
    }

    fn scene(control: RenderTextInputControl) -> RenderScene {
        RenderScene {
            dialogue: None,
            choices: Vec::new(),
            text_inputs: vec![control],
            images: Vec::new(),
            viewport: RenderViewport {
                logical_width: 640.0,
                logical_height: 360.0,
                physical_width: 640,
                physical_height: 360,
                scale_factor: 1.0,
            },
            visual_time_millis: 0,
            preferences: RenderPreferences::default(),
            interaction: InteractionVisualState::default(),
            choice_scroll: ChoiceScroll::default(),
        }
    }

    #[test]
    fn text_input_edits_player_owned_focused_text_editor_state() {
        let target = target("text_input.editor");
        let session = TextInputSessionId(42);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "abc",
            TextRange::new(TextByteOffset(3), TextByteOffset(3)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();
        let outcome = input
            .text_input(
                &frame,
                TextInput::committed(session, TextInputSerial(7), "d"),
            )
            .unwrap();

        assert!(outcome.redraw);
        assert_eq!(input.focused_text_editor().unwrap().text(), "abcd");
        let next_control = input.apply_live_text_control_state(control);
        assert_eq!(next_control.value, "abcd");
    }

    #[test]
    fn committed_text_input_emits_typed_change_write_back() {
        let target = target("text_input.change");
        let session = TextInputSessionId(52);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "ab",
            TextRange::new(TextByteOffset(2), TextByteOffset(2)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let outcome = input
            .text_input(
                &frame,
                TextInput::committed(session, TextInputSerial(8), "c"),
            )
            .unwrap();

        assert_eq!(outcome.text_control_write_backs().len(), 1);
        let event = &outcome.text_control_write_backs()[0];
        assert_eq!(event.kind(), TextControlWriteBackKind::Change);
        assert_eq!(event.value().as_str(), "abc");
        assert_eq!(
            event.target(),
            input.focused_text_editor().unwrap().target()
        );
    }

    #[test]
    fn submit_command_is_distinguishable_from_change() {
        let target = target("text_input.submit");
        let session = TextInputSessionId(53);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "ready",
            TextRange::new(TextByteOffset(5), TextByteOffset(5)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let outcome = input
            .text_input(
                &frame,
                TextInput::single(
                    session,
                    TextInputSerial(9),
                    TextInputOperation::Command(TextEditCommand::Submit),
                ),
            )
            .unwrap();

        assert_eq!(outcome.text_control_write_backs().len(), 1);
        let event = &outcome.text_control_write_backs()[0];
        assert!(event.is_submit());
        assert!(!event.is_change());
        assert_eq!(event.value().as_str(), "ready");
    }

    #[test]
    fn ime_preedit_does_not_write_back_until_commit() {
        let target = target("text_input.ime");
        let session = TextInputSessionId(54);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let preedit = input
            .text_input(
                &frame,
                TextInput::single(
                    session,
                    TextInputSerial(10),
                    TextInputOperation::SetComposition(TextCompositionUpdate::new(
                        "に",
                        TextRange::new(TextByteOffset(0), TextByteOffset(3)),
                    )),
                ),
            )
            .unwrap();
        assert!(preedit.text_control_write_backs().is_empty());

        let commit = input
            .text_input(
                &frame,
                TextInput::committed(session, TextInputSerial(11), "日"),
            )
            .unwrap();
        assert_eq!(commit.text_control_write_backs().len(), 1);
        assert_eq!(commit.text_control_write_backs()[0].value().as_str(), "日");
    }

    #[test]
    fn no_op_delete_command_does_not_emit_change_write_back() {
        let target = target("text_input.noop_delete");
        let session = TextInputSessionId(56);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let outcome = input
            .text_input(
                &frame,
                TextInput::single(
                    session,
                    TextInputSerial(13),
                    TextInputOperation::Command(TextEditCommand::Backspace),
                ),
            )
            .unwrap();

        assert!(outcome.text_control_write_backs().is_empty());
    }

    #[test]
    fn secure_write_back_value_is_available_but_redacted_in_debug() {
        let target = target("text_input.secure");
        let session = TextInputSessionId(55);
        let control = RenderTextInputControl::new(
            target.clone(),
            session,
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default().secure(true),
            SemanticRole::SecureTextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&RenderScene {
            interaction: InteractionVisualState {
                focused: Some(target),
                hovered: None,
                pressed: None,
            },
            ..scene(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let outcome = input
            .text_input(
                &frame,
                TextInput::committed(session, TextInputSerial(12), "secret")
                    .with_privacy(TextInputPrivacy::Sensitive),
            )
            .unwrap();

        let event = &outcome.text_control_write_backs()[0];
        assert_eq!(event.value().as_str(), "secret");
        assert!(event.value().is_sensitive());
        let debug = format!("{event:?}");
        assert!(!debug.contains("secret"));
        assert!(debug.contains("<redacted>"));
    }
}
