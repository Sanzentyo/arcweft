use arcweft_agent_protocol::ids::{SessionId, StableHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Persisted Agent debug/product session row.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugSession {
    pub session_id: SessionId,
    pub program_hash: Option<StableHash>,
    pub profile: String,
    pub transport: String,
    pub started_unix_ms: i64,
    pub ended_unix_ms: Option<i64>,
    pub status: DebugSessionStatus,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Lifecycle state stored for a persisted Agent session.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSessionStatus {
    Running,
    Finished,
    Failed,
}

impl DebugSessionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Finished => "finished",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "finished" => Some(Self::Finished),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}
