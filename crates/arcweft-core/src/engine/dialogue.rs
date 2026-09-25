//! Native dialogue execution around one revisioned activation transaction.
//!
//! Pre-reveal operations, lexical cleanup, line-task progression, host-command
//! outcomes, and result publication all mutate the same activation frame and
//! line ledger. Keeping their transitions together lets each step stage a
//! complete candidate before publishing requests or child fibers; the registry
//! and shared ledger remain separate owners in `store` and `line_task`.

mod store;

pub(in crate::engine) use store::{
    DialogueActivationFrame, DialogueActivationScope, DialogueActivationStore,
    DialogueActivationTransaction, DialogueCommitDisposition, DialogueLineTaskState,
    DialogueRuntimePhase, PendingActivationHostCall, PendingLineOperation,
};

use super::{
    Engine, RuntimeCallableValue, RuntimeDiagnostic, RuntimeEvalError, RuntimeLocalBinding,
    RuntimeStepOutput, RuntimeValue,
};
use crate::effect::{
    LineEffectRequest, RuntimeDropPolicy, RuntimeDropPolicyExpr, RuntimeEffectExpr,
    RuntimeEffectMaterializeError,
};
use crate::line_task::{
    LineRuntimeError, LineTaskLiveState, LineTaskReadyEvents, MAX_LINE_SCHEDULED_CALLBACKS,
    RuntimeCueLease, RuntimeCueOrigin, RuntimeDeferUnwindStep, RuntimeDialogueActivationState,
    RuntimeDialogueResultState, RuntimeHandleLeaseState, RuntimeHandleOwnerSlot,
    RuntimeHandleResource, RuntimeLineHandleSiteKind, RuntimeScheduledLineTask,
    RuntimeStageActorLease, RuntimeVoiceLease, ScopeExit, progress_live_line_task_group,
};
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{FlowEvent, FlowOp, RuntimeDeferOwner, RuntimeLineOperation};
use crate::presentation::{
    RuntimeCommandQueue, RuntimeDialogueVoiceState, RuntimeLineHostOutcome,
    RuntimeStageCommandOutcome, RuntimeVoiceCommandOutcome,
};
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{RuntimeDeferSiteId, RuntimeLineHandleToken, RuntimePlanTypeId};
use crate::value::RuntimeExprKind;
use crate::value::ownership::RuntimeOwnedSlotId;
use thiserror::Error;

type NativeDialogueActivationState = RuntimeDialogueActivationState<RuntimePlanTypeId>;

