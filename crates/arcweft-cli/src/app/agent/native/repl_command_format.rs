use arcweft_agent_repl::command::{
    ReplBackgroundQueuedEvidence, ReplBackgroundRequest, ReplCancelEvidence, ReplCancelOutcome,
    ReplCancelTarget, ReplCellSubmissionEvidence, ReplCommandDiagnostic,
    ReplCommandDiagnosticSeverity, ReplCommandEvidence, ReplCommandResult, ReplCommandStatus,
    ReplCommandTarget, ReplGenerationCommandEvidence, ReplHelpEvidence, ReplLoadEvidence,
    ReplObservationEvidence, ReplReloadEvidence, ReplResetEvidence, ReplStepEvidence,
    ReplTaskRecord, ReplTaskStatus, ReplTasksEvidence, ReplTracePolicy, ReplUndoEvidence,
};
use arcweft_agent_repl::{
    ReplBaseChangeOutcome, ReplBindingEvidence, ReplBindingRecord, ReplBindingStatus,
    ReplBytecodeStats, ReplCapabilityReport, ReplCellExecutionStatus, ReplCellId, ReplCellList,
    ReplCellRecord, ReplCodegenStatus, ReplDebugEventCount, ReplExecutionRecord,
    ReplGenerationEvidence, ReplGenerationId, ReplTierDiagnostic, ReplTierDiagnosticSeverity,
    ReplTierInvalidationReason, ReplTierInvalidationToken, ReplTierStatusProjection,
    ReplTierStatusRecord, ReplWarmOutcome, ReplWarmUnsupportedReason,
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
        "evidence": evidence_json(&result.evidence, options),
        "diagnostics": diagnostics,
    })
}

fn evidence_json(evidence: &ReplCommandEvidence, options: &ReplCommandFormatOptions) -> Value {
    match evidence {
        ReplCommandEvidence::Empty => json!({ "kind": "empty" }),
        ReplCommandEvidence::Observation(value) => observation_json(value),
        ReplCommandEvidence::Step(value) => step_json(value),
        ReplCommandEvidence::Tasks(value) => tasks_json(value, options),
        ReplCommandEvidence::Cancel(value) => cancel_json(value),
        ReplCommandEvidence::Load(value) => load_json(value),
        ReplCommandEvidence::Reload(value) => reload_json(value),
        ReplCommandEvidence::Cells(value) => cells_json(value, options),
        ReplCommandEvidence::Undo(value) => undo_json(value, options),
        ReplCommandEvidence::Reset(value) => reset_json(value, options),
        ReplCommandEvidence::Capabilities(value) => capabilities_json(value),
        ReplCommandEvidence::Generations(value) => generations_json(value, options),
        ReplCommandEvidence::Warm(value) => warm_json(value, options),
        ReplCommandEvidence::Codegen(value) => codegen_json(value, options),
        ReplCommandEvidence::BackgroundQueued(value) => background_queued_json(value),
        ReplCommandEvidence::Help(value) => help_json(value),
        ReplCommandEvidence::Quit => json!({ "kind": "quit" }),
        ReplCommandEvidence::CellSubmissionRejected(value) => cell_submission_rejected_json(value),
    }
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

fn observation_json(value: &ReplObservationEvidence) -> Value {
    json!({
        "kind": "observation",
        "request": {
            "include_images": value.request.include_images,
            "include_objects": value.request.include_objects,
            "include_logs": value.request.include_logs,
        },
        "tick": value.tick,
        "frame_id": &value.frame_id,
        "state_hash": &value.state_hash,
        "render_hash": &value.render_hash,
        "action_count": value.action_count,
        "signal_count": value.signal_count,
    })
}

fn step_json(value: &ReplStepEvidence) -> Value {
    json!({
        "kind": "step",
        "frames": value.frames,
        "observation": observation_json(&value.observation),
    })
}

fn tasks_json(value: &ReplTasksEvidence, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "tasks",
        "include_completed": value.include_completed,
        "tasks": value.tasks.tasks.iter().take(options.max_items).map(task_json).collect::<Vec<_>>(),
        "truncated": value.tasks.tasks.len() > options.max_items,
        "total_tasks": value.tasks.tasks.len(),
    })
}

