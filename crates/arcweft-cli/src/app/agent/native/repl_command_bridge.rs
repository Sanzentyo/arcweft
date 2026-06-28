use super::repl::{
    AgentReplCellReport, AgentReplState, agent_repl_eval_compiled_cell, agent_repl_eval_meta,
    agent_repl_ok,
};
use super::repl_command_format::{
    CliReplCommandFormatter, ReplCommandFormatMode, ReplCommandFormatOptions,
    ReplCommandFormattedOutput, ReplCommandResultFormatter,
};
use super::{AgentReplOptions, NativeAdapterRegistrar};
use arcweft_agent_repl::command::{
    ReplCancelEvidence, ReplCancelOutcome, ReplCellSubmissionEvidence, ReplCommand,
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandDiagnosticSeverity,
    ReplCommandEvidence, ReplCommandId, ReplCommandResult, ReplCommandStatus, ReplHelpEvidence,
    ReplInput, ReplTaskList, ReplTasksEvidence, ReplTracePolicy, parse_repl_input,
    repl_command_names,
};

pub(super) fn agent_repl_eval_line(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
) -> AgentReplCellReport {
    match parse_repl_input(input) {
        Ok(ReplInput::Empty) => {
            agent_repl_ok(index, input, "empty", serde_json::json!({ "empty": true }))
        }
        Ok(ReplInput::Cell(cell)) => {
            if state.read_only {
                return agent_repl_typed_cell_report(
                    index,
                    input,
                    options,
                    &ReplCommandResult::rejected(
                        agent_repl_command_id(index),
                        ReplCommandEvidence::CellSubmissionRejected(ReplCellSubmissionEvidence {
                            source_len: cell.source_text().len(),
                            policy: ReplTracePolicy::ReadOnlyTrace,
                        }),
                        ReplCommandDiagnostic::error(
                            ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                            "read-only Agent REPL does not execute Agent cells",
                        ),
                    ),
                );
            }
            agent_repl_eval_compiled_cell(index, input, state)
        }
        Ok(ReplInput::Command(command)) => agent_repl_eval_typed_or_cli_meta(
            index,
            input,
            options,
            state,
            adapter_registrars,
            command,
        ),
        Err(_) if agent_repl_cli_meta_command(input).is_some() => {
            agent_repl_eval_meta(index, input, options, state, adapter_registrars)
        }
        Err(error) => agent_repl_typed_cell_report(
            index,
            input,
            options,
            &ReplCommandResult::error(
                agent_repl_command_id(index),
                ReplCommandEvidence::Empty,
                error.into_diagnostic(),
            ),
        ),
    }
}

fn agent_repl_eval_typed_or_cli_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &mut AgentReplState,
    adapter_registrars: &[NativeAdapterRegistrar],
    command: ReplCommand,
) -> AgentReplCellReport {
    match agent_repl_eval_typed_meta(index, input, options, state, command) {
        Some(report) => report,
        None => agent_repl_eval_meta(index, input, options, state, adapter_registrars),
    }
}

