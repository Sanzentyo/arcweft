use arcweft_agent_protocol::protocol::{ObservationEnvelope, ObserveRequest};

use crate::{
    ReplBaseChangeOutcome, ReplBindingEvidence, ReplCapabilityReport, ReplCellId, ReplCellInput,
    ReplCellKind, ReplCellList, ReplGenerationEvidence, ReplGenerationId, ReplResetOutcome,
    ReplTierInvalidationToken, ReplTierStatusProjection, ReplUndoOutcome,
};

/// Stable per-dispatch id assigned before a command is executed.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplCommandId(u64);

/// Background request id returned by a seq05.3-capable sink.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ReplBackgroundRequestId(u64);

/// Frontend input after separating meta-commands from Arcweft source cells.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplInput {
    Empty,
    Command(ReplCommand),
    Cell(ReplCellInput),
}

/// Typed command surface. Formatting and transport adapters should consume this
/// model rather than re-parsing stringly CLI commands.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplCommand {
    Observe(ObserveCommand),
    Step(StepCommand),
    Tasks(TasksCommand),
    Cancel(CancelCommand),
    Load(LoadCommand),
    Reload(ReloadCommand),
    Cells(CellsCommand),
    Undo(UndoCommand),
    Reset(ResetCommand),
    Capabilities(CapabilitiesCommand),
    Generations(GenerationsCommand),
    Warm(WarmCommand),
    Codegen(CodegenCommand),
    Help(HelpCommand),
    Quit,
}

/// `:observe` host read command.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ObserveCommand {
    pub request: ObserveRequest,
}

/// `:step` host read command. The count is normalized to at least one frame.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepCommand {
    pub frames: u32,
}

/// `:tasks` host task inspection command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TasksCommand {
    pub include_completed: bool,
}

/// `:cancel` host task mutation command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CancelCommand {
    pub target: ReplCancelTarget,
}

/// `:load` project-base replacement command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadCommand {
    pub path: String,
}

/// `:reload` project-base replacement command using the current or explicit path.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReloadCommand {
    pub path: Option<String>,
}

/// `:cells` overlay inspection command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CellsCommand {
    pub include_invalidated: bool,
}

/// `:undo` overlay mutation command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct UndoCommand {
    pub preserve_execution_evidence: bool,
}

/// `:reset` overlay mutation command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ResetCommand {
    pub preserve_generation: bool,
}

/// `:capabilities` inspection command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CapabilitiesCommand;

/// `:generations` inspection command.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GenerationsCommand {
    pub include_tiers: bool,
}

/// `:warm` typed extension hook reserved for seq05.3.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WarmCommand {
    pub target: ReplCommandTarget,
}

/// `:codegen` typed extension hook reserved for seq05.3.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodegenCommand {
    pub target: ReplCommandTarget,
}

/// `:help` command.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HelpCommand {
    pub topic: Option<String>,
}

/// Target selector accepted by background tiering commands.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ReplCommandTarget {
    #[default]
    All,
    Latest,
    Cell(ReplCellId),
    Selector(String),
}

/// Target selector accepted by `:cancel`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReplCancelTarget {
    All,
    Task(String),
    Scope(String),
}

/// Command effect class used by read-only trace policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCommandEffect {
    ReadOnly,
    HostRead,
    SessionMutation,
    HostMutation,
    BackgroundMutation,
    Exit,
}

/// Read-only trace policy for command adapters.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReplTracePolicy {
    #[default]
    ReadWrite,
    ReadOnlyTrace,
}

/// Stable command status before any terminal/LSP/MCP formatting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCommandStatus {
    Ok,
    Queued,
    Rejected,
    Error,
    ExitRequested,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCommandDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Stable diagnostic code for command parse/dispatch errors.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplCommandDiagnosticCode {
    EmptyCommand,
    UnknownCommand,
    MissingArgument,
    InvalidArgument,
    UnexpectedArgument,
    ReadOnlyTraceRejected,
    HostUnavailable,
    HostError,
    ProjectLoaderUnavailable,
    ProjectLoaderError,
    SessionError,
    UnhandledExtension,
    TieringUnavailable,
}

/// One typed command diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCommandDiagnostic {
    pub severity: ReplCommandDiagnosticSeverity,
    pub code: ReplCommandDiagnosticCode,
    pub message: String,
    pub field: Option<String>,
}

/// Command result with deterministic evidence before formatting.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplCommandResult {
    pub command_id: ReplCommandId,
    pub status: ReplCommandStatus,
    pub evidence: ReplCommandEvidence,
    pub diagnostics: Vec<ReplCommandDiagnostic>,
}

/// Evidence variants exposed to tests and Agent clients.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplCommandEvidence {
    Empty,
    Observation(ReplObservationEvidence),
    Step(ReplStepEvidence),
    Tasks(ReplTasksEvidence),
    Cancel(ReplCancelEvidence),
    Load(ReplLoadEvidence),
    Reload(ReplReloadEvidence),
    Cells(ReplCellList),
    Undo(ReplUndoEvidence),
    Reset(ReplResetEvidence),
    Capabilities(ReplCapabilityReport),
    Generations(ReplGenerationCommandEvidence),
    BackgroundQueued(ReplBackgroundQueuedEvidence),
    Help(ReplHelpEvidence),
    Quit,
    CellSubmissionRejected(ReplCellSubmissionEvidence),
}

