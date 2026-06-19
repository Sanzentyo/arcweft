//! Controller runner boundaries for compiled Agent Script programs.
//!
//! This crate does not interpret `.awfagent` source and does not own CLI, MCP,
//! renderer, database, filesystem, or transport I/O. It coordinates typed host
//! requests emitted by a controller VM with an `AgentSession`, debug sink, and
//! RAG service.

use arcweft_agent_protocol::{
    AgentResource,
    ids::{AgentRunId, SessionId},
    predicate::{CompareOp, Predicate, Probe},
    protocol::{
        ActionResult, AgentAction, AgentHostRequest, AgentHostResponse, AgentSessionInfo,
        CaptureRequest, CaptureResult, ObservationEnvelope, ObserveRequest, RagRequest,
        WaitRequest,
    },
    value::AgentValue,
};
use arcweft_debug_model::{
    event::{DebugEvent, DebugEventKind},
    rag::RagContextPack,
    sink::DebugEventSink,
};
use std::collections::BTreeSet;
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

/// Runtime policy resolved from compiled effects and launch profile.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RuntimeAgentPolicy {
    allowed: BTreeSet<RuntimeAgentCapability>,
}

/// Capability that may be granted to an Agent controller at runtime.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum RuntimeAgentCapability {
    Observe,
    Act,
    Capture,
    ResourceRead,
    Rag,
}

/// Runner configuration that must remain deterministic under replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunnerConfig {
    pub session_id: SessionId,
    pub run_id: Option<AgentRunId>,
    pub created_unix_ms: i64,
}

/// Host-call execution report for the current vertical slice.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentHostCallReport {
    pub response: AgentHostResponse,
    pub events_emitted: u64,
}

/// Runner for Agent controller host calls.
pub struct AgentRunner<S, D, R> {
    session: S,
    debug: D,
    rag: R,
    policy: RuntimeAgentPolicy,
    config: AgentRunnerConfig,
    sequence: u64,
}

/// Agent runner failure.
#[derive(Debug, Error)]
pub enum AgentRunError<SessionError, DebugError, RagError>
where
    SessionError: std::error::Error + Send + Sync + 'static,
    DebugError: std::error::Error + Send + Sync + 'static,
    RagError: std::error::Error + Send + Sync + 'static,
{
    #[error("Agent host request is denied by runtime policy: {0}")]
    PolicyDenied(&'static str),
    #[error("Agent session failed: {0}")]
    Session(#[source] SessionError),
    #[error("Agent debug sink failed: {0}")]
    Debug(#[source] DebugError),
    #[error("Agent RAG service failed: {0}")]
    Rag(#[source] RagError),
    #[error("Agent wait timed out after {timeout_millis} ms")]
    WaitTimeout { timeout_millis: u64 },
}

impl Default for RuntimeAgentPolicy {
    fn default() -> Self {
        Self::new([RuntimeAgentCapability::Observe])
    }
}

impl RuntimeAgentPolicy {
    pub fn new(capabilities: impl IntoIterator<Item = RuntimeAgentCapability>) -> Self {
        Self {
            allowed: capabilities.into_iter().collect(),
        }
    }

    pub fn allows(&self, capability: RuntimeAgentCapability) -> bool {
        self.allowed.contains(&capability)
    }
}

impl RuntimeAgentCapability {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Observe => "agent.observe",
            Self::Act => "agent.act",
            Self::Capture => "agent.capture",
            Self::ResourceRead => "agent.resource.read",
            Self::Rag => "agent.rag.query",
        }
    }
}

/// Result type for runner operations.
pub type AgentRunnerResult<T, S, D, R> = Result<
    T,
    AgentRunError<
        <S as AgentSession>::Error,
        <D as DebugEventSink>::Error,
        <R as RagService>::Error,
    >,
>;

impl AgentRunnerConfig {
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            run_id: None,
            created_unix_ms: 0,
        }
    }

    #[must_use]
    pub fn with_run_id(mut self, run_id: AgentRunId) -> Self {
        self.run_id = Some(run_id);
        self
    }

    #[must_use]
    pub const fn with_created_unix_ms(mut self, created_unix_ms: i64) -> Self {
        self.created_unix_ms = created_unix_ms;
        self
    }
}

