use arcweft_agent_repl::command::{
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandParseError, ReplInput,
    parse_repl_input,
};
use thiserror::Error;

use super::types::{
    CliCaptureCommand, CliCaptureTarget, CliCompleteCommand, CliConnectCommand, CliConnectTarget,
    CliDropCommand, CliHighlightCommand, CliInspectionCommand, CliInspectionKind, CliParseCommand,
    CliParseKind, CliQueryCommand, CliReplCommand, CliReplCommandKind, CliReplCommandSpan,
    CliSaveCommand,
};

/// Result of the CLI bridge's two-stage parser.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::app::agent::native) enum AgentReplParsedInput {
    Shared(ReplInput),
    Cli(CliReplCommand),
}

/// Structured parse diagnostic after both typed parser stages have been considered.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::app::agent::native) struct AgentReplInputParseError {
    diagnostic: ReplCommandDiagnostic,
}

/// Parser error for the CLI-owned second stage.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub(super) struct CliReplCommandParseError {
    pub(super) code: ReplCommandDiagnosticCode,
    pub(super) command: Option<String>,
    pub(super) anchor: Option<CliReplCommandSpan>,
    pub(super) message: String,
}

struct CliCommandParts<'a> {
    name: &'a str,
    arg_text: &'a str,
    anchor: CliReplCommandSpan,
}

/// Parses one Agent REPL input through the shared typed parser first, then the
/// CLI-owned typed inspection/debug parser.
pub(in crate::app::agent::native) fn parse_agent_repl_input(
    source: &str,
) -> Result<AgentReplParsedInput, AgentReplInputParseError> {
    match parse_repl_input(source) {
        Ok(input) => Ok(AgentReplParsedInput::Shared(input)),
        Err(shared_error) => parse_cli_repl_input(source)
            .map(AgentReplParsedInput::Cli)
            .map_err(|cli_error| {
                AgentReplInputParseError::from_stage_errors(shared_error, cli_error)
            }),
    }
}

/// Parses a CLI-owned REPL command. The shared parser must be tried first by
/// callers so shared command names keep one owner.
pub(super) fn parse_cli_repl_input(
    source: &str,
) -> Result<CliReplCommand, CliReplCommandParseError> {
    let parts = split_cli_command(source)?;
    let command = match parts.name {
        "trace" => no_arg_command(&parts, CliReplCommandKind::Trace),
        "actions" => no_arg_command(&parts, CliReplCommandKind::Actions),
        "type" => inspection_command(&parts, CliInspectionKind::Type, "an expression"),
        "ast" => inspection_command(&parts, CliInspectionKind::Ast, "a fragment"),
        "hir" => inspection_command(&parts, CliInspectionKind::Hir, "a fragment"),
        "bytecode" => inspection_command(&parts, CliInspectionKind::Bytecode, "a fragment"),
        "capture" => capture_command(&parts),
        "query" => required_text_command(&parts, "text", |text| {
            CliReplCommandKind::Query(CliQueryCommand { text })
        }),
        "drop" => drop_command(&parts),
        "save" => required_text_command(&parts, "a .awfagent path", |path| {
            CliReplCommandKind::Save(CliSaveCommand { path })
        }),
        "connect" => connect_command(&parts),
        "parse" => parse_command(&parts, CliParseKind::Parse),
        "classify" => parse_command(&parts, CliParseKind::Classify),
        "complete" => required_text_command(
            &parts,
            "source text before the cursor",
            |source_before_cursor| {
                CliReplCommandKind::Complete(CliCompleteCommand {
                    source_before_cursor,
                })
            },
        ),
        "highlight" => required_text_command(&parts, "source text", |source| {
            CliReplCommandKind::Highlight(CliHighlightCommand { source })
        }),
        "history" => no_arg_command(&parts, CliReplCommandKind::History),
        "bindings" => no_arg_command(&parts, CliReplCommandKind::Bindings),
        other => Err(CliReplCommandParseError::new(
            ReplCommandDiagnosticCode::UnknownCommand,
            Some(format!(":{other}")),
            Some(parts.anchor),
            format!("unknown CLI Agent REPL command `:{other}`"),
        )),
    }?;
    Ok(CliReplCommand::new(parts.anchor, command))
}

#[cfg(test)]
#[must_use]
pub(super) fn cli_repl_command_names() -> Vec<&'static str> {
    vec![
        ":trace",
        ":actions",
        ":type",
        ":ast",
        ":hir",
        ":bytecode",
        ":capture",
        ":query",
        ":drop",
        ":save",
        ":connect",
        ":parse",
        ":classify",
        ":complete",
        ":highlight",
        ":history",
        ":bindings",
    ]
}

