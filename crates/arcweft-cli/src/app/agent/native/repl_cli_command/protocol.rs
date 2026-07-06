use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandDiagnosticSeverity, ReplCommandId,
    ReplCommandStatus, ReplTracePolicy,
};
use serde_json::{Value, json};

use super::types::{
    CliCaptureTarget, CliConnectTarget, CliReplCommand, CliReplCommandEvidence,
    CliReplCommandResult,
};

/// JSON projection options for CLI-local protocol adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::app::agent::native) struct CliReplCommandJsonOptions {
    pub(in crate::app::agent::native) include_diagnostics: bool,
}

impl Default for CliReplCommandJsonOptions {
    fn default() -> Self {
        Self {
            include_diagnostics: true,
        }
    }
}

impl CliReplCommandJsonOptions {
    #[must_use]
    pub(in crate::app::agent::native) const fn new(include_diagnostics: bool) -> Self {
        Self {
            include_diagnostics,
        }
    }
}

/// Converts one CLI-local command result into structured protocol JSON.
#[must_use]
pub(in crate::app::agent::native) fn cli_repl_command_result_json(
    result: &CliReplCommandResult,
    options: CliReplCommandJsonOptions,
) -> Value {
    let diagnostics = if options.include_diagnostics {
        result
            .diagnostics
            .iter()
            .map(diagnostic_json)
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    json!({
        "command_id": result.command_id.as_u64(),
        "status": status_label(result.status),
        "command": result.command_name,
        "evidence": evidence_json(&result.evidence),
        "diagnostics": diagnostics,
    })
}

/// Builds a deterministic protocol result for CLI-local commands that cannot be
/// executed by a generic MCP/LSP REPL command endpoint.
#[must_use]
pub(in crate::app::agent::native) fn cli_repl_protocol_unavailable_result(
    command_id: ReplCommandId,
    command: &CliReplCommand,
    trace_policy: ReplTracePolicy,
) -> CliReplCommandResult {
    let command_name = command.name();
    if !trace_policy.permits_command(command.effect()) {
        return CliReplCommandResult::rejected(
            command_id,
            command_name,
            CliReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                format!("read-only trace mode rejects mutating CLI command `{command_name}`"),
            )
            .with_field(command_name),
        );
    }

    CliReplCommandResult::error(
        command_id,
        command_name,
        CliReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(
            ReplCommandDiagnosticCode::UnhandledExtension,
            format!(
                "CLI-local command `{command_name}` is not executable through MCP/LSP REPL command-result adapters because it depends on CLI process state; use a dedicated protocol tool or the local CLI REPL"
            ),
        )
        .with_field(command_name),
    )
}

fn evidence_json(evidence: &CliReplCommandEvidence) -> Value {
    match evidence {
        CliReplCommandEvidence::Empty => json!({ "kind": "empty" }),
        CliReplCommandEvidence::Trace { value } => json!({
            "kind": "trace",
            "value": value,
        }),
        CliReplCommandEvidence::Actions { value } => json!({
            "kind": "actions",
            "value": value,
        }),
        CliReplCommandEvidence::Inspection {
            kind,
            source,
            value,
        } => json!({
            "kind": "inspection",
            "inspection": kind.as_str(),
            "source": source,
            "value": value,
        }),
        CliReplCommandEvidence::Capture { target, value } => json!({
            "kind": "capture",
            "target": capture_target_json(target),
            "value": value,
        }),
        CliReplCommandEvidence::Query { text, value } => json!({
            "kind": "query",
            "text": text,
            "value": value,
        }),
        CliReplCommandEvidence::Drop { name, value } => json!({
            "kind": "drop",
            "name": name,
            "value": value,
        }),
        CliReplCommandEvidence::Save { path, value } => json!({
            "kind": "save",
            "path": path,
            "value": value,
        }),
        CliReplCommandEvidence::Connect { target, value } => json!({
            "kind": "connect",
            "target": connect_target_json(target),
            "value": value,
        }),
        CliReplCommandEvidence::Parse {
            kind,
            source,
            value,
        } => json!({
            "kind": "parse",
            "parser": kind.as_str(),
            "source": source,
            "value": value,
        }),
        CliReplCommandEvidence::Complete {
            source_before_cursor,
            value,
        } => json!({
            "kind": "complete",
            "source_before_cursor": source_before_cursor,
            "value": value,
        }),
        CliReplCommandEvidence::Highlight { source, value } => json!({
            "kind": "highlight",
            "source": source,
            "value": value,
        }),
        CliReplCommandEvidence::History { value } => json!({
            "kind": "history",
            "value": value,
        }),
        CliReplCommandEvidence::Bindings { value } => json!({
            "kind": "bindings",
            "value": value,
        }),
    }
}

