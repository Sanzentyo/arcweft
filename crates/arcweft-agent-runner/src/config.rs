use arcweft_agent_protocol::{
    ids::{AgentRunId, SessionId},
    protocol::AgentHostResponse,
};
use arcweft_core::engine::FlowFiberStatus;
use arcweft_debug_model::sink::DebugEventSink;

use crate::error::AgentRunError;
use crate::session::{AgentSession, RagService};

/// Runner configuration that must remain deterministic under replay.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentRunnerConfig {
    pub session_id: SessionId,
    pub run_id: Option<AgentRunId>,
    pub created_unix_ms: i64,
}

/// Deterministic controller-bytecode execution limits.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AgentControllerRunConfig {
    pub max_steps: usize,
    pub max_ops_per_step: usize,
}

/// Host-call execution report for the current vertical slice.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentHostCallReport {
    pub response: AgentHostResponse,
    pub events_emitted: u64,
}

/// Summary returned after running one Agent controller bytecode program.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentControllerRunReport {
    pub steps: usize,
    pub host_calls: usize,
    pub responses: Vec<AgentHostResponse>,
    pub events_emitted: u64,
    pub final_status: Option<FlowFiberStatus>,
}
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

impl Default for AgentControllerRunConfig {
    fn default() -> Self {
        Self {
            max_steps: 256,
            max_ops_per_step: 1024,
        }
    }
}
