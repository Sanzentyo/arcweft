use super::dialogue::ProductDialogueTransaction;
use super::{
    ActiveDialogue, AwbcLineTaskPlanView, ProductDialogueClosing, ProductDialogueClosingState,
    ProductDialoguePhase, ProductPendingLineOperation, ProductStepError,
};
use crate::awbc::fiber::{FiberCursor, FiberState, FiberTerminalValue, runtime_value_matches_type};
use crate::awbc::schema::{
    AwbcChildJoinPolicy, AwbcContentUnitId, AwbcLineHandleSite, AwbcLineOperation,
    AwbcLineTaskGroupId, AwbcLineTaskNode, AwbcRuntimeTypeShape, AwbcTypeId,
};
use crate::awbc::vm::{VmExit, VmLineOperationArgument, VmObservation, VmStepOptions};
use crate::effect::RuntimeDropPolicy;
use crate::line_task::{
    ChildCancelPolicy, ChildJoinPolicy, LineRuntimeError, LineTaskActivation, LineTaskCommand,
    LineTaskLiveState, LineTaskReadyEvents, LineTaskScheduledCompletion, LineTaskWork,
    LineTaskWorkTag, RuntimeCueLease, RuntimeCueOrigin, RuntimeDialogueActivationState,
    RuntimeDialogueResultState, RuntimeHandleLeaseState, RuntimeHandleOwnerSlot,
    RuntimeHandleResource, RuntimeLineHandleLedger, RuntimePreparedLineCommands,
    RuntimePreparedScheduledPacketTake, RuntimeScheduledChildAdmissionProof,
    RuntimeScheduledCompletionStage, RuntimeScheduledState, RuntimeScopedDeferDecision,
    RuntimeStageActorLease, RuntimeVoiceLease, complete_live_line_task_work,
    progress_live_line_task_group,
};
use crate::pattern::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
};
use crate::presentation::{
    RuntimeCommandQueue, RuntimeDialogueVoiceState, RuntimeLineHostOutcome,
    RuntimeStageCommandOutcome, RuntimeVoiceCommandOutcome,
};
use crate::pure::RuntimeCallBackend;
use crate::runtime_id::{
    DialogueActivationId, RuntimeFiberInstanceId, RuntimeLineHandleSiteId, RuntimeLineHandleToken,
};
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
    pub(super) host_result_take: Option<super::PreparedHostResultTake>,
}

pub(super) enum ProductPublicationProgress {
    Pending,
    Ready {
        resume: crate::awbc::fiber::PreparedFiberResume,
        pattern: crate::awbc::vm::PreparedPatternBinding,
    },
}

struct ProductLineSiteEvidence {
    runtime_site: RuntimeLineHandleSiteId,
    site: AwbcLineHandleSite,
    opaque_owner: RuntimeOpaqueTypeOwner,
}

/// Handle movement and command admission checked while the yielded VM
/// observation still owns its operand packet. No runtime value is copied.
struct PreparedActivationFiberReconciliation {
    ledger: RuntimeLineHandleLedger,
    commands: Option<RuntimePreparedLineCommands>,
}

struct PreparedLineChildSpawn {
    child_generation: u64,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    fiber_instance: crate::runtime_id::RuntimeFiberInstanceId,
    inputs: crate::awbc::fiber::PreparedFunctionInputBinding,
}