#[derive(Clone, Debug, Error, PartialEq)]
pub(crate) enum DialogueExecutionError {
    #[error(transparent)]
    Evaluation(#[from] RuntimeEvalError),
    #[error(transparent)]
    Pattern(#[from] crate::pattern::RuntimePatternMatchError),
    #[error(transparent)]
    Line(#[from] LineRuntimeError),
    #[error(transparent)]
    LineTaskCompletion(#[from] crate::line_task::LineTaskCompletionError),
    #[error("line-task child failed: {message}")]
    ChildFailed { message: String },
    #[error("dialogue activation host call failed: {message}")]
    HostCallFailed { message: String },
    #[error(transparent)]
    EffectMaterialization(#[from] RuntimeEffectMaterializeError),
    #[error("dialogue activation effect failed: {message}")]
    ActivationEffectFailed { message: String },
}

pub(super) struct DialogueLineTaskStart {
    pub(super) event: Option<FlowEvent>,
    pub(super) request_cancellation: bool,
    pub(super) group: crate::line_task::LineTaskGroup,
    pub(super) activation: crate::line_task::LineTaskActivation,
    pub(super) captures: Box<[RuntimeLocalBinding]>,
    pub(super) callbacks: Vec<(
        crate::runtime_id::RuntimeDialogueEffectSiteId,
        RuntimeCallableValue,
    )>,
}

pub(super) enum DialogueActivationStep {
    Continue,
    Reveal(DialogueLineTaskStart),
    Deferred(super::NativeLineTaskExecutionBatch),
    HostCall(crate::step::RuntimeHostCallRequest),
    Effect(LineEffectRequest),
}

enum ActivationScopeStep {
    Waiting,
    Deferred(super::NativeLineTaskExecutionBatch),
    Finished,
}

enum DialoguePublicationOutcome {
    Pending,
    Published,
}

impl Engine {
    pub(super) fn begin_dialogue_activation_transaction(
        &self,
        activation: &crate::runtime_id::DialogueActivationId,
    ) -> Result<DialogueActivationTransaction, DialogueExecutionError> {
        self.dialogue_activations
            .begin_transaction(activation)
            .map_err(Into::into)
    }

    pub(super) fn commit_dialogue_activation_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) -> Result<(), DialogueExecutionError> {
        let receipt = self
            .dialogue_activations
            .commit_transaction(transaction)
            .map_err(DialogueExecutionError::from)?;
        Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        Ok(())
    }

    fn commit_terminal_dialogue_activation_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) -> Result<DialogueCommitDisposition, DialogueExecutionError> {
        let receipt = self
            .dialogue_activations
            .commit_terminal_transaction(transaction)
            .map_err(DialogueExecutionError::from)?;
        let (line, disposition) = receipt.into_parts();
        Self::publish_dialogue_line_receipt(line, output);
        Ok(disposition)
    }

    fn commit_published_dialogue_activation_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) -> Result<DialogueCommitDisposition, DialogueExecutionError> {
        let receipt = self
            .dialogue_activations
            .commit_published_transaction(transaction)
            .map_err(DialogueExecutionError::from)?;
        let (line, disposition) = receipt.into_parts();
        Self::publish_dialogue_line_receipt(line, output);
        Ok(disposition)
    }

    pub(super) fn publish_dialogue_line_receipt(
        receipt: crate::line_task::RuntimeDialogueCommitReceipt,
        output: &mut RuntimeStepOutput,
    ) {
        output
            .requests
            .line_commands
            .extend(receipt.into_commands());
    }

    fn apply_dialogue_commit_disposition(&mut self, disposition: DialogueCommitDisposition) {
        match disposition {
            DialogueCommitDisposition::Published { resume, bindings } => {
                self.fiber.env.bind_all(bindings);
                self.fiber.cursor = resume;
                self.fiber.status = super::FlowFiberStatus::Running;
            }
            DialogueCommitDisposition::Failed { error } => {
                self.fiber.status = super::FlowFiberStatus::Failed(error.to_string());
            }
        }
    }

    pub(super) fn commit_and_suspend_dialogue(
        &mut self,
        transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
        step: DialogueActivationStep,
    ) {
        let activation = transaction.activation().clone();
        let (start, deferred_batch, host_call, effect) = match step {
            DialogueActivationStep::Continue => (None, None, None, None),
            DialogueActivationStep::Reveal(start) => (Some(start), None, None, None),
            DialogueActivationStep::Deferred(batch) => (None, Some(batch), None, None),
            DialogueActivationStep::HostCall(request) => (None, None, Some(request), None),
            DialogueActivationStep::Effect(effect) => (None, None, None, Some(effect)),
        };
        let (transaction, batch, event) = match start {
            Some(start) => {
                let mut candidate = transaction.clone();
                let mut batch = match self.prepare_line_task_commands(
                    &mut candidate,
                    &start.group,
                    start.activation,
                    &start.captures,
                    start.request_cancellation,
                ) {
                    Ok(batch) => batch,
                    Err(error) => {
                        self.begin_dialogue_failure(transaction, error, output);
                        return;
                    }
                };
                if let Err(error) =
                    self.stage_dialogue_effect_callbacks(&mut batch, &activation, &start.callbacks)
                {
                    self.begin_dialogue_failure(transaction, error.into(), output);
                    return;
                }
                (candidate, Some(batch), start.event)
            }
            None => (transaction, None, None),
        };
        match self.commit_dialogue_activation_transaction(transaction, output) {
            Ok(()) => {
                self.fiber.status = super::FlowFiberStatus::Dialogue(activation.clone());
            }
            Err(error) => {
                self.fail_eval(error, output);
                return;
            }
        }
        if let Some(batch) = batch {
            self.commit_line_task_execution_batch(batch);
        }
        if let Some(batch) = deferred_batch {
            self.commit_line_task_execution_batch(batch);
        }
        if let Some(request) = host_call {
            self.advance_host_call_sequence();
            output.requests.host_calls.push(request);
        }
        if let Some(effect) = effect {
            output.effects.line.push(effect);
        }
        if let Some(event) = event {
            output.flow_events.push(event);
        }
    }

    pub(super) fn begin_dialogue_failure(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        error: DialogueExecutionError,
        output: &mut RuntimeStepOutput,
    ) {
        let state = transaction.frame_mut();
        if state.failure.is_none() {
            output
                .diagnostics
                .push(RuntimeDiagnostic::new(error.to_string()));
            state.failure = Some(error);
        }
        state.phase = DialogueRuntimePhase::Closing;
        self.request_line_task_cancellation();
        self.resume_dialogue_failure_close(transaction, output);
    }

    fn prepare_dialogue_deferred_child(
        &self,
        transaction: &mut DialogueActivationTransaction,
        exit: ScopeExit,
    ) -> Result<Option<super::NativeLineTaskExecutionBatch>, DialogueExecutionError> {
        if transaction.line().deferred_exit().is_none()
            && transaction.line().deferred_registrations().is_empty()
        {
            return Ok(None);
        }
        let activation_id = transaction.activation().clone();
        let fixed_exit = transaction.line().deferred_exit().unwrap_or(exit);
        transaction.line_mut().begin_deferred_unwind(fixed_exit)?;
        if transaction.line().deferred_inflight().is_some()
            || transaction.line().has_pending_commands()
        {
            return Ok(None);
        }
        loop {
            match transaction
                .line_mut()
                .prepare_next_deferred(&activation_id)?
            {
                Some(RuntimeDeferUnwindStep::Run(registration)) => {
                    return self
                        .prepare_deferred_line_child(&activation_id, registration)
                        .map(Some);
                }
                Some(RuntimeDeferUnwindStep::Skipped(id)) => {
                    if transaction
                        .line()
                        .deferred_registrations()
                        .last()
                        .is_some_and(|pending| pending.id() >= id)
                    {
                        return Err(LineRuntimeError::InvalidDeferredTransition.into());
                    }
                    if transaction.line().has_pending_commands() {
                        return Ok(None);
                    }
                }
                None => return Ok(None),
            }
        }
    }

    pub(super) fn resume_dialogue_failure_close(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) {
        let activation_id = transaction.activation().clone();
        if let Err(error) = transaction.line_mut().abandon() {
            self.fail_eval(error, output);
            return;
        }
        if !self.has_joined_work() {
            while !transaction.frame().scopes.is_empty() {
                let (frame, line) = transaction.parts_mut();
                let step = match self.prepare_activation_scope_exit(
                    &activation_id,
                    frame,
                    line,
                    ScopeExit::Failed,
                ) {
                    Ok(step) => step,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
                match step {
                    ActivationScopeStep::Finished => {}
                    ActivationScopeStep::Waiting => {
                        match self.commit_dialogue_activation_transaction(transaction, output) {
                            Ok(()) => {
                                self.fiber.status = super::FlowFiberStatus::Dialogue(activation_id);
                            }
                            Err(error) => self.fail_eval(error, output),
                        }
                        return;
                    }
                    ActivationScopeStep::Deferred(batch) => {
                        match self.commit_dialogue_activation_transaction(transaction, output) {
                            Ok(()) => {
                                self.fiber.status = super::FlowFiberStatus::Dialogue(activation_id);
                                self.commit_line_task_execution_batch(batch);
                            }
                            Err(error) => self.fail_eval(error, output),
                        }
                        return;
                    }
                }
            }
            let batch =
                match self.prepare_dialogue_deferred_child(&mut transaction, ScopeExit::Failed) {
                    Ok(batch) => batch,
                    Err(error) => {
                        self.fail_eval(error, output);
                        return;
                    }
                };
            if let Some(batch) = batch {
                match self.commit_dialogue_activation_transaction(transaction, output) {
                    Ok(()) => {
                        self.fiber.status = super::FlowFiberStatus::Dialogue(activation_id);
                        self.commit_line_task_execution_batch(batch);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
                return;
            }
            let activation = transaction.line_mut();
            if activation.deferred_inflight().is_none()
                && activation.deferred_registrations().is_empty()
                && !activation.has_pending_commands()
                && let Err(cleanup) =
                    Self::unwind_dialogue_handles(&activation_id, activation, false)
            {
                output.diagnostics.push(RuntimeDiagnostic::new(format!(
                    "dialogue cleanup after primary failure also failed: {cleanup}"
                )));
            }
        }
        let (state, activation) = transaction.parts_mut();
        let terminal = activation.failure_close_ready() && !self.has_joined_work();
        if terminal {
            if let Err(error) = activation.release_frame() {
                self.fail_eval(error, output);
                return;
            }
            let Some(error) = state.failure.clone() else {
                self.fail_eval(LineRuntimeError::InvalidResultTransition, output);
                return;
            };
            if let Err(error) =
                transaction.stage_disposition(DialogueCommitDisposition::Failed { error })
            {
                self.fail_eval(error, output);
                return;
            }
            match self.commit_terminal_dialogue_activation_transaction(transaction, output) {
                Ok(disposition) => self.apply_dialogue_commit_disposition(disposition),
                Err(error) => self.fail_eval(error, output),
            }
        } else {
            let activation = transaction.activation().clone();
            match self.commit_dialogue_activation_transaction(transaction, output) {
                Ok(()) => {
                    self.fiber.status = super::FlowFiberStatus::Dialogue(activation);
                }
                Err(error) => self.fail_eval(error, output),
            }
        }
    }

    pub(super) fn resume_dialogue_successful_close(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) {
        let activation_id = transaction.activation().clone();
        transaction.frame_mut().phase = DialogueRuntimePhase::Closing;
        if !self.has_joined_work() {
            let mut candidate = transaction.clone();
            let batch =
                match self.prepare_dialogue_deferred_child(&mut candidate, ScopeExit::Completed) {
                    Ok(batch) => batch,
                    Err(error) => {
                        self.begin_dialogue_failure(transaction, error, output);
                        return;
                    }
                };
            transaction = candidate;
            if let Some(batch) = batch {
                match self.commit_dialogue_activation_transaction(transaction, output) {
                    Ok(()) => {
                        self.fiber.status = super::FlowFiberStatus::Dialogue(activation_id);
                        self.commit_line_task_execution_batch(batch);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
                return;
            }
            let activation = transaction.line_mut();
            if activation.deferred_inflight().is_none()
                && activation.deferred_registrations().is_empty()
                && !activation.has_pending_commands()
                && let Err(error) = Self::unwind_dialogue_handles(&activation_id, activation, true)
            {
                self.begin_dialogue_failure(transaction, error, output);
                return;
            }
        }
        let activation = transaction.line();
        let terminal = activation.successful_close_ready() && !self.has_joined_work();
        if terminal {
            self.resume_dialogue_publication_with_transaction(transaction, output);
        } else {
            let activation = transaction.activation().clone();
            match self.commit_dialogue_activation_transaction(transaction, output) {
                Ok(()) => {
                    self.fiber.status = super::FlowFiberStatus::Dialogue(activation);
                }
                Err(error) => self.fail_eval(error, output),
            }
        }
    }

    fn unwind_dialogue_handles(
        activation_id: &crate::runtime_id::DialogueActivationId,
        activation: &mut NativeDialogueActivationState,
        preserve_result: bool,
    ) -> Result<(), DialogueExecutionError> {
        activation.prepare_handle_unwind(activation_id, preserve_result)?;
        Ok(())
    }

    pub(super) fn consume_dialogue_host_outcomes(
        &mut self,
        transaction: &mut DialogueActivationTransaction,
        outcomes: &[RuntimeLineHostOutcome],
    ) -> Result<(), DialogueExecutionError> {
        let activation_id = transaction.activation().clone();
        let (state, activation) = transaction.parts_mut();
        let mut ledger = activation.ledger().clone();
        let pending = pending_command_id(state).cloned();
        let mut outcome_error = None;
        for outcome in outcomes {
            let command_id = outcome.command();
            if command_id.activation() != &activation_id {
                return Err(LineRuntimeError::StaleCommandOutcome.into());
            }
            if state.phase == DialogueRuntimePhase::Activating
                && pending
                    .as_ref()
                    .is_some_and(|pending| pending == command_id)
            {
                continue;
            }
            let Some(command) = activation.issued_command(command_id).cloned() else {
                if activation.is_resolved(command_id) {
                    return Err(LineRuntimeError::DuplicateCommandOutcome.into());
                }
                let Some(command) = activation.superseded_command(command_id).cloned() else {
                    return Err(LineRuntimeError::UnknownCommandOutcome.into());
                };
                match (&command, outcome) {
                    (
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook {
                                cue, ..
                            },
                        ),
                        RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Accepted {
                            cue: echoed,
                            ..
                        }),
                    ) if cue == echoed => continue,
                    (
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook {
                                cue, ..
                            },
                        ),
                        RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Completed {
                            cue: echoed,
                            ..
                        }),
                    ) if cue == echoed => {
                        ledger.set_state(
                            cue,
                            RuntimeHandleLeaseState::Cancelling,
                            RuntimeHandleLeaseState::Released,
                        )?;
                        let _ = activation.resolve_superseded(command_id);
                        activation.resolve_issued_cancel_for_cue(cue);
                        continue;
                    }
                    (
                        crate::presentation::RuntimeLineHostCommand::Stage(
                            crate::presentation::RuntimeStageCommand::SetCharacterLook {
                                cue, ..
                            },
                        ),
                        RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                            ..
                        }),
                    ) => {
                        ledger.set_state(
                            cue,
                            RuntimeHandleLeaseState::Cancelling,
                            RuntimeHandleLeaseState::Released,
                        )?;
                        let _ = activation.resolve_superseded(command_id);
                        activation.resolve_issued_cancel_for_cue(cue);
                        continue;
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                }
            };
            let mut terminal = false;
            match (&command, outcome) {
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::AcquireActor { actor, .. },
                    ),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Acquired {
                        actor: echoed,
                        ..
                    }),
                ) if actor == echoed => {
                    ledger.set_state(
                        actor,
                        RuntimeHandleLeaseState::Allocating,
                        RuntimeHandleLeaseState::Active,
                    )?;
                    terminal = true;
                    clear_pending_command(state, command_id);
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. },
                    ),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Accepted {
                        cue: echoed,
                        ..
                    }),
                ) if cue == echoed => {
                    let lease = ledger.lease(cue).ok_or(LineRuntimeError::UnknownHandle)?;
                    if lease.state() == RuntimeHandleLeaseState::Pending {
                        ledger.set_state(
                            cue,
                            RuntimeHandleLeaseState::Pending,
                            RuntimeHandleLeaseState::Running,
                        )?;
                    } else if lease.state() != RuntimeHandleLeaseState::Running {
                        return Err(LineRuntimeError::StageOutcomeMismatch.into());
                    }
                    clear_pending_command(state, command_id);
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. },
                    ),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Completed {
                        cue: echoed,
                        ..
                    }),
                ) if cue == echoed => {
                    ledger.set_state(
                        cue,
                        RuntimeHandleLeaseState::Running,
                        RuntimeHandleLeaseState::Completed,
                    )?;
                    activation.resolve_superseded_cue(cue);
                    terminal = true;
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::CancelCue { cue, .. },
                    ),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Cancelled {
                        cue: echoed,
                        ..
                    }),
                ) if cue == echoed => {
                    ledger.set_state(
                        cue,
                        RuntimeHandleLeaseState::Cancelling,
                        RuntimeHandleLeaseState::Cancelled,
                    )?;
                    ledger.set_state(
                        cue,
                        RuntimeHandleLeaseState::Cancelled,
                        RuntimeHandleLeaseState::Released,
                    )?;
                    terminal = true;
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. },
                    ),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::ReleasedActor {
                        actor: echoed,
                        ..
                    }),
                ) if actor == echoed => {
                    ledger.set_state(
                        actor,
                        RuntimeHandleLeaseState::Cancelling,
                        RuntimeHandleLeaseState::Released,
                    )?;
                    terminal = true;
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Voice(
                        crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice {
                            handle,
                            ..
                        },
                    ),
                    RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Released {
                        handle: echoed,
                        ..
                    }),
                ) if handle == echoed => {
                    ledger.set_state(
                        handle,
                        RuntimeHandleLeaseState::Cancelling,
                        RuntimeHandleLeaseState::Released,
                    )?;
                    terminal = true;
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Voice(
                        crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. },
                    ),
                    RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Started {
                        session,
                        ..
                    }),
                ) => {
                    if state.phase == DialogueRuntimePhase::Closing {
                        let Some(PendingLineOperation::StartVoice {
                            command: pending,
                            site,
                            ..
                        }) = state.pending_line_operation.as_ref()
                        else {
                            return Err(LineRuntimeError::StageOutcomeMismatch.into());
                        };
                        if pending != command_id {
                            return Err(LineRuntimeError::StageOutcomeMismatch.into());
                        }
                        let site = self
                            .plan
                            .line_task_groups()
                            .get(state.task_group.index())
                            .and_then(|group| group.handle_site(*site))
                            .cloned()
                            .ok_or(LineRuntimeError::InvalidHandleSite)?;
                        if site.site_kind() != RuntimeLineHandleSiteKind::Voice {
                            return Err(LineRuntimeError::InvalidHandleSite.into());
                        }
                        let ordinal = ledger.next_voice_lease_ordinal()?;
                        let _ = ledger.issue(
                            &activation_id,
                            &site,
                            RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                                session.clone(),
                                ordinal,
                                true,
                            )),
                            RuntimeHandleOwnerSlot::LineScope,
                        )?;
                    }
                    state.voice = RuntimeDialogueVoiceState::Ready(session.clone());
                    terminal = true;
                    clear_pending_command(state, command_id);
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Stage(_),
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                        code,
                        ..
                    }),
                ) => {
                    fail_issued_command_lease(&mut ledger, &command)?;
                    if let crate::presentation::RuntimeLineHostCommand::Stage(
                        crate::presentation::RuntimeStageCommand::CancelCue { cue, .. },
                    ) = &command
                    {
                        activation.resolve_superseded_cue(cue);
                    }
                    clear_pending_command(state, command_id);
                    activation.consume_issued_command(command_id)?;
                    outcome_error =
                        Some(LineRuntimeError::StageCommandRejected { code: *code }.into());
                    break;
                }
                (
                    crate::presentation::RuntimeLineHostCommand::Voice(_),
                    RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Rejected {
                        failure,
                        ..
                    }),
                ) => {
                    fail_issued_command_lease(&mut ledger, &command)?;
                    state.voice = RuntimeDialogueVoiceState::Failed(failure.clone());
                    clear_pending_command(state, command_id);
                    activation.consume_issued_command(command_id)?;
                    outcome_error = Some(
                        LineRuntimeError::VoiceStartRejected {
                            failure: failure.clone(),
                        }
                        .into(),
                    );
                    break;
                }
                _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
            }
            if terminal {
                activation.consume_issued_command(command_id)?;
            }
        }
        let scheduled = activation.scheduled().to_vec();
        activation.replace_transaction_parts(ledger, scheduled);
        match outcome_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    pub(super) fn resume_dialogue_activation(
        &mut self,
        transaction: &mut DialogueActivationTransaction,
        outcomes: &[RuntimeLineHostOutcome],
        host_results: &[crate::step::RuntimeHostCallResult],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<DialogueActivationStep, DialogueExecutionError> {
        let activation_id = transaction.activation().clone();
        let (frame, activation) = transaction.parts_mut();
        if frame.pending_line_operation.is_some() {
            match self.resume_pending_line_operation(&activation_id, frame, activation, outcomes) {
                Ok(true) => advance_activation_pc(frame)?,
                Ok(false) => {}
                Err(error) => return Err(error),
            }
            return Ok(DialogueActivationStep::Continue);
        }
        if let Some(pending) = frame.pending_host_call.clone() {
            let Some(result) = host_results.iter().find(|result| result.id == pending.id) else {
                return Ok(DialogueActivationStep::Continue);
            };
            let value = result
                .outcome
                .as_ref()
                .map_err(|error| DialogueExecutionError::HostCallFailed {
                    message: error.message.clone(),
                })?
                .value();
            self.plan
                .validate_live_value(
                    pending.result,
                    value,
                    crate::entry::RuntimeSchemaLimits::engine_default(),
                )
                .map_err(|error| DialogueExecutionError::HostCallFailed {
                    message: error.to_string(),
                })?;
            if !unique_affine_line_handles(value)?.is_empty() {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            if let Some(binding) = &pending.binding {
                let bindings =
                    match_runtime_pattern(&self.plan, binding, value)?.ok_or_else(|| {
                        RuntimeEvalError::PatternMismatch(super::runtime_value_label(value))
                    })?;
                frame.locals.bind_all(bindings);
            }
            frame.pending_host_call = None;
            advance_activation_pc(frame)?;
            return Ok(DialogueActivationStep::Continue);
        }
        let activation_exhausted = !frame.exiting_for_result
            && self
                .plan
                .line_task_groups()
                .get(frame.task_group.index())
                .is_some_and(|group| frame.activation_pc == group.activation_ops().len());
        if frame.exiting_for_result || activation_exhausted {
            if !frame.scopes.is_empty() {
                let step = self.prepare_activation_scope_exit(
                    &activation_id,
                    frame,
                    activation,
                    ScopeExit::Completed,
                )?;
                return match step {
                    ActivationScopeStep::Waiting | ActivationScopeStep::Finished => {
                        Ok(DialogueActivationStep::Continue)
                    }
                    ActivationScopeStep::Deferred(batch) => {
                        Ok(DialogueActivationStep::Deferred(batch))
                    }
                };
            }
            if activation.has_pending_commands() {
                return Ok(DialogueActivationStep::Continue);
            }
            return self
                .begin_dialogue_reveal(&activation_id, frame, activation)
                .map(DialogueActivationStep::Reveal);
        }
        let group = self
            .plan
            .line_task_groups()
            .get(frame.task_group.index())
            .cloned()
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let operation = group
            .activation_ops()
            .get(frame.activation_pc)
            .cloned()
            .ok_or(LineRuntimeError::ResultNotCommitted)?;
        match operation {
            FlowOp::EnterScope { identity } => {
                frame.locals.push_scope_with_identity(identity);
                frame.scopes.push(DialogueActivationScope::new());
            }
            FlowOp::ExitScope => {
                let step = self.prepare_activation_scope_exit(
                    &activation_id,
                    frame,
                    activation,
                    ScopeExit::Completed,
                )?;
                return match step {
                    ActivationScopeStep::Waiting => Ok(DialogueActivationStep::Continue),
                    ActivationScopeStep::Deferred(batch) => {
                        Ok(DialogueActivationStep::Deferred(batch))
                    }
                    ActivationScopeStep::Finished => {
                        advance_activation_pc(frame)?;
                        Ok(DialogueActivationStep::Continue)
                    }
                };
            }
            FlowOp::Let { pattern, expr } => {
                self.bind_dialogue_let(
                    &activation_id,
                    frame,
                    activation,
                    &pattern,
                    &expr,
                    pure_backend,
                )?;
            }
            FlowOp::HostCall { binding, target } => {
                let (args, named_args) = {
                    std::mem::swap(&mut self.fiber.env, &mut frame.locals);
                    let evaluated = self.evaluate_host_call_arguments(&target.args, pure_backend);
                    std::mem::swap(&mut self.fiber.env, &mut frame.locals);
                    evaluated
                        .map_err(|message| DialogueExecutionError::HostCallFailed { message })?
                };
                for value in args
                    .iter()
                    .chain(named_args.iter().map(|argument| &argument.value))
                {
                    if !unique_affine_line_handles(value.value())?.is_empty() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                let result = self
                    .plan
                    .type_table()
                    .get(target.result)
                    .map(|declaration| declaration.semantic_identity())
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                let id = self.preview_host_call_id(&target.public_id);
                frame.pending_host_call = Some(PendingActivationHostCall {
                    id: id.clone(),
                    result: target.result,
                    binding,
                });
                return Ok(DialogueActivationStep::HostCall(
                    crate::step::RuntimeHostCallRequest {
                        id,
                        public_id: target.public_id,
                        capability: target.capability,
                        operation: target.operation,
                        contract: target.contract,
                        args,
                        named_args,
                        result,
                        mode: target.mode,
                        deterministic: target.deterministic,
                    },
                ));
            }
            FlowOp::LineOperation { binding, operation } => {
                self.execute_line_operation(
                    &activation_id,
                    frame,
                    activation,
                    binding,
                    operation,
                    pure_backend,
                )?;
            }
            FlowOp::CommitDialogueResult { value } => {
                let value = self.evaluate_dialogue_expr(frame, &value, pure_backend)?;
                self.stage_dialogue_result(frame, activation, value)?;
                frame.exiting_for_result = true;
            }
            FlowOp::EvaluatedEffect(effect) => {
                let request = self.execute_dialogue_evaluated_effect(
                    &activation_id,
                    frame,
                    activation,
                    &effect,
                    pure_backend,
                )?;
                if let Some(request) = request {
                    let request = match request {
                        LineEffectRequest::Panic(message)
                        | LineEffectRequest::Fail(message)
                        | LineEffectRequest::Bail(message) => {
                            return Err(DialogueExecutionError::ActivationEffectFailed { message });
                        }
                        request => request,
                    };
                    advance_activation_pc(frame)?;
                    return Ok(DialogueActivationStep::Effect(request));
                }
            }
            FlowOp::RegisterDefer {
                site,
                outcome,
                captures,
                owner,
            } => {
                self.register_dialogue_defer(frame, activation, site, outcome, &captures, owner)?;
            }
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        }
        if frame.pending_line_operation.is_none() {
            advance_activation_pc(frame)?;
        }
        Ok(DialogueActivationStep::Continue)
    }

    /// Captures the exact reached defer site without executing its body.
    /// Affine locals move only after the complete capture/ledger preflight.
    fn register_dialogue_defer(
        &mut self,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        site: RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: &[crate::value::RuntimeExpr],
        owner_kind: RuntimeDeferOwner,
    ) -> Result<(), DialogueExecutionError> {
        activation.can_register_deferred()?;
        match owner_kind {
            RuntimeDeferOwner::CurrentScope
                if frame
                    .scopes
                    .last()
                    .is_some_and(|scope| scope.exit.is_none()) => {}
            RuntimeDeferOwner::LineRoot if frame.scopes.is_empty() => {}
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        }
        let function_id = self
            .plan
            .defer_function_site(site)
            .ok_or(RuntimeEvalError::UnknownDeferredSite { site })?;
        let function = self
            .plan
            .function_sites()
            .get(function_id)
            .ok_or(RuntimeEvalError::UnknownDeferredSite { site })?;
        if captures.len() != function.capture_inputs().count() {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }

        let mut ledger = activation.ledger().clone();
        let mut moved_locals = std::collections::BTreeSet::new();
        let mut captured_handles = std::collections::BTreeSet::new();
        let mut scoped_transfers = Vec::new();
        for (capture, input) in captures.iter().zip(function.capture_inputs()) {
            let RuntimeExprKind::Local(local) = capture.kind() else {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            };
            if capture.ty() != input.pattern().ty() {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let value = frame
                .locals
                .get(*local)
                .ok_or(RuntimeEvalError::UnknownLocal(*local))?;
            if !self
                .plan
                .value_matches_type(capture.ty(), value)
                .map_err(RuntimeEvalError::from)?
            {
                return Err(RuntimeEvalError::InvalidExpressionType(capture.ty()).into());
            }
            if value.ownership().permits_copy() {
                continue;
            }
            if !moved_locals.insert(*local) {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            for handle in unique_affine_line_handles(value)? {
                if !captured_handles.insert(handle.token().clone()) {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                }
                let lease = ledger
                    .lease(handle.token())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if lease.resource().kind() != handle.kind() {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
                }
                match lease.owner() {
                    RuntimeHandleOwnerSlot::LineScope => {
                        if owner_kind == RuntimeDeferOwner::CurrentScope {
                            scoped_transfers
                                .push((handle.token().clone(), RuntimeHandleOwnerSlot::LineScope));
                        }
                    }
                    source @ RuntimeHandleOwnerSlot::ActivationLocal(_) => {
                        let source = source.clone();
                        if owner_kind == RuntimeDeferOwner::CurrentScope {
                            scoped_transfers.push((handle.token().clone(), source));
                        } else {
                            ledger.transfer(
                                handle.token(),
                                &source,
                                RuntimeHandleOwnerSlot::LineScope,
                            )?;
                        }
                    }
                    _ => return Err(LineRuntimeError::WrongOwner.into()),
                }
            }
        }

        let values = captures
            .iter()
            .map(|capture| {
                let RuntimeExprKind::Local(local) = capture.kind() else {
                    unreachable!("defer capture was checked as a local")
                };
                frame
                    .locals
                    .get(*local)
                    .cloned()
                    .ok_or(RuntimeEvalError::UnknownLocal(*local))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut candidate = activation.clone();
        let scoped_registration = match owner_kind {
            RuntimeDeferOwner::CurrentScope => {
                let registration =
                    candidate.allocate_deferred_registration(site, outcome, values)?;
                for (token, source) in &scoped_transfers {
                    ledger.transfer(
                        token,
                        source,
                        RuntimeHandleOwnerSlot::ScopedDefer(registration.id()),
                    )?;
                }
                Some(registration)
            }
            RuntimeDeferOwner::LineRoot => {
                candidate.register_deferred(site, outcome, values)?;
                None
            }
        };
        candidate.commit_ledger(ledger);
        for local in moved_locals {
            let _ = frame
                .locals
                .take(local)
                .expect("captured local was preflighted");
        }
        *activation = candidate;
        if let Some(registration) = scoped_registration {
            frame
                .scopes
                .last_mut()
                .expect("current scope was checked")
                .deferred
                .push(registration);
        }
        Ok(())
    }

    /// Advances the top activation-owned lexical scope by at most one deferred
    /// child. The fixed exit and any in-flight child remain in the cloned
    /// activation frame until the whole transaction commits.
    fn prepare_activation_scope_exit(
        &self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        exit: ScopeExit,
    ) -> Result<ActivationScopeStep, DialogueExecutionError> {
        let scope = frame
            .scopes
            .last_mut()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        scope.freeze_exit(exit);
        if scope.inflight.is_some() || activation.has_pending_commands() {
            return Ok(ActivationScopeStep::Waiting);
        }
        let fixed_exit = scope.exit.expect("scope exit was frozen");
        while let Some(registration) = scope.deferred.last().cloned() {
            // Validate and construct the child before moving its capture packet.
            let batch = registration
                .outcome_filter()
                .matches(fixed_exit)
                .then(|| self.prepare_deferred_line_child(activation_id, registration.clone()))
                .transpose()?;
            let step = activation.prepare_scoped_deferred(
                activation_id,
                registration.clone(),
                fixed_exit,
            )?;
            let popped = scope.deferred.pop().expect("checked pending registration");
            if popped.id() != registration.id() {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }
            match step {
                RuntimeDeferUnwindStep::Run(registration) => {
                    scope.inflight = Some((registration.id(), registration.site()));
                    return Ok(ActivationScopeStep::Deferred(
                        batch.expect("matching registration built a child"),
                    ));
                }
                RuntimeDeferUnwindStep::Skipped(_) => {
                    if activation.has_pending_commands() {
                        return Ok(ActivationScopeStep::Waiting);
                    }
                }
            }
        }
        self.release_activation_scope_locals(activation_id, frame, activation)?;
        frame.scopes.pop();
        Ok(ActivationScopeStep::Finished)
    }

    /// Releases lexical locals after all deferred callbacks have observed them.
    /// A value already staged as the dialogue result has DialogueResult custody
    /// and survives this scope exit; every remaining affine local is dropped
    /// through the line command journal before reveal can start.
    fn release_activation_scope_locals(
        &self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
    ) -> Result<(), DialogueExecutionError> {
        let mut candidate = activation.clone();
        let mut ledger = candidate.ledger().clone();
        let mut commands =
            RuntimeCommandQueue::new(activation_id.clone(), candidate.command_sequence());
        let mut seen = std::collections::BTreeSet::new();
        for binding in frame.locals.current_scope_bindings() {
            for handle in unique_affine_line_handles(&binding.value)? {
                if handle.token().activation() != activation_id
                    || !seen.insert(handle.token().clone())
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
                let lease = ledger
                    .lease(handle.token())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if lease.resource().kind() != handle.kind() {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
                }
                if matches!(lease.owner(), RuntimeHandleOwnerSlot::DialogueResult(_)) {
                    continue;
                }
                let owner =
                    RuntimeHandleOwnerSlot::ActivationLocal(self.owned_slot(binding.local)?);
                ledger.drop_owned(handle.token(), &owner, &mut commands)?;
            }
        }
        candidate.commit_ledger(ledger);
        flush_commands(activation_id, &mut candidate, commands)?;
        *activation = candidate;
        let _ = frame.locals.pop_scope_bindings();
        Ok(())
    }

    fn evaluate_dialogue_expr(
        &mut self,
        state: &mut DialogueActivationFrame,
        expression: &crate::value::RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<RuntimeValue, RuntimeEvalError> {
        std::mem::swap(&mut self.fiber.env, &mut state.locals);
        let result = self.evaluate_expr_with_backend(expression, pure_backend);
        std::mem::swap(&mut self.fiber.env, &mut state.locals);
        result
    }

    fn bind_dialogue_let(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        frame: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        pattern: &RuntimePattern,
        expression: &crate::value::RuntimeExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), DialogueExecutionError> {
        let value = self.evaluate_dialogue_expr(frame, expression, pure_backend)?;
        let bindings = match_runtime_pattern(&self.plan, pattern, &value)?
            .ok_or_else(|| RuntimeEvalError::PatternMismatch(super::runtime_value_label(&value)))?;
        let handles = unique_affine_line_handles(&value)?;
        let result_tokens = handles
            .iter()
            .map(|handle| handle.token().clone())
            .collect::<std::collections::BTreeSet<_>>();
        let mut candidate = activation.clone();
        let mut ledger = candidate.ledger().clone();
        let mut commands =
            RuntimeCommandQueue::new(activation_id.clone(), candidate.command_sequence());
        let mut moved_sources = std::collections::BTreeSet::new();
        for handle in handles {
            if handle.token().activation() != activation_id {
                return Err(LineRuntimeError::WrongActivation.into());
            }
            let lease = ledger
                .lease(handle.token())
                .ok_or(LineRuntimeError::UnknownHandle)?;
            if lease.resource().kind() != handle.kind() {
                return Err(LineRuntimeError::WrongOpaqueProducer.into());
            }
            let source = lease.owner().clone();
            match &source {
                RuntimeHandleOwnerSlot::LineScope => {}
                RuntimeHandleOwnerSlot::ActivationLocal(slot) => {
                    let mut source_local = None;
                    for binding in frame.locals.bindings() {
                        if &self.owned_slot(binding.local)? == slot
                            && value_contains_token(&binding.value, handle.token())?
                        {
                            if source_local.replace(binding.local).is_some() {
                                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                            }
                        }
                    }
                    let local = source_local.ok_or(LineRuntimeError::WrongOwner)?;
                    moved_sources.insert(local);
                }
                _ => return Err(LineRuntimeError::WrongOwner.into()),
            }
            if let Some(local) = binding_destination_local(&bindings, handle.token())? {
                let destination = RuntimeHandleOwnerSlot::ActivationLocal(self.owned_slot(local)?);
                if source != destination {
                    ledger.transfer(handle.token(), &source, destination)?;
                }
            } else {
                ledger.drop_owned(handle.token(), &source, &mut commands)?;
            }
        }
        if !value.ownership().permits_copy() && result_tokens.is_empty() {
            let RuntimeExprKind::Local(local) = expression.kind() else {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            };
            moved_sources.insert(*local);
        }
        for local in &moved_sources {
            let source = frame
                .locals
                .get(*local)
                .ok_or(RuntimeEvalError::UnknownLocal(*local))?;
            if unique_affine_line_handles(source)?
                .iter()
                .any(|handle| !result_tokens.contains(handle.token()))
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
        }
        candidate.commit_ledger(ledger);
        flush_commands(activation_id, &mut candidate, commands)?;
        for local in moved_sources {
            let _ = frame
                .locals
                .take(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        }
        frame.locals.bind_all(bindings);
        *activation = candidate;
        Ok(())
    }

    fn execute_line_operation(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        binding: Option<RuntimePattern>,
        operation: RuntimeLineOperation,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<(), DialogueExecutionError> {
        let group = self
            .plan
            .line_task_groups()
            .get(state.task_group.index())
            .cloned()
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let site = group
            .handle_site(operation.site())
            .cloned()
            .ok_or(LineRuntimeError::InvalidHandleSite)?;
        match operation {
            RuntimeLineOperation::AcquireActor {
                character, scope, ..
            } => {
                if site.site_kind() != RuntimeLineHandleSiteKind::StageActor
                    || site.character() != Some(&character)
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let mut ledger = activation.ledger().clone();
                let value = RuntimeValue::Opaque(ledger.issue(
                    activation_id,
                    &site,
                    RuntimeHandleResource::StageActor(RuntimeStageActorLease::new(
                        character.clone(),
                    )),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let token = RuntimeLineHandleToken::try_decode_payload(match &value {
                    RuntimeValue::Opaque(value) => value.payload(),
                    _ => unreachable!("ledger issue returns one opaque value"),
                })
                .map_err(|_| LineRuntimeError::InvalidHandlePayload)?;
                let mut commands =
                    RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
                let command = commands
                    .push_acquire_actor(token.clone(), character, scope)
                    .map_err(LineRuntimeError::from)?;
                activation.commit_ledger(ledger);
                flush_commands(activation_id, activation, commands)?;
                state.pending_line_operation = Some(PendingLineOperation::AcquireActor {
                    command,
                    binding,
                    value,
                    token,
                });
            }
            RuntimeLineOperation::Schedule {
                delay,
                child,
                captures,
                ..
            } => {
                if site.site_kind() != RuntimeLineHandleSiteKind::ScheduledCue
                    || site.scheduled_child() != Some(child)
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let RuntimeValue::Duration(delay) =
                    self.evaluate_dialogue_expr(state, &delay, pure_backend)?
                else {
                    return Err(LineRuntimeError::InvalidCueDelay.into());
                };
                let deadline = state
                    .elapsed
                    .checked_add(delay)
                    .ok_or(LineRuntimeError::CueDeadlineOverflow)?;
                let captures = captures
                    .iter()
                    .map(|capture| {
                        self.evaluate_dialogue_expr(state, capture.value(), pure_backend)
                            .map(|value| RuntimeLocalBinding {
                                local: capture.local(),
                                value,
                            })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if activation.scheduled().len() >= MAX_LINE_SCHEDULED_CALLBACKS {
                    return Err(LineRuntimeError::ScheduledCallbackLimitExceeded.into());
                }
                let mut ledger = activation.ledger().clone();
                let (scope, join_policy) = match group.node(child) {
                    Some(crate::line_task::LineTaskNode::Child {
                        scope, join_policy, ..
                    }) => (*scope, *join_policy),
                    _ => return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into()),
                };
                let mut captured_tokens = std::collections::BTreeSet::new();
                let mut capture_transfers = Vec::new();
                for capture in &captures {
                    for handle in unique_affine_line_handles(&capture.value)? {
                        if !captured_tokens.insert(handle.token().clone()) {
                            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                        }
                        if join_policy == crate::line_task::ChildJoinPolicy::Detached {
                            return Err(LineRuntimeError::DetachedAffineCapture.into());
                        }
                        let expected = ledger
                            .lease(handle.token())
                            .map(|lease| lease.owner().clone())
                            .ok_or(LineRuntimeError::UnknownHandle)?;
                        if !matches!(
                            expected,
                            RuntimeHandleOwnerSlot::LineScope
                                | RuntimeHandleOwnerSlot::ActivationLocal(_)
                        ) {
                            return Err(LineRuntimeError::WrongOwner.into());
                        }
                        capture_transfers.push((handle.token().clone(), expected));
                    }
                }
                let value = RuntimeValue::Opaque(ledger.issue(
                    activation_id,
                    &site,
                    RuntimeHandleResource::Cue(RuntimeCueLease::new(RuntimeCueOrigin::Scheduled {
                        child,
                        deadline,
                    })),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let token = token_from_value(&value)?;
                let work = crate::line_task::LineTaskWorkTag::scheduled(token.clone(), scope);
                for (captured, expected) in capture_transfers {
                    ledger.transfer(
                        &captured,
                        &expected,
                        RuntimeHandleOwnerSlot::ChildScope(work.clone()),
                    )?;
                }
                let mut commands =
                    RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
                let bindings = self.plan_operation_binding(
                    &mut ledger,
                    &mut commands,
                    binding.as_ref(),
                    &value,
                )?;
                activation.schedule(RuntimeScheduledLineTask::new(
                    token,
                    child,
                    work,
                    deadline,
                    captures.into_boxed_slice(),
                )?)?;
                activation.commit_ledger(ledger);
                state.locals.bind_all(bindings);
                flush_commands(activation_id, activation, commands)?;
            }
            RuntimeLineOperation::ActorLook {
                character,
                actor,
                look,
                crossfade,
                ..
            } => {
                if site.site_kind() != RuntimeLineHandleSiteKind::StageLookCue
                    || site.character() != Some(&character)
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let actor_value = self.evaluate_dialogue_expr(state, &actor, pure_backend)?;
                let RuntimeValue::Opaque(actor_opaque) = &actor_value else {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
                };
                let actor_lease = activation.ledger().validate_value(
                    actor_opaque,
                    crate::value::RuntimeHandleKind::StageActor,
                    activation_id,
                )?;
                let RuntimeHandleResource::StageActor(actor_resource) = actor_lease.resource()
                else {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
                };
                if actor_resource.character() != &character {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                }
                let actor_token = actor_lease.token().clone();
                let RuntimeExprKind::EntityRef(look) = look.kind() else {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                };
                let Some((look_character, look)) = look.character_look() else {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                };
                if look_character != &character {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                }
                let look = look.clone();
                let RuntimeValue::Duration(crossfade) =
                    self.evaluate_dialogue_expr(state, &crossfade, pure_backend)?
                else {
                    return Err(LineRuntimeError::InvalidCrossfade.into());
                };
                let mut ledger = activation.ledger().clone();
                let value = RuntimeValue::Opaque(ledger.issue(
                    activation_id,
                    &site,
                    RuntimeHandleResource::Cue(RuntimeCueLease::new(RuntimeCueOrigin::StageLook)),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let token = token_from_value(&value)?;
                let mut commands =
                    RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
                let command = commands
                    .push_set_character_look(token.clone(), actor_token, character, look, crossfade)
                    .map_err(LineRuntimeError::from)?;
                activation.commit_ledger(ledger);
                flush_commands(activation_id, activation, commands)?;
                state.pending_line_operation = Some(PendingLineOperation::ActorLook {
                    command,
                    binding,
                    value,
                    token,
                });
            }
            RuntimeLineOperation::VoiceHandle { .. } => {
                if site.site_kind() != RuntimeLineHandleSiteKind::Voice {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                match state.voice.clone() {
                    RuntimeDialogueVoiceState::Ready(session)
                    | RuntimeDialogueVoiceState::Completed(session) => {
                        let mut ledger = activation.ledger().clone();
                        let ordinal = ledger.next_voice_lease_ordinal()?;
                        let value = RuntimeValue::Opaque(ledger.issue(
                            activation_id,
                            &site,
                            RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                                session, ordinal, true,
                            )),
                            RuntimeHandleOwnerSlot::LineScope,
                        )?);
                        let mut commands = RuntimeCommandQueue::new(
                            activation_id.clone(),
                            activation.command_sequence(),
                        );
                        let bindings = self.plan_operation_binding(
                            &mut ledger,
                            &mut commands,
                            binding.as_ref(),
                            &value,
                        )?;
                        activation.commit_ledger(ledger);
                        state.locals.bind_all(bindings);
                        flush_commands(activation_id, activation, commands)?;
                    }
                    RuntimeDialogueVoiceState::Lazy(ticket) => {
                        let mut commands = RuntimeCommandQueue::new(
                            activation_id.clone(),
                            activation.command_sequence(),
                        );
                        let command = commands
                            .push_start_voice(ticket)
                            .map_err(LineRuntimeError::from)?;
                        flush_commands(activation_id, activation, commands)?;
                        state.pending_line_operation = Some(PendingLineOperation::StartVoice {
                            command,
                            binding,
                            site: site.id(),
                        });
                    }
                    RuntimeDialogueVoiceState::Absent => {
                        return Err(LineRuntimeError::MissingActiveVoice.into());
                    }
                    RuntimeDialogueVoiceState::Failed(failure) => {
                        return Err(LineRuntimeError::VoiceStartRejected { failure }.into());
                    }
                }
            }
        }
        Ok(())
    }

    fn resume_pending_line_operation(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        outcomes: &[RuntimeLineHostOutcome],
    ) -> Result<bool, DialogueExecutionError> {
        let pending = state
            .pending_line_operation
            .clone()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        match pending {
            PendingLineOperation::AcquireActor {
                command,
                binding,
                value,
                token,
            } => {
                require_issued_command(activation_id, activation, &command)?;
                let Some(outcome) = outcomes.iter().find_map(|outcome| match outcome {
                    RuntimeLineHostOutcome::Stage(outcome) if outcome.command() == &command => {
                        Some(outcome)
                    }
                    _ => None,
                }) else {
                    return Ok(false);
                };
                match outcome {
                    RuntimeStageCommandOutcome::Acquired { actor, .. } if actor == &token => {
                        activation.consume_issued_command(&command)?;
                        let mut ledger = activation.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Allocating,
                            RuntimeHandleLeaseState::Active,
                        )?;
                        let mut commands = RuntimeCommandQueue::new(
                            activation_id.clone(),
                            activation.command_sequence(),
                        );
                        let bindings = self.plan_operation_binding(
                            &mut ledger,
                            &mut commands,
                            binding.as_ref(),
                            &value,
                        )?;
                        activation.commit_ledger(ledger);
                        state.locals.bind_all(bindings);
                        flush_commands(activation_id, activation, commands)?;
                    }
                    RuntimeStageCommandOutcome::Rejected { code, .. } => {
                        activation.consume_issued_command(&command)?;
                        let mut ledger = activation.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Allocating,
                            RuntimeHandleLeaseState::Failed,
                        )?;
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Failed,
                            RuntimeHandleLeaseState::Released,
                        )?;
                        activation.commit_ledger(ledger);
                        state.pending_line_operation = None;
                        return Err(LineRuntimeError::StageCommandRejected { code: *code }.into());
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                }
            }
            PendingLineOperation::ActorLook {
                command,
                binding,
                value,
                token,
            } => {
                require_issued_command(activation_id, activation, &command)?;
                let Some(outcome) = outcomes.iter().find_map(|outcome| match outcome {
                    RuntimeLineHostOutcome::Stage(outcome) if outcome.command() == &command => {
                        Some(outcome)
                    }
                    _ => None,
                }) else {
                    return Ok(false);
                };
                match outcome {
                    RuntimeStageCommandOutcome::Accepted { cue, .. } if cue == &token => {
                        let mut ledger = activation.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Pending,
                            RuntimeHandleLeaseState::Running,
                        )?;
                        let mut commands = RuntimeCommandQueue::new(
                            activation_id.clone(),
                            activation.command_sequence(),
                        );
                        let bindings = self.plan_operation_binding(
                            &mut ledger,
                            &mut commands,
                            binding.as_ref(),
                            &value,
                        )?;
                        activation.commit_ledger(ledger);
                        state.locals.bind_all(bindings);
                        flush_commands(activation_id, activation, commands)?;
                    }
                    RuntimeStageCommandOutcome::Rejected { code, .. } => {
                        activation.consume_issued_command(&command)?;
                        let mut ledger = activation.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Pending,
                            RuntimeHandleLeaseState::Failed,
                        )?;
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Failed,
                            RuntimeHandleLeaseState::Released,
                        )?;
                        activation.commit_ledger(ledger);
                        state.pending_line_operation = None;
                        return Err(LineRuntimeError::StageCommandRejected { code: *code }.into());
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                }
            }
            PendingLineOperation::StartVoice {
                command,
                binding,
                site,
            } => {
                require_issued_command(activation_id, activation, &command)?;
                let Some(outcome) = outcomes.iter().find_map(|outcome| match outcome {
                    RuntimeLineHostOutcome::Voice(outcome) if outcome.command() == &command => {
                        Some(outcome)
                    }
                    _ => None,
                }) else {
                    return Ok(false);
                };
                activation.consume_issued_command(&command)?;
                let session = match outcome {
                    RuntimeVoiceCommandOutcome::Started { session, .. } => session,
                    RuntimeVoiceCommandOutcome::Rejected { failure, .. } => {
                        state.voice = RuntimeDialogueVoiceState::Failed(failure.clone());
                        state.pending_line_operation = None;
                        return Err(LineRuntimeError::VoiceStartRejected {
                            failure: failure.clone(),
                        }
                        .into());
                    }
                    RuntimeVoiceCommandOutcome::Released { .. } => {
                        return Err(LineRuntimeError::StageOutcomeMismatch.into());
                    }
                };
                state.voice = RuntimeDialogueVoiceState::Ready(session.clone());
                let group = self
                    .plan
                    .line_task_groups()
                    .get(state.task_group.index())
                    .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                let site = group
                    .handle_site(site)
                    .cloned()
                    .ok_or(LineRuntimeError::InvalidHandleSite)?;
                let mut ledger = activation.ledger().clone();
                let ordinal = ledger.next_voice_lease_ordinal()?;
                let value = RuntimeValue::Opaque(ledger.issue(
                    activation_id,
                    &site,
                    RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                        session.clone(),
                        ordinal,
                        true,
                    )),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let mut commands =
                    RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
                let bindings = self.plan_operation_binding(
                    &mut ledger,
                    &mut commands,
                    binding.as_ref(),
                    &value,
                )?;
                activation.commit_ledger(ledger);
                state.locals.bind_all(bindings);
                flush_commands(activation_id, activation, commands)?;
            }
        }
        state.pending_line_operation = None;
        Ok(true)
    }

    fn plan_operation_binding(
        &self,
        ledger: &mut crate::line_task::RuntimeLineHandleLedger,
        commands: &mut RuntimeCommandQueue,
        pattern: Option<&RuntimePattern>,
        value: &RuntimeValue,
    ) -> Result<Vec<RuntimeLocalBinding>, DialogueExecutionError> {
        let Some(pattern) = pattern else {
            return Ok(Vec::new());
        };
        let bindings = match_runtime_pattern(&self.plan, pattern, value)?
            .ok_or(LineRuntimeError::ResultPatternOrTypeMismatch)?;
        for handle in unique_affine_line_handles(value)? {
            let expected = ledger
                .lease(handle.token())
                .map(|lease| lease.owner().clone())
                .ok_or(LineRuntimeError::UnknownHandle)?;
            if !matches!(expected, RuntimeHandleOwnerSlot::LineScope) {
                return Err(LineRuntimeError::WrongOwner.into());
            }
            let destination = binding_destination_local(&bindings, handle.token())?
                .map(|local| self.owned_slot(local))
                .transpose()?
                .map(RuntimeHandleOwnerSlot::ActivationLocal);
            match destination {
                Some(destination) => ledger.transfer(handle.token(), &expected, destination)?,
                None => ledger.drop_owned(handle.token(), &expected, commands)?,
            }
        }
        Ok(bindings)
    }

    fn stage_dialogue_result(
        &mut self,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        value: RuntimeValue,
    ) -> Result<(), DialogueExecutionError> {
        if !matches!(activation.result(), RuntimeDialogueResultState::Uncommitted) {
            return Err(LineRuntimeError::ResultAlreadyCommitted.into());
        }
        let checked = self
            .plan
            .checked_type(state.result_target.ty())
            .map_err(|_| LineRuntimeError::ResultPatternOrTypeMismatch)?
            .ok_or(LineRuntimeError::ResultPatternOrTypeMismatch)?;
        if !checked.accepts_value(&value) {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let handles = unique_affine_line_handles(&value)?;
        let mut ledger = activation.ledger().clone();
        for handle in &handles {
            let lease = ledger
                .lease(handle.token())
                .ok_or(LineRuntimeError::UnknownHandle)?;
            if lease.resource().kind() != handle.kind() {
                return Err(LineRuntimeError::WrongOpaqueProducer.into());
            }
            let expected = lease.owner().clone();
            if !matches!(
                expected,
                RuntimeHandleOwnerSlot::LineScope | RuntimeHandleOwnerSlot::ActivationLocal(_)
            ) {
                return Err(LineRuntimeError::WrongOwner.into());
            }
            ledger.transfer(
                handle.token(),
                &expected,
                RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone()),
            )?;
        }
        activation.commit_ledger(ledger);
        activation.commit_result(state.result_target.ty(), value)?;
        Ok(())
    }

    fn begin_dialogue_reveal(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
    ) -> Result<DialogueLineTaskStart, DialogueExecutionError> {
        if !matches!(
            activation.result(),
            RuntimeDialogueResultState::Uncommitted | RuntimeDialogueResultState::Committed { .. }
        ) || !state.scopes.is_empty()
            || activation.has_pending_commands()
        {
            return Err(LineRuntimeError::ResultNotCommitted.into());
        }
        let group = self
            .plan
            .line_task_groups()
            .get(state.task_group.index())
            .cloned()
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let mut live = LineTaskLiveState::new(&group, activation_id.clone());
        for token in activation.arm_due_schedules(state.elapsed)? {
            live.mark_scheduled_ready(token)?;
        }
        let line_task_activation = progress_live_line_task_group(
            &group,
            state.elapsed,
            LineTaskReadyEvents::new(&std::collections::BTreeSet::new()),
            &mut live,
        )?;
        let template = self
            .plan
            .dialogue_content()
            .get(state.content)
            .ok_or(crate::line_task::LineRuntimeError::UnknownContentPlan)?
            .template();
        state.line_task = DialogueLineTaskState::Live(live);
        state.phase = DialogueRuntimePhase::Ready;
        Ok(DialogueLineTaskStart {
            event: Some(FlowEvent::DialogueLine {
                activation: activation_id.clone(),
                line: state.line.clone(),
                template,
                target: state.target.clone(),
                values: state.values.clone(),
            }),
            request_cancellation: false,
            group,
            activation: line_task_activation,
            captures: state.captures.clone(),
            callbacks: Vec::new(),
        })
    }

    pub(super) fn resume_dialogue_publication(
        &mut self,
        transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) {
        self.resume_dialogue_publication_with_transaction(transaction, output);
    }

    fn resume_dialogue_publication_with_transaction(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) {
        let activation_id = transaction.activation().clone();
        transaction.frame_mut().phase = DialogueRuntimePhase::Publishing;
        match self.try_publish_dialogue_result(&activation_id, &mut transaction) {
            Ok(DialoguePublicationOutcome::Pending) => {
                let activation = transaction.activation().clone();
                match self.commit_dialogue_activation_transaction(transaction, output) {
                    Ok(()) => {
                        self.fiber.status = super::FlowFiberStatus::Dialogue(activation);
                    }
                    Err(error) => self.fail_eval(error, output),
                }
            }
            Ok(DialoguePublicationOutcome::Published) => {
                match self.commit_published_dialogue_activation_transaction(transaction, output) {
                    Ok(disposition) => self.apply_dialogue_commit_disposition(disposition),
                    Err(error) => self.fail_eval(error, output),
                }
            }
            Err(error) => self.begin_dialogue_failure(transaction, error, output),
        }
    }

    fn try_publish_dialogue_result(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        transaction: &mut DialogueActivationTransaction,
    ) -> Result<DialoguePublicationOutcome, DialogueExecutionError> {
        let (state, activation) = transaction.parts_mut();
        let (ty, value, begin_publication) = match activation.result().clone() {
            RuntimeDialogueResultState::Committed { ty, value } => (ty, value, true),
            RuntimeDialogueResultState::Selected { ty, value, .. } => (ty, value, true),
            RuntimeDialogueResultState::Publishing { ty, value } => (ty, value, false),
            RuntimeDialogueResultState::Uncommitted
            | RuntimeDialogueResultState::Published
            | RuntimeDialogueResultState::Abandoned => {
                return Err(LineRuntimeError::ResultNotCommitted.into());
            }
        };
        if ty != state.result_target.ty() {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let checked = self
            .plan
            .checked_type(ty)
            .map_err(|_| LineRuntimeError::ResultPatternOrTypeMismatch)?
            .ok_or(LineRuntimeError::ResultPatternOrTypeMismatch)?;
        if !checked.accepts_value(&value) {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let bindings = match_runtime_pattern(&self.plan, state.result_target.pattern(), &value)?
            .ok_or(LineRuntimeError::ResultPatternOrTypeMismatch)?;
        if begin_publication {
            let handles = unique_affine_line_handles(&value)?;
            let mut ledger = activation.ledger().clone();
            let mut commands =
                RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
            let mut remaining = ledger
                .leases()
                .values()
                .filter(|lease| {
                    !matches!(
                        lease.owner(),
                        RuntimeHandleOwnerSlot::DialogueResult(_)
                            | RuntimeHandleOwnerSlot::ParentFiber(_)
                    ) && lease.state() != RuntimeHandleLeaseState::Released
                })
                .map(|lease| (lease.token().clone(), lease.owner().clone()))
                .collect::<Vec<_>>();
            remaining.reverse();
            for (token, owner) in remaining {
                ledger.drop_owned(&token, &owner, &mut commands)?;
            }
            for handle in &handles {
                let expected = RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone());
                let destination = binding_destination_local(&bindings, handle.token())?
                    .map(|local| self.owned_slot(local))
                    .transpose()?
                    .map(RuntimeHandleOwnerSlot::ParentFiber);
                match destination {
                    Some(destination) => ledger.transfer(handle.token(), &expected, destination)?,
                    None => ledger.drop_owned(handle.token(), &expected, &mut commands)?,
                }
            }
            activation.commit_ledger(ledger);
            flush_commands(activation_id, activation, commands)?;
            activation.begin_result_publication()?;
        }
        if activation.has_pending_commands() {
            return Ok(DialoguePublicationOutcome::Pending);
        }
        if activation.ledger().leases().values().any(|lease| {
            lease.state() != RuntimeHandleLeaseState::Released
                && !matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
        }) {
            return Err(LineRuntimeError::UnownedLeaseAtPublish.into());
        }
        activation.finish_result_publication()?;
        activation.release_frame()?;
        state.line_task = DialogueLineTaskState::Closed;
        let resume = state.resume;
        transaction.stage_disposition(DialogueCommitDisposition::Published { resume, bindings })?;
        Ok(DialoguePublicationOutcome::Published)
    }

    fn execute_dialogue_evaluated_effect(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        effect: &RuntimeEffectExpr,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<LineEffectRequest>, DialogueExecutionError> {
        if !matches!(effect, RuntimeEffectExpr::Drop { .. }) {
            let values = effect
                .argument_exprs()
                .into_iter()
                .map(|expression| self.evaluate_dialogue_expr(state, expression, pure_backend))
                .collect::<Result<Vec<_>, _>>()?;
            return effect.materialize(&values).map_err(Into::into);
        }
        let RuntimeEffectExpr::Drop { target, policy } = effect else {
            unreachable!("non-drop effects were handled above");
        };
        let policy = match policy {
            RuntimeDropPolicyExpr::Default => RuntimeDropPolicy::Default,
            RuntimeDropPolicyExpr::Cancel => RuntimeDropPolicy::Cancel,
            RuntimeDropPolicyExpr::Stop { fade } => {
                let RuntimeValue::Duration(fade) =
                    self.evaluate_dialogue_expr(state, fade, pure_backend)?
                else {
                    return Err(LineRuntimeError::InvalidDropPolicy.into());
                };
                RuntimeDropPolicy::Stop { fade }
            }
            RuntimeDropPolicyExpr::Finish => RuntimeDropPolicy::Finish,
            RuntimeDropPolicyExpr::Release => RuntimeDropPolicy::Release,
            RuntimeDropPolicyExpr::Detach => RuntimeDropPolicy::Detach,
        };
        let source_local = match target.kind() {
            RuntimeExprKind::Local(local) => Some(*local),
            _ => None,
        };
        let target = match source_local {
            Some(local) => state
                .locals
                .take(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?,
            None => self.evaluate_dialogue_expr(state, target, pure_backend)?,
        };
        let result = (|| {
            let handles = unique_affine_line_handles(&target)?;
            let mut ledger = activation.ledger().clone();
            let mut commands =
                RuntimeCommandQueue::new(activation_id.clone(), activation.command_sequence());
            for handle in handles {
                let expected = ledger
                    .lease(handle.token())
                    .map(|lease| lease.owner().clone())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if !matches!(
                    expected,
                    RuntimeHandleOwnerSlot::LineScope | RuntimeHandleOwnerSlot::ActivationLocal(_)
                ) {
                    return Err(DialogueExecutionError::from(LineRuntimeError::WrongOwner));
                }
                ledger.drop_owned_with_policy(handle.token(), &expected, policy, &mut commands)?;
            }
            activation.commit_ledger(ledger);
            flush_commands(activation_id, activation, commands)?;
            Ok(())
        })();
        if result.is_err()
            && let Some(local) = source_local
        {
            state.locals.set(local, target);
        }
        result.map(|()| None)
    }
    fn owned_slot(
        &self,
        local: crate::runtime_id::RuntimeLocalDeclarationId,
    ) -> Result<RuntimeOwnedSlotId, LineRuntimeError> {
        self.plan
            .local_declarations()
            .get(local)
            .ok_or(LineRuntimeError::UnknownOwnedLocal { local })?;
        Ok(RuntimeOwnedSlotId::environment_local(
            self.fiber.execution,
            local,
        ))
    }
}

fn token_from_value(value: &RuntimeValue) -> Result<RuntimeLineHandleToken, LineRuntimeError> {
    let RuntimeValue::Opaque(value) = value else {
        return Err(LineRuntimeError::WrongOpaqueProducer);
    };
    RuntimeLineHandleToken::try_decode_payload(value.payload())
        .map_err(|_| LineRuntimeError::InvalidHandlePayload)
}

fn advance_activation_pc(state: &mut DialogueActivationFrame) -> Result<(), LineRuntimeError> {
    state.activation_pc = state
        .activation_pc
        .checked_add(1)
        .ok_or(LineRuntimeError::ActivationProgramCounterOverflow)?;
    Ok(())
}

fn unique_affine_line_handles(
    value: &RuntimeValue,
) -> Result<Vec<crate::value::ownership::RuntimeAffineLineHandle>, LineRuntimeError> {
    let handles = value
        .affine_line_handles()
        .map_err(|_| LineRuntimeError::InvalidHandlePayload)?;
    let mut tokens = std::collections::BTreeSet::new();
    for handle in &handles {
        if !tokens.insert(handle.token().clone()) {
            return Err(LineRuntimeError::DuplicateHandleOccurrence);
        }
    }
    Ok(handles)
}

fn value_contains_token(
    value: &RuntimeValue,
    token: &RuntimeLineHandleToken,
) -> Result<bool, LineRuntimeError> {
    Ok(unique_affine_line_handles(value)?
        .iter()
        .any(|handle| handle.token() == token))
}

fn binding_destination_local(
    bindings: &[RuntimeLocalBinding],
    token: &RuntimeLineHandleToken,
) -> Result<Option<crate::runtime_id::RuntimeLocalDeclarationId>, LineRuntimeError> {
    let mut destination = None;
    for binding in bindings {
        if !value_contains_token(&binding.value, token)? {
            continue;
        }
        if destination.is_some() {
            return Err(LineRuntimeError::DuplicateHandleOccurrence);
        }
        destination = Some(binding.local);
    }
    Ok(destination)
}

fn flush_commands(
    activation_id: &crate::runtime_id::DialogueActivationId,
    activation: &mut NativeDialogueActivationState,
    commands: RuntimeCommandQueue,
) -> Result<(), LineRuntimeError> {
    activation.record_commands(activation_id, commands)
}

fn require_issued_command(
    activation_id: &crate::runtime_id::DialogueActivationId,
    activation: &NativeDialogueActivationState,
    command: &crate::presentation::RuntimeLineCommandId,
) -> Result<(), LineRuntimeError> {
    if command.activation() != activation_id || activation.issued_command(command).is_none() {
        return Err(LineRuntimeError::UnknownCommandOutcome);
    }
    Ok(())
}

fn pending_command_id(
    state: &DialogueActivationFrame,
) -> Option<&crate::presentation::RuntimeLineCommandId> {
    match state.pending_line_operation.as_ref()? {
        PendingLineOperation::AcquireActor { command, .. }
        | PendingLineOperation::ActorLook { command, .. }
        | PendingLineOperation::StartVoice { command, .. } => Some(command),
    }
}

fn clear_pending_command(
    state: &mut DialogueActivationFrame,
    command: &crate::presentation::RuntimeLineCommandId,
) {
    if pending_command_id(state).is_some_and(|pending| pending == command) {
        state.pending_line_operation = None;
    }
}

fn fail_issued_command_lease(
    ledger: &mut crate::line_task::RuntimeLineHandleLedger,
    command: &crate::presentation::RuntimeLineHostCommand,
) -> Result<(), LineRuntimeError> {
    let token = match command {
        crate::presentation::RuntimeLineHostCommand::Stage(command) => match command {
            crate::presentation::RuntimeStageCommand::AcquireActor { actor, .. }
            | crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. } => Some(actor),
            crate::presentation::RuntimeStageCommand::SetCharacterLook { cue, .. }
            | crate::presentation::RuntimeStageCommand::CancelCue { cue, .. } => Some(cue),
        },
        crate::presentation::RuntimeLineHostCommand::Voice(command) => match command {
            crate::presentation::RuntimeVoiceCommand::StartDialogueVoice { .. } => None,
            crate::presentation::RuntimeVoiceCommand::ReleaseDialogueVoice { handle, .. } => {
                Some(handle)
            }
        },
    };
    let Some(token) = token else {
        return Ok(());
    };
    let state_before = ledger
        .lease(token)
        .ok_or(LineRuntimeError::UnknownHandle)?
        .state();
    if state_before == RuntimeHandleLeaseState::Released {
        return Ok(());
    }
    ledger.set_state(token, state_before, RuntimeHandleLeaseState::Failed)?;
    ledger.set_state(
        token,
        RuntimeHandleLeaseState::Failed,
        RuntimeHandleLeaseState::Released,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ActivationScopeStep, DialogueActivationFrame, DialogueActivationScope,
        DialogueLineTaskState, DialogueRuntimePhase, Engine,
    };
    use crate::effect::{RuntimeDropPolicyExpr, RuntimeEffectExpr};
    use crate::line_task::{
        RuntimeDeferOutcomeFilter, RuntimeDialogueActivationState, RuntimeHandleLeaseState,
        RuntimeHandleOwnerSlot, RuntimeHandleResource, RuntimeLineHandleLedger,
        RuntimeLineHandleSite, RuntimeLineHandleSiteKind, RuntimeStageActorLease, ScopeExit,
    };
    use crate::pattern::{
        RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimePattern,
        RuntimePatternBindingCoordinate, RuntimePatternBindingPath, RuntimePatternBindingStep,
        RuntimePatternKind, RuntimeSemanticTypeId,
    };
    use crate::plan::{
        RuntimeDialogueResultTarget, RuntimeEffectSet, RuntimeExecutableBodySeed,
        RuntimeFlowOpSeed, RuntimeFunctionSiteBodyKind, RuntimeFunctionSiteBodySeed,
        RuntimeFunctionSiteDeclarationSeed, RuntimeLocalDeclarationSeed, RuntimePlanBuilder,
        RuntimePlanTypeProjection, RuntimePlanTypeSeed,
    };
    use crate::pure::VmRuntimePureCallBackend;
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeLineHandleSiteId,
        RuntimeLineTaskGroupId, RuntimeLocalDeclarationId, RuntimePersistentFiberId,
        RuntimePlanTypeId,
    };
    use crate::time::LogicalDuration;
    use crate::value::{RuntimeExpr, RuntimeExprKind, RuntimeValue};
    use std::num::NonZeroU32;

    fn activation_id() -> DialogueActivationId {
        DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x5e; 32])
                .expect("artifact"),
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            0,
        )
    }

    fn scoped_defer_engine() -> (Engine, crate::runtime_id::RuntimeDeferSiteId) {
        let unit = RuntimeSemanticTypeId::from_bytes([0x71; 32]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    unit,
                    RuntimePlanTypeProjection::Unit,
                )],
                [],
            )
            .expect("unit type");
        let function = builder
            .reserve_function_site_seed(RuntimeFunctionSiteDeclarationSeed {
                inputs: Box::new([]),
                result: unit,
                body_kind: RuntimeFunctionSiteBodyKind::Executable,
                effects: RuntimeEffectSet::empty(),
            })
            .expect("defer body");
        builder
            .define_function_site_seed(
                &function,
                RuntimeFunctionSiteBodySeed::Executable(RuntimeExecutableBodySeed {
                    effects: RuntimeEffectSet::empty(),
                    ops: Box::new([RuntimeFlowOpSeed::Noop]),
                }),
            )
            .expect("defer body definition");
        let site = builder
            .reserve_defer_site_seed(&function)
            .expect("defer site");
        (Engine::new(builder.finish().expect("plan")), site)
    }

    fn activation_frame(local: RuntimeLocalDeclarationId) -> DialogueActivationFrame {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let mut locals = crate::value::RuntimeEnv::default();
        locals.set(local, RuntimeValue::Unit);
        DialogueActivationFrame {
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line"),
            content: RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            target: crate::value::RuntimeOpaqueValue::new_exact(
                &crate::pattern::RuntimeOpaqueTypeOwner::exact(
                    crate::value::RuntimeCharacterDialogueProducerId::get(),
                    crate::pattern::RuntimeSemanticTypeId::from_bytes([0x47; 32]),
                ),
                RuntimeValue::Unit,
            ),
            task_group: RuntimeLineTaskGroupId::from_zero_based(0).expect("group"),
            resume: None,
            captures: Box::new([]),
            locals,
            line_task: DialogueLineTaskState::NotStarted,
            elapsed: LogicalDuration::default(),
            phase: DialogueRuntimePhase::Activating,
            result_target: RuntimeDialogueResultTarget::new(
                ty,
                RuntimePattern::from_admitted_parts(ty, RuntimePatternKind::Discard),
            ),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            values: Box::new([]),
            effect_callbacks: Box::new([]),
            activation_pc: 0,
            exiting_for_result: false,
            scopes: Vec::new(),
            pending_line_operation: None,
            pending_host_call: None,
            failure: None,
        }
    }

    #[test]
    fn dialogue_drop_source_take_is_committed_with_registry_revision() {
        let plan = RuntimePlanBuilder::new().finish().expect("empty plan");
        let mut engine = Engine::new(plan);
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let activation = DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x5e; 32])
                .expect("artifact"),
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            0,
        );
        engine
            .dialogue_activations
            .begin(activation.clone(), activation_frame(local))
            .expect("activation");
        let mut stale = engine
            .dialogue_activations
            .begin_transaction(&activation)
            .expect("stale candidate");
        let effect = RuntimeEffectExpr::Drop {
            target: RuntimeExpr::from_admitted_parts(
                RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
                RuntimeExprKind::Local(local),
            ),
            policy: RuntimeDropPolicyExpr::Default,
        };
        let mut pure = VmRuntimePureCallBackend::default();
        let (frame, line) = stale.parts_mut();
        engine
            .execute_dialogue_evaluated_effect(&activation, frame, line, &effect, &mut pure)
            .expect("drop candidate");
        assert!(stale.frame().locals.get(local).is_none());
        stale.frame_mut().locals.push_scope();
        stale
            .frame_mut()
            .scopes
            .push(DialogueActivationScope::new());

        let fresh = engine
            .dialogue_activations
            .begin_transaction(&activation)
            .expect("revision advance");
        engine
            .dialogue_activations
            .commit_transaction(fresh)
            .expect("advance revision");
        assert_eq!(
            engine.dialogue_activations.commit_transaction(stale),
            Err(crate::line_task::LineRuntimeError::StaleActivationTransaction)
        );
        let live = engine
            .dialogue_activations
            .begin_transaction(&activation)
            .expect("live frame");
        assert_eq!(live.frame().locals.get(local), Some(&RuntimeValue::Unit));
        assert!(live.frame().scopes.is_empty());
        assert!(engine.fiber.env.get(local).is_none());
    }

    #[test]
    fn init_scope_deferred_children_run_lifo_with_frozen_completion_filter() {
        let (engine, site) = scoped_defer_engine();
        let id = activation_id();
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let mut frame = activation_frame(local);
        frame.locals.push_scope();
        frame.scopes.push(DialogueActivationScope::new());
        let mut line = RuntimeDialogueActivationState::new();
        let first = line
            .allocate_deferred_registration(site, RuntimeDeferOutcomeFilter::Completed, vec![])
            .expect("first reached defer");
        let second = line
            .allocate_deferred_registration(site, RuntimeDeferOutcomeFilter::Completed, vec![])
            .expect("second reached defer");
        let failed = line
            .allocate_deferred_registration(site, RuntimeDeferOutcomeFilter::Failed, vec![])
            .expect("failed-only defer");
        frame.scopes[0]
            .deferred
            .extend([first.clone(), second.clone(), failed]);
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        line.commit_result(ty, RuntimeValue::Unit)
            .expect("result staged before scope exit");

        let ActivationScopeStep::Deferred(_) = engine
            .prepare_activation_scope_exit(&id, &mut frame, &mut line, ScopeExit::Completed)
            .expect("first scope step")
        else {
            panic!("the last matching registration must run first");
        };
        assert_eq!(frame.scopes[0].inflight, Some((second.id(), site)));
        assert_eq!(frame.scopes[0].exit, Some(ScopeExit::Completed));
        line.complete_scoped_deferred_child(&id, second.id(), &Default::default())
            .expect("second child closes");
        frame.scopes[0].inflight = None;

        let ActivationScopeStep::Deferred(_) = engine
            .prepare_activation_scope_exit(&id, &mut frame, &mut line, ScopeExit::Failed)
            .expect("frozen scope step")
        else {
            panic!("earlier completion registration must run next");
        };
        assert_eq!(frame.scopes[0].inflight, Some((first.id(), site)));
        assert_eq!(frame.scopes[0].exit, Some(ScopeExit::Completed));
        line.complete_scoped_deferred_child(&id, first.id(), &Default::default())
            .expect("first child closes");
        frame.scopes[0].inflight = None;

        assert!(matches!(
            engine.prepare_activation_scope_exit(&id, &mut frame, &mut line, ScopeExit::Failed),
            Ok(ActivationScopeStep::Finished)
        ));
        assert!(frame.scopes.is_empty());
        assert!(matches!(
            line.result(),
            crate::line_task::RuntimeDialogueResultState::Committed {
                value: RuntimeValue::Unit,
                ..
            }
        ));
    }

    #[test]
    fn init_out_unwinds_scope_without_resuming_later_plan_ops() {
        let plan = RuntimePlanBuilder::new().finish().expect("empty plan");
        let mut engine = Engine::new(plan);
        let id = activation_id();
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let mut frame = activation_frame(local);
        frame.locals.push_scope();
        frame.scopes.push(DialogueActivationScope::new());
        frame.activation_pc = 3;
        frame.exiting_for_result = true;
        engine
            .dialogue_activations
            .begin(id.clone(), frame)
            .expect("activation");
        let mut transaction = engine
            .dialogue_activations
            .begin_transaction(&id)
            .expect("transaction");
        transaction
            .line_mut()
            .commit_result(
                RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
                RuntimeValue::Unit,
            )
            .expect("staged result");

        assert!(matches!(
            engine.resume_dialogue_activation(
                &mut transaction,
                &[],
                &[],
                &mut VmRuntimePureCallBackend::default(),
            ),
            Ok(super::DialogueActivationStep::Continue)
        ));
        assert!(transaction.frame().scopes.is_empty());
        assert_eq!(transaction.frame().activation_pc, 3);
        assert!(matches!(
            transaction.line().result(),
            crate::line_task::RuntimeDialogueResultState::Committed { .. }
        ));
    }

    #[test]
    fn init_host_call_request_commits_before_reply_binds_continuation() {
        let unit = RuntimeSemanticTypeId::from_bytes([0x74; 32]);
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    unit,
                    RuntimePlanTypeProjection::Unit,
                )],
                [],
            )
            .expect("unit type");
        let mut engine = Engine::new(builder.finish().expect("plan"));
        let id = activation_id();
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let mut frame = activation_frame(local);
        frame.activation_pc = 2;
        let call_id = crate::step::RuntimeHostCallId("init.log".to_owned());
        frame.pending_host_call = Some(super::PendingActivationHostCall {
            id: call_id.clone(),
            result: RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            binding: None,
        });
        engine
            .dialogue_activations
            .begin(id.clone(), frame)
            .expect("activation");
        let transaction = engine
            .dialogue_activations
            .begin_transaction(&id)
            .expect("transaction");
        let request = crate::step::RuntimeHostCallRequest {
            id: call_id.clone(),
            public_id: "log.info".to_owned(),
            capability: "log".to_owned(),
            operation: "info".to_owned(),
            contract: None,
            args: Vec::new(),
            named_args: Vec::new(),
            result: unit,
            mode: crate::step::RuntimeHostCallMode::Suspend,
            deterministic: true,
        };
        let mut output = crate::step::RuntimeStepOutput::default();
        engine.commit_and_suspend_dialogue(
            transaction,
            &mut output,
            super::DialogueActivationStep::HostCall(request.clone()),
        );
        assert_eq!(output.requests.host_calls, vec![request]);
        let mut transaction = engine
            .dialogue_activations
            .begin_transaction(&id)
            .expect("committed host call");
        let reply = crate::step::RuntimeHostCallResult {
            id: call_id,
            outcome: Ok(crate::value::RuntimePayload::from(RuntimeValue::Unit)),
        };
        assert!(matches!(
            engine.resume_dialogue_activation(
                &mut transaction,
                &[],
                &[reply],
                &mut VmRuntimePureCallBackend::default(),
            ),
            Ok(super::DialogueActivationStep::Continue)
        ));
        assert!(transaction.frame().pending_host_call.is_none());
        assert_eq!(transaction.frame().activation_pc, 3);
    }

    #[test]
    fn init_evaluated_log_effect_publishes_after_activation_commit() {
        let plan = RuntimePlanBuilder::new().finish().expect("empty plan");
        let mut engine = Engine::new(plan);
        let id = activation_id();
        let local = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        engine
            .dialogue_activations
            .begin(id.clone(), activation_frame(local))
            .expect("activation");
        let mut transaction = engine
            .dialogue_activations
            .begin_transaction(&id)
            .expect("transaction");
        let effect = RuntimeEffectExpr::Log {
            level: "info".to_owned(),
            message: RuntimeExpr::from_admitted_parts(
                RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
                RuntimeExprKind::Value(RuntimeValue::String("line init".to_owned())),
            ),
            fields: Vec::new(),
        };
        let (frame, line) = transaction.parts_mut();
        let request = engine
            .execute_dialogue_evaluated_effect(
                &id,
                frame,
                line,
                &effect,
                &mut VmRuntimePureCallBackend::default(),
            )
            .expect("evaluated effect")
            .expect("log request");
        assert!(matches!(request, crate::effect::LineEffectRequest::Log(_)));
        let mut output = crate::step::RuntimeStepOutput::default();
        engine.commit_and_suspend_dialogue(
            transaction,
            &mut output,
            super::DialogueActivationStep::Effect(request.clone()),
        );
        assert_eq!(output.effects.line, vec![request]);
        assert!(engine.dialogue_activations.begin_transaction(&id).is_ok());
    }

    #[test]
    fn init_scope_let_moves_affine_custody_and_exit_journals_release() {
        let actor_type = RuntimeSemanticTypeId::from_bytes([0x73; 32]);
        let producer = crate::value::RuntimeHandleKind::StageActor
            .try_producer()
            .expect("producer");
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                [RuntimePlanTypeSeed::new(
                    actor_type,
                    RuntimePlanTypeProjection::Opaque {
                        producer: producer.clone(),
                        admission: RuntimeOpaqueTypeAdmission::ExactIdentity,
                        value_class: crate::value::RuntimeOpaqueValueClass::AffineHandle(
                            crate::value::RuntimeHandleKind::StageActor,
                        ),
                        persistence: crate::value::RuntimeOpaquePersistence::SnapshotOnly,
                        arguments: Box::new([]),
                    },
                )],
                [
                    RuntimeLocalDeclarationSeed::new(actor_type),
                    RuntimeLocalDeclarationSeed::new(actor_type),
                ],
            )
            .expect("local declaration");
        let mut engine = Engine::new(builder.finish().expect("plan"));
        let id = activation_id();
        let source = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let destination = RuntimeLocalDeclarationId::from_accepted_ordinal(
            NonZeroU32::new(2).expect("second local"),
        );
        let mut frame = activation_frame(source);
        frame.locals.push_scope();
        frame.scopes.push(DialogueActivationScope::new());
        let character =
            arcweft_character::id::CharacterId::try_new("character.fixture").expect("character");
        let site = RuntimeLineHandleSite::new(
            RuntimeLineHandleSiteId::from_zero_based(0),
            0,
            RuntimeLineHandleSiteKind::StageActor,
            RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            Some(character.clone()),
            None,
            RuntimeOpaqueTypeOwner::exact_with(
                producer,
                actor_type,
                crate::value::RuntimeOpaqueValueClass::AffineHandle(
                    crate::value::RuntimeHandleKind::StageActor,
                ),
                crate::value::RuntimeOpaquePersistence::SnapshotOnly,
            ),
        )
        .expect("site");
        let mut ledger = RuntimeLineHandleLedger::default();
        let owner = RuntimeHandleOwnerSlot::ActivationLocal(
            engine.owned_slot(source).expect("local owner"),
        );
        let actor = ledger
            .issue(
                &id,
                &site,
                RuntimeHandleResource::StageActor(RuntimeStageActorLease::new(character)),
                owner,
            )
            .expect("actor issue");
        let token = crate::runtime_id::RuntimeLineHandleToken::try_decode_payload(actor.payload())
            .expect("actor token");
        ledger
            .set_state(
                &token,
                RuntimeHandleLeaseState::Allocating,
                RuntimeHandleLeaseState::Active,
            )
            .expect("actor acquired");
        frame.locals.set(source, RuntimeValue::Opaque(actor));
        let mut line = RuntimeDialogueActivationState::new();
        line.commit_ledger(ledger);
        let path = RuntimePatternBindingPath::try_from_steps([RuntimePatternBindingStep::Whole])
            .expect("whole binding");
        let pattern = RuntimePattern::from_admitted_parts(
            RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            RuntimePatternKind::Bind {
                mutable: false,
                binding: RuntimePatternBindingCoordinate::from_admitted_parts(destination, path),
            },
        );
        let expr = RuntimeExpr::from_admitted_parts(
            RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN),
            RuntimeExprKind::Local(source),
        );
        engine
            .bind_dialogue_let(
                &id,
                &mut frame,
                &mut line,
                &pattern,
                &expr,
                &mut VmRuntimePureCallBackend::default(),
            )
            .expect("affine let transfers owner");
        assert_eq!(frame.locals.get(source), Some(&RuntimeValue::Unit));
        assert!(matches!(
            frame.locals.get(destination),
            Some(RuntimeValue::Opaque(_))
        ));
        assert_eq!(
            line.ledger().lease(&token).expect("lease").owner(),
            &RuntimeHandleOwnerSlot::ActivationLocal(
                engine.owned_slot(destination).expect("destination owner"),
            )
        );

        assert!(matches!(
            engine.prepare_activation_scope_exit(&id, &mut frame, &mut line, ScopeExit::Completed),
            Ok(ActivationScopeStep::Finished)
        ));
        assert!(frame.scopes.is_empty());
        assert_eq!(frame.locals.get(source), Some(&RuntimeValue::Unit));
        assert!(frame.locals.get(destination).is_none());
        assert!(line.has_pending_commands());
        assert!(matches!(
            line.take_commit_receipt().into_commands().as_slice(),
            [crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::ReleaseActor { .. }
            )]
        ));
    }
}
