use crate::chunk::{ChunkId, ChunkSourceKind, SourceAnchor};
use arcweft_agent_protocol::ids::{PublicId, StableHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Retrieval channel contributing a ranked candidate list.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchChannel {
    ExactEntity,
    Lexical,
    Vector,
    Graph,
    History,
    Diagnostics,
    Trace,
    Summary,
}

/// One channel-local search hit.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct SearchHit {
    pub chunk_id: ChunkId,
    pub channel: SearchChannel,
    pub rank: usize,
    pub score: Option<f64>,
}

/// Query supplied by Agent Script, REPL, CLI, or MCP.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RagQuery {
    pub query_id: String,
    pub text: String,
    pub program_hash: StableHash,
    pub roots: Vec<PublicId>,
    pub graph_depth: u32,
    pub limit: usize,
    pub max_context_bytes: usize,
}

/// Result after deterministic multi-channel fusion.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct FusedSearchHit {
    pub chunk_id: ChunkId,
    pub fused_score: f64,
    pub channels: BTreeSet<SearchChannel>,
}

/// Context item selected for an LLM/debugger request.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RagContextItem {
    pub chunk_id: ChunkId,
    pub kind: ChunkSourceKind,
    pub title: String,
    pub body: String,
    pub fused_score: f64,
    pub channels: BTreeSet<SearchChannel>,
    pub entity_ids: Vec<PublicId>,
    pub source_anchor: Option<SourceAnchor>,
}

/// Explainable context package; never a raw untracked text concatenation.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct RagContextPack {
    pub schema_version: u32,
    pub query: RagQuery,
    pub items: Vec<RagContextItem>,
    pub truncated: bool,
}
