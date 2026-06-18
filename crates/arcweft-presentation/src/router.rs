use crate::hit::{HitRecord, HitTree};
use crate::input::{
    InputEvent, InputEventKind, KeyboardInput, RawInputEvent, RawInputKind, TextInput,
};
use crate::interaction::InteractionState;
use crate::layer::{LayerId, LayerInputPolicy, LayerTree, LayerVisibility};

/// Stateless Sans I/O router over `LayerTree`, `HitTree`, and `InteractionState`.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct InputRouter;

/// Routing result for one raw event.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoutedInput {
    raw_epoch: crate::input::InputEpoch,
    decision: RouteDecision,
}

/// Auditable routing decision for replay and Agent diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteDecision {
    Routed(InputEvent),
    BlockedByModal { modal: LayerId },
    NoTarget,
    TargetUnavailable,
    LayerUnavailable { layer: LayerId },
    Ignored,
}

impl InputRouter {
    pub fn route(
        raw: &RawInputEvent,
        layers: &LayerTree,
        hits: &HitTree,
        state: &InteractionState,
    ) -> RoutedInput {
        let decision = match raw.kind() {
            RawInputKind::Pointer(pointer) => {
                if let Some(capture) = state.capture_for(pointer.pointer) {
                    route_to_known_layer(
                        raw,
                        layers,
                        capture.layer(),
                        capture.target(),
                        InputEventKind::Pointer {
                            phase: pointer.phase,
                        },
                    )
                } else {
                    route_pointer(raw, layers, hits)
                }
            }
            RawInputKind::Keyboard(keyboard) => route_keyboard(raw, layers, hits, state, keyboard),
            RawInputKind::Text(text) => route_text(raw, layers, hits, state, text),
            RawInputKind::Agent(agent) => {
                let Some(target) = &agent.target else {
                    return RoutedInput::new(raw.epoch(), RouteDecision::NoTarget);
                };
                let Some(record) = hits.find_target(target) else {
                    return RoutedInput::new(raw.epoch(), RouteDecision::TargetUnavailable);
                };
                route_hit_record(
                    raw,
                    layers,
                    record,
                    InputEventKind::AgentInvoke {
                        action: agent.action.clone(),
                    },
                )
            }
        };
        RoutedInput::new(raw.epoch(), decision)
    }
}

impl RoutedInput {
    pub const fn new(raw_epoch: crate::input::InputEpoch, decision: RouteDecision) -> Self {
        Self {
            raw_epoch,
            decision,
        }
    }

    pub const fn raw_epoch(&self) -> crate::input::InputEpoch {
        self.raw_epoch
    }

    pub const fn decision(&self) -> &RouteDecision {
        &self.decision
    }

    pub const fn event(&self) -> Option<&InputEvent> {
        match &self.decision {
            RouteDecision::Routed(event) => Some(event),
            RouteDecision::BlockedByModal { .. }
            | RouteDecision::NoTarget
            | RouteDecision::TargetUnavailable
            | RouteDecision::LayerUnavailable { .. }
            | RouteDecision::Ignored => None,
        }
    }
}

fn route_pointer(raw: &RawInputEvent, layers: &LayerTree, hits: &HitTree) -> RouteDecision {
    let RawInputKind::Pointer(pointer) = raw.kind() else {
        return RouteDecision::Ignored;
    };
    let x = pointer.position.x;
    let y = pointer.position.y;

    for layer in layers.input_order() {
        let Some(node) = layers.get(layer) else {
            return RouteDecision::LayerUnavailable {
                layer: layer.clone(),
            };
        };
        match node.input_policy() {
            LayerInputPolicy::Ignore | LayerInputPolicy::PassThrough => {}
            LayerInputPolicy::HitTest | LayerInputPolicy::Capture => {
                if let Some(record) = hits.hit_in_layer(layer, x, y) {
                    return route_hit_record(
                        raw,
                        layers,
                        record,
                        InputEventKind::Pointer {
                            phase: pointer.phase,
                        },
                    );
                }
            }
            LayerInputPolicy::Modal => {
                if let Some(record) = hits.hit_in_layer(layer, x, y) {
                    return route_hit_record(
                        raw,
                        layers,
                        record,
                        InputEventKind::Pointer {
                            phase: pointer.phase,
                        },
                    );
                }
                return RouteDecision::BlockedByModal {
                    modal: layer.clone(),
                };
            }
        }
    }

    RouteDecision::NoTarget
}

fn route_keyboard(
    raw: &RawInputEvent,
    layers: &LayerTree,
    hits: &HitTree,
    state: &InteractionState,
    keyboard: &KeyboardInput,
) -> RouteDecision {
    let Some(target) = state.focus().target() else {
        return RouteDecision::NoTarget;
    };
    let Some(record) = hits.find_target(target) else {
        return RouteDecision::TargetUnavailable;
    };
    route_hit_record(
        raw,
        layers,
        record,
        InputEventKind::Key {
            key: keyboard.key.clone(),
            phase: keyboard.phase,
        },
    )
}

fn route_text(
    raw: &RawInputEvent,
    layers: &LayerTree,
    hits: &HitTree,
    state: &InteractionState,
    text: &TextInput,
) -> RouteDecision {
    let Some(target) = state.focus().target() else {
        return RouteDecision::NoTarget;
    };
    let Some(record) = hits.find_target(target) else {
        return RouteDecision::TargetUnavailable;
    };
    route_hit_record(
        raw,
        layers,
        record,
        InputEventKind::Text(text.value().to_owned()),
    )
}

fn route_hit_record(
    raw: &RawInputEvent,
    layers: &LayerTree,
    record: &HitRecord,
    kind: InputEventKind,
) -> RouteDecision {
    if !record.enabled() || !record.visible() {
        return RouteDecision::TargetUnavailable;
    }
    route_to_known_layer(raw, layers, record.layer(), record.target(), kind)
}

fn route_to_known_layer(
    raw: &RawInputEvent,
    layers: &LayerTree,
    layer: &LayerId,
    target: &crate::input::InteractionTarget,
    kind: InputEventKind,
) -> RouteDecision {
    let Some(node) = layers.get(layer) else {
        return RouteDecision::LayerUnavailable {
            layer: layer.clone(),
        };
    };
    if node.visibility() == LayerVisibility::Hidden {
        return RouteDecision::TargetUnavailable;
    }
    match node.input_policy() {
        LayerInputPolicy::Ignore | LayerInputPolicy::PassThrough => return RouteDecision::Ignored,
        LayerInputPolicy::HitTest | LayerInputPolicy::Capture | LayerInputPolicy::Modal => {}
    }
    if let Some(modal) = blocking_modal_for(layers, layer) {
        return RouteDecision::BlockedByModal { modal };
    }
    RouteDecision::Routed(InputEvent::new(raw.epoch(), target.clone(), kind))
}

fn blocking_modal_for(layers: &LayerTree, target_layer: &LayerId) -> Option<LayerId> {
    for layer in layers.input_order() {
        if layer == target_layer {
            return None;
        }
        let node = layers.get(layer)?;
        if node.input_policy() == LayerInputPolicy::Modal {
            return Some(layer.clone());
        }
    }
    None
}
