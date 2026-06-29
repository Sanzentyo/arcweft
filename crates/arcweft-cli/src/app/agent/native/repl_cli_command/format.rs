use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandDiagnosticSeverity, ReplCommandStatus,
};
use serde_json::{Value, json};

use super::types::{
    CliCaptureTarget, CliConnectTarget, CliReplCommandEvidence, CliReplCommandResult,
};

#[derive(Clone, Debug, PartialEq)]
pub(in crate::app::agent::native) struct CliReplCommandFormattedOutput {
    pub(in crate::app::agent::native) text: String,
    pub(in crate::app::agent::native) json: Value,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::app::agent::native) struct CliReplLocalCommandFormatter;

impl CliReplLocalCommandFormatter {
    #[must_use]
    pub(in crate::app::agent::native) fn format_result(
        result: &CliReplCommandResult,
    ) -> CliReplCommandFormattedOutput {
        let json = result_json(result);
        let text = human_result(result);
        CliReplCommandFormattedOutput { text, json }
    }
}

fn result_json(result: &CliReplCommandResult) -> Value {
    json!({
        "command_id": result.command_id.as_u64(),
        "status": status_label(result.status),
        "command": result.command_name,
        "evidence": evidence_json(&result.evidence),
        "diagnostics": result.diagnostics.iter().map(diagnostic_json).collect::<Vec<_>>(),
    })
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

fn human_result(result: &CliReplCommandResult) -> String {
    let mut lines = vec![human_evidence(
        status_label(result.status),
        result.command_name,
        &result.evidence,
    )];
    lines.extend(result.diagnostics.iter().map(human_diagnostic));
    lines.join("\n")
}

fn human_evidence(
    status: &'static str,
    command_name: &'static str,
    evidence: &CliReplCommandEvidence,
) -> String {
    match evidence {
        CliReplCommandEvidence::Empty => format!("{status}: {command_name} no evidence"),
        CliReplCommandEvidence::Trace { value } => format!(
            "{status}: {command_name} loaded={} path={} records={} resources={} read_only={}",
            json_bool(value, "loaded"),
            json_string(value, "path").unwrap_or("<none>"),
            json_usize(value, "record_count"),
            json_usize(value, "resource_count"),
            json_bool(value, "read_only"),
        ),
        CliReplCommandEvidence::Actions { value } => format!(
            "{status}: {command_name} tick={} actions={}",
            json_usize(value, "tick"),
            value
                .get("actions")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        CliReplCommandEvidence::Inspection { kind, source, .. } => {
            format!("{status}: {} source_bytes={}", kind.name(), source.len())
        }
        CliReplCommandEvidence::Capture { target, value } => format!(
            "{status}: {command_name} target={} frame={} images={}",
            target.label(),
            json_string(value, "frame_id").unwrap_or("<none>"),
            value
                .get("images")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        CliReplCommandEvidence::Query { text, .. } => {
            format!("{status}: {command_name} query_bytes={}", text.len())
        }
        CliReplCommandEvidence::Drop { name, value } => format!(
            "{status}: {command_name} name={} kind={}",
            name,
            json_string(value, "binding_kind").unwrap_or("<unknown>"),
        ),
        CliReplCommandEvidence::Save { path, value } => format!(
            "{status}: {command_name} path={} bytes={}",
            path,
            json_usize(value, "bytes"),
        ),
        CliReplCommandEvidence::Connect { target, value } => format!(
            "{status}: {command_name} target={} program_hash={}",
            target.label(),
            json_string(value, "program_hash").unwrap_or("<none>"),
        ),
        CliReplCommandEvidence::Parse { kind, source, .. } => {
            format!("{status}: {} source_bytes={}", kind.name(), source.len())
        }
        CliReplCommandEvidence::Complete {
            source_before_cursor,
            value,
        } => format!(
            "{status}: {command_name} source_bytes={} items={}",
            source_before_cursor.len(),
            value
                .get("items")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        CliReplCommandEvidence::Highlight { source, value } => format!(
            "{status}: {command_name} source_bytes={} tokens={}",
            source.len(),
            value
                .get("tokens")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        CliReplCommandEvidence::History { value } => format!(
            "{status}: {command_name} cells={}",
            value
                .get("cells")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
        CliReplCommandEvidence::Bindings { value } => format!(
            "{status}: {command_name} bindings={}",
            value
                .get("bindings")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        ),
    }
}

fn capture_target_json(target: &CliCaptureTarget) -> Value {
    match target {
        CliCaptureTarget::Viewport => json!({ "kind": "viewport" }),
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

fn human_diagnostic(value: &ReplCommandDiagnostic) -> String {
    let field = value
        .field
        .as_deref()
        .map_or(String::new(), |field| format!(" field={field}"));
    format!(
        "diagnostic[{}:{}]{}: {}",
        diagnostic_severity_label(value.severity),
        value.code.as_str(),
        field,
        value.message,
    )
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

fn json_bool(value: &Value, field: &str) -> bool {
    value.get(field).and_then(Value::as_bool).unwrap_or(false)
}

fn json_usize(value: &Value, field: &str) -> usize {
    value
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_default()
}

fn json_string<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

#[cfg(test)]
mod tests {
    use arcweft_agent_repl::command::{
        ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandId,
    };

    use super::*;

    #[test]
    fn repl_cli_inspection_cli_formatter_preserves_trace_json_without_formatted_text() {
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
        let output = CliReplLocalCommandFormatter::format_result(&result);
        assert!(output.text.contains(":trace"));
        assert_eq!(output.json["command"], ":trace");
        assert_eq!(output.json["evidence"]["kind"], "trace");
        assert!(output.json.get("formatted_text").is_none());
    }

    #[test]
    fn repl_cli_inspection_cli_formatter_reports_read_only_rejection() {
        let result = CliReplCommandResult::rejected(
            ReplCommandId::new(8),
            ":save",
            CliReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                "read-only trace mode rejects mutating CLI command `:save`",
            )
            .with_field(":save"),
        );
        let output = CliReplLocalCommandFormatter::format_result(&result);
        assert!(output.text.contains("read_only_trace_rejected"));
        assert_eq!(output.json["status"], "rejected");
        assert_eq!(output.json["diagnostics"][0]["field"], ":save");
    }
}
