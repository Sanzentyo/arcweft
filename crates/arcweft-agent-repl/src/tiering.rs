use arcweft_agent_runner::runner::AgentControllerExecutorTierPolicy;
use arcweft_core::awbc::schema::AwbcDigest;
use arcweft_runtime_codegen::policy::{
    RuntimeCodegenPolicy, RuntimeCodegenTarget, RuntimeExecutorKind, RuntimeOptimizationLevel,
};

use crate::command::{
    BuiltinReplCommandHandler, CodegenCommand, ReplCommand, ReplCommandContext,
    ReplCommandDiagnostic, ReplCommandDiagnosticCode, ReplCommandEffect, ReplCommandEvidence,
    ReplCommandHandler, ReplCommandResult, ReplCommandStatus, ReplCommandTarget, WarmCommand,
};
use crate::{
    ReplCellId, ReplExecutableSnapshot, ReplGenerationEvidence, ReplGenerationId, ReplSession,
    ReplTierCursor, ReplTierInvalidationReason, ReplTierInvalidationToken, ReplTierStatusRecord,
};

/// REPL/dev executor and codegen policy.
///
/// The default status-only policy does not assume a full-script executable
/// backend. It lets `:warm` and `:codegen` return deterministic VM-fallback
/// status while immediate cell execution remains the bytecode VM.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierPolicy {
    pub executor: AgentControllerExecutorTierPolicy,
    pub codegen: RuntimeCodegenPolicy,
    pub enabled_backends: Vec<RuntimeExecutorKind>,
    pub allow_background_warm: bool,
    pub status_only_when_backend_missing: bool,
}

/// Stable request captured for one `:warm` command.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplWarmRequest {
    pub request_id: u64,
    pub target: ReplCommandTarget,
    pub snapshot: ReplExecutableSnapshot,
    pub generation: ReplGenerationEvidence,
    pub invalidations: Vec<ReplTierInvalidationToken>,
    pub policy_digest: String,
}

/// Stable outcome returned by a `:warm` command.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplWarmOutcome {
    pub request_id: u64,
    pub requested: bool,
    pub started_background_job: bool,
    pub target: ReplCommandTarget,
    pub backend_status: ReplTierBackendStatus,
    pub fallback: ReplTierFallback,
    pub reason: Option<ReplWarmUnsupportedReason>,
    pub generation: ReplGenerationId,
    pub overlay_hash: String,
    pub warmed_cells: Vec<ReplCellId>,
    pub warmed_regions: Vec<String>,
    pub invalidated_artifacts: Vec<String>,
    pub diagnostics: Vec<ReplTierDiagnostic>,
}

/// Stable status returned by `:codegen`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplCodegenStatus {
    pub requested: bool,
    pub backend_status: ReplTierBackendStatus,
    pub fallback: ReplTierFallback,
    pub enabled_backends: Vec<RuntimeExecutorKind>,
    pub warmed_generations: Vec<ReplGenerationId>,
    pub warmed_cells: Vec<ReplCellId>,
    pub warmed_regions: Vec<String>,
    pub pending_jobs: Vec<ReplPendingWarmJob>,
    pub failures: Vec<ReplTierDiagnostic>,
    pub invalidated_artifacts: Vec<String>,
    pub diagnostics: Vec<ReplTierDiagnostic>,
}

/// Combined snapshot for adapters that want one value instead of separate warm
/// and codegen projections.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierStatus {
    pub policy: ReplTierPolicy,
    pub codegen: ReplCodegenStatus,
    pub diagnostics: Vec<ReplTierDiagnostic>,
}