#[derive(Clone, Copy)]
pub(super) struct ReservedLineRunIdentity {
    child_generation: u64,
    next_generation: u64,
    next_fiber_instance: crate::runtime_id::RuntimeIdCursor,
    fiber_instance: RuntimeFiberInstanceId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProductExistingChildAction {
    CancelAndJoin,
    MarkClosing,
    Detach,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExistingChildLocation {
    Executor,
    Batch,
}

enum PreparedLineRunSource {
    Shared,
    Scheduled(RuntimePreparedScheduledPacketTake),
}

struct PreparedLineRun {
    owner: super::ProductChildFiberOwner,
    source: PreparedLineRunSource,
    spawn: PreparedLineChildSpawn,
    cancelled_before_start: bool,
}

impl PreparedLineRun {
    fn identity(&self) -> ReservedLineRunIdentity {
        ReservedLineRunIdentity {
            child_generation: self.spawn.child_generation,
            next_generation: self.spawn.next_generation,
            next_fiber_instance: self.spawn.next_fiber_instance,
            fiber_instance: self.spawn.fiber_instance,
        }
    }
}

struct PreparedExistingChildCancel {
    instance: RuntimeFiberInstanceId,
    location: ExistingChildLocation,
    scheduled: Option<(RuntimeLineHandleToken, RuntimeScheduledChildAdmissionProof)>,
}

pub(super) struct PreparedLineTaskCommands {
    line_task: Option<LineTaskLiveState>,
    scheduled: RuntimeScheduledCompletionStage,
    commands: RuntimePreparedLineCommands,
    runs: Vec<PreparedLineRun>,
    cancellations: Vec<PreparedExistingChildCancel>,
    executor_actions: BTreeMap<RuntimeFiberInstanceId, ProductExistingChildAction>,
    batch_actions: BTreeMap<RuntimeFiberInstanceId, ProductExistingChildAction>,
}

impl PreparedLineTaskCommands {
    pub(super) fn child_action(
        &self,
        instance: RuntimeFiberInstanceId,
    ) -> Option<ProductExistingChildAction> {
        self.executor_actions
            .get(&instance)
            .or_else(|| self.batch_actions.get(&instance))
            .copied()
    }

    pub(super) fn has_joined_run(&self, activation: &DialogueActivationId) -> bool {
        self.runs.iter().any(|run| {
            !run.cancelled_before_start
                && matches!(
                    &run.owner,
                    super::ProductChildFiberOwner::LineTask { tag, policy, .. }
                        if tag.activation_id() == activation && policy.join == ChildJoinPolicy::Join
                )
        })
    }
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
            ProductDialoguePhase::Closing(_) | ProductDialoguePhase::Transitioning => {
                return Err(LineRuntimeError::InvalidResultTransition.into());
            }
        };
        if !line_task.is_closed() {
            return Err(LineRuntimeError::ResultNotCommitted.into());
        }
        let prepared_resume = self.fiber.validate_resume_at(&self.program, resume)?;
        let (ty, value, begin) = match line.result() {
            RuntimeDialogueResultState::Committed { ty, value }
            | RuntimeDialogueResultState::Selected { ty, value, .. } => (*ty, value, true),
            RuntimeDialogueResultState::Publishing { ty, value } => (*ty, value, false),
            RuntimeDialogueResultState::Uncommitted
            | RuntimeDialogueResultState::Published
            | RuntimeDialogueResultState::Abandoned => {
                return Err(LineRuntimeError::ResultNotCommitted.into());
            }
        };
        if ty != frame.result.ty
            || !runtime_value_matches_type(&self.program, value, frame.result.ty, 0)
        {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        let prepared_pattern = crate::awbc::vm::prepare_pattern_binding(
            &self.program,
            &self.fiber,
            frame.result.pattern,
            value,
        )
        .map_err(|_| LineRuntimeError::ResultPatternOrTypeMismatch)?;
        let written_registers = prepared_pattern
            .registers()
            .iter()
            .copied()
            .collect::<BTreeSet<_>>();
        let frame_instance = self.fiber.active_frame()?.instance;
        let mut parent_owners =
            parent_fiber_handle_owners(self.facade_fiber.execution, &activation, &self.fiber)?;
        parent_owners.retain(|_, owner| {
            !matches!(owner, RuntimeOwnedSlotId::AwbcRegister { fiber, frame, register, .. }
                if *fiber == self.fiber.instance
                    && *frame == frame_instance
                    && written_registers.contains(register))
        });
        let result_handles = unique_line_handles(value)?;
        for handle in &result_handles {
            let destinations = crate::awbc::vm::pattern_handle_destinations(
                &self.program,
                &self.fiber,
                frame.result.pattern,
                value,
                handle.token(),
            )
            .map_err(|_| LineRuntimeError::ResultPatternOrTypeMismatch)?;
            if destinations.len() > 1 {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
            if let Some(register) = destinations.into_iter().next() {
                let owner = RuntimeOwnedSlotId::AwbcRegister {
                    execution: self.facade_fiber.execution,
                    fiber: self.fiber.instance,
                    frame: frame_instance,
                    register,
                };
                if parent_owners
                    .insert(handle.token().clone(), owner)
                    .is_some()
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
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
            if emitted_command {
                line.record_commands(&activation, commands)?;
            }
            line.commit_ledger(ledger);
            line.begin_result_publication()?;
            frame.phase = ProductDialoguePhase::Publishing { line_task };
        }
        if line.has_pending_commands() {
            return Ok(ProductPublicationProgress::Pending);
        }
        let mut ledger = line.ledger().clone();
        let mut commands = RuntimeCommandQueue::new(activation.clone(), line.command_sequence());
        let mut emitted_command = false;
        for handle in &result_handles {
            if parent_owners.contains_key(handle.token()) {
                continue;
            }
            let lease = ledger
                .lease(handle.token())
                .ok_or(LineRuntimeError::UnknownHandle)?;
            if matches!(
                lease.state(),
                RuntimeHandleLeaseState::Released | RuntimeHandleLeaseState::Cancelling
            ) {
                continue;
            }
            let before_sequence = commands.next_sequence();
            ledger.drop_owned(
                handle.token(),
                &RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone()),
                &mut commands,
            )?;
            emitted_command |= commands.next_sequence() != before_sequence;
        }
        if emitted_command {
            line.record_commands(&activation, commands)?;
        }
        line.commit_ledger(ledger);
        if line.has_pending_commands() {
            return Ok(ProductPublicationProgress::Pending);
        }
        let mut ledger = line.ledger().clone();
        for handle in &result_handles {
            if let Some(destination) = parent_owners.get(handle.token()) {
                ledger.transfer(
                    handle.token(),
                    &RuntimeHandleOwnerSlot::DialogueResult(handle.path().clone()),
                    RuntimeHandleOwnerSlot::ParentFiber(*destination),
                )?;
            }
        }
        line.commit_ledger(ledger);
        if line.ledger().leases().values().any(|lease| {
            lease.state() != RuntimeHandleLeaseState::Released
                && !matches!(lease.owner(), RuntimeHandleOwnerSlot::ParentFiber(_))
        }) {
            return Err(LineRuntimeError::UnownedLeaseAtPublish.into());
        }
        Ok(ProductPublicationProgress::Ready {
            resume: prepared_resume,
            pattern: prepared_pattern,
        })
    }

    pub(super) fn prepare_line_task_commands(
        &self,
        transaction: &mut ProductDialogueTransaction,
        activation: LineTaskActivation,
    ) -> Result<super::ProductLineTaskExecutionBatch, ProductStepError> {
        let mut batch = super::ProductLineTaskExecutionBatch {
            child_fibers: VecDeque::new(),
            existing_child_actions: BTreeMap::new(),
            dialogue_effect_callback_activations: self.dialogue_effect_callback_activations.clone(),
            next_generation: self.next_generation,
            next_fiber_instance: self.next_fiber_instance,
            observations: Vec::new(),
            pure_stats: None,
            line_task_activations: Vec::new(),
            line_task_baseline: None,
            line_task_reserved_runs: Vec::new(),
        };
        self.prepare_line_task_commands_from(transaction, activation, &mut batch)?;
        Ok(batch)
    }

    pub(super) fn prepare_line_task_commands_from(
        &self,
        transaction: &mut ProductDialogueTransaction,
        activation: LineTaskActivation,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) -> Result<(), ProductStepError> {
        let baseline = batch
            .line_task_baseline
            .clone()
            .unwrap_or_else(|| transaction.frame().line_task().cloned());
        let mut trial = LineTaskActivation::default();
        for staged in &batch.line_task_activations {
            trial.append(staged.clone());
        }
        trial.append(activation.clone());
        let preview = self.preflight_line_task_command_sequence(
            transaction,
            trial,
            batch,
            Some(&baseline),
            &batch.line_task_reserved_runs,
            true,
        )?;
        if preview.runs.len() < batch.line_task_reserved_runs.len() {
            return Err(ProductStepError::Internal(
                "line-task reservation preview removed a prior Run".to_owned(),
            ));
        }
        if let Some(last) = preview.runs.last()
            && preview.runs.len() > batch.line_task_reserved_runs.len()
        {
            batch.next_generation = last.spawn.next_generation;
            batch.next_fiber_instance = last.spawn.next_fiber_instance;
        }
        if let Some(line_task) = &preview.line_task {
            *transaction
                .frame_mut()
                .line_task_mut()
                .expect("preflighted line-task metadata remains present") = line_task.clone();
        }
        batch.line_task_baseline.get_or_insert(baseline);
        batch.line_task_activations.push(activation);
        batch.line_task_reserved_runs =
            preview.runs.iter().map(PreparedLineRun::identity).collect();
        Ok(())
    }

    pub(super) fn rollback_line_task_preview(
        &self,
        transaction: &mut ProductDialogueTransaction,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) {
        if let Some(Some(baseline)) = batch.line_task_baseline.take() {
            *transaction
                .frame_mut()
                .line_task_mut()
                .expect("previewed line-task metadata remains present") = baseline;
        }
        batch.line_task_activations.clear();
        batch.line_task_reserved_runs.clear();
        // Reserved identities are deliberately burned. Other staged child
        // owners may already use later IDs from this same batch.
    }

    fn preflight_existing_line_child_cancel(
        &self,
        line: &RuntimeDialogueActivationState<AwbcTypeId>,
        stage: &mut RuntimeScheduledCompletionStage,
        commands: &mut RuntimeCommandQueue,
        child: &super::ProductChildFiber,
        tag: &LineTaskWorkTag,
    ) -> Result<
        Option<(RuntimeLineHandleToken, RuntimeScheduledChildAdmissionProof)>,
        ProductStepError,
    > {
        let live = product_fiber_handle_tokens(self.facade_fiber.execution, &child.fiber)?;
        let Some(token) = tag.scheduled_token().cloned() else {
            line.stage_child_scope_finish(
                stage,
                commands,
                tag,
                &live,
                &BTreeSet::new(),
                RuntimeDropPolicy::Default,
            )?;
            return Ok(None);
        };
        let locals = line.scheduled_child_locals(&token)?;
        let values = child.fiber.function_argument_storage(&self.program)?;
        if locals.len() != values.len() {
            return Err(LineRuntimeError::InvalidScheduledCaptureGraph.into());
        }
        let references = locals
            .iter()
            .zip(values)
            .map(|(local, storage)| (*local, storage))
            .collect::<Vec<_>>();
        let mut returned = BTreeSet::new();
        for value in references.iter().flat_map(|(_, storage)| storage.values()) {
            returned.extend(
                unique_line_handles(value)?
                    .into_iter()
                    .map(|handle| handle.token().clone()),
            );
        }
        let proof = line.inspect_scheduled_child_binding_refs(
            &token,
            &references,
            RuntimeScheduledState::Cancelled,
        )?;
        line.stage_child_scope_finish(
            stage,
            commands,
            tag,
            &live,
            &returned,
            RuntimeDropPolicy::Default,
        )?;
        line.stage_scheduled_child_work_completion_refs(stage, &proof, &references, false, true)?;
        Ok(Some((token, proof)))
    }

    pub(super) fn preflight_line_task_commands(
        &self,
        transaction: &mut ProductDialogueTransaction,
        batch: &super::ProductLineTaskExecutionBatch,
    ) -> Result<PreparedLineTaskCommands, ProductStepError> {
        let mut activation = LineTaskActivation::default();
        for staged in &batch.line_task_activations {
            activation.append(staged.clone());
        }
        let prepared = self.preflight_line_task_command_sequence(
            transaction,
            activation,
            batch,
            batch.line_task_baseline.as_ref(),
            &batch.line_task_reserved_runs,
            false,
        )?;
        if prepared.runs.len() != batch.line_task_reserved_runs.len() {
            return Err(ProductStepError::Internal(
                "line-task final Run inventory disagrees with its reserved identities".to_owned(),
            ));
        }
        Ok(prepared)
    }

    fn preflight_line_task_command_sequence(
        &self,
        transaction: &mut ProductDialogueTransaction,
        activation: LineTaskActivation,
        batch: &super::ProductLineTaskExecutionBatch,
        baseline: Option<&Option<LineTaskLiveState>>,
        reserved_runs: &[ReservedLineRunIdentity],
        allow_new_runs: bool,
    ) -> Result<PreparedLineTaskCommands, ProductStepError> {
        let content = transaction.frame().content;
        let mut line_task = baseline
            .cloned()
            .unwrap_or_else(|| transaction.frame().line_task().cloned());
        let mut scheduled = transaction.line().begin_scheduled_completion_stage();
        let mut host_commands = RuntimeCommandQueue::new(
            transaction.activation().clone(),
            transaction.line().command_sequence(),
        );
        let mut pending = VecDeque::from(activation.commands);
        let mut scheduled_completions = VecDeque::from(activation.scheduled_completions);
        let mut completed_tokens = BTreeSet::new();
        let mut started_tokens = BTreeSet::new();
        let mut runs = Vec::<PreparedLineRun>::new();
        let mut cancellations = Vec::<PreparedExistingChildCancel>::new();
        let mut executor_actions = batch.existing_child_actions.clone();
        let mut batch_actions = BTreeMap::new();
        let mut generation = batch.next_generation;
        let mut fiber_cursor = batch.next_fiber_instance;

        while let Some(completion) = scheduled_completions.pop_front() {
            if !completed_tokens.insert(completion.token().clone()) {
                return Err(LineRuntimeError::InvalidScheduledCaptureTransition.into());
            }
            transaction
                .line()
                .stage_unstarted_scheduled_completion(&mut scheduled, &completion)?;
        }
        while let Some(command) = pending.pop_front() {
            match command {
                LineTaskCommand::Run { tag, policy } => {
                    let view = self
                        .line_task_view(content)
                        .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                    let function = view
                        .function_for(&tag)
                        .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                    let (source, arguments) = if let Some(token) = tag.scheduled_token().cloned() {
                        if completed_tokens.contains(&token)
                            || !started_tokens.insert(token.clone())
                        {
                            return Err(LineRuntimeError::InvalidScheduledCaptureTransition.into());
                        }
                        let packet = transaction.line().scheduled_packet_for_child(&token)?;
                        let arguments = packet
                            .iter()
                            .map(|binding| &binding.value)
                            .collect::<Vec<_>>();
                        (
                            PreparedLineRunSource::Scheduled(
                                transaction.line().inspect_scheduled_packet_take(&token)?,
                            ),
                            arguments,
                        )
                    } else {
                        if transaction
                            .frame()
                            .task_inputs
                            .iter()
                            .any(|value| !value.ownership().permits_copy())
                        {
                            return Err(ProductStepError::Type(
                                "line-task shared input has no deep Copy proof".to_owned(),
                            ));
                        }
                        (
                            PreparedLineRunSource::Shared,
                            transaction.frame().task_inputs.iter().collect::<Vec<_>>(),
                        )
                    };
                    if policy.join == ChildJoinPolicy::Detached {
                        for value in &arguments {
                            if !unique_line_handles(value)?.is_empty() {
                                return Err(LineRuntimeError::DetachedAffineCapture.into());
                            }
                        }
                    }
                    let inputs = crate::awbc::fiber::validate_function_argument_value_refs(
                        &self.program,
                        function,
                        &arguments,
                    )
                    .map_err(|error| ProductStepError::Type(error.to_string()))?;
                    let identity = if let Some(identity) = reserved_runs.get(runs.len()).copied() {
                        identity
                    } else if allow_new_runs {
                        let next_generation = generation
                            .checked_add(1)
                            .ok_or(ProductStepError::ChildGenerationOverflow)?;
                        let mut next_fiber_instance = fiber_cursor;
                        let fiber_instance = next_fiber_instance
                            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
                            .map(RuntimeFiberInstanceId::from_allocated)?;
                        let identity = ReservedLineRunIdentity {
                            child_generation: generation,
                            next_generation,
                            next_fiber_instance,
                            fiber_instance,
                        };
                        generation = next_generation;
                        fiber_cursor = next_fiber_instance;
                        identity
                    } else {
                        return Err(ProductStepError::Internal(
                            "line-task Run has no pre-reserved child identity".to_owned(),
                        ));
                    };
                    let phase = if matches!(tag.work(), LineTaskWork::Node(_)) {
                        super::ProductLineTaskFiberPhase::Active
                    } else {
                        super::ProductLineTaskFiberPhase::Closing
                    };
                    runs.push(PreparedLineRun {
                        owner: super::ProductChildFiberOwner::LineTask {
                            content,
                            tag,
                            policy,
                            phase,
                        },
                        source,
                        spawn: PreparedLineChildSpawn {
                            child_generation: identity.child_generation,
                            next_generation: identity.next_generation,
                            next_fiber_instance: identity.next_fiber_instance,
                            fiber_instance: identity.fiber_instance,
                            inputs,
                        },
                        cancelled_before_start: false,
                    });
                }
                LineTaskCommand::Cancel { tag } => {
                    let mut completed = Vec::new();
                    let mut unstarted_cancelled_tokens = Vec::new();
                    for (location, child) in self
                        .child_fibers
                        .iter()
                        .map(|child| (ExistingChildLocation::Executor, child))
                        .chain(
                            batch
                                .child_fibers
                                .iter()
                                .map(|child| (ExistingChildLocation::Batch, child)),
                        )
                    {
                        let super::ProductChildFiberOwner::LineTask {
                            content: child_content,
                            tag: child_tag,
                            policy,
                            ..
                        } = &child.owner
                        else {
                            continue;
                        };
                        if *child_content != content || child_tag != &tag {
                            continue;
                        }
                        let actions = match location {
                            ExistingChildLocation::Executor => &mut executor_actions,
                            ExistingChildLocation::Batch => &mut batch_actions,
                        };
                        let instance = child.fiber.instance;
                        if matches!(
                            actions.get(&instance),
                            Some(
                                ProductExistingChildAction::CancelAndJoin
                                    | ProductExistingChildAction::Detach
                            )
                        ) {
                            continue;
                        }
                        match policy.cancel {
                            ChildCancelPolicy::CancelAndJoin => {
                                let scheduled_child = self.preflight_existing_line_child_cancel(
                                    transaction.line(),
                                    &mut scheduled,
                                    &mut host_commands,
                                    child,
                                    child_tag,
                                )?;
                                cancellations.push(PreparedExistingChildCancel {
                                    instance,
                                    location,
                                    scheduled: scheduled_child,
                                });
                                actions.insert(instance, ProductExistingChildAction::CancelAndJoin);
                                completed.push(child_tag.clone());
                            }
                            ChildCancelPolicy::Finish => {
                                actions.insert(instance, ProductExistingChildAction::MarkClosing);
                            }
                            ChildCancelPolicy::Detach => {
                                if !product_fiber_handle_owners(
                                    self.facade_fiber.execution,
                                    &child.fiber,
                                )?
                                .is_empty()
                                {
                                    return Err(LineRuntimeError::DetachedAffineCapture.into());
                                }
                                actions.insert(instance, ProductExistingChildAction::Detach);
                            }
                        }
                    }
                    for run in &mut runs {
                        let super::ProductChildFiberOwner::LineTask {
                            content: run_content,
                            tag: run_tag,
                            policy,
                            phase,
                        } = &mut run.owner
                        else {
                            continue;
                        };
                        if *run_content != content || run_tag != &tag || run.cancelled_before_start
                        {
                            continue;
                        }
                        match policy.cancel {
                            ChildCancelPolicy::CancelAndJoin => {
                                run.cancelled_before_start = true;
                                if let Some(token) = run_tag.scheduled_token().cloned() {
                                    unstarted_cancelled_tokens.push(token);
                                }
                                completed.push(run_tag.clone());
                            }
                            ChildCancelPolicy::Finish => {
                                *phase = super::ProductLineTaskFiberPhase::Closing
                            }
                            ChildCancelPolicy::Detach => {
                                let values = match &run.source {
                                    PreparedLineRunSource::Shared => {
                                        transaction.frame().task_inputs.iter().collect::<Vec<_>>()
                                    }
                                    PreparedLineRunSource::Scheduled(_) => transaction
                                        .line()
                                        .scheduled_packet_for_child(
                                            run_tag
                                                .scheduled_token()
                                                .expect("scheduled run has token"),
                                        )?
                                        .iter()
                                        .map(|binding| &binding.value)
                                        .collect::<Vec<_>>(),
                                };
                                for value in values {
                                    if !unique_line_handles(value)?.is_empty() {
                                        return Err(LineRuntimeError::DetachedAffineCapture.into());
                                    }
                                }
                                run.owner = super::ProductChildFiberOwner::Independent;
                            }
                        }
                    }
                    for completed_tag in completed {
                        let view = self
                            .line_task_view(content)
                            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
                        let state = line_task
                            .as_mut()
                            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
                        let completion =
                            complete_live_line_task_work(&view, state, completed_tag, false)?;
                        pending.extend(completion.commands);
                        scheduled_completions.extend(completion.scheduled_completions);
                    }
                    for token in unstarted_cancelled_tokens {
                        if !scheduled_completions
                            .iter()
                            .any(|completion| completion.token() == &token)
                        {
                            scheduled_completions.push_back(LineTaskScheduledCompletion::new(
                                token,
                                crate::line_task::ScopeExit::Cancelled,
                            ));
                        }
                    }
                    while let Some(completion) = scheduled_completions.pop_front() {
                        if !completed_tokens.insert(completion.token().clone()) {
                            return Err(LineRuntimeError::InvalidScheduledCaptureTransition.into());
                        }
                        transaction
                            .line()
                            .stage_unstarted_scheduled_completion(&mut scheduled, &completion)?;
                    }
                }
            }
        }
        let activation = transaction.activation().clone();
        let commands = transaction
            .line_mut()
            .inspect_record_commands(&activation, &host_commands)?;
        Ok(PreparedLineTaskCommands {
            line_task,
            scheduled,
            commands,
            runs,
            cancellations,
            executor_actions,
            batch_actions,
        })
    }

    pub(super) fn realize_line_task_commands(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        batch: &mut super::ProductLineTaskExecutionBatch,
        mut prepared: PreparedLineTaskCommands,
    ) {
        batch.line_task_activations.clear();
        transaction
            .line_mut()
            .record_commands_prepared(prepared.commands);
        for cancellation in prepared.cancellations {
            let children = match cancellation.location {
                ExistingChildLocation::Executor => &mut self.child_fibers,
                ExistingChildLocation::Batch => &mut batch.child_fibers,
            };
            let index = children
                .iter()
                .position(|child| child.fiber.instance == cancellation.instance)
                .expect("preflighted line child remains in its owner queue");
            let mut child = children
                .remove(index)
                .expect("preflighted line child has one owner");
            if let Some((token, proof)) = cancellation.scheduled {
                let locals = transaction
                    .line()
                    .scheduled_child_locals(&token)
                    .expect("preflighted scheduled child retains its local layout");
                let values = child
                    .fiber
                    .take_function_argument_storage(&self.program)
                    .expect("preflighted scheduled child retains its input frame");
                let bindings = locals
                    .into_vec()
                    .into_iter()
                    .zip(values)
                    .map(|(local, storage)| crate::value::RuntimeLocalSlot::new(local, storage))
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                transaction
                    .line_mut()
                    .admit_scheduled_child_bindings_prepared(proof, bindings);
            }
            batch
                .observations
                .extend(crate::awbc::vm::cancel_fiber(&mut child.fiber).observations);
            match cancellation.location {
                ExistingChildLocation::Executor => {
                    prepared.executor_actions.remove(&cancellation.instance);
                }
                ExistingChildLocation::Batch => {
                    prepared.batch_actions.remove(&cancellation.instance);
                }
            }
        }
        for run in prepared.runs {
            if run.cancelled_before_start {
                continue;
            }
            let args = match run.source {
                PreparedLineRunSource::Shared => transaction.frame().task_inputs.to_vec(),
                PreparedLineRunSource::Scheduled(packet) => transaction
                    .line_mut()
                    .take_scheduled_capture_packet_prepared(packet)
                    .into_vec()
                    .into_iter()
                    .map(|binding| binding.value)
                    .collect(),
            };
            let child = FiberState::for_function_with_arguments_prepared(
                &self.program,
                crate::awbc::fiber::AwbcFiberRoot::Function(run.spawn.inputs.function()),
                args,
                run.spawn.inputs,
                run.spawn.fiber_instance,
                run.spawn.child_generation,
                self.fiber.budget.quantum.max(1),
            );
            batch.child_fibers.push_back(super::ProductChildFiber {
                owner: run.owner,
                fiber: child,
                runtime_generation: self.runtime_generation,
                pending_host_call: None,
            });
        }
        transaction
            .line_mut()
            .commit_scheduled_completion_stage(prepared.scheduled);
        if let Some(line_task) = prepared.line_task {
            *transaction
                .frame_mut()
                .line_task_mut()
                .expect("preflighted line-task phase remains present") = line_task;
        }
        for child in &mut batch.child_fibers {
            match prepared.batch_actions.remove(&child.fiber.instance) {
                Some(ProductExistingChildAction::MarkClosing) => {
                    let super::ProductChildFiberOwner::LineTask { phase, .. } = &mut child.owner
                    else {
                        unreachable!("preflighted child action names a line-task owner")
                    };
                    *phase = super::ProductLineTaskFiberPhase::Closing;
                }
                Some(ProductExistingChildAction::Detach) => {
                    child.owner = super::ProductChildFiberOwner::Independent;
                }
                Some(ProductExistingChildAction::CancelAndJoin) => {
                    unreachable!("preflighted cancellation was consumed above")
                }
                None => {}
            }
        }
        assert!(
            prepared.batch_actions.is_empty(),
            "preflighted batch actions name existing staged children"
        );
        batch.existing_child_actions = prepared.executor_actions;
    }

    pub(super) fn prepare_next_deferred_child(
        &self,
        transaction: &mut ProductDialogueTransaction,
        exit: crate::line_task::ScopeExit,
        batch: &mut super::ProductLineTaskExecutionBatch,
    ) -> Result<bool, ProductStepError> {
        let activation = transaction.activation().clone();
        if transaction.line().deferred_exit().is_none()
            && transaction.line().deferred_registrations().is_empty()
        {
            return Ok(false);
        }
        if transaction.line().has_pending_commands() {
            return Ok(false);
        }
        if transaction.line().deferred_inflight().is_some() {
            return Ok(false);
        }
        let effective_exit = transaction.line().deferred_exit().unwrap_or(exit);
        let prepared_spawn = transaction
            .line()
            .deferred_registrations()
            .last()
            .filter(|registration| registration.outcome_filter().matches(effective_exit))
            .map(|registration| {
                let function = self
                    .program
                    .defer_sites
                    .get(registration.site().index())
                    .copied()
                    .ok_or(LineRuntimeError::UnknownDeferredSite {
                        site: registration.site(),
                    })?;
                batch.prepare_spawn(self, function, registration.captures())
            })
            .transpose()?;
        let prepared = transaction
            .line_mut()
            .inspect_next_deferred(&activation, exit)?;
        let Some(step) = transaction
            .line_mut()
            .commit_next_deferred_prepared(prepared)
        else {
            return Ok(false);
        };
        match step {
            crate::line_task::RuntimeDeferUnwindStep::Skipped(_) => Ok(true),
            crate::line_task::RuntimeDeferUnwindStep::Run(registration) => {
                let (registration_id, site, _outcome, captures) = registration.into_parts();
                assert_eq!(
                    transaction.line().deferred_inflight(),
                    Some((registration_id, site)),
                    "prepared line-root defer retains its in-flight identity"
                );
                let content = transaction.frame().content;
                batch.spawn_prepared(
                    self,
                    super::ProductChildFiberOwner::Deferred {
                        content,
                        activation,
                        registration: registration_id,
                        site,
                    },
                    captures,
                    prepared_spawn.expect("Run registration had a preflighted child ABI"),
                );
                Ok(true)
            }
        }
    }

    pub(super) fn commit_line_task_commands(
        &mut self,
        mut batch: super::ProductLineTaskExecutionBatch,
        output: &mut crate::step::RuntimeStepOutput,
    ) {
        let mut index = 0;
        while index < self.child_fibers.len() {
            let instance = self.child_fibers[index].fiber.instance;
            match batch.existing_child_actions.remove(&instance) {
                Some(ProductExistingChildAction::CancelAndJoin) => {
                    let mut child = self
                        .child_fibers
                        .remove(index)
                        .expect("preflighted child action retains its exact instance");
                    batch
                        .observations
                        .extend(crate::awbc::vm::cancel_fiber(&mut child.fiber).observations);
                }
                Some(ProductExistingChildAction::MarkClosing) => {
                    let super::ProductChildFiberOwner::LineTask { phase, .. } =
                        &mut self.child_fibers[index].owner
                    else {
                        unreachable!("preflighted MarkClosing names a line-task child")
                    };
                    *phase = super::ProductLineTaskFiberPhase::Closing;
                    index += 1;
                }
                Some(ProductExistingChildAction::Detach) => {
                    self.child_fibers[index].owner = super::ProductChildFiberOwner::Independent;
                    index += 1;
                }
                None => index += 1,
            }
        }
        assert!(
            batch.existing_child_actions.is_empty(),
            "preflighted child actions must target existing fibers"
        );
        self.child_fibers.append(&mut batch.child_fibers);
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
        self.step_dialogue_activation_with_host_results(transaction, &mut Vec::new(), pure_backend)
    }

    pub(super) fn step_dialogue_activation_with_host_results(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        host_results: &mut Vec<crate::step::RuntimeHostCallResult>,
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
                host_result_take: None,
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
                    host_result_take: None,
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
                host_result_take: None,
            });
        }
        if !outcomes.is_empty() {
            return Err(LineRuntimeError::StaleCommandOutcome.into());
        }
        settle_scoped_defer_releases(transaction);
        if let Some(pending) = transaction.frame().pending_activation_host_call.clone() {
            return self.resume_activation_host_call(transaction, pending, host_results);
        }
        let activation_returned_unit = matches!(
            &transaction.frame().phase,
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } if matches!(
                fiber.terminal.as_ref(),
                Some(FiberTerminalValue::Returned(None))
            )
        );
        if matches!(
            transaction.line().result(),
            RuntimeDialogueResultState::Committed { .. }
        ) || activation_returned_unit
        {
            return self.resume_activation_out(transaction);
        }
        let (frame, line) = transaction.parts_mut();
        let owner_content = frame.content;
        let before = match &frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => fiber,
            ProductDialoguePhase::Activating {
                pending: Some(_), ..
            }
            | ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Closing(_)
            | ProductDialoguePhase::Transitioning => {
                return Ok(ProductActivationProgress {
                    progressed: false,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: None,
                    execution: None,
                    host_calls: Vec::new(),
                    host_result_take: None,
                });
            }
        };
        if fiber_has_scoped_defer_inflight(before) {
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
                host_result_take: None,
            });
        }
        let before_owners =
            activation_fiber_handle_owners(self.facade_fiber.execution, &activation, before)?;
        // Effect and cleanup observations do not yield at their source
        // instruction. An inert checkpoint retains the pre-step owner graph
        // until their complete ledger/materialization preflight commits.
        let effect_site = before.cursor;
        let effect_frame = before.active_frame()?.instance;
        let mut effect_checkpoint = Some(before.checkpoint()?);
        let phase = std::mem::replace(&mut frame.phase, ProductDialoguePhase::Transitioning);
        let ProductDialoguePhase::Activating {
            fiber,
            pending: None,
        } = phase
        else {
            unreachable!("borrowed activation phase was checked before taking its fiber")
        };
        let mut candidate = Some(fiber);
        let result: Result<ProductActivationProgress, ProductStepError> = (|| {
            let mut candidate_stats = self.compact_pure_stats;
            let mut host = super::ProductVmHost {
                backend: pure_backend,
                fallback_stats: &mut candidate_stats,
                program_owner: crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
            };
            let context = crate::awbc::vm::VmExecutionContext::for_program(
                self.artifact_fingerprint,
                Arc::clone(&self.program),
            );
            let step = crate::awbc::vm::step_with_host_context(
                &self.program,
                candidate.as_mut().expect("activation fiber remains owned"),
                VmStepOptions {
                    max_instructions: 1,
                },
                &context,
                &mut host,
            );
            let step = match step {
                Ok(step) => step,
                Err(error) => {
                    candidate
                        .as_mut()
                        .expect("activation fiber remains owned")
                        .restore(
                            effect_checkpoint
                                .take()
                                .expect("activation step retained its checkpoint"),
                            &crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
                        )
                        .expect("accepted activation checkpoint restores in its original program");
                    return Err(ProductStepError::Internal(error.to_string()));
                }
            };

            let mut owned_observation = None;
            let mut line_defer_observation = None;
            let mut scoped_defer_observation = None;
            let mut scoped_unwind_observation = None;
            let mut scoped_failure = None;
            let mut activation_effects = Vec::new();
            let mut drops = crate::line_task::RuntimeHandleDropAuthorization::default();
            let collected = (|| -> Result<(), ProductStepError> {
                for observation in step.observations {
                    match observation {
                        VmObservation::Instruction { .. } => {}
                        VmObservation::Drop { policy } => {
                            drops.set_boundary(Some(policy))?;
                        }
                        VmObservation::DiscardedValue(value) => {
                            drops.authorize_displaced(&value)?
                        }
                        VmObservation::LineOperation { .. }
                        | VmObservation::DialogueResult { .. } => {
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
                Ok(())
            })();
            if let Err(error) = collected {
                drop(owned_observation.take());
                drop(line_defer_observation.take());
                drop(scoped_defer_observation.take());
                drop(scoped_unwind_observation.take());
                drop(std::mem::take(&mut activation_effects));
                candidate
                    .as_mut()
                    .expect("activation fiber remains owned")
                    .restore(
                        effect_checkpoint
                            .take()
                            .expect("activation step retained its checkpoint"),
                        &crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
                    )
                    .expect("accepted activation checkpoint restores in its original program");
                return Err(error);
            }

            if !activation_effects.is_empty()
                && (line_defer_observation.is_some()
                    || scoped_defer_observation.is_some()
                    || scoped_unwind_observation.is_some()
                    || owned_observation.is_some())
            {
                drop(activation_effects);
                candidate
                    .as_mut()
                    .expect("activation fiber remains owned")
                    .restore(
                        effect_checkpoint
                            .take()
                            .expect("effect-producing step retained its checkpoint"),
                        &crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
                    )
                    .expect("accepted activation checkpoint restores in its original program");
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }

            let deferred_tokens = match line_defer_observation {
                Some(VmObservation::LineDeferRegistration {
                    cursor,
                    site,
                    outcome,
                    mut captures,
                }) => match self.register_line_root_defer(
                    &activation,
                    line,
                    candidate.as_mut().expect("activation fiber remains owned"),
                    cursor,
                    site,
                    outcome,
                    &mut captures,
                ) {
                    Ok(tokens) => tokens,
                    Err(error) => {
                        restore_observed_operands(
                            &self.program,
                            candidate.as_mut().expect("activation fiber remains owned"),
                            cursor,
                            captures,
                        );
                        return Err(error);
                    }
                },
                Some(_) => return Err(LineRuntimeError::InvalidActivationOperation.into()),
                None => BTreeSet::new(),
            };
            match scoped_defer_observation {
                Some(VmObservation::ScopedDeferRegistration {
                    cursor,
                    scope,
                    site,
                    outcome,
                    mut captures,
                }) => {
                    if let Err(error) = self.register_scoped_defer(
                        &activation,
                        line,
                        candidate.as_mut().expect("activation fiber remains owned"),
                        cursor,
                        scope,
                        site,
                        outcome,
                        &mut captures,
                    ) {
                        restore_observed_operands(
                            &self.program,
                            candidate.as_mut().expect("activation fiber remains owned"),
                            cursor,
                            captures,
                        );
                        return Err(error);
                    }
                }
                Some(_) => return Err(LineRuntimeError::InvalidActivationOperation.into()),
                None => {}
            }
            let (mut execution, scoped_reconciled_tokens) = match scoped_unwind_observation {
                Some(VmObservation::ScopedDeferUnwind { cursor, scope }) => {
                    let mut batch = super::ProductLineTaskExecutionBatch {
                        child_fibers: VecDeque::new(),
                        existing_child_actions: BTreeMap::new(),
                        line_task_activations: Vec::new(),
                        line_task_baseline: None,
                        line_task_reserved_runs: Vec::new(),
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
                        owner_content,
                        line,
                        candidate.as_mut().expect("activation fiber remains owned"),
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
            let composed_observation = matches!(
                owned_observation.as_ref(),
                Some(VmObservation::LineOperation { operation, .. })
                    if matches!(
                        self.program.line_operations.get(operation.index()),
                        Some(AwbcLineOperation::Schedule { .. })
                    )
            ) || matches!(
                owned_observation.as_ref(),
                Some(VmObservation::DialogueResult { .. })
            );
            let inspected = self.inspect_activation_fiber_reconciliation(
                &activation,
                line,
                &before_owners,
                candidate.as_ref().expect("activation fiber remains owned"),
                owned_observation.as_ref(),
                (!activation_effects.is_empty()).then_some((
                    effect_site,
                    effect_frame,
                    activation_effects.as_slice(),
                )),
                &drops,
                &deferred_tokens,
                &scoped_reconciled_tokens,
            );
            let mut reconciliation = Some(match inspected {
                Ok(prepared) => prepared,
                Err(error) => {
                    if !activation_effects.is_empty() {
                        drop(std::mem::take(&mut activation_effects));
                        candidate
                            .as_mut()
                            .expect("activation fiber remains owned")
                            .restore(
                                effect_checkpoint
                                    .take()
                                    .expect("effect-producing step retained its checkpoint"),
                                &crate::task::RuntimeProgramOwner::Awbc(Arc::clone(&self.program)),
                            )
                            .expect(
                                "accepted activation checkpoint restores in its original program",
                            );
                    } else if let Some(observation) = owned_observation.take() {
                        restore_owned_vm_observation(
                            &self.program,
                            candidate.as_mut().expect("activation fiber remains owned"),
                            observation,
                        );
                    }
                    return Err(error);
                }
            });
            if !composed_observation {
                Self::commit_activation_fiber_reconciliation(
                    line,
                    reconciliation
                        .take()
                        .expect("inspected activation reconciliation"),
                );
            }
            if !activation_effects.is_empty() {
                let batch = execution.get_or_insert_with(|| empty_activation_batch(self));
                batch.observations.extend(activation_effects);
            }
            if let Some(trap) = scoped_failure {
                if let Some(observation) = owned_observation.take() {
                    restore_owned_vm_observation(
                        &self.program,
                        candidate.as_mut().expect("activation fiber remains owned"),
                        observation,
                    );
                }
                return Err(ProductStepError::ActivationTrap(trap));
            }
            match owned_observation {
                Some(VmObservation::LineOperation {
                    cursor,
                    dst,
                    operation,
                    mut args,
                }) => {
                    let pending_operation = self.execute_product_line_operation(
                        &activation,
                        frame,
                        line,
                        candidate.as_mut().expect("activation fiber remains owned"),
                        cursor,
                        dst,
                        operation,
                        &mut args,
                        reconciliation.take(),
                    );
                    let pending_operation = match pending_operation {
                        Ok(pending) => pending,
                        Err(error) => {
                            restore_owned_vm_observation(
                                &self.program,
                                candidate.as_mut().expect("activation fiber remains owned"),
                                VmObservation::LineOperation {
                                    cursor,
                                    dst,
                                    operation,
                                    args,
                                },
                            );
                            return Err(error);
                        }
                    };
                    let progressed = pending_operation.is_none();
                    frame.phase = ProductDialoguePhase::Activating {
                        fiber: candidate.take().expect("activation fiber remains owned"),
                        pending: pending_operation,
                    };
                    Ok(ProductActivationProgress {
                        progressed,
                        presented: None,
                        reducer: LineTaskActivation::default(),
                        pure_stats: Some(candidate_stats),
                        execution: None,
                        host_calls: Vec::new(),
                        host_result_take: None,
                    })
                }
                Some(VmObservation::DialogueResult {
                    cursor,
                    source_register,
                    source,
                }) => {
                    let mut source = Some(source);
                    let progress = self.commit_product_dialogue_result(
                        &activation,
                        frame,
                        line,
                        candidate.as_mut().expect("activation fiber remains owned"),
                        cursor,
                        source_register,
                        &mut source,
                        reconciliation.take(),
                    );
                    let mut progress = match progress {
                        Ok(progress) => progress,
                        Err(error) => {
                            restore_observed_operands(
                                &self.program,
                                candidate.as_mut().expect("activation fiber remains owned"),
                                cursor,
                                vec![(
                                    source_register,
                                    source.expect("failed result preflight retains observation"),
                                )],
                            );
                            return Err(error);
                        }
                    };
                    progress.pure_stats = Some(candidate_stats);
                    Ok(progress)
                }
                Some(_) => unreachable!("owned observation variants are exhaustive"),
                None => match step.exit {
                    VmExit::Running | VmExit::BudgetYield(_) => {
                        frame.phase = ProductDialoguePhase::Activating {
                            fiber: candidate.take().expect("activation fiber remains owned"),
                            pending: None,
                        };
                        Ok(ProductActivationProgress {
                            progressed: true,
                            presented: None,
                            reducer: LineTaskActivation::default(),
                            pure_stats: Some(candidate_stats),
                            execution,
                            host_calls: Vec::new(),
                            host_result_take: None,
                        })
                    }
                    VmExit::Returned(None) => {
                        frame.phase = ProductDialoguePhase::Activating {
                            fiber: candidate.take().expect("activation fiber remains owned"),
                            pending: None,
                        };
                        Ok(ProductActivationProgress {
                            progressed: true,
                            presented: None,
                            reducer: LineTaskActivation::default(),
                            pure_stats: Some(candidate_stats),
                            execution,
                            host_calls: Vec::new(),
                            host_result_take: None,
                        })
                    }
                    VmExit::Returned(Some(_)) => Err(LineRuntimeError::ResultNotCommitted.into()),
                    VmExit::DialogueResultSelected(_) => Err(ProductStepError::ActivationTrap(
                        crate::awbc::fiber::FiberTrap {
                            code: crate::awbc::schema::AwbcTrapCode::InternalInvariant,
                            message: Some(
                                "line activation selected a result outside its line-task child"
                                    .to_owned(),
                            ),
                            source_map: None,
                        },
                    )),
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
                            fiber: candidate.take().expect("activation fiber remains owned"),
                            pending: None,
                        };
                        Ok(ProductActivationProgress {
                            progressed: false,
                            presented: None,
                            reducer: LineTaskActivation::default(),
                            pure_stats: Some(candidate_stats),
                            execution: None,
                            host_calls: vec![request],
                            host_result_take: None,
                        })
                    }
                    VmExit::Suspended(reason) => Err(ProductStepError::Internal(format!(
                        "line activation suspended outside a supported host call: {reason:?}"
                    ))),
                },
            }
        })();
        if let Some(fiber) = candidate.take() {
            frame.phase = ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            };
        }
        result
    }

    fn resume_activation_host_call(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        pending: super::PendingHostCall,
        host_results: &mut Vec<crate::step::RuntimeHostCallResult>,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
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
                    if args.iter().any(|value| !value.ownership().permits_copy()) {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                    (resume, *destination, args.clone())
                };
                (fiber, resume, destination, args)
            }
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        };
        let Some(index) = host_results
            .iter()
            .position(|result| result.id == pending.id)
        else {
            let (_, request) =
                self.activation_host_call_request(pending.call, &args, Some(pending.clone()))?;
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: vec![request],
                host_result_take: None,
            });
        };
        let outcome = match &host_results[index].outcome {
            Err(error) => super::PreparedHostResultOutcome::Failed {
                kind: error.kind,
                message: error.message.clone(),
            },
            Ok(value) => {
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
                    Some(result_type) => {
                        runtime_value_matches_type(&self.program, value.value(), result_type, 0)
                    }
                    None => value.value() == &RuntimeValue::Unit,
                };
                if !valid || destination.is_some() && result_type.is_none() {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                }
                let prepared_resume = fiber.validate_resume_at(&self.program, resume)?;
                if let Some(destination) = destination {
                    let frame = fiber.active_frame()?;
                    if !frame
                        .registers
                        .get(destination.index())
                        .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                    {
                        return Err(LineRuntimeError::InvalidActivationOperation.into());
                    }
                }
                super::PreparedHostResultOutcome::Ready {
                    destination,
                    resume: prepared_resume,
                }
            }
        };
        Ok(ProductActivationProgress {
            progressed: true,
            presented: None,
            reducer: LineTaskActivation::default(),
            pure_stats: None,
            execution: None,
            host_calls: Vec::new(),
            host_result_take: Some(super::PreparedHostResultTake {
                result_id: pending.id,
                result_index: index,
                target: super::PreparedHostResultTarget::Activation {
                    activation,
                    call: pending.call,
                    outcome,
                },
            }),
        })
    }

    /// Final, owner-moving activation host-result publication. The caller has
    /// already inspected the complete dialogue transaction and command batch;
    /// no fallible check remains after the exact input row leaves the pool.
    pub(super) fn commit_activation_host_result(
        &mut self,
        transaction: &mut ProductDialogueTransaction,
        host_results: &mut Vec<crate::step::RuntimeHostCallResult>,
        prepared: super::PreparedHostResultTake,
    ) {
        let super::PreparedHostResultTake {
            result_id,
            result_index,
            target:
                super::PreparedHostResultTarget::Activation {
                    activation,
                    call,
                    outcome,
                },
        } = prepared
        else {
            unreachable!("activation commit requires an activation host-result ticket")
        };
        assert_eq!(transaction.activation(), &activation);
        let (frame, _) = transaction.parts_mut();
        let pending = frame
            .pending_activation_host_call
            .as_ref()
            .expect("prepared activation retains its pending host call");
        assert_eq!(pending.id, result_id);
        assert_eq!(pending.call, call);
        assert_eq!(host_results[result_index].id, result_id);
        let ProductDialoguePhase::Activating {
            fiber,
            pending: None,
        } = &mut frame.phase
        else {
            unreachable!("prepared activation retains its suspended fiber")
        };
        let result = host_results.remove(result_index);
        match (outcome, result.outcome) {
            (
                super::PreparedHostResultOutcome::Ready {
                    destination,
                    resume,
                },
                Ok(value),
            ) => {
                if let Some(destination) = destination {
                    fiber
                        .active_frame_mut()
                        .expect("prepared activation frame remains active")
                        .set_register(destination, value.into_value())
                        .expect("prepared activation destination remains available");
                }
                fiber.resume_at_prepared(resume);
            }
            (super::PreparedHostResultOutcome::Failed { kind, message }, Err(_)) => fiber
                .mark_trapped(crate::awbc::fiber::FiberTrap {
                    code: match kind {
                        crate::step::RuntimeHostCallErrorKind::UnsupportedCapability => {
                            crate::awbc::schema::AwbcTrapCode::CapabilityDenied
                        }
                        crate::step::RuntimeHostCallErrorKind::Rejected
                        | crate::step::RuntimeHostCallErrorKind::Failed => {
                            crate::awbc::schema::AwbcTrapCode::HostAbiMismatch
                        }
                    },
                    message: Some(message),
                    source_map: None,
                }),
            _ => unreachable!("prepared activation result retains its checked outcome"),
        }
        frame.pending_activation_host_call = None;
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
            crate::step::RuntimeHostCallRequest::admit(
                pending.id,
                host.producer,
                self.runtime_generation,
                crate::task::HostTaskRequest::Custom {
                    capability: crate::task::HostCapabilityId(
                        self.program.strings[host.capability.index()].clone(),
                    ),
                    operation: self.program.strings[host.operation.index()].clone(),
                    manifest_contract: host.contract,
                    args: positional,
                    named_args,
                },
                result_type.semantic_identity(),
                match host.mode {
                    crate::awbc::schema::AwbcHostCallMode::Immediate => {
                        crate::step::RuntimeHostCallMode::Immediate
                    }
                    crate::awbc::schema::AwbcHostCallMode::Suspend => {
                        crate::step::RuntimeHostCallMode::Suspend
                    }
                },
                host.deterministic,
                &mut self.need_producers,
            )
            .map_err(|error| ProductStepError::Host(error.to_string()))?,
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
        args: &mut Vec<VmLineOperationArgument>,
        reconciliation: Option<PreparedActivationFiberReconciliation>,
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
                line.record_commands(activation, commands)?;
                line.commit_ledger(ledger);
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
                let Some(reconciliation) = reconciliation else {
                    return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into());
                };
                let PreparedActivationFiberReconciliation {
                    mut ledger,
                    commands,
                } = reconciliation;
                let Some((
                    VmLineOperationArgument::OwnedValue {
                        value: RuntimeValue::Duration(delay),
                        ..
                    },
                    capture_args,
                )) = args.split_first()
                else {
                    return Err(LineRuntimeError::InvalidCueDelay.into());
                };
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
                let mut captured_tokens = BTreeSet::new();
                let mut captured_registers = BTreeSet::new();
                let mut capture_transfers = Vec::new();
                let mut capture_locals = Vec::with_capacity(captures.len());
                let observed_frame = fiber.active_frame()?.instance;
                for (ordinal, (capture, argument)) in captures.iter().zip(capture_args).enumerate()
                {
                    let VmLineOperationArgument::OwnedValue { register, value } = argument else {
                        return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into());
                    };
                    if !captured_registers.insert(*register)
                        || !runtime_value_matches_type(&self.program, value, capture.ty, 0)
                    {
                        return Err(LineRuntimeError::InvalidScheduledCaptureOwner.into());
                    }
                    capture_locals.push(capture.local);
                    let expected = RuntimeHandleOwnerSlot::ActivationLocal(
                        RuntimeOwnedSlotId::AwbcLineObservationArg {
                            execution: self.facade_fiber.execution,
                            fiber: fiber.instance,
                            frame: observed_frame,
                            site: cursor,
                            ordinal: u32::try_from(ordinal + 1)
                                .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?,
                        },
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
                let schedule =
                    line.inspect_schedule(&token, local_child, &work, deadline, &capture_locals)?;
                let destination_write =
                    fiber.validate_yielded_register_write(cursor, destination)?;
                let captured_values = captures
                    .into_iter()
                    .zip(std::mem::take(args).into_iter().skip(1))
                    .map(|(capture, argument)| {
                        let VmLineOperationArgument::OwnedValue { value, .. } = argument else {
                            unreachable!("preflighted Schedule capture remains owned")
                        };
                        RuntimeLocalBinding {
                            local: capture.local,
                            value,
                        }
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice();
                if let Some(commands) = commands {
                    line.record_commands_prepared(commands);
                }
                line.schedule_prepared(schedule, captured_values);
                fiber.commit_yielded_register_write_prepared(destination_write, value);
                line.commit_ledger(ledger);
                Ok(None)
            }
            AwbcLineOperation::ActorLook { character, .. } => {
                let [
                    VmLineOperationArgument::BorrowedRegister(actor_register),
                    VmLineOperationArgument::OwnedValue {
                        value: RuntimeValue::EntityRef(look),
                        ..
                    },
                    VmLineOperationArgument::OwnedValue {
                        value: RuntimeValue::Duration(crossfade),
                        ..
                    },
                ] = args.as_slice()
                else {
                    return Err(LineRuntimeError::InvalidCrossfade.into());
                };
                let RuntimeValue::Opaque(actor) =
                    fiber.active_frame()?.register(*actor_register)?
                else {
                    return Err(LineRuntimeError::WrongOpaqueProducer.into());
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
                line.record_commands(activation, commands)?;
                line.commit_ledger(ledger);
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
                    let destination_write =
                        fiber.validate_yielded_register_write(cursor, destination)?;
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
                    fiber.commit_yielded_register_write_prepared(destination_write, value);
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
        captures: &mut Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
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
        let mut capture_registers = BTreeSet::new();
        let mut deferred_tokens = BTreeSet::new();
        for ((register, value), expected) in captures.iter().zip(&signature.params) {
            if !capture_registers.insert(*register)
                || !crate::awbc::fiber::runtime_value_matches_type(
                    &self.program,
                    value,
                    *expected,
                    0,
                )
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            if !frame
                .registers
                .get(register.index())
                .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
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
        }
        let capture_values = captures.iter().map(|(_, value)| value).collect::<Vec<_>>();
        let prepared = line.inspect_deferred_registration_refs(&capture_values)?;
        let yielded = fiber.validate_yielded_instruction(cursor)?;
        fiber.commit_yielded_instruction_prepared(yielded);
        line.register_deferred_prepared(
            site,
            outcome,
            std::mem::take(captures)
                .into_iter()
                .map(|(_, value)| value)
                .collect(),
            prepared,
        );
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
        captures: &mut Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
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
        let mut seen_registers = BTreeSet::new();
        let capture_registers = captures
            .iter()
            .map(|(register, _)| *register)
            .collect::<Vec<_>>();
        let mut capture_tokens = BTreeSet::new();
        for ((register, value), expected) in captures.iter().zip(&signature.params) {
            if !seen_registers.insert(*register)
                || !frame
                    .registers
                    .get(register.index())
                    .is_some_and(crate::value::RuntimePlaceStorage::is_vacant)
                || !runtime_value_matches_type(&self.program, value, *expected, 0)
            {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let handles = unique_line_handles(value)?;
            if !value.ownership().permits_copy() && handles.is_empty() {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
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
        }
        let scope = fiber
            .active_frame()?
            .scopes
            .last()
            .filter(|scope| scope.id == scope_id)
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        if scope.defer_exit.is_some() || scope.defer_inflight.is_some() {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let capture_values = captures.iter().map(|(_, value)| value).collect::<Vec<_>>();
        let prepared = line.inspect_deferred_registration_refs(&capture_values)?;
        let yielded = fiber.validate_yielded_instruction(cursor)?;
        fiber.commit_yielded_instruction_prepared(yielded);
        let packet = line.commit_deferred_registration(
            site,
            outcome,
            std::mem::take(captures)
                .into_iter()
                .map(|(_, value)| value)
                .collect(),
            prepared,
        );
        let (id, site, outcome, captures) = packet.into_parts();
        let deferred = crate::awbc::fiber::FiberDeferredRegistration {
            id,
            site,
            outcome,
            capture_registers,
            captures,
        };
        let frame = fiber
            .active_frame_mut()
            .expect("prepared yielded instruction retained its active frame");
        let scope = frame
            .scopes
            .last_mut()
            .filter(|scope| scope.id == scope_id)
            .expect("borrowed scoped defer preflight retained its lexical scope");
        scope.defers.push(deferred);
        Ok(())
    }

    fn prepare_scoped_defer_unwind(
        &self,
        activation: &DialogueActivationId,
        owner_content: AwbcContentUnitId,
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
        let (handled_tokens, decision, prepared_spawn, content, frame_instance) = {
            let frame = fiber.active_frame()?;
            let frame_instance = frame.instance;
            let scope = frame
                .scopes
                .last()
                .filter(|scope| scope.id == scope_id)
                .ok_or(LineRuntimeError::InvalidActivationOperation)?;
            if scope.defer_inflight.is_some()
                || scope.defers.is_empty()
                || !scope.defer_releasing.is_empty()
            {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }
            let exit = scope.defer_exit.unwrap_or(requested_exit);
            let deferred = scope
                .defers
                .last()
                .expect("nonempty lexical defer stack was checked");
            let mut handled_tokens = BTreeSet::new();
            for capture in &deferred.captures {
                for handle in unique_line_handles(capture)? {
                    if !handled_tokens.insert(handle.token().clone()) {
                        return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                    }
                }
            }
            let (prepared_spawn, content) = if deferred.outcome.matches(exit) {
                let function = self
                    .program
                    .defer_sites
                    .get(deferred.site.index())
                    .copied()
                    .ok_or(LineRuntimeError::UnknownDeferredSite {
                        site: deferred.site,
                    })?;
                let prepared = batch.prepare_spawn(self, function, &deferred.captures)?;
                (Some(prepared), Some(owner_content))
            } else {
                (None, None)
            };
            let decision = line.prepare_scoped_deferred_parts(
                activation,
                deferred.id,
                deferred.outcome,
                &deferred.captures,
                exit,
            )?;
            (
                handled_tokens,
                decision,
                prepared_spawn,
                content,
                frame_instance,
            )
        };
        let scope = fiber
            .active_frame_mut()
            .expect("checked scoped defer retained its active frame")
            .scopes
            .last_mut()
            .filter(|scope| scope.id == scope_id)
            .expect("checked scoped defer retained its lexical scope");
        scope.defer_exit.get_or_insert(requested_exit);
        let deferred = scope
            .defers
            .pop()
            .expect("checked scoped defer retained its owner packet");
        let id = deferred.id;
        let site = deferred.site;
        match decision {
            RuntimeScopedDeferDecision::Skipped => {
                if !handled_tokens.is_empty() {
                    scope
                        .defer_releasing
                        .push(crate::awbc::fiber::FiberDeferredRelease {
                            registration: id,
                            site,
                            tokens: handled_tokens.iter().cloned().collect(),
                        });
                }
            }
            RuntimeScopedDeferDecision::Run => {
                let prepared = prepared_spawn.expect("Run child was preflighted above");
                let content = content.expect("Run content was preflighted above");
                batch.spawn_prepared(
                    self,
                    super::ProductChildFiberOwner::ScopedDeferred {
                        content,
                        activation: activation.clone(),
                        frame: frame_instance,
                        scope: scope_id,
                        registration: id,
                        site,
                    },
                    deferred.captures,
                    prepared,
                );
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
        self.resume_pending_line_operation_staged(transaction, outcomes)
    }

    fn resume_pending_line_operation_staged(
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
            _ if outcomes.is_empty() => return Ok(false),
            _ => return Err(LineRuntimeError::StaleCommandOutcome.into()),
        };
        let Some(operation) = pending.as_ref() else {
            return if outcomes.is_empty() {
                Ok(false)
            } else {
                Err(LineRuntimeError::UnknownCommandOutcome.into())
            };
        };
        if outcomes.is_empty() {
            return Ok(false);
        }
        let command = match operation {
            ProductPendingLineOperation::AcquireActor { command, .. }
            | ProductPendingLineOperation::ActorLook { command, .. }
            | ProductPendingLineOperation::StartVoice { command, .. } => command,
        };
        let mut pending_outcome = None;
        let mut unrelated = Vec::new();
        for outcome in outcomes {
            if outcome.command() == command {
                if pending_outcome.replace(outcome).is_some() {
                    return Err(LineRuntimeError::DuplicateCommandOutcome.into());
                }
            } else {
                unrelated.push(outcome.clone());
            }
        }
        let (mut stage, diagnostics) = line.stage_runtime_outcomes(&unrelated)?;
        if let Some(error) = diagnostics.into_iter().next() {
            return Err(error.into());
        }
        let Some(outcome) = pending_outcome else {
            line.commit_runtime_outcomes(stage);
            return Ok(false);
        };
        line.with_staged_outcomes(&stage, |staged| {
            require_pending_command(&activation, staged, command, outcome)
        })?;

        match (operation, outcome) {
            (
                ProductPendingLineOperation::AcquireActor {
                    cursor,
                    destination,
                    command,
                    token,
                    ..
                },
                RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Acquired {
                    actor, ..
                }),
            ) if actor == token => {
                let owner =
                    activation_register_owner(self.facade_fiber.execution, fiber, *destination)?;
                stage.consume_issued_command(command)?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Allocating,
                    RuntimeHandleLeaseState::Active,
                )?;
                stage.ledger_mut().transfer(
                    token,
                    &RuntimeHandleOwnerSlot::LineScope,
                    RuntimeHandleOwnerSlot::ActivationLocal(owner),
                )?;
                let write = fiber.validate_yielded_register_write(*cursor, *destination)?;
                let Some(ProductPendingLineOperation::AcquireActor { value, .. }) = pending.take()
                else {
                    unreachable!("preflight retained the pending AcquireActor owner packet")
                };
                line.commit_runtime_outcomes(stage);
                fiber.commit_yielded_register_write_prepared(write, value);
                Ok(true)
            }
            (
                ProductPendingLineOperation::ActorLook {
                    cursor,
                    destination,
                    command,
                    token,
                    ..
                },
                RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Accepted { cue, .. }),
            ) if cue == token => {
                let owner =
                    activation_register_owner(self.facade_fiber.execution, fiber, *destination)?;
                stage.consume_issued_command(command)?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Pending,
                    RuntimeHandleLeaseState::Running,
                )?;
                stage.ledger_mut().transfer(
                    token,
                    &RuntimeHandleOwnerSlot::LineScope,
                    RuntimeHandleOwnerSlot::ActivationLocal(owner),
                )?;
                let write = fiber.validate_yielded_register_write(*cursor, *destination)?;
                let Some(ProductPendingLineOperation::ActorLook { value, .. }) = pending.take()
                else {
                    unreachable!("preflight retained the pending ActorLook owner packet")
                };
                line.commit_runtime_outcomes(stage);
                fiber.commit_yielded_register_write_prepared(write, value);
                Ok(true)
            }
            (
                ProductPendingLineOperation::AcquireActor { command, token, .. },
                RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                    code, ..
                }),
            ) => {
                stage.consume_issued_command(command)?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Allocating,
                    RuntimeHandleLeaseState::Failed,
                )?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Failed,
                    RuntimeHandleLeaseState::Released,
                )?;
                line.commit_runtime_outcomes(stage);
                pending.take();
                Err(LineRuntimeError::StageCommandRejected { code: *code }.into())
            }
            (
                ProductPendingLineOperation::ActorLook { command, token, .. },
                RuntimeLineHostOutcome::Stage(RuntimeStageCommandOutcome::Rejected {
                    code, ..
                }),
            ) => {
                stage.consume_issued_command(command)?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Pending,
                    RuntimeHandleLeaseState::Failed,
                )?;
                stage.ledger_mut().set_state(
                    token,
                    RuntimeHandleLeaseState::Failed,
                    RuntimeHandleLeaseState::Released,
                )?;
                line.commit_runtime_outcomes(stage);
                pending.take();
                Err(LineRuntimeError::StageCommandRejected { code: *code }.into())
            }
            (
                ProductPendingLineOperation::StartVoice {
                    cursor,
                    destination,
                    command,
                    site,
                },
                RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Started {
                    session, ..
                }),
            ) => {
                let group = self
                    .dialogue_group(frame.content)
                    .ok_or(LineRuntimeError::MissingTaskGroup)?;
                let operation = AwbcLineOperation::VoiceHandle {
                    group,
                    site: *site,
                    result_type: self
                        .program
                        .line_task_groups
                        .get(group.index())
                        .and_then(|group| group.handle_sites.get(site.index()))
                        .map(|site| site.result_type)
                        .ok_or(LineRuntimeError::InvalidHandleSite)?,
                };
                let evidence = product_line_site_evidence(&self.program, group, &operation)?;
                let ordinal = stage.ledger().next_voice_lease_ordinal()?;
                let owner =
                    activation_register_owner(self.facade_fiber.execution, fiber, *destination)?;
                let write = fiber.validate_yielded_register_write(*cursor, *destination)?;
                stage.consume_issued_command(command)?;
                let value = RuntimeValue::Opaque(stage.ledger_mut().issue_exact(
                    &activation,
                    evidence.runtime_site,
                    RuntimeHandleKind::Voice,
                    &evidence.opaque_owner,
                    RuntimeHandleResource::Voice(RuntimeVoiceLease::new(
                        session.clone(),
                        ordinal,
                        true,
                    )),
                    RuntimeHandleOwnerSlot::ActivationLocal(owner),
                )?);
                line.commit_runtime_outcomes(stage);
                frame.voice = RuntimeDialogueVoiceState::Ready(session.clone());
                pending.take();
                fiber.commit_yielded_register_write_prepared(write, value);
                Ok(true)
            }
            (
                ProductPendingLineOperation::StartVoice { command, .. },
                RuntimeLineHostOutcome::Voice(RuntimeVoiceCommandOutcome::Rejected {
                    failure, ..
                }),
            ) => {
                stage.consume_issued_command(command)?;
                line.commit_runtime_outcomes(stage);
                frame.voice = RuntimeDialogueVoiceState::Failed(failure.clone());
                pending.take();
                Err(LineRuntimeError::VoiceStartRejected {
                    failure: failure.clone(),
                }
                .into())
            }
            _ => Err(LineRuntimeError::StageOutcomeMismatch.into()),
        }
    }

    fn commit_product_dialogue_result(
        &self,
        _activation: &DialogueActivationId,
        frame: &mut ActiveDialogue,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        fiber: &mut FiberState,
        cursor: FiberCursor,
        _source_register: crate::awbc::schema::AwbcRegisterId,
        source: &mut Option<RuntimeValue>,
        reconciliation: Option<PreparedActivationFiberReconciliation>,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let Some(reconciliation) = reconciliation else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        let PreparedActivationFiberReconciliation {
            mut ledger,
            commands,
        } = reconciliation;
        let source_value = source
            .as_ref()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        let group_id = self
            .dialogue_group(frame.content)
            .ok_or(LineRuntimeError::MissingTaskGroup)?;
        let group = self
            .program
            .line_task_groups
            .get(group_id.index())
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        if group.result_type != frame.result.ty
            || !runtime_value_matches_type(&self.program, source_value, group.result_type, 0)
        {
            return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
        }
        if !matches!(line.result(), RuntimeDialogueResultState::Uncommitted) {
            return Err(LineRuntimeError::ResultAlreadyCommitted.into());
        }
        let yielded = fiber.validate_yielded_instruction(cursor)?;
        let expected = RuntimeHandleOwnerSlot::ActivationLocal(
            RuntimeOwnedSlotId::AwbcDialogueResultObservation {
                execution: self.facade_fiber.execution,
                fiber: fiber.instance,
                frame: fiber.active_frame()?.instance,
                site: cursor,
            },
        );
        for handle in unique_line_handles(source_value)? {
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
        if let Some(commands) = commands {
            line.record_commands_prepared(commands);
        }
        line.commit_ledger(ledger);
        line.commit_result(
            group.result_type,
            source
                .take()
                .expect("preflighted result observation retains its value"),
        )
        .expect("result was preflighted as uncommitted");
        fiber.commit_yielded_instruction_prepared(yielded);
        Ok(ProductActivationProgress {
            progressed: true,
            presented: None,
            reducer: LineTaskActivation::default(),
            pure_stats: None,
            execution: None,
            host_calls: Vec::new(),
            host_result_take: None,
        })
    }

    fn resume_activation_out(
        &self,
        transaction: &mut ProductDialogueTransaction,
    ) -> Result<ProductActivationProgress, ProductStepError> {
        let activation = transaction.activation().clone();
        let (frame, line) = transaction.parts_mut();
        let owner_content = frame.content;
        let fiber = match &mut frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => fiber,
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        };
        if fiber_has_scoped_defer_inflight(fiber) {
            return Ok(ProductActivationProgress {
                progressed: false,
                presented: None,
                reducer: LineTaskActivation::default(),
                pure_stats: None,
                execution: None,
                host_calls: Vec::new(),
                host_result_take: None,
            });
        }
        if let Some(scope) = fiber.active_frame()?.scopes.last() {
            let scope_id = scope.id;
            let before_owners =
                activation_fiber_handle_owners(self.facade_fiber.execution, &activation, fiber)?;
            if !scope.defers.is_empty() {
                let mut batch = empty_activation_batch(self);
                let cursor = fiber.cursor;
                let handled = self.prepare_scoped_defer_unwind(
                    &activation,
                    owner_content,
                    line,
                    fiber,
                    cursor,
                    scope_id,
                    crate::line_task::ScopeExit::Completed,
                    &mut batch,
                )?;
                self.reconcile_activation_fiber_ownership(
                    &activation,
                    line,
                    &before_owners,
                    fiber,
                    None,
                    &Default::default(),
                    &BTreeSet::new(),
                    &handled,
                )?;
                return Ok(ProductActivationProgress {
                    progressed: true,
                    presented: None,
                    reducer: LineTaskActivation::default(),
                    pure_stats: Some(self.compact_pure_stats),
                    execution: Some(batch),
                    host_calls: Vec::new(),
                    host_result_take: None,
                });
            }
            if !scope.defer_releasing.is_empty() {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }

            let (cleanups, failure) = pop_activation_scope(&self.program, fiber, scope_id)?;
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
                &before_owners,
                fiber,
                None,
                &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                    crate::effect::RuntimeDropPolicy::Default,
                )),
                &BTreeSet::new(),
                &BTreeSet::new(),
            )?;
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
                host_result_take: None,
            });
        }

        if !fiber.active_frame()?.root_defers.is_empty() {
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
        let owner_content = frame.content;
        let fiber = match &mut frame.phase {
            ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::Activation { fiber, .. },
                ..
            }) => fiber,
            ProductDialoguePhase::Activating { .. }
            | ProductDialoguePhase::Reducing { .. }
            | ProductDialoguePhase::Publishing { .. }
            | ProductDialoguePhase::Transitioning
            | ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::LineTask { .. },
                ..
            }) => return Ok((false, None)),
        };
        let Some(scope) = fiber.active_frame()?.scopes.last() else {
            if !fiber.active_frame()?.root_defers.is_empty() {
                return Err(LineRuntimeError::InvalidDeferredTransition.into());
            }
            return Ok((false, None));
        };
        let scope_id = scope.id;
        let before_owners =
            activation_fiber_handle_owners(self.facade_fiber.execution, &activation, fiber)?;
        if scope.defer_inflight.is_some() {
            return Ok((true, None));
        }
        if !scope.defers.is_empty() {
            let exit = scope
                .defer_exit
                .unwrap_or(crate::line_task::ScopeExit::Failed);
            let cursor = fiber.cursor;
            let handled = self.prepare_scoped_defer_unwind(
                &activation,
                owner_content,
                line,
                fiber,
                cursor,
                scope_id,
                exit,
                batch,
            )?;
            self.reconcile_activation_fiber_ownership(
                &activation,
                line,
                &before_owners,
                fiber,
                None,
                &Default::default(),
                &BTreeSet::new(),
                &handled,
            )?;
            return Ok((true, None));
        }
        if !scope.defer_releasing.is_empty() {
            if line.has_pending_commands() {
                return Ok((true, None));
            }
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let (cleanups, failure) = pop_activation_scope(&self.program, fiber, scope_id)?;
        for cleanup in cleanups.into_iter().rev() {
            batch.observations.push(VmObservation::Effect {
                effect: cleanup.effect,
                args: cleanup.args,
            });
        }
        self.reconcile_activation_fiber_ownership(
            &activation,
            line,
            &before_owners,
            fiber,
            None,
            &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                crate::effect::RuntimeDropPolicy::Default,
            )),
            &BTreeSet::new(),
            &BTreeSet::new(),
        )?;
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
        let view = AwbcLineTaskPlanView::new(&self.program, group)
            .ok_or(LineRuntimeError::UnknownTaskGroup)?;
        match line.result() {
            RuntimeDialogueResultState::Committed { ty, value } => {
                if *ty != group.result_type
                    || *ty != frame.result.ty
                    || !runtime_value_matches_type(&self.program, value, *ty, 0)
                {
                    return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
                }
            }
            RuntimeDialogueResultState::Uncommitted => {
                if group.result_type != frame.result.ty {
                    return Err(LineRuntimeError::ResultPatternOrTypeMismatch.into());
                }
                if !view.has_mark_result_selector() {
                    return Err(LineRuntimeError::ResultNotCommitted.into());
                }
            }
            RuntimeDialogueResultState::Selected { .. }
            | RuntimeDialogueResultState::Publishing { .. }
            | RuntimeDialogueResultState::Published
            | RuntimeDialogueResultState::Abandoned => {
                return Err(LineRuntimeError::ResultNotCommitted.into());
            }
        }
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
        let exports = match &frame.phase {
            ProductDialoguePhase::Activating {
                fiber,
                pending: None,
            } => group
                .activation_exports
                .iter()
                .map(|export| {
                    let value = fiber.active_frame()?.register(export.register)?;
                    if !value.ownership().permits_copy() {
                        return Err(LineRuntimeError::AffineGroupCapture.into());
                    }
                    if !runtime_value_matches_type(&self.program, value, export.ty, 0) {
                        return Err(LineRuntimeError::InvalidScheduledCaptureGraph.into());
                    }
                    Ok(value.clone())
                })
                .collect::<Result<Vec<_>, ProductStepError>>()?,
            _ => return Err(LineRuntimeError::InvalidActivationOperation.into()),
        };
        let mut task_inputs = frame.captures.to_vec();
        task_inputs.extend(exports);
        frame.task_inputs = task_inputs.into_boxed_slice();
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
            host_result_take: None,
        })
    }

    fn reconcile_activation_fiber_ownership(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        before: &BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>,
        fiber: &FiberState,
        observation: Option<&VmObservation>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
        deferred_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        scoped_reconciled_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    ) -> Result<(), ProductStepError> {
        let prepared = self.inspect_activation_fiber_reconciliation(
            activation,
            line,
            before,
            fiber,
            observation,
            None,
            drops,
            deferred_tokens,
            scoped_reconciled_tokens,
        )?;
        Self::commit_activation_fiber_reconciliation(line, prepared);
        Ok(())
    }

    fn commit_activation_fiber_reconciliation(
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        prepared: PreparedActivationFiberReconciliation,
    ) {
        if let Some(commands) = prepared.commands {
            line.record_commands_prepared(commands);
        }
        line.commit_ledger(prepared.ledger);
    }

    fn inspect_activation_fiber_reconciliation(
        &self,
        activation: &DialogueActivationId,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        before: &BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>,
        fiber: &FiberState,
        observation: Option<&VmObservation>,
        effect_observations: Option<(
            FiberCursor,
            crate::runtime_id::RuntimeFrameInstanceId,
            &[VmObservation],
        )>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
        deferred_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        scoped_reconciled_tokens: &BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    ) -> Result<PreparedActivationFiberReconciliation, ProductStepError> {
        let mut after =
            activation_fiber_handle_owners(self.facade_fiber.execution, activation, fiber)?;
        if let Some(VmObservation::LineOperation { cursor, args, .. }) = observation {
            let frame = fiber.active_frame()?.instance;
            for (ordinal, argument) in args.iter().enumerate() {
                let VmLineOperationArgument::OwnedValue { value, .. } = argument else {
                    continue;
                };
                let owner = RuntimeHandleOwnerSlot::ActivationLocal(
                    RuntimeOwnedSlotId::AwbcLineObservationArg {
                        execution: self.facade_fiber.execution,
                        fiber: fiber.instance,
                        frame,
                        site: *cursor,
                        ordinal: u32::try_from(ordinal)
                            .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?,
                    },
                );
                for handle in unique_line_handles(value)? {
                    if handle.token().activation() != activation
                        || after
                            .insert(handle.token().clone(), owner.clone())
                            .is_some()
                    {
                        return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                    }
                }
            }
        }
        if let Some(VmObservation::DialogueResult { cursor, source, .. }) = observation {
            let owner = RuntimeHandleOwnerSlot::ActivationLocal(
                RuntimeOwnedSlotId::AwbcDialogueResultObservation {
                    execution: self.facade_fiber.execution,
                    fiber: fiber.instance,
                    frame: fiber.active_frame()?.instance,
                    site: *cursor,
                },
            );
            for handle in unique_line_handles(source)? {
                if handle.token().activation() != activation
                    || after
                        .insert(handle.token().clone(), owner.clone())
                        .is_some()
                {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
        }
        let mut effect_handles = Vec::new();
        if let Some((site, frame, effects)) = effect_observations {
            for (effect_ordinal, observation) in effects.iter().enumerate() {
                let VmObservation::Effect { args, .. } = observation else {
                    return Err(LineRuntimeError::InvalidActivationOperation.into());
                };
                let effect_ordinal = u32::try_from(effect_ordinal)
                    .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
                for (arg_ordinal, value) in args.iter().enumerate() {
                    let arg_ordinal = u32::try_from(arg_ordinal)
                        .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
                    let owner = RuntimeHandleOwnerSlot::ActivationLocal(
                        RuntimeOwnedSlotId::AwbcEffectObservationArg {
                            execution: self.facade_fiber.execution,
                            fiber: fiber.instance,
                            frame,
                            site,
                            effect_ordinal,
                            arg_ordinal,
                        },
                    );
                    for handle in unique_line_handles(value)? {
                        if handle.token().activation() != activation
                            || after
                                .insert(handle.token().clone(), owner.clone())
                                .is_some()
                        {
                            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                        }
                        effect_handles.push((handle.token().clone(), owner.clone()));
                    }
                }
            }
        }
        drops.validate_removed(
            |token| before.contains_key(token),
            |token| after.contains_key(token),
        )?;
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
                    let policy = drops
                        .policy_for(&token)
                        .ok_or(LineRuntimeError::UnjournaledHandleDrop)?;
                    let before_sequence = commands.next_sequence();
                    ledger.drop_owned_with_policy(&token, source, policy, &mut commands)?;
                    emitted_command |= commands.next_sequence() != before_sequence;
                }
                (None, Some(_)) => return Err(LineRuntimeError::UnknownHandle.into()),
                (Some(_), Some(_)) | (None, None) => {}
            }
        }
        for (token, owner) in effect_handles {
            let before_sequence = commands.next_sequence();
            ledger.drop_owned_with_policy(
                &token,
                &owner,
                crate::effect::RuntimeDropPolicy::Default,
                &mut commands,
            )?;
            emitted_command |= commands.next_sequence() != before_sequence;
        }
        let commands = emitted_command
            .then(|| line.inspect_record_commands(activation, &commands))
            .transpose()?;
        Ok(PreparedActivationFiberReconciliation { ledger, commands })
    }
}