impl<S, D, R> AgentRunner<S, D, R>
where
    S: AgentSession,
    D: DebugEventSink,
    R: RagService,
{
    pub fn new(
        session: S,
        debug: D,
        rag: R,
        policy: RuntimeAgentPolicy,
        config: AgentRunnerConfig,
    ) -> Self {
        Self {
            session,
            debug,
            rag,
            policy,
            config,
            sequence: 0,
        }
    }

    pub fn session_mut(&mut self) -> &mut S {
        &mut self.session
    }

    pub fn debug_mut(&mut self) -> &mut D {
        &mut self.debug
    }

    pub fn rag_mut(&mut self) -> &mut R {
        &mut self.rag
    }

    pub fn handle_host_request(
        &mut self,
        request: AgentHostRequest,
    ) -> AgentRunnerResult<AgentHostCallReport, S, D, R> {
        self.emit(DebugEventKind::StepStarted, None, serde_json::json!({}))?;
        let response = match request {
            AgentHostRequest::Observe(request) => {
                self.ensure(RuntimeAgentCapability::Observe)?;
                let observation = self
                    .session
                    .observe(request)
                    .map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Observation,
                    Some(observation.tick),
                    serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Observation(observation)
            }
            AgentHostRequest::Act(action) => {
                self.ensure(RuntimeAgentCapability::Act)?;
                let result = self.session.act(action).map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Action,
                    Some(result.after_tick),
                    serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Action(result)
            }
            AgentHostRequest::Wait(request) => {
                let observation = self.wait(&request)?;
                AgentHostResponse::Observation(observation)
            }
            AgentHostRequest::Capture(request) => {
                self.ensure(RuntimeAgentCapability::Capture)?;
                let result = self
                    .session
                    .capture(request)
                    .map_err(AgentRunError::Session)?;
                self.emit(
                    DebugEventKind::Capture,
                    None,
                    serde_json::to_value(&result).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::Capture(result)
            }
            AgentHostRequest::ReadResource { uri } => {
                self.ensure(RuntimeAgentCapability::ResourceRead)?;
                let resource = self
                    .session
                    .read_resource(uri.as_str())
                    .map_err(AgentRunError::Session)?;
                AgentHostResponse::Resource(
                    serde_json::to_value(resource).unwrap_or(serde_json::Value::Null),
                )
            }
            AgentHostRequest::RagQuery(request) => {
                self.ensure(RuntimeAgentCapability::Rag)?;
                let context = self.rag.query(request).map_err(AgentRunError::Rag)?;
                self.emit(
                    DebugEventKind::RagQuery,
                    None,
                    serde_json::to_value(&context).unwrap_or(serde_json::Value::Null),
                )?;
                AgentHostResponse::RagContext(
                    serde_json::to_value(context).unwrap_or(serde_json::Value::Null),
                )
            }
            AgentHostRequest::Checkpoint { name } => {
                self.emit(
                    DebugEventKind::Diagnostic,
                    None,
                    serde_json::json!({ "checkpoint": name }),
                )?;
                AgentHostResponse::Unit
            }
        };
        self.emit(DebugEventKind::StepFinished, None, serde_json::json!({}))?;
        Ok(AgentHostCallReport {
            response,
            events_emitted: self.sequence,
        })
    }

    fn wait(&mut self, request: &WaitRequest) -> AgentRunnerResult<ObservationEnvelope, S, D, R> {
        self.ensure(RuntimeAgentCapability::Observe)?;
        let poll_frames = request.poll_frames.max(1);
        let stable_frames = request.stable_frames.max(1);
        let max_polls = (request.timeout_millis / u64::from(poll_frames)).max(1);
        let mut stable_count = 0;
        let mut last_observation = None;

        for _ in 0..max_polls {
            let observation = self
                .session
                .step_frames(poll_frames)
                .map_err(AgentRunError::Session)?;
            if predicate_matches(&request.predicate, &observation) {
                stable_count += 1;
                if stable_count >= stable_frames {
                    self.emit(
                        DebugEventKind::Observation,
                        Some(observation.tick),
                        serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
                    )?;
                    return Ok(observation);
                }
            } else {
                stable_count = 0;
            }
            last_observation = Some(observation);
        }

        if let Some(observation) = last_observation {
            self.emit(
                DebugEventKind::Observation,
                Some(observation.tick),
                serde_json::to_value(&observation).unwrap_or(serde_json::Value::Null),
            )?;
        }
        Err(AgentRunError::WaitTimeout {
            timeout_millis: request.timeout_millis,
        })
    }

    fn ensure(&self, capability: RuntimeAgentCapability) -> AgentRunnerResult<(), S, D, R> {
        self.policy
            .allows(capability)
            .then_some(())
            .ok_or(AgentRunError::PolicyDenied(capability.as_str()))
    }

    fn emit(
        &mut self,
        kind: DebugEventKind,
        tick: Option<u64>,
        payload: serde_json::Value,
    ) -> AgentRunnerResult<(), S, D, R> {
        self.sequence += 1;
        let event = DebugEvent {
            schema_version: 1,
            session_id: self.config.session_id.clone(),
            run_id: self.config.run_id.clone(),
            sequence: self.sequence,
            tick,
            kind,
            payload,
            created_unix_ms: self.config.created_unix_ms,
        };
        self.debug.append(&event).map_err(AgentRunError::Debug)
    }
}

fn predicate_matches(predicate: &Predicate, observation: &ObservationEnvelope) -> bool {
    match predicate {
        Predicate::Compare { probe, op, value } => observation_value(probe, observation)
            .is_some_and(|actual| compare_values(&actual, *op, value)),
        Predicate::Exists { probe } => observation_value(probe, observation).is_some(),
        Predicate::ActionEnabled { .. } => false,
        Predicate::All { predicates } => predicates
            .iter()
            .all(|predicate| predicate_matches(predicate, observation)),
        Predicate::Any { predicates } => predicates
            .iter()
            .any(|predicate| predicate_matches(predicate, observation)),
        Predicate::Not { predicate } => !predicate_matches(predicate, observation),
    }
}

fn observation_value(probe: &Probe, observation: &ObservationEnvelope) -> Option<AgentValue> {
    match probe {
        Probe::Signal { target } | Probe::Metric { target } => {
            observation.signals.get(target.as_str()).cloned()
        }
        Probe::ObservationField { path } if path == "tick" => {
            Some(AgentValue::I64(i64::try_from(observation.tick).ok()?))
        }
        Probe::StatePath { .. } | Probe::ObservationField { .. } => None,
    }
}

fn compare_values(actual: &AgentValue, op: CompareOp, expected: &AgentValue) -> bool {
    match op {
        CompareOp::Eq => actual == expected,
        CompareOp::NotEq => actual != expected,
        CompareOp::Greater => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left > right),
        CompareOp::GreaterOrEqual => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left >= right),
        CompareOp::Less => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left < right),
        CompareOp::LessOrEqual => numeric_value(actual)
            .zip(numeric_value(expected))
            .is_some_and(|(left, right)| left <= right),
    }
}