/// One deterministic tiering diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierDiagnostic {
    pub severity: ReplTierDiagnosticSeverity,
    pub code: ReplTierDiagnosticCode,
    pub message: String,
    pub cell_id: Option<ReplCellId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTierDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTierDiagnosticCode {
    BackgroundDisabled,
    FullScriptBackendNotAvailable,
    NoExecutableCells,
    TargetNotFound,
    SelectorUnsupported,
    StaleGeneration,
    PolicyChanged,
    BackendFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTierBackendStatus {
    Unsupported,
    StatusOnly,
    Queued,
    Running,
    Ready,
    Failed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplWarmUnsupportedReason {
    TieringDisabled,
    FullScriptBackendNotAvailable,
    NoExecutableCells,
    TargetNotFound,
    SelectorUnsupported,
    StaleGeneration,
    CapabilityOrEffectPolicyChanged,
    BackendFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplTierFallback {
    BytecodeVm,
    None,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplPendingWarmJob {
    pub request_id: u64,
    pub generation: ReplGenerationId,
    pub overlay_hash: String,
    pub cells: Vec<ReplCellId>,
    pub status: ReplTierBackendStatus,
}

/// In-memory status-only tiering manager used by seq05.3 command handlers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReplTierManager {
    policy: ReplTierPolicy,
    next_request_id: u64,
    observed_cursor: ReplTierCursor,
    pending_jobs: Vec<ReplPendingWarmJob>,
    warmed_generations: Vec<ReplGenerationId>,
    warmed_cells: Vec<ReplCellId>,
    warmed_regions: Vec<String>,
    failures: Vec<ReplTierDiagnostic>,
    invalidated_artifacts: Vec<String>,
}

/// Command handler that consumes seq05.2 `:warm` / `:codegen` hooks and returns
/// concrete seq05.3 status. Other commands are delegated to the composed handler.
pub struct ReplTierCommandHandler<H = BuiltinReplCommandHandler> {
    manager: ReplTierManager,
    fallback: H,
}

impl ReplTierPolicy {
    #[must_use]
    pub fn status_only() -> Self {
        Self::status_only_for_target(RuntimeCodegenTarget {
            triple: "status-only".to_owned(),
            cpu_features_digest: AwbcDigest::default(),
            wasm_features_digest: None,
        })
    }

    #[must_use]
    pub fn status_only_for_target(target: RuntimeCodegenTarget) -> Self {
        Self {
            executor: AgentControllerExecutorTierPolicy::bytecode_vm_first(),
            codegen: RuntimeCodegenPolicy {
                preferred: RuntimeExecutorKind::CompactVm,
                allow_vm_fallback: true,
                optimization: RuntimeOptimizationLevel::Baseline,
                target,
            },
            enabled_backends: Vec::new(),
            allow_background_warm: true,
            status_only_when_backend_missing: true,
        }
    }

    #[must_use]
    pub fn with_enabled_backends(mut self, enabled_backends: Vec<RuntimeExecutorKind>) -> Self {
        self.enabled_backends = enabled_backends;
        self
    }

    #[must_use]
    pub fn has_full_script_backend(&self) -> bool {
        self.enabled_backends
            .iter()
            .copied()
            .any(RuntimeExecutorKind::is_compiled_backend)
    }

    #[must_use]
    pub fn backend_status(&self) -> ReplTierBackendStatus {
        if self.has_full_script_backend() {
            ReplTierBackendStatus::StatusOnly
        } else {
            ReplTierBackendStatus::Unsupported
        }
    }

    #[must_use]
    pub fn deterministic_digest(&self) -> String {
        let backends = self
            .enabled_backends
            .iter()
            .map(|backend| backend.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "executor={};preferred={};fallback={};opt={};target={};backends={}",
            self.executor.requested_tier().as_str(),
            self.codegen.preferred.as_str(),
            self.codegen.allow_vm_fallback,
            self.codegen.optimization.as_str(),
            &self.codegen.target.triple,
            backends
        )
    }
}

impl Default for ReplTierPolicy {
    fn default() -> Self {
        Self::status_only()
    }
}

impl ReplWarmRequest {
    #[must_use]
    pub fn new(
        request_id: u64,
        target: ReplCommandTarget,
        snapshot: ReplExecutableSnapshot,
        generation: ReplGenerationEvidence,
        invalidations: Vec<ReplTierInvalidationToken>,
        policy_digest: String,
    ) -> Self {
        Self {
            request_id,
            target,
            snapshot,
            generation,
            invalidations,
            policy_digest,
        }
    }
}

impl ReplWarmOutcome {
    #[must_use]
    pub fn status_record(&self) -> ReplTierStatusRecord {
        let cell_id = match self.warmed_cells.as_slice() {
            [cell_id] => Some(*cell_id),
            [] | [_, ..] => None,
        };
        ReplTierStatusRecord {
            generation: self.generation,
            overlay_hash: self.overlay_hash.clone(),
            cell_id,
            tier: self.fallback.as_str().to_owned(),
            status: self.backend_status.as_str().to_owned(),
            detail: self.reason.map(|reason| reason.as_str().to_owned()),
        }
    }
}

impl ReplCodegenStatus {
    #[must_use]
    pub fn status_record(&self, generation: &ReplGenerationEvidence) -> ReplTierStatusRecord {
        ReplTierStatusRecord {
            generation: generation.active_generation,
            overlay_hash: generation.overlay_hash.clone(),
            cell_id: None,
            tier: self.fallback.as_str().to_owned(),
            status: self.backend_status.as_str().to_owned(),
            detail: self
                .failures
                .first()
                .map(|failure| failure.code.as_str().to_owned()),
        }
    }
}

impl ReplTierDiagnostic {
    #[must_use]
    pub fn warning(code: ReplTierDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: ReplTierDiagnosticSeverity::Warning,
            code,
            message: message.into(),
            cell_id: None,
        }
    }

    #[must_use]
    pub fn error(code: ReplTierDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            severity: ReplTierDiagnosticSeverity::Error,
            code,
            message: message.into(),
            cell_id: None,
        }
    }
}

impl ReplTierDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BackgroundDisabled => "background_disabled",
            Self::FullScriptBackendNotAvailable => "full_script_backend_not_available",
            Self::NoExecutableCells => "no_executable_cells",
            Self::TargetNotFound => "target_not_found",
            Self::SelectorUnsupported => "selector_unsupported",
            Self::StaleGeneration => "stale_generation",
            Self::PolicyChanged => "policy_changed",
            Self::BackendFailed => "backend_failed",
        }
    }
}

