use arcweft_agent_protocol::ids::{AgentRunId, PublicId, SessionId, StableHash};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Persisted Agent controller run row.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugScriptRun {
    pub run_id: AgentRunId,
    pub session_id: SessionId,
    pub agent_id: Option<PublicId>,
    pub artifact_hash: Option<StableHash>,
    pub source_hash: Option<StableHash>,
    pub project_binding_mode: String,
    pub started_sequence: u64,
    pub finished_sequence: Option<u64>,
    pub outcome: DebugScriptRunOutcome,
    pub partially_effectful: bool,
    pub trace_uri: Option<String>,
    pub error: Option<serde_json::Value>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Final lifecycle fields applied when an Agent controller run completes.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugScriptRunFinish {
    pub outcome: DebugScriptRunOutcome,
    pub finished_sequence: u64,
    pub partially_effectful: bool,
    pub trace_uri: Option<String>,
    pub error: Option<serde_json::Value>,
    pub metadata: BTreeMap<String, serde_json::Value>,
}

/// Lifecycle state stored for a persisted Agent controller run.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugScriptRunOutcome {
    Running,
    Done,
    Failed,
}

impl DebugScriptRunOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Done => "done",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "running" => Some(Self::Running),
            "done" => Some(Self::Done),
            "failed" => Some(Self::Failed),
            _ => None,
        }
    }
}
