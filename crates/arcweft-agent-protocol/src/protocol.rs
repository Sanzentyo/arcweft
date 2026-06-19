use crate::{
    ids::{AgentResourceUri, PublicId},
    predicate::Predicate,
    value::AgentValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable session metadata exposed to Agent controllers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSessionInfo {
    pub session_id: String,
    pub program_hash: String,
    pub profile: Option<String>,
    pub capabilities: Vec<String>,
}

/// Minimal observation request used by the controller host boundary.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObserveRequest {
    pub include_images: bool,
    pub include_objects: bool,
    pub include_logs: bool,
}

/// Physical or semantic action requested by a compiled Agent program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentAction {
    AdvanceText,
    SelectChoice {
        choice: PublicId,
    },
    Invoke {
        target: PublicId,
        action: String,
        args: BTreeMap<String, AgentValue>,
    },
    PointerClick {
        x: u32,
        y: u32,
        button: PointerButton,
    },
}

/// Pointer button for explicit physical input.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Primary,
    Secondary,
    Middle,
}

/// Result returned after dispatching one action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ActionResult {
    pub accepted: bool,
    pub before_tick: u64,
    pub after_tick: u64,
    pub before_state_hash: String,
    pub after_state_hash: String,
}

/// Capture target visible to an Agent script.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CaptureTarget {
    Viewport,
    Layer { id: PublicId },
    Object { id: String },
}

/// Capture encoding and semantic attachment kind.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFormat {
    Png,
    RawRgba,
    Svg,
}

/// Capture request emitted by a compiled controller.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureRequest {
    pub target: CaptureTarget,
    pub format: CaptureFormat,
    pub capture_kind: String,
    pub name: String,
}

/// Stored capture reference returned to the script.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CaptureResult {
    pub uri: AgentResourceUri,
    pub content_hash: String,
    pub media_type: String,
    pub byte_len: u64,
}

/// Bounded wait request executed as a runner state machine.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WaitRequest {
    pub predicate: Predicate,
    pub timeout_millis: u64,
    pub stable_frames: u32,
    pub poll_frames: u32,
}

/// RAG request kept separate from pass/fail assertion semantics.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RagRequest {
    pub query: String,
    pub roots: Vec<PublicId>,
    pub graph_depth: u32,
    pub limit: usize,
}

/// Generic host request emitted by Agent controller bytecode.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "request", rename_all = "snake_case")]
pub enum AgentHostRequest {
    Observe(Box<ObserveRequest>),
    Act(Box<AgentAction>),
    Wait(Box<WaitRequest>),
    Capture(Box<CaptureRequest>),
    ReadResource { uri: AgentResourceUri },
    RagQuery(Box<RagRequest>),
    Checkpoint { name: String },
}

/// Response envelope returned across the Agent controller host boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "response", rename_all = "snake_case")]
pub enum AgentHostResponse {
    Observation(Box<ObservationEnvelope>),
    Action(ActionResult),
    Capture(CaptureResult),
    Resource(Box<serde_json::Value>),
    RagContext(Box<serde_json::Value>),
    Unit,
}

/// Observation envelope used when a controller needs a compact host response.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ObservationEnvelope {
    pub tick: u64,
    pub frame_id: String,
    pub state_hash: String,
    pub render_hash: String,
    pub signals: BTreeMap<String, AgentValue>,
    pub payload: serde_json::Value,
}
