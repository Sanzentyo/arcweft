use arcweft_agent_protocol::ids::{AgentRunId, StableHash};
use serde::{Deserialize, Serialize};

/// Rebuildable test, bench, or visual-comparison result row.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugTestResult {
    pub test_result_id: String,
    pub program_hash: Option<StableHash>,
    pub run_id: Option<AgentRunId>,
    pub test_id: String,
    pub kind: String,
    pub outcome: String,
    pub duration_millis: Option<u64>,
    pub diagnostic_ids: Vec<String>,
    pub artifact_refs: Vec<String>,
    pub summary: String,
    pub created_unix_ms: i64,
}
