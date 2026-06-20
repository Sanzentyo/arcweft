use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use serde::{Deserialize, Serialize};

/// Rebuildable parser/sema/runtime/verifier/test diagnostic row.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugDiagnostic {
    pub diagnostic_id: String,
    pub program_hash: Option<StableHash>,
    pub session_id: Option<SessionId>,
    pub run_id: Option<AgentRunId>,
    pub sequence: Option<u64>,
    pub code: Option<String>,
    pub severity: String,
    pub phase: String,
    pub message: String,
    pub source_path: Option<String>,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
    pub related_ids: Vec<PublicId>,
    pub payload: serde_json::Value,
    pub created_unix_ms: i64,
}
