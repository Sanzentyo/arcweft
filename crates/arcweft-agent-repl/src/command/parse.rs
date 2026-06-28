use thiserror::Error;

use crate::{ReplCellId, ReplCellInput};

use super::types::{
    CancelCommand, CapabilitiesCommand, CellsCommand, CodegenCommand, GenerationsCommand,
    HelpCommand, LoadCommand, ObserveCommand, ReloadCommand, ReplCancelTarget, ReplCommand,
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandTarget, ReplInput, ResetCommand,
    StepCommand, TasksCommand, UndoCommand, WarmCommand,
};

/// Structured parser error.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("{message}")]
pub struct ReplCommandParseError {
    pub code: ReplCommandDiagnosticCode,
    pub command: Option<String>,
    pub message: String,
}

impl ReplCommandParseError {
    #[must_use]
    pub fn new(
        code: ReplCommandDiagnosticCode,
        command: Option<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            command,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn into_diagnostic(self) -> ReplCommandDiagnostic {
        let diagnostic = ReplCommandDiagnostic::error(self.code, self.message);
        if let Some(command) = self.command {
            diagnostic.with_field(command)
        } else {
            diagnostic
        }
    }
}

/// Parses either a typed meta-command or an Arcweft source cell input.
pub fn parse_repl_input(source: &str) -> Result<ReplInput, ReplCommandParseError> {
    let trimmed = source.trim_start();
    if trimmed.is_empty() {
        return Ok(ReplInput::Empty);
    }
    if trimmed.starts_with(':') {
        parse_repl_command(trimmed).map(ReplInput::Command)
    } else {
        Ok(ReplInput::Cell(ReplCellInput::source(source)))
    }
}

/// Parses a typed REPL command. The leading `:` is required.
pub fn parse_repl_command(source: &str) -> Result<ReplCommand, ReplCommandParseError> {
    let trimmed = source.trim();
    let body = trimmed.strip_prefix(':').ok_or_else(|| {
        ReplCommandParseError::new(
            ReplCommandDiagnosticCode::UnknownCommand,
            None,
            "REPL commands must start with `:`",
        )
    })?;
    let (name, arg_text) = split_command_name(body)?;
    let args = arg_text.split_whitespace().collect::<Vec<_>>();
    match name {
        "observe" => parse_observe_command(&args),
        "step" => parse_step_command(&args),
        "tasks" => parse_tasks_command(&args),
        "cancel" => parse_cancel_command(&args),
        "load" => parse_load_command(arg_text),
        "reload" => Ok(parse_reload_command(arg_text)),
        "cells" => parse_cells_command(&args),
        "undo" => parse_undo_command(&args),
        "reset" => parse_reset_command(&args),
        "capabilities" => {
            parse_no_arg_command(name, &args, ReplCommand::Capabilities(CapabilitiesCommand))
        }
        "generations" => parse_generations_command(&args),
        "warm" => Ok(ReplCommand::Warm(WarmCommand {
            target: parse_command_target(arg_text)?,
        })),
        "codegen" => Ok(ReplCommand::Codegen(CodegenCommand {
            target: parse_command_target(arg_text)?,
        })),
        "help" => Ok(ReplCommand::Help(HelpCommand {
            topic: (!arg_text.is_empty()).then(|| arg_text.to_owned()),
        })),
        "quit" | "q" | "exit" => parse_no_arg_command(name, &args, ReplCommand::Quit),
        other => Err(ReplCommandParseError::new(
            ReplCommandDiagnosticCode::UnknownCommand,
            Some(format!(":{other}")),
            format!("unknown REPL command `:{other}`"),
        )),
    }
}

fn split_command_name(body: &str) -> Result<(&str, &str), ReplCommandParseError> {
    let trimmed = body.trim_start();
    if trimmed.is_empty() {
        return Err(ReplCommandParseError::new(
            ReplCommandDiagnosticCode::EmptyCommand,
            None,
            "empty REPL command",
        ));
    }
    let name_end = trimmed.find(char::is_whitespace).unwrap_or(trimmed.len());
    Ok((&trimmed[..name_end], trimmed[name_end..].trim()))
}

fn parse_observe_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut command = ObserveCommand::default();
    for arg in args {
        match *arg {
            "images" | "--images" => command.request.include_images = true,
            "--no-images" => command.request.include_images = false,
            "objects" | "--objects" => command.request.include_objects = true,
            "--no-objects" => command.request.include_objects = false,
            "logs" | "--logs" => command.request.include_logs = true,
            "--no-logs" => command.request.include_logs = false,
            other => return invalid_arg(":observe", other),
        }
    }
    Ok(ReplCommand::Observe(command))
}

fn parse_step_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    match args {
        [] => Ok(ReplCommand::Step(StepCommand::default())),
        [frames] => {
            let frames = frames.parse::<u32>().map_err(|_| {
                ReplCommandParseError::new(
                    ReplCommandDiagnosticCode::InvalidArgument,
                    Some(":step".to_owned()),
                    format!("`:step` frame count must be a positive integer, got `{frames}`"),
                )
            })?;
            if frames == 0 {
                return Err(ReplCommandParseError::new(
                    ReplCommandDiagnosticCode::InvalidArgument,
                    Some(":step".to_owned()),
                    "`:step` frame count must be greater than zero",
                ));
            }
            Ok(ReplCommand::Step(StepCommand { frames }))
        }
        [unexpected, ..] => unexpected_arg(":step", unexpected),
    }
}

