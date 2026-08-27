mod store;

pub(in crate::engine) use store::{
    DialogueActivationFrame, DialogueActivationStore, DialogueActivationTransaction,
    DialogueCommitDisposition, DialogueLineTaskState, DialogueRuntimePhase, PendingLineOperation,
};

use super::{
    Engine, RuntimeDiagnostic, RuntimeEvalError, RuntimeLocalBinding, RuntimeStepOutput,
    RuntimeValue,
};
use crate::effect::{RuntimeDropPolicy, RuntimeDropPolicyExpr, RuntimeEffectExpr};
use crate::line_task::{
    LineRuntimeError, LineTaskLiveState, LineTaskReadyEvents, MAX_LINE_SCHEDULED_CALLBACKS,
    RuntimeCueLease, RuntimeCueOrigin, RuntimeDialogueActivationState, RuntimeDialogueResultState,
    RuntimeHandleLeaseState, RuntimeHandleOwnerSlot, RuntimeHandleResource,
    RuntimeLineHandleSiteKind, RuntimeScheduledLineTask, RuntimeStageActorLease, RuntimeVoiceLease,
    progress_live_line_task_group,
};
use crate::pattern::{RuntimePattern, match_runtime_pattern};
use crate::plan::{FlowEvent, FlowOp, RuntimeLineOperation};
use crate::presentation::{
    RuntimeCommandQueue, RuntimeDialogueVoiceState, RuntimeLineHostOutcome,
    RuntimeStageCommandOutcome, RuntimeVoiceCommandOutcome,
};
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{RuntimeLineHandleToken, RuntimePlanTypeId};
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
}

pub(super) struct DialogueLineTaskStart {
    pub(super) event: Option<FlowEvent>,
    pub(super) request_cancellation: bool,
    pub(super) group: crate::line_task::LineTaskGroup,
    pub(super) activation: crate::line_task::LineTaskActivation,
    pub(super) captures: Box<[RuntimeLocalBinding]>,
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
        self.publish_dialogue_line_receipt(receipt.into_line(), output);
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
        self.publish_dialogue_line_receipt(line, output);
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
        self.publish_dialogue_line_receipt(line, output);
        Ok(disposition)
    }

    pub(super) fn publish_dialogue_line_receipt(
        &mut self,
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
        start: Option<DialogueLineTaskStart>,
    ) {
        let activation = transaction.activation().clone();
        let (transaction, batch, event) = match start {
            Some(start) => {
                let mut candidate = transaction.clone();
                let batch = match self.prepare_line_task_commands(
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

    pub(super) fn resume_dialogue_failure_close(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        output: &mut RuntimeStepOutput,
    ) {
        let activation_id = transaction.activation().clone();
        let (state, activation) = transaction.parts_mut();
        if let Err(error) = activation.abandon() {
            self.fail_eval(error, output);
            return;
        }
        if let Err(cleanup) = self.unwind_dialogue_handles(&activation_id, activation, false) {
            output.diagnostics.push(RuntimeDiagnostic::new(format!(
                "dialogue cleanup after primary failure also failed: {cleanup}"
            )));
        }
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
        let (state, activation) = transaction.parts_mut();
        state.phase = DialogueRuntimePhase::Closing;
        if let Err(error) = self.unwind_dialogue_handles(&activation_id, activation, true) {
            self.begin_dialogue_failure(transaction, error, output);
            return;
        }
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
        &mut self,
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
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<Option<DialogueLineTaskStart>, DialogueExecutionError> {
        let activation_id = transaction.activation().clone();
        let (frame, activation) = transaction.parts_mut();
        if frame.pending_line_operation.is_some() {
            match self.resume_pending_line_operation(&activation_id, frame, activation, outcomes) {
                Ok(true) => advance_activation_pc(frame)?,
                Ok(false) => {}
                Err(error) => return Err(error),
            }
            return Ok(None);
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
        let mut line_task_start = None;
        match operation {
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
                line_task_start =
                    Some(self.commit_dialogue_result(&activation_id, frame, activation, value)?);
            }
            FlowOp::EvaluatedEffect(effect) => {
                self.execute_dialogue_evaluated_effect(
                    &activation_id,
                    frame,
                    activation,
                    &effect,
                    pure_backend,
                )?;
            }
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        }
        if frame.pending_line_operation.is_none() {
            advance_activation_pc(frame)?;
        }
        Ok(line_task_start)
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

    fn commit_dialogue_result(
        &mut self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        state: &mut DialogueActivationFrame,
        activation: &mut NativeDialogueActivationState,
        value: RuntimeValue,
    ) -> Result<DialogueLineTaskStart, DialogueExecutionError> {
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
            LineTaskReadyEvents::new(
                &std::collections::BTreeSet::new(),
                &std::collections::BTreeSet::new(),
            ),
            &mut live,
        )?;
        state.line_task = DialogueLineTaskState::Live(live);
        state.phase = DialogueRuntimePhase::Ready;
        Ok(DialogueLineTaskStart {
            event: Some(FlowEvent::DialogueLine {
                activation: activation_id.clone(),
                line: state.line.clone(),
                values: state.values.clone(),
            }),
            request_cancellation: false,
            group,
            activation: line_task_activation,
            captures: state.captures.clone(),
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
    ) -> Result<(), DialogueExecutionError> {
        let RuntimeEffectExpr::Drop { target, policy } = effect else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
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
        result
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
    use super::{DialogueActivationFrame, DialogueLineTaskState, DialogueRuntimePhase, Engine};
    use crate::effect::{RuntimeDropPolicyExpr, RuntimeEffectExpr};
    use crate::pattern::{RuntimePattern, RuntimePatternKind};
    use crate::plan::{RuntimeDialogueResultTarget, RuntimePlanBuilder};
    use crate::pure::VmRuntimePureCallBackend;
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeLineTaskGroupId,
        RuntimeLocalDeclarationId, RuntimePersistentFiberId, RuntimePlanTypeId,
    };
    use crate::time::LogicalDuration;
    use crate::value::{RuntimeExpr, RuntimeExprKind, RuntimeValue};
    use std::num::NonZeroU32;

    fn activation_frame(local: RuntimeLocalDeclarationId) -> DialogueActivationFrame {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let mut locals = crate::value::RuntimeEnv::default();
        locals.set(local, RuntimeValue::Unit);
        DialogueActivationFrame {
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line"),
            content: RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
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
            activation_pc: 0,
            pending_line_operation: None,
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
        assert_eq!(
            engine
                .dialogue_activations
                .begin_transaction(&activation)
                .expect("live frame")
                .frame()
                .locals
                .get(local),
            Some(&RuntimeValue::Unit)
        );
        assert!(engine.fiber.env.get(local).is_none());
    }
}
