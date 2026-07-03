use arcweft_agent_repl::command::{
    ReplBackgroundQueuedEvidence, ReplBackgroundRequest, ReplCancelOutcome, ReplCancelTarget,
    ReplCommandDiagnostic, ReplCommandDiagnosticSeverity, ReplCommandEvidence,
    ReplCommandJsonOptions, ReplCommandResult, ReplCommandStatus, ReplCommandTarget,
    ReplGenerationCommandEvidence, ReplHelpEvidence, ReplResetEvidence, ReplTaskRecord,
    ReplTaskStatus, ReplTasksEvidence, ReplTracePolicy, ReplUndoEvidence, repl_command_result_json,
};
use arcweft_agent_repl::{
    ReplBaseChangeOutcome, ReplCapabilityReport, ReplCellExecutionStatus, ReplCellId, ReplCellList,
    ReplCellRecord, ReplCodegenStatus, ReplGenerationId, ReplWarmOutcome,
    ReplWarmUnsupportedReason,
};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) enum ReplCommandFormatMode {
    #[default]
    Human,
    Json,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ReplCommandFormatOptions {
    pub(super) mode: ReplCommandFormatMode,
    pub(super) max_items: usize,
    pub(super) max_string_bytes: usize,
    pub(super) include_diagnostics: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ReplCommandFormattedOutput {
    pub(super) text: String,
    pub(super) json: Value,
}

pub(super) trait ReplCommandResultFormatter {
    fn format_result(
        &self,
        result: &ReplCommandResult,
        options: &ReplCommandFormatOptions,
    ) -> ReplCommandFormattedOutput;
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct CliReplCommandFormatter;

impl Default for ReplCommandFormatOptions {
    fn default() -> Self {
        Self {
            mode: ReplCommandFormatMode::Human,
            max_items: 32,
            max_string_bytes: 240,
            include_diagnostics: true,
        }
    }
}

impl ReplCommandResultFormatter for CliReplCommandFormatter {
    fn format_result(
        &self,
        result: &ReplCommandResult,
        options: &ReplCommandFormatOptions,
    ) -> ReplCommandFormattedOutput {
        let json = result_json(result, options);
        let text = match options.mode {
            ReplCommandFormatMode::Human => human_result(result, options),
            ReplCommandFormatMode::Json => serde_json::to_string(&json).unwrap_or_else(|error| {
                json!({ "formatter_error": error.to_string() }).to_string()
            }),
        };
        ReplCommandFormattedOutput { text, json }
    }
}

fn result_json(result: &ReplCommandResult, options: &ReplCommandFormatOptions) -> Value {
    repl_command_result_json(
        result,
        &ReplCommandJsonOptions {
            max_items: options.max_items,
            max_string_bytes: options.max_string_bytes,
            include_diagnostics: options.include_diagnostics,
        },
    )
}

fn human_result(result: &ReplCommandResult, options: &ReplCommandFormatOptions) -> String {
    let mut lines = vec![human_evidence(
        status_label(result.status),
        &result.evidence,
        options,
    )];
    if options.include_diagnostics {
        lines.extend(result.diagnostics.iter().map(human_diagnostic));
    }
    lines.join("\n")
}

fn human_evidence(
    status: &'static str,
    evidence: &ReplCommandEvidence,
    options: &ReplCommandFormatOptions,
) -> String {
    match evidence {
        ReplCommandEvidence::Empty => format!("{status}: no evidence"),
        ReplCommandEvidence::Observation(value) => format!(
            "{status}: observe tick={} frame={} state={} render={} actions={} signals={}",
            value.tick,
            value.frame_id,
            value.state_hash,
            value.render_hash,
            value.action_count,
            value.signal_count
        ),
        ReplCommandEvidence::Step(value) => format!(
            "{status}: step frames={} tick={} frame={} state={} render={}",
            value.frames,
            value.observation.tick,
            value.observation.frame_id,
            value.observation.state_hash,
            value.observation.render_hash
        ),
        ReplCommandEvidence::Tasks(value) => human_tasks(status, value, options),
        ReplCommandEvidence::Cancel(value) => human_cancel(status, &value.outcome),
        ReplCommandEvidence::Load(value) => format!(
            "{status}: load path={} base={} outcome={}",
            value.path,
            value.base_label,
            base_change_outcome_label(&value.outcome)
        ),
        ReplCommandEvidence::Reload(value) => format!(
            "{status}: reload path={} base={} outcome={}",
            value.path.as_deref().unwrap_or("<current>"),
            value.base_label,
            base_change_outcome_label(&value.outcome)
        ),
        ReplCommandEvidence::Cells(value) => human_cells(status, value, options),
        ReplCommandEvidence::Undo(value) => human_undo(status, value),
        ReplCommandEvidence::Reset(value) => human_reset(status, value),
        ReplCommandEvidence::Capabilities(value) => human_capabilities(status, value),
        ReplCommandEvidence::Generations(value) => human_generations(status, value),
        ReplCommandEvidence::Warm(value) => human_warm(status, value),
        ReplCommandEvidence::Codegen(value) => human_codegen(status, value),
        ReplCommandEvidence::BackgroundQueued(value) => human_background_queued(status, value),
        ReplCommandEvidence::Help(value) => human_help(status, value),
        ReplCommandEvidence::Quit => format!("{status}: quit requested"),
        ReplCommandEvidence::CellSubmissionRejected(value) => format!(
            "{status}: cell submission rejected source_len={} policy={}",
            value.source_len,
            trace_policy_label(value.policy)
        ),
    }
}

fn human_undo(status: &'static str, value: &ReplUndoEvidence) -> String {
    format!(
        "{status}: undo removed={} kind={} bindings_removed={} remaining_cells={} overlay={} bindings_after={} active_generation={} tier_invalidations={}",
        value.summary.removed_cell_id.label(),
        value.summary.removed_cell_kind.as_str(),
        value.summary.removed_binding_count,
        value.summary.remaining_cells,
        value.summary.overlay_hash,
        value.binding_evidence_after.bindings.len(),
        value.generation_evidence_after.active_generation.as_u64(),
        value.tier_invalidations.len()
    )
}

fn human_reset(status: &'static str, value: &ReplResetEvidence) -> String {
    format!(
        "{status}: reset removed_cells={} retained_generation={} overlay={} bindings_after={} tier_invalidations={}",
        value.summary.removed_cells,
        value.summary.retained_generation.as_u64(),
        value.summary.overlay_hash,
        value.binding_evidence_after.bindings.len(),
        value.tier_invalidations.len()
    )
}

fn human_capabilities(status: &'static str, value: &ReplCapabilityReport) -> String {
    format!(
        "{status}: capabilities allowed={}",
        value
            .allowed
            .iter()
            .map(|capability| format!("{capability:?}"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn human_generations(status: &'static str, value: &ReplGenerationCommandEvidence) -> String {
    format!(
        "{status}: generations active={} base={} overlay={} cells={} invalidations={} bindings={} tiers={}",
        value.generation.active_generation.as_u64(),
        value.generation.base_program_hash,
        value.generation.overlay_hash,
        value.generation.committed_cells,
        value.generation.invalidation_events,
        value.bindings.bindings.len(),
        value.tiers.as_ref().map_or(0, |tiers| tiers.records.len())
    )
}

fn human_warm(status: &'static str, value: &ReplWarmOutcome) -> String {
    format!(
        "{status}: warm request={} target={} backend={} fallback={} requested={} background={} generation={} warmed_cells={} invalidated_artifacts={} reason={}",
        value.request_id,
        target_label(&value.target),
        value.backend_status.as_str(),
        value.fallback.as_str(),
        value.requested,
        value.started_background_job,
        value.generation.as_u64(),
        cell_labels(&value.warmed_cells).join(","),
        value.invalidated_artifacts.len(),
        value
            .reason
            .map_or("none", ReplWarmUnsupportedReason::as_str)
    )
}

fn human_codegen(status: &'static str, value: &ReplCodegenStatus) -> String {
    format!(
        "{status}: codegen requested={} backend={} fallback={} enabled_backends={} warmed_generations={} warmed_cells={} pending_jobs={} failures={} invalidated_artifacts={}",
        value.requested,
        value.backend_status.as_str(),
        value.fallback.as_str(),
        value.enabled_backends.len(),
        generation_label_text(&value.warmed_generations),
        cell_labels(&value.warmed_cells).join(","),
        value.pending_jobs.len(),
        value.failures.len(),
        value.invalidated_artifacts.len()
    )
}

fn human_background_queued(status: &'static str, value: &ReplBackgroundQueuedEvidence) -> String {
    format!(
        "{status}: background queued request_id={} kind={}",
        value.request_id.as_u64(),
        background_request_kind(&value.request)
    )
}

fn human_help(status: &'static str, value: &ReplHelpEvidence) -> String {
    format!(
        "{status}: help topic={} commands={}",
        value.topic.as_deref().unwrap_or("all"),
        value.commands.join(",")
    )
}

fn human_tasks(
    status: &'static str,
    value: &ReplTasksEvidence,
    options: &ReplCommandFormatOptions,
) -> String {
    if value.tasks.tasks.is_empty() {
        return format!(
            "{status}: tasks none include_completed={}",
            value.include_completed
        );
    }
    let task_lines = value
        .tasks
        .tasks
        .iter()
        .take(options.max_items)
        .map(human_task)
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if value.tasks.tasks.len() > options.max_items {
        format!("; ... {} more", value.tasks.tasks.len() - options.max_items)
    } else {
        String::new()
    };
    format!(
        "{status}: tasks include_completed={} total={} {task_lines}{suffix}",
        value.include_completed,
        value.tasks.tasks.len()
    )
}

fn human_task(value: &ReplTaskRecord) -> String {
    format!(
        "{} status={} generation={} epoch={} sequence={} scope={}",
        value.id,
        task_status_label(value.status),
        value
            .generation
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        value
            .logical_epoch
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        value
            .sequence
            .map_or_else(|| "-".to_owned(), |value| value.to_string()),
        value.cancel_scope.as_deref().unwrap_or("-")
    )
}

fn human_cancel(status: &'static str, value: &ReplCancelOutcome) -> String {
    format!(
        "{status}: cancel target={} cancelled={} pending_after={}",
        cancel_target_label(&value.target),
        value.cancelled,
        value.pending_after
    )
}

fn human_cells(
    status: &'static str,
    value: &ReplCellList,
    options: &ReplCommandFormatOptions,
) -> String {
    if value.cells.is_empty() {
        return format!("{status}: cells none");
    }
    let cell_lines = value
        .cells
        .iter()
        .take(options.max_items)
        .map(human_cell)
        .collect::<Vec<_>>()
        .join("; ");
    let suffix = if value.cells.len() > options.max_items {
        format!("; ... {} more", value.cells.len() - options.max_items)
    } else {
        String::new()
    };
    format!(
        "{status}: cells total={} {cell_lines}{suffix}",
        value.cells.len()
    )
}

fn human_cell(value: &ReplCellRecord) -> String {
    format!(
        "{} kind={} status={} gen={} source_hash={} bindings={}",
        value.id.label(),
        value.kind.as_str(),
        cell_execution_status_label(value.execution.status),
        value.generation.as_u64(),
        value.source_hash,
        value.bindings.len()
    )
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
        value.message
    )
}

fn cell_labels(values: &[ReplCellId]) -> Vec<String> {
    values.iter().map(|value| value.label()).collect()
}

fn generation_label_text(values: &[ReplGenerationId]) -> String {
    values
        .iter()
        .map(|value| value.as_u64().to_string())
        .collect::<Vec<_>>()
        .join(",")
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

fn task_status_label(value: ReplTaskStatus) -> &'static str {
    match value {
        ReplTaskStatus::Pending => "pending",
        ReplTaskStatus::Running => "running",
        ReplTaskStatus::Completed => "completed",
        ReplTaskStatus::Cancelled => "cancelled",
        ReplTaskStatus::Failed => "failed",
    }
}

fn cell_execution_status_label(value: ReplCellExecutionStatus) -> &'static str {
    match value {
        ReplCellExecutionStatus::PendingExecution => "pending_execution",
        ReplCellExecutionStatus::Executed => "executed",
        ReplCellExecutionStatus::ExecutionFailed => "execution_failed",
        ReplCellExecutionStatus::Invalidated => "invalidated",
    }
}

fn trace_policy_label(value: ReplTracePolicy) -> &'static str {
    match value {
        ReplTracePolicy::ReadWrite => "read_write",
        ReplTracePolicy::ReadOnlyTrace => "read_only_trace",
    }
}

fn target_label(value: &ReplCommandTarget) -> String {
    match value {
        ReplCommandTarget::All => "all".to_owned(),
        ReplCommandTarget::Latest => "latest".to_owned(),
        ReplCommandTarget::Cell(cell_id) => cell_id.label(),
        ReplCommandTarget::Selector(selector) => format!("selector:{selector}"),
    }
}

fn cancel_target_label(value: &ReplCancelTarget) -> String {
    match value {
        ReplCancelTarget::All => "all".to_owned(),
        ReplCancelTarget::Task(id) => format!("task:{id}"),
        ReplCancelTarget::Scope(id) => format!("scope:{id}"),
    }
}

fn background_request_kind(value: &ReplBackgroundRequest) -> &'static str {
    match value {
        ReplBackgroundRequest::Warm { .. } => "warm",
        ReplBackgroundRequest::Codegen { .. } => "codegen",
    }
}

fn base_change_outcome_label(value: &ReplBaseChangeOutcome) -> String {
    format!("{value:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcweft_agent_repl::ReplGenerationEvidence;
    use arcweft_agent_repl::command::{
        ReplBackgroundRequestId, ReplCellSubmissionEvidence, ReplCommandDiagnosticCode,
        ReplCommandId, ReplTaskList,
    };

    fn human_options() -> ReplCommandFormatOptions {
        ReplCommandFormatOptions {
            mode: ReplCommandFormatMode::Human,
            max_items: 4,
            max_string_bytes: 64,
            include_diagnostics: true,
        }
    }

    fn json_options() -> ReplCommandFormatOptions {
        ReplCommandFormatOptions {
            mode: ReplCommandFormatMode::Json,
            ..human_options()
        }
    }

    fn generation() -> ReplGenerationEvidence {
        ReplGenerationEvidence {
            active_generation: ReplGenerationId::new(9),
            base_program_hash: "base.hash".to_owned(),
            overlay_hash: "overlay.hash".to_owned(),
            committed_cells: 2,
            invalidation_events: 1,
        }
    }

    #[test]
    fn repl_command_formatter_formats_help_with_typed_evidence() {
        let result = ReplCommandResult::ok(
            ReplCommandId::new(1),
            ReplCommandEvidence::Help(ReplHelpEvidence {
                topic: None,
                commands: vec![":observe", ":tasks", ":cancel"],
            }),
        );
        let output = CliReplCommandFormatter.format_result(&result, &human_options());
        assert!(output.text.contains(":observe,:tasks,:cancel"));
        assert_eq!(output.json["status"], "ok");
        assert_eq!(output.json["evidence"]["kind"], "help");
    }

    #[test]
    fn repl_command_formatter_preserves_task_json() {
        let result = ReplCommandResult::ok(
            ReplCommandId::new(2),
            ReplCommandEvidence::Tasks(ReplTasksEvidence {
                include_completed: false,
                tasks: ReplTaskList {
                    tasks: vec![ReplTaskRecord {
                        id: "task.alpha".to_owned(),
                        status: ReplTaskStatus::Running,
                        generation: Some(4),
                        logical_epoch: Some(12),
                        sequence: Some(7),
                        cancel_scope: Some("scope.ui".to_owned()),
                    }],
                },
            }),
        );
        let output = CliReplCommandFormatter.format_result(&result, &json_options());
        assert_eq!(output.json["evidence"]["tasks"][0]["id"], "task.alpha");
        assert_eq!(output.json["evidence"]["tasks"][0]["status"], "running");
        assert!(output.text.contains("\"kind\":\"tasks\""));
    }

    #[test]
    fn repl_command_formatter_reports_read_only_rejection_diagnostic() {
        let result = ReplCommandResult::rejected(
            ReplCommandId::new(3),
            ReplCommandEvidence::CellSubmissionRejected(ReplCellSubmissionEvidence {
                source_len: 18,
                policy: ReplTracePolicy::ReadOnlyTrace,
            }),
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                "read-only trace mode does not allow cell execution",
            ),
        );
        let output = CliReplCommandFormatter.format_result(&result, &human_options());
        assert!(output.text.contains("cell submission rejected"));
        assert!(output.text.contains("read_only_trace_rejected"));
        assert_eq!(output.json["status"], "rejected");
    }

    #[test]
    fn repl_command_formatter_formats_background_warm_queue() {
        let result = ReplCommandResult::queued(
            ReplCommandId::new(4),
            ReplCommandEvidence::BackgroundQueued(ReplBackgroundQueuedEvidence {
                request_id: ReplBackgroundRequestId::new(99),
                request: ReplBackgroundRequest::Warm {
                    command: arcweft_agent_repl::command::WarmCommand {
                        target: ReplCommandTarget::Latest,
                    },
                    generation: generation(),
                },
            }),
        );
        let output = CliReplCommandFormatter.format_result(&result, &human_options());
        assert!(output.text.contains("background queued"));
        assert_eq!(output.json["evidence"]["request"]["kind"], "warm");
        assert_eq!(output.json["evidence"]["request_id"], 99);
    }
}