/// Deterministic projection of one host observation.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplObservationEvidence {
    pub request: ObserveRequest,
    pub tick: u64,
    pub frame_id: String,
    pub state_hash: String,
    pub render_hash: String,
    pub action_count: usize,
    pub signal_count: usize,
}

/// Deterministic projection of one host step result.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplStepEvidence {
    pub frames: u32,
    pub observation: ReplObservationEvidence,
}

/// Stable host task list.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplTaskList {
    pub tasks: Vec<ReplTaskRecord>,
}

/// Stable projection of one host task.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTaskRecord {
    pub id: String,
    pub status: ReplTaskStatus,
    pub generation: Option<u64>,
    pub logical_epoch: Option<u64>,
    pub sequence: Option<u64>,
    pub cancel_scope: Option<String>,
}

/// Stable task status labels for adapters over runtime-driver task state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTaskStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// Evidence for `:tasks`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReplTasksEvidence {
    pub include_completed: bool,
    pub tasks: ReplTaskList,
}

/// Result returned by a host task cancellation adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCancelOutcome {
    pub target: ReplCancelTarget,
    pub cancelled: usize,
    pub pending_after: usize,
}

/// Evidence for `:cancel`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCancelEvidence {
    pub outcome: ReplCancelOutcome,
}

/// Evidence for `:load`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplLoadEvidence {
    pub path: String,
    pub base_label: String,
    pub outcome: ReplBaseChangeOutcome,
}

/// Evidence for `:reload`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplReloadEvidence {
    pub path: Option<String>,
    pub base_label: String,
    pub outcome: ReplBaseChangeOutcome,
}

/// Evidence for `:undo`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplUndoEvidence {
    pub summary: ReplUndoSummary,
    pub binding_evidence_after: ReplBindingEvidence,
    pub generation_evidence_after: ReplGenerationEvidence,
    pub tier_invalidations: Vec<ReplTierInvalidationToken>,
}

/// Evidence for `:reset`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplResetEvidence {
    pub summary: ReplResetSummary,
    pub binding_evidence_after: ReplBindingEvidence,
    pub generation_evidence_after: ReplGenerationEvidence,
    pub tier_invalidations: Vec<ReplTierInvalidationToken>,
}

/// Compact summary of a seq05.1 undo outcome for command evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplUndoSummary {
    pub removed_cell_id: ReplCellId,
    pub removed_cell_kind: ReplCellKind,
    pub removed_source_hash: String,
    pub removed_binding_count: usize,
    pub remaining_cells: usize,
    pub overlay_hash: String,
}

/// Compact summary of a seq05.1 reset outcome for command evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplResetSummary {
    pub removed_cells: usize,
    pub retained_generation: ReplGenerationId,
    pub overlay_hash: String,
}

/// Evidence for `:generations`.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplGenerationCommandEvidence {
    pub generation: ReplGenerationEvidence,
    pub bindings: ReplBindingEvidence,
    pub tiers: Option<ReplTierStatusProjection>,
}

/// Evidence returned when seq05.3-capable background work is queued.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplBackgroundQueuedEvidence {
    pub request_id: ReplBackgroundRequestId,
    pub request: ReplBackgroundRequest,
}

/// Evidence for `:help`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplHelpEvidence {
    pub topic: Option<String>,
    pub commands: Vec<&'static str>,
}

/// Evidence returned when read-only trace mode rejects a cell submission.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCellSubmissionEvidence {
    pub source_len: usize,
    pub policy: ReplTracePolicy,
}

/// Request passed to seq05.3 or an out-of-crate background tiering adapter.
#[derive(Clone, Debug, PartialEq)]
pub enum ReplBackgroundRequest {
    Warm {
        command: WarmCommand,
        generation: ReplGenerationEvidence,
    },
    Codegen {
        command: CodegenCommand,
        generation: ReplGenerationEvidence,
    },
}

impl ReplCommandId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }
}

impl ReplBackgroundRequestId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for StepCommand {
    fn default() -> Self {
        Self { frames: 1 }
    }
}

