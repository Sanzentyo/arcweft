use super::dialogue::ProductDialogueTransaction;
use super::{
    ActiveDialogue, AwbcLineTaskPlanView, ProductDialogueClosing, ProductDialogueClosingState,
    ProductDialoguePhase, ProductPendingLineOperation, ProductStepError,
};
use crate::awbc::fiber::{FiberCursor, FiberState, runtime_value_matches_type};
use crate::awbc::schema::{
    AwbcChildJoinPolicy, AwbcLineHandleSite, AwbcLineOperation, AwbcLineTaskGroupId,
    AwbcLineTaskNode, AwbcRuntimeTypeShape, AwbcTypeId,
};
use crate::awbc::vm::{VmExit, VmObservation, VmStepOptions};
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineRuntimeError, LineTaskActivation, LineTaskCommand,
    LineTaskLiveState, LineTaskReadyEvents, LineTaskWork, LineTaskWorkTag, RuntimeCueLease,
    RuntimeCueOrigin, RuntimeDialogueActivationState, RuntimeDialogueResultState,
    RuntimeHandleLeaseState, RuntimeHandleOwnerSlot, RuntimeHandleResource,
    RuntimeLineHandleLedger, RuntimeScheduledLineTask, RuntimeStageActorLease, RuntimeVoiceLease,
    complete_live_line_task_work, progress_live_line_task_group,
};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::presentation::{
    RuntimeCommandQueue, RuntimeDialogueVoiceState, RuntimeLineHostOutcome,
    RuntimeStageCommandOutcome, RuntimeVoiceCommandOutcome,
};
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{DialogueActivationId, RuntimeLineHandleSiteId, RuntimeLineHandleToken};
use crate::time::LogicalDuration;
use crate::value::ownership::RuntimeOwnedSlotId;
use crate::value::{RuntimeHandleKind, RuntimeLocalBinding, RuntimePayload, RuntimeValue};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

pub(super) struct ProductActivationProgress {
    pub(super) progressed: bool,
    pub(super) presented: Option<crate::plan::FlowEvent>,
    pub(super) reducer: LineTaskActivation,
    pub(super) pure_stats: Option<crate::step::RuntimePureCallStats>,
    pub(super) execution: Option<super::ProductLineTaskExecutionBatch>,
    pub(super) host_calls: Vec<crate::step::RuntimeHostCallRequest>,
}

pub(super) enum ProductPublicationProgress {
    Pending,
    Ready(FiberState),
}

struct ProductLineSiteEvidence {
    runtime_site: RuntimeLineHandleSiteId,
    site: AwbcLineHandleSite,
    opaque_owner: RuntimeOpaqueTypeOwner,
}

impl super::AwbcProductStepExecutor {
    pub(super) fn prepare_dialogue_publication(
        &self,
        transaction: &mut ProductDialogueTransaction,
        resume: crate::awbc::schema::AwbcResumePointId,
    ) -> Result<ProductPublicationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
        let (frame, line) = transaction.parts_mut();
        let line_task = match &frame.phase {
            ProductDialoguePhase::Reducing { line_task }
            | ProductDialoguePhase::Publishing { line_task } => line_task.clone(),
            ProductDialoguePhase::Activating { .. } => {
                return Err(LineRuntimeError::ResultNotCommitted.into());
            }
            ProductDialoguePhase::Closing(_) => {
                return Err(LineRuntimeError::InvalidResultTransition.into());
            }
        };
        if !line_task.is_closed() {
            return Err(LineRuntimeError::ResultNotCommitted.into());
        }
        let (ty, value, begin) = match line.result().clone() {
            RuntimeDialogueResultState::Committed { ty, value }
            | RuntimeDialogueResultState::Selected { ty, value, .. } => (ty, value, true),
            RuntimeDialogueResultState::Publishing { ty, value } => (ty, value, false),
            RuntimeDialogueResultState::Uncommitted
            | RuntimeDialogueResultState::Published
            | RuntimeDialogueResultState::Abandoned => {
                return Err(LineRuntimeError::ResultNotCommitted.into());
            }
        };
        if ty != frame.result.ty
            || !runtime_value_matches_type(&self.program, &value, frame.result.ty, 0)
        {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }

        let mut parent = self.fiber.clone();
        crate::awbc::vm::bind_pattern(&self.program, &mut parent, frame.result.pattern, &value)
            .map_err(|_| LineRuntimeError::ResultPatternOrTypeMismatch)?;
        let parent_owners =
            parent_fiber_handle_owners(self.facade_fiber.execution, &activation, &parent)?;
        let result_handles = unique_line_handles(&value)?;
        let result_tokens = result_handles
            .iter()
            .map(|handle| handle.token().clone())
            .collect::<BTreeSet<_>>();
        if parent_owners
            .keys()
            .any(|token| !result_tokens.contains(token))
        {
            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
        }

