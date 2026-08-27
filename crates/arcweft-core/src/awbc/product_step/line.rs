use super::dialogue::ProductDialogueTransaction;
use super::{
    ActiveDialogue, AwbcLineTaskPlanView, ProductDialoguePhase, ProductPendingLineOperation,
    ProductStepError,
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
use crate::value::{RuntimeHandleKind, RuntimeLocalBinding, RuntimeValue};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(super) struct ProductActivationProgress {
    pub(super) progressed: bool,
    pub(super) presented: Option<crate::plan::FlowEvent>,
    pub(super) reducer: LineTaskActivation,
    pub(super) pure_stats: Option<crate::step::RuntimePureCallStats>,
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
            RuntimeDialogueResultState::Committed { ty, value } => (ty, value, true),
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
                match parent_owners.get(handle.token()) {
                    Some(destination) => ledger.transfer(
                        handle.token(),
                        &expected,
                        RuntimeHandleOwnerSlot::ParentFiber(*destination),
                    )?,
                    None => {
                        let before_sequence = commands.next_sequence();
                        ledger.drop_owned(handle.token(), &expected, &mut commands)?;
                        emitted_command |= commands.next_sequence() != before_sequence;
                    }
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

    pub(super) fn commit_line_task_commands(
        &mut self,
        batch: super::ProductLineTaskExecutionBatch,
        output: &mut crate::step::RuntimeStepOutput,
    ) {
        self.child_fibers = batch.child_fibers;
        self.next_generation = batch.next_generation;
        self.next_fiber_instance = batch.next_fiber_instance;
        if let Some(stats) = batch.pure_stats {
            self.compact_pure_stats = stats;
        }
        self.consume_observations(batch.observations, output);
    }

    pub(super) fn step_dialogue_activation(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        pure_backend: &mut impl RuntimeCallBackend,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
        let outcomes = std::mem::take(&mut transaction.frame_mut().pending_line_outcomes);
        if self.resume_pending_line_operation(transaction, &outcomes)? {
            return Ok(ProductActivationProgress {
                progressed: true,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
            });
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
                });
            }
        };
        let mut candidate = before.clone();
        let mut candidate_stats = self.compact_pure_stats.clone();
        let mut host = super::ProductVmHost {
            backend: pure_backend,
            fallback_stats: &mut candidate_stats,
        };
        let step = crate::awbc::vm::step_with_host(
            &self.program,
            &mut candidate,
            VmStepOptions {
                max_instructions: 1,
            },
            &mut host,
        )
        .map_err(|error| ProductStepError::Internal(error.to_string()))?;

        let mut owned_observation = None;
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
                VmObservation::Trap(trap) => {
                    return Err(ProductStepError::Internal(format!(
                        "line activation trapped: {trap:?}"
                    )));
                }
                _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
            }
        }

        self.reconcile_activation_fiber_ownership(
            &activation,
            line,
            &before,
            &candidate,
            drop_policy,
        )?;
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
                    })
                }
                VmExit::Returned(_) => Err(LineRuntimeError::ResultNotCommitted.into()),
                VmExit::Cancelled => Err(LineRuntimeError::ResultNotCommitted.into()),
                VmExit::Trapped(trap) => Err(ProductStepError::Internal(format!(
                    "line activation trapped: {trap:?}"
                ))),
                VmExit::Suspended(reason) => Err(ProductStepError::Internal(format!(
                    "line activation suspended outside a typed line operation: {reason:?}"
                ))),
            },
        }
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

    pub(super) fn resume_pending_line_operation(
        &self,
        transaction: &mut ProductDialogueTransaction,
        outcomes: &[RuntimeLineHostOutcome],
    ) -> Result<bool, ProductStepError> {
        let mut candidate = transaction.clone();
        let result = self.resume_pending_line_operation_candidate(&mut candidate, outcomes);
        match &result {
            Ok(_) => *transaction = candidate,
            Err(ProductStepError::Line(
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
            ProductDialoguePhase::Activating { fiber, pending } => (fiber, pending),
            ProductDialoguePhase::Closing(super::ProductDialogueClosing {
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
        activation: &DialogueActivationId,
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

        let view = AwbcLineTaskPlanView::new(&self.program, group)
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        let mut live = LineTaskLiveState::new(&view, activation.clone());
        let elapsed = LogicalDuration::from_nanos(frame.elapsed_nanos);
        for token in line.arm_due_schedules(elapsed)? {
            live.mark_scheduled_ready(token)?;
        }
        let reducer = progress_live_line_task_group(
            &view,
            elapsed,
            LineTaskReadyEvents::new(&BTreeSet::new(), &BTreeSet::new()),
            &mut live,
        )?;
        frame.phase = ProductDialoguePhase::Reducing { line_task: live };
        Ok(ProductActivationProgress {
            progressed: true,
            presented: Some(crate::plan::FlowEvent::DialogueLine {
                activation: activation.clone(),
                line: frame.line.clone(),
                values: frame.values.clone(),
            }),
            reducer,
            pure_stats: None,
        })
    }

    fn reconcile_activation_fiber_ownership(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        before: &FiberState,
        after: &FiberState,
        drop_policy: Option<crate::effect::RuntimeDropPolicy>,
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
                    ledger.transfer(
                        &token,
                        &RuntimeHandleOwnerSlot::ActivationLocal(*source),
                        RuntimeHandleOwnerSlot::ActivationLocal(*destination),
                    )?;
                }
                (Some(source), None) => {
                    let policy = drop_policy.ok_or(LineRuntimeError::UnjournaledHandleDrop)?;
                    let before_sequence = commands.next_sequence();
                    ledger.drop_owned_with_policy(
                        &token,
                        &RuntimeHandleOwnerSlot::ActivationLocal(*source),
                        policy,
                        &mut commands,
                    )?;
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

fn activation_fiber_handle_owners(
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
                    return Err(LineRuntimeError::WrongActivation.into());
                }
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
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