impl ReplCommand {
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Observe(_) => ":observe",
            Self::Step(_) => ":step",
            Self::Tasks(_) => ":tasks",
            Self::Cancel(_) => ":cancel",
            Self::Load(_) => ":load",
            Self::Reload(_) => ":reload",
            Self::Cells(_) => ":cells",
            Self::Undo(_) => ":undo",
            Self::Reset(_) => ":reset",
            Self::Capabilities(_) => ":capabilities",
            Self::Generations(_) => ":generations",
            Self::Warm(_) => ":warm",
            Self::Codegen(_) => ":codegen",
            Self::Help(_) => ":help",
            Self::Quit => ":quit",
        }
    }

    #[must_use]
    pub const fn effect(&self) -> ReplCommandEffect {
        match self {
            Self::Observe(_) | Self::Step(_) => ReplCommandEffect::HostRead,
            Self::Tasks(_)
            | Self::Cells(_)
            | Self::Capabilities(_)
            | Self::Generations(_)
            | Self::Help(_) => ReplCommandEffect::ReadOnly,
            Self::Load(_) | Self::Reload(_) | Self::Undo(_) | Self::Reset(_) => {
                ReplCommandEffect::SessionMutation
            }
            Self::Cancel(_) => ReplCommandEffect::HostMutation,
            Self::Warm(_) | Self::Codegen(_) => ReplCommandEffect::BackgroundMutation,
            Self::Quit => ReplCommandEffect::Exit,
        }
    }
}

impl From<ReplUndoOutcome> for ReplUndoSummary {
    fn from(outcome: ReplUndoOutcome) -> Self {
        Self {
            removed_cell_id: outcome.removed.id,
            removed_cell_kind: outcome.removed.kind,
            removed_source_hash: outcome.removed.source_hash,
            removed_binding_count: outcome.removed.bindings.len(),
            remaining_cells: outcome.remaining_cells,
            overlay_hash: outcome.overlay_hash,
        }
    }
}

impl From<ReplResetOutcome> for ReplResetSummary {
    fn from(outcome: ReplResetOutcome) -> Self {
        Self {
            removed_cells: outcome.removed_cells,
            retained_generation: outcome.retained_generation,
            overlay_hash: outcome.overlay_hash,
        }
    }
}

impl ReplTracePolicy {
    #[must_use]
    pub const fn permits_command(self, effect: ReplCommandEffect) -> bool {
        match self {
            Self::ReadWrite => true,
            Self::ReadOnlyTrace => matches!(
                effect,
                ReplCommandEffect::ReadOnly | ReplCommandEffect::HostRead | ReplCommandEffect::Exit
            ),
        }
    }

    #[must_use]
    pub const fn permits_cell_submission(self) -> bool {
        matches!(self, Self::ReadWrite)
    }
}

impl ReplCommandDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmptyCommand => "empty_command",
            Self::UnknownCommand => "unknown_command",
            Self::MissingArgument => "missing_argument",
            Self::InvalidArgument => "invalid_argument",
            Self::UnexpectedArgument => "unexpected_argument",
            Self::ReadOnlyTraceRejected => "read_only_trace_rejected",
            Self::HostUnavailable => "host_unavailable",
            Self::HostError => "host_error",
            Self::ProjectLoaderUnavailable => "project_loader_unavailable",
            Self::ProjectLoaderError => "project_loader_error",
            Self::SessionError => "session_error",
            Self::UnhandledExtension => "unhandled_extension",
            Self::TieringUnavailable => "tiering_unavailable",
        }
    }
}

impl ReplCommandDiagnostic {
    #[must_use]
    pub fn error(code: ReplCommandDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: ReplCommandDiagnosticSeverity::Error,
            code,
            message: message.into(),
            field: None,
        }
    }

    #[must_use]
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }
}

impl ReplCommandResult {
    #[must_use]
    pub fn ok(command_id: ReplCommandId, evidence: ReplCommandEvidence) -> Self {
        Self {
            command_id,
            status: ReplCommandStatus::Ok,
            evidence,
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn queued(command_id: ReplCommandId, evidence: ReplCommandEvidence) -> Self {
        Self {
            command_id,
            status: ReplCommandStatus::Queued,
            evidence,
            diagnostics: Vec::new(),
        }
    }

    #[must_use]
    pub fn rejected(
        command_id: ReplCommandId,
        evidence: ReplCommandEvidence,
        diagnostic: ReplCommandDiagnostic,
    ) -> Self {
        Self {
            command_id,
            status: ReplCommandStatus::Rejected,
            evidence,
            diagnostics: vec![diagnostic],
        }
    }

    #[must_use]
    pub fn error(
        command_id: ReplCommandId,
        evidence: ReplCommandEvidence,
        diagnostic: ReplCommandDiagnostic,
    ) -> Self {
        Self {
            command_id,
            status: ReplCommandStatus::Error,
            evidence,
            diagnostics: vec![diagnostic],
        }
    }

    #[must_use]
    pub fn exit_requested(command_id: ReplCommandId) -> Self {
        Self {
            command_id,
            status: ReplCommandStatus::ExitRequested,
            evidence: ReplCommandEvidence::Quit,
            diagnostics: Vec::new(),
        }
    }
}

impl ReplObservationEvidence {
    #[must_use]
    pub fn from_observation(request: ObserveRequest, observation: ObservationEnvelope) -> Self {
        let action_count = observation.actions.len();
        let signal_count = observation.signals.len();
        Self {
            request,
            tick: observation.tick,
            frame_id: observation.frame_id,
            state_hash: observation.state_hash,
            render_hash: observation.render_hash,
            action_count,
            signal_count,
        }
    }
}
