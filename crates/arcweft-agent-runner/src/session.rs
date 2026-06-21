use arcweft_agent_protocol::{
    protocol::{
        ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureRequest,
        CaptureResult, ObservationEnvelope, ObserveRequest, RagRequest,
    },
    resource::AgentResource,
    trace::{AgentTraceKind, AgentTraceRecord},
};
use arcweft_debug_model::rag::RagContextPack;
use thiserror::Error;

/// Target application boundary used by the controller runner.
pub trait AgentSession {
    type Error: std::error::Error + Send + Sync + 'static;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error>;

    fn observe(&mut self, request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error>;

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error>;

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error>;

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error>;

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error>;
}

/// Deterministic retrieval boundary used by `rag.query`.
pub trait RagService {
    type Error: std::error::Error + Send + Sync + 'static;

    fn query(&mut self, request: RagRequest) -> Result<RagContextPack, Self::Error>;
}

/// RAG service used when retrieval is disabled by policy.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoopRagService;

impl RagService for NoopRagService {
    type Error = std::convert::Infallible;

    fn query(&mut self, _request: RagRequest) -> Result<RagContextPack, Self::Error> {
        Ok(RagContextPack {
            schema_version: 1,
            query: arcweft_debug_model::rag::RagQuery {
                query_id: "noop".to_owned(),
                text: String::new(),
                program_hash: arcweft_agent_protocol::ids::StableHash::new("noop")
                    .expect("static noop hash is nonempty"),
                roots: Vec::new(),
                graph_depth: 0,
                limit: 0,
                max_context_bytes: 0,
            },
            items: Vec::new(),
            truncated: false,
        })
    }
}

/// Read-only `AgentSession` backed by validated `.arcwx` trace records.
///
/// This session does not validate trace hashes or read files; callers should
/// use the normal trace reader before constructing it. It replays recorded host
/// responses in trace order and fails if the controller asks for a different
/// response family than the next recorded host event.
#[derive(Clone, Debug)]
pub struct ReplayAgentSession {
    session_info: AgentSessionInfo,
    records: Vec<AgentTraceRecord>,
    cursor: usize,
}

impl ReplayAgentSession {
    #[must_use]
    pub fn new(session_info: AgentSessionInfo, records: Vec<AgentTraceRecord>) -> Self {
        Self {
            session_info,
            records,
            cursor: 0,
        }
    }

    #[must_use]
    pub fn from_trace_records(records: Vec<AgentTraceRecord>) -> Self {
        let session_id = records
            .iter()
            .find_map(|record| record.session_id.as_ref())
            .map_or_else(
                || "session.replay".to_owned(),
                |session| session.as_str().to_owned(),
            );
        let program_hash = records.first().map_or_else(
            || "trace.replay".to_owned(),
            |record| record.run_id.as_str().to_owned(),
        );
        Self::new(
            AgentSessionInfo {
                session_id,
                program_hash,
                project_entities: Vec::new(),
                project_graph: AgentProjectGraph::default(),
                profile: Some("trace.replay".to_owned()),
                capabilities: vec![
                    "observe".to_owned(),
                    "act".to_owned(),
                    "capture".to_owned(),
                    "resource_read".to_owned(),
                    "step_frames".to_owned(),
                ],
            },
            records,
        )
    }

    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    fn next_payload(
        &mut self,
        expected: AgentTraceKind,
    ) -> Result<serde_json::Value, ReplayAgentSessionError> {
        while let Some(record) = self.records.get(self.cursor) {
            self.cursor += 1;
            if record.kind == expected {
                return Ok(record.payload.clone());
            }
            if replay_agent_session_host_response_kind(record.kind) {
                return Err(ReplayAgentSessionError::UnexpectedRecordKind {
                    expected,
                    found: record.kind,
                    sequence: record.sequence,
                });
            }
        }
        Err(ReplayAgentSessionError::TraceExhausted { expected })
    }

    fn next_decoded<T>(&mut self, expected: AgentTraceKind) -> Result<T, ReplayAgentSessionError>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_value(self.next_payload(expected)?).map_err(|error| {
            ReplayAgentSessionError::PayloadDecode {
                kind: expected,
                message: error.to_string(),
            }
        })
    }
}

impl AgentSession for ReplayAgentSession {
    type Error = ReplayAgentSessionError;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        Ok(self.session_info.clone())
    }

    fn observe(&mut self, _request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        self.next_decoded(AgentTraceKind::ObservationReceived)
    }

    fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
        self.next_decoded(AgentTraceKind::ActionCompleted)
    }

    fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        self.next_decoded(AgentTraceKind::CaptureStored)
    }

    fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
        self.next_decoded(AgentTraceKind::ResourceReadCompleted)
    }

    fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
        self.next_decoded(AgentTraceKind::ObservationReceived)
    }
}

fn replay_agent_session_host_response_kind(kind: AgentTraceKind) -> bool {
    matches!(
        kind,
        AgentTraceKind::ObservationReceived
            | AgentTraceKind::ActionCompleted
            | AgentTraceKind::CaptureStored
            | AgentTraceKind::ResourceReadCompleted
    )
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ReplayAgentSessionError {
    #[error("trace replay expected {expected:?}, but found {found:?} at sequence {sequence}")]
    UnexpectedRecordKind {
        expected: AgentTraceKind,
        found: AgentTraceKind,
        sequence: u64,
    },
    #[error("trace replay exhausted before {expected:?}")]
    TraceExhausted { expected: AgentTraceKind },
    #[error("trace replay could not decode {kind:?} payload: {message}")]
    PayloadDecode {
        kind: AgentTraceKind,
        message: String,
    },
}
