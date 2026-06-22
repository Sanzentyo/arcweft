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
    pub const fn rank(self) -> u8 {
        match self {
            Self::Public => 0,
            Self::Project => 1,
            Self::Sensitive => 2,
            Self::Secret => 3,
        }
    }

    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        if self.rank() >= other.rank() {
            self
        } else {
            other
        }
    }

    /// Returns true when an explicit, audited declassification may lower
    /// `self` to `target`. This method expresses direction, not authorization.
    pub const fn can_declassify_to(self, target: Self) -> bool {
        target.rank() <= self.rank()
    }

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
        self.rank() <= max.rank()
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
