use std::path::Path;

use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandId, ReplTracePolicy,
};
use serde_json::{Value, json};

use super::super::repl::{AgentReplCellReport, AgentReplConnection, AgentReplState};
use super::super::{AgentReplOptions, NativeAdapterRegistrar};
use super::types::{
    CliCaptureCommand, CliCompleteCommand, CliConnectCommand, CliConnectTarget, CliDropCommand,
    CliHighlightCommand, CliInspectionCommand, CliInspectionKind, CliParseCommand, CliQueryCommand,
    CliReplCommand, CliReplCommandEvidence, CliReplCommandKind, CliReplCommandResult,
    CliSaveCommand,
};

pub(in crate::app::agent::native) struct CliReplCommandContext<'a> {
    pub(in crate::app::agent::native) command_id: ReplCommandId,
    pub(in crate::app::agent::native) index: usize,
    pub(in crate::app::agent::native) input: &'a str,
    pub(in crate::app::agent::native) options: &'a AgentReplOptions,
    pub(in crate::app::agent::native) state: &'a mut AgentReplState,
    pub(in crate::app::agent::native) adapter_registrars: &'a [NativeAdapterRegistrar],
    pub(in crate::app::agent::native) trace_policy: ReplTracePolicy,
}

pub(in crate::app::agent::native) fn dispatch_cli_repl_command(
    context: CliReplCommandContext<'_>,
    command: CliReplCommand,
) -> CliReplCommandResult {
    let command_name = command.name();
    if !context.trace_policy.permits_command(command.effect()) {
        return CliReplCommandResult::rejected(
            context.command_id,
            command_name,
            CliReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                format!("read-only trace mode rejects mutating CLI command `{command_name}`"),
            )
            .with_field(command_name),
        );
    }

    let mut context = context;
    match command.kind {
        CliReplCommandKind::Trace => dispatch_trace(&mut context, command_name),
        CliReplCommandKind::Actions => dispatch_actions(&mut context, command_name),
        CliReplCommandKind::Inspection(command) => {
            dispatch_inspection(&mut context, command_name, command)
        }
        CliReplCommandKind::Capture(command) => {
            dispatch_capture(&mut context, command_name, command)
        }
        CliReplCommandKind::Query(command) => dispatch_query(&mut context, command_name, command),
        CliReplCommandKind::Drop(command) => dispatch_drop(&mut context, command_name, command),
        CliReplCommandKind::Save(command) => dispatch_save(&mut context, command_name, command),
        CliReplCommandKind::Connect(command) => {
            dispatch_connect(&mut context, command_name, command)
        }
        CliReplCommandKind::Parse(command) => dispatch_parse(&mut context, command_name, command),
        CliReplCommandKind::Complete(command) => {
            dispatch_complete(&mut context, command_name, command)
        }
        CliReplCommandKind::Highlight(command) => {
            dispatch_highlight(&mut context, command_name, command)
        }
        CliReplCommandKind::History => dispatch_history(&mut context, command_name),
        CliReplCommandKind::Bindings => dispatch_bindings(&mut context, command_name),
    }
}

fn dispatch_trace(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
) -> CliReplCommandResult {
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Trace { value },
        super::super::repl::agent_repl_trace(context.index, context.input, context.state),
    )
}

fn dispatch_actions(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
) -> CliReplCommandResult {
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Actions { value },
        super::super::repl::agent_repl_actions(context.index, context.input, context.state),
    )
}

fn dispatch_inspection(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliInspectionCommand,
) -> CliReplCommandResult {
    let report = match command.kind {
        CliInspectionKind::Type => {
            super::super::repl::agent_repl_type(context.index, context.input, &command.source)
        }
        CliInspectionKind::Ast => {
            super::super::repl::agent_repl_ast(context.index, context.input, &command.source)
        }
        CliInspectionKind::Hir => {
            super::super::repl::agent_repl_hir(context.index, context.input, &command.source)
        }
        CliInspectionKind::Bytecode => {
            super::super::repl::agent_repl_awbc(context.index, context.input, &command.source)
        }
    };
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Inspection {
            kind: command.kind,
            source: command.source,
            value,
        },
        report,
    )
}

fn dispatch_capture(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliCaptureCommand,
) -> CliReplCommandResult {
    let target_arg = command.target.to_repl_arg();
    let report = super::super::repl::agent_repl_capture(
        context.index,
        context.input,
        &target_arg,
        context.options,
        context.state,
        context.adapter_registrars,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Capture {
            target: command.target,
            value,
        },
        report,
    )
}

fn dispatch_query(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliQueryCommand,
) -> CliReplCommandResult {
    let report = super::super::repl::agent_repl_query(
        context.index,
        context.input,
        &command.text,
        context.state,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Query {
            text: command.text,
            value,
        },
        report,
    )
}

fn dispatch_drop(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliDropCommand,
) -> CliReplCommandResult {
    let report = super::super::repl::agent_repl_drop(
        context.index,
        context.input,
        &command.name,
        context.state,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Drop {
            name: command.name,
            value,
        },
        report,
    )
}

fn dispatch_save(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliSaveCommand,
) -> CliReplCommandResult {
    let report = super::super::repl::agent_repl_save(
        context.index,
        context.input,
        &command.path,
        context.state,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Save {
            path: command.path,
            value,
        },
        report,
    )
}

