use crate::controller::{
    ControllerInputChange, ControllerInputNormalizer, NormalizedControllerAction,
};
use arcweft_id::PublicId;
use arcweft_presentation::clipboard::{
    TextClipboardOutcome, TextClipboardRequest, TextClipboardRequestId,
};
use arcweft_presentation::input::{
    Action, InputEpoch, KeyPhase, PointerId, PointerInput, PointerPhase, RawInputEvent,
    RawInputKind, ViewportPoint,
};
use arcweft_presentation::interaction::{
    FocusState, InteractionState, PointerCapture, PressedTarget,
};
use arcweft_presentation::router::{InputRouter, RouteDecision};
use arcweft_presentation::semantic::SemanticActionError;
use arcweft_presentation::text_editor::{
    TextEditorError, TextEditorLayout, TextEditorLocalClipboard, TextEditorOutput, TextEditorState,
};
use arcweft_presentation::text_index::TextIndexSnapshot;
use arcweft_presentation::text_input::{
    CompositionEndReason, TextByteOffset, TextCharacterBounds, TextControlValue,
    TextControlWriteBack, TextInput, TextInputKeyDisposition, TextInputOperation, TextInputPrivacy,
    TextRange,
};
use arcweft_render_wgpu::geometry::{
    ChoiceScroll, FocusNavigationDirection, FramePlanError, InteractionVisualState, PreparedFrame,
    PreparedSelectableTextBlock, RenderActionButtonAction, RenderFocusAutoScrollPolicy,
    RenderScrollAxis, RenderScrollRegion, RenderTextInputControl,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

const POINTER_ACTIVATION_DISTANCE_SQUARED: f32 = 64.0;

#[derive(Clone, Debug, PartialEq)]
pub struct DragState {
    pub pointer: PointerId,
    pub target: arcweft_presentation::input::InteractionTarget,
    pub start: ViewportPoint,
    pub current: ViewportPoint,
    pub modifiers: InputPointerModifiers,
    advances_dialogue: bool,
    intent: DragIntent,
}

#[derive(Clone, Debug, PartialEq)]
struct BlankPointerPressState {
    start: ViewportPoint,
    current: ViewportPoint,
    advances_dialogue: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum DragIntent {
    #[default]
    SelectOrActivate,
    MoveSelectedText,
    SelectTextBlock,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputPointerModifiers {
    shift: bool,
}

#[derive(Clone, Debug, PartialEq)]
struct TextPointerSelectionState {
    pointer: PointerId,
    target: arcweft_presentation::input::InteractionTarget,
    position: ViewportPoint,
    selecting: bool,
    kind: TextPointerSelectionKind,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum TextPointerSelectionKind {
    #[default]
    Caret,
    Word,
    Line,
}

#[derive(Clone, Debug, PartialEq)]
struct LastPointerActivation {
    target: arcweft_presentation::input::InteractionTarget,
    position: ViewportPoint,
    click_count: u8,
    epoch: u64,
}

#[derive(Clone, Debug, PartialEq)]
struct TextBlockSelectionState {
    target: arcweft_presentation::input::InteractionTarget,
    text: String,
    anchor: TextByteOffset,
    selection: TextRange<TextByteOffset>,
}

const MULTI_CLICK_DISTANCE_SQUARED: f32 = 64.0;
const MULTI_CLICK_EPOCH_WINDOW: u64 = 8;
const TEXT_DRAG_AUTOSCROLL_EDGE: f32 = 24.0;
const TEXT_DRAG_AUTOSCROLL_STEP: f32 = 32.0;
const KEYBOARD_PAGE_SCROLL_FRACTION: f32 = 0.9;
const SCROLL_DELTA_EPSILON: f32 = 0.001;

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

impl BlankPointerPressState {
    fn distance_squared(&self) -> f32 {
        let dx = self.current.x - self.start.x;
        let dy = self.current.y - self.start.y;
        dx.mul_add(dx, dy * dy)
    }

    fn activation_outcome(self, frame: &PreparedFrame) -> InputOutcome {
        activation_outcome(
            frame,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            self.advances_dialogue
                && self.distance_squared() <= POINTER_ACTIVATION_DISTANCE_SQUARED,
        )
    }
}

impl InputPointerModifiers {
    pub const NONE: Self = Self { shift: false };

    pub const fn new(shift: bool) -> Self {
        Self { shift }
    }

    pub const fn shift(self) -> bool {
        self.shift
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputOutcome {
    pub actions: Vec<Action>,
    pub text_control_write_backs: Vec<TextControlWriteBack>,
    pub clipboard_requests: Vec<TextClipboardRequest>,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputDiagnosticKind {
    SemanticActionRejected {
        action: PublicId,
        reason: SemanticActionError,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct ActionButtonSubmitOutcome {
    action: Option<Action>,
    write_back: Option<TextControlWriteBack>,
    diagnostic: Option<InputDiagnostic>,
}

#[derive(Debug, Default)]
struct PointerActivationEffects {
    actions: Vec<Action>,
    text_control_write_backs: Vec<TextControlWriteBack>,
    diagnostics: Vec<InputDiagnostic>,
    action_button_activation: bool,
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

    pub fn clipboard_requests(&self) -> &[TextClipboardRequest] {
        &self.clipboard_requests
    }

    pub fn into_text_control_write_backs(self) -> Vec<TextControlWriteBack> {
        self.text_control_write_backs
    }

    fn redraw(redraw: bool) -> Self {
        Self {
            actions: Vec::new(),
            text_control_write_backs: Vec::new(),
            clipboard_requests: Vec::new(),
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
        self.clipboard_requests.extend(other.clipboard_requests);
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
    blank_presses: BTreeMap<u64, BlankPointerPressState>,
    pending_text_pointer_selection: Option<TextPointerSelectionState>,
    text_block_selection: Option<TextBlockSelectionState>,
    choice_scroll: ChoiceScroll,
    scroll_offsets: BTreeMap<String, ScrollOffset>,
    controller: ControllerInputNormalizer,
    window_focused: bool,
    ime_composing: bool,
    focused_text_editor: Option<TextEditorState>,
    text_editor_clipboard: TextEditorLocalClipboard,
    next_clipboard_request_id: TextClipboardRequestId,
    pending_clipboard_requests: BTreeMap<TextClipboardRequestId, TextClipboardRequest>,
    last_pointer_activation: Option<LastPointerActivation>,
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

    pub(crate) fn text_block_selection_for(
        &self,
        target: &arcweft_presentation::input::InteractionTarget,
        text: &str,
    ) -> Option<TextRange<TextByteOffset>> {
        self.text_block_selection
            .as_ref()
            .filter(|selection| selection.target == *target && selection.text == text)
            .map(|selection| selection.selection)
            .filter(|selection| selection.start() != selection.end())
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
            && self.focused_text_editor.is_none()
            && let Some(target) = frame.choices.first().map(|choice| choice.target.clone())
        {
            self.set_focus(frame, target);
            self.interaction.focus().target().is_some()
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
        if let Some(press) = self.blank_presses.get_mut(&pointer.0) {
            press.current = position;
        }
        if self.update_text_block_drag_selection(frame, pointer, position) {
            return InputOutcome::redraw(true);
        }
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
        let _ = self.auto_scroll_text_drag(frame, pointer, position);
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
        modifiers: InputPointerModifiers,
    ) -> InputOutcome {
        self.pointer_positions.insert(pointer.0, position);
        self.blank_presses.remove(&pointer.0);
        if let Some(block) = frame.selectable_text_block_at(position).cloned() {
            self.pending_text_pointer_selection = None;
            self.deactivate_focused_text_editor();
            let _ = self.apply_text_block_pointer_selection(
                &block,
                position,
                modifiers.shift(),
                TextPointerSelectionKind::Caret,
            );
            self.drags.insert(
                pointer.0,
                DragState {
                    pointer,
                    target: block.target.clone(),
                    start: position,
                    current: position,
                    modifiers,
                    advances_dialogue: false,
                    intent: DragIntent::SelectTextBlock,
                },
            );
            return InputOutcome::redraw(true);
        }
        let raw = self.raw(RawInputKind::Pointer(PointerInput {
            pointer,
            position,
            phase: PointerPhase::Down,
        }));
        let routed = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
        let had_view_control_focus = self.focused_target_is_view_control(frame);
        let advances_dialogue =
            !had_view_control_focus && self.dialogue_can_advance_from_unfocused_input(frame);
        let mut focused_routed_target = false;
        if let RouteDecision::Routed(event) = routed.decision() {
            let target = event.target().clone();
            if let Some(node) = frame.semantics.find(&target) {
                focused_routed_target = true;
                let intent = if frame_target_is_text_input(frame, &target) {
                    let intent = self.text_drag_intent(frame, &target, position, modifiers);
                    self.pending_text_pointer_selection = (intent == DragIntent::SelectOrActivate)
                        .then(|| TextPointerSelectionState {
                            pointer,
                            target: target.clone(),
                            position,
                            selecting: modifiers.shift(),
                            kind: TextPointerSelectionKind::Caret,
                        });
                    intent
                } else {
                    self.pending_text_pointer_selection = None;
                    self.deactivate_focused_text_editor();
                    DragIntent::SelectOrActivate
                };
                self.set_focus(frame, target.clone());
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
                        modifiers,
                        advances_dialogue,
                        intent,
                    },
                );
            }
        }
        if !focused_routed_target {
            self.deactivate_focused_text_editor();
            self.interaction.clear_focus();
            self.blank_presses.insert(
                pointer.0,
                BlankPointerPressState {
                    start: position,
                    current: position,
                    advances_dialogue,
                },
            );
        }
        InputOutcome::redraw(true)
    }

    pub fn pointer_up(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
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
        let blank_press = self.blank_presses.remove(&pointer.0);
        let is_activation = drag
            .as_ref()
            .is_some_and(|drag| drag.distance_squared() <= POINTER_ACTIVATION_DISTANCE_SQUARED);
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| drag.intent == DragIntent::SelectTextBlock)
        {
            if let Some(block) = frame
                .selectable_text_block_for_target(&drag.target)
                .cloned()
            {
                let kind = if is_activation {
                    self.next_text_click_kind(&drag.target, position)
                } else {
                    TextPointerSelectionKind::Caret
                };
                let _ = self.apply_text_block_pointer_selection(
                    &block,
                    position,
                    drag.modifiers.shift() || modifiers.shift() || !is_activation,
                    kind,
                );
            }
            return InputOutcome::redraw(true);
        }
        if let Some(blank_press) = blank_press {
            return blank_press.activation_outcome(frame);
        }
        let mut pointer_text_write_backs = Vec::new();
        if let Some(drag) = drag
            .as_ref()
            .filter(|drag| frame_target_is_text_input(frame, &drag.target))
        {
            if drag.intent == DragIntent::MoveSelectedText && !is_activation {
                if let Some(write_back) =
                    self.move_selected_text_to_pointer(frame, &drag.target, position)
                {
                    pointer_text_write_backs.push(write_back);
                }
            } else {
                let kind = if is_activation {
                    self.next_text_click_kind(&drag.target, position)
                } else {
                    TextPointerSelectionKind::Caret
                };
                let _ = self.apply_or_defer_text_pointer_selection(
                    frame,
                    TextPointerSelectionState {
                        pointer,
                        target: drag.target.clone(),
                        position,
                        selecting: drag.modifiers.shift() || modifiers.shift() || !is_activation,
                        kind,
                    },
                );
            }
        }
        let mut effects =
            pointer_activation_effects(frame, released.as_ref(), routed.decision(), is_activation);
        effects
            .text_control_write_backs
            .extend(pointer_text_write_backs);
        let text_input_activation = routed
            .event()
            .is_some_and(|event| frame_target_is_text_input(frame, event.target()));
        activation_outcome(
            frame,
            effects.actions,
            effects.text_control_write_backs,
            effects.diagnostics,
            is_activation
                && drag.as_ref().is_some_and(|drag| drag.advances_dialogue)
                && !text_input_activation
                && !effects.action_button_activation,
        )
    }

    pub fn pointer_context_menu(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
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
            if frame_target_is_text_input(frame, &target) {
                self.set_focus(frame, target.clone());
                let _ = self.apply_or_defer_text_pointer_selection(
                    frame,
                    TextPointerSelectionState {
                        pointer,
                        target,
                        position,
                        selecting: modifiers.shift(),
                        kind: TextPointerSelectionKind::Caret,
                    },
                );
            }
        }
        InputOutcome::redraw(true)
    }

    pub fn pointer_cancel(&mut self, pointer: PointerId) -> InputOutcome {
        self.pointer_positions.remove(&pointer.0);
        self.pressed.remove(&pointer.0);
        self.drags.remove(&pointer.0);
        self.blank_presses.remove(&pointer.0);
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

    fn update_text_block_drag_selection(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> bool {
        let Some(drag) = self
            .drags
            .get_mut(&pointer.0)
            .filter(|drag| drag.intent == DragIntent::SelectTextBlock)
        else {
            return false;
        };
        drag.current = position;
        frame
            .selectable_text_block_for_target(&drag.target)
            .cloned()
            .is_some_and(|block| {
                self.apply_text_block_pointer_selection(
                    &block,
                    position,
                    true,
                    TextPointerSelectionKind::Caret,
                )
            })
    }

    fn apply_text_block_pointer_selection(
        &mut self,
        block: &PreparedSelectableTextBlock,
        position: ViewportPoint,
        selecting: bool,
        kind: TextPointerSelectionKind,
    ) -> bool {
        let offset = viewport_text_hit_offset(&block.character_bounds, position);
        let existing_anchor = self
            .text_block_selection
            .as_ref()
            .filter(|selection| selection.target == block.target && selection.text == block.text)
            .map(|selection| selection.anchor);
        let selection = match kind {
            TextPointerSelectionKind::Word if !selecting => {
                word_range_at_text_offset(&block.text, offset)
            }
            TextPointerSelectionKind::Line if !selecting => {
                line_range_at_text_offset(&block.text, offset)
            }
            TextPointerSelectionKind::Caret
            | TextPointerSelectionKind::Word
            | TextPointerSelectionKind::Line => {
                let anchor = if selecting {
                    existing_anchor.unwrap_or(offset)
                } else {
                    offset
                };
                ordered_text_range(anchor, offset)
            }
        };
        let anchor = if selecting {
            existing_anchor.unwrap_or(*selection.start())
        } else {
            *selection.start()
        };
        let next = TextBlockSelectionState {
            target: block.target.clone(),
            text: block.text.clone(),
            anchor,
            selection,
        };
        let changed = self.text_block_selection.as_ref() != Some(&next);
        self.text_block_selection = Some(next);
        changed
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
            "PageDown" => self.scroll_focus_or_pointer_page(frame, 1.0),
            "PageUp" => self.scroll_focus_or_pointer_page(frame, -1.0),
            "Home" => {
                if self.scroll_focus_or_pointer_to_edge(frame, false).redraw {
                    return InputOutcome::redraw(true);
                }
                if let Some(target) = frame.first_keyboard_focus_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "End" => {
                if self.scroll_focus_or_pointer_to_edge(frame, true).redraw {
                    return InputOutcome::redraw(true);
                }
                if let Some(target) = frame.last_keyboard_focus_target() {
                    self.set_focus(frame, target);
                }
                InputOutcome::redraw(true)
            }
            "Enter" | " " | "Space" => self.activate_focused(frame),
            "Backspace" => self.dialogue_advance_from_keyboard(frame),
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
                clipboard_requests: Vec::new(),
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
                clipboard_requests: Vec::new(),
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
        let text_control_write_backs: Vec<TextControlWriteBack> =
            submit.write_back.into_iter().collect();
        let diagnostics = submit.diagnostic.into_iter().collect();
        let activates_choice = focused
            .as_ref()
            .is_some_and(|target| frame.choice_for_target(target).is_some());
        let focused_view_control = focused
            .as_ref()
            .is_some_and(|target| frame_target_is_view_control(frame, target));
        activation_outcome(
            frame,
            actions,
            text_control_write_backs,
            diagnostics,
            !activates_choice && !focused_view_control,
        )
    }

    fn dialogue_advance_from_keyboard(&self, frame: &PreparedFrame) -> InputOutcome {
        let dialogue_advance = self.dialogue_can_advance_from_unfocused_input(frame);
        InputOutcome {
            dialogue_advance,
            redraw: dialogue_advance,
            ..InputOutcome::default()
        }
    }

    fn dialogue_can_advance_from_unfocused_input(&self, frame: &PreparedFrame) -> bool {
        frame.has_dialogue()
            && frame.choices.is_empty()
            && !self.focused_target_is_view_control(frame)
    }

    fn focused_target_is_view_control(&self, frame: &PreparedFrame) -> bool {
        self.interaction
            .focus()
            .target()
            .or_else(|| {
                self.focused_text_editor
                    .as_ref()
                    .map(TextEditorState::target)
            })
            .is_some_and(|target| frame_target_is_view_control(frame, target))
    }

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
        Ok(InputOutcome {
            actions: Vec::new(),
            text_control_write_backs,
            clipboard_requests,
            diagnostics: Vec::new(),
            dialogue_advance: submitted_runtime_text_control
                && self.dialogue_can_advance_from_unfocused_input(frame),
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
            text_control_write_backs,
            clipboard_requests: Vec::new(),
            diagnostics: Vec::new(),
            dialogue_advance: false,
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

    pub fn wheel(&mut self, frame: &PreparedFrame, delta_y: f32) -> InputOutcome {
        self.precision_scroll(frame, 0.0, delta_y)
    }

    pub fn precision_scroll(
        &mut self,
        frame: &PreparedFrame,
        delta_x: f32,
        delta_y: f32,
    ) -> InputOutcome {
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
            return InputOutcome::redraw(false);
        };
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
    }

    pub fn scroll_region_by_id(
        &mut self,
        frame: &PreparedFrame,
        region_id: &str,
        delta_x: f32,
        delta_y: f32,
    ) -> InputOutcome {
        let Some(region) = frame
            .scroll_regions
            .iter()
            .find(|region| region.id == region_id)
        else {
            return InputOutcome::redraw(false);
        };
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
    }

    fn scroll_focus_or_pointer_page(&mut self, frame: &PreparedFrame, sign: f32) -> InputOutcome {
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
            return InputOutcome::default();
        };
        let (delta_x, delta_y) = match region.axis {
            RenderScrollAxis::Vertical => (
                0.0,
                -sign * region.bounds.height * KEYBOARD_PAGE_SCROLL_FRACTION,
            ),
            RenderScrollAxis::Horizontal => (
                -sign * region.bounds.width * KEYBOARD_PAGE_SCROLL_FRACTION,
                0.0,
            ),
        };
        InputOutcome::redraw(self.scroll_region(region, delta_x, delta_y))
    }

    fn scroll_focus_or_pointer_to_edge(
        &mut self,
        frame: &PreparedFrame,
        end: bool,
    ) -> InputOutcome {
        let Some(region) = self.scroll_region_for_pointer_or_focus(frame) else {
            return InputOutcome::default();
        };
        let next = if end {
            ScrollOffset::new(region.max_offset_x(), region.max_offset_y())
        } else {
            ScrollOffset::new(0.0, 0.0)
        };
        InputOutcome::redraw(self.store_scroll_offset(&region.id, next))
    }

    fn scroll_region_for_pointer_or_focus<'a>(
        &self,
        frame: &'a PreparedFrame,
    ) -> Option<&'a RenderScrollRegion> {
        self.primary_pointer_position()
            .and_then(|position| {
                frame
                    .scroll_regions
                    .iter()
                    .rev()
                    .find(|region| region.contains(position))
            })
            .or_else(|| {
                self.interaction
                    .focus()
                    .target()
                    .and_then(|target| frame.scroll_region_for_target(target))
            })
    }

    fn scroll_region(&mut self, region: &RenderScrollRegion, delta_x: f32, delta_y: f32) -> bool {
        let current = self
            .scroll_offsets
            .get(&region.id)
            .copied()
            .unwrap_or_else(|| ScrollOffset::new(region.offset_x, region.offset_y));
        let next = match region.axis {
            RenderScrollAxis::Vertical => ScrollOffset::new(
                0.0,
                region.clamped_offset_y(current.y - finite_delta(delta_y)),
            ),
            RenderScrollAxis::Horizontal => {
                let primary = if delta_x.abs() > SCROLL_DELTA_EPSILON {
                    delta_x
                } else {
                    delta_y
                };
                ScrollOffset::new(
                    region.clamped_offset_x(current.x - finite_delta(primary)),
                    0.0,
                )
            }
        };
        self.store_scroll_offset(&region.id, next)
    }

    fn store_scroll_offset(&mut self, region_id: &str, next: ScrollOffset) -> bool {
        let before = self
            .scroll_offsets
            .get(region_id)
            .copied()
            .unwrap_or_default();
        if next.is_zero() {
            self.scroll_offsets.remove(region_id);
        } else {
            self.scroll_offsets.insert(region_id.to_owned(), next);
        }
        before != next
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
            clipboard_requests: Vec::new(),
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
        let auto_scroll_target = target.clone();
        if !frame_target_is_text_input(frame, &target) {
            self.deactivate_focused_text_editor();
        }
        if let Some(node) = frame.semantics.find(&target) {
            self.interaction
                .set_focus(FocusState::new(node.layer().clone(), target));
            self.ensure_focused_target_visible(frame, &auto_scroll_target);
        }
    }

    fn ensure_focused_target_visible(
        &mut self,
        frame: &PreparedFrame,
        target: &arcweft_presentation::input::InteractionTarget,
    ) -> bool {
        let Some(region) = frame.scroll_region_for_target(target) else {
            return false;
        };
        let Some(bounds) = frame.target_bounds(target) else {
            return false;
        };
        if region.auto_scroll_focus == RenderFocusAutoScrollPolicy::Disabled {
            return false;
        }
        let current = self
            .scroll_offsets
            .get(&region.id)
            .copied()
            .unwrap_or_else(|| ScrollOffset::new(region.offset_x, region.offset_y));
        let next = match region.axis {
            RenderScrollAxis::Vertical => ScrollOffset::new(
                current.x,
                focus_auto_scroll_offset(
                    region.auto_scroll_focus,
                    current.y,
                    region.bounds.y,
                    region.bounds.height,
                    bounds.y,
                    bounds.height,
                    region.max_offset_y(),
                ),
            ),
            RenderScrollAxis::Horizontal => ScrollOffset::new(
                focus_auto_scroll_offset(
                    region.auto_scroll_focus,
                    current.x,
                    region.bounds.x,
                    region.bounds.width,
                    bounds.x,
                    bounds.width,
                    region.max_offset_x(),
                ),
                current.y,
            ),
        };
        self.store_scroll_offset(&region.id, next)
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
        (drag.intent == DragIntent::SelectOrActivate
            && frame_target_is_text_input(frame, &drag.target))
        .then(|| TextPointerSelectionState {
            pointer,
            target: drag.target.clone(),
            position,
            selecting,
            kind: TextPointerSelectionKind::Caret,
        })
    }

    fn text_drag_intent(
        &self,
        frame: &PreparedFrame,
        target: &arcweft_presentation::input::InteractionTarget,
        position: ViewportPoint,
        modifiers: InputPointerModifiers,
    ) -> DragIntent {
        if modifiers.shift() {
            return DragIntent::SelectOrActivate;
        }
        let Some(focused) = frame.focused_text_input_target() else {
            return DragIntent::SelectOrActivate;
        };
        let Some(editor) = self.focused_text_editor.as_ref() else {
            return DragIntent::SelectOrActivate;
        };
        if focused.snapshot.target() != target || editor.target() != target {
            return DragIntent::SelectOrActivate;
        }
        let selection = editor.selection();
        if selection.start() == selection.end() {
            return DragIntent::SelectOrActivate;
        }
        let offset =
            viewport_text_hit_offset(focused.geometry.viewport_character_bounds(), position);
        if offset.0 > selection.start().0 && offset.0 < selection.end().0 {
            DragIntent::MoveSelectedText
        } else {
            DragIntent::SelectOrActivate
        }
    }

    fn auto_scroll_text_drag(
        &mut self,
        frame: &PreparedFrame,
        pointer: PointerId,
        position: ViewportPoint,
    ) -> bool {
        let Some(drag) = self.drags.get(&pointer.0) else {
            return false;
        };
        if drag.intent != DragIntent::SelectOrActivate
            || !frame_target_is_text_input(frame, &drag.target)
        {
            return false;
        }
        let Some(focused) = frame.focused_text_input_target() else {
            return false;
        };
        if focused.snapshot.target() != &drag.target {
            return false;
        }
        let control = focused.geometry.viewport_control_rect();
        let control_center = ViewportPoint::new(
            control.x + control.width * 0.5,
            control.y + control.height * 0.5,
        );
        let Some(region) = frame
            .scroll_regions
            .iter()
            .find(|region| region.contains(control_center))
        else {
            return false;
        };
        let current = self
            .scroll_offsets
            .get(&region.id)
            .copied()
            .unwrap_or_else(|| ScrollOffset::new(region.offset_x, region.offset_y));
        let next = match region.axis {
            arcweft_render_wgpu::geometry::RenderScrollAxis::Vertical => {
                if position.y < region.bounds.y + TEXT_DRAG_AUTOSCROLL_EDGE {
                    ScrollOffset::new(
                        current.x,
                        region.clamped_offset_y(current.y - TEXT_DRAG_AUTOSCROLL_STEP),
                    )
                } else if position.y
                    > region.bounds.y + region.bounds.height - TEXT_DRAG_AUTOSCROLL_EDGE
                {
                    ScrollOffset::new(
                        current.x,
                        region.clamped_offset_y(current.y + TEXT_DRAG_AUTOSCROLL_STEP),
                    )
                } else {
                    current
                }
            }
            arcweft_render_wgpu::geometry::RenderScrollAxis::Horizontal => {
                if position.x < region.bounds.x + TEXT_DRAG_AUTOSCROLL_EDGE {
                    ScrollOffset::new(
                        region.clamped_offset_x(current.x - TEXT_DRAG_AUTOSCROLL_STEP),
                        current.y,
                    )
                } else if position.x
                    > region.bounds.x + region.bounds.width - TEXT_DRAG_AUTOSCROLL_EDGE
                {
                    ScrollOffset::new(
                        region.clamped_offset_x(current.x + TEXT_DRAG_AUTOSCROLL_STEP),
                        current.y,
                    )
                } else {
                    current
                }
            }
        };
        if next == current {
            return false;
        }
        if next.is_zero() {
            self.scroll_offsets.remove(&region.id);
        } else {
            self.scroll_offsets.insert(region.id.clone(), next);
        }
        true
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
        match selection.kind {
            TextPointerSelectionKind::Caret => {
                editor.set_caret_to_text_offset(caret, selection.selecting)?;
            }
            TextPointerSelectionKind::Word => {
                editor.select_word_at_text_offset(caret, selection.selecting)?;
            }
            TextPointerSelectionKind::Line => {
                editor.select_line_at_text_offset(caret, selection.selecting)?;
            }
        }
        Ok(editor.selection() != before_selection || editor.caret() != before_caret)
    }

    fn next_text_click_kind(
        &mut self,
        target: &arcweft_presentation::input::InteractionTarget,
        position: ViewportPoint,
    ) -> TextPointerSelectionKind {
        let epoch = self.next_epoch;
        let click_count = self
            .last_pointer_activation
            .as_ref()
            .filter(|last| {
                &last.target == target
                    && epoch.saturating_sub(last.epoch) <= MULTI_CLICK_EPOCH_WINDOW
                    && point_distance_squared(last.position, position)
                        <= MULTI_CLICK_DISTANCE_SQUARED
            })
            .map_or(1, |last| last.click_count.saturating_add(1).min(3));
        self.last_pointer_activation = Some(LastPointerActivation {
            target: target.clone(),
            position,
            click_count,
            epoch,
        });
        match click_count {
            2 => TextPointerSelectionKind::Word,
            3.. => TextPointerSelectionKind::Line,
            _ => TextPointerSelectionKind::Caret,
        }
    }

    fn move_selected_text_to_pointer(
        &mut self,
        frame: &PreparedFrame,
        target: &arcweft_presentation::input::InteractionTarget,
        position: ViewportPoint,
    ) -> Option<TextControlWriteBack> {
        let focused = frame.focused_text_input_target()?;
        let editor = self.focused_text_editor.as_mut()?;
        if focused.snapshot.target() != target || editor.target() != target {
            return None;
        }
        let offset =
            viewport_text_hit_offset(focused.geometry.viewport_character_bounds(), position);
        editor
            .move_selection_to_text_offset(offset)
            .ok()
            .and_then(|changed| changed.then(|| text_control_change_writeback(editor)))
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
        let action = match frame.semantics.lower_action(target, action) {
            Ok(action) => Some(match payload {
                Some(payload) => action.with_payload(payload.clone()),
                None => action,
            }),
            Err(reason) => {
                return ActionButtonSubmitOutcome {
                    action: None,
                    write_back: None,
                    diagnostic: Some(InputDiagnostic {
                        kind: InputDiagnosticKind::SemanticActionRejected {
                            action: action.clone(),
                            reason,
                        },
                        target: target.clone(),
                    }),
                };
            }
        };
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

fn ordered_text_range(left: TextByteOffset, right: TextByteOffset) -> TextRange<TextByteOffset> {
    if left.0 <= right.0 {
        TextRange::new(left, right)
    } else {
        TextRange::new(right, left)
    }
}

fn word_range_at_text_offset(text: &str, offset: TextByteOffset) -> TextRange<TextByteOffset> {
    TextIndexSnapshot::try_new(text.to_owned())
        .and_then(|index| index.word_range_at(offset))
        .unwrap_or_else(|_| TextRange::new(offset, offset))
}

fn line_range_at_text_offset(text: &str, offset: TextByteOffset) -> TextRange<TextByteOffset> {
    let offset = TextIndexSnapshot::try_new(text.to_owned())
        .and_then(|index| index.validate_byte_offset(offset))
        .unwrap_or(TextByteOffset(0));
    let byte = usize::try_from(offset.0)
        .unwrap_or(usize::MAX)
        .min(text.len());
    let start = text[..byte].rfind('\n').map_or(0, |index| index + 1);
    let end = text[byte..]
        .find('\n')
        .map_or(text.len(), |index| byte + index);
    TextRange::new(
        TextByteOffset(u32::try_from(start).unwrap_or(u32::MAX)),
        TextByteOffset(u32::try_from(end).unwrap_or(u32::MAX)),
    )
}

fn point_distance_squared(left: ViewportPoint, right: ViewportPoint) -> f32 {
    let dx = left.x - right.x;
    let dy = left.y - right.y;
    dx.mul_add(dx, dy * dy)
}

fn text_control_change_writeback(editor: &TextEditorState) -> TextControlWriteBack {
    let privacy = if editor.options().is_secure() {
        TextInputPrivacy::Sensitive
    } else {
        TextInputPrivacy::Plain
    };
    TextControlWriteBack::change(
        editor.target().clone(),
        editor.session(),
        TextControlValue::new(editor.text(), privacy),
        editor.selection(),
        editor.revision(),
    )
}

fn activation_outcome(
    frame: &PreparedFrame,
    actions: Vec<Action>,
    text_control_write_backs: Vec<TextControlWriteBack>,
    diagnostics: Vec<InputDiagnostic>,
    advances_dialogue: bool,
) -> InputOutcome {
    let dialogue_advance = advances_dialogue && frame.has_dialogue() && frame.choices.is_empty();
    InputOutcome {
        actions,
        text_control_write_backs,
        clipboard_requests: Vec::new(),
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

fn finite_delta(value: f32) -> f32 {
    if value.is_finite() { value } else { 0.0 }
}

fn focus_auto_scroll_offset(
    policy: RenderFocusAutoScrollPolicy,
    current: f32,
    viewport_start: f32,
    viewport_extent: f32,
    target_start: f32,
    target_extent: f32,
    max_offset: f32,
) -> f32 {
    match policy {
        RenderFocusAutoScrollPolicy::Nearest => nearest_offset_for_axis(
            current,
            viewport_start,
            viewport_extent,
            target_start,
            target_extent,
            max_offset,
        ),
        RenderFocusAutoScrollPolicy::Start => {
            (current + target_start - viewport_start).clamp(0.0, max_offset)
        }
        RenderFocusAutoScrollPolicy::End => {
            (current + target_start + target_extent - viewport_start - viewport_extent)
                .clamp(0.0, max_offset)
        }
        RenderFocusAutoScrollPolicy::Disabled => current.clamp(0.0, max_offset),
    }
}

fn nearest_offset_for_axis(
    current: f32,
    viewport_start: f32,
    viewport_extent: f32,
    target_start: f32,
    target_extent: f32,
    max_offset: f32,
) -> f32 {
    let viewport_end = viewport_start + viewport_extent;
    let target_end = target_start + target_extent;
    let desired = if target_start < viewport_start {
        current + target_start - viewport_start
    } else if target_end > viewport_end {
        current + target_end - viewport_end
    } else {
        current
    };
    desired.clamp(0.0, max_offset)
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

fn pointer_activation_effects(
    frame: &PreparedFrame,
    released: Option<&arcweft_presentation::input::InteractionTarget>,
    decision: &RouteDecision,
    is_activation: bool,
) -> PointerActivationEffects {
    let (Some(pressed), RouteDecision::Routed(event), true) = (released, decision, is_activation)
    else {
        return PointerActivationEffects::default();
    };
    if pressed != event.target() {
        return PointerActivationEffects::default();
    }
    let mut actions = choice_action(frame, event.target())
        .into_iter()
        .collect::<Vec<_>>();
    let submit = InputController::action_button_submit(frame, event.target());
    actions.extend(submit.action);
    PointerActivationEffects {
        actions,
        text_control_write_backs: submit.write_back.into_iter().collect(),
        diagnostics: submit.diagnostic.into_iter().collect(),
        action_button_activation: frame_target_is_action_button(frame, event.target()),
    }
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

fn frame_target_is_view_control(
    frame: &PreparedFrame,
    target: &arcweft_presentation::input::InteractionTarget,
) -> bool {
    frame_target_is_text_input(frame, target) || frame_target_is_action_button(frame, target)
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
        RenderActionButton, RenderControlStyle, RenderDialogue, RenderPreferences, RenderScene,
        RenderScrollAxis, RenderScrollOverflow, RenderScrollRegion, RenderViewport,
        SharedFramePlanner,
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
                auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
            }],
        })
        .expect("scroll frame prepares")
    }

    fn horizontal_scroll_frame() -> PreparedFrame {
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
                id: "scroll.gallery".to_owned(),
                bounds: HitRect::new(20.0, 30.0, 100.0, 80.0),
                content_width: 260.0,
                content_height: 80.0,
                offset_x: 0.0,
                offset_y: 0.0,
                axis: RenderScrollAxis::Horizontal,
                overflow: RenderScrollOverflow::Auto,
                auto_scroll_focus: RenderFocusAutoScrollPolicy::Nearest,
            }],
        })
        .expect("horizontal scroll frame prepares")
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
    fn precision_scroll_uses_x_delta_for_horizontal_region() {
        let frame = horizontal_scroll_frame();
        let mut input = InputController::default();

        input.pointer_move(&frame, PointerId(0), ViewportPoint::new(30.0, 40.0));
        let outcome = input.precision_scroll(&frame, -40.0, -5.0);

        assert!(outcome.redraw);
        assert!((input.scroll_offset_x("scroll.gallery") - 40.0).abs() < f32::EPSILON);
        assert!(input.scroll_offset_y("scroll.gallery").abs() < f32::EPSILON);
    }

    #[test]
    fn scroll_region_by_id_scrolls_without_pointer_and_clamps() {
        let frame = horizontal_scroll_frame();
        let mut input = InputController::default();

        let outcome = input.scroll_region_by_id(&frame, "scroll.gallery", -400.0, 0.0);

        assert!(outcome.redraw);
        assert!((input.scroll_offset_x("scroll.gallery") - 160.0).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_scroll_region_by_id_is_noop() {
        let frame = horizontal_scroll_frame();
        let mut input = InputController::default();

        let outcome = input.scroll_region_by_id(&frame, "scroll.missing", -400.0, 0.0);

        assert!(!outcome.redraw);
        assert!(input.snapshot().scroll_offsets.is_empty());
    }

    #[test]
    fn focus_auto_scroll_policy_offsets_are_clamped() {
        assert!(
            (focus_auto_scroll_offset(
                RenderFocusAutoScrollPolicy::Nearest,
                0.0,
                30.0,
                100.0,
                80.0,
                80.0,
                160.0,
            ) - 30.0)
                .abs()
                < f32::EPSILON
        );
        assert!(
            (focus_auto_scroll_offset(
                RenderFocusAutoScrollPolicy::Start,
                0.0,
                30.0,
                100.0,
                80.0,
                80.0,
                160.0,
            ) - 50.0)
                .abs()
                < f32::EPSILON
        );
        assert!(
            (focus_auto_scroll_offset(
                RenderFocusAutoScrollPolicy::End,
                0.0,
                30.0,
                100.0,
                80.0,
                80.0,
                160.0,
            ) - 30.0)
                .abs()
                < f32::EPSILON
        );
        assert!(
            (focus_auto_scroll_offset(
                RenderFocusAutoScrollPolicy::Disabled,
                24.0,
                30.0,
                100.0,
                80.0,
                80.0,
                160.0,
            ) - 24.0)
                .abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn ensure_choice_focus_does_not_autofocus_view_text_controls() {
        let target = target("text_input.no_auto_focus");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(43),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextArea,
            HitRect::new(20.0, 30.0, 220.0, 80.0),
        );
        let frame = SharedFramePlanner::prepare(&scene(control)).unwrap();
        let mut input = InputController::default();

        assert!(!input.ensure_choice_focus(&frame));
        assert!(input.visual_state().focused.is_none());
        assert!(input.focused_text_editor().is_none());
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
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control.clone())).unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();
        let position = ViewportPoint::new(30.0, 40.0);

        let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

        assert!(!down.dialogue_advance);
        assert!(!up.dialogue_advance);
    }

    #[test]
    fn pointer_activation_on_action_button_clears_text_editor_focus() {
        let text_target = target("text_input.button_defocus");
        let button_target = target("button.button_defocus");
        let control = RenderTextInputControl::new(
            text_target,
            TextInputSessionId(69),
            "draft",
            TextRange::new(TextByteOffset(5), TextByteOffset(5)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let scene = RenderScene {
            action_buttons: vec![RenderActionButton {
                target: button_target.clone(),
                label: "Send".to_owned(),
                enabled: true,
                containing_scroll_region: None,
                bounds: HitRect::new(300.0, 30.0, 120.0, 32.0),
                viewport_clip: None,
                style: RenderControlStyle::default(),
                action: RenderActionButtonAction::Noop,
            }],
            ..scene_with_dialogue(control.clone())
        };
        let frame = SharedFramePlanner::prepare(&scene).unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();
        assert!(input.focused_text_editor().is_some());

        let position = ViewportPoint::new(320.0, 44.0);
        let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

        assert!(!down.dialogue_advance);
        assert!(!up.dialogue_advance);
        assert!(input.focused_text_editor().is_none());
        assert_eq!(input.interaction().focus().target(), Some(&button_target));
    }

    #[test]
    fn pointer_activation_on_blank_area_advances_dialogue_without_view_control_focus() {
        let target = target("text_input.blank_advance");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(68),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control)).unwrap();
        let mut input = InputController::default();
        let position = ViewportPoint::new(500.0, 80.0);

        let down = input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        let up = input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

        assert!(!down.dialogue_advance);
        assert!(up.dialogue_advance);
    }

    #[test]
    fn enter_without_view_control_focus_advances_dialogue() {
        let target = target("text_input.unfocused_enter");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(64),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control.clone())).unwrap();
        let mut input = InputController::default();

        let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

        assert!(outcome.dialogue_advance);
    }

    #[test]
    fn enter_with_text_input_focus_does_not_advance_dialogue() {
        let target = target("text_input.focused_enter");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(65),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control.clone())).unwrap();
        let mut input = InputController::default();
        let position = ViewportPoint::new(30.0, 40.0);
        input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);

        let outcome = input.keyboard(&frame, "Enter", KeyPhase::Down);

        assert!(!outcome.dialogue_advance);
    }

    #[test]
    fn backspace_advances_dialogue_only_without_view_control_focus() {
        let target = target("text_input.backspace_focus");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(66),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control)).unwrap();
        let mut input = InputController::default();

        let unfocused = input.keyboard(&frame, "Backspace", KeyPhase::Down);
        assert!(unfocused.dialogue_advance);

        let position = ViewportPoint::new(30.0, 40.0);
        input.pointer_down(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        input.pointer_up(&frame, PointerId(0), position, InputPointerModifiers::NONE);
        let focused = input.keyboard(&frame, "Backspace", KeyPhase::Down);

        assert!(!focused.dialogue_advance);
    }

    #[test]
    fn pointer_down_outside_view_control_clears_text_focus_without_advancing() {
        let target = target("text_input.blank_defocus");
        let control = RenderTextInputControl::new(
            target,
            TextInputSessionId(67),
            "",
            TextRange::new(TextByteOffset(0), TextByteOffset(0)),
            TextInputOptions::default(),
            SemanticRole::TextField,
            HitRect::new(20.0, 30.0, 220.0, 32.0),
        );
        let frame = SharedFramePlanner::prepare(&scene_with_dialogue(control.clone())).unwrap();
        let mut input = InputController::default();

        let text_position = ViewportPoint::new(30.0, 40.0);
        input.pointer_down(
            &frame,
            PointerId(0),
            text_position,
            InputPointerModifiers::NONE,
        );
        input.pointer_up(
            &frame,
            PointerId(0),
            text_position,
            InputPointerModifiers::NONE,
        );
        assert!(input.interaction().focus().target().is_some());
        input.activate_text_control(&control).unwrap();
        assert!(input.focused_text_editor().is_some());

        let blank_position = ViewportPoint::new(500.0, 500.0);
        let blank = input.pointer_down(
            &frame,
            PointerId(0),
            blank_position,
            InputPointerModifiers::NONE,
        );
        let blank_up = input.pointer_up(
            &frame,
            PointerId(0),
            blank_position,
            InputPointerModifiers::NONE,
        );
        assert!(!blank.dialogue_advance);
        assert!(!blank_up.dialogue_advance);
        assert!(input.interaction().focus().target().is_none());
        assert!(input.focused_text_editor().is_none());

        let second_down = input.pointer_down(
            &frame,
            PointerId(0),
            blank_position,
            InputPointerModifiers::NONE,
        );
        let second_up = input.pointer_up(
            &frame,
            PointerId(0),
            blank_position,
            InputPointerModifiers::NONE,
        );
        assert!(!second_down.dialogue_advance);
        assert!(second_up.dialogue_advance);

        let advance = input.keyboard(&frame, "Backspace", KeyPhase::Down);
        assert!(advance.dialogue_advance);
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
    fn submit_command_with_text_focus_does_not_advance_active_dialogue() {
        let target = target("text_input.dialogue_submit");
        let session = TextInputSessionId(63);
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
            ..scene_with_dialogue(control.clone())
        })
        .unwrap();
        let mut input = InputController::default();
        input.activate_text_control(&control).unwrap();

        let outcome = input
            .text_input(
                &frame,
                TextInput::single(
                    session,
                    TextInputSerial(19),
                    TextInputOperation::Command(TextEditCommand::Submit),
                ),
            )
            .unwrap();

        assert!(!outcome.dialogue_advance);
        assert_eq!(outcome.text_control_write_backs().len(), 1);
        assert!(outcome.text_control_write_backs()[0].is_submit());
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
