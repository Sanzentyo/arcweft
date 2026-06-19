use arcweft_agent_protocol::ids::{AgentRunId, SessionId, StableHash};
use serde::{Deserialize, Serialize};

/// Rebuildable Agent REPL cell row persisted for debugger continuity.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugReplCell {
    pub cell_id: String,
    pub session_id: SessionId,
    pub run_id: Option<AgentRunId>,
    pub ordinal: i64,
    pub source: String,
    pub source_hash: StableHash,
    pub status: String,
    pub inferred_type: Option<serde_json::Value>,
    pub display: Option<serde_json::Value>,
    pub partially_effectful: bool,
    pub diagnostic_ids: Vec<String>,
    pub created_unix_ms: i64,
}