fn task_json(value: &ReplTaskRecord) -> Value {
    json!({
        "id": &value.id,
        "status": task_status_label(value.status),
        "generation": value.generation,
        "logical_epoch": value.logical_epoch,
        "sequence": value.sequence,
        "cancel_scope": &value.cancel_scope,
    })
}

fn cancel_json(value: &ReplCancelEvidence) -> Value {
    json!({
        "kind": "cancel",
        "outcome": cancel_outcome_json(&value.outcome),
    })
}

fn cancel_outcome_json(value: &ReplCancelOutcome) -> Value {
    json!({
        "target": cancel_target_json(&value.target),
        "cancelled": value.cancelled,
        "pending_after": value.pending_after,
    })
}

fn load_json(value: &ReplLoadEvidence) -> Value {
    json!({
        "kind": "load",
        "path": &value.path,
        "base_label": &value.base_label,
        "outcome": base_change_outcome_json(&value.outcome),
    })
}

fn reload_json(value: &ReplReloadEvidence) -> Value {
    json!({
        "kind": "reload",
        "path": &value.path,
        "base_label": &value.base_label,
        "outcome": base_change_outcome_json(&value.outcome),
    })
}

fn cells_json(value: &ReplCellList, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "cells",
        "cells": value.cells.iter().take(options.max_items).map(|cell| cell_json(cell, options)).collect::<Vec<_>>(),
        "truncated": value.cells.len() > options.max_items,
        "total_cells": value.cells.len(),
    })
}

fn cell_json(value: &ReplCellRecord, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "id": value.id.label(),
        "ordinal": value.ordinal,
        "kind": value.kind.as_str(),
        "source": summarized_string(&value.source, options.max_string_bytes),
        "source_hash": &value.source_hash,
        "synthetic_source_hash": &value.synthetic_source_hash,
        "synthetic_agent_id": &value.synthetic_agent_id,
        "base_program_hash": &value.base_program_hash,
        "generation": value.generation.as_u64(),
        "commit_hash": &value.commit_hash,
        "overlay_hash": &value.overlay_hash,
        "entry_flow": &value.entry_flow,
        "bytecode_stats": bytecode_stats_json(&value.bytecode_stats),
        "verified_effects": &value.verified_effects,
        "bindings": value.bindings.iter().take(options.max_items).map(binding_record_json).collect::<Vec<_>>(),
        "bindings_truncated": value.bindings.len() > options.max_items,
        "execution": execution_json(&value.execution),
    })
}

fn undo_json(value: &ReplUndoEvidence, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "undo",
        "summary": {
            "removed_cell_id": value.summary.removed_cell_id.label(),
            "removed_cell_kind": value.summary.removed_cell_kind.as_str(),
            "removed_source_hash": &value.summary.removed_source_hash,
            "removed_binding_count": value.summary.removed_binding_count,
            "remaining_cells": value.summary.remaining_cells,
            "overlay_hash": &value.summary.overlay_hash,
        },
        "binding_evidence_after": binding_evidence_json(&value.binding_evidence_after, options),
        "generation_evidence_after": generation_evidence_json(&value.generation_evidence_after),
        "tier_invalidations": tier_invalidations_json(&value.tier_invalidations, options),
    })
}

fn reset_json(value: &ReplResetEvidence, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "reset",
        "summary": {
            "removed_cells": value.summary.removed_cells,
            "retained_generation": value.summary.retained_generation.as_u64(),
            "overlay_hash": &value.summary.overlay_hash,
        },
        "binding_evidence_after": binding_evidence_json(&value.binding_evidence_after, options),
        "generation_evidence_after": generation_evidence_json(&value.generation_evidence_after),
        "tier_invalidations": tier_invalidations_json(&value.tier_invalidations, options),
    })
}