        if begin {
            let mut ledger = line.ledger().clone();
            let mut commands =
                RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
            let mut emitted_command = false;
            let mut remaining = ledger
                .leases()
                .values()
                .filter(|lease| {
                    lease.state() != RuntimeHandleLeaseState::Released
                        && !matches!(lease.owner(), RuntimeHandleOwnerSlot::DialogueResult(_))
                })
                .map(|lease| (lease.token().clone(), lease.owner().clone()))
                .collect::<Vec<_>>();
            remaining.reverse();
            for (token, owner) in remaining {
                let before_sequence = commands.next_sequence();
                ledger.drop_owned(&token, &owner, &mut commands)?;
                emitted_command |= commands.next_sequence() != before_sequence;
            }
            for handle in &result_handles {
                let expected = RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone());
                if let Some(destination) = parent_owners.get(handle.token()) {
                    ledger.transfer(
                        handle.token(),
                        &expected,
                        RuntimeHandleOwnerSlot::ParentFiber(*destination),
                    )?;
                } else {
                    let before_sequence = commands.next_sequence();
                    ledger.drop_owned(handle.token(), &expected, &mut commands)?;
                    emitted_command |= commands.next_sequence() != before_sequence;
                }
            }
            line.commit_ledger(ledger);
            if emitted_command {
                line.record_commands(&activation, commands)?;
            }
            line.begin_result_publication()?;
            frame.phase = ProductDialoguePhase::Publishing { line_task };
        } else {
            for handle in &result_handles {
                let expected = match parent_owners.get(handle.token()) {
                    Some(destination) => RuntimeHandleOwnerSlot::ParentFiber(*destination),
                    None => continue,
                };
                if line
                    .ledger()
                    .lease(handle.token())
                    .is_none_or(|lease| lease.owner() != &expected)
                {
                    return Err(LineRuntimeError::WrongOwner.into());
                }
            }
        }
        if line.has_pending_commands() {
            return Ok(ProductPublicationProgress::Pending);
        }
        if line.ledger().leases().values().any(|lease| {
            lease.state() != RuntimeHandleLeaseState::Released
                && !matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
        }) {
            return Err(LineRuntimeError::UnownedLeaseAtPublish.into());
        }
        line.finish_result_publication()?;
        line.release_frame()?;
        parent.resume_at(&self.program, resume)?;
        Ok(ProductPublicationProgress::Ready(parent))
    }

    pub(super) fn prepare_line_task_commands(
        &self,
        transaction: &mut ProductDialogueTransaction,
        activation: LineTaskActivation,
    ) -> Result<super::ProductLineTaskExecutionBatch, ProductStepError> {
        let mut candidate = transaction.clone();
        let batch = super::ProductLineTaskExecutionBatch {
            child_fibers: self.child_fibers.clone(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
        };
        let batch = self.prepare_line_task_commands_from(&mut candidate, activation, batch)?;
        *transaction = candidate;
        Ok(batch)
    }

    pub(super) fn prepare_line_task_commands_from(
        &self,
        transaction: &mut ProductDialogueTransaction,
        activation: LineTaskActivation,
        mut batch: super::ProductLineTaskExecutionBatch,
    ) -> Result<super::ProductLineTaskExecutionBatch, ProductStepError> {
        let mut pending = VecDeque::from(activation.commands);
        let mut scheduled_completions = VecDeque::from(activation.scheduled_completions);
        while let Some(completion) = scheduled_completions.pop_front() {
            transaction
                .line_mut()
                .complete_unstarted_scheduled(&completion)?;
        }
        while let Some(command) = pending.pop_front() {
            match command {
                LineTaskCommand::Run { tag, policy } => {
                    let (content, group_captures) = {
                        let frame = transaction.frame();
                        (frame.content, frame.captures.clone())
                    };
                    let view = self
                        .line_task_view(content)
                        .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                    let function = view
                        .function_for(&tag)
                        .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                    let args = if let Some(token) = tag.scheduled_token().cloned() {
                        transaction
                            .line_mut()
                            .take_scheduled_capture_packet(&token)?
                            .into_vec()
                            .into_iter()
                            .map(|capture| capture.value)
                            .collect()
                    } else {
                        group_captures.into_vec()
                    };
                    if policy.join == ChildJoinPolicy::Detached {
                        for value in &args {
                            if !unique_line_handles(value)?.is_empty() {
                                return Err(LineRuntimeError::DetachedAffineCapture.into());
                            }
                        }
                    }
                    let phase = if matches!(tag.work(), LineTaskWork::Node(_)) {
                        super::ProductLineTaskFiberPhase::Active
                    } else {
                        super::ProductLineTaskFiberPhase::Closing
                    };
                    batch.spawn(
                        self,
                        super::ProductChildFiberOwner::LineTask {
                            content,
                            tag,
                            policy,
                            phase,
                        },
                        function,
                        args,
                    )?;
                }
                LineTaskCommand::Cancel { tag } => {
                    let completions =
                        batch.cancel_line_task_children(self, transaction.frame().content, &tag)?;
                    for (tag, failed, cancelled) in completions {
                        if let Some(token) = tag.scheduled_token().cloned() {
                            transaction
                                .line_mut()
                                .complete_scheduled_work(&token, failed, cancelled)?;
                        }
                        let view = self
                            .line_task_view(transaction.frame().content)
                            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                        let line_task = transaction
                            .frame_mut()
                            .line_task_mut()
                            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                        let completion =
                            complete_live_line_task_work(&view, line_task, tag, failed)?;
                        pending.extend(completion.commands);
                        scheduled_completions.extend(completion.scheduled_completions);
                        while let Some(completion) = scheduled_completions.pop_front() {
                            transaction
                                .line_mut()
                                .complete_unstarted_scheduled(&completion)?;
                        }
                    }
                }
            }
        }
        Ok(batch)
    }

    pub(super) fn prepare_next_deferred_child(
        &self,
        transaction: &mut ProductDialogueTransaction,
        exit: crate::line_task::ScopeExit,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) -> Result<bool, ProductStepError> {
        let activation = transaction.activation().clone();
        let mut candidate = transaction.clone();
        let mut candidate_batch = batch.clone();
        if candidate.line().deferred_exit().is_none()
            && candidate.line().deferred_registrations().is_empty()
        {
            return Ok(false);
        }
        if candidate.line().has_pending_commands() {
            return Ok(false);
        }
        if candidate.line().deferred_exit().is_none() {
            candidate.line_mut().begin_deferred_unwind(exit)?;
        }
        if candidate.line().deferred_inflight().is_some() {
            return Ok(false);
        }
        let Some(step) = candidate.line_mut().prepare_next_deferred(&activation)? else {
            *transaction = candidate;
            *batch = candidate_batch;
            return Ok(false);
        };
        match step {
            crate::line_task::RuntimeDeferUnwindStep::Skipped(_) => {
                *transaction = candidate;
                *batch = candidate_batch;
                Ok(true)
            }
            crate::line_task::RuntimeDeferUnwindStep::Run(registration) => {
                let (registration_id, site, _outcome, captures) = registration.into_parts();
                if candidate.line().deferred_inflight() != Some((registration_id, site)) {
                    return Err(LineRuntimeError::InvalidDeferredTransition.into());
                }
                let function = self
                    .program
                    .defer_sites
                    .get(site.index())
                    .copied()
                    .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
                let content = candidate.frame().content;
                candidate_batch.spawn(
                    self,
                    super::ProductChildFiberOwner::Deferred {
                        content,
                        activation,
                        registration: registration_id,
                        site,
                    },
                    function,
                    captures,
                )?;
                *transaction = candidate;
                *batch = candidate_batch;
                Ok(true)
            }
        }
    }

    pub(super) fn commit_line_task_commands(
        &mut self,
        batch: super::ProductLineTaskExecutionBatch,
        output: &mut crate::step::RuntimeStepOutput,
    ) {
        self.child_fibers = batch.child_fibers;
        self.dialogue_effect_callback_activations = batch.dialogue_effect_callback_activations;
        self.next_generation = batch.next_generation;
        self.next_fiber_instance = batch.next_fiber_instance;
        if let Some(stats) = batch.pure_stats {
            self.compact_pure_stats = stats;
        }
        self.consume_observations(batch.observations, output);
    }

    #[cfg(test)]
    pub(super) fn step_dialogue_activation(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        self.step_dialogue_activation_with_host_results(transaction, &[], pure_backend)
    }

    pub(super) fn step_dialogue_activation_with_host_results(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        host_results: &[crate::step::RuntimeHostCallResult],
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
        let outcomes = std::mem::take(&mut transaction.frame_mut().pending_line_outcomes);
        let has_pending_operation = matches!(
            transaction.frame().phase,
            ProductDialoguePhase::Activating {
                pending: Some(_),
                ..
            }
        );
        if has_pending_operation && self.resume_pending_line_operation(transaction, &outcomes)? {
            return Ok(ProductActivationProgress {
                progressed: true,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
            });
        }
        if !has_pending_operation && transaction.line().has_pending_commands() {
            if outcomes.is_empty() {
                return Ok(ProductActivationProgress {
                    progressed: false,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: None,
                    execution: None,
                    host_calls: Vec::new(),
                });
            }
            let diagnostics = transaction.line_mut().accept_runtime_outcomes(&outcomes)?;
            if let Some(error) = diagnostics.into_iter().next() {
                return Err(error.into());
            }
            settle_scoped_defer_releases(transaction);
            return Ok(ProductActivationProgress {
                progressed: true,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
            });
        }
        if !outcomes.is_empty() {
            return Err(LineRuntimeError::StaleCommandOutcome.into());
        }
        settle_scoped_defer_releases(transaction);
        if let Some(pending) = transaction.frame().pending_activation_host_call.clone() {
            return self.resume_activation_host_call(transaction, pending, host_results);
        }
        if matches!(
            transaction.line().result(),
            RuntimeDialogueResultState::Committed { .. }
        ) {
            return self.resume_activation_out(transaction);
        }
        let (frame, line) = transaction.parts_mut();
        let before = match &frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => fiber.clone(),
            ProductDialoguePhase::Activating {
                pending: Some(_), ..
            }
            | ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Closing(_) => {
                return Ok(ProductActivationProgress {
                    progressed: false,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: None,
                    execution: None,
                    host_calls: Vec::new(),
                });
            }
        };
        if fiber_has_scoped_defer_inflight(&before) {
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
            });
        }
        let mut candidate = before.clone();
        let mut candidate_stats = self.compact_pure_stats;
        let mut host = super::ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
            context: crate::awbc::vm::VmExecutionContext::for_program(
                self.artifact_fingerprint,
                Arc::clone(&self.program),
            ),
            program_owner: crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
        };
        let context = crate::awbc::vm::VmExecutionContext::for_program(
            self.artifact_fingerprint,
            Arc::clone(&self.program),
        );
        let step = crate::awbc::vm::step_with_host_context(
            &self.program,
            &mut candidate,
            VmStepOptions {
                max_instructions: 1,
            },
            &context,
            &mut host,
        )
        .map_err(|error| ProductStepError::Internal(error.to_string()))?;

        let mut owned_observation = None;
        let mut line_defer_observation = None;
        let mut scoped_defer_observation = None;
        let mut scoped_unwind_observation = None;
        let mut scoped_failure = None;
        let mut activation_effects = Vec::new();
        let mut drop_policy = None;
        for observation in step.observations {
            match observation {
                VmObservation::Instruction { .. } => {}
                VmObservation::Drop { policy } => {
                    if drop_policy.replace(policy).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::LineOperation { .. } | VmObservation::DialogueResult { .. } => {
                    if owned_observation.replace(observation).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::LineDeferRegistration { .. } => {
                    if line_defer_observation.replace(observation).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::ScopedDeferRegistration { .. } => {
                    if scoped_defer_observation.replace(observation).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::ScopedDeferUnwind { .. } => {
                    if scoped_unwind_observation.replace(observation).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::ScopedDeferFailure(trap) => {
                    if scoped_failure.replace(trap).is_some() {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                VmObservation::Effect { .. } => activation_effects.push(observation),
                VmObservation::Trap(trap) => {
                    return Err(ProductStepError::ActivationTrap(trap));
                }
                _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
            }
        }

        let deferred_tokens = match line_defer_observation {
            Some(VmObservation::LineDeferRegistration {
                cursor,
                site,
                outcome,
                captures,
            }) => self.register_line_root_defer(
                &activation,
                line,
                &mut candidate,
                cursor,
                site,
                outcome,
                captures,
            )?,
            Some(_) => return Err(LineRuntimeError::InvalidActivationOperation.into()),
            None => BTreeSet::new(),
        };
        match scoped_defer_observation {
            Some(VmObservation::ScopedDeferRegistration {
                cursor,
                scope,
                site,
                outcome,
                captures,
            }) => self.register_scoped_defer(
                &activation,
                line,
                &mut candidate,
                cursor,
                scope,
                site,
                outcome,
                captures,
            )?,
            Some(_) => return Err(LineRuntimeError::InvalidActivationOperation.into()),
            None => {}
        }
        let (mut execution, scoped_reconciled_tokens) = match scoped_unwind_observation {
            Some(VmObservation::ScopedDeferUnwind { cursor, scope }) => {
                let mut batch = super::ProductLineTaskExecutionBatch {
                    child_fibers: self.child_fibers.clone(),
                    dialogue_effect_callback_activations: self
                        .dialogue_effect_callback_activations
                        .clone(),
                    next_generation: self.next_generation,
                    next_fiber_instance: self.next_fiber_instance,
                    observations: Vec::new(),
                    pure_stats: None,
                };
                let tokens = self.prepare_scoped_defer_unwind(
                    &activation,
                    line,
                    &mut candidate,
                    cursor,
                    scope,
                    crate::line_task::ScopeExit::Completed,
                    &mut batch,
                )?;
                (Some(batch), tokens)
            }
            Some(_) => return Err(LineRuntimeError::InvalidActivationOperation.into()),
            None => (None, BTreeSet::new()),
        };
        if !activation_effects.is_empty() {
            let batch = execution.get_or_insert_with(|| empty_activation_batch(self));
            batch.observations.extend(activation_effects);
        }
        self.reconcile_activation_fiber_ownership(
            &activation,
            line,
            &before,
            &candidate,
            drop_policy,
            &deferred_tokens,
            &scoped_reconciled_tokens,
        )?;
        if let Some(trap) = scoped_failure {
            frame.phase = ProductDialoguePhase::Activating {
                fiber: candidate,
                pending: None,
            };
            return Err(ProductStepError::ActivationTrap(trap));
        }
        match owned_observation {
            Some(VmObservation::LineOperation {
                cursor,
                dst,
                operation,
                args,
            }) => {
                let pending_operation = self.execute_product_line_operation(
                    &activation,
                    frame,
                    line,
                    &mut candidate,
                    cursor,
                    dst,
                    operation,
                    &args,
                )?;
                let progressed = pending_operation.is_none();
                frame.phase = ProductDialoguePhase::Activating {
                    fiber: candidate,
                    pending: pending_operation,
                };
                Ok(ProductActivationProgress {
                    progressed,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: Some(candidate_stats),
                    execution: None,
                    host_calls: Vec::new(),
                })
            }
            Some(VmObservation::DialogueResult {
                cursor,
                source_register,
                source,
            }) => {
                let mut progress = self.commit_product_dialogue_result(
                    &activation,
                    frame,
                    line,
                    &mut candidate,
                    cursor,
                    source_register,
                    source,
                )?;
                progress.pure_stats = Some(candidate_stats);
                Ok(progress)
            }
            Some(_) => unreachable!("owned observation variants are exhaustive"),
            None => match step.exit {
                VmExit::Running | VmExit::BudgetYield(_) => {
                    frame.phase = ProductDialoguePhase::Activating {
                        fiber: candidate,
                        pending: None,
                    };
                    Ok(ProductActivationProgress {
                        progressed: true,
                        presented: None,
                        reducer: LineTaskActivation::default(),
                        pure_stats: Some(candidate_stats),
                        execution,
                        host_calls: Vec::new(),
                    })
                }
                VmExit::Returned(_) => Err(LineRuntimeError::ResultNotCommitted.into()),
                VmExit::Cancelled => Err(ProductStepError::ActivationTrap(
                    crate::awbc::fiber::FiberTrap {
                        code: crate::awbc::schema::AwbcTrapCode::InternalInvariant,
                        message: Some("line activation was cancelled".to_owned()),
                        source_map: None,
                    },
                )),
                VmExit::Trapped(trap) => Err(ProductStepError::ActivationTrap(trap)),
                VmExit::Suspended(crate::awbc::fiber::FiberSuspensionReason::HostCall {
                    call,
                    args,
                    ..
                }) => {
                    let (pending, request) =
                        self.activation_host_call_request(call, &args, None)?;
                    frame.pending_activation_host_call = Some(pending);
                    frame.phase = ProductDialoguePhase::Activating {
                        fiber: candidate,
                        pending: None,
                    };
                    Ok(ProductActivationProgress {
                        progressed: false,
                        presented: None,
                        reducer: LineTaskActivation::default(),
                        pure_stats: Some(candidate_stats),
                        execution: None,
                        host_calls: vec![request],
                    })
                }
                VmExit::Suspended(reason) => Err(ProductStepError::Internal(format!(
                    "line activation suspended outside a supported host call: {reason:?}"
                ))),
            },
        }
    }

    fn resume_activation_host_call(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        pending: super::PendingHostCall,
        host_results: &[crate::step::RuntimeHostCallResult],
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let (frame, _) = transaction.parts_mut();
        let (fiber, resume, destination, args) = match &mut frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => {
                let (resume, destination, args) = {
                    let suspension = fiber
                        .suspension
                        .as_ref()
                        .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                    let crate::awbc::fiber::FiberSuspensionReason::HostCall {
                        call,
                        args,
                        destination,
                    } = &suspension.reason
                    else {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    };
                    if *call != pending.call {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                    let crate::awbc::fiber::FiberResumeTarget::Declared(resume) = suspension.resume
                    else {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    };
                    (resume, *destination, args.clone())
                };
                (fiber, resume, destination, args)
            }
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        };
        let Some(result) = host_results.iter().find(|result| result.id == pending.id) else {
            let (_, request) =
                self.activation_host_call_request(pending.call, &args, Some(pending.clone()))?;
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: vec![request],
            });
        };
        let value = match &result.outcome {
            Ok(value) => value.value(),
            Err(error) => {
                let trap = crate::awbc::fiber::FiberTrap {
                    code: match error.kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            crate::awbc::schema::AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            crate::awbc::schema::AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    message: Some(error.message.clone()),
                    source_map: None,
                };
                fiber.mark_trapped(trap.clone());
                frame.pending_activation_host_call = None;
                return Err(ProductStepError::ActivationTrap(trap));
            }
        };
        let host = self
            .program
            .host_calls
            .get(pending.call.index())
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let signature = self
            .program
            .signatures
            .get(host.signature.index())
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let result_type = signature.result;
        let valid = match result_type {
            Some(result_type) => runtime_value_matches_type(&self.program, value, result_type, 0),
            None => value == &RuntimeValue::Unit,
        };
        if !valid
            || destination.is_some() && result_type.is_none()
            || !value.ownership().permits_copy()
        {
            let trap = crate::awbc::fiber::FiberTrap {
                code: crate::awbc::schema::AwbcTrapCode::HostAbiMismatch,
                message: Some("activation host-call result violates its AWBC signature".to_owned()),
                source_map: None,
            };
            fiber.mark_trapped(trap.clone());
            frame.pending_activation_host_call = None;
            return Err(ProductStepError::ActivationTrap(trap));
        }
        if let Some(destination) = destination {
            fiber
                .active_frame_mut()?
                .set_register(destination, value.clone())?;
        }
        fiber.resume_at(&self.program, resume)?;
        frame.pending_activation_host_call = None;
        Ok(ProductActivationProgress {
            progressed: true,
            presented: None,
            reducer: LineTaskActivation::default(),
            pure_stats: None,
            execution: None,
            host_calls: Vec::new(),
        })
    }

    fn activation_host_call_request(
        &mut self,
        call: crate::awbc::schema::AwbcHostCallId,
        args: &[RuntimeValue],
        existing: Option<super::PendingHostCall>,
    ) -> Result<(super::PendingHostCall, crate::step::RuntimeHostCallRequest), ProductStepError>
    {
        let public_id = self
            .program
            .host_calls
            .get(call.index())
            .and_then(|host| self.program.strings.get(host.public_id.index()))
            .cloned()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let pending = if let Some(pending) = existing {
            if pending.call != call {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            pending
        } else {
            let sequence = self.next_host_call_sequence;
            self.next_host_call_sequence = sequence.saturating_add(1);
            super::PendingHostCall {
                call,
                id: crate::step::RuntimeHostCallId(if sequence == 0 {
                    public_id.clone()
                } else {
                    format!("{public_id}.{sequence}")
                }),
            }
        };
        let host = self
            .program
            .host_calls
            .get(call.index())
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let signature = self
            .program
            .signatures
            .get(host.signature.index())
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        if args.len() != host.arguments.len()
            || args.iter().any(|value| !value.ownership().permits_copy())
        {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let result_type = match signature.result {
            Some(result) => self
                .program
                .runtime_types
                .get(result.index())
                .ok_or(LineRuntimeError::InvalidActivationOperation)?,
            None => self
                .program
                .runtime_types
                .iter()
                .find(|ty| matches!(ty.shape(), crate::awbc::schema::AwbcRuntimeTypeShape::Unit))
                .ok_or(LineRuntimeError::InvalidActivationOperation)?,
        };
        let mut positional = Vec::new();
        let mut named_args = Vec::new();
        for (descriptor, value) in host.arguments.iter().zip(args) {
            if descriptor.spread {
                let values = crate::value::runtime_value_into_sequence_values(value.clone())
                    .map_err(|_| LineRuntimeError::InvalidActivationOperation)?;
                positional.extend(values.into_iter().map(RuntimePayload::from));
            } else if let Some(name) = descriptor.name {
                let name = self
                    .program
                    .strings
                    .get(name.index())
                    .cloned()
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                named_args.push(crate::task::NamedHostArg {
                    name,
                    value: RuntimePayload::from(value.clone()),
                });
            } else {
                positional.push(RuntimePayload::from(value.clone()));
            }
        }
        Ok((
            pending.clone(),
            crate::step::RuntimeHostCallRequest {
                id: pending.id,
                public_id,
                capability: self
                    .program
                    .strings
                    .get(host.capability.index())
                    .cloned()
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?,
                operation: self
                    .program
                    .strings
                    .get(host.operation.index())
                    .cloned()
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?,
                contract: host.contract,
                args: positional,
                named_args,
                result: result_type.semantic_identity(),
                mode: match host.mode {
                    crate::awbc::schema::AwbcHostCallMode::Immediate => {
                        crate::step::RuntimeHostCallMode::Immediate
                    }
                    crate::awbc::schema::AwbcHostCallMode::Suspend => {
                        crate::step::RuntimeHostCallMode::Suspend
                    }
                },
                deterministic: host.deterministic,
            },
        ))
    }

    fn execute_product_line_operation(
        &self,
        activation: &DialogueActivationId,
        frame: &mut ActiveDialogue,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        destination: crate::awbc::schema::AwbcRegisterId,
        operation_id: crate::awbc::schema::AwbcLineOperationId,
        args: &[(crate::awbc::schema::AwbcRegisterId, RuntimeValue)],
    ) -> Result<Option<ProductPendingLineOperation>, ProductStepError> {
        let operation = self
            .program
            .line_operations
            .get(operation_id.index())
            .cloned()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let group = self
            .dialogue_group(frame.content)
            .ok_or(LineRuntimeError::MissingTaskGroup)?;
        if operation.group() != group {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let evidence = product_line_site_evidence(&self.program, group, &operation)?;
        let destination_owner =
            activation_register_owner(self.facade_fiber.execution, fiber, destination)?;
        match operation {
            AwbcLineOperation::AcquireActor {
                character, scope, ..
            } => {
                if !args.is_empty()
                    || evidence.site.kind != RuntimeHandleKind::StageActor
                    || evidence.site.character.as_ref() != Some(&character)
                    || evidence.site.scheduled_child.is_some()
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let mut ledger = line.ledger().clone();
                let value = RuntimeValue::Opaque(ledger.issue_exact(
                    activation,
                    evidence.runtime_site,
                    RuntimeHandleKind::StageActor,
                    &evidence.opaque_owner,
                    RuntimeHandleResource::StageActor(RuntimeStageActorLease::new(
                        character.clone(),
                    )),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let token = RuntimeLineHandleLedger::token_from_value(&value)?;
                let mut commands =
                    RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
                let command = commands.push_acquire_actor(token.clone(), character, scope)?;
                line.commit_ledger(ledger);
                line.record_commands(activation, commands)?;
                Ok(Some(ProductPendingLineOperation::AcquireActor {
                    cursor,
                    destination,
                    command,
                    value,
                    token,
                }))
            }
            AwbcLineOperation::Schedule {
                child, captures, ..
            } => {
                let Some(((delay_register, RuntimeValue::Duration(delay)), capture_args)) =
                    args.split_first()
                else {
                    return Err(LineRuntimeError::InvalidCueDelay.into());
                };
                let _ = delay_register;
                if capture_args.len() != captures.len()
                    || evidence.site.kind != RuntimeHandleKind::Cue
                    || evidence.site.character.is_some()
                    || evidence.site.scheduled_child != Some(child)
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let deadline = LogicalDuration::from_nanos(frame.elapsed_nanos)
                    .checked_add(*delay)
                    .ok_or(LineRuntimeError::CueDeadlineOverflow)?;
                let view = self
                    .line_task_view(frame.content)
                    .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                let local_child = view
                    .global_node_to_local(child)
                    .ok_or(LineRuntimeError::InvalidScheduledCaptureOwner)?;
                let node = self
                    .program
                    .line_task_nodes
                    .get(child.index())
                    .ok_or(LineRuntimeError::InvalidScheduledCaptureOwner)?;
                let AwbcLineTaskNode::Child { scope, join, .. } = node else {
                    return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into());
                };
                let local_scope = view
                    .global_node_to_local(*scope)
                    .ok_or(LineRuntimeError::InvalidScheduledCaptureOwner)?;
                let join = match join {
                    AwbcChildJoinPolicy::Join => ChildJoinPolicy::Join,
                    AwbcChildJoinPolicy::Detached => ChildJoinPolicy::Detached,
                };
                let mut ledger = line.ledger().clone();
                let mut captured_tokens = BTreeSet::new();
                let mut captured_registers = BTreeSet::new();
                let mut capture_transfers = Vec::new();
                let mut captured_values = Vec::with_capacity(captures.len());
                for (capture, (register, value)) in captures.iter().zip(capture_args) {
                    if !captured_registers.insert(*register)
                        || !runtime_value_matches_type(&self.program, value, capture.ty, 0)
                    {
                        return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into());
                    }
                    let expected = RuntimeHandleOwnerSlot::ActivationLocal(
                        activation_register_owner(self.facade_fiber.execution, fiber, *register)?,
                    );
                    for handle in unique_line_handles(value)? {
                        if !captured_tokens.insert(handle.token().clone()) {
                            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                        }
                        if join == ChildJoinPolicy::Detached {
                            return Err(LineRuntimeError::DetachedAffineCapture.into());
                        }
                        capture_transfers.push((handle.token().clone(), expected.clone()));
                    }
                    let moved = fiber.active_frame_mut()?.take_register(*register)?;
                    if &moved != value {
                        return Err(LineRuntimeError::InvalidScheduledCaptureGraph.into());
                    }
                    captured_values.push(RuntimeLocalBinding {
                        local: capture.local,
                        value: moved,
                    });
                }
                let value = RuntimeValue::Opaque(ledger.issue_exact(
                    activation,
                    evidence.runtime_site,
                    RuntimeHandleKind::Cue,
                    &evidence.opaque_owner,
                    RuntimeHandleResource::Cue(RuntimeCueLease::new(RuntimeCueOrigin::Scheduled {
                        child: local_child,
                        deadline,
                    })),
                    RuntimeHandleOwnerSlot::ActivationLocal(destination_owner),
                )?);
                let token = RuntimeLineHandleLedger::token_from_value(&value)?;
                let work = LineTaskWorkTag::scheduled(token.clone(), local_scope);
                for (captured, expected) in capture_transfers {
                    ledger.transfer(
                        &captured,
                        &expected,
                        RuntimeHandleOwnerSlot::ChildScope(work.clone()),
                    )?;
                }
                fiber.active_frame_mut()?.set_register(destination, value)?;
                fiber.commit_yielded_instruction(cursor)?;
                line.schedule(RuntimeScheduledLineTask::new(
                    token,
                    local_child,
                    work,
                    deadline,
                    captured_values.into_boxed_slice(),
                )?)?;
                line.commit_ledger(ledger);
                Ok(None)
            }
            AwbcLineOperation::ActorLook { character, .. } => {
                let [
                    (actor_register, RuntimeValue::Opaque(actor)),
                    (_, RuntimeValue::EntityRef(look)),
                    (_, RuntimeValue::Duration(crossfade)),
                ] = args
                else {
                    return Err(LineRuntimeError::InvalidCrossfade.into());
                };
                if evidence.site.kind != RuntimeHandleKind::Cue
                    || evidence.site.character.as_ref() != Some(&character)
                    || evidence.site.scheduled_child.is_some()
                {
                    return Err(LineRuntimeError::InvalidHandleSite.into());
                }
                let actor_lease = line.ledger().validate_value(
                    actor,
                    RuntimeHandleKind::StageActor,
                    activation,
                )?;
                let expected_actor_owner = RuntimeHandleOwnerSlot::ActivationLocal(
                    activation_register_owner(self.facade_fiber.execution, fiber, *actor_register)?,
                );
                if actor_lease.owner() != &expected_actor_owner {
                    return Err(LineRuntimeError::WrongOwner.into());
                }
                let RuntimeHandleResource::StageActor(actor_resource) = actor_lease.resource()
                else {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
                };
                if actor_resource.character() != &character {
                    return Err(LineRuntimeError::WrongActorCharacter.into());
                }
                let Some((look_character, look)) = look.character_look() else {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                };
                if look_character != &character {
                    return Err(LineRuntimeError::WrongLookOwner.into());
                }
                let actor_token = actor_lease.token().clone();
                let mut ledger = line.ledger().clone();
                let value = RuntimeValue::Opaque(ledger.issue_exact(
                    activation,
                    evidence.runtime_site,
                    RuntimeHandleKind::Cue,
                    &evidence.opaque_owner,
                    RuntimeHandleResource::Cue(RuntimeCueLease::new(RuntimeCueOrigin::StageLook)),
                    RuntimeHandleOwnerSlot::LineScope,
                )?);
                let token = RuntimeLineHandleLedger::token_from_value(&value)?;
                let mut commands =
                    RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
                let command = commands.push_set_character_look(
                    token.clone(),
                    actor_token,
                    character,
                    look.clone(),
                    *crossfade,
                )?;
                line.commit_ledger(ledger);
                line.record_commands(activation, commands)?;
                Ok(Some(ProductPendingLineOperation::ActorLook {
                    cursor,
                    destination,
                    command,
                    value,
                    token,
                }))
            }
            AwbcLineOperation::VoiceHandle { .. } => match frame.voice.clone() {
                RuntimeDialogueVoiceState::Ready(session)
                | RuntimeDialogueVoiceState::Completed(session) => {
                    let mut ledger = line.ledger().clone();
                    let ordinal = ledger.next_voice_lease_ordinal()?;
                    let value = RuntimeValue::Opaque(ledger.issue_exact(
                        activation,
                        evidence.runtime_site,
                        RuntimeHandleKind::Voice,
                        &evidence.opaque_owner,
                        RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                            session, ordinal, true,
                        )),
                        RuntimeHandleOwnerSlot::ActivationLocal(destination_owner),
                    )?);
                    fiber.active_frame_mut()?.set_register(destination, value)?;
                    fiber.commit_yielded_instruction(cursor)?;
                    line.commit_ledger(ledger);
                    Ok(None)
                }
                RuntimeDialogueVoiceState::Lazy(ticket) => {
                    let mut commands =
                        RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
                    let command = commands.push_start_voice(ticket)?;
                    line.record_commands(activation, commands)?;
                    Ok(Some(ProductPendingLineOperation::StartVoice {
                        cursor,
                        destination,
                        command,
                        site: operation_id_to_site(&self.program, operation_id)?,
                    }))
                }
                RuntimeDialogueVoiceState::Absent => {
                    Err(LineRuntimeError::MissingActiveVoice.into())
                }
                RuntimeDialogueVoiceState::Failed(failure) => {
                    Err(LineRuntimeError::VoiceStartRejected { failure }.into())
                }
            },
        }
    }

    fn register_line_root_defer(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
    ) -> Result<BTreeSet<crate::runtime_id::RuntimeLineHandleToken>, ProductStepError> {
        line.can_register_deferred()?;
        let target_id = self
            .program
            .defer_sites
            .get(site.index())
            .copied()
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        let target = self
            .program
            .functions
            .get(target_id.index())
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        let signature = self
            .program
            .signatures
            .get(target.signature.index())
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        if signature.params.len() != captures.len()
            || !signature.result.is_some_and(|result| {
                matches!(
                    self.program
                        .runtime_types
                        .get(result.index())
                        .map(crate::awbc::schema::AwbcRuntimeType::shape),
                    Some(crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                )
            })
        {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let frame = fiber.active_frame()?;
        let mut registers = frame.registers.clone();
        let mut capture_registers = BTreeSet::new();
        let mut captured_values = Vec::with_capacity(captures.len());
        let mut deferred_tokens = BTreeSet::new();
        for ((register, value), expected) in captures.iter().zip(&signature.params) {
            let first_register_use = capture_registers.insert(*register);
            if !first_register_use && !value.ownership().permits_copy()
                || !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value,
                    *expected,
                    0,
                )
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            if frame
                .registers
                .get(register.index())
                .and_then(Option::as_ref)
                != Some(value)
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            if first_register_use && !value.ownership().permits_copy() {
                let slot = registers
                    .get_mut(register.index())
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                slot.take()
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
            }
            for handle in unique_line_handles(value)? {
                if handle.token().activation() != activation
                    || !deferred_tokens.insert(handle.token().clone())
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
                let owner =
                    activation_register_owner(self.facade_fiber.execution, fiber, *register)?;
                let lease = line
                    .ledger()
                    .lease(handle.token())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if lease.owner() != &RuntimeHandleOwnerSlot::ActivationLocal(owner) {
                    return Err(LineRuntimeError::WrongOwner.into());
                }
            }
            captured_values.push(value.clone());
        }
        line.register_deferred(site, outcome, captured_values)?;
        fiber.active_frame_mut()?.registers = registers;
        fiber.commit_yielded_instruction(cursor)?;
        Ok(deferred_tokens)
    }

    fn register_scoped_defer(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        scope_id: crate::awbc::schema::AwbcScopeId,
        site: crate::runtime_id::RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
    ) -> Result<(), ProductStepError> {
        line.can_register_deferred()?;
        if fiber.cursor != cursor
            || fiber.active_frame()?.scopes.last().map(|scope| scope.id) != Some(scope_id)
        {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let target_id = self
            .program
            .defer_sites
            .get(site.index())
            .copied()
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        let target = self
            .program
            .functions
            .get(target_id.index())
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        let signature = self
            .program
            .signatures
            .get(target.signature.index())
            .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
        if target.kind != crate::awbc::schema::AwbcFunctionKind::Ordinary
            || signature.params.len() != captures.len()
            || !signature.result.is_some_and(|result| {
                matches!(
                    self.program
                        .runtime_types
                        .get(result.index())
                        .map(crate::awbc::schema::AwbcRuntimeType::shape),
                    Some(crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                )
            })
        {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }

        let frame = fiber.active_frame()?;
        let mut registers = frame.registers.clone();
        let mut seen_registers = BTreeSet::new();
        let capture_registers = captures
            .iter()
            .map(|(register, _)| *register)
            .collect::<Vec<_>>();
        let mut capture_values = Vec::with_capacity(captures.len());
        let mut capture_tokens = BTreeSet::new();
        for ((register, value), expected) in captures.iter().zip(&signature.params) {
            let first_register_use = seen_registers.insert(*register);
            if (!first_register_use && !value.ownership().permits_copy())
                || frame
                    .registers
                    .get(register.index())
                    .and_then(Option::as_ref)
                    != Some(value)
                || !runtime_value_matches_type(&self.program, value, *expected, 0)
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let handles = unique_line_handles(value)?;
            if !value.ownership().permits_copy() && handles.is_empty() {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            if first_register_use && !value.ownership().permits_copy() {
                registers
                    .get_mut(register.index())
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?
                    .take()
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
            }
            for handle in handles {
                if handle.token().activation() != activation
                    || !capture_tokens.insert(handle.token().clone())
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
                let owner =
                    activation_register_owner(self.facade_fiber.execution, fiber, *register)?;
                let lease = line
                    .ledger()
                    .lease(handle.token())
                    .ok_or(LineRuntimeError::UnknownHandle)?;
                if lease.owner() != &RuntimeHandleOwnerSlot::ActivationLocal(owner) {
                    return Err(LineRuntimeError::WrongOwner.into());
                }
            }
            capture_values.push(value.clone());
        }
        let packet = line.allocate_deferred_registration(site, outcome, capture_values)?;
        let (id, site, outcome, captures) = packet.into_parts();
        let deferred = crate::awbc::fiber::FiberDeferredRegistration {
            id,
            site,
            outcome,
            capture_registers,
            captures,
        };
        let frame = fiber.active_frame_mut()?;
        let scope = frame
            .scopes
            .last_mut()
            .filter(|scope| scope.id == scope_id)
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        if scope.defer_exit.is_some() || scope.defer_inflight.is_some() {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        scope.defers.push(deferred);
        frame.registers = registers;
        fiber.commit_yielded_instruction(cursor)?;
        Ok(())
    }

    fn prepare_scoped_defer_unwind(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        scope_id: crate::awbc::schema::AwbcScopeId,
        requested_exit: crate::line_task::ScopeExit,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) -> Result<BTreeSet<crate::runtime_id::RuntimeLineHandleToken>, ProductStepError> {
        if fiber.cursor != cursor {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let frame = fiber.active_frame_mut()?;
        let scope = frame
            .scopes
            .last_mut()
            .filter(|scope| scope.id == scope_id)
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        if scope.defer_inflight.is_some()
            || scope.defers.is_empty()
            || !scope.defer_releasing.is_empty()
        {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let exit = *scope.defer_exit.get_or_insert(requested_exit);
        let deferred = scope
            .defers
            .pop()
            .expect("nonempty lexical defer stack was checked");
        let registration = crate::line_task::RuntimeLineDeferredRegistration::new(
            deferred.id,
            deferred.site,
            deferred.outcome,
            deferred.captures,
        );
        let (id, site, _outcome, captures) = registration.clone().into_parts();
        let mut handled_tokens = BTreeSet::new();
        for capture in &captures {
            for handle in unique_line_handles(capture)? {
                if !handled_tokens.insert(handle.token().clone()) {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
        match line.prepare_scoped_deferred(activation, registration, exit)? {
            crate::line_task::RuntimeDeferUnwindStep::Skipped(skipped) => {
                if skipped != id {
                    return Err(LineRuntimeError::InvalidDeferredTransition.into());
                }
                if !handled_tokens.is_empty() {
                    fiber
                        .active_frame_mut()?
                        .scopes
                        .last_mut()
                        .filter(|scope| scope.id == scope_id)
                        .ok_or(LineRuntimeError::InvalidActivationOperation)?
                        .defer_releasing
                        .push(crate::awbc::fiber::FiberDeferredRelease {
                            registration: id,
                            site,
                            tokens: handled_tokens.iter().cloned().collect(),
                        });
                }
            }
            crate::line_task::RuntimeDeferUnwindStep::Run(registration) => {
                if registration.id() != id || registration.site() != site {
                    return Err(LineRuntimeError::InvalidDeferredTransition.into());
                }
                let function = self
                    .program
                    .defer_sites
                    .get(site.index())
                    .copied()
                    .ok_or(LineRuntimeError::UnknownDeferredSite { site })?;
                let (frame_instance, scope) = {
                    let frame = fiber.active_frame()?;
                    (frame.instance, scope_id)
                };
                batch.spawn(
                    self,
                    super::ProductChildFiberOwner::ScopedDeferred {
                        content: self
                            .dialogues
                            .active_frame()
                            .map(|frame| frame.content)
                            .ok_or(LineRuntimeError::ActivationFrameReleased)?,
                        activation: activation.clone(),
                        frame: frame_instance,
                        scope,
                        registration: id,
                        site,
                    },
                    function,
                    captures,
                )?;
                let scope = fiber
                    .active_frame_mut()?
                    .scopes
                    .last_mut()
                    .filter(|scope| scope.id == scope_id)
                    .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                scope.defer_inflight = Some(crate::awbc::fiber::FiberDeferredInFlight {
                    registration: id,
                    site,
                });
            }
        }
        Ok(handled_tokens)
    }

    pub(super) fn resume_pending_line_operation(
        &self,
        transaction: &mut ProductDialogueTransaction,
        outcomes: &[RuntimeLineHostOutcome],
    ) -> Result<bool, ProductStepError> {
        let mut candidate = transaction.clone();
        let result = self.resume_pending_line_operation_candidate(&mut candidate, outcomes);
        match &result {
            Ok(_)
            | Err(ProductStepError::Line(
                LineRuntimeError::StageCommandRejected { .. }
                | LineRuntimeError::VoiceStartRejected { .. },
            )) => *transaction = candidate,
            Err(_) => {}
        }
        result
    }

    fn resume_pending_line_operation_candidate(
        &self,
        transaction: &mut ProductDialogueTransaction,
        outcomes: &[RuntimeLineHostOutcome],
    ) -> Result<bool, ProductStepError> {
        let activation = transaction.activation().clone();
        let (frame, line) = transaction.parts_mut();
        let (fiber, pending) = match &mut frame.phase {
            ProductDialoguePhase::Activating { fiber, pending }
            | ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::Activation { fiber, pending },
                ..
            }) => (fiber, pending),
            ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::LineTask { .. },
                ..
            }) => {
                if outcomes.is_empty() {
                    return Ok(false);
                }
                return Err(LineRuntimeError::StaleCommandOutcome.into());
            }
        };
        let Some(operation) = pending.clone() else {
            if outcomes.is_empty() {
                return Ok(false);
            }
            return Err(LineRuntimeError::UnknownCommandOutcome.into());
        };
        if outcomes.is_empty() {
            return Ok(false);
        }
        let pending_command = match &operation {
            ProductPendingLineOperation::AcquireActor { command, .. }
            | ProductPendingLineOperation::ActorLook { command, .. }
            | ProductPendingLineOperation::StartVoice { command, .. } => command,
        };
        let mut pending_outcome = None;
        for outcome in outcomes {
            if outcome.command() == pending_command {
                if pending_outcome.replace(outcome).is_some() {
                    return Err(LineRuntimeError::DuplicateCommandOutcome.into());
                }
            } else if let Some(error) = line.accept_runtime_outcome(outcome)? {
                return Err(error.into());
            }
        }
        let Some(outcome) = pending_outcome else {
            return Ok(false);
        };
        match operation {
            ProductPendingLineOperation::AcquireActor {
                cursor,
                destination,
                command,
                value,
                token,
            } => {
                require_pending_command(&activation, line, &command, outcome)?;
                match outcome {
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Acquired {
                        actor,
                        ..
                    }) if actor == &token => {
                        line.consume_issued_command(&command)?;
                        let mut ledger = line.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Allocating,
                            RuntimeHandleLeaseState::Active,
                        )?;
                        let owner = activation_register_owner(
                            self.facade_fiber.execution,
                            fiber,
                            destination,
                        )?;
                        ledger.transfer(
                            &token,
                            &RuntimeHandleOwnerSlot::LineScope,
                            RuntimeHandleOwnerSlot::ActivationLocal(owner),
                        )?;
                        fiber.active_frame_mut()?.set_register(destination, value)?;
                        fiber.commit_yielded_instruction(cursor)?;
                        line.commit_ledger(ledger);
                    }
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                        code,
                        ..
                    }) => {
                        line.consume_issued_command(&command)?;
                        let mut ledger = line.ledger().clone();
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
                        line.commit_ledger(ledger);
                        *pending = None;
                        return Err(LineRuntimeError::StageCommandRejected { code: *code }.into());
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                }
            }
            ProductPendingLineOperation::ActorLook {
                cursor,
                destination,
                command,
                value,
                token,
            } => {
                require_pending_command(&activation, line, &command, outcome)?;
                match outcome {
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Accepted {
                        cue,
                        ..
                    }) if cue == &token => {
                        let mut ledger = line.ledger().clone();
                        ledger.set_state(
                            &token,
                            RuntimeHandleLeaseState::Pending,
                            RuntimeHandleLeaseState::Running,
                        )?;
                        let owner = activation_register_owner(
                            self.facade_fiber.execution,
                            fiber,
                            destination,
                        )?;
                        ledger.transfer(
                            &token,
                            &RuntimeHandleOwnerSlot::LineScope,
                            RuntimeHandleOwnerSlot::ActivationLocal(owner),
                        )?;
                        fiber.active_frame_mut()?.set_register(destination, value)?;
                        fiber.commit_yielded_instruction(cursor)?;
                        line.commit_ledger(ledger);
                    }
                    RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                        code,
                        ..
                    }) => {
                        line.consume_issued_command(&command)?;
                        let mut ledger = line.ledger().clone();
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
                        line.commit_ledger(ledger);
                        *pending = None;
                        return Err(LineRuntimeError::StageCommandRejected { code: *code }.into());
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                }
            }
            ProductPendingLineOperation::StartVoice {
                cursor,
                destination,
                command,
                site,
            } => {
                require_pending_command(&activation, line, &command, outcome)?;
                line.consume_issued_command(&command)?;
                let session = match outcome {
                    RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Started {
                        session,
                        ..
                    }) => session.clone(),
                    RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Rejected {
                        failure,
                        ..
                    }) => {
                        frame.voice = RuntimeDialogueVoiceState::Failed(failure.clone());
                        *pending = None;
                        return Err(LineRuntimeError::VoiceStartRejected {
                            failure: failure.clone(),
                        }
                        .into());
                    }
                    _ => return Err(LineRuntimeError::StageOutcomeMismatch.into()),
                };
                frame.voice = RuntimeDialogueVoiceState::Ready(session.clone());
                let group = self
                    .dialogue_group(frame.content)
                    .ok_or(LineRuntimeError::MissingTaskGroup)?;
                let operation = AwbcLineOperation::VoiceHandle {
                    group,
                    site,
                    result_type: self
                        .program
                        .line_task_groups
                        .get(group.index())
                        .and_then(|group| group.handle_sites.get(site.index()))
                        .map(|site| site.result_type)
                        .ok_or(LineRuntimeError::InvalidHandleSite)?,
                };
                let evidence = product_line_site_evidence(&self.program, group, &operation)?;
                let mut ledger = line.ledger().clone();
                let ordinal = ledger.next_voice_lease_ordinal()?;
                let value = RuntimeValue::Opaque(ledger.issue_exact(
                    &activation,
                    evidence.runtime_site,
                    RuntimeHandleKind::Voice,
                    &evidence.opaque_owner,
                    RuntimeHandleResource::Voice(RuntimeVoiceLease::new(session, ordinal, true)),
                    RuntimeHandleOwnerSlot::ActivationLocal(activation_register_owner(
                        self.facade_fiber.execution,
                        fiber,
                        destination,
                    )?),
                )?);
                fiber.active_frame_mut()?.set_register(destination, value)?;
                fiber.commit_yielded_instruction(cursor)?;
                line.commit_ledger(ledger);
            }
        }
        *pending = None;
        Ok(true)
    }

    fn commit_product_dialogue_result(
        &self,
        _activation: &DialogueActivationId,
        frame: &mut ActiveDialogue,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        source_register: crate::awbc::schema::AwbcRegisterId,
        source: RuntimeValue,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let group_id = self
            .dialogue_group(frame.content)
            .ok_or(LineRuntimeError::MissingTaskGroup)?;
        let group = self
            .program
            .line_task_groups
            .get(group_id.index())
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        if group.result_type != frame.result.ty
            || !runtime_value_matches_type(&self.program, &source, group.result_type, 0)
        {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let expected = RuntimeHandleOwnerSlot::ActivationLocal(activation_register_owner(
            self.facade_fiber.execution,
            fiber,
            source_register,
        )?);
        let mut ledger = line.ledger().clone();
        for handle in unique_line_handles(&source)? {
            let lease = ledger
                .lease(handle.token())
                .ok_or(LineRuntimeError::UnknownHandle)?;
            if lease.owner() != &expected || lease.resource().kind() != handle.kind() {
                return Err(LineRuntimeError::WrongOwner.into());
            }
            ledger.transfer(
                handle.token(),
                &expected,
                RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone()),
            )?;
        }
        let moved = fiber.active_frame_mut()?.take_register(source_register)?;
        if moved != source {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        fiber.commit_yielded_instruction(cursor)?;
        line.commit_ledger(ledger);
        line.commit_result(group.result_type, source)?;
        frame.phase = ProductDialoguePhase::Activating {
            fiber: fiber.clone(),
            pending: None,
        };
        Ok(ProductActivationProgress {
            progressed: true,
            presented: None,
            reducer: LineTaskActivation::default(),
            pure_stats: None,
            execution: None,
            host_calls: Vec::new(),
        })
    }

    fn resume_activation_out(
        &self,
        transaction: &mut ProductDialogueTransaction,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
        let (frame, line) = transaction.parts_mut();
        let before = match &frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => fiber.clone(),
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        };
        if fiber_has_scoped_defer_inflight(&before) {
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
            });
        }
        if let Some(scope) = before.active_frame()?.scopes.last() {
            let scope_id = scope.id;
            if !scope.defers.is_empty() {
                let mut candidate = before.clone();
                let mut batch = empty_activation_batch(self);
                let cursor = candidate.cursor;
                let handled = self.prepare_scoped_defer_unwind(
                    &activation,
                    line,
                    &mut candidate,
                    cursor,
                    scope_id,
                    crate::line_task::ScopeExit::Completed,
                    &mut batch,
                )?;
                self.reconcile_activation_fiber_ownership(
                    &activation,
                    line,
                    &before,
                    &candidate,
                    None,
                    &BTreeSet::new(),
                    &handled,
                )?;
                frame.phase = ProductDialoguePhase::Activating {
                    fiber: candidate,
                    pending: None,
                };
                return Ok(ProductActivationProgress {
                    progressed: true,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: Some(self.compact_pure_stats),
                    execution: Some(batch),
                    host_calls: Vec::new(),
                });
            }
            if !scope.defer_releasing.is_empty() {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }

            let mut candidate = before.clone();
            let (cleanups, failure) =
                pop_activation_scope(&self.program, &mut candidate, scope_id)?;
            let mut batch = empty_activation_batch(self);
            for cleanup in cleanups.into_iter().rev() {
                batch.observations.push(VmObservation::Effect {
                    effect: cleanup.effect,
                    args: cleanup.args,
                });
            }
            self.reconcile_activation_fiber_ownership(
                &activation,
                line,
                &before,
                &candidate,
                Some(crate::effect::RuntimeDropPolicy::Default),
                &BTreeSet::new(),
                &BTreeSet::new(),
            )?;
            frame.phase = ProductDialoguePhase::Activating {
                fiber: candidate,
                pending: None,
            };
            if let Some(trap) = failure {
                return Err(ProductStepError::ActivationTrap(trap));
            }
            return Ok(ProductActivationProgress {
                progressed: true,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: Some(self.compact_pure_stats),
                execution: (!batch.observations.is_empty()).then_some(batch),
                host_calls: Vec::new(),
            });
        }

        if !before.active_frame()?.root_defers.is_empty() {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        self.start_product_dialogue_after_activation(&activation, frame, line)
    }

    pub(super) fn unwind_failed_activation_scope(
        &self,
        transaction: &mut ProductDialogueTransaction,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) -> Result<(bool, Option<crate::awbc::fiber::FiberTrap>), ProductStepError> {
        let activation = transaction.activation().clone();
        let (frame, line) = transaction.parts_mut();
        let before = match &frame.phase {
            ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::Activation { fiber, .. },
                ..
            }) => fiber.clone(),
            ProductDialoguePhase::Activating { .. }
            | ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::LineTask { .. },
                ..
            }) => return Ok((false, None)),
        };
        let Some(scope) = before.active_frame()?.scopes.last() else {
            if !before.active_frame()?.root_defers.is_empty() {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }
            return Ok((false, None));
        };
        let scope_id = scope.id;
        if scope.defer_inflight.is_some() {
            return Ok((true, None));
        }
        if !scope.defers.is_empty() {
            let exit = scope
                .defer_exit
                .unwrap_or(crate::line_task::ScopeExit::Failed);
            let mut candidate = before.clone();
            let cursor = candidate.cursor;
            let handled = self.prepare_scoped_defer_unwind(
                &activation,
                line,
                &mut candidate,
                cursor,
                scope_id,
                exit,
                batch,
            )?;
            self.reconcile_activation_fiber_ownership(
                &activation,
                line,
                &before,
                &candidate,
                None,
                &BTreeSet::new(),
                &handled,
            )?;
            let ProductDialoguePhase::Closing(closing) = &mut frame.phase else {
                unreachable!("activation closing phase was checked above")
            };
            let ProductDialogueClosingState::Activation { fiber, .. } = &mut closing.state else {
                unreachable!("activation closing state was checked above")
            };
            *fiber = candidate;
            return Ok((true, None));
        }
        if !scope.defer_releasing.is_empty() {
            if line.has_pending_commands() {
                return Ok((true, None));
            }
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let mut candidate = before.clone();
        let (cleanups, failure) = pop_activation_scope(&self.program, &mut candidate, scope_id)?;
        for cleanup in cleanups.into_iter().rev() {
            batch.observations.push(VmObservation::Effect {
                effect: cleanup.effect,
                args: cleanup.args,
            });
        }
        self.reconcile_activation_fiber_ownership(
            &activation,
            line,
            &before,
            &candidate,
            Some(crate::effect::RuntimeDropPolicy::Default),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )?;
        let ProductDialoguePhase::Closing(closing) = &mut frame.phase else {
            unreachable!("activation closing phase was checked above")
        };
        let ProductDialogueClosingState::Activation { fiber, .. } = &mut closing.state else {
            unreachable!("activation closing state was checked above")
        };
        *fiber = candidate;
        Ok((true, failure))
    }

    fn start_product_dialogue_after_activation(
        &self,
        activation: &DialogueActivationId,
        frame: &mut ActiveDialogue,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let group_id = self
            .dialogue_group(frame.content)
            .ok_or(LineRuntimeError::MissingTaskGroup)?;
        let group = self
            .program
            .line_task_groups
            .get(group_id.index())
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let (result_type, result_value) = match line.result() {
            RuntimeDialogueResultState::Committed { ty, value } => (*ty, value),
            _ => return Err(LineRuntimeError::ResultNotCommitted.into()),
        };
        if result_type != group.result_type
            || result_type != frame.result.ty
            || !runtime_value_matches_type(&self.program, result_value, result_type, 0)
        {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let view = AwbcLineTaskPlanView::new(&self.program, group)
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let mut line_task = LineTaskLiveState::new(&view, activation.clone());
        let elapsed = LogicalDuration::from_nanos(frame.elapsed_nanos);
        for token in line.arm_due_schedules(elapsed)? {
            line_task.mark_scheduled_ready(token)?;
        }
        let reducer = progress_live_line_task_group(
            &view,
            elapsed,
            LineTaskReadyEvents::new(&BTreeSet::new()),
            &mut line_task,
        )?;
        frame.phase = ProductDialoguePhase::Reducing { line_task };
        Ok(ProductActivationProgress {
            progressed: true,
            presented: Some(build_dialogue_line_event(
                &self.program,
                activation.clone(),
                &frame.line,
                frame.content,
                &frame.target,
                &frame.values,
            )?),
            reducer,
            pure_stats: None,
            execution: None,
            host_calls: Vec::new(),
        })
    }

    fn reconcile_activation_fiber_ownership(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        before: &FiberState,
        after: &FiberState,
        drop_policy: Option<crate::effect::RuntimeDropPolicy>,
        deferred_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        scoped_reconciled_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    ) -> Result<(), ProductStepError> {
        let before =
            activation_fiber_handle_owners(self.facade_fiber.execution, activation, before)?;
        let after = activation_fiber_handle_owners(self.facade_fiber.execution, activation, after)?;
        let mut ledger = line.ledger().clone();
        let mut commands = RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
        let mut emitted_command = false;
        let tokens = before
            .keys()
            .chain(after.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        for token in tokens {
            match (before.get(&token), after.get(&token)) {
                (Some(source), Some(destination)) if source != destination => {
                    ledger.transfer(&token, source, destination.clone())?;
                }
                (Some(source), None) => {
                    if scoped_reconciled_tokens.contains(&token) {
                        continue;
                    }
                    if deferred_tokens.contains(&token) {
                        ledger.transfer(&token, source, RuntimeHandleOwnerSlot::LineScope)?;
                        continue;
                    }
                    let policy = drop_policy.ok_or(LineRuntimeError::UnjournaledHandleDrop)?;
                    let before_sequence = commands.next_sequence();
                    ledger.drop_owned_with_policy(&token, source, policy, &mut commands)?;
                    emitted_command |= commands.next_sequence() != before_sequence;
                }
                (None, Some(_)) => return Err(LineRuntimeError::UnknownHandle.into()),
                (Some(_), Some(_)) | (None, None) => {}
            }
        }
        line.commit_ledger(ledger);
        if emitted_command {
            line.record_commands(activation, commands)?;
        }
        Ok(())
    }
}

impl super::ProductLineTaskExecutionBatch {
    fn spawn(
        &mut self,
        executor: &super::AwbcProductStepExecutor,
        owner: super::ProductChildFiberOwner,
        function: crate::awbc::schema::AwbcFunctionId,
        args: Vec<RuntimeValue>,
    ) -> Result<(), ProductStepError> {
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProductStepError::ChildGenerationOverflow)?;
        let mut next_fiber_instance = self.next_fiber_instance;
        let fiber_instance = next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
            .map(crate::runtime_id::RuntimeFiberInstanceId::from_allocated)?;
        let mut child = FiberState::for_function_with_instance(
            &executor.program,
            executor.fiber.entry,
            function,
            fiber_instance,
            self.next_generation,
            executor.fiber.budget.quantum.max(1),
        )
        .map_err(|error| ProductStepError::Internal(error.to_string()))?;
        child
            .bind_function_argument_values_owned(&executor.program, args)
            .map_err(|error| ProductStepError::Type(error.to_string()))?;
        self.next_generation = next_generation;
        self.next_fiber_instance = next_fiber_instance;
        self.child_fibers.push_back(super::ProductChildFiber {
            owner,
            fiber: child,
            pending_host_call: None,
        });
        Ok(())
    }

    fn cancel_line_task_children(
        &mut self,
        executor: &super::AwbcProductStepExecutor,
        content: crate::awbc::schema::AwbcContentUnitId,
        expected_tag: &LineTaskWorkTag,
    ) -> Result<Vec<(LineTaskWorkTag, bool, bool)>, ProductStepError> {
        let mut completions = Vec::new();
        let mut index = 0;
        while index < self.child_fibers.len() {
            let matches = matches!(
                &self.child_fibers[index].owner,
                super::ProductChildFiberOwner::LineTask { content: owner_content, tag, .. }
                    if owner_content == &content
                        && tag == expected_tag
            );
            if !matches {
                index += 1;
                continue;
            }
            let owner = self.child_fibers[index].owner.clone();
            let super::ProductChildFiberOwner::LineTask { tag, policy, .. } = owner else {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            };
            match policy.cancel {
                ChildCancelPolicy::CancelAndJoin => {
                    let mut child = self
                        .child_fibers
                        .remove(index)
                        .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                    self.observations
                        .extend(crate::awbc::vm::cancel_fiber(&mut child.fiber).observations);
                    completions.push((tag, false, true));
                }
                ChildCancelPolicy::Finish => {
                    let super::ProductChildFiberOwner::LineTask { phase, .. } =
                        &mut self.child_fibers[index].owner
                    else {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    };
                    *phase = super::ProductLineTaskFiberPhase::Closing;
                    index += 1;
                }
                ChildCancelPolicy::Detach => {
                    if !product_fiber_handle_owners(
                        executor.facade_fiber.execution,
                        &self.child_fibers[index].fiber,
                    )?
                    .is_empty()
                    {
                        return Err(LineRuntimeError::DetachedAffineCapture.into());
                    }
                    self.child_fibers[index].owner = super::ProductChildFiberOwner::Independent;
                    index += 1;
                }
            }
        }
        Ok(completions)
    }
}

