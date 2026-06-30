//! Shared protocol-facing typed Agent REPL command endpoint.
//!
//! MCP and LSP adapters use this borrowed endpoint instead of duplicating
//! command dispatch, synthesizing task registries, or reparsing CLI-formatted
//! terminal output. Concrete hosts decide which `ReplSession`, command host,
//! runtime task owner, loader, and background sink are available for one
//! protocol request.

use arcweft_runtime_driver::task::RuntimeTaskOwner;

use crate::{ReplCellInput, ReplSession};

use super::dispatch::{ReplBackgroundRequestSink, ReplCommandContext, ReplCommandHandler};
use super::host::{ReplCommandHost, ReplProjectLoader};
use super::parse::parse_repl_input;
use super::runtime_task::RuntimeTaskReplCommandHost;
use super::types::{
    ReplCommand, ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandEvidence,
    ReplCommandId, ReplCommandResult, ReplInput, ReplTracePolicy,
};

/// Borrowed protocol request after a transport-specific DTO has been decoded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCommandEndpointRequest {
    /// Raw REPL input parsed through the typed `parse_repl_input` boundary.
    pub input: String,
    /// Stable command id used in deterministic command-result evidence.
    pub command_id: ReplCommandId,
    /// Trace policy chosen by the protocol request.
    pub trace_policy: ReplCommandEndpointTracePolicy,
}

/// Transport-neutral trace policy for protocol command endpoints.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReplCommandEndpointTracePolicy {
    /// Commands and meta-command mutations are allowed.
    #[default]
    ReadWrite,
    /// Read-only trace replay: session mutations and cell execution are rejected.
    ReadOnlyTrace,
}

/// Borrowed REPL command endpoint shared by protocol adapters.
pub struct ReplCommandEndpoint<'a> {
    session: &'a mut ReplSession,
    handler: &'a mut dyn ReplCommandHandler,
    host: Option<&'a mut dyn ReplCommandHost>,
    runtime_tasks: Option<&'a mut dyn RuntimeTaskOwner>,
    loader: Option<&'a mut dyn ReplProjectLoader>,
    background: Option<&'a mut dyn ReplBackgroundRequestSink>,
    cell_execution_message: &'static str,
}

impl ReplCommandEndpointRequest {
    /// Creates a request with read-write trace policy.
    #[must_use]
    pub fn new(input: impl Into<String>, command_id: ReplCommandId) -> Self {
        Self {
            input: input.into(),
            command_id,
            trace_policy: ReplCommandEndpointTracePolicy::ReadWrite,
        }
    }

    /// Selects the request trace policy.
    #[must_use]
    pub const fn with_trace_policy(mut self, trace_policy: ReplCommandEndpointTracePolicy) -> Self {
        self.trace_policy = trace_policy;
        self
    }
}

impl From<ReplCommandEndpointTracePolicy> for ReplTracePolicy {
    fn from(value: ReplCommandEndpointTracePolicy) -> Self {
        match value {
            ReplCommandEndpointTracePolicy::ReadWrite => Self::ReadWrite,
            ReplCommandEndpointTracePolicy::ReadOnlyTrace => Self::ReadOnlyTrace,
        }
    }
}

impl<'a> ReplCommandEndpoint<'a> {
    /// Creates a borrowed endpoint for one protocol request scope.
    #[must_use]
    pub fn new(session: &'a mut ReplSession, handler: &'a mut dyn ReplCommandHandler) -> Self {
        Self {
            session,
            handler,
            host: None,
            runtime_tasks: None,
            loader: None,
            background: None,
            cell_execution_message: "REPL command protocol endpoints accept meta-commands; cell execution requires an Agent REPL evaluation runtime",
        }
    }

    /// Supplies an existing command host. The endpoint does not own it.
    #[must_use]
    pub fn with_host(mut self, host: &'a mut dyn ReplCommandHost) -> Self {
        self.host = Some(host);
        self
    }

    /// Supplies an existing runtime task owner. No global scheduler registry is used.
    #[must_use]
    pub fn with_runtime_tasks(mut self, runtime_tasks: &'a mut dyn RuntimeTaskOwner) -> Self {
        self.runtime_tasks = Some(runtime_tasks);
        self
    }

    /// Supplies an existing project loader for `:load` / `:reload`.
    #[must_use]
    pub fn with_loader(mut self, loader: &'a mut dyn ReplProjectLoader) -> Self {
        self.loader = Some(loader);
        self
    }

    /// Supplies an existing background sink for tiering commands.
    #[must_use]
    pub fn with_background(mut self, background: &'a mut dyn ReplBackgroundRequestSink) -> Self {
        self.background = Some(background);
        self
    }

    /// Overrides the transport-specific diagnostic used for cell submissions.
    #[must_use]
    pub const fn with_cell_execution_message(mut self, message: &'static str) -> Self {
        self.cell_execution_message = message;
        self
    }

    /// Executes one request and returns transport-neutral typed result evidence.
    #[must_use]
    pub fn result(&mut self, request: &ReplCommandEndpointRequest) -> ReplCommandResult {
        match parse_repl_input(&request.input) {
            Ok(ReplInput::Empty) => {
                ReplCommandResult::ok(request.command_id, ReplCommandEvidence::Empty)
            }
            Ok(ReplInput::Cell(cell)) => {
                self.cell_result(request.command_id, request.trace_policy, &cell)
            }
            Ok(ReplInput::Command(command)) => {
                self.command_result(request.command_id, request.trace_policy, command)
            }
            Err(error) => ReplCommandResult::error(
                request.command_id,
                ReplCommandEvidence::Empty,
                error.into_diagnostic(),
            ),
        }
    }

    fn cell_result(
        &mut self,
        command_id: ReplCommandId,
        trace_policy: ReplCommandEndpointTracePolicy,
        cell: &ReplCellInput,
    ) -> ReplCommandResult {
        let mut context = ReplCommandContext::new(&mut *self.session)
            .with_next_command_id(command_id)
            .with_trace_policy(trace_policy.into());
        if let Some(result) = context.reject_cell_submission_if_read_only(cell) {
            return result;
        }
        ReplCommandResult::error(
            command_id,
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::UnhandledExtension,
                self.cell_execution_message,
            ),
        )
    }

    fn command_result(
        &mut self,
        command_id: ReplCommandId,
        trace_policy: ReplCommandEndpointTracePolicy,
        command: ReplCommand,
    ) -> ReplCommandResult {
        let mut context = ReplCommandContext::new(&mut *self.session)
            .with_next_command_id(command_id)
            .with_trace_policy(trace_policy.into());
        if let Some(loader) = self.loader.as_deref_mut() {
            context = context.with_loader(loader);
        }
        if let Some(background) = self.background.as_deref_mut() {
            context = context.with_background(background);
        }

        match (self.host.as_deref_mut(), self.runtime_tasks.as_deref_mut()) {
            (Some(host), Some(runtime_tasks)) => {
                let mut task_host = RuntimeTaskReplCommandHost::new(host, runtime_tasks);
                let mut context = context.with_host(&mut task_host);
                self.handler.handle(&mut context, command)
            }
            (Some(host), None) => {
                let mut context = context.with_host(host);
                self.handler.handle(&mut context, command)
            }
            (None, Some(_) | None) => self.handler.handle(&mut context, command),
        }
    }
}
