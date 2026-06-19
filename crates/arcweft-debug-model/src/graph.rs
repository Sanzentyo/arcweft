use arcweft_agent_protocol::ids::{PublicId, StableHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Rebuildable symbol row used by graph debug search and RAG expansion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugGraphSymbol {
    pub symbol_id: String,
    pub program_hash: StableHash,
    pub public_id: Option<PublicId>,
    pub qualified_name: Option<String>,
    pub kind: String,
    pub type_json: Option<serde_json::Value>,
    pub start_byte: Option<u64>,
    pub end_byte: Option<u64>,
    pub semantic_hash: Option<StableHash>,
    pub summary: String,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Rebuildable directed edge between two indexed symbols.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugGraphEdge {
    pub program_hash: StableHash,
    pub from_symbol_id: String,
    pub to_symbol_id: String,
    pub edge_kind: String,
    pub weight: f64,
    pub metadata: BTreeMap<String, serde_json::Value>,
}