fn numeric_value(value: &AgentValue) -> Option<i64> {
    match value {
        AgentValue::I64(value) => Some(*value),
        AgentValue::U64(value) => i64::try_from(*value).ok(),
        AgentValue::Null
        | AgentValue::Bool(_)
        | AgentValue::F64(_)
        | AgentValue::String(_)
        | AgentValue::Entity(_)
        | AgentValue::List(_)
        | AgentValue::Map(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_protocol::{
        ids::{AgentResourceUri, PublicId},
        protocol::{CaptureFormat, CaptureTarget},
    };
    use arcweft_debug_model::sink::NullDebugEventSink;
    use std::collections::BTreeMap;
    use std::convert::Infallible;

    #[derive(Default)]
    struct TestSession {
        observations: Vec<ObservationEnvelope>,
    }

    impl AgentSession for TestSession {
        type Error = Infallible;

        fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
            Ok(AgentSessionInfo {
                session_id: "session.test".to_owned(),
                program_hash: "hash".to_owned(),
                profile: None,
                capabilities: Vec::new(),
            })
        }

        fn observe(
            &mut self,
            _request: ObserveRequest,
        ) -> Result<ObservationEnvelope, Self::Error> {
            Ok(self.observations.remove(0))
        }

        fn act(&mut self, _action: AgentAction) -> Result<ActionResult, Self::Error> {
            Ok(ActionResult {
                accepted: true,
                before_tick: 1,
                after_tick: 2,
                before_state_hash: "a".to_owned(),
                after_state_hash: "b".to_owned(),
            })
        }

        fn capture(&mut self, _request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
            Ok(CaptureResult {
                uri: AgentResourceUri::new("agent://capture/test").expect("valid uri"),
                content_hash: "hash".to_owned(),
                media_type: "image/png".to_owned(),
                byte_len: 4,
            })
        }

        fn read_resource(&mut self, _uri: &str) -> Result<AgentResource, Self::Error> {
            unreachable!("not used by test")
        }

        fn step_frames(&mut self, _count: u32) -> Result<ObservationEnvelope, Self::Error> {
            Ok(self.observations.remove(0))
        }
    }

    fn observation(tick: u64, ready: bool) -> ObservationEnvelope {
        ObservationEnvelope {
            tick,
            frame_id: format!("frame.{tick}"),
            state_hash: format!("state.{tick}"),
            render_hash: format!("render.{tick}"),
            signals: BTreeMap::from([("signal.ready".to_owned(), AgentValue::Bool(ready))]),
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn wait_requires_stable_predicate_matches() {
        let session = TestSession {
            observations: vec![
                observation(1, false),
                observation(2, true),
                observation(3, true),
            ],
        };
        let mut runner = AgentRunner::new(
            session,
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let report = runner
            .handle_host_request(AgentHostRequest::Wait(WaitRequest {
                predicate: Predicate::Compare {
                    probe: Probe::Signal {
                        target: PublicId::new("signal.ready").expect("valid public id"),
                    },
                    op: CompareOp::Eq,
                    value: AgentValue::Bool(true),
                },
                timeout_millis: 5,
                stable_frames: 2,
                poll_frames: 1,
            }))
            .expect("wait succeeds");

        assert!(matches!(
            report.response,
            AgentHostResponse::Observation(ObservationEnvelope { tick: 3, .. })
        ));
    }

    #[test]
    fn capture_requires_policy_capability() {
        let mut runner = AgentRunner::new(
            TestSession::default(),
            NullDebugEventSink,
            NoopRagService,
            RuntimeAgentPolicy::default(),
            AgentRunnerConfig::new(SessionId::new("session.test").expect("valid session id")),
        );

        let error = runner
            .handle_host_request(AgentHostRequest::Capture(CaptureRequest {
                target: CaptureTarget::Viewport,
                format: CaptureFormat::Png,
                capture_kind: "color".to_owned(),
                name: "viewport".to_owned(),
            }))
            .expect_err("capture is denied");

        assert!(matches!(
            error,
            AgentRunError::PolicyDenied("agent.capture")
        ));
    }
}
