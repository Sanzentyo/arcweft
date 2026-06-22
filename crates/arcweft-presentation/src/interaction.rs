use crate::hover::{HoverPath, HoverTransition};
use crate::input::{InteractionTarget, PointerId};
use crate::layer::LayerId;

/// Stable keyboard/text focus state used by routing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FocusState {
    layer: Option<LayerId>,
    target: Option<InteractionTarget>,
}

/// Active pointer capture owned by one routed target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PointerCapture {
    pointer: PointerId,
    layer: LayerId,
    target: InteractionTarget,
}

/// Pointer-down visual ownership retained until release or cancellation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PressedTarget {
    pointer: PointerId,
    layer: LayerId,
    target: InteractionTarget,
}

/// Frame-crossing interaction state owned by presentation/runtime-host.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InteractionState {
    focus: FocusState,
    captures: Vec<PointerCapture>,
    hover_paths: Vec<HoverPath>,
    pressed_targets: Vec<PressedTarget>,
}

impl FocusState {
    pub const fn new(layer: LayerId, target: InteractionTarget) -> Self {
        Self {
            layer: Some(layer),
            target: Some(target),
        }
    }

    pub const fn layer(&self) -> Option<&LayerId> {
        self.layer.as_ref()
    }

    pub const fn target(&self) -> Option<&InteractionTarget> {
        self.target.as_ref()
    }
}

impl PointerCapture {
    pub const fn new(pointer: PointerId, layer: LayerId, target: InteractionTarget) -> Self {
        Self {
            pointer,
            layer,
            target,
        }
    }

    pub const fn pointer(&self) -> PointerId {
        self.pointer
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }
}

impl PressedTarget {
    pub const fn new(pointer: PointerId, layer: LayerId, target: InteractionTarget) -> Self {
        Self {
            pointer,
            layer,
            target,
        }
    }

    pub const fn pointer(&self) -> PointerId {
        self.pointer
    }

    pub const fn layer(&self) -> &LayerId {
        &self.layer
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }
}

impl InteractionState {
    pub const fn focus(&self) -> &FocusState {
        &self.focus
    }

    pub fn set_focus(&mut self, focus: FocusState) {
        self.focus = focus;
    }

    pub fn clear_focus(&mut self) {
        self.focus = FocusState::default();
    }

    pub fn captures(&self) -> &[PointerCapture] {
        &self.captures
    }

    pub fn capture_pointer(&mut self, capture: PointerCapture) {
        let _ = self.release_pointer(capture.pointer());
        self.captures.push(capture);
        self.captures.sort_by_key(PointerCapture::pointer);
    }

    pub fn release_pointer(&mut self, pointer: PointerId) -> Option<PointerCapture> {
        self.captures
            .iter()
            .position(|capture| capture.pointer() == pointer)
            .map(|index| self.captures.remove(index))
    }

    pub fn capture_for(&self, pointer: PointerId) -> Option<&PointerCapture> {
        self.captures()
            .iter()
            .find(|capture| capture.pointer() == pointer)
    }

    pub fn hover_paths(&self) -> &[HoverPath] {
        &self.hover_paths
    }

    pub fn hover_path(&self, pointer: PointerId) -> Option<&HoverPath> {
        self.hover_paths
            .iter()
            .find(|path| path.pointer() == pointer)
    }

    pub fn set_hover_path(&mut self, path: HoverPath) -> Option<HoverTransition> {
        let pointer = path.pointer();
        let previous = self
            .hover_paths
            .iter()
            .position(|candidate| candidate.pointer() == pointer)
            .map(|index| self.hover_paths.remove(index));
        let transition = HoverTransition::diff(previous.as_ref(), Some(&path));
        self.hover_paths.push(path);
        self.hover_paths.sort_by_key(HoverPath::pointer);
        transition.filter(|transition| !transition.is_empty())
    }

    pub fn clear_hover(&mut self, pointer: PointerId) -> Option<HoverTransition> {
        let previous = self
            .hover_paths
            .iter()
            .position(|candidate| candidate.pointer() == pointer)
            .map(|index| self.hover_paths.remove(index));
        HoverTransition::diff(previous.as_ref(), None).filter(|transition| !transition.is_empty())
    }

    pub fn hovered_target(&self, pointer: PointerId) -> Option<&InteractionTarget> {
        self.hover_path(pointer).and_then(HoverPath::leaf)
    }

    pub fn primary_hovered_target(&self) -> Option<&InteractionTarget> {
        self.hover_paths
            .iter()
            .min_by_key(|path| path.pointer().0)
            .and_then(HoverPath::leaf)
    }

    pub fn pressed_targets(&self) -> &[PressedTarget] {
        &self.pressed_targets
    }

    pub fn press_pointer(&mut self, pressed: PressedTarget) {
        let _ = self.release_pressed(pressed.pointer());
        self.pressed_targets.push(pressed);
        self.pressed_targets.sort_by_key(PressedTarget::pointer);
    }

    pub fn release_pressed(&mut self, pointer: PointerId) -> Option<PressedTarget> {
        self.pressed_targets
            .iter()
            .position(|pressed| pressed.pointer() == pointer)
            .map(|index| self.pressed_targets.remove(index))
    }

    pub fn pressed_for(&self, pointer: PointerId) -> Option<&PressedTarget> {
        self.pressed_targets
            .iter()
            .find(|pressed| pressed.pointer() == pointer)
    }

    pub fn primary_pressed_target(&self) -> Option<&InteractionTarget> {
        self.pressed_targets
            .iter()
            .min_by_key(|pressed| pressed.pointer().0)
            .map(PressedTarget::target)
    }

    pub fn is_hovered(&self, target: &InteractionTarget) -> bool {
        self.hover_paths.iter().any(|path| path.contains(target))
    }

    pub fn is_focused(&self, target: &InteractionTarget) -> bool {
        self.focus.target().is_some_and(|focused| focused == target)
    }

    pub fn is_pressed(&self, target: &InteractionTarget) -> bool {
        self.pressed_targets
            .iter()
            .any(|pressed| pressed.target() == target)
    }

    pub fn clear_pointer(&mut self, pointer: PointerId) {
        let _ = self.release_pointer(pointer);
        let _ = self.release_pressed(pointer);
        let _ = self.clear_hover(pointer);
    }

    pub fn clear_pointer_state(&mut self) {
        self.captures.clear();
        self.hover_paths.clear();
        self.pressed_targets.clear();
    }
}
