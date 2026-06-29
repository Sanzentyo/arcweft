//! CLI-owned typed Agent REPL inspection/debug command adapter.
//!
//! The shared `arcweft-agent-repl` parser remains the first parser stage for
//! session, task, and tiering commands. This module owns the second typed stage
//! for CLI process-facing inspection/debug commands that need compiler tooling,
//! trace resources, capture, filesystem, or MCP transport access.

mod dispatch;
mod format;
mod parse;
mod types;

pub(super) use self::dispatch::{CliReplCommandContext, dispatch_cli_repl_command};
pub(super) use self::format::{CliReplCommandFormattedOutput, CliReplLocalCommandFormatter};
pub(super) use self::parse::{AgentReplParsedInput, parse_agent_repl_input};
pub(super) use self::types::{CliReplCommand, CliReplCommandResult};