fn dispatch_connect(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliConnectCommand,
) -> CliReplCommandResult {
    let connection = match command
        .target
        .clone()
        .into_agent_repl_connection(context.options)
    {
        Ok(connection) => connection,
        Err(message) => {
            return CliReplCommandResult::error(
                context.command_id,
                command_name,
                CliReplCommandEvidence::Empty,
                ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::InvalidArgument, message)
                    .with_field(command_name),
            );
        }
    };
    let report = super::super::repl::agent_repl_apply_connection(
        context.index,
        context.input,
        connection,
        context.state,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Connect {
            target: command.target,
            value,
        },
        report,
    )
}

fn dispatch_parse(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliParseCommand,
) -> CliReplCommandResult {
    let value = super::super::repl::agent_repl_parse_cell(&command.source);
    CliReplCommandResult::ok(
        context.command_id,
        command_name,
        CliReplCommandEvidence::Parse {
            kind: command.kind,
            source: command.source,
            value,
        },
    )
}

fn dispatch_complete(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliCompleteCommand,
) -> CliReplCommandResult {
    let report = super::super::repl::agent_repl_complete(
        context.index,
        context.input,
        &command.source_before_cursor,
        context.state,
    );
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Complete {
            source_before_cursor: command.source_before_cursor,
            value,
        },
        report,
    )
}

fn dispatch_highlight(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
    command: CliHighlightCommand,
) -> CliReplCommandResult {
    let report =
        super::super::repl::agent_repl_highlight(context.index, context.input, &command.source);
    result_from_report(
        context.command_id,
        command_name,
        |value| CliReplCommandEvidence::Highlight {
            source: command.source,
            value,
        },
        report,
    )
}

fn dispatch_history(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
) -> CliReplCommandResult {
    let value = json!({ "cells": &context.state.history });
    CliReplCommandResult::ok(
        context.command_id,
        command_name,
        CliReplCommandEvidence::History { value },
    )
}

fn dispatch_bindings(
    context: &mut CliReplCommandContext<'_>,
    command_name: &'static str,
) -> CliReplCommandResult {
    let value = json!({
        "bindings": context.state.bindings.values().collect::<Vec<_>>(),
    });
    CliReplCommandResult::ok(
        context.command_id,
        command_name,
        CliReplCommandEvidence::Bindings { value },
    )
}

fn result_from_report(
    command_id: ReplCommandId,
    command_name: &'static str,
    evidence: impl FnOnce(Value) -> CliReplCommandEvidence,
    report: AgentReplCellReport,
) -> CliReplCommandResult {
    match report.status.as_str() {
        "ok" | "parsed" => CliReplCommandResult::ok(
            command_id,
            command_name,
            evidence(report.value.unwrap_or(Value::Null)),
        ),
        _ => CliReplCommandResult::error(
            command_id,
            command_name,
            CliReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::HostError,
                report
                    .message
                    .unwrap_or_else(|| format!("CLI command `{command_name}` failed")),
            )
            .with_field(command_name),
        ),
    }
}

impl CliConnectTarget {
    fn into_agent_repl_connection(
        self,
        options: &AgentReplOptions,
    ) -> Result<Option<AgentReplConnection>, String> {
        match self {
            Self::Current => {
                if options.path.is_none() && options.profile.profile.is_none() {
                    return Err(
                        ":connect current requires the REPL to start with a source path or --profile"
                            .to_owned(),
                    );
                }
                Ok(None)
            }
            Self::Source { path } => Ok(Some(AgentReplConnection::Source { path })),
            Self::Profile { id, manifest } => Ok(Some(AgentReplConnection::Profile {
                id,
                manifest: manifest
                    .unwrap_or_else(|| options.profile.manifest.display().to_string()),
            })),
            Self::StdioMcp { program, args } => {
                Ok(Some(AgentReplConnection::StdioMcp { program, args }))
            }
            Self::Inferred { target } => {
                if Path::new(&target)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("arcw"))
                {
                    Ok(Some(AgentReplConnection::Source { path: target }))
                } else {
                    Ok(Some(AgentReplConnection::Profile {
                        id: target,
                        manifest: options.profile.manifest.display().to_string(),
                    }))
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use arcweft_agent_repl::command::{ReplCommandEffect, ReplTracePolicy};

    use super::super::parse::parse_cli_repl_input;

    #[test]
    fn repl_cli_inspection_read_only_trace_rejects_mutating_cli_commands() {
        for source in [
            ":capture",
            ":connect current",
            ":drop cell.1",
            ":save out.awfagent",
        ] {
            let command = parse_cli_repl_input(source).expect("CLI command parses");
            assert!(
                !ReplTracePolicy::ReadOnlyTrace.permits_command(command.effect()),
                "{source}"
            );
        }
    }

    #[test]
    fn repl_cli_inspection_read_only_trace_permits_inspection_cli_commands() {
        for source in [
            ":trace",
            ":actions",
            ":type 1",
            ":parse return 1",
            ":highlight agent",
        ] {
            let command = parse_cli_repl_input(source).expect("CLI command parses");
            assert_eq!(command.effect(), ReplCommandEffect::ReadOnly);
            assert!(
                ReplTracePolicy::ReadOnlyTrace.permits_command(command.effect()),
                "{source}"
            );
        }
    }
}
