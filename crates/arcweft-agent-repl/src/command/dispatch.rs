use crate::{
    ReplCellFilter, ReplCellInput, ReplResetOptions, ReplSession, ReplTierCursor,
    ReplTransactionError, ReplUndoOptions,
};

use super::host::{ReplCommandHost, ReplCommandHostError, ReplProjectLoader};
use super::parse::repl_command_names;
use super::types::{
    CancelCommand, CellsCommand, CodegenCommand, GenerationsCommand, LoadCommand, ObserveCommand,
    ReloadCommand, ReplBackgroundQueuedEvidence, ReplBackgroundRequest, ReplBackgroundRequestId,
    ReplCancelEvidence, ReplCellSubmissionEvidence, ReplCommand, ReplCommandDiagnostic,
    ReplCommandDiagnosticCode, ReplCommandEvidence, ReplCommandId, ReplCommandResult,
    ReplGenerationCommandEvidence, ReplHelpEvidence, ReplLoadEvidence, ReplObservationEvidence,
    ReplReloadEvidence, ReplResetEvidence, ReplResetSummary, ReplStepEvidence, ReplTasksEvidence,
    ReplTracePolicy, ReplUndoEvidence, ReplUndoSummary, ResetCommand, StepCommand, TasksCommand,
    UndoCommand, WarmCommand,
};

/// Seq05.3 background extension point for `:warm` and `:codegen`.
pub trait ReplBackgroundRequestSink {
    fn enqueue(&mut self, request: ReplBackgroundRequest) -> ReplBackgroundRequestId;
}

/// Typed command handler extension point. Seq05.3 can wrap or compose the builtin
/// handler rather than editing a stringly command table.
pub trait ReplCommandHandler {
    fn handle(
        &mut self,
        context: &mut ReplCommandContext<'_>,
        command: ReplCommand,
    ) -> ReplCommandResult;
}

/// Command dispatch context. It borrows the seq05.1 session and optional host
/// adapters without taking ownership of either.
pub struct ReplCommandContext<'a> {
    session: &'a mut ReplSession,
    host: Option<&'a mut dyn ReplCommandHost>,
    loader: Option<&'a mut dyn ReplProjectLoader>,
    background: Option<&'a mut dyn ReplBackgroundRequestSink>,
    trace_policy: ReplTracePolicy,
    next_command_id: ReplCommandId,
}

/// Builtin seq05.2 handler for all non-tiering commands.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BuiltinReplCommandHandler;

impl<'a> ReplCommandContext<'a> {
    #[must_use]
    pub fn new(session: &'a mut ReplSession) -> Self {
        Self {
            session,
            host: None,
            loader: None,
            background: None,
            trace_policy: ReplTracePolicy::ReadWrite,
            next_command_id: ReplCommandId::new(1),
        }
    }

    #[must_use]
    pub fn with_host(mut self, host: &'a mut dyn ReplCommandHost) -> Self {
        self.host = Some(host);
        self
    }

    #[must_use]
    pub fn with_loader(mut self, loader: &'a mut dyn ReplProjectLoader) -> Self {
        self.loader = Some(loader);
        self
    }

    #[must_use]
    pub fn with_background(mut self, background: &'a mut dyn ReplBackgroundRequestSink) -> Self {
        self.background = Some(background);
        self
    }

    #[must_use]
    pub fn with_trace_policy(mut self, trace_policy: ReplTracePolicy) -> Self {
        self.trace_policy = trace_policy;
        self
    }

    #[must_use]
    pub fn trace_policy(&self) -> ReplTracePolicy {
        self.trace_policy
    }

    #[must_use]
    pub fn session(&self) -> &ReplSession {
        &*self.session
    }

    pub fn session_mut(&mut self) -> &mut ReplSession {
        &mut *self.session
    }

    pub fn allocate_command_id(&mut self) -> ReplCommandId {
        let id = self.next_command_id;
        self.next_command_id = id.next();
        id
    }

    pub fn reject_cell_submission_if_read_only(
        &mut self,
        input: &ReplCellInput,
    ) -> Option<ReplCommandResult> {
        if self.trace_policy.permits_cell_submission() {
            return None;
        }
        let command_id = self.allocate_command_id();
        Some(ReplCommandResult::rejected(
            command_id,
            ReplCommandEvidence::CellSubmissionRejected(ReplCellSubmissionEvidence {
                source_len: input.source_text().len(),
                policy: self.trace_policy,
            }),
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                "read-only trace mode does not allow cell execution",
            ),
        ))
    }
}

