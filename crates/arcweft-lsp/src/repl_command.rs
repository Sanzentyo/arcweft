//! LSP-facing typed Agent REPL command-result adapter.
//!
//! The stdio LSP session remains document-only. A native runtime/debug host can
//! provide an executor that borrows an existing `ReplSession`, command host, and
//! optional runtime task owner for a single `arcweft/replCommand` request.

use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandDiagnosticSeverity,
    ReplCommandEndpoint, ReplCommandEndpointRequest, ReplCommandEndpointTracePolicy,
    ReplCommandEvidence, ReplCommandId, ReplCommandJsonOptions, ReplCommandResult,
    ReplCommandStatus, ReplTracePolicy, repl_command_result_json,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Stable LSP custom request method for Agent REPL meta-command execution.
pub const LSP_REPL_COMMAND_METHOD: &str = "arcweft/replCommand";

/// Request params for `arcweft/replCommand`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LspReplCommandRequest {
    /// Raw REPL input. Meta-commands are executed; cells are explicitly out of scope.
    pub input: String,
    /// Optional stable command id; defaults to 1 for single-shot LSP requests.
    #[serde(default = "default_command_id")]
    pub command_id: u64,
    /// Explicit trace policy for this request.
    #[serde(default)]
    pub trace_policy: LspReplTracePolicy,
    /// JSON projection item bound.
    #[serde(default = "default_max_items")]
    pub max_items: usize,
    /// JSON projection string byte bound.
    #[serde(default = "default_max_string_bytes")]
    pub max_string_bytes: usize,
    /// Include typed command diagnostics in the protocol JSON projection.
    #[serde(default = "default_true")]
    pub include_diagnostics: bool,
}

/// Wire representation of the REPL trace policy.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LspReplTracePolicy {
    #[default]
    ReadWrite,
    ReadOnlyTrace,
}

/// Response result for `arcweft/replCommand`.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct LspReplCommandResponse {
    /// Shared structured `ReplCommandResult` JSON projection.
    pub result: Value,
    /// Request-scoped diagnostics mirrored from `ReplCommandDiagnostic`.
    pub diagnostics: Vec<LspReplCommandDiagnostic>,
    /// True when the underlying command status is rejected or error.
    pub is_error: bool,
}

/// LSP protocol mirror of one REPL command diagnostic.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LspReplCommandDiagnostic {
    pub severity: LspReplCommandDiagnosticSeverity,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

/// LSP protocol mirror of REPL command diagnostic severity.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LspReplCommandDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Executor borrowed by `ArcweftLspSession` for a single REPL command request.
pub trait LspReplCommandExecutor {
    fn execute_repl_command(&mut self, request: LspReplCommandRequest) -> LspReplCommandResponse;
}

/// LSP wrapper around the shared borrowed REPL command endpoint.
pub struct LspReplCommandEndpoint<'a> {
    endpoint: ReplCommandEndpoint<'a>,
}

impl LspReplCommandRequest {
    #[must_use]
    pub fn json_options(&self) -> ReplCommandJsonOptions {
        ReplCommandJsonOptions {
            max_items: self.max_items,
            max_string_bytes: self.max_string_bytes,
            include_diagnostics: self.include_diagnostics,
        }
    }

    #[must_use]
    pub fn repl_command_id(&self) -> ReplCommandId {
        ReplCommandId::new(self.command_id.max(1))
    }

    #[must_use]
    pub fn endpoint_request(&self) -> ReplCommandEndpointRequest {
        ReplCommandEndpointRequest {
            input: self.input.clone(),
            command_id: self.repl_command_id(),
            trace_policy: self.trace_policy.into(),
        }
    }
}

impl From<LspReplTracePolicy> for ReplTracePolicy {
    fn from(value: LspReplTracePolicy) -> Self {
        match value {
            LspReplTracePolicy::ReadWrite => Self::ReadWrite,
            LspReplTracePolicy::ReadOnlyTrace => Self::ReadOnlyTrace,
        }
    }
}

impl From<LspReplTracePolicy> for ReplCommandEndpointTracePolicy {
    fn from(value: LspReplTracePolicy) -> Self {
        match value {
            LspReplTracePolicy::ReadWrite => Self::ReadWrite,
            LspReplTracePolicy::ReadOnlyTrace => Self::ReadOnlyTrace,
        }
    }
}

