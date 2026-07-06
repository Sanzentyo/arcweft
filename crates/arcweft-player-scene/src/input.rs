use crate::controller::{
    ControllerInputChange, ControllerInputNormalizer, NormalizedControllerAction,
};
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
    TextEditorClipboard, TextEditorError, TextEditorLayout, TextEditorOutput, TextEditorState,
};
use arcweft_presentation::text_input::{
    CompositionEndReason, TextByteOffset, TextCharacterBounds, TextControlValue,
    TextControlWriteBack, TextInput, TextInputKeyDisposition, TextInputOperation, TextInputPrivacy,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, FocusNavigationDirection, FramePlanError, InteractionVisualState, PreparedFrame,
    RenderActionButtonAction, RenderTextInputControl,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct DragState {
    pub pointer: PointerId,
    pub target: arcweft_presentation::input::InteractionTarget,
    pub start: ViewportPoint,
    pub current: ViewportPoint,
}

#[derive(Clone, Debug, PartialEq)]
struct TextPointerSelectionState {
    pointer: PointerId,
    target: arcweft_presentation::input::InteractionTarget,
    position: ViewportPoint,
    selecting: bool,
}

/// Portable player-input state that can be stored alongside a runtime save.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct InputControllerSnapshot {
    #[serde(default, skip_serializing_if = "is_zero_f32")]
    pub choice_scroll_offset_y: f32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scroll_offsets: Vec<InputScrollOffsetSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InputScrollOffsetSnapshot {
    pub region_id: String,
    pub offset_x: f32,
    pub offset_y: f32,
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum InputControllerSnapshotError {
    #[error("input snapshot has non-finite choice scroll offset {offset_y}")]
    NonFiniteChoiceScroll { offset_y: f32 },
    #[error("input snapshot has negative choice scroll offset {offset_y}")]
    NegativeChoiceScroll { offset_y: f32 },
    #[error("input snapshot has an empty scroll region id")]
    EmptyScrollRegionId,
    #[error(
        "input snapshot has non-finite scroll offset ({offset_x}, {offset_y}) for region `{region_id}`"
    )]
    NonFiniteScrollOffset {
        region_id: String,
        offset_x: f32,
        offset_y: f32,
    },
    #[error(
        "input snapshot has negative scroll offset ({offset_x}, {offset_y}) for region `{region_id}`"
    )]
    NegativeScrollOffset {
        region_id: String,
        offset_x: f32,
        offset_y: f32,
    },
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
    pub diagnostics: Vec<InputDiagnostic>,
    pub dialogue_advance: bool,
    pub cancel: bool,
    pub redraw: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputDiagnostic {
    pub kind: InputDiagnosticKind,
    pub target: arcweft_presentation::input::InteractionTarget,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputDiagnosticKind {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ActionButtonSubmitOutcome {
    action: Option<Action>,
    write_back: Option<TextControlWriteBack>,
    diagnostic: Option<InputDiagnostic>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollOffset {
    x: f32,
    y: f32,
}

impl ScrollOffset {
    const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn is_zero(self) -> bool {
        is_zero_f32(&self.x) && is_zero_f32(&self.y)
    }
}

impl InputOutcome {
    pub fn actions(&self) -> &[Action] {
        &self.actions
    }

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
            diagnostics: Vec::new(),
            dialogue_advance: false,
            cancel: false,
            redraw,
        }
    }

    pub fn cancel() -> Self {
        Self {
            cancel: true,
            redraw: true,
            ..Self::default()
        }
    }

    pub fn merge(&mut self, other: Self) {
        self.actions.extend(other.actions);
        self.text_control_write_backs
            .extend(other.text_control_write_backs);
        self.diagnostics.extend(other.diagnostics);
        self.dialogue_advance |= other.dialogue_advance;
        self.cancel |= other.cancel;
        self.redraw |= other.redraw;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputController {
    next_epoch: u64,
    interaction: InteractionState,
    pointer_positions: BTreeMap<u64, ViewportPoint>,
    pressed: BTreeMap<u64, arcweft_presentation::input::InteractionTarget>,
    drags: BTreeMap<u64, DragState>,
    pending_text_pointer_selection: Option<TextPointerSelectionState>,
    choice_scroll: ChoiceScroll,
    scroll_offsets: BTreeMap<String, ScrollOffset>,
    controller: ControllerInputNormalizer,
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

    pub fn scroll_offset_y(&self, region_id: &str) -> f32 {
        self.scroll_offsets
            .get(region_id)
            .map_or(0.0, |offset| offset.y)
    }

    pub fn scroll_offset_x(&self, region_id: &str) -> f32 {
        self.scroll_offsets
            .get(region_id)
            .map_or(0.0, |offset| offset.x)
    }

    #[must_use]
    pub fn snapshot(&self) -> InputControllerSnapshot {
        InputControllerSnapshot {
            choice_scroll_offset_y: self.choice_scroll.offset_y,
            scroll_offsets: self
                .scroll_offsets
                .iter()
                .filter(|(_, offset)| !offset.is_zero())
                .map(|(region_id, offset)| InputScrollOffsetSnapshot {
                    region_id: region_id.clone(),
                    offset_x: offset.x,
                    offset_y: offset.y,
                })
                .collect(),
        }
    }

    pub fn restore_snapshot(
        &mut self,
        snapshot: InputControllerSnapshot,
    ) -> Result<(), InputControllerSnapshotError> {
        let choice_scroll_offset_y = snapshot.choice_scroll_offset_y;
        if !choice_scroll_offset_y.is_finite() {
            return Err(InputControllerSnapshotError::NonFiniteChoiceScroll {
                offset_y: choice_scroll_offset_y,
            });
        }
        if choice_scroll_offset_y < 0.0 {
            return Err(InputControllerSnapshotError::NegativeChoiceScroll {
                offset_y: choice_scroll_offset_y,
            });
        }
        let scroll_offsets = snapshot.scroll_offsets.into_iter().try_fold(
            BTreeMap::new(),
            |mut offsets, entry| {
                if entry.region_id.is_empty() {
                    return Err(InputControllerSnapshotError::EmptyScrollRegionId);
                }
                if !entry.offset_x.is_finite() || !entry.offset_y.is_finite() {
                    return Err(InputControllerSnapshotError::NonFiniteScrollOffset {
                        region_id: entry.region_id,
                        offset_x: entry.offset_x,
                        offset_y: entry.offset_y,
                    });
                }
                if entry.offset_x < 0.0 || entry.offset_y < 0.0 {
                    return Err(InputControllerSnapshotError::NegativeScrollOffset {
                        region_id: entry.region_id,
                        offset_x: entry.offset_x,
                        offset_y: entry.offset_y,
                    });
                }
                let offset = ScrollOffset::new(entry.offset_x, entry.offset_y);
                if !offset.is_zero() {
                    offsets.insert(entry.region_id, offset);
                }
                Ok(offsets)
            },
        )?;
        self.choice_scroll.offset_y = choice_scroll_offset_y;
        self.scroll_offsets = scroll_offsets;
        Ok(())
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

    pub(crate) fn retain_live_text_control_focus(&mut self, controls: &[RenderTextInputControl]) {
        let stale = self.focused_text_editor.as_ref().is_some_and(|editor| {
            !controls
                .iter()
                .any(|control| text_control_matches_editor(control, editor))
        });
        if stale {
            self.deactivate_focused_text_editor();
        }
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

    pub fn ensure_choice_focus(&mut self, frame: &PreparedFrame) -> bool {
        if self.interaction.focus().target().is_none()
            && let Some(target) = frame.first_keyboard_focus_target()
        {
            self.set_focus(frame, target);
            true
        } else {
            false
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
        if let Some(selection) = self.text_drag_selection(frame, pointer, position, true) {
            let _ = self.apply_or_defer_text_pointer_selection(frame, selection);
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
                if frame_target_is_text_input(frame, &target) {
                    self.pending_text_pointer_selection = Some(TextPointerSelectionState {
                        pointer,
                        target: target.clone(),
                        position,
                        selecting: false,
                    });
                } else {
                    self.pending_text_pointer_selection = None;
                }
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
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| frame_target_is_text_input(frame, &drag.target))
        {
            let _ = self.apply_or_defer_text_pointer_selection(
                frame,
                TextPointerSelectionState {
                    pointer,
                    target: drag.target.clone(),
                    position,
                    selecting: !is_activation,
                },
            );
        }
        let (actions, text_control_write_backs, diagnostics, action_button_activation) =
            match (released, routed.decision(), is_activation) {
                (Some(pressed), RouteDecision::Routed(event), true)
                    if &pressed == event.target() =>
                {
                    let mut actions = choice_action(frame, event.target())
                        .into_iter()
                        .collect::<Vec<_>>();
                    let submit = Self::action_button_submit(frame, event.target());
                    actions.extend(submit.action);
                    let text_control_write_backs = submit.write_back.into_iter().collect();
                    let diagnostics = submit.diagnostic.into_iter().collect();
                    (
                        actions,
                        text_control_write_backs,
                        diagnostics,
                        frame_target_is_action_button(frame, event.target()),
                    )
                }
                _ => (Vec::new(), Vec::new(), Vec::new(), false),
            };
        let text_input_activation = routed
            .event()
            .is_some_and(|event| frame_target_is_text_input(frame, event.target()));
        activation_outcome(
            frame,
            actions,
            text_control_write_backs,
            diagnostics,
            is_activation && !text_input_activation && !action_button_activation,
        )
    }

    pub fn pointer_cancel(&mut self, pointer: PointerId) -> InputOutcome {
        self.pointer_positions.remove(&pointer.0);
        self.pressed.remove(&pointer.0);
        self.drags.remove(&pointer.0);
        if self
            .pending_text_pointer_selection
            .as_ref()
            .is_some_and(|selection| selection.pointer == pointer)
        {
            self.pending_text_pointer_selection = None;
        }
        self.interaction.clear_pointer(pointer);
        InputOutcome::redraw(true)
    }

    pub fn apply_pending_text_pointer_selection(
        &mut self,
        frame: &PreparedFrame,
    ) -> Result<bool, TextEditorError> {
        let Some(selection) = self.pending_text_pointer_selection.take() else {
            return Ok(false);
        };
        self.apply_or_defer_text_pointer_selection(frame, selection)
    }

    pub fn keyboard(&mut self, frame: &PreparedFrame, key: &str, phase: KeyPhase) -> InputOutcome {
        if phase == KeyPhase::Up {
            return InputOutcome::default();
        }
        match key {
            "ArrowUp" => self.move_focus(frame, FocusNavigationDirection::Up),
            "ArrowDown" => self.move_focus(frame, FocusNavigationDirection::Down),
            "ArrowLeft" => self.move_focus(frame, FocusNavigationDirection::Left),
            "ArrowRight" => self.move_focus(frame, FocusNavigationDirection::Right),
            "Tab" => self.move_focus(frame, FocusNavigationDirection::Next),
            "Home" => {
                if let Some(target) = frame.first_keyboard_focus_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "End" => {
                if let Some(target) = frame.last_keyboard_focus_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "Enter" | " " | "Space" => self.activate_focused(frame),
            _ => InputOutcome::default(),
        }
    }

    pub fn keyboard_with_modifiers(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        shift: bool,
    ) -> InputOutcome {
        if key == "Tab" && phase == KeyPhase::Down && shift {
            return self.move_focus(frame, FocusNavigationDirection::Previous);
        }
        self.keyboard(frame, key, phase)
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
                diagnostics: Vec::new(),
                dialogue_advance: false,
                cancel: false,
                redraw: self.ime_composing,
            };
        }
        self.keyboard(frame, key, phase)
    }

    pub fn keyboard_with_modifiers_and_ime(
        &mut self,
        frame: &PreparedFrame,
        key: &str,
        phase: KeyPhase,
        shift: bool,
        disposition: TextInputKeyDisposition,
    ) -> InputOutcome {
        if disposition.shortcuts_suppressed() || self.ime_composing {
            return InputOutcome {
                actions: Vec::new(),
                text_control_write_backs: Vec::new(),
                diagnostics: Vec::new(),
                dialogue_advance: false,
                cancel: false,
                redraw: self.ime_composing,
            };
        }
        self.keyboard_with_modifiers(frame, key, phase, shift)
    }

    pub fn controller(
        &mut self,
        frame: &PreparedFrame,
        change: ControllerInputChange,
    ) -> InputOutcome {
        self.controller.normalize(change).into_iter().fold(
            InputOutcome::default(),
            |mut outcome, action| {
                outcome.merge(self.normalized_controller_action(frame, action));
                outcome
            },
        )
    }

    fn normalized_controller_action(
        &mut self,
        frame: &PreparedFrame,
        action: NormalizedControllerAction,
    ) -> InputOutcome {
        match action {
            NormalizedControllerAction::Move(direction) => self.move_focus(frame, direction),
            NormalizedControllerAction::Confirm => self.activate_focused(frame),
            NormalizedControllerAction::Cancel => InputOutcome::cancel(),
        }
    }

    fn move_focus(
        &mut self,
        frame: &PreparedFrame,
        direction: FocusNavigationDirection,
    ) -> InputOutcome {
        let current = self.interaction.focus().target().or_else(|| {
            self.focused_text_editor
                .as_ref()
                .map(TextEditorState::target)
        });
        if let Some(next) = frame.focus_target(current, direction) {
            self.set_focus(frame, next);
        }
        InputOutcome::redraw(true)
    }

    fn activate_focused(&mut self, frame: &PreparedFrame) -> InputOutcome {
        let focused = self.interaction.focus().target().cloned();
        let actions = focused
            .as_ref()
            .and_then(|target| choice_action(frame, target))
            .into_iter()
            .collect::<Vec<_>>();
        let submit = focused
            .as_ref()
            .map_or_else(ActionButtonSubmitOutcome::default, |target| {
                Self::action_button_submit(frame, target)
            });
        let mut actions = actions;
        actions.extend(submit.action);
        let text_control_write_backs = submit.write_back.into_iter().collect();
        let diagnostics = submit.diagnostic.into_iter().collect();
        let activates_target = focused.as_ref().is_some_and(|target| {
            frame.choice_for_target(target).is_some()
                || frame.action_button_for_target(target).is_some()
        });
        activation_outcome(
            frame,
            actions,
            text_control_write_backs,
            diagnostics,
            !activates_target,
        )
    }

    pub fn text_input(
        &mut self,
        frame: &PreparedFrame,
        input: TextInput,
    ) -> Result<InputOutcome, TextEditorError> {
        let mut text_control_write_backs = Vec::new();
        let stale = self.focused_text_editor.as_ref().is_some_and(|editor| {
            editor.session() == input.session() && !focused_editor_matches_frame(frame, editor)
        });
        if stale {
            self.deactivate_focused_text_editor();
        }
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
            diagnostics: Vec::new(),
            dialogue_advance: false,
            cancel: false,
            redraw: true,
        })
    }

    pub fn wheel(&mut self, frame: &PreparedFrame, delta_y: f32) -> InputOutcome {
        if let Some(position) = self.primary_pointer_position()
            && let Some(region) = frame
                .scroll_regions
                .iter()
                .rev()
                .find(|region| region.contains(position))
        {
            let current = self
                .scroll_offsets
                .get(&region.id)
                .copied()
                .unwrap_or_else(|| ScrollOffset::new(region.offset_x, region.offset_y));
            let next = match region.axis {
                arcweft_render_wgpu::geometry::RenderScrollAxis::Vertical => {
                    ScrollOffset::new(0.0, region.clamped_offset_y(current.y - delta_y))
                }
                arcweft_render_wgpu::geometry::RenderScrollAxis::Horizontal => {
                    ScrollOffset::new(region.clamped_offset_x(current.x - delta_y), 0.0)
                }
            };
            if next.is_zero() {
                self.scroll_offsets.remove(&region.id);
            } else {
                self.scroll_offsets.insert(region.id.clone(), next);
            }
        }
        InputOutcome::redraw(true)
    }

    fn primary_pointer_position(&self) -> Option<ViewportPoint> {
        self.pointer_positions.values().next().copied()
    }

    pub fn focus_changed(&mut self, focused: bool) -> InputOutcome {
        self.window_focused = focused;
        let mut text_control_write_backs = Vec::new();
        if !focused {
            if self.ime_composing
                && let Some(editor) = self.focused_text_editor.as_mut()
            {
                let had_composition = editor.composition_range().is_some();
                if had_composition
                    && editor
                        .apply_operation(
                            &TextInputOperation::EndComposition {
                                reason: CompositionEndReason::Committed,
                            },
                            &mut self.text_editor_clipboard,
                        )
                        .is_ok()
                {
                    let privacy = if editor.options().is_secure() {
                        TextInputPrivacy::Sensitive
                    } else {
                        TextInputPrivacy::Plain
                    };
                    text_control_write_backs.push(TextControlWriteBack::change(
                        editor.target().clone(),
                        editor.session(),
                        TextControlValue::new(editor.text(), privacy),
                        editor.selection(),
                        editor.revision(),
                    ));
                }
            }
            self.interaction.clear_focus();
            self.pointer_positions.clear();
            self.pressed.clear();
            self.drags.clear();
            self.pending_text_pointer_selection = None;
            self.focused_text_editor = None;
            self.interaction.clear_pointer_state();
            self.ime_composing = false;
        }
        InputOutcome {
            actions: Vec::new(),
            text_control_write_backs,
            diagnostics: Vec::new(),
            dialogue_advance: false,
            cancel: false,
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

    fn text_drag_selection(
        &self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        selecting: bool,
    ) -> Option<TextPointerSelectionState> {
        let drag = self.drags.get(&pointer.0)?;
        frame_target_is_text_input(frame, &drag.target).then(|| TextPointerSelectionState {
            pointer,
            target: drag.target.clone(),
            position,
            selecting,
        })
    }

    fn apply_or_defer_text_pointer_selection(
        &mut self,
        frame: &PreparedFrame,
        selection: TextPointerSelectionState,
    ) -> Result<bool, TextEditorError> {
        let Some(focused) = frame.focused_text_input_target() else {
            self.pending_text_pointer_selection = Some(selection);
            return Ok(false);
        };
        let Some(editor) = self.focused_text_editor.as_mut() else {
            self.pending_text_pointer_selection = Some(selection);
            return Ok(false);
        };
        if focused.snapshot.target() != &selection.target || editor.target() != &selection.target {
            self.pending_text_pointer_selection = Some(selection);
            return Ok(false);
        }
        let before_selection = editor.selection();
        let before_caret = editor.caret();
        let caret = viewport_text_hit_offset(
            focused.geometry.viewport_character_bounds(),
            selection.position,
        );
        editor.set_caret_to_text_offset(caret, selection.selecting)?;
        Ok(editor.selection() != before_selection || editor.caret() != before_caret)
    }

    fn action_button_submit(
        frame: &PreparedFrame,
        target: &arcweft_presentation::input::InteractionTarget,
    ) -> ActionButtonSubmitOutcome {
        let Some(button) = frame.action_button_for_target(target) else {
            return ActionButtonSubmitOutcome::default();
        };
        if !button.enabled {
            return ActionButtonSubmitOutcome::default();
        }
        let RenderActionButtonAction::ActionInvoke { action, payload } = &button.action else {
            return ActionButtonSubmitOutcome::default();
        };
        let action =
            frame
                .semantics
                .lower_action(target, action)
                .ok()
                .map(|action| match payload {
                    Some(payload) => action.with_payload(payload.clone()),
                    None => action,
                });
        ActionButtonSubmitOutcome {
            action,
            write_back: None,
            diagnostic: None,
        }
    }

    fn deactivate_focused_text_editor(&mut self) {
        if let Some(editor) = &self.focused_text_editor
            && self.interaction.focus().target() == Some(editor.target())
        {
            self.interaction.clear_focus();
        }
        self.focused_text_editor = None;
        self.pending_text_pointer_selection = None;
        self.ime_composing = false;
    }
}

fn text_control_matches_editor(control: &RenderTextInputControl, editor: &TextEditorState) -> bool {
    control.session == editor.session() && control.target == *editor.target()
}

fn focused_editor_matches_frame(frame: &PreparedFrame, editor: &TextEditorState) -> bool {
    frame.focused_text_input_target().is_some_and(|focused| {
        focused.snapshot.session() == editor.session()
            && focused.snapshot.target() == editor.target()
    })
}

fn viewport_text_hit_offset(
    bounds: &[TextCharacterBounds],
    position: ViewportPoint,
) -> TextByteOffset {
    let same_line = bounds
        .iter()
        .copied()
        .filter(|bounds| {
            position.y >= bounds.bounds.y && position.y <= bounds.bounds.y + bounds.bounds.height
        })
        .collect::<Vec<_>>();
    let candidates = if same_line.is_empty() {
        bounds
    } else {
        same_line.as_slice()
    };
    if candidates.is_empty() {
        return TextByteOffset(0);
    }
    for bounds in candidates {
        let midpoint = bounds.bounds.x + bounds.bounds.width * 0.5;
        if position.x <= midpoint {
            return *bounds.range.start();
        }
    }
    candidates
        .last()
        .map_or(TextByteOffset(0), |bounds| *bounds.range.end())
}

fn activation_outcome(
    frame: &PreparedFrame,
    actions: Vec<Action>,
    text_control_write_backs: Vec<TextControlWriteBack>,
    diagnostics: Vec<InputDiagnostic>,
    advances_dialogue: bool,
) -> InputOutcome {
    let dialogue_advance = advances_dialogue
        && actions.is_empty()
        && text_control_write_backs.is_empty()
        && frame.has_dialogue()
        && frame.choices.is_empty();
    InputOutcome {
        actions,
        text_control_write_backs,
        diagnostics,
        dialogue_advance,
        cancel: false,
        redraw: true,
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)]
fn is_zero_f32(value: &f32) -> bool {
    value.abs() <= f32::EPSILON
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

fn frame_target_is_text_input(
    frame: &PreparedFrame,
    target: &arcweft_presentation::input::InteractionTarget,
) -> bool {
    frame
        .semantics
        .find(target)
        .is_some_and(|node| node.role().is_text_input_control())
}

fn frame_target_is_action_button(
    frame: &PreparedFrame,
    target: &arcweft_presentation::input::InteractionTarget,
) -> bool {
    frame.action_button_for_target(target).is_some()
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
        RenderDialogue, RenderPreferences, RenderScene, RenderScrollAxis, RenderScrollOverflow,
        RenderScrollRegion, RenderViewport, SharedFramePlanner,
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
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
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
            scroll_regions: Vec::new(),
        }
    }

    fn scene_with_dialogue(control: RenderTextInputControl) -> RenderScene {
        RenderScene {
            dialogue: Some(RenderDialogue {
                speaker: "narrator".to_owned(),
                text: "click dialogue to advance".to_owned(),
                base_styles: Vec::new(),
                text_runs: Vec::new(),
            }),
            ..scene(control)
        }
    }

    fn scroll_frame() -> PreparedFrame {
        SharedFramePlanner::prepare(&RenderScene {
            dialogue: None,
            choices: Vec::new(),
            text_inputs: Vec::new(),
            action_buttons: Vec::new(),
            focus_groups: Vec::new(),
            focus_navigation: Vec::new(),
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
            scroll_regions: vec![RenderScrollRegion {
                id: "scroll.editor".to_owned(),
                bounds: HitRect::new(20.0, 30.0, 220.0, 80.0),
                content_width: 220.0,
                content_height: 260.0,
                offset_x: 0.0,
                offset_y: 0.0,
                axis: RenderScrollAxis::Vertical,
                overflow: RenderScrollOverflow::Auto,
            }],
        })
        .expect("scroll frame prepares")
    }

    #[test]
    fn wheel_updates_scroll_region_under_pointer_and_clamps() {
        let frame = scroll_frame();
        let mut input = InputController::default();

        input.pointer_move(&frame, PointerId(0), ViewportPoint::new(30.0, 40.0));
        input.wheel(&frame, -90.0);
        assert!((input.scroll_offset_y("scroll.editor") - 90.0).abs() < f32::EPSILON);

        input.wheel(&frame, -300.0);
        assert!((input.scroll_offset_y("scroll.editor") - 180.0).abs() < f32::EPSILON);

        input.wheel(&frame, 300.0);
        assert!(input.scroll_offset_y("scroll.editor").abs() < f32::EPSILON);
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
    fn pointer_activation_on_text_input_does_not_advance_dialogue() {
        let target = target("text_input.pointer");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(51),
            "abc",
            TextRange::new(TextByteOffset(3), TextByteOffset(3)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control)).unwrap();
        let mut input = InputController::default();
        let position = ViewportPoint::new(30.0, 40.0);

        let down = input.pointer_down(&frame, PointerId(0), position);
        let up = input.pointer_up(&frame, PointerId(0), position);

        assert!(!down.dialogue_advance);
        assert!(!up.dialogue_advance);
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
    fn focus_loss_commits_active_ime_composition() {
        let target = target("text_input.ime_focus_loss");
        let session = TextInputSessionId(55);
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
        let preedit = "ちょう";

        let outcome = input
            .text_input(
                &frame,
                TextInput::single(
                    session,
                    TextInputSerial(12),
                    TextInputOperation::SetComposition(TextCompositionUpdate::new(
                        preedit,
                        TextRange::new(
                            TextByteOffset(0),
                            TextByteOffset(u32::try_from(preedit.len()).unwrap()),
                        ),
                    )),
                ),
            )
            .unwrap();
        assert!(outcome.redraw);
        assert!(input.ime_composing());
        assert!(input.focused_text_editor().is_some());

        let outcome = input.focus_changed(false);

        assert!(outcome.redraw);
        assert!(!input.ime_composing());
        assert!(input.focused_text_editor().is_none());
        assert_eq!(outcome.text_control_write_backs().len(), 1);
        assert_eq!(
            outcome.text_control_write_backs()[0].value().as_str(),
            preedit
        );
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