impl ReplCommandHandler for BuiltinReplCommandHandler {
    fn handle(
        &mut self,
        context: &mut ReplCommandContext<'_>,
        command: ReplCommand,
    ) -> ReplCommandResult {
        let command_id = context.allocate_command_id();
        if !context.trace_policy.permits_command(command.effect()) {
            return ReplCommandResult::rejected(
                command_id,
                ReplCommandEvidence::Empty,
                ReplCommandDiagnostic::error(
                    ReplCommandDiagnosticCode::ReadOnlyTraceRejected,
                    format!(
                        "read-only trace mode rejects mutating command `{}`",
                        command.name()
                    ),
                ),
            );
        }

        match command {
            ReplCommand::Observe(command) => handle_observe(context, command_id, command),
            ReplCommand::Step(command) => handle_step(context, command_id, command),
            ReplCommand::Tasks(command) => handle_tasks(context, command_id, command),
            ReplCommand::Cancel(command) => handle_cancel(context, command_id, &command),
            ReplCommand::Load(command) => handle_load(context, command_id, command),
            ReplCommand::Reload(command) => handle_reload(context, command_id, command),
            ReplCommand::Cells(command) => handle_cells(context, command_id, command),
            ReplCommand::Undo(command) => handle_undo(context, command_id, command),
            ReplCommand::Reset(command) => handle_reset(context, command_id, command),
            ReplCommand::Capabilities(_) => handle_capabilities(context, command_id),
            ReplCommand::Generations(command) => handle_generations(context, command_id, command),
            ReplCommand::Warm(command) => handle_warm(context, command_id, command),
            ReplCommand::Codegen(command) => handle_codegen(context, command_id, command),
            ReplCommand::Help(command) => ReplCommandResult::ok(
                command_id,
                ReplCommandEvidence::Help(ReplHelpEvidence {
                    topic: command.topic,
                    commands: repl_command_names(),
                }),
            ),
            ReplCommand::Quit => ReplCommandResult::exit_requested(command_id),
        }
    }
}

fn handle_observe(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: ObserveCommand,
) -> ReplCommandResult {
    let Some(host) = context.host.as_deref_mut() else {
        return host_unavailable(command_id, ":observe requires a command host adapter");
    };
    match host.observe(&command) {
        Ok(observation) => ReplCommandResult::ok(
            command_id,
            ReplCommandEvidence::Observation(ReplObservationEvidence::from_observation(
                command.request,
                observation,
            )),
        ),
        Err(error) => host_error(command_id, error),
    }
}

fn handle_step(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: StepCommand,
) -> ReplCommandResult {
    let Some(host) = context.host.as_deref_mut() else {
        return host_unavailable(command_id, ":step requires a command host adapter");
    };
    match host.step(&command) {
        Ok(observation) => {
            let request = arcweft_agent_protocol::protocol::ObserveRequest::default();
            ReplCommandResult::ok(
                command_id,
                ReplCommandEvidence::Step(ReplStepEvidence {
                    frames: command.frames,
                    observation: ReplObservationEvidence::from_observation(request, observation),
                }),
            )
        }
        Err(error) => host_error(command_id, error),
    }
}

fn handle_tasks(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: TasksCommand,
) -> ReplCommandResult {
    let Some(host) = context.host.as_deref_mut() else {
        return host_unavailable(command_id, ":tasks requires a command host adapter");
    };
    match host.tasks(&command) {
        Ok(tasks) => ReplCommandResult::ok(
            command_id,
            ReplCommandEvidence::Tasks(ReplTasksEvidence {
                include_completed: command.include_completed,
                tasks,
            }),
        ),
        Err(error) => host_error(command_id, error),
    }
}

fn handle_cancel(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: &CancelCommand,
) -> ReplCommandResult {
    let Some(host) = context.host.as_deref_mut() else {
        return host_unavailable(command_id, ":cancel requires a command host adapter");
    };
    match host.cancel(command) {
        Ok(outcome) => ReplCommandResult::ok(
            command_id,
            ReplCommandEvidence::Cancel(ReplCancelEvidence { outcome }),
        ),
        Err(error) => host_error(command_id, error),
    }
}

fn handle_load(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: LoadCommand,
) -> ReplCommandResult {
    let Some(loader) = context.loader.as_deref_mut() else {
        return loader_unavailable(command_id, ":load requires a project loader adapter");
    };
    match loader.load(&command) {
        Ok(base) => {
            let base_label = base.label().to_owned();
            let outcome = context.session.replace_base_snapshot(base);
            ReplCommandResult::ok(
                command_id,
                ReplCommandEvidence::Load(ReplLoadEvidence {
                    path: command.path,
                    base_label,
                    outcome,
                }),
            )
        }
        Err(error) => loader_error(command_id, error),
    }
}

fn handle_reload(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: ReloadCommand,
) -> ReplCommandResult {
    let Some(loader) = context.loader.as_deref_mut() else {
        return loader_unavailable(command_id, ":reload requires a project loader adapter");
    };
    match loader.reload(&command) {
        Ok(base) => {
            let base_label = base.label().to_owned();
            let outcome = context.session.replace_base_snapshot(base);
            ReplCommandResult::ok(
                command_id,
                ReplCommandEvidence::Reload(ReplReloadEvidence {
                    path: command.path,
                    base_label,
                    outcome,
                }),
            )
        }
        Err(error) => loader_error(command_id, error),
    }
}