fn capabilities_json(value: &ReplCapabilityReport) -> Value {
    json!({
        "kind": "capabilities",
        "allowed": value.allowed.iter().map(|capability| format!("{capability:?}")).collect::<Vec<_>>(),
    })
}

fn generations_json(
    value: &ReplGenerationCommandEvidence,
    options: &ReplCommandFormatOptions,
) -> Value {
    json!({
        "kind": "generations",
        "generation": generation_evidence_json(&value.generation),
        "bindings": binding_evidence_json(&value.bindings, options),
        "tiers": value.tiers.as_ref().map(|tiers| tier_status_json(tiers, options)),
    })
}

fn warm_json(value: &ReplWarmOutcome, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "warm",
        "request_id": value.request_id,
        "requested": value.requested,
        "started_background_job": value.started_background_job,
        "target": command_target_json(&value.target),
        "backend_status": value.backend_status.as_str(),
        "fallback": value.fallback.as_str(),
        "reason": value.reason.map(ReplWarmUnsupportedReason::as_str),
        "generation": value.generation.as_u64(),
        "overlay_hash": &value.overlay_hash,
        "warmed_cells": cell_labels(&value.warmed_cells),
        "warmed_regions": bounded_strings(&value.warmed_regions, options),
        "invalidated_artifacts": bounded_strings(&value.invalidated_artifacts, options),
        "diagnostics": value.diagnostics.iter().take(options.max_items).map(tier_diagnostic_json).collect::<Vec<_>>(),
        "diagnostics_truncated": value.diagnostics.len() > options.max_items,
    })
}

fn codegen_json(value: &ReplCodegenStatus, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "kind": "codegen",
        "requested": value.requested,
        "backend_status": value.backend_status.as_str(),
        "fallback": value.fallback.as_str(),
        "enabled_backends": value.enabled_backends.iter().map(|backend| format!("{backend:?}")).collect::<Vec<_>>(),
        "warmed_generations": generation_labels(&value.warmed_generations),
        "warmed_cells": cell_labels(&value.warmed_cells),
        "warmed_regions": bounded_strings(&value.warmed_regions, options),
        "pending_jobs": value.pending_jobs.iter().take(options.max_items).map(|job| json!({
            "request_id": job.request_id,
            "generation": job.generation.as_u64(),
            "overlay_hash": &job.overlay_hash,
            "cells": cell_labels(&job.cells),
            "status": job.status.as_str(),
        })).collect::<Vec<_>>(),
        "pending_jobs_truncated": value.pending_jobs.len() > options.max_items,
        "failures": value.failures.iter().take(options.max_items).map(tier_diagnostic_json).collect::<Vec<_>>(),
        "failures_truncated": value.failures.len() > options.max_items,
        "invalidated_artifacts": bounded_strings(&value.invalidated_artifacts, options),
        "diagnostics": value.diagnostics.iter().take(options.max_items).map(tier_diagnostic_json).collect::<Vec<_>>(),
        "diagnostics_truncated": value.diagnostics.len() > options.max_items,
    })
}

fn background_queued_json(value: &ReplBackgroundQueuedEvidence) -> Value {
    json!({
        "kind": "background_queued",
        "request_id": value.request_id.as_u64(),
        "request": background_request_json(&value.request),
    })
}

fn help_json(value: &ReplHelpEvidence) -> Value {
    json!({
        "kind": "help",
        "topic": &value.topic,
        "commands": &value.commands,
    })
}

fn cell_submission_rejected_json(value: &ReplCellSubmissionEvidence) -> Value {
    json!({
        "kind": "cell_submission_rejected",
        "source_len": value.source_len,
        "policy": trace_policy_label(value.policy),
    })
}

fn diagnostic_json(value: &ReplCommandDiagnostic) -> Value {
    json!({
        "severity": diagnostic_severity_label(value.severity),
        "code": value.code.as_str(),
        "message": &value.message,
        "field": &value.field,
    })
}