fn product_line_site_evidence(
    program: &crate::awbc::schema::AwbcProgram,
    group_id: AwbcLineTaskGroupId,
    operation: &AwbcLineOperation,
) -> Result<ProductLineSiteEvidence, ProductStepError> {
    let group = program
        .line_task_groups
        .get(group_id.index())
        .ok_or(LineRuntimeError::UnknownTaskGroup)?;
    let site_id = operation.site();
    let site = group
        .handle_sites
        .get(site_id.index())
        .cloned()
        .ok_or(LineRuntimeError::InvalidHandleSite)?;
    if site.result_type != operation.result_type() {
        return Err(LineRuntimeError::InvalidHandleSite.into());
    }
    let ty = program
        .runtime_types
        .get(site.result_type.index())
        .ok_or(LineRuntimeError::WrongOpaqueProducer)?;
    let AwbcRuntimeTypeShape::Opaque {
        producer,
        admission,
        value_class,
        persistence,
        ..
    } = ty.shape()
    else {
        return Err(LineRuntimeError::WrongOpaqueProducer.into());
    };
    if *admission != RuntimeOpaqueTypeAdmission::ExactIdentity {
        return Err(LineRuntimeError::WrongOpaqueProducer.into());
    }
    let producer = program
        .strings
        .get(producer.index())
        .ok_or(LineRuntimeError::WrongOpaqueProducer)?;
    let producer = RuntimeOpaqueTypeProducerId::try_new(producer.clone())
        .map_err(|_| LineRuntimeError::WrongOpaqueProducer)?;
    Ok(ProductLineSiteEvidence {
        runtime_site: RuntimeLineHandleSiteId::from_zero_based(site_id.0),
        site,
        opaque_owner: RuntimeOpaqueTypeOwner::with_admission(
            producer,
            ty.semantic_identity(),
            *admission,
            *value_class,
            *persistence,
        ),
    })
}