fn parse_tasks_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut include_completed = false;
    for arg in args {
        match *arg {
            "--all" | "--include-completed" => include_completed = true,
            other => return invalid_arg(":tasks", other),
        }
    }
    Ok(ReplCommand::Tasks(TasksCommand { include_completed }))
}

fn parse_cancel_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let target = match args {
        [] => {
            return Err(ReplCommandParseError::new(
                ReplCommandDiagnosticCode::MissingArgument,
                Some(":cancel".to_owned()),
                "`:cancel` requires `all`, `task <id>`, `scope <id>`, or a task id",
            ));
        }
        ["all" | "--all"] => ReplCancelTarget::All,
        ["task", id] | [id] => ReplCancelTarget::Task((*id).to_owned()),
        ["scope", id] => ReplCancelTarget::Scope((*id).to_owned()),
        [unexpected, ..] => return unexpected_arg(":cancel", unexpected),
    };
    Ok(ReplCommand::Cancel(CancelCommand { target }))
}

fn parse_load_command(arg_text: &str) -> Result<ReplCommand, ReplCommandParseError> {
    if arg_text.is_empty() {
        return Err(ReplCommandParseError::new(
            ReplCommandDiagnosticCode::MissingArgument,
            Some(":load".to_owned()),
            "`:load` requires a project path",
        ));
    }
    Ok(ReplCommand::Load(LoadCommand {
        path: arg_text.to_owned(),
    }))
}

fn parse_reload_command(arg_text: &str) -> ReplCommand {
    ReplCommand::Reload(ReloadCommand {
        path: (!arg_text.is_empty()).then(|| arg_text.to_owned()),
    })
}

fn parse_cells_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut include_invalidated = false;
    for arg in args {
        match *arg {
            "--all" | "--include-invalidated" => include_invalidated = true,
            other => return invalid_arg(":cells", other),
        }
    }
    Ok(ReplCommand::Cells(CellsCommand {
        include_invalidated,
    }))
}

fn parse_undo_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut preserve_execution_evidence = false;
    for arg in args {
        match *arg {
            "--preserve-execution-evidence" => preserve_execution_evidence = true,
            other => return invalid_arg(":undo", other),
        }
    }
    Ok(ReplCommand::Undo(UndoCommand {
        preserve_execution_evidence,
    }))
}

fn parse_reset_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut preserve_generation = false;
    for arg in args {
        match *arg {
            "--preserve-generation" => preserve_generation = true,
            other => return invalid_arg(":reset", other),
        }
    }
    Ok(ReplCommand::Reset(ResetCommand {
        preserve_generation,
    }))
}

fn parse_generations_command(args: &[&str]) -> Result<ReplCommand, ReplCommandParseError> {
    let mut include_tiers = false;
    for arg in args {
        match *arg {
            "--tiers" | "--include-tiers" => include_tiers = true,
            other => return invalid_arg(":generations", other),
        }
    }
    Ok(ReplCommand::Generations(GenerationsCommand {
        include_tiers,
    }))
}

fn parse_command_target(arg_text: &str) -> Result<ReplCommandTarget, ReplCommandParseError> {
    let args = arg_text.split_whitespace().collect::<Vec<_>>();
    match args.as_slice() {
        [] | ["all" | "--all"] => Ok(ReplCommandTarget::All),
        ["latest" | "--latest"] => Ok(ReplCommandTarget::Latest),
        ["cell", id] => parse_cell_target(id),
        [selector] if selector.starts_with("cell.") => parse_cell_target(selector),
        [selector] => Ok(ReplCommandTarget::Selector((*selector).to_owned())),
        [unexpected, ..] => unexpected_arg(":warm/:codegen", unexpected),
    }
}

fn parse_cell_target(id: &str) -> Result<ReplCommandTarget, ReplCommandParseError> {
    let value = id
        .strip_prefix("cell.")
        .unwrap_or(id)
        .parse::<u64>()
        .map_err(|_| {
            ReplCommandParseError::new(
                ReplCommandDiagnosticCode::InvalidArgument,
                Some(":warm/:codegen".to_owned()),
                format!("cell target must be `cell <number>` or `cell.<number>`, got `{id}`"),
            )
        })?;
    Ok(ReplCommandTarget::Cell(ReplCellId::new(value)))
}

fn parse_no_arg_command(
    name: &str,
    args: &[&str],
    command: ReplCommand,
) -> Result<ReplCommand, ReplCommandParseError> {
    if let Some(unexpected) = args.first() {
        return unexpected_arg(&format!(":{name}"), unexpected);
    }
    Ok(command)
}

fn invalid_arg<T>(command: &str, arg: &str) -> Result<T, ReplCommandParseError> {
    Err(ReplCommandParseError::new(
        ReplCommandDiagnosticCode::InvalidArgument,
        Some(command.to_owned()),
        format!("invalid argument `{arg}` for `{command}`"),
    ))
}

fn unexpected_arg<T>(command: &str, arg: &str) -> Result<T, ReplCommandParseError> {
    Err(ReplCommandParseError::new(
        ReplCommandDiagnosticCode::UnexpectedArgument,
        Some(command.to_owned()),
        format!("unexpected argument `{arg}` for `{command}`"),
    ))
}

/// Stable command names exposed by `:help` and adapter completion tests.
#[must_use]
pub fn repl_command_names() -> Vec<&'static str> {
    vec![
        ":observe",
        ":step",
        ":tasks",
        ":cancel",
        ":load",
        ":reload",
        ":cells",
        ":undo",
        ":reset",
        ":capabilities",
        ":generations",
        ":warm",
        ":codegen",
        ":help",
        ":quit",
    ]
}