fn tier_diagnostic_json(value: &ReplTierDiagnostic) -> Value {
    json!({
        "severity": tier_diagnostic_severity_label(value.severity),
        "code": value.code.as_str(),
        "message": &value.message,
        "cell_id": value.cell_id.map(ReplCellId::label),
    })
}

fn generation_evidence_json(value: &ReplGenerationEvidence) -> Value {
    json!({
        "active_generation": value.active_generation.as_u64(),
        "base_program_hash": &value.base_program_hash,
        "overlay_hash": &value.overlay_hash,
        "committed_cells": value.committed_cells,
        "invalidation_events": value.invalidation_events,
    })
}

fn binding_evidence_json(value: &ReplBindingEvidence, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "base_program_hash": &value.base_program_hash,
        "generation": value.generation.as_u64(),
        "bindings": value.bindings.iter().take(options.max_items).map(binding_record_json).collect::<Vec<_>>(),
        "truncated": value.bindings.len() > options.max_items,
        "total_bindings": value.bindings.len(),
    })
}

fn binding_record_json(value: &ReplBindingRecord) -> Value {
    json!({
        "name": &value.name,
        "cell_id": value.cell_id.label(),
        "source": &value.source,
        "snapshot_kind": value.snapshot_kind.as_str(),
        "project_bound": value.project_bound,
        "status": binding_status_label(value.status),
        "invalidation": value.invalidated.as_ref().map(|invalidation| json!({
            "reason": &invalidation.reason,
            "old_program_hash": &invalidation.old_program_hash,
            "new_program_hash": &invalidation.new_program_hash,
            "old_generation": invalidation.old_generation.as_u64(),
            "new_generation": invalidation.new_generation.as_u64(),
        })),
    })
}

fn tier_invalidations_json(
    values: &[ReplTierInvalidationToken],
    options: &ReplCommandFormatOptions,
) -> Value {
    json!({
        "items": values.iter().take(options.max_items).map(tier_invalidation_json).collect::<Vec<_>>(),
        "truncated": values.len() > options.max_items,
        "total": values.len(),
    })
}

fn tier_invalidation_json(value: &ReplTierInvalidationToken) -> Value {
    json!({
        "cursor": value.cursor.as_u64(),
        "reason": tier_invalidation_reason_label(value.reason),
        "generation": value.generation.as_u64(),
        "overlay_hash": &value.overlay_hash,
        "cell_id": value.cell_id.map(ReplCellId::label),
        "detail": &value.detail,
    })
}

fn tier_status_json(value: &ReplTierStatusProjection, options: &ReplCommandFormatOptions) -> Value {
    json!({
        "records": value.records.iter().take(options.max_items).map(tier_status_record_json).collect::<Vec<_>>(),
        "truncated": value.records.len() > options.max_items,
        "total_records": value.records.len(),
    })
}

fn tier_status_record_json(value: &ReplTierStatusRecord) -> Value {
    json!({
        "generation": value.generation.as_u64(),
        "overlay_hash": &value.overlay_hash,
        "cell_id": value.cell_id.map(ReplCellId::label),
        "tier": &value.tier,
        "status": &value.status,
        "detail": &value.detail,
    })
}

fn bytecode_stats_json(value: &ReplBytecodeStats) -> Value {
    json!({
        "flows": value.flows,
        "instructions": value.instructions,
        "line_task_groups": value.line_task_groups,
        "stream_plans": value.stream_plans,
        "source_plans": value.source_plans,
    })
}

fn execution_json(value: &ReplExecutionRecord) -> Value {
    json!({
        "status": cell_execution_status_label(value.status),
        "steps": value.steps,
        "host_calls": value.host_calls,
        "responses": value.responses,
        "events_emitted": value.events_emitted,
        "final_status": &value.final_status,
        "error": &value.error,
        "host_effects": {
            "host_calls": value.host_effects.host_calls,
            "events_emitted": value.host_effects.events_emitted,
            "partially_effectful": value.host_effects.partially_effectful,
            "event_kinds": value.host_effects.event_kinds.iter().map(debug_event_count_json).collect::<Vec<_>>(),
        },
    })
}