fn operation_id_to_site(
    program: &crate::awbc::schema::AwbcProgram,
    operation: crate::awbc::schema::AwbcLineOperationId,
) -> Result<crate::awbc::schema::AwbcLineHandleSiteId, ProductStepError> {
    program
        .line_operations
        .get(operation.index())
        .map(AwbcLineOperation::site)
        .ok_or_else(|| LineRuntimeError::InvalidActivationOperation.into())
}

fn activation_register_owner(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
    register: crate::awbc::schema::AwbcRegisterId,
) -> Result<RuntimeOwnedSlotId, ProductStepError> {
    let frame = fiber
        .active_frame()
        .map_err(|error| ProductStepError::Internal(error.to_string()))?;
    Ok(RuntimeOwnedSlotId::AwbcRegister {
        execution,
        fiber: fiber.instance,
        frame: frame.instance,
        register,
    })
}

fn fiber_has_scoped_defer_inflight(fiber: &FiberState) -> bool {
    fiber
        .frames
        .iter()
        .flat_map(|frame| &frame.scopes)
        .any(|scope| scope.defer_inflight.is_some())
}

pub(super) fn settle_scoped_defer_releases(transaction: &mut ProductDialogueTransaction) {
    let lease_states = transaction
        .line()
        .ledger()
        .leases()
        .iter()
        .map(|(token, lease)| (token.clone(), lease.state()))
        .collect::<BTreeMap<_, _>>();
    let fiber = match &mut transaction.frame_mut().phase {
        ProductDialoguePhase::Activating { fiber, .. }
        | ProductDialoguePhase::Closing(ProductDialogueClosing {
            state: ProductDialogueClosingState::Activation { fiber, .. },
            ..
        }) => fiber,
        ProductDialoguePhase::Reducing { .. }
        | ProductDialoguePhase::Publishing { .. }
        | ProductDialoguePhase::Closing(ProductDialogueClosing {
            state: ProductDialogueClosingState::LineTask { .. },
            ..
        }) => return,
    };
    for scope in fiber.frames.iter_mut().flat_map(|frame| &mut frame.scopes) {
        scope.defer_releasing.retain(|release| {
            release.tokens.iter().any(|token| {
                lease_states.get(token).is_some_and(|state| {
                    !matches!(
                        state,
                        RuntimeHandleLeaseState::Released
                            | RuntimeHandleLeaseState::Cancelled
                            | RuntimeHandleLeaseState::Failed
                            | RuntimeHandleLeaseState::Completed
                    )
                })
            })
        });
    }
}

