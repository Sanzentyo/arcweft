use arcweft_agent_protocol::ids::StableHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Rebuildable project-history row used by debug search and RAG context.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugHistoryEntry {
    pub history_id: String,
    pub program_hash: Option<StableHash>,
    pub symbol_id: Option<String>,
    pub change_id: String,
    pub operation_id: Option<String>,
    pub ordinal: i64,
    pub semantic_hash_before: Option<StableHash>,
    pub semantic_hash_after: Option<StableHash>,
    pub summary: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub created_unix_ms: i64,
}