fn handle_cells(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: CellsCommand,
) -> ReplCommandResult {
    ReplCommandResult::ok(
        command_id,
        ReplCommandEvidence::Cells(context.session.cells(ReplCellFilter {
            include_invalidated: command.include_invalidated,
        })),
    )
}

fn handle_undo(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: UndoCommand,
) -> ReplCommandResult {
    let before = context.session.generation_evidence();
    let cursor = ReplTierCursor::new(u64::try_from(before.invalidation_events).unwrap_or(u64::MAX));
    match context.session.undo_latest_cell(ReplUndoOptions {
        preserve_execution_evidence: command.preserve_execution_evidence,
    }) {
        Ok(outcome) => ReplCommandResult::ok(
            command_id,
            ReplCommandEvidence::Undo(ReplUndoEvidence {
                summary: ReplUndoSummary::from(outcome),
                binding_evidence_after: context.session.binding_evidence(),
                generation_evidence_after: context.session.generation_evidence(),
                tier_invalidations: context.session.tier_invalidation_tokens_since(cursor),
            }),
        ),
        Err(error) => session_error(command_id, &error),
    }
}

fn handle_reset(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: ResetCommand,
) -> ReplCommandResult {
    let before = context.session.generation_evidence();
    let cursor = ReplTierCursor::new(u64::try_from(before.invalidation_events).unwrap_or(u64::MAX));
    let outcome = context.session.reset_to_base(ReplResetOptions {
        preserve_generation: command.preserve_generation,
    });
    ReplCommandResult::ok(
        command_id,
        ReplCommandEvidence::Reset(ReplResetEvidence {
            summary: ReplResetSummary::from(outcome),
            binding_evidence_after: context.session.binding_evidence(),
            generation_evidence_after: context.session.generation_evidence(),
            tier_invalidations: context.session.tier_invalidation_tokens_since(cursor),
        }),
    )
}

fn handle_capabilities(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
) -> ReplCommandResult {
    ReplCommandResult::ok(
        command_id,
        ReplCommandEvidence::Capabilities(context.session.capabilities()),
    )
}

fn handle_generations(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: GenerationsCommand,
) -> ReplCommandResult {
    ReplCommandResult::ok(
        command_id,
        ReplCommandEvidence::Generations(ReplGenerationCommandEvidence {
            generation: context.session.generation_evidence(),
            bindings: context.session.binding_evidence(),
            tiers: command.include_tiers.then(|| context.session.tier_status()),
        }),
    )
}

fn handle_warm(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: WarmCommand,
) -> ReplCommandResult {
    let generation = context.session.generation_evidence();
    let request = ReplBackgroundRequest::Warm {
        command,
        generation,
    };
    enqueue_background(context, command_id, request)
}

fn handle_codegen(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    command: CodegenCommand,
) -> ReplCommandResult {
    let generation = context.session.generation_evidence();
    let request = ReplBackgroundRequest::Codegen {
        command,
        generation,
    };
    enqueue_background(context, command_id, request)
}

fn enqueue_background(
    context: &mut ReplCommandContext<'_>,
    command_id: ReplCommandId,
    request: ReplBackgroundRequest,
) -> ReplCommandResult {
    let Some(background) = context.background.as_deref_mut() else {
        return ReplCommandResult::error(
            command_id,
            ReplCommandEvidence::Empty,
            ReplCommandDiagnostic::error(
                ReplCommandDiagnosticCode::UnhandledExtension,
                ":warm/:codegen require a seq05.3 background request sink",
            ),
        );
    };
    let request_id = background.enqueue(request.clone());
    ReplCommandResult::queued(
        command_id,
        ReplCommandEvidence::BackgroundQueued(ReplBackgroundQueuedEvidence {
            request_id,
            request,
        }),
    )
}

fn host_unavailable(command_id: ReplCommandId, message: &str) -> ReplCommandResult {
    ReplCommandResult::error(
        command_id,
        ReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::HostUnavailable, message),
    )
}

fn loader_unavailable(command_id: ReplCommandId, message: &str) -> ReplCommandResult {
    ReplCommandResult::error(
        command_id,
        ReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::ProjectLoaderUnavailable, message),
    )
}

fn host_error(command_id: ReplCommandId, error: ReplCommandHostError) -> ReplCommandResult {
    ReplCommandResult::error(
        command_id,
        ReplCommandEvidence::Empty,
        error.into_diagnostic(),
    )
}

fn loader_error(command_id: ReplCommandId, error: ReplCommandHostError) -> ReplCommandResult {
    ReplCommandResult::error(
        command_id,
        ReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::ProjectLoaderError, error.message),
    )
}

fn session_error(command_id: ReplCommandId, error: &ReplTransactionError) -> ReplCommandResult {
    ReplCommandResult::error(
        command_id,
        ReplCommandEvidence::Empty,
        ReplCommandDiagnostic::error(ReplCommandDiagnosticCode::SessionError, error.to_string()),
    )
}