fn empty_activation_batch(
    executor: &super::AwbcProductStepExecutor,
) -> super::ProductLineTaskExecutionBatch {
    super::ProductLineTaskExecutionBatch {
        child_fibers: executor.child_fibers.clone(),
        dialogue_effect_callback_activations: executor.dialogue_effect_callback_activations.clone(),
        next_generation: executor.next_generation,
        next_fiber_instance: executor.next_fiber_instance,
        observations: Vec::new(),
        pure_stats: None,
    }
}

fn pop_activation_scope(
    program: &crate::awbc::schema::AwbcProgram,
    fiber: &mut FiberState,
    scope_id: crate::awbc::schema::AwbcScopeId,
) -> Result<
    (
        Vec<crate::awbc::fiber::FiberScopeCleanup>,
        Option<crate::awbc::fiber::FiberTrap>,
    ),
    ProductStepError,
> {
    let frame = fiber.active_frame_mut()?;
    let Some(scope) = frame.scopes.last() else {
        return Err(LineRuntimeError::InvalidActivationOperation.into());
    };
    if scope.id != scope_id
        || !scope.defers.is_empty()
        || !scope.defer_releasing.is_empty()
        || scope.defer_inflight.is_some()
    {
        return Err(LineRuntimeError::InvalidDeferredTransition.into());
    }
    let scope = frame
        .scopes
        .pop()
        .expect("active lexical scope was checked above");
    let layout = program
        .frame_layouts
        .get(frame.layout.index())
        .ok_or(LineRuntimeError::InvalidActivationOperation)?;
    let active_scope_depth = u32::try_from(frame.scopes.len())
        .map_err(|_| LineRuntimeError::InvalidActivationOperation)?;
    for (register, slot) in frame.registers.iter_mut().zip(&layout.slots) {
        if slot.scope_depth > active_scope_depth
            && !matches!(
                slot.role,
                crate::awbc::schema::AwbcFrameSlotRole::Parameter
                    | crate::awbc::schema::AwbcFrameSlotRole::RuntimeState
            )
        {
            *register = None;
        }
    }
    Ok((scope.cleanups, scope.defer_failure))
}

