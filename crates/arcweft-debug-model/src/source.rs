use arcweft_agent_protocol::ids::StableHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Rebuildable source-file inventory row for a debug-store program.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugSourceFile {
    pub program_hash: StableHash,
    pub path: String,
    pub language: String,
    pub content_hash: StableHash,
    pub byte_len: u64,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