impl ReplTierBackendStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::StatusOnly => "status_only",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }
}

impl ReplWarmUnsupportedReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TieringDisabled => "tiering_disabled",
            Self::FullScriptBackendNotAvailable => "full_script_backend_not_available",
            Self::NoExecutableCells => "no_executable_cells",
            Self::TargetNotFound => "target_not_found",
            Self::SelectorUnsupported => "selector_unsupported",
            Self::StaleGeneration => "stale_generation",
            Self::CapabilityOrEffectPolicyChanged => "capability_or_effect_policy_changed",
            Self::BackendFailed => "backend_failed",
        }
    }
}

impl ReplTierFallback {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BytecodeVm => "bytecode_vm",
            Self::None => "none",
        }
    }
}

impl ReplTierManager {
    #[must_use]
    pub fn new(policy: ReplTierPolicy) -> Self {
        Self {
            policy,
            next_request_id: 1,
            observed_cursor: ReplTierCursor::default(),
            pending_jobs: Vec::new(),
            warmed_generations: Vec::new(),
            warmed_cells: Vec::new(),
            warmed_regions: Vec::new(),
            failures: Vec::new(),
            invalidated_artifacts: Vec::new(),
        }
    }

    #[must_use]
    pub fn policy(&self) -> &ReplTierPolicy {
        &self.policy
    }

    pub fn allocate_request_id(&mut self) -> u64 {
        let id = self.next_request_id;
        self.next_request_id = self.next_request_id.saturating_add(1);
        id
    }

    #[must_use]
    pub fn codegen_status(
        &self,
        requested: bool,
        invalidations: &[ReplTierInvalidationToken],
    ) -> ReplCodegenStatus {
        let invalidated_artifacts = Self::invalidated_artifacts_from_tokens(invalidations);
        let mut failures = self.failures.clone();
        if !self.policy.has_full_script_backend() {
            failures.push(full_script_backend_not_available());
        }
        ReplCodegenStatus {
            requested,
            backend_status: self.policy.backend_status(),
            fallback: ReplTierFallback::BytecodeVm,
            enabled_backends: self.policy.enabled_backends.clone(),
            warmed_generations: self.warmed_generations.clone(),
            warmed_cells: self.warmed_cells.clone(),
            warmed_regions: self.warmed_regions.clone(),
            pending_jobs: self.pending_jobs.clone(),
            failures: failures.clone(),
            invalidated_artifacts,
            diagnostics: failures,
        }
    }