impl AgentReplInputParseError {
    #[must_use]
    fn from_stage_errors(
        shared_error: ReplCommandParseError,
        cli_error: CliReplCommandParseError,
    ) -> Self {
        if cli_error.code != ReplCommandDiagnosticCode::UnknownCommand
            && cli_error.code != ReplCommandDiagnosticCode::EmptyCommand
        {
            return Self {
                diagnostic: cli_error.into_diagnostic(),
            };
        }
        if shared_error.code != ReplCommandDiagnosticCode::UnknownCommand {
            return Self {
                diagnostic: shared_error.into_diagnostic(),
            };
        }
        let command = cli_error.command.or(shared_error.command);
        let message = command.as_ref().map_or_else(
            || "unknown Agent REPL command after shared and CLI parser stages".to_owned(),
            |command| {
                format!(
                    "unknown Agent REPL command `{command}` after shared REPL and CLI inspection/debug parser stages"
                )
            },
        );
        let diagnostic =
            ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::UnknownCommand, message);
        Self {
            diagnostic: match command {
                Some(command) => diagnostic.with_field(command),
                None => diagnostic,
            },
        }
    }

    #[must_use]
    pub(in crate::app::agent::native) fn into_diagnostic(self) -> ReplCommandDiagnostic {
        self.diagnostic
    }
}

impl CliReplCommandParseError {
    #[must_use]
    fn new(
        code: ReplCommandDiagnosticCode,
        command: Option<String>,
        anchor: Option<CliReplCommandSpan>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            command,
            anchor,
            message: message.into(),
        }
    }

    #[must_use]
    fn into_diagnostic(self) -> ReplCommandDiagnostic {
        let diagnostic = ReplCommandDiagnostic::error(self.code, self.message);
        let field = match (self.command, self.anchor) {
            (Some(command), _) => Some(command),
            (None, Some(anchor)) => Some(format!("bytes:{}..{}", anchor.start, anchor.end)),
            (None, None) => None,
        };
        match field {
            Some(field) => diagnostic.with_field(field),
            None => diagnostic,
        }
    }
}

fn split_cli_command(source: &str) -> Result<CliCommandParts<'_>, CliReplCommandParseError> {
    let trimmed = source.trim_start();
    if trimmed.is_empty() {
        return Err(CliReplCommandParseError::new(
            ReplCommandDiagnosticCode::EmptyCommand,
            None,
            None,
            "empty REPL command",
        ));
    }
    let leading = source.len() - trimmed.len();
    let Some(after_colon) = trimmed.strip_prefix(':') else {
        return Err(CliReplCommandParseError::new(
            ReplCommandDiagnosticCode::UnknownCommand,
            None,
            None,
            "CLI REPL commands must start with `:`",
        ));
    };
    let body_leading = after_colon.len() - after_colon.trim_start().len();
    let name_start = leading + 1 + body_leading;
    let name_body = &source[name_start..];
    if name_body.trim_start().is_empty() {
        return Err(CliReplCommandParseError::new(
            ReplCommandDiagnosticCode::EmptyCommand,
            None,
            Some(CliReplCommandSpan::new(leading, leading + 1)),
            "empty REPL command",
        ));
    }
    let name_end_offset = name_body
        .find(char::is_whitespace)
        .unwrap_or(name_body.len());
    let name = &name_body[..name_end_offset];
    let name_end = name_start + name_end_offset;
    let anchor = CliReplCommandSpan::new(leading, name_end);
    Ok(CliCommandParts {
        name,
        arg_text: source[name_end..].trim(),
        anchor,
    })
}

fn no_arg_command(
    parts: &CliCommandParts<'_>,
    command: CliReplCommandKind,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    if let Some(arg) = first_arg(parts.arg_text) {
        return Err(unexpected_arg(parts, arg));
    }
    Ok(command)
}

fn inspection_command(
    parts: &CliCommandParts<'_>,
    kind: CliInspectionKind,
    required: &str,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    required_text_command(parts, required, |source| {
        CliReplCommandKind::Inspection(CliInspectionCommand { kind, source })
    })
}

fn parse_command(
    parts: &CliCommandParts<'_>,
    kind: CliParseKind,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    required_text_command(parts, "a fragment", |source| {
        CliReplCommandKind::Parse(CliParseCommand { kind, source })
    })
}

