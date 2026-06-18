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

/// Frame-crossing interaction state owned by presentation/runtime-host.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct InteractionState {
    focus: FocusState,
    captures: Vec<PointerCapture>,
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
        self.release_pointer(capture.pointer());
        self.captures.push(capture);
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
}
