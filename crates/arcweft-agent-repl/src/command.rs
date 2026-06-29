//! Typed user-visible Agent REPL command parsing and dispatch.
//!
//! This module is the seq05.2 command owner boundary. It deliberately delegates
//! overlay mutations to `ReplSession`, Agent host interaction to the existing
//! `AgentSession`/runtime adapter boundary, and project loading to an external
//! adapter so this crate remains Sans I/O.

mod dispatch;
mod host;
mod json;
mod parse;
mod runtime_task;
mod types;

pub use self::dispatch::{
    BuiltinReplCommandHandler, ReplBackgroundRequestSink, ReplCommandContext, ReplCommandHandler,
};
pub use self::host::{
    AgentSessionReplCommandHost, ReplCommandHost, ReplCommandHostError, ReplCommandHostResult,
    ReplProjectLoader,
};
pub use self::json::{ReplCommandJsonOptions, repl_command_result_json};
pub use self::parse::{
    ReplCommandParseError, parse_repl_command, parse_repl_input, repl_command_names,
};
pub use self::runtime_task::RuntimeTaskReplCommandHost;
pub use self::types::{
    CancelCommand, CapabilitiesCommand, CellsCommand, CodegenCommand, GenerationsCommand,
    HelpCommand, LoadCommand, ObserveCommand, ReloadCommand, ReplBackgroundQueuedEvidence,
    ReplBackgroundRequest, ReplBackgroundRequestId, ReplCancelEvidence, ReplCancelOutcome,
    ReplCancelTarget, ReplCellSubmissionEvidence, ReplCommand, ReplCommandDiagnostic,
    ReplCommandDiagnosticCode, ReplCommandDiagnosticSeverity, ReplCommandEffect,
    ReplCommandEvidence, ReplCommandId, ReplCommandResult, ReplCommandStatus, ReplCommandTarget,
    ReplGenerationCommandEvidence, ReplHelpEvidence, ReplInput, ReplLoadEvidence,
    ReplObservationEvidence, ReplReloadEvidence, ReplResetEvidence, ReplResetSummary,
    ReplStepEvidence, ReplTaskList, ReplTaskRecord, ReplTaskStatus, ReplTasksEvidence,
    ReplTracePolicy, ReplUndoEvidence, ReplUndoSummary, ResetCommand, StepCommand, TasksCommand,
    UndoCommand, WarmCommand,
};