fn capture_command(
    parts: &CliCommandParts<'_>,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    let args = parts.arg_text.split_whitespace().collect::<Vec<_>>();
    let target = match args.as_slice() {
        [] | ["viewport"] => CliCaptureTarget::Viewport,
        ["layer", id] => CliCaptureTarget::Layer {
            id: (*id).to_owned(),
        },
        ["object", id] => CliCaptureTarget::Object {
            id: (*id).to_owned(),
        },
        [kind] if matches!(*kind, "layer" | "object") => {
            return Err(CliReplCommandParseError::new(
                ReplCommandDiagnosticCode::MissingArgument,
                Some(qualified_name(parts)),
                Some(parts.anchor),
                format!("`:capture {kind}` requires an id"),
            ));
        }
        [unexpected, ..] => {
            return Err(CliReplCommandParseError::new(
                ReplCommandDiagnosticCode::InvalidArgument,
                Some(qualified_name(parts)),
                Some(parts.anchor),
                format!(
                    "`:capture` accepts only viewport, layer ID, or object ID; got `{unexpected}`"
                ),
            ));
        }
    };
    Ok(CliReplCommandKind::Capture(CliCaptureCommand { target }))
}

fn drop_command(
    parts: &CliCommandParts<'_>,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    let args = parts.arg_text.split_whitespace().collect::<Vec<_>>();
    match args.as_slice() {
        [] => Err(missing_arg(parts, "a binding name")),
        [name] => Ok(CliReplCommandKind::Drop(CliDropCommand {
            name: (*name).to_owned(),
        })),
        [_, unexpected, ..] => Err(unexpected_arg(parts, unexpected)),
    }
}

fn connect_command(
    parts: &CliCommandParts<'_>,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    let target = parts.arg_text.trim();
    if target.is_empty() {
        return Err(missing_arg(parts, "a target"));
    }
    let parsed = if target == "current" {
        CliConnectTarget::Current
    } else if let Some(endpoint) = target.strip_prefix("stdio:") {
        parse_stdio_mcp_target(parts, endpoint)?
    } else if let Some(endpoint) = target.strip_prefix("mcp:") {
        parse_stdio_mcp_target(parts, endpoint)?
    } else if let Some(endpoint) = target.strip_prefix("stdio ") {
        parse_stdio_mcp_target(parts, endpoint)?
    } else if let Some(path) = target.strip_prefix("source ") {
        let path = path.trim();
        if path.is_empty() {
            return Err(missing_arg(parts, "a source path"));
        }
        CliConnectTarget::Source {
            path: path.to_owned(),
        }
    } else if let Some(rest) = target.strip_prefix("profile ") {
        parse_profile_target(parts, rest)?
    } else {
        CliConnectTarget::Inferred {
            target: target.to_owned(),
        }
    };
    Ok(CliReplCommandKind::Connect(CliConnectCommand {
        target: parsed,
    }))
}

