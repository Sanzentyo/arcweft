//! MCP-facing typed Agent REPL command surface.
//!
//! The endpoint is Sans I/O: it borrows an existing REPL session, optional
//! command host, and optional runtime task owner supplied by the concrete MCP
//! host. It does not create task registries or parse formatted terminal text.

use arcweft_agent_repl::ReplSession;
use arcweft_agent_repl::command::{
    ReplBackgroundRequestSink, ReplCommandEndpoint, ReplCommandEndpointRequest,
    ReplCommandEndpointTracePolicy, ReplCommandHandler, ReplCommandHost, ReplCommandId,
    ReplCommandJsonOptions, ReplCommandResult, ReplCommandStatus, ReplProjectLoader,
    ReplTracePolicy, repl_command_result_json,
};
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
    endpoint: ReplCommandEndpoint<'a>,
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

    fn endpoint_request(&self) -> ReplCommandEndpointRequest {
        ReplCommandEndpointRequest {
            input: self.input.clone(),
            command_id: self.repl_command_id(),
            trace_policy: self.trace_policy.into(),
        }
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

impl From<McpReplTracePolicy> for ReplCommandEndpointTracePolicy {
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
            endpoint: ReplCommandEndpoint::new(session, handler).with_cell_execution_message(
                "MCP REPL command endpoint accepts meta-commands; cell execution requires an Agent REPL evaluation runtime",
            ),
        }
    }

    #[must_use]
    pub fn with_host(mut self, host: &'a mut dyn ReplCommandHost) -> Self {
        self.endpoint = self.endpoint.with_host(host);
        self
    }

    #[must_use]
    pub fn with_runtime_tasks(mut self, runtime_tasks: &'a mut dyn RuntimeTaskOwner) -> Self {
        self.endpoint = self.endpoint.with_runtime_tasks(runtime_tasks);
        self
    }

    #[must_use]
    pub fn with_loader(mut self, loader: &'a mut dyn ReplProjectLoader) -> Self {
        self.endpoint = self.endpoint.with_loader(loader);
        self
    }

    #[must_use]
    pub fn with_background(mut self, background: &'a mut dyn ReplBackgroundRequestSink) -> Self {
        self.endpoint = self.endpoint.with_background(background);
        self
    }

    /// Execute one typed request and return an MCP tool result containing JSON text.
    #[must_use]
    pub fn execute(&mut self, request: &McpReplCommandRequest) -> McpCallToolResult {
        let result = self.endpoint.result(&request.endpoint_request());
        Self::tool_result(&result, request)
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

#[cfg(test)]
mod tests {
    use arcweft_agent_repl::command::{ReplCommandEvidence, ReplHelpEvidence};

    use super::*;

    #[test]
    fn repl_command_mcp_tool_result_uses_structured_json_text() {
        let request = McpReplCommandRequest {
            input: ":help".to_owned(),
            command_id: 5,
            trace_policy: McpReplTracePolicy::ReadWrite,
            max_items: 8,
            max_string_bytes: 64,
            include_diagnostics: true,
        };
        let result = ReplCommandResult::ok(
            ReplCommandId::new(5),
            ReplCommandEvidence::Help(ReplHelpEvidence {
                topic: None,
                commands: vec![":observe", ":tasks"],
            }),
        );
        let tool = McpReplCommandEndpoint::tool_result(&result, &request);
        assert!(!tool.is_error);
        let McpContentBlock::Text { text } = &tool.content[0] else {
            panic!("REPL command tool result must return JSON text");
        };
        let json: serde_json::Value = serde_json::from_str(text).expect("json text parses");
        assert_eq!(json["command_id"], 5);
        assert_eq!(json["evidence"]["kind"], "help");
        assert!(json.get("formatted_text").is_none());
    }

    #[test]
    fn repl_command_mcp_trace_policy_conversion_is_typed() {
        assert_eq!(
            ReplTracePolicy::from(McpReplTracePolicy::ReadOnlyTrace),
            ReplTracePolicy::ReadOnlyTrace
        );
        assert_eq!(
            ReplCommandEndpointTracePolicy::from(McpReplTracePolicy::ReadOnlyTrace),
            ReplCommandEndpointTracePolicy::ReadOnlyTrace
        );
    }
}