    pub fn warm(&mut self, request: ReplWarmRequest) -> ReplWarmOutcome {
        let invalidated_artifacts = Self::invalidated_artifacts_from_tokens(&request.invalidations);
        if !self.policy.allow_background_warm {
            return self.unsupported_outcome(
                request,
                ReplWarmUnsupportedReason::TieringDisabled,
                ReplTierDiagnostic::warning(
                    ReplTierDiagnosticCode::BackgroundDisabled,
                    "REPL tier warm requests are disabled by policy",
                ),
                invalidated_artifacts,
            );
        }
        if !self.policy.has_full_script_backend() {
            return self.unsupported_outcome(
                request,
                ReplWarmUnsupportedReason::FullScriptBackendNotAvailable,
                full_script_backend_not_available(),
                invalidated_artifacts,
            );
        }
        let cells = match Self::resolve_target_cells(&request) {
            Ok(cells) if cells.is_empty() => {
                return self.unsupported_outcome(
                    request,
                    ReplWarmUnsupportedReason::NoExecutableCells,
                    ReplTierDiagnostic::warning(
                        ReplTierDiagnosticCode::NoExecutableCells,
                        "no executed REPL cells are available for warming",
                    ),
                    invalidated_artifacts,
                );
            }
            Ok(cells) => cells,
            Err(reason) => {
                return self.unsupported_outcome(
                    request,
                    reason,
                    diagnostic_for_reason(reason),
                    invalidated_artifacts,
                );
            }
        };
        let pending = ReplPendingWarmJob {
            request_id: request.request_id,
            generation: request.generation.active_generation,
            overlay_hash: request.snapshot.overlay_hash.clone(),
            cells,
            status: ReplTierBackendStatus::Queued,
        };
        self.pending_jobs.push(pending.clone());
        ReplWarmOutcome {
            request_id: request.request_id,
            requested: true,
            started_background_job: true,
            target: request.target,
            backend_status: ReplTierBackendStatus::Queued,
            fallback: ReplTierFallback::BytecodeVm,
            reason: None,
            generation: request.generation.active_generation,
            overlay_hash: request.snapshot.overlay_hash,
            warmed_cells: pending.cells,
            warmed_regions: Vec::new(),
            invalidated_artifacts,
            diagnostics: Vec::new(),
        }
    }

    pub fn collect_invalidations(
        &mut self,
        session: &ReplSession,
    ) -> Vec<ReplTierInvalidationToken> {
        let invalidations = session.tier_invalidation_tokens_since(self.observed_cursor);
        self.observed_cursor = invalidations.last().map_or_else(
            || {
                ReplTierCursor::new(
                    u64::try_from(session.generation_evidence().invalidation_events)
                        .unwrap_or(u64::MAX),
                )
            },
            |token| token.cursor,
        );
        invalidations
    }

    #[must_use]
    pub fn status(&self, _generation: ReplGenerationEvidence) -> ReplTierStatus {
        let codegen = self.codegen_status(false, &[]);
        ReplTierStatus {
            policy: self.policy.clone(),
            diagnostics: codegen.diagnostics.clone(),
            codegen,
        }
    }

    fn unsupported_outcome(
        &mut self,
        request: ReplWarmRequest,
        reason: ReplWarmUnsupportedReason,
        diagnostic: ReplTierDiagnostic,
        invalidated_artifacts: Vec<String>,
    ) -> ReplWarmOutcome {
        self.failures.push(diagnostic.clone());
        ReplWarmOutcome {
            request_id: request.request_id,
            requested: true,
            started_background_job: false,
            target: request.target,
            backend_status: ReplTierBackendStatus::Unsupported,
            fallback: ReplTierFallback::BytecodeVm,
            reason: Some(reason),
            generation: request.generation.active_generation,
            overlay_hash: request.snapshot.overlay_hash,
            warmed_cells: Vec::new(),
            warmed_regions: Vec::new(),
            invalidated_artifacts,
            diagnostics: vec![diagnostic],
        }
    }