fn activation_fiber_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    activation: &DialogueActivationId,
    fiber: &FiberState,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>, ProductStepError> {
    let mut owners = BTreeMap::new();
    for frame in &fiber.frames {
        for (index, value) in frame.registers.iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let register = u32::try_from(index)
                .map(crate::awbc::schema::AwbcRegisterId)
                .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
            let owner = RuntimeOwnedSlotId::AwbcRegister {
                execution,
                fiber: fiber.instance,
                frame: frame.instance,
                register,
            };
            for handle in unique_line_handles(value)? {
                if handle.token().activation() != activation {
                    return Err(LineRuntimeError::WrongActivation.into());
                }
                if owners
                    .insert(
                        handle.token().clone(),
                        RuntimeHandleOwnerSlot::ActivationLocal(owner),
                    )
                    .is_some()
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
        for scope in &frame.scopes {
            for deferred in &scope.defers {
                for capture in &deferred.captures {
                    for handle in unique_line_handles(capture)? {
                        if handle.token().activation() != activation
                            || owners
                                .insert(
                                    handle.token().clone(),
                                    RuntimeHandleOwnerSlot::ScopedDefer(deferred.id),
                                )
                                .is_some()
                        {
                            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                        }
                    }
                }
            }
        }
    }
    Ok(owners)
}

fn parent_fiber_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    activation: &DialogueActivationId,
    fiber: &FiberState,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>, ProductStepError> {
    let mut owners = BTreeMap::new();
    for frame in &fiber.frames {
        for (index, value) in frame.registers.iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let register = u32::try_from(index)
                .map(crate::awbc::schema::AwbcRegisterId)
                .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
            let owner = RuntimeOwnedSlotId::AwbcRegister {
                execution,
                fiber: fiber.instance,
                frame: frame.instance,
                register,
            };
            for handle in unique_line_handles(value)? {
                if handle.token().activation() != activation {
                    continue;
                }
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
    }
    Ok(owners)
}

pub(super) fn product_fiber_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>, ProductStepError> {
    let mut owners = BTreeMap::new();
    for frame in &fiber.frames {
        for (index, value) in frame.registers.iter().enumerate() {
            let Some(value) = value else {
                continue;
            };
            let register = u32::try_from(index)
                .map(crate::awbc::schema::AwbcRegisterId)
                .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
            let owner = RuntimeOwnedSlotId::AwbcRegister {
                execution,
                fiber: fiber.instance,
                frame: frame.instance,
                register,
            };
            for handle in unique_line_handles(value)? {
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
    }
    Ok(owners)
}

pub(super) fn unique_line_handles(
    value: &RuntimeValue,
) -> Result<Vec<crate::value::ownership::RuntimeAffineLineHandle>, ProductStepError> {
    let handles = value
        .affine_line_handles()
        .map_err(|_| LineRuntimeError::InvalidHandlePayload)?;
    let mut unique = BTreeSet::new();
    for handle in &handles {
        if !unique.insert(handle.token().clone()) {
            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
        }
    }
    Ok(handles)
}

fn require_pending_command(
    activation: &DialogueActivationId,
    line: &RuntimeDialogueActivationState<AwbcTypeId>,
    command: &crate::presentation::RuntimeLineCommandId,
    outcome: &RuntimeLineHostOutcome,
) -> Result<(), ProductStepError> {
    if command.activation() != activation || outcome.command() != command {
        return Err(LineRuntimeError::StaleCommandOutcome.into());
    }
    if line.issued_command(command).is_none() {
        return Err(if line.is_resolved(command) {
            LineRuntimeError::DuplicateCommandOutcome
        } else {
            LineRuntimeError::UnknownCommandOutcome
        }
        .into());
    }
    Ok(())
}

fn build_dialogue_line_event(
    program: &crate::awbc::schema::AwbcProgram,
    activation: DialogueActivationId,
    line: &crate::plan::RuntimeLineId,
    content: crate::awbc::schema::AwbcContentUnitId,
    target: &crate::value::RuntimeOpaqueValue,
    values: &[crate::plan::RuntimeDialogueValueBinding],
) -> Result<crate::plan::FlowEvent, LineRuntimeError> {
    let template = program
        .content_units
        .get(content.index())
        .ok_or(LineRuntimeError::UnknownContentPlan)?
        .template;
    Ok(crate::plan::FlowEvent::DialogueLine {
        activation,
        line: line.clone(),
        template,
        target: target.clone(),
        values: values.to_vec().into_boxed_slice(),
    })
}

#[cfg(test)]
mod tests {
    use super::build_dialogue_line_event;
    use crate::awbc::schema::{AwbcContentUnit, AwbcContentUnitId, AwbcProgram};
    use crate::entry::RuntimeDialogueContentTemplateDigest;
    use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
    use crate::runtime_id::{
        DialogueActivationId, RuntimeDialogueContentPlanId, RuntimeDialogueContentTemplateId,
        RuntimePersistentFiberId,
    };
    use crate::value::{
        RuntimeCharacterDialogueProducerId, RuntimeOpaquePersistence, RuntimeOpaqueValue,
        RuntimeOpaqueValueClass, RuntimeValue,
    };
    use std::num::NonZeroU32;

    #[test]
    fn awbc_dialogue_line_event_keeps_the_exact_opaque_target() {
        let content = AwbcContentUnitId(0);
        let template = RuntimeDialogueContentTemplateId::from_zero_based(0)
            .expect("fixture template identity");
        let program = AwbcProgram {
            content_templates: vec![crate::awbc::schema::AwbcDialogueContentTemplate {
                id: template,
                digest: RuntimeDialogueContentTemplateDigest::ZERO,
                slots: Vec::new(),
                effects: Vec::new(),
            }],
            content_units: vec![AwbcContentUnit {
                public_id: crate::awbc::schema::AwbcStringId(0),
                template,
                marks: Vec::new(),
                effect_site_count: 0,
                line_task_group: None,
                display: None,
                source: None,
                resources: Vec::new(),
            }],
            ..AwbcProgram::default()
        };
        let activation = DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x61; 32])
                .expect("fixture artifact"),
            RuntimePersistentFiberId::from_allocated(7),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            0,
        );
        let line = crate::plan::RuntimeLineId::from_runtime_line_value("line.target")
            .expect("fixture line identity");
        let owner = RuntimeOpaqueTypeOwner::exact_with(
            RuntimeCharacterDialogueProducerId::get(),
            RuntimeSemanticTypeId::from_bytes([0x62; 32]),
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        );
        let target =
            RuntimeOpaqueValue::new_exact(&owner, RuntimeValue::String("alice".to_owned()));

        let event =
            build_dialogue_line_event(&program, activation.clone(), &line, content, &target, &[])
                .expect("line event uses accepted content");

        assert_eq!(
            event,
            crate::plan::FlowEvent::DialogueLine {
                activation,
                line,
                template,
                target: target.clone(),
                values: Box::new([]),
            }
        );
    }
}
