use arcweft_agent_protocol::protocol::{AgentSessionInfo, ObservationEnvelope};
use arcweft_agent_runner::session::AgentSession;
use thiserror::Error;

use crate::ReplBaseSnapshot;

use super::types::{
    CancelCommand, LoadCommand, ObserveCommand, ReloadCommand, ReplCancelOutcome,
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplTaskList, StepCommand, TasksCommand,
};

/// Host/session integration boundary for user-visible REPL commands.
pub trait ReplCommandHost {
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo>;

    fn observe(&mut self, command: &ObserveCommand) -> ReplCommandHostResult<ObservationEnvelope>;

    fn step(&mut self, command: &StepCommand) -> ReplCommandHostResult<ObservationEnvelope>;

    fn tasks(&mut self, _command: &TasksCommand) -> ReplCommandHostResult<ReplTaskList> {
        Err(ReplCommandHostError::unsupported(
            "task inspection is not available for this host adapter",
        ))
    }

    fn cancel(&mut self, _command: &CancelCommand) -> ReplCommandHostResult<ReplCancelOutcome> {
        Err(ReplCommandHostError::unsupported(
            "task cancellation is not available for this host adapter",
        ))
    }
}

/// Project-loading adapter. Implementations live in CLI/MCP/LSP hosts because
/// file IO and project compilation are not command-state ownership.
pub trait ReplProjectLoader {
    fn load(&mut self, command: &LoadCommand) -> ReplCommandHostResult<ReplBaseSnapshot>;

    fn reload(&mut self, command: &ReloadCommand) -> ReplCommandHostResult<ReplBaseSnapshot>;
}

/// Adapter from the existing `AgentSession` trait to the command host boundary.
pub struct AgentSessionReplCommandHost<'a, S>
where
    S: AgentSession,
{
    session: &'a mut S,
}

/// Error returned by a host adapter.
pub type ReplCommandHostResult<T> = Result<T, ReplCommandHostError>;

/// Stable host/loader error shape.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ReplCommandHostError {
    pub code: ReplCommandDiagnosticCode,
    pub message: String,
}

impl<'a, S> AgentSessionReplCommandHost<'a, S>
where
    S: AgentSession,
{
    #[must_use]
    pub fn new(session: &'a mut S) -> Self {
        Self { session }
    }
}

impl<S> ReplCommandHost for AgentSessionReplCommandHost<'_, S>
where
    S: AgentSession,
{
    fn session_info(&mut self) -> ReplCommandHostResult<AgentSessionInfo> {
        self.session
            .info()
            .map_err(ReplCommandHostError::from_error)
    }

    fn observe(&mut self, command: &ObserveCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        self.session
            .observe(command.request.clone())
            .map_err(ReplCommandHostError::from_error)
    }

    fn step(&mut self, command: &StepCommand) -> ReplCommandHostResult<ObservationEnvelope> {
        self.session
            .step_frames(command.frames)
            .map_err(ReplCommandHostError::from_error)
    }
}

impl ReplCommandHostError {
    #[must_use]
    pub fn new(code: ReplCommandDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::new(ReplCommandDiagnosticCode::HostUnavailable, message)
    }

    #[must_use]
    pub fn from_error(error: impl std::error::Error) -> Self {
        Self::new(ReplCommandDiagnosticCode::HostError, error.to_string())
    }

    #[must_use]
    pub fn into_diagnostic(self) -> ReplCommandDiagnostic {
        ReplCommandDiagnostic::error(self.code, self.message)
    }
}
