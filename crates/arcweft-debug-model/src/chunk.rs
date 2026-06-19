use arcweft_agent_protocol::ids::{PublicId, StableHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable debug chunk identifier.
#[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct ChunkId(String);

/// Privacy policy applied before persistence and embedding.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClass {
    Public,
    Project,
    Sensitive,
    Secret,
}

/// Semantic source of one retrieval chunk.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChunkSourceKind {
    Source,
    Symbol,
    GraphSummary,
    Diagnostic,
    TestResult,
    AgentTrace,
    History,
    Documentation,
}

/// Optional source range retained for explainable retrieval.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct SourceAnchor {
    pub path: String,
    pub start_byte: u64,
    pub end_byte: u64,
}

/// Rebuildable semantic retrieval unit.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DebugChunk {
    pub id: ChunkId,
    pub program_hash: Option<StableHash>,
    pub source_kind: ChunkSourceKind,
    pub source_key: String,
    pub title: String,
    pub body: String,
    pub content_hash: StableHash,
    pub semantic_hash: Option<StableHash>,
    pub source_anchor: Option<SourceAnchor>,
    pub entity_ids: Vec<PublicId>,
    pub privacy: PrivacyClass,
    pub metadata: BTreeMap<String, serde_json::Value>,
    pub created_unix_ms: i64,
}

impl ChunkId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl PrivacyClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Project => "project",
            Self::Sensitive => "sensitive",
            Self::Secret => "secret",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "public" => Some(Self::Public),
            "project" => Some(Self::Project),
            "sensitive" => Some(Self::Sensitive),
            "secret" => Some(Self::Secret),
            _ => None,
        }
    }

    pub const fn is_allowed_by(self, max: Self) -> bool {
        matches!(
            (self, max),
            (Self::Public, _)
                | (
                    Self::Project,
                    Self::Project | Self::Sensitive | Self::Secret
                )
                | (Self::Sensitive, Self::Sensitive | Self::Secret)
                | (Self::Secret, Self::Secret)
        )
    }
}

impl ChunkSourceKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Symbol => "symbol",
            Self::GraphSummary => "graph_summary",
            Self::Diagnostic => "diagnostic",
            Self::TestResult => "test_result",
            Self::AgentTrace => "agent_trace",
            Self::History => "history",
            Self::Documentation => "documentation",
        }
    }
}