impl LspReplCommandResponse {
    #[must_use]
    pub fn from_result(result: &ReplCommandResult, options: &ReplCommandJsonOptions) -> Self {
        Self {
            result: repl_command_result_json(result, options),
            diagnostics: result
                .diagnostics
                .iter()
                .map(LspReplCommandDiagnostic::from)
                .collect(),
            is_error: matches!(
                result.status,
                ReplCommandStatus::Rejected | ReplCommandStatus::Error
            ),
        }
    }

    #[must_use]
    pub fn host_unavailable(request: &LspReplCommandRequest) -> Self {
        let result = ReplCommandResult::error(
            request.repl_command_id(),
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostUnavailable,
                "arcweft/replCommand requires a native runtime/debug host that borrows a ReplSession; the document-only stdio LSP session does not own REPL state",
            ),
        );
        Self::from_result(&result, &request.json_options())
    }
}

impl From<&ReplCommandDiagnostic> for LspReplCommandDiagnostic {
    fn from(value: &ReplCommandDiagnostic) -> Self {
        Self {
            severity: LspReplCommandDiagnosticSeverity::from(value.severity),
            code: value.code.as_str().to_owned(),
            message: value.message.clone(),
            field: value.field.clone(),
        }
    }
}

impl From<ReplCommandDiagnosticSeverity> for LspReplCommandDiagnosticSeverity {
    fn from(value: ReplCommandDiagnosticSeverity) -> Self {
        match value {
            ReplCommandDiagnosticSeverity::Info => Self::Info,
            ReplCommandDiagnosticSeverity::Warning => Self::Warning,
            ReplCommandDiagnosticSeverity::Error => Self::Error,
        }
    }
}

impl<'a> LspReplCommandEndpoint<'a> {
    #[must_use]
    pub fn new(endpoint: ReplCommandEndpoint<'a>) -> Self {
        Self {
            endpoint: endpoint.with_cell_execution_message(
                "LSP REPL command endpoint accepts meta-commands; cell execution requires an Agent REPL evaluation runtime",
            ),
        }
    }

    pub fn endpoint_mut(&mut self) -> &mut ReplCommandEndpoint<'a> {
        &mut self.endpoint
    }
}

impl LspReplCommandExecutor for LspReplCommandEndpoint<'_> {
    fn execute_repl_command(&mut self, request: LspReplCommandRequest) -> LspReplCommandResponse {
        let result = self.endpoint.result(&request.endpoint_request());
        LspReplCommandResponse::from_result(&result, &request.json_options())
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
    use serde_json::json;

    use super::*;

    #[test]
    fn repl_command_request_defaults_are_stable() {
        let request: LspReplCommandRequest =
            serde_json::from_value(json!({ "input": ":help" })).expect("request decodes");

        assert_eq!(request.command_id, 1);
        assert_eq!(request.trace_policy, LspReplTracePolicy::ReadWrite);
        assert_eq!(
            request.max_items,
            ReplCommandJsonOptions::default().max_items
        );
        assert_eq!(
            request.max_string_bytes,
            ReplCommandJsonOptions::default().max_string_bytes
        );
        assert!(request.include_diagnostics);
    }

    #[test]
    fn host_unavailable_response_mirrors_repl_diagnostics() {
        let request = LspReplCommandRequest {
            input: ":tasks".to_owned(),
            command_id: 22,
            trace_policy: LspReplTracePolicy::ReadWrite,
            max_items: 8,
            max_string_bytes: 64,
            include_diagnostics: true,
        };

        let response = LspReplCommandResponse::host_unavailable(&request);

        assert!(response.is_error);
        assert_eq!(response.result["status"], json!("error"));
        assert_eq!(response.result["command_id"], json!(22));
        assert_eq!(response.result["evidence"]["kind"], json!("empty"));
        assert_eq!(
            response.result["diagnostics"][0]["code"],
            json!("host_unavailable")
        );
        assert_eq!(response.diagnostics.len(), 1);
        assert_eq!(response.diagnostics[0].code, "host_unavailable");
        assert_eq!(
            response.diagnostics[0].severity,
            LspReplCommandDiagnosticSeverity::Error
        );
        assert!(response.result.get("formatted_text").is_none());
    }
}
