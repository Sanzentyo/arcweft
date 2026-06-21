use crate::{
    id::{Identifier, IdentifierError},
    payload::InteractionPayload,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioResourceId(Identifier);

impl AudioResourceId {
    /// Creates an audio resource identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the identifier is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct AudioVoiceId(Identifier);

impl AudioVoiceId {
    /// Creates an audio voice identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdentifierError`] when the identifier is empty.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentifierError> {
        Identifier::new(value).map(Self)
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

/// Typed host-to-runtime audio event.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AudioEvent {
    Play {
        voice: AudioVoiceId,
        resource: AudioResourceId,
        looped: bool,
    },
    Stop {
        voice: AudioVoiceId,
    },
    SetGain {
        voice: AudioVoiceId,
        gain_milli: i32,
    },
    Seek {
        voice: AudioVoiceId,
        position_millis: u64,
    },
    Finished {
        voice: AudioVoiceId,
    },
}

/// Typed host event family consumed at the runtime step boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostEvent {
    Audio {
        event: AudioEvent,
    },
    Signal {
        name: Identifier,
        value: InteractionPayload,
    },
    Metric {
        name: Identifier,
        value: InteractionPayload,
    },
    Custom {
        name: Identifier,
        payload: InteractionPayload,
    },
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(transparent)]
pub struct HostEventBatch(Vec<HostEvent>);

impl HostEventBatch {
    #[must_use]
    pub fn new(events: Vec<HostEvent>) -> Self {
        Self(events)
    }

    #[must_use]
    pub fn as_slice(&self) -> &[HostEvent] {
        &self.0
    }

    #[must_use]
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HostEvent> {
        self.0.iter()
    }

    #[must_use]
    pub fn into_vec(self) -> Vec<HostEvent> {
        self.0
    }
}
