use serde::{Deserialize, Serialize};

/// Current audio observation slice.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentAudioState {
    pub active_voices: Vec<String>,
    pub pending_events: Vec<String>,
}

/// Named observation value in the shared typed Agent value grammar.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentAssignment {
    pub name: String,
    pub value: crate::value::AgentValue,
}
