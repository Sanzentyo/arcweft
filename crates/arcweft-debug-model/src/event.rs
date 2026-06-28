use arcweft_agent_protocol::ids::{AgentRunId, SessionId};
use serde::{Deserialize, Serialize};

/// Append-only event consumed by a debug store adapter.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct DebugEvent {
    pub schema_version: u32,
    pub session_id: SessionId,
    pub run_id: Option<AgentRunId>,
    pub sequence: u64,
    pub tick: Option<u64>,
    pub kind: DebugEventKind,
    pub payload: serde_json::Value,
    pub created_unix_ms: i64,
}

/// Stable debug event family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugEventKind {
    SessionStarted,
    SessionFinished,
    RunStarted,
    RunFinished,
    StepStarted,
    StepFinished,
    Observation,
    Action,
    Capture,
    ResourceRead,
    Assertion,
    Diagnostic,
    RagQuery,
    ReplCell,
}

impl DebugEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionStarted => "session_started",
            Self::SessionFinished => "session_finished",
            Self::RunStarted => "run_started",
            Self::RunFinished => "run_finished",
            Self::StepStarted => "step_started",
            Self::StepFinished => "step_finished",
            Self::Observation => "observation",
            Self::Action => "action",
            Self::Capture => "capture",
            Self::ResourceRead => "resource_read",
            Self::Assertion => "assertion",
            Self::Diagnostic => "diagnostic",
            Self::RagQuery => "rag_query",
            Self::ReplCell => "repl_cell",
        }
    }
}
