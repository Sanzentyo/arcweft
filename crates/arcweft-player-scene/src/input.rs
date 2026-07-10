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
    RenderScrollAxis, RenderScrollOverscrollPolicy, RenderScrollRegion, RenderTextInputControl,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod keyboard;
mod pointer;
mod scroll;
mod state;
#[cfg(test)]
mod tests;
mod text_edit;

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
    pub dialogue_progress: DialogueProgress,
    pub cancel: bool,
    pub redraw: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum DialogueProgress {
    #[default]
    None,
    Reveal,
    Advance,
}

impl DialogueProgress {
    pub const fn reveals(self) -> bool {
        matches!(self, Self::Reveal)
    }

    pub const fn advances(self) -> bool {
        matches!(self, Self::Advance)
    }

    pub const fn redraws(self) -> bool {
        !matches!(self, Self::None)
    }

    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Reveal, _) | (_, Self::Reveal) => Self::Reveal,
            (Self::Advance, _) | (_, Self::Advance) => Self::Advance,
            (Self::None, Self::None) => Self::None,
        }
    }
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

/// Shared state for one retained scroll region.
///
/// Only `offset` is persisted. Elastic displacement, velocity, and indicator
/// activity are presentation transients reconstructed from new interactions.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct ScrollState {
    offset: ScrollOffset,
    overscroll: ScrollOffset,
    velocity: ScrollOffset,
    spring_time_millis: Option<u64>,
    activity_millis: Option<u64>,
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
            dialogue_progress: DialogueProgress::None,
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
        self.dialogue_progress = self.dialogue_progress.merge(other.dialogue_progress);
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
    scroll_states: BTreeMap<String, ScrollState>,
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
    pub fn focus_changed(&mut self, focused: bool) -> InputOutcome {
        self.window_focused = focused;
        let mut text_control_write_backs = Vec::new();
        if !focused {
            self.controller.reset_transient_state();
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
            dialogue_progress: DialogueProgress::None,
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
        let Some(region) = frame.scroll_region_for_target(target).cloned() else {
            return false;
        };
        self.mark_scroll_activity(&region.id, frame.visual_time_millis);
        let Some(bounds) = frame.target_bounds(target) else {
            return false;
        };
        if region.auto_scroll_focus == RenderFocusAutoScrollPolicy::Disabled {
            return false;
        }
        let current = self.scroll_states.get(&region.id).map_or_else(
            || ScrollOffset::new(region.offset_x, region.offset_y),
            |state| state.offset,
        );
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
        self.store_scroll_offset(&region.id, next, frame.visual_time_millis)
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
            .cloned()
        else {
            return false;
        };
        let current = self.scroll_states.get(&region.id).map_or_else(
            || ScrollOffset::new(region.offset_x, region.offset_y),
            |state| state.offset,
        );
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
        next != current && self.store_scroll_offset(&region.id, next, frame.visual_time_millis)
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
    let dialogue_progress = dialogue_progress_for_frame(
        frame,
        advances_dialogue
            && frame.has_dialogue()
            && frame.dialogue_advance_available()
            && frame.choices.is_empty(),
    );
    InputOutcome {
        actions,
        text_control_write_backs,
        clipboard_requests: Vec::new(),
        diagnostics,
        dialogue_progress,
        cancel: false,
        redraw: true,
    }
}

fn dialogue_progress_for_frame(frame: &PreparedFrame, requested: bool) -> DialogueProgress {
    if !requested {
        return DialogueProgress::None;
    }
    if frame.has_revealing_dialogue() {
        DialogueProgress::Reveal
    } else {
        DialogueProgress::Advance
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