    fn resolve_target_cells(
        request: &ReplWarmRequest,
    ) -> Result<Vec<ReplCellId>, ReplWarmUnsupportedReason> {
        match &request.target {
            ReplCommandTarget::All => Ok(request
                .snapshot
                .cells
                .iter()
                .map(|cell| cell.cell_id)
                .collect()),
            ReplCommandTarget::Latest => Ok(request
                .snapshot
                .cells
                .last()
                .map_or_else(Vec::new, |cell| vec![cell.cell_id])),
            ReplCommandTarget::Cell(id) => request
                .snapshot
                .cells
                .iter()
                .any(|cell| cell.cell_id == *id)
                .then_some(vec![*id])
                .ok_or(ReplWarmUnsupportedReason::TargetNotFound),
            ReplCommandTarget::Selector(_) => Err(ReplWarmUnsupportedReason::SelectorUnsupported),
        }
    }

    fn invalidated_artifacts_from_tokens(tokens: &[ReplTierInvalidationToken]) -> Vec<String> {
        tokens
            .iter()
            .filter(|token| invalidates_compiled_artifacts(token.reason))
            .map(|token| {
                format!(
                    "cursor={};reason={};generation={};overlay={}",
                    token.cursor.as_u64(),
                    invalidation_reason_label(token.reason),
                    token.generation.as_u64(),
                    token.overlay_hash
                )
            })
            .collect()
    }
}

impl Default for ReplTierManager {
    fn default() -> Self {
        Self::new(ReplTierPolicy::default())
    }
}

impl<H> ReplTierCommandHandler<H> {
    #[must_use]
    pub fn new(manager: ReplTierManager, fallback: H) -> Self {
        Self { manager, fallback }
    }

    #[must_use]
    pub fn manager(&self) -> &ReplTierManager {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut ReplTierManager {
        &mut self.manager
    }
}

impl Default for ReplTierCommandHandler<BuiltinReplCommandHandler> {
    fn default() -> Self {
        Self::new(ReplTierManager::default(), BuiltinReplCommandHandler)
    }
}

impl<H> ReplCommandHandler for ReplTierCommandHandler<H>
where
    H: ReplCommandHandler,
{
    fn handle(
        &mut self,
        context: &mut ReplCommandContext<'_>,
        command: ReplCommand,
    ) -> ReplCommandResult {
        match command {
            ReplCommand::Warm(command) => self.handle_warm(context, command),
            ReplCommand::Codegen(command) => self.handle_codegen(context, command),
            other => self.fallback.handle(context, other),
        }
    }
}

impl<H> ReplTierCommandHandler<H> {
    fn handle_warm(
        &mut self,
        context: &mut ReplCommandContext<'_>,
        command: WarmCommand,
    ) -> ReplCommandResult {
        let command_id = context.allocate_command_id();
        if !context
            .trace_policy()
            .permits_command(ReplCommandEffect::BackgroundMutation)
        {
            return read_only_rejected(command_id, ":warm");
        }
        let request_id = self.manager.allocate_request_id();
        let snapshot = context.session().executable_snapshot();
        let generation = context.session().generation_evidence();
        let invalidations = self.manager.collect_invalidations(context.session());
        let request = ReplWarmRequest::new(
            request_id,
            command.target,
            snapshot,
            generation,
            invalidations,
            self.manager.policy.deterministic_digest(),
        );
        let outcome = self.manager.warm(request);
        context
            .session_mut()
            .record_tier_status(outcome.status_record());
        if outcome.started_background_job {
            ReplCommandResult::queued(command_id, ReplCommandEvidence::Warm(outcome))
        } else {
            ReplCommandResult::ok(command_id, ReplCommandEvidence::Warm(outcome))
        }
    }

