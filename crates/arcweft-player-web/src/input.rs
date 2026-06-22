use arcweft_id::PublicId;
use arcweft_presentation::hover::HoverPath;
use arcweft_presentation::input::{
    Action, InputEpoch, KeyPhase, KeyboardInput, PointerId, PointerInput, PointerPhase,
    RawInputEvent, RawInputKind, ViewportPoint,
};
use arcweft_presentation::interaction::{FocusState, InteractionState, PointerCapture};
use arcweft_presentation::router::{InputRouter, RouteDecision};
use arcweft_render_wgpu::geometry::{ChoiceScroll, InteractionVisualState, PreparedFrame};
use std::collections::BTreeMap;

/// Pointer drag state owned by Arcweft presentation input, not the DOM drag API.
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

/// Result of one normalized input operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InputOutcome {
    pub actions: Vec<Action>,
    pub redraw: bool,
}

/// Platform-independent input state used by the winit adapter.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InputController {
    next_epoch: u64,
    interaction: InteractionState,
    hover: Option<HoverPath>,
    pointer_positions: BTreeMap<u64, ViewportPoint>,
    pressed: BTreeMap<u64, arcweft_presentation::input::InteractionTarget>,
    drags: BTreeMap<u64, DragState>,
    choice_scroll: ChoiceScroll,
    window_focused: bool,
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

    pub fn pointer_position(&self, pointer: PointerId) -> Option<ViewportPoint> {
        self.pointer_positions.get(&pointer.0).copied()
    }

    pub fn visual_state(&self) -> InteractionVisualState {
        InteractionVisualState {
            focused: self.interaction.focus().target().cloned(),
            hovered: self
                .hover
                .as_ref()
                .and_then(|path| path.targets().last().cloned()),
            pressed: self.pressed.values().next().cloned(),
        }
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
        self.hover = InputRouter::hover_path(pointer, position, &frame.layers, &frame.hits);
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
        self.interaction.release_pointer(pointer);
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
        self.interaction.release_pointer(pointer);
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
            _ => {
                let raw = self.raw(RawInputKind::Keyboard(KeyboardInput {
                    key: key.to_owned(),
                    phase,
                }));
                let _ = InputRouter::route(&raw, &frame.layers, &frame.hits, &self.interaction);
                InputOutcome::default()
            }
        }
    }

    pub fn wheel(&mut self, delta_y: f32) -> InputOutcome {
        self.choice_scroll.offset_y = (self.choice_scroll.offset_y + delta_y).max(0.0);
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
            let pointers = self
                .interaction
                .captures()
                .iter()
                .map(|capture| capture.pointer())
                .collect::<Vec<_>>();
            pointers.into_iter().for_each(|pointer| {
                self.interaction.release_pointer(pointer);
            });
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
