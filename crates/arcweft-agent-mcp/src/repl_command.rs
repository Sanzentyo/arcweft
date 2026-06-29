//! MCP-facing typed Agent REPL command surface.
//!
//! The endpoint is Sans I/O: it borrows an existing REPL session, optional
//! command host, and optional runtime task owner supplied by the concrete MCP
//! host. It does not create task registries or parse formatted terminal text.

use arcweft_agent_repl::command::{
    ReplBackgroundRequestSink, ReplCommand, ReplCommandContext, ReplCommandDiagnostic,
    ReplCommandDiagnosticCode, ReplCommandEvidence, ReplCommandHandler, ReplCommandHost,
    ReplCommandId, ReplCommandJsonOptions, ReplCommandResult, ReplCommandStatus, ReplInput,
    ReplProjectLoader, ReplTracePolicy, RuntimeTaskReplCommandHost, parse_repl_input,
    repl_command_result_json,
};
use arcweft_agent_repl::{ReplCellInput, ReplSession};
use arcweft_runtime_driver::task::RuntimeTaskOwner;
use serde::{Deserialize, Serialize};

use crate::model::{McpCallToolResult, McpContentBlock};

/// Stable MCP tool name for the typed REPL command endpoint.
pub const MCP_REPL_COMMAND_TOOL: &str = "arcweft.repl.command";

/// MCP request body for `arcweft.repl.command`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct McpReplCommandRequest {
    /// Raw REPL input parsed through `parse_repl_input`.
    pub input: String,
    /// Optional stable command id; defaults to 1 for single-shot tool calls.
    #[serde(default = "default_command_id")]
    pub command_id: u64,
    /// Explicit trace policy for this protocol request.
    #[serde(default)]
    pub trace_policy: McpReplTracePolicy,
    /// JSON projection item bound.
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    /// JSON projection string byte bound.
    #[serde(default = "default_max_string_bytes")]
    pub max_string_bytes: usize,
    /// Include typed diagnostics in the protocol JSON.
    #[serde(default = "default_true")]
    pub include_diagnostics: bool,
}

/// Wire representation of REPL trace policy.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum McpReplTracePolicy {
    #[default]
    ReadWrite,
    ReadOnlyTrace,
}

/// Borrowed execution surface used by a concrete MCP host.
pub struct McpReplCommandEndpoint<'a> {
    session: &'a mut ReplSession,
    handler: &'a mut dyn ReplCommandHandler,
    host: Option<&'a mut dyn ReplCommandHost>,
    runtime_tasks: Option<&'a mut dyn RuntimeTaskOwner>,
    loader: Option<&'a mut dyn ReplProjectLoader>,
    background: Option<&'a mut dyn ReplBackgroundRequestSink>,
}

impl McpReplCommandRequest {
    /// Decode an MCP `arguments` object into the typed request.
    pub fn from_arguments(arguments: &serde_json::Value) -> Result<Self, serde_json::Error> {
        serde_json::from_value(arguments.clone())
    }

    fn json_options(&self) -> ReplCommandJsonOptions {
        ReplCommandJsonOptions {
            max_items: self.max_items,
            max_string_bytes: self.max_string_bytes,
            include_diagnostics: self.include_diagnostics,
        }
    }

    fn repl_command_id(&self) -> ReplCommandId {
        ReplCommandId::new(self.command_id.max(1))
    }
}

impl From<McpReplTracePolicy> for ReplTracePolicy {
    fn from(value: McpReplTracePolicy) -> Self {
        match value {
            McpReplTracePolicy::ReadWrite => Self::ReadWrite,
            McpReplTracePolicy::ReadOnlyTrace => Self::ReadOnlyTrace,
        }
    }
}

impl<'a> McpReplCommandEndpoint<'a> {
    #[must_use]
    pub fn new(session: &'a mut ReplSession, handler: &'a mut dyn ReplCommandHandler) -> Self {
        Self {
            session,
            handler,
            host: None,
            runtime_tasks: None,
            loader: None,
            background: None,
        }
    }

    #[must_use]
    pub fn with_host(mut self, host: &'a mut dyn ReplCommandHost) -> Self {
        self.host = Some(host);
        self
    }

    #[must_use]
    pub fn with_runtime_tasks(mut self, runtime_tasks: &'a mut dyn RuntimeTaskOwner) -> Self {
        self.runtime_tasks = Some(runtime_tasks);
        self
    }

    #[must_use]
    pub fn with_loader(mut self, loader: &'a mut dyn ReplProjectLoader) -> Self {
        self.loader = Some(loader);
        self
    }

    #[must_use]
    pub fn with_background(mut self, background: &'a mut dyn ReplBackgroundRequestSink) -> Self {
        self.background = Some(background);
        self
    }

    /// Execute one typed request and return an MCP tool result containing JSON text.
    #[must_use]
    pub fn execute(&mut self, request: &McpReplCommandRequest) -> McpCallToolResult {
        let result = self.result(request);
        Self::tool_result(&result, request)
    }

    fn result(&mut self, request: &McpReplCommandRequest) -> ReplCommandResult {
        let command_id = request.repl_command_id();
        match parse_repl_input(&request.input) {
            Ok(ReplInput::Empty) => ReplCommandResult::ok(command_id, ReplCommandEvidence::Empty),
            Ok(ReplInput::Cell(cell)) => self.cell_result(command_id, request.trace_policy, &cell),
            Ok(ReplInput::Command(command)) => {
                self.command_result(command_id, request.trace_policy, command)
            }
            Err(error) => ReplCommandResult::error(
                command_id,
                ReplCommandEvidence::Empty,
                error.into_diagnostic(),
            ),
        }
    }

    fn cell_result(
        &mut self,
        command_id: ReplCommandId,
        trace_policy: McpReplTracePolicy,
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
                "MCP REPL command endpoint accepts meta-commands; cell execution requires an Agent REPL evaluation runtime",
            ),
        )
    }

    fn command_result(
        &mut self,
        command_id: ReplCommandId,
        trace_policy: McpReplTracePolicy,
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

    fn tool_result(
        result: &ReplCommandResult,
        request: &McpReplCommandRequest,
    ) -> McpCallToolResult {
        let json = repl_command_result_json(result, &request.json_options());
        let text = serde_json::to_string(&json).unwrap_or_else(|error| {
            serde_json::json!({ "formatter_error": error.to_string() }).to_string()
        });
        McpCallToolResult {
            content: vec![McpContentBlock::Text { text }],
            is_error: matches!(
                result.status,
                ReplCommandStatus::Rejected | ReplCommandStatus::Error
            ),
        }
    }
}

fn default_command_id() -> u64 {
    1
}

fn default_max_items() -> usize {
    ReplCommandJsonOptions::default().max_items
}

fn default_max_string_bytes() -> usize {
    ReplCommandJsonOptions::default().max_string_bytes
}

fn default_true() -> bool {
    true
}