fn agent_repl_eval_typed_meta(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    state: &AgentReplState,
    command: ReplCommand,
) -> Option<AgentReplCellReport> {
    let command_id = agent_repl_command_id(index);
    let result = match command {
        ReplCommand::Help(command) => ReplCommandResult::ok(
            command_id,
            ReplCommandEvidence::Help(ReplHelpEvidence {
                topic: command.topic,
                commands: repl_command_names(),
            }),
        ),
        ReplCommand::Quit => ReplCommandResult::exit_requested(command_id),
        ReplCommand::Tasks(command) => ReplCommandResult::error(
            command_id,
            ReplCommandEvidence::Tasks(ReplTasksEvidence {
                include_completed: command.include_completed,
                tasks: ReplTaskList::default(),
            }),
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostUnavailable,
                format!(
                    "task inspection is not available for this CLI Agent REPL host adapter (include_completed={})",
                    command.include_completed
                ),
            ),
        ),
        ReplCommand::Cancel(command) => {
            if state.read_only {
                ReplCommandResult::rejected(
                    command_id,
                    ReplCommandEvidence::Cancel(ReplCancelEvidence {
                        outcome: ReplCancelOutcome {
                            target: command.target,
                            cancelled: 0,
                            pending_after: 0,
                        },
                    }),
                    ReplCommandDiagnostic::error(
                        ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                        "read-only trace mode rejects mutating command `:cancel`",
                    ),
                )
            } else {
                ReplCommandResult::error(
                    command_id,
                    ReplCommandEvidence::Cancel(ReplCancelEvidence {
                        outcome: ReplCancelOutcome {
                            target: command.target,
                            cancelled: 0,
                            pending_after: 0,
                        },
                    }),
                    ReplCommandDiagnostic::error(
                        ReplCommandDiagnosticCode::HostUnavailable,
                        "task cancellation is not available for this CLI Agent REPL host adapter",
                    ),
                )
            }
        }
        ReplCommand::Warm(_) | ReplCommand::Codegen(_) if state.read_only => {
            ReplCommandResult::rejected(
                command_id,
                ReplCommandEvidence::Empty,
                ReplCommandDiagnostic::error(
                    ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                    "read-only trace mode rejects background tiering commands",
                ),
            )
        }
        ReplCommand::Warm(_) | ReplCommand::Codegen(_) => ReplCommandResult::error(
            command_id,
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic {
                severity: ReplCommandDiagnosticSeverity::Warning,
                code: ReplCommandDiagnosticCode::TieringUnavailable,
                message:
                    "this CLI Agent REPL adapter is not yet backed by a seq05.3 tiering manager"
                        .to_owned(),
                field: None,
            },
        ),
        ReplCommand::Step(_)
        | ReplCommand::Cells(_)
        | ReplCommand::Undo(_)
        | ReplCommand::Reload(_)
        | ReplCommand::Reset(_)
        | ReplCommand::Capabilities(_)
        | ReplCommand::Generations(_) => ReplCommandResult::error(
            command_id,
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostUnavailable,
                "this command requires a seq05.1 session-backed CLI adapter",
            ),
        ),
        ReplCommand::Observe(_) | ReplCommand::Load(_) => return None,
    };
    Some(agent_repl_typed_cell_report(index, input, options, &result))
}

fn agent_repl_typed_cell_report(
    index: usize,
    input: &str,
    options: &AgentReplOptions,
    result: &ReplCommandResult,
) -> AgentReplCellReport {
    let format_options = ReplCommandFormatOptions {
        mode: if options.json {
            ReplCommandFormatMode::Json
        } else {
            ReplCommandFormatMode::Human
        },
        ..ReplCommandFormatOptions::default()
    };
    let output = CliReplCommandFormatter.format_result(result, &format_options);
    let status = match result.status {
        ReplCommandStatus::Ok | ReplCommandStatus::ExitRequested => "ok",
        ReplCommandStatus::Queued => "queued",
        ReplCommandStatus::Rejected | ReplCommandStatus::Error => "error",
    };
    let text = output.text.clone();
    AgentReplCellReport {
        index,
        input: input.to_owned(),
        kind: "meta".to_owned(),
        status: status.to_owned(),
        message: matches!(
            result.status,
            ReplCommandStatus::Rejected | ReplCommandStatus::Error
        )
        .then_some(text.clone()),
        value: Some(if options.json {
            output.json
        } else {
            agent_repl_formatted_output_value(output)
        }),
        quit: matches!(result.status, ReplCommandStatus::ExitRequested),
    }
}

fn agent_repl_formatted_output_value(output: ReplCommandFormattedOutput) -> serde_json::Value {
    match output.json {
        serde_json::Value::Object(mut value) => {
            value.insert(
                "formatted_text".to_owned(),
                serde_json::Value::String(output.text),
            );
            serde_json::Value::Object(value)
        }
        other => serde_json::json!({ "formatted_text": output.text, "value": other }),
    }
}

fn agent_repl_command_id(index: usize) -> ReplCommandId {
    ReplCommandId::new(u64::try_from(index).unwrap_or(u64::MAX).saturating_add(1))
}

fn agent_repl_cli_meta_command(input: &str) -> Option<&str> {
    input.split_whitespace().next().filter(|command| {
        matches!(
            *command,
            ":trace"
                | ":actions"
                | ":type"
                | ":ast"
                | ":hir"
                | ":bytecode"
                | ":capture"
                | ":complete"
                | ":highlight"
                | ":query"
                | ":history"
                | ":bindings"
                | ":connect"
                | ":drop"
                | ":save"
                | ":parse"
                | ":classify"
        )
    })
}