impl super::ProductLineTaskExecutionBatch {
    fn prepare_spawn(
        &self,
        executor: &super::AwbcProductStepExecutor,
        function: crate::awbc::schema::AwbcFunctionId,
        args: &[RuntimeValue],
    ) -> Result<PreparedLineChildSpawn, ProductStepError> {
        let next_generation = self
            .next_generation
            .checked_add(1)
            .ok_or(ProductStepError::ChildGenerationOverflow)?;
        let mut next_fiber_instance = self.next_fiber_instance;
        let fiber_instance = next_fiber_instance
            .take_next(crate::runtime_id::RuntimeIdNamespace::FiberInstance)
            .map(crate::runtime_id::RuntimeFiberInstanceId::from_allocated)?;
        let inputs = crate::awbc::fiber::validate_function_argument_values(
            &executor.program,
            function,
            args,
        )
        .map_err(|error| ProductStepError::Type(error.to_string()))?;
        Ok(PreparedLineChildSpawn {
            child_generation: self.next_generation,
            next_generation,
            next_fiber_instance,
            fiber_instance,
            inputs,
        })
    }

    fn spawn_prepared(
        &mut self,
        executor: &super::AwbcProductStepExecutor,
        owner: super::ProductChildFiberOwner,
        args: Vec<RuntimeValue>,
        prepared: PreparedLineChildSpawn,
    ) {
        let child = FiberState::for_function_with_arguments_prepared(
            &executor.program,
            crate::awbc::fiber::AwbcFiberRoot::Function(prepared.inputs.function()),
            args,
            prepared.inputs,
            prepared.fiber_instance,
            prepared.child_generation,
            executor.fiber.budget.quantum.max(1),
        );
        self.next_generation = prepared.next_generation;
        self.next_fiber_instance = prepared.next_fiber_instance;
        self.child_fibers.push_back(super::ProductChildFiber {
            owner,
            fiber: child,
            runtime_generation: executor.runtime_generation,
            pending_host_call: None,
        });
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

fn awbc_formatter_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
    frame: crate::runtime_id::RuntimeFrameInstanceId,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>, ProductStepError> {
    let mut owners = BTreeMap::new();
    fiber.visit_formatter_operand_values(
        |value_frame, site, ordinal, value| -> Result<(), ProductStepError> {
            if value_frame != frame {
                return Ok(());
            }
            let ordinal =
                u32::try_from(ordinal).map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
            let owner = RuntimeOwnedSlotId::AwbcFormatOperand {
                execution,
                fiber: fiber.instance,
                frame: value_frame,
                site,
                ordinal,
            };
            for handle in unique_line_handles(value)? {
                if owners.insert(handle.token().clone(), owner).is_some() {
                    return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
                }
            }
            Ok(())
        },
    )?;
    Ok(owners)
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
        | ProductDialoguePhase::Transitioning
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
        child_fibers: VecDeque::new(),
        existing_child_actions: BTreeMap::new(),
        line_task_activations: Vec::new(),
        line_task_baseline: None,
        line_task_reserved_runs: Vec::new(),
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
            *register = Default::default();
        }
    }
    Ok((scope.cleanups, scope.defer_failure))
}

