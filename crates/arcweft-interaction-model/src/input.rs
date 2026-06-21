use crate::{
    id::{Identifier, IdentifierError},
    payload::InteractionPayload,
};
use serde::{Deserialize, Serialize};

/// Epoch assigned by the input router when its routing state changes.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct InputEpoch(u64);

impl InputEpoch {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Monotonic sequence within one input source.
#[derive(
    Clone, Copy, Debug, Default, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
)]
#[serde(transparent)]
pub struct InputSequence(u64);

impl InputSequence {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// Semantic destination selected by the presentation input router.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct InteractionTarget(Identifier);

impl InteractionTarget {
    /// Creates a semantic input target.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the target is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Keyboard key token after platform normalization.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct KeyCode(Identifier);

impl KeyCode {
    /// Creates a normalized key code.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the key code is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Auxiliary,
    Back,
    Forward,
    Other(u16),
}

/// Viewport-space pointer coordinates in logical pixels.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct PointerPosition {
    pub x: i32,
    pub y: i32,
}

/// Routed input event kind. Platform-specific raw event types do not cross this boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum InputEventKind {
    PointerMove {
        position: PointerPosition,
    },
    PointerDown {
        position: PointerPosition,
        button: PointerButton,
    },
    PointerUp {
        position: PointerPosition,
        button: PointerButton,
    },
    Scroll {
        delta_x_milli: i32,
        delta_y_milli: i32,
    },
    KeyDown {
        key: KeyCode,
        repeat: bool,
    },
    KeyUp {
        key: KeyCode,
    },
    Text {
        text: String,
    },
    FocusGained,
    FocusLost,
    Custom {
        name: Identifier,
    },
}

/// Input event after presentation routing and before deterministic runtime execution.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RoutedInputEvent {
    pub epoch: InputEpoch,
    pub sequence: InputSequence,
    pub target: InteractionTarget,
    pub event: InputEventKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub payload: Option<InteractionPayload>,
}

impl RoutedInputEvent {
    #[must_use]
    pub fn new(
        epoch: InputEpoch,
        sequence: InputSequence,
        target: InteractionTarget,
        event: InputEventKind,
    ) -> Self {
        Self {
            epoch,
            sequence,
            target,
            event,
            payload: None,
        }
    }

    #[must_use]
    pub fn with_payload(mut self, payload: InteractionPayload) -> Self {
        self.payload = Some(payload);
        self
    }
}