fn debug_event_count_json(value: &ReplDebugEventCount) -> Value {
    json!({
        "kind": format!("{:?}", value.kind),
        "count": value.count,
    })
}

fn command_target_json(value: &ReplCommandTarget) -> Value {
    match value {
        ReplCommandTarget::All => json!({ "kind": "all" }),
        ReplCommandTarget::Latest => json!({ "kind": "latest" }),
        ReplCommandTarget::Cell(cell_id) => json!({ "kind": "cell", "id": cell_id.label() }),
        ReplCommandTarget::Selector(selector) => {
            json!({ "kind": "selector", "selector": selector })
        }
    }
}

fn cancel_target_json(value: &ReplCancelTarget) -> Value {
    match value {
        ReplCancelTarget::All => json!({ "kind": "all" }),
        ReplCancelTarget::Task(id) => json!({ "kind": "task", "id": id }),
        ReplCancelTarget::Scope(id) => json!({ "kind": "scope", "id": id }),
    }
}

fn background_request_json(value: &ReplBackgroundRequest) -> Value {
    match value {
        ReplBackgroundRequest::Warm {
            command,
            generation,
        } => json!({
            "kind": "warm",
            "target": command_target_json(&command.target),
            "generation": generation_evidence_json(generation),
        }),
        ReplBackgroundRequest::Codegen {
            command,
            generation,
        } => json!({
            "kind": "codegen",
            "target": command_target_json(&command.target),
            "generation": generation_evidence_json(generation),
        }),
    }
}

fn base_change_outcome_json(value: &ReplBaseChangeOutcome) -> Value {
    json!({
        "debug": base_change_outcome_label(value),
    })
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

fn bounded_strings(values: &[String], options: &ReplCommandFormatOptions) -> Value {
    json!({
        "items": values.iter().take(options.max_items).map(|value| summarized_string(value, options.max_string_bytes)).collect::<Vec<_>>(),
        "truncated": values.len() > options.max_items,
        "total": values.len(),
    })
}

fn summarized_string(value: &str, max_bytes: usize) -> Value {
    if value.len() <= max_bytes {
        return json!({ "text": value, "truncated": false, "bytes": value.len() });
    }
    let mut end = max_bytes.min(value.len());
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    json!({
        "text": &value[..end],
        "truncated": true,
        "bytes": value.len(),
    })
}

fn cell_labels(values: &[ReplCellId]) -> Vec<String> {
    values.iter().map(|value| value.label()).collect()
}

fn generation_labels(values: &[ReplGenerationId]) -> Vec<u64> {
    values.iter().map(|value| value.as_u64()).collect()
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

fn tier_diagnostic_severity_label(value: ReplTierDiagnosticSeverity) -> &'static str {
    match value {
        ReplTierDiagnosticSeverity::Info => "info",
        ReplTierDiagnosticSeverity::Warning => "warning",
        ReplTierDiagnosticSeverity::Error => "error",
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

fn binding_status_label(value: ReplBindingStatus) -> &'static str {
    match value {
        ReplBindingStatus::Active => "active",
        ReplBindingStatus::Invalidated => "invalidated",
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

fn tier_invalidation_reason_label(value: ReplTierInvalidationReason) -> &'static str {
    match value {
        ReplTierInvalidationReason::CellCommitted => "cell_committed",
        ReplTierInvalidationReason::CellExecutionFailed => "cell_execution_failed",
        ReplTierInvalidationReason::CellUndone => "cell_undone",
        ReplTierInvalidationReason::ResetToBase => "reset_to_base",
        ReplTierInvalidationReason::BaseProjectChanged => "base_project_changed",
        ReplTierInvalidationReason::GenerationChanged => "generation_changed",
        ReplTierInvalidationReason::TierStatusRecorded => "tier_status_recorded",
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
    use arcweft_agent_repl::command::{
        ReplBackgroundRequestId, ReplCommandDiagnosticCode, ReplCommandId, ReplTaskList,
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
    fn repl_command_formatter_formats_help_without_legacy_table() {
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
