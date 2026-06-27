use crate::text_input::TextInput;
use arcweft_id::PublicId;

/// Stable target produced by `LayerTree` routing.
///
/// Runtime and Agent code must use this target instead of frame-local hit-test
/// IDs so replay, modal policy, focus, and capture decisions can be audited.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct InteractionTarget {
    id: PublicId,
}

/// Monotonic input epoch assigned by the host before routing.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct InputEpoch(pub u64);

/// Host-normalized input before `LayerTree` routing.
#[derive(Clone, Debug, PartialEq)]
pub struct RawInputEvent {
    epoch: InputEpoch,
    kind: RawInputKind,
}

/// Minimal raw input family kept out of `arcweft-core`.
#[derive(Clone, Debug, PartialEq)]
pub enum RawInputKind {
    Pointer(PointerInput),
    Keyboard(KeyboardInput),
    Text(TextInput),
    Agent(AgentInput),
}

/// Pointer data in viewport coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointerInput {
    pub pointer: PointerId,
    pub position: ViewportPoint,
    pub phase: PointerPhase,
}

/// Keyboard input after host normalization.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeyboardInput {
    pub key: String,
    pub phase: KeyPhase,
}

/// Agent semantic input enters the same routing system as physical input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentInput {
    pub action: PublicId,
    pub target: Option<InteractionTarget>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointerId(pub u64);

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewportPoint {
    pub x: f32,
    pub y: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyPhase {
    Down,
    Up,
}

/// LayerTree-routed input delivered to a stable target.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InputEvent {
    raw_epoch: InputEpoch,
    target: InteractionTarget,
    kind: InputEventKind,
}

/// Routed input kind after hover/focus/modal/capture policy has been applied.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InputEventKind {
    Activate,
    Pointer { phase: PointerPhase },
    Key { key: String, phase: KeyPhase },
    Text(TextInput),
    Focus { focused: bool },
    AgentInvoke { action: PublicId },
}

/// Typed semantic action emitted by Component, `TextBox`, Activity, or Agent
/// handlers after routed input has been accepted.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Action {
    target: ActionTarget,
    kind: PublicId,
    payload: Option<String>,
}

/// Target family for semantic actions. This is intentionally not named
/// `UiEvent`; UI, `TextBox`, Activity, and runtime handlers share the same data.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionTarget {
    Runtime,
    Entity(InteractionTarget),
    Activity(InteractionTarget),
}

/// Ordered semantic actions emitted in one routed input epoch.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ActionBatch {
    actions: Vec<Action>,
}

/// Host notification delivered to runtime-host orchestration after external
/// work completes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostEvent {
    source: HostEventSource,
    kind: PublicId,
    payload: Option<String>,
}

/// Host event source family. These events are owned data and may cross task or
/// Activity boundaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostEventSource {
    Task(PublicId),
    Audio(PublicId),
    Activity(InteractionTarget),
    Resource(PublicId),
}

/// Ordered host notifications collected for one runtime-host step.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HostEventBatch {
    events: Vec<HostEvent>,
}

impl InteractionTarget {
    pub const fn new(id: PublicId) -> Self {
        Self { id }
    }

    pub const fn id(&self) -> &PublicId {
        &self.id
    }
}

impl RawInputEvent {
    pub const fn new(epoch: InputEpoch, kind: RawInputKind) -> Self {
        Self { epoch, kind }
    }

    pub const fn epoch(&self) -> InputEpoch {
        self.epoch
    }

    pub const fn kind(&self) -> &RawInputKind {
        &self.kind
    }
}

impl ViewportPoint {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

impl InputEventKind {
    /// Returns the pointer phase for pointer-routed events.
    pub const fn pointer_phase(&self) -> Option<PointerPhase> {
        match self {
            Self::Pointer { phase } => Some(*phase),
            Self::Activate
            | Self::Key { .. }
            | Self::Text(_)
            | Self::Focus { .. }
            | Self::AgentInvoke { .. } => None,
        }
    }

    pub const fn focus_changed(&self) -> Option<bool> {
        match self {
            Self::Focus { focused } => Some(*focused),
            Self::Activate
            | Self::Pointer { .. }
            | Self::Key { .. }
            | Self::Text(_)
            | Self::AgentInvoke { .. } => None,
        }
    }

    /// Returns the session-scoped text-input batch for IME/text routed events.
    pub fn text_input(&self) -> Option<&TextInput> {
        match self {
            Self::Text(text) => Some(text),
            Self::Activate
            | Self::Pointer { .. }
            | Self::Key { .. }
            | Self::Focus { .. }
            | Self::AgentInvoke { .. } => None,
        }
    }

    pub const fn is_activate(&self) -> bool {
        matches!(self, Self::Activate)
    }
}

impl InputEvent {
    pub const fn new(
        raw_epoch: InputEpoch,
        target: InteractionTarget,
        kind: InputEventKind,
    ) -> Self {
        Self {
            raw_epoch,
            target,
            kind,
        }
    }

    pub const fn activate(raw_epoch: InputEpoch, target: InteractionTarget) -> Self {
        Self::new(raw_epoch, target, InputEventKind::Activate)
    }

    pub const fn focus_changed(
        raw_epoch: InputEpoch,
        target: InteractionTarget,
        focused: bool,
    ) -> Self {
        Self::new(raw_epoch, target, InputEventKind::Focus { focused })
    }

    pub const fn raw_epoch(&self) -> InputEpoch {
        self.raw_epoch
    }

    pub const fn target(&self) -> &InteractionTarget {
        &self.target
    }

    pub const fn kind(&self) -> &InputEventKind {
        &self.kind
    }
}

impl Action {
    pub fn new(target: ActionTarget, kind: PublicId) -> Self {
        Self {
            target,
            kind,
            payload: None,
        }
    }

    #[must_use]
    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub const fn target(&self) -> &ActionTarget {
        &self.target
    }

    pub const fn kind(&self) -> &PublicId {
        &self.kind
    }

    pub const fn payload(&self) -> Option<&String> {
        self.payload.as_ref()
    }
}

impl ActionBatch {
    pub fn push(&mut self, action: Action) {
        self.actions.push(action);
    }

    pub fn extend(&mut self, other: Self) {
        self.actions.extend(other.actions);
    }

    pub fn as_slice(&self) -> &[Action] {
        &self.actions
    }

    pub fn into_vec(self) -> Vec<Action> {
        self.actions
    }
}

impl HostEvent {
    pub fn new(source: HostEventSource, kind: PublicId) -> Self {
        Self {
            source,
            kind,
            payload: None,
        }
    }

    #[must_use]
    pub fn with_payload(mut self, payload: impl Into<String>) -> Self {
        self.payload = Some(payload.into());
        self
    }

    pub const fn source(&self) -> &HostEventSource {
        &self.source
    }

    pub const fn kind(&self) -> &PublicId {
        &self.kind
    }

    pub const fn payload(&self) -> Option<&String> {
        self.payload.as_ref()
    }
}

impl HostEventBatch {
    pub fn push(&mut self, event: HostEvent) {
        self.events.push(event);
    }

    pub fn extend(&mut self, other: Self) {
        self.events.extend(other.events);
    }

    pub fn as_slice(&self) -> &[HostEvent] {
        &self.events
    }

    pub fn into_vec(self) -> Vec<HostEvent> {
        self.events
    }
}