fn capture_target_json(target: &CliCaptureTarget) -> Value {
    match target {
        CliCaptureTarget::Viewport => json!({ "kind": "viewport" }),
        CliCaptureTarget::View { id } => json!({ "kind": "view", "id": id }),
        CliCaptureTarget::Layer { id } => json!({ "kind": "layer", "id": id }),
        CliCaptureTarget::Object { id } => json!({ "kind": "object", "id": id }),
    }
}

fn connect_target_json(target: &CliConnectTarget) -> Value {
    match target {
        CliConnectTarget::Current => json!({ "kind": "current" }),
        CliConnectTarget::Source { path } => json!({ "kind": "source", "path": path }),
        CliConnectTarget::Profile { id, manifest } => json!({
            "kind": "profile",
            "id": id,
            "manifest": manifest,
        }),
        CliConnectTarget::StdioMcp { program, args } => json!({
            "kind": "stdio_mcp",
            "program": program,
            "args": args,
        }),
        CliConnectTarget::Inferred { target } => json!({ "kind": "inferred", "target": target }),
    }
}

fn diagnostic_json(value: &ReplCommandDiagnostic) -> Value {
    json!({
        "severity": diagnostic_severity_label(value.severity),
        "code": value.code.as_str(),
        "message": &value.message,
        "field": &value.field,
    })
}

fn status_label(value: ReplCommandStatus) -> &'static str {
    match value {
        ReplCommandStatus::Ok => "ok",
        ReplCommandStatus::Queued => "queued",
        ReplCommandStatus::Rejected => "rejected",
        ReplCommandStatus::Error => "error",
        ReplCommandStatus::ExitRequested => "exit_requested",
    }
}

fn diagnostic_severity_label(value: ReplCommandDiagnosticSeverity) -> &'static str {
    match value {
        ReplCommandDiagnosticSeverity::Info => "info",
        ReplCommandDiagnosticSeverity::Warning => "warning",
        ReplCommandDiagnosticSeverity::Error => "error",
    }
}

#[cfg(test)]
mod tests {
    use arcweft_agent_repl::command::{ReplCommandDiagnosticCode, ReplCommandId};
    use serde_json::json;

    use super::*;
    use crate::app::agent::native::repl_cli_command::parse::parse_cli_repl_input;

    #[test]
    fn repl_cli_protocol_json_is_typed_without_formatted_text() {
        let result = CliReplCommandResult::ok(
            ReplCommandId::new(7),
            ":trace",
            CliReplCommandEvidence::Trace {
                value: json!({
                    "loaded": true,
                    "path": "trace.arcwx",
                    "record_count": 3,
                    "resource_count": 1,
                    "read_only": true,
                }),
            },
        );
        let json = cli_repl_command_result_json(&result, CliReplCommandJsonOptions::default());
        assert_eq!(json["command"], ":trace");
        assert_eq!(json["evidence"]["kind"], "trace");
        assert!(json.get("formatted_text").is_none());
    }

    #[test]
    fn repl_cli_protocol_json_can_omit_diagnostics() {
        let result = CliReplCommandResult::error(
            ReplCommandId::new(8),
            ":trace",
            CliReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::UnhandledExtension,
                "protocol adapter unavailable",
            ),
        );
        let json = cli_repl_command_result_json(&result, CliReplCommandJsonOptions::new(false));
        assert_eq!(json["status"], "error");
        assert_eq!(json["diagnostics"].as_array().map(Vec::len), Some(0));
    }

    #[test]
    fn repl_cli_protocol_rejects_mutating_command_in_read_only_trace() {
        let command = parse_cli_repl_input(":save out.awfagent").expect("save parses");
        let result = cli_repl_protocol_unavailable_result(
            ReplCommandId::new(9),
            &command,
            ReplTracePolicy::ReadOnlyTrace,
        );
        let json = cli_repl_command_result_json(&result, CliReplCommandJsonOptions::default());
        assert_eq!(json["status"], "rejected");
        assert_eq!(json["command"], ":save");
        assert_eq!(json["diagnostics"][0]["code"], "read_only_trace_rejected");
    }

    #[test]
    fn repl_cli_protocol_reports_unavailable_for_read_only_cli_command() {
        let command = parse_cli_repl_input(":trace").expect("trace parses");
        let result = cli_repl_protocol_unavailable_result(
            ReplCommandId::new(10),
            &command,
            ReplTracePolicy::ReadWrite,
        );
        let json = cli_repl_command_result_json(&result, CliReplCommandJsonOptions::default());
        assert_eq!(json["status"], "error");
        assert_eq!(json["command"], ":trace");
        assert_eq!(json["diagnostics"][0]["code"], "unhandled_extension");
        assert!(json.get("formatted_text").is_none());
    }
}
