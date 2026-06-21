//! Reusable Agent session fixtures for runner, REPL, and MCP contract tests.

use std::collections::VecDeque;

use arcweft_agent_protocol::{
    protocol::{
        ActionResult, AgentAction, AgentProjectGraph, AgentSessionInfo, CaptureRequest,
        CaptureResult, ObservationEnvelope, ObserveRequest,
    },
    resource::AgentResource,
};
use arcweft_agent_runner::session::AgentSession;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ExpectedAgentCall {
    Info,
    Observe(ObserveRequest),
    Act(AgentAction),
    Capture(CaptureRequest),
    ReadResource { uri: String },
    StepFrames { count: u32 },
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum FixtureAgentResponse {
    Info(AgentSessionInfo),
    Observe(ObservationEnvelope),
    Act(ActionResult),
    Capture(CaptureResult),
    ReadResource(Box<AgentResource>),
    StepFrames(ObservationEnvelope),
    Error(String),
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct ExpectedAgentExchange {
    pub call: ExpectedAgentCall,
    pub response: FixtureAgentResponse,
}

#[derive(Clone, Debug, Default)]
pub struct FixtureAgentSession {
    expected: VecDeque<ExpectedAgentExchange>,
    observed: Vec<ExpectedAgentCall>,
}

impl FixtureAgentSession {
    #[must_use]
    pub fn new(expected: impl IntoIterator<Item = ExpectedAgentExchange>) -> Self {
        Self {
            expected: expected.into_iter().collect(),
            observed: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_info(info: AgentSessionInfo) -> Self {
        Self::new([ExpectedAgentExchange {
            call: ExpectedAgentCall::Info,
            response: FixtureAgentResponse::Info(info),
        }])
    }

    #[must_use]
    pub fn default_info(session_id: impl Into<String>) -> AgentSessionInfo {
        AgentSessionInfo {
            session_id: session_id.into(),
            program_hash: "fixture.program".to_owned(),
            project_entities: Vec::new(),
            project_graph: AgentProjectGraph::default(),
            profile: Some("fixture".to_owned()),
            capabilities: vec![
                "observe".to_owned(),
                "act".to_owned(),
                "capture".to_owned(),
                "resource_read".to_owned(),
                "step_frames".to_owned(),
            ],
        }
    }

    #[must_use]
    pub fn observed_calls(&self) -> &[ExpectedAgentCall] {
        &self.observed
    }

    pub fn assert_exhausted(&self) -> Result<(), FixtureAgentSessionError> {
        if self.expected.is_empty() {
            Ok(())
        } else {
            Err(FixtureAgentSessionError::ExpectedCallsRemaining {
                remaining: self.expected.len(),
            })
        }
    }

    fn exchange(
        &mut self,
        call: ExpectedAgentCall,
    ) -> Result<FixtureAgentResponse, FixtureAgentSessionError> {
        self.observed.push(call.clone());
        let Some(exchange) = self.expected.pop_front() else {
            return Err(FixtureAgentSessionError::UnexpectedCall {
                call: Box::new(call),
            });
        };
        if exchange.call != call {
            return Err(FixtureAgentSessionError::CallMismatch {
                expected: Box::new(exchange.call),
                found: Box::new(call),
            });
        }
        match exchange.response {
            FixtureAgentResponse::Error(message) => {
                Err(FixtureAgentSessionError::Fixture { message })
            }
            response => Ok(response),
        }
    }
}

impl AgentSession for FixtureAgentSession {
    type Error = FixtureAgentSessionError;

    fn info(&mut self) -> Result<AgentSessionInfo, Self::Error> {
        match self.exchange(ExpectedAgentCall::Info)? {
            FixtureAgentResponse::Info(value) => Ok(value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "info",
                found: response.kind_name(),
            }),
        }
    }

    fn observe(&mut self, request: ObserveRequest) -> Result<ObservationEnvelope, Self::Error> {
        match self.exchange(ExpectedAgentCall::Observe(request))? {
            FixtureAgentResponse::Observe(value) => Ok(value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "observe",
                found: response.kind_name(),
            }),
        }
    }

    fn act(&mut self, action: AgentAction) -> Result<ActionResult, Self::Error> {
        match self.exchange(ExpectedAgentCall::Act(action))? {
            FixtureAgentResponse::Act(value) => Ok(value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "act",
                found: response.kind_name(),
            }),
        }
    }

    fn capture(&mut self, request: CaptureRequest) -> Result<CaptureResult, Self::Error> {
        match self.exchange(ExpectedAgentCall::Capture(request))? {
            FixtureAgentResponse::Capture(value) => Ok(value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "capture",
                found: response.kind_name(),
            }),
        }
    }

    fn read_resource(&mut self, uri: &str) -> Result<AgentResource, Self::Error> {
        match self.exchange(ExpectedAgentCall::ReadResource {
            uri: uri.to_owned(),
        })? {
            FixtureAgentResponse::ReadResource(value) => Ok(*value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "read_resource",
                found: response.kind_name(),
            }),
        }
    }

    fn step_frames(&mut self, count: u32) -> Result<ObservationEnvelope, Self::Error> {
        match self.exchange(ExpectedAgentCall::StepFrames { count })? {
            FixtureAgentResponse::StepFrames(value) => Ok(value),
            response => Err(FixtureAgentSessionError::ResponseMismatch {
                expected: "step_frames",
                found: response.kind_name(),
            }),
        }
    }
}

impl FixtureAgentResponse {
    const fn kind_name(&self) -> &'static str {
        match self {
            Self::Info(_) => "info",
            Self::Observe(_) => "observe",
            Self::Act(_) => "act",
            Self::Capture(_) => "capture",
            Self::ReadResource(_) => "read_resource",
            Self::StepFrames(_) => "step_frames",
            Self::Error(_) => "error",
        }
    }
}

#[derive(Clone, Debug, Error, PartialEq)]
pub enum FixtureAgentSessionError {
    #[error("unexpected Agent fixture call: {call:?}")]
    UnexpectedCall { call: Box<ExpectedAgentCall> },
    #[error("Agent fixture expected {expected:?}, but found {found:?}")]
    CallMismatch {
        expected: Box<ExpectedAgentCall>,
        found: Box<ExpectedAgentCall>,
    },
    #[error("Agent fixture response mismatch: expected {expected}, found {found}")]
    ResponseMismatch {
        expected: &'static str,
        found: &'static str,
    },
    #[error("Agent fixture error: {message}")]
    Fixture { message: String },
    #[error("Agent fixture has {remaining} expected calls remaining")]
    ExpectedCallsRemaining { remaining: usize },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_agent_session_matches_exact_call_sequence() {
        let info = FixtureAgentSession::default_info("session.fixture");
        let mut session = FixtureAgentSession::with_info(info.clone());

        assert_eq!(session.info().expect("info response"), info);
        assert_eq!(session.observed_calls(), &[ExpectedAgentCall::Info]);
        session.assert_exhausted().expect("all calls consumed");
    }

    #[test]
    fn fixture_agent_session_reports_mismatched_call() {
        let mut session =
            FixtureAgentSession::with_info(FixtureAgentSession::default_info("session.fixture"));

        let error = session
            .step_frames(1)
            .expect_err("step should not match queued info call");
        let FixtureAgentSessionError::CallMismatch { expected, found } = error else {
            panic!("expected call mismatch");
        };
        assert_eq!(*expected, ExpectedAgentCall::Info);
        assert_eq!(*found, ExpectedAgentCall::StepFrames { count: 1 });
    }
}