fn parse_stdio_mcp_target(
    parts: &CliCommandParts<'_>,
    endpoint: &str,
) -> Result<CliConnectTarget, CliReplCommandParseError> {
    let mut fields = endpoint.split_whitespace();
    let program = fields
        .next()
        .filter(|field| !field.is_empty())
        .ok_or_else(|| missing_arg(parts, "an executable name"))?;
    validate_stdio_part(parts, program)?;
    let args = fields
        .map(|arg| validate_stdio_part(parts, arg).map(|()| arg.to_owned()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CliConnectTarget::StdioMcp {
        program: program.to_owned(),
        args,
    })
}

fn validate_stdio_part(
    parts: &CliCommandParts<'_>,
    value: &str,
) -> Result<(), CliReplCommandParseError> {
    if value.contains(['|', '>', '<', '&', ';']) {
        return Err(CliReplCommandParseError::new(
            ReplCommandDiagnosticCode::InvalidArgument,
            Some(qualified_name(parts)),
            Some(parts.anchor),
            format!("unsupported shell metacharacter in stdio endpoint segment `{value}`"),
        ));
    }
    Ok(())
}

fn parse_profile_target(
    parts: &CliCommandParts<'_>,
    rest: &str,
) -> Result<CliConnectTarget, CliReplCommandParseError> {
    let mut fields = rest.split_whitespace();
    let id = fields
        .next()
        .ok_or_else(|| missing_arg(parts, "a profile id"))?;
    let mut manifest = None;
    while let Some(flag) = fields.next() {
        match flag {
            "--manifest" => {
                let path = fields
                    .next()
                    .ok_or_else(|| missing_arg(parts, "a manifest path"))?;
                manifest = Some(path.to_owned());
            }
            other => return Err(invalid_arg(parts, other)),
        }
    }
    Ok(CliConnectTarget::Profile {
        id: id.to_owned(),
        manifest,
    })
}

fn required_text_command(
    parts: &CliCommandParts<'_>,
    required: &str,
    build: impl FnOnce(String) -> CliReplCommandKind,
) -> Result<CliReplCommandKind, CliReplCommandParseError> {
    if parts.arg_text.is_empty() {
        return Err(missing_arg(parts, required));
    }
    Ok(build(parts.arg_text.to_owned()))
}

fn missing_arg(parts: &CliCommandParts<'_>, required: &str) -> CliReplCommandParseError {
    CliReplCommandParseError::new(
        ReplCommandDiagnosticCode::MissingArgument,
        Some(qualified_name(parts)),
        Some(parts.anchor),
        format!("`{}` requires {required}", qualified_name(parts)),
    )
}

fn invalid_arg(parts: &CliCommandParts<'_>, arg: &str) -> CliReplCommandParseError {
    CliReplCommandParseError::new(
        ReplCommandDiagnosticCode::InvalidArgument,
        Some(qualified_name(parts)),
        Some(parts.anchor),
        format!("invalid argument `{arg}` for `{}`", qualified_name(parts)),
    )
}

fn unexpected_arg(parts: &CliCommandParts<'_>, arg: &str) -> CliReplCommandParseError {
    CliReplCommandParseError::new(
        ReplCommandDiagnosticCode::UnexpectedArgument,
        Some(qualified_name(parts)),
        Some(parts.anchor),
        format!(
            "unexpected argument `{arg}` for `{}`",
            qualified_name(parts)
        ),
    )
}

fn first_arg(arg_text: &str) -> Option<&str> {
    arg_text.split_whitespace().next()
}

fn qualified_name(parts: &CliCommandParts<'_>) -> String {
    format!(":{}", parts.name)
}

#[cfg(test)]
mod tests {
    use arcweft_agent_repl::command::{ReplCommand, ReplInput, repl_command_names};

    use super::*;
    use crate::app::agent::native::repl_cli_command::types::CliReplCommandKind;

    #[test]
    fn repl_cli_inspection_parser_selects_shared_repl_command_first() {
        let parsed = parse_agent_repl_input(":cells --all").expect("shared command parses");
        match parsed {
            AgentReplParsedInput::Shared(ReplInput::Command(ReplCommand::Cells(command))) => {
                assert!(command.include_invalidated);
            }
            other => panic!("expected shared cells command, got {other:?}"),
        }
    }

    #[test]
    fn repl_cli_inspection_parser_selects_cli_command_second() {
        let parsed = parse_agent_repl_input(":trace").expect("cli command parses");
        match parsed {
            AgentReplParsedInput::Cli(command) => {
                assert_eq!(command.name(), ":trace");
                assert_eq!(command.anchor, CliReplCommandSpan::new(0, 6));
            }
            AgentReplParsedInput::Shared(other) => {
                panic!("expected CLI trace command, got shared input {other:?}")
            }
        }
    }

    #[test]
    fn repl_cli_inspection_unknown_command_reports_after_both_parser_stages() {
        let error =
            parse_agent_repl_input(":definitely-not-a-command").expect_err("unknown command");
        let diagnostic = error.into_diagnostic();
        assert_eq!(diagnostic.code, ReplCommandDiagnosticCode::UnknownCommand);
        assert_eq!(
            diagnostic.field.as_deref(),
            Some(":definitely-not-a-command")
        );
        assert!(
            diagnostic
                .message
                .contains("shared REPL and CLI inspection/debug")
        );
    }

    #[test]
    fn repl_cli_inspection_malformed_cli_command_reports_cli_diagnostic() {
        let error = parse_agent_repl_input(":type").expect_err("missing expression");
        let diagnostic = error.into_diagnostic();
        assert_eq!(diagnostic.code, ReplCommandDiagnosticCode::MissingArgument);
        assert_eq!(diagnostic.field.as_deref(), Some(":type"));
        assert!(diagnostic.message.contains("requires an expression"));
    }

    #[test]
    fn repl_cli_inspection_capture_target_is_structured() {
        let command = parse_cli_repl_input(":capture layer dialog.front").expect("capture parses");
        match command.kind {
            CliReplCommandKind::Capture(command) => {
                assert_eq!(
                    command.target,
                    CliCaptureTarget::Layer {
                        id: "dialog.front".to_owned()
                    }
                );
            }
            other => panic!("expected capture, got {other:?}"),
        }
    }

    #[test]
    fn repl_cli_inspection_connect_profile_manifest_is_structured() {
        let command = parse_cli_repl_input(":connect profile dev --manifest profiles.toml")
            .expect("connect parses");
        match command.kind {
            CliReplCommandKind::Connect(command) => {
                assert_eq!(
                    command.target,
                    CliConnectTarget::Profile {
                        id: "dev".to_owned(),
                        manifest: Some("profiles.toml".to_owned()),
                    }
                );
            }
            other => panic!("expected connect, got {other:?}"),
        }
    }

    #[test]
    fn repl_cli_inspection_cli_parser_does_not_overlap_shared_command_names() {
        let shared = repl_command_names();
        let cli = cli_repl_command_names();
        let overlap = cli
            .iter()
            .filter(|name| shared.contains(name))
            .copied()
            .collect::<Vec<_>>();
        assert!(overlap.is_empty(), "ambiguous command owners: {overlap:?}");
    }
}
