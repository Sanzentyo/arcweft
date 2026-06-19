use crate::ids::{AgentRunId, SessionId, StableHash};
use serde::{Deserialize, Serialize};

/// Logical Agent execution trace record stored in `.arcwx`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct AgentTraceRecord {
    pub schema_version: u32,
    pub run_id: AgentRunId,
    pub session_id: Option<SessionId>,
    pub sequence: u64,
    pub tick: Option<u64>,
    pub kind: AgentTraceKind,
    pub payload_hash: StableHash,
    pub payload: serde_json::Value,
    pub blob_refs: Vec<StableHash>,
}

/// Stable trace event family.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentTraceKind {
    RunStarted,
    VmStep,
    HostCallRequested,
    ObservationReceived,
    ActionCompleted,
    CaptureStored,
    ResourceReadCompleted,
    AssertionEvaluated,
    RagQueryCompleted,
    DiagnosticEmitted,
    RunFinished,
}