fn restore_observed_operands(
    program: &crate::awbc::schema::AwbcProgram,
    fiber: &mut FiberState,
    cursor: FiberCursor,
    operands: Vec<(crate::awbc::schema::AwbcRegisterId, RuntimeValue)>,
) {
    if operands.is_empty() {
        return;
    }
    let borrowed = operands
        .iter()
        .map(|(register, value)| (*register, value))
        .collect::<Vec<_>>();
    let prepared = fiber
        .inspect_yielded_operand_restore(program, cursor, &borrowed)
        .expect("VM yielded observation retains its exact vacant operand registers");
    fiber.restore_yielded_operands_prepared(program, prepared, operands);
}

fn restore_owned_vm_observation(
    program: &crate::awbc::schema::AwbcProgram,
    fiber: &mut FiberState,
    observation: VmObservation,
) {
    match observation {
        VmObservation::LineOperation { cursor, args, .. } => {
            let operands = args
                .into_iter()
                .filter_map(|argument| match argument {
                    VmLineOperationArgument::OwnedValue { register, value } => {
                        Some((register, value))
                    }
                    VmLineOperationArgument::BorrowedRegister(_) => None,
                })
                .collect();
            restore_observed_operands(program, fiber, cursor, operands);
        }
        VmObservation::DialogueResult {
            cursor,
            source_register,
            source,
        } => restore_observed_operands(program, fiber, cursor, vec![(source_register, source)]),
        _ => {}
    }
}