    fn handle_codegen(
        &mut self,
        context: &mut ReplCommandContext<'_>,
        _command: CodegenCommand,
    ) -> ReplCommandResult {
        let command_id = context.allocate_command_id();
        if !context
            .trace_policy()
            .permits_command(ReplCommandEffect::BackgroundMutation)
        {
            return read_only_rejected(command_id, ":codegen");
        }
        let generation = context.session().generation_evidence();
        let invalidations = self.manager.collect_invalidations(context.session());
        let status = self.manager.codegen_status(true, &invalidations);
        context
            .session_mut()
            .record_tier_status(status.status_record(&generation));
        ReplCommandResult {
            command_id,
            status: ReplCommandStatus::Ok,
            evidence: ReplCommandEvidence::Codegen(status),
            diagnostics: Vec::new(),
        }
    }
}

fn full_script_backend_not_available() -> ReplTierDiagnostic {
    ReplTierDiagnostic::warning(
        ReplTierDiagnosticCode::FullScriptBackendNotAvailable,
        "no full-script codegen backend is available; committed cells keep executing through bytecode VM",
    )
}

fn diagnostic_for_reason(reason: ReplWarmUnsupportedReason) -> ReplTierDiagnostic {
    match reason {
        ReplWarmUnsupportedReason::TieringDisabled => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::BackgroundDisabled,
            "background tiering is disabled",
        ),
        ReplWarmUnsupportedReason::FullScriptBackendNotAvailable => {
            full_script_backend_not_available()
        }
        ReplWarmUnsupportedReason::NoExecutableCells => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::NoExecutableCells,
            "no executable cells matched the warm target",
        ),
        ReplWarmUnsupportedReason::TargetNotFound => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::TargetNotFound,
            "the requested REPL cell is not present in the executable snapshot",
        ),
        ReplWarmUnsupportedReason::SelectorUnsupported => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::SelectorUnsupported,
            "selector targets are parsed but not yet supported by the tiering backend",
        ),
        ReplWarmUnsupportedReason::StaleGeneration => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::StaleGeneration,
            "warm request generation is stale",
        ),
        ReplWarmUnsupportedReason::CapabilityOrEffectPolicyChanged => ReplTierDiagnostic::warning(
            ReplTierDiagnosticCode::PolicyChanged,
            "capability or effect policy changed since the request was captured",
        ),
        ReplWarmUnsupportedReason::BackendFailed => ReplTierDiagnostic::error(
            ReplTierDiagnosticCode::BackendFailed,
            "codegen backend failed; bytecode VM fallback remains active",
        ),
    }
}

fn invalidates_compiled_artifacts(reason: ReplTierInvalidationReason) -> bool {
    matches!(
        reason,
        ReplTierInvalidationReason::CellCommitted
            | ReplTierInvalidationReason::CellExecutionFailed
            | ReplTierInvalidationReason::CellUndone
            | ReplTierInvalidationReason::ResetToBase
            | ReplTierInvalidationReason::BaseProjectChanged
            | ReplTierInvalidationReason::GenerationChanged
    )
}

fn invalidation_reason_label(reason: ReplTierInvalidationReason) -> &'static str {
    match reason {
        ReplTierInvalidationReason::CellCommitted => "cell_committed",
        ReplTierInvalidationReason::CellExecutionFailed => "cell_execution_failed",
        ReplTierInvalidationReason::CellUndone => "cell_undone",
        ReplTierInvalidationReason::ResetToBase => "reset_to_base",
        ReplTierInvalidationReason::BaseProjectChanged => "base_project_changed",
        ReplTierInvalidationReason::GenerationChanged => "generation_changed",
        ReplTierInvalidationReason::TierStatusRecorded => "tier_status_recorded",
    }
}

fn read_only_rejected(
    command_id: crate::command::ReplCommandId,
    command: &str,
) -> ReplCommandResult {
    ReplCommandResult::rejected(
        command_id,
        ReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(
            ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
            format!("read-only trace mode rejects mutating command `{command}`"),
        ),
    )
}
