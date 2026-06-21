use crate::action::AgentActionTarget;
use crate::artifact::RequiredEntity;
use crate::ids::{AgentResourceUri, PublicId};
use crate::predicate::Predicate;
use crate::value::AgentValue;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable session metadata exposed to Agent controllers.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentSessionInfo {
    pub session_id: String,
    pub program_hash: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_entities: Vec<RequiredEntity>,
    #[serde(default, skip_serializing_if = "AgentProjectGraph::is_empty")]
    pub project_graph: AgentProjectGraph,
    pub profile: Option<String>,
    pub capabilities: Vec<String>,
}

/// Agent-readable project graph snapshot for typed debug/readback queries.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectGraph {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<AgentProjectGraphSymbol>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<AgentProjectGraphEdge>,
}

impl AgentProjectGraph {
    pub fn is_empty(&self) -> bool {
        self.symbols.is_empty() && self.edges.is_empty()
    }
}

/// One graph symbol known to the Agent runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectGraphSymbol {
    pub symbol_id: String,
    pub public_id: Option<PublicId>,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub semantic_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flow_control: Option<AgentProjectFlowControlSummary>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_summary: Option<AgentProjectGraphSummary>,
    pub summary: String,
}

/// Project-level graph counters attached to the project summary graph symbol.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectGraphSummary {
    pub entity_count: u32,
    pub agent_action_count: u32,
    pub project_callable_count: u32,
    pub relation_count: u32,
    pub dependency_edge_count: u32,
    pub dynamic_control_flow_count: u32,
    pub debug_query_count: u32,
}

/// Static and dynamic control-flow counters attached to a flow graph symbol.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectFlowControlSummary {
    pub has_dynamic_control: bool,
    pub static_goto_count: u32,
    pub dynamic_goto_count: u32,
    pub branch_count: u32,
    pub loop_count: u32,
    pub await_count: u32,
    pub thread_count: u32,
    pub select_branch_count: u32,
}

/// One directed graph edge known to the Agent runtime.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectGraphEdge {
    pub from_symbol_id: String,
    pub to_symbol_id: String,
    pub edge_kind: String,
}

/// Typed Agent response containing graph symbols and edges near one root.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentProjectGraphNeighborhood {
    pub root: PublicId,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub symbols: Vec<AgentProjectGraphSymbol>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<AgentProjectGraphEdge>,
}

/// Minimal observation request used by the controller host boundary.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ObserveRequest {
    pub include_images: bool,
    pub include_objects: bool,
    pub include_logs: bool,
}

impl Default for ObserveRequest {
    fn default() -> Self {
        Self {
            include_images: false,
            include_objects: true,
            include_logs: false,
        }
    }
}

/// Physical or semantic action requested by a compiled Agent program.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AgentAction {
    AdvanceText,
    SelectChoice {
        choice: PublicId,
    },
    Invoke(Box<AgentInvokeAction>),
    PointerClick {
        x: u32,
        y: u32,
        button: PointerButton,
    },
}

/// Semantic invocation action payload.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentInvokeAction {
    pub target: PublicId,
    pub action: String,
    pub args: Box<BTreeMap<String, AgentValue>>,
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

/// Assertion polarity requested by an Agent controller.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentAssertionKind {
    Expect,
    Deny,
}

/// Runtime-evaluated Agent assertion emitted by controller bytecode.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct AgentAssertionRequest {
    pub kind: AgentAssertionKind,
    pub condition: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub message: String,
}

/// Debug-record attachment emitted by an Agent controller.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentAttachment {
    pub resource: Box<serde_json::Value>,
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
    EntityMetadata { entity: PublicId },
    ProjectGraphNeighborhood { root: PublicId, depth: u32 },
    RagQuery(Box<RagRequest>),
    Assert(Box<AgentAssertionRequest>),
    Attach(Box<AgentAttachment>),
    Checkpoint { name: String },
}

/// Response envelope returned across the Agent controller host boundary.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "response", rename_all = "snake_case")]
pub enum AgentHostResponse {
    Observation(Box<ObservationEnvelope>),
    Action(Box<ActionResult>),
    Capture(Box<CaptureResult>),
    Resource(Box<serde_json::Value>),
    EntityMetadata(Box<RequiredEntity>),
    ProjectGraphNeighborhood(Box<AgentProjectGraphNeighborhood>),
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
    #[serde(default)]
    pub actions: Vec<AgentActionTarget>,
    pub signals: BTreeMap<String, AgentValue>,
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn invoke_action_keeps_flat_wire_args() {
        let action = AgentAction::Invoke(Box::new(AgentInvokeAction {
            target: PublicId::new("@object.dialogue").expect("valid public id"),
            action: "highlight".to_owned(),
            args: Box::new(BTreeMap::from([(
                "intensity".to_owned(),
                AgentValue::I64(7),
            )])),
        }));

        let value = serde_json::to_value(&action).expect("serializes invoke action");

        assert_eq!(
            value,
            json!({
                "kind": "invoke",
                "target": "@object.dialogue",
                "action": "highlight",
                "args": {
                    "intensity": {
                        "kind": "i64",
                        "value": 7
                    }
                }
            })
        );
    }
}