fn activation_fiber_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    activation: &DialogueActivationId,
    fiber: &FiberState,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeHandleOwnerSlot>, ProductStepError> {
    let mut owners = BTreeMap::new();
    for frame in &fiber.frames {
        for (index, value) in frame
            .registers
            .iter()
            .enumerate()
            .flat_map(|(index, storage)| storage.values().map(move |value| (index, value)))
        {
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
        for (token, owner) in awbc_formatter_handle_owners(execution, fiber, frame.instance)? {
            if token.activation() != activation {
                return Err(LineRuntimeError::WrongActivation.into());
            }
            if owners
                .insert(token, RuntimeHandleOwnerSlot::ActivationLocal(owner))
                .is_some()
            {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
        for (token, owner) in awbc_cleanup_handle_owners(execution, fiber, frame)? {
            if token.activation() != activation
                || owners
                    .insert(token, RuntimeHandleOwnerSlot::ActivationLocal(owner))
                    .is_some()
            {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
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
        for (index, value) in frame
            .registers
            .iter()
            .enumerate()
            .flat_map(|(index, storage)| storage.values().map(move |value| (index, value)))
        {
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
        for (token, owner) in awbc_formatter_handle_owners(execution, fiber, frame.instance)? {
            if token.activation() == activation && owners.insert(token, owner).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
        for (token, owner) in awbc_cleanup_handle_owners(execution, fiber, frame)? {
            if token.activation() == activation && owners.insert(token, owner).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
    }
    for (token, owner) in awbc_await_many_handle_owners(execution, fiber)? {
        if token.activation() == activation && owners.insert(token, owner).is_some() {
            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
        }
    }
    Ok(owners)
}

pub(super) fn product_fiber_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
) -> Result<BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>, ProductStepError> {
    let mut owners = BTreeMap::new();
    if let Some(FiberTerminalValue::Returned(Some(value))) = &fiber.terminal
        && matches!(fiber.root, crate::awbc::fiber::AwbcFiberRoot::Program(_))
    {
        let owner = RuntimeOwnedSlotId::ProgramResult {
            execution,
            fiber: crate::runtime_id::RuntimePersistentFiberId::from_allocated(
                fiber.instance.get().get(),
            ),
        };
        for handle in unique_line_handles(value)? {
            if owners.insert(handle.token().clone(), owner).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
    }
    for frame in &fiber.frames {
        for (index, value) in frame
            .registers
            .iter()
            .enumerate()
            .flat_map(|(index, storage)| storage.values().map(move |value| (index, value)))
        {
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
        for (token, owner) in awbc_formatter_handle_owners(execution, fiber, frame.instance)? {
            if owners.insert(token, owner).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
        for (token, owner) in awbc_cleanup_handle_owners(execution, fiber, frame)? {
            if owners.insert(token, owner).is_some() {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
    }
    for (token, owner) in awbc_await_many_handle_owners(execution, fiber)? {
        if owners.insert(token, owner).is_some() {
            return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
        }
    }
    Ok(owners)
}

/// Predicts the parent handle graph after one AwaitMany suspension releases
/// its item and result slots. Pattern-bound result handles can then be added
/// at their checked destination registers before ledger preflight.
pub(super) fn without_await_many_slots(
    before: &BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>,
    fiber: RuntimeFiberInstanceId,
    plan: crate::awbc::schema::AwbcTaskPlanId,
) -> BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId> {
    let mut after = before.clone();
    after.retain(|_, owner| {
        !matches!(
            owner,
            RuntimeOwnedSlotId::AwbcAwaitManyItem { fiber: owner_fiber, plan: owner_plan, .. }
                | RuntimeOwnedSlotId::AwbcAwaitManyResult { fiber: owner_fiber, plan: owner_plan, .. }
                if *owner_fiber == fiber && *owner_plan == plan
        )
    });
    after
}

fn awbc_await_many_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
) -> Result<Vec<(RuntimeLineHandleToken, RuntimeOwnedSlotId)>, ProductStepError> {
    let Some(crate::awbc::fiber::FiberSuspensionReason::AwaitMany(state)) = fiber
        .suspension
        .as_ref()
        .map(|suspension| &suspension.reason)
    else {
        return Ok(Vec::new());
    };
    let frame = fiber.active_frame()?.instance;
    let mut owners = Vec::new();
    for (index, value) in state.items.iter().enumerate() {
        let index = u32::try_from(index).map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
        let owner = RuntimeOwnedSlotId::AwbcAwaitManyItem {
            execution,
            fiber: fiber.instance,
            frame,
            plan: state.plan,
            index,
        };
        owners.extend(
            unique_line_handles(value)?
                .into_iter()
                .map(|handle| (handle.token().clone(), owner)),
        );
    }
    for (index, value) in state.results.iter().enumerate() {
        let Some(value) = value else {
            continue;
        };
        let index = u32::try_from(index).map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
        let owner = RuntimeOwnedSlotId::AwbcAwaitManyResult {
            execution,
            fiber: fiber.instance,
            frame,
            plan: state.plan,
            index,
        };
        owners.extend(
            unique_line_handles(value)?
                .into_iter()
                .map(|handle| (handle.token().clone(), owner)),
        );
    }
    Ok(owners)
}

fn awbc_cleanup_handle_owners(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
    frame: &crate::awbc::fiber::FiberFrame,
) -> Result<Vec<(RuntimeLineHandleToken, RuntimeOwnedSlotId)>, ProductStepError> {
    let mut owners = Vec::new();
    for (scope, cleanups) in std::iter::once((None, frame.root_cleanups.as_slice())).chain(
        frame
            .scopes
            .iter()
            .map(|scope| (Some(scope.id), scope.cleanups.as_slice())),
    ) {
        for (cleanup_ordinal, cleanup) in cleanups.iter().enumerate() {
            let cleanup_ordinal =
                u32::try_from(cleanup_ordinal).map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
            for (arg_ordinal, value) in cleanup.args.iter().enumerate() {
                let arg_ordinal =
                    u32::try_from(arg_ordinal).map_err(|_| LineRuntimeError::OwnedSlotOverflow)?;
                let owner = RuntimeOwnedSlotId::AwbcCleanupArg {
                    execution,
                    fiber: fiber.instance,
                    frame: frame.instance,
                    scope,
                    cleanup_ordinal,
                    arg_ordinal,
                };
                owners.extend(
                    unique_line_handles(value)?
                        .into_iter()
                        .map(|handle| (handle.token().clone(), owner)),
                );
            }
        }
    }
    Ok(owners)
}

pub(super) fn product_fiber_handle_tokens(
    execution: crate::runtime_id::ExecutionInstanceId,
    fiber: &FiberState,
) -> Result<BTreeSet<RuntimeLineHandleToken>, ProductStepError> {
    let mut tokens = product_fiber_handle_owners(execution, fiber)?
        .into_keys()
        .collect::<BTreeSet<_>>();
    if let Some(FiberTerminalValue::DialogueResultSelected(value)) = fiber.terminal.as_ref() {
        for handle in unique_line_handles(value)? {
            if !tokens.insert(handle.token().clone()) {
                return Err(LineRuntimeError::DuplicateHandleOccurrence.into());
            }
        }
    }
    Ok(tokens)
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
