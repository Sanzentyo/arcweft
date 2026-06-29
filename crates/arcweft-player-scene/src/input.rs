use arcweft_id::PublicId;
use arcweft_presentation::input::{
    Action, InputEpoch, KeyPhase, PointerId, PointerInput, PointerPhase, RawInputEvent,
    RawInputKind, ViewportPoint,
};
use arcweft_presentation::interaction::{
    FocusState, InteractionState, PointerCapture, PressedTarget,
};
use arcweft_presentation::router::{InputRouter, RouteDecision};
use arcweft_presentation::text_editor::{TextEditorClipboard, TextEditorError, TextEditorState};
use arcweft_presentation::text_input::{TextInput, TextInputKeyDisposition, TextInputOperation};
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
    pub redraw: bool,
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
        InputOutcome {
            actions: Vec::new(),
            redraw: true,
        }
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
        InputOutcome {
            actions: Vec::new(),
            redraw: true,
        }
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
        InputOutcome {
            actions,
            redraw: true,
        }
    }

    pub fn pointer_cancel(&mut self, pointer: PointerId) -> InputOutcome {
        self.pointer_positions.remove(&pointer.0);
        self.pressed.remove(&pointer.0);
        self.drags.remove(&pointer.0);
        self.interaction.clear_pointer(pointer);
        InputOutcome {
            actions: Vec::new(),
            redraw: true,
        }
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
                InputOutcome {
                    actions: Vec::new(),
                    redraw: true,
                }
            }
            "ArrowDown" | "ArrowRight" => {
                let next = frame.adjacent_choice_target(self.interaction.focus().target(), 1);
                if let Some(next) = next {
                    self.set_focus(frame, next);
                }
                InputOutcome {
                    actions: Vec::new(),
                    redraw: true,
                }
            }
            "Home" => {
                if let Some(target) = frame.first_choice_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome {
                    actions: Vec::new(),
                    redraw: true,
                }
            }
            "End" => {
                if let Some(target) = frame.last_choice_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome {
                    actions: Vec::new(),
                    redraw: true,
                }
            }
            "Enter" | " " | "Space" => InputOutcome {
                actions: self
                    .interaction
                    .focus()
                    .target()
                    .and_then(|target| choice_action(frame, target))
                    .into_iter()
                    .collect(),
                redraw: true,
            },
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
        if let Some(editor) = self
            .focused_text_editor
            .as_mut()
            .filter(|editor| editor.session() == input.session())
        {
            let _outputs = editor.apply_text_input(&input, &mut self.text_editor_clipboard)?;
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
            redraw: true,
        })
    }

    pub fn wheel(&mut self, _delta_y: f32) -> InputOutcome {
        InputOutcome {
            actions: Vec::new(),
            redraw: true,
        }
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
        InputOutcome {
            actions: Vec::new(),
            redraw: true,
        }
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
        TextByteOffset, TextInputOptions, TextInputSerial, TextInputSessionId, TextRange,
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
}
