use super::{ActiveDialogue, AwbcProductStepExecutor, ProductStepError};
use crate::awbc::fiber::{FiberState, FiberStateError, FiberSuspensionReason};
use crate::awbc::schema::{AwbcResumePointId, AwbcTypeId};
use crate::effect::{RuntimeRegistryOwner, RuntimeScopeRegistry};
use crate::line_task::{
    LineRuntimeError, PreparedRuntimeParentFiberReconciliation,
    RuntimeDialogueAbandonedCommitProof, RuntimeDialogueActivationRegistry,
    RuntimeDialogueActivationState, RuntimeDialogueActivationTransaction,
    RuntimeDialogueCommitProof, RuntimeDialoguePublishedCommitProof,
    RuntimeDialogueRegistryCommitReceipt, RuntimeLineHandleLedger,
};
use crate::runtime_id::{DialogueActivationId, RuntimeLineHandleToken};
use crate::step::RuntimeDialogueContentEvent;
use crate::task::RuntimeProgramOwner;
use crate::time::LogicalDuration;
use crate::value::{RuntimeValue, RuntimeValueView, ownership::RuntimeOwnedSlotId};
use arcweft_character::id::CharacterId;
use std::collections::{BTreeMap, BTreeSet};

mod receiver;
pub(super) use receiver::ProductDialogueReceivers;

/// Product adapter over the executor-neutral dialogue registry. Product owns
/// only ingress readiness and its AWBC frame payload; Active/PublishedHandles,
/// revisions, ledger/command transactions, publication, and parent drops are
/// shared with structured execution.
#[derive(Debug, Default, PartialEq)]
pub(super) struct ProductDialogueStore {
    registry: RuntimeDialogueActivationRegistry<ActiveDialogue, AwbcTypeId>,
}

pub(super) type ProductDialogueTransaction =
    RuntimeDialogueActivationTransaction<ActiveDialogue, AwbcTypeId>;

#[derive(Clone, Copy)]
pub(super) enum ProductDialogueReceiver {
    Main,
    Activation(DialogueActivationId),
}

#[derive(Clone, Copy)]
pub(super) enum ProductDialogueDispatch {
    Start(ProductDialogueReceiver),
    Resume {
        activation: DialogueActivationId,
        resume: AwbcResumePointId,
    },
}

impl ProductDialogueDispatch {
    pub(super) fn foreground(self) -> Option<DialogueActivationId> {
        match self {
            Self::Start(ProductDialogueReceiver::Main) => None,
            Self::Start(ProductDialogueReceiver::Activation(activation))
            | Self::Resume { activation, .. } => Some(activation),
        }
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn dialogue_dispatch(
        &self,
    ) -> Result<Option<ProductDialogueDispatch>, FiberStateError> {
        let mut owner = ProductDialogueReceiver::Main;
        let mut receiver = &self.fiber;
        loop {
            let Some(suspension) = receiver.suspension.as_ref() else {
                return Ok(None);
            };
            match suspension.reason {
                FiberSuspensionReason::Dialogue { .. } => {
                    return Ok(Some(ProductDialogueDispatch::Start(owner)));
                }
                FiberSuspensionReason::DialogueActive { activation } => {
                    let invalid = || FiberStateError::InvalidDialogueReceiver { activation };
                    let active = self
                        .dialogues
                        .active_frame_for(&activation)
                        .ok_or_else(invalid)?;
                    let resume = suspension.declared_resume().ok_or_else(invalid)?;
                    if let Some(fiber) = active.receiving_fiber()
                        && fiber.suspension.as_ref().is_some_and(|suspension| {
                            matches!(
                                suspension.reason,
                                FiberSuspensionReason::Dialogue { .. }
                                    | FiberSuspensionReason::DialogueActive { .. }
                            )
                        })
                    {
                        if fiber.instance.get() <= receiver.instance.get() {
                            return Err(invalid());
                        }
                        owner = ProductDialogueReceiver::Activation(activation);
                        receiver = fiber;
                    } else {
                        return Ok(Some(ProductDialogueDispatch::Resume { activation, resume }));
                    }
                }
                _ => return Ok(None),
            }
        }
    }

    pub(super) fn foreground_dialogue(&self) -> Option<DialogueActivationId> {
        self.dialogue_dispatch()
            .ok()
            .flatten()
            .and_then(ProductDialogueDispatch::foreground)
    }

    pub(super) fn dialogue_receiver(
        &self,
        child: DialogueActivationId,
    ) -> Result<ProductDialogueReceiver, FiberStateError> {
        if self.fiber.dialogue_receiver() == Some(child) {
            return Ok(ProductDialogueReceiver::Main);
        }
        self.dialogues
            .active_frames()
            .find_map(|frame| {
                let fiber = frame.receiving_fiber()?;
                (fiber.dialogue_receiver() == Some(child)
                    && fiber.instance.get().get() == child.owner_fiber().get())
                .then_some(ProductDialogueReceiver::Activation(frame.activation))
            })
            .ok_or(FiberStateError::InvalidDialogueReceiver { activation: child })
    }

    pub(super) fn dialogue_receiver_fiber(
        &self,
        receiver: ProductDialogueReceiver,
    ) -> Option<&FiberState> {
        match receiver {
            ProductDialogueReceiver::Main => Some(&self.fiber),
            ProductDialogueReceiver::Activation(id) => {
                self.dialogues.active_frame_for(&id)?.receiving_fiber()
            }
        }
    }
}

struct ReachedProductFlow<'a> {
    owner: RuntimeRegistryOwner,
    rows: &'a RuntimeScopeRegistry,
    activation_frame: Option<DialogueActivationId>,
}

fn reached_flow<'a>(
    program: &crate::awbc::schema::AwbcProgram,
    activation: DialogueActivationId,
    dialogues: &'a ProductDialogueStore,
    session: impl Iterator<Item = &'a FiberState> + Clone,
) -> Result<ReachedProductFlow<'a>, FiberStateError> {
    let invalid = || FiberStateError::InvalidDialogueReceiver { activation };
    let mut owner = activation.owner_fiber();
    loop {
        let active = dialogues.active_frames().find(|frame| {
            frame
                .activation_fiber()
                .is_some_and(|fiber| fiber.instance.get().get() == owner.get())
        });
        let fiber = match active {
            Some(frame) => frame.activation_fiber(),
            None => session
                .clone()
                .find(|fiber| fiber.instance.get().get() == owner.get()),
        }
        .ok_or_else(invalid)?;
        if let Some(index) = fiber.reached_flow_frame_index(program) {
            let frame = &fiber.frames[index];
            return Ok(ReachedProductFlow {
                owner: RuntimeRegistryOwner::AwbcFlow {
                    fiber: fiber.instance,
                    frame: frame.instance,
                },
                rows: &frame.flow_registry,
                activation_frame: active.map(|frame| frame.activation),
            });
        }
        let ancestor = active
            .ok_or(FiberStateError::InvalidFrame)?
            .activation
            .owner_fiber();
        if ancestor.get() >= owner.get() {
            return Err(invalid());
        }
        owner = ancestor;
    }
}

pub(super) fn reached_registry<'a>(
    program: &crate::awbc::schema::AwbcProgram,
    activation: DialogueActivationId,
    scope: &arcweft_id::LifetimeScopeKind,
    line: &'a RuntimeDialogueActivationState<AwbcTypeId>,
    dialogues: &'a ProductDialogueStore,
    parents: impl Iterator<Item = &'a FiberState> + Clone,
) -> Result<(RuntimeRegistryOwner, &'a RuntimeScopeRegistry), FiberStateError> {
    use arcweft_id::LifetimeScopeKind;
    match scope {
        LifetimeScopeKind::Line => Ok((
            RuntimeRegistryOwner::Dialogue(activation),
            line.line_registry()
                .map_err(|_| FiberStateError::InvalidFrame)?,
        )),
        LifetimeScopeKind::Flow => {
            let reached = reached_flow(program, activation, dialogues, parents)?;
            Ok((reached.owner, reached.rows))
        }
        _ => Err(FiberStateError::InvalidFrame),
    }
}

impl AwbcProductStepExecutor {
    pub(super) fn reserve_reached_registry_mutation(
        &mut self,
        activation: DialogueActivationId,
        owner: RuntimeRegistryOwner,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        write: &crate::effect::RuntimePreparedRegistryMutation,
    ) -> Result<(), ProductStepError> {
        self.edit_reached_registry(activation, owner, line, false, |rows| {
            write.reserve(rows).map_err(FiberStateError::Snapshot)
        })
    }

    pub(super) fn commit_reached_registry_mutation(
        &mut self,
        activation: DialogueActivationId,
        owner: RuntimeRegistryOwner,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        write: crate::effect::RuntimePreparedRegistryMutation,
    ) {
        self.edit_reached_registry(activation, owner, line, true, |rows| {
            write.commit(rows).map_err(FiberStateError::Snapshot)
        })
        .expect("prepared mutation retains its reached registry");
    }

    fn edit_reached_registry(
        &mut self,
        activation: DialogueActivationId,
        owner: RuntimeRegistryOwner,
        line: &mut RuntimeDialogueActivationState<AwbcTypeId>,
        publish: bool,
        edit: impl FnOnce(&mut RuntimeScopeRegistry) -> Result<(), FiberStateError>,
    ) -> Result<(), ProductStepError> {
        if owner == RuntimeRegistryOwner::Dialogue(activation) {
            return Ok(edit(line.line_registry_mut()?)?);
        }
        let RuntimeRegistryOwner::AwbcFlow { fiber, frame } = owner else {
            return Err(FiberStateError::InvalidFrame.into());
        };
        let reached = reached_flow(
            &self.program,
            activation,
            &self.dialogues,
            std::iter::once(&self.fiber).chain(self.child_fibers.iter().map(|child| &child.fiber)),
        )?;
        if reached.owner != owner {
            return Err(FiberStateError::InvalidFrame.into());
        }
        let invalid = || FiberStateError::InvalidDialogueReceiver { activation };
        let edit_fiber = |parent: &mut FiberState| {
            let index = parent
                .reached_flow_frame_index(&self.program)
                .ok_or_else(invalid)?;
            let reached = &mut parent.frames[index];
            if reached.instance != frame {
                return Err(FiberStateError::InvalidFrame);
            }
            edit(&mut reached.flow_registry)
        };
        if let Some(id) = reached.activation_frame {
            self.dialogues
                .preflight_receiver_edit(ProductDialogueReceiver::Activation(id))?;
            let mut parent = self.dialogues.begin_transaction(&id)?;
            let result = parent
                .frame_mut()
                .receiving_fiber_mut()
                .ok_or_else(invalid)
                .and_then(edit_fiber);
            if publish && result.is_ok() {
                self.dialogues.commit_receiver(parent);
            } else {
                self.dialogues.restore_transaction(parent)?;
            }
            return Ok(result?);
        }
        let parent = std::iter::once(&mut self.fiber)
            .chain(self.child_fibers.iter_mut().map(|child| &mut child.fiber))
            .find(|parent| parent.instance == fiber)
            .ok_or_else(invalid)?;
        Ok(edit_fiber(parent)?)
    }
}
impl ProductDialogueStore {
    pub(super) fn into_published(
        self,
    ) -> Result<crate::line_task::RuntimePublishedDialogueRegistry, (Self, LineRuntimeError)> {
        self.registry
            .into_published()
            .map_err(|(registry, reason)| (Self { registry }, reason))
    }
    pub(super) fn from_published(
        custody: crate::line_task::RuntimePublishedDialogueRegistry,
    ) -> Self {
        Self {
            registry: custody.into_registry(),
        }
    }

    pub(super) fn begin(&mut self, frame: ActiveDialogue) -> Result<(), LineRuntimeError> {
        if self.registry.has_active_or_inflight() {
            return Err(LineRuntimeError::DuplicateActivationLedger);
        }
        self.registry.begin(frame.activation, frame)
    }

    pub(super) fn begin_nested(
        &mut self,
        parent: &ProductDialogueTransaction,
        frame: ActiveDialogue,
    ) -> Result<(), LineRuntimeError> {
        self.registry.inspect_commit(parent)?;
        let fiber = parent
            .frame()
            .activation_fiber()
            .ok_or(LineRuntimeError::InvalidActivationOperation)?;
        if frame.activation.owner_fiber().get() != fiber.instance.get().get() {
            return Err(LineRuntimeError::InvalidActivationOperation);
        }
        self.registry.begin(frame.activation, frame)
    }

    pub(super) fn preflight_nested_start(&self) -> Result<(), LineRuntimeError> {
        let depth = self.active_frames().count().saturating_add(1);
        if !crate::entry::RuntimeSchemaLimits::engine_default().permits_depth(depth) {
            return Err(LineRuntimeError::InvalidActivationOperation);
        }
        Ok(())
    }

    pub(super) fn active_frames(&self) -> impl Iterator<Item = &ActiveDialogue> {
        self.registry.active_frames()
    }

    pub(super) fn active_frame_for(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&ActiveDialogue> {
        self.registry.active_frame(activation)
    }

    pub(super) fn preflight_receiver_edit(
        &self,
        receiver: ProductDialogueReceiver,
    ) -> Result<(), LineRuntimeError> {
        if let ProductDialogueReceiver::Activation(id) = receiver {
            self.registry
                .active_revision(&id)
                .ok_or(LineRuntimeError::UnknownActivationLedger)?
                .checked_add(1)
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn active_activation(&self) -> Option<DialogueActivationId> {
        self.registry.active_ids().into_iter().next()
    }

    #[cfg(test)]
    pub(super) fn active_frame(&self) -> Option<&ActiveDialogue> {
        self.active_activation()
            .as_ref()
            .and_then(|activation| self.registry.active_frame(activation))
    }

    #[cfg(test)]
    pub(super) fn active_line(
        &self,
    ) -> Option<&crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>> {
        self.active_activation()
            .as_ref()
            .and_then(|activation| self.active_line_for(activation))
    }

    pub(super) fn active_line_for(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>> {
        self.registry.active_line(activation)
    }

    /// Borrows the executor's actual phase and the same command/defer ledger
    /// consumed by activation. No transported or cached readiness is admitted.
    pub(super) fn has_internal_work_for(
        &self,
        activation: &DialogueActivationId,
        joined_work: bool,
    ) -> bool {
        let Some(frame) = self.active_frame_for(activation) else {
            return false;
        };
        let Some(line) = self.active_line_for(activation) else {
            return false;
        };
        if line.has_pending_commands() || line.deferred_inflight().is_some() {
            return false;
        }
        match &frame.phase {
            super::ProductDialoguePhase::Activating { fiber, pending } => {
                pending.is_none()
                    && frame.pending_activation_host_call.is_none()
                    && !self.activation_budget_exhausted(activation)
                    && !matches!(
                        fiber
                            .suspension
                            .as_ref()
                            .map(|suspension| &suspension.reason),
                        Some(crate::awbc::fiber::FiberSuspensionReason::DurationWait(_))
                    )
                    && !super::line::fiber_has_scoped_defer_inflight(fiber)
            }
            super::ProductDialoguePhase::Reducing { line_task }
            | super::ProductDialoguePhase::Publishing { line_task } => {
                !joined_work && line_task.is_closed()
            }
            super::ProductDialoguePhase::Closing(closing) => {
                !joined_work
                    && match &closing.state {
                        super::ProductDialogueClosingState::Activation { fiber, .. } => {
                            !super::line::fiber_has_scoped_defer_inflight(fiber)
                        }
                        super::ProductDialogueClosingState::LineTask { line_task } => {
                            line_task.is_closed()
                        }
                    }
            }
            super::ProductDialoguePhase::Transitioning => false,
        }
    }

    #[cfg(test)]
    pub(super) fn has_internal_work(&self, joined_work: bool) -> bool {
        self.active_activation()
            .is_some_and(|activation| self.has_internal_work_for(&activation, joined_work))
    }

    pub(super) fn activation_budget_exhausted(&self, activation: &DialogueActivationId) -> bool {
        self.active_frame_for(activation).is_some_and(|frame| {
            matches!(
                &frame.phase,
                super::ProductDialoguePhase::Activating { fiber, .. }
                    if fiber.status == crate::awbc::fiber::FiberStatus::Suspended
                        && matches!(
                            fiber.suspension.as_ref().map(|suspension| &suspension.reason),
                            Some(crate::awbc::fiber::FiberSuspensionReason::BudgetYield)
                        )
            )
        })
    }

    pub(super) fn visit_runtime_values<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        self.registry.try_visit_active(|_, frame, line| {
            visit_value_graph(frame.target.payload(), &mut visitor)?;
            visit_value_slice(&frame.captures, &mut visitor)?;
            visit_value_slice(&frame.task_inputs, &mut visitor)?;
            for binding in frame.values.iter() {
                visit_value_graph(&binding.value, &mut visitor)?;
            }
            for callback in frame.effect_callbacks.values() {
                visit_value_slice(callback.retained(), &mut visitor)?;
            }
            match &frame.phase {
                super::ProductDialoguePhase::Activating { fiber, pending } => {
                    fiber.visit_runtime_values(&mut visitor)?;
                    visit_pending_line_operation(pending.as_ref(), &mut visitor)?;
                }
                super::ProductDialoguePhase::Reducing { .. }
                | super::ProductDialoguePhase::Publishing { .. } => {}
                super::ProductDialoguePhase::Transitioning => {
                    unreachable!("transient dialogue phase cannot be inspected")
                }
                super::ProductDialoguePhase::Closing(closing) => match &closing.state {
                    super::ProductDialogueClosingState::Activation { fiber, pending } => {
                        fiber.visit_runtime_values(&mut visitor)?;
                        visit_pending_line_operation(pending.as_ref(), &mut visitor)?;
                    }
                    super::ProductDialogueClosingState::LineTask { .. } => {}
                },
            }
            line.visit_runtime_values(&mut visitor)?;
            Ok(())
        })
    }

    #[cfg(test)]
    pub(super) fn begin_active_transaction(
        &mut self,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        let activation = self
            .active_activation()
            .ok_or(LineRuntimeError::UnknownActivationLedger)?;
        self.registry.begin_transaction(&activation)
    }

    pub(super) fn begin_transaction(
        &mut self,
        activation: &DialogueActivationId,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        self.registry.begin_transaction(activation)
    }

    pub(super) fn restore_transaction(
        &mut self,
        transaction: ProductDialogueTransaction,
    ) -> Result<(), LineRuntimeError> {
        self.registry.restore_transaction(transaction)
    }

    pub(super) fn inspect_commit(
        &self,
        transaction: &ProductDialogueTransaction,
    ) -> Result<RuntimeDialogueCommitProof, LineRuntimeError> {
        self.registry.inspect_commit(transaction)
    }

    pub(super) fn commit_prepared(
        &mut self,
        transaction: ProductDialogueTransaction,
        proof: RuntimeDialogueCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        self.registry.commit_prepared(transaction, proof)
    }

    pub(super) fn commit_receiver(&mut self, transaction: ProductDialogueTransaction) {
        let proof = self
            .registry
            .inspect_commit(&transaction)
            .expect("the receiving activation retains its preflighted revision");
        self.registry.commit_prepared(transaction, proof);
    }

    pub(super) fn inspect_published(
        &self,
        transaction: &ProductDialogueTransaction,
    ) -> Result<RuntimeDialoguePublishedCommitProof, LineRuntimeError> {
        self.registry.inspect_published(transaction)
    }

    pub(super) fn commit_published_prepared(
        &mut self,
        transaction: ProductDialogueTransaction,
        proof: RuntimeDialoguePublishedCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        self.registry.commit_published_prepared(transaction, proof)
    }

    pub(super) fn inspect_abandoned(
        &self,
        transaction: &ProductDialogueTransaction,
    ) -> Result<RuntimeDialogueAbandonedCommitProof, LineRuntimeError> {
        self.registry.inspect_abandoned(transaction)
    }

    pub(super) fn commit_abandoned_prepared(
        &mut self,
        transaction: ProductDialogueTransaction,
        proof: RuntimeDialogueAbandonedCommitProof,
    ) -> RuntimeDialogueRegistryCommitReceipt {
        self.registry.commit_abandoned_prepared(transaction, proof)
    }

    pub(super) fn latch_step_input(
        &mut self,
        program: &crate::awbc::schema::AwbcProgram,
        foreground: Option<DialogueActivationId>,
        dt: LogicalDuration,
        content_events: &[RuntimeDialogueContentEvent],
        advances: &[DialogueActivationId],
        line_outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<Vec<LineRuntimeError>, ProductStepError> {
        // Validate the complete ingress before updating the foreground clock
        // or advancing any activation revision.
        // Published ledgers are copyable metadata and are staged separately.
        let published = self.registry.stage_published_outcomes(line_outcomes)?;
        if let Some(frame) = foreground.as_ref().and_then(|id| self.active_frame_for(id))
            && frame.is_ingress_ready()
        {
            frame
                .elapsed_nanos
                .checked_add(dt.as_nanos())
                .ok_or(LineRuntimeError::DialogueElapsedOverflow)?;
        }
        let mut seen_content = BTreeSet::new();
        for event in content_events {
            let frame = self
                .registry
                .active_frame(event.activation())
                .ok_or_else(|| {
                    if self.registry.is_published(event.activation()) {
                        LineRuntimeError::ActivationFrameReleased
                    } else {
                        LineRuntimeError::UnknownActivationLedger
                    }
                })?;
            if !frame.is_ingress_ready() {
                return Err(LineRuntimeError::DialogueIngressNotReady {
                    activation: event.activation().clone(),
                }
                .into());
            }
            let kind = event.kind();
            if frame.pending_content_events.contains(&kind)
                || !seen_content.insert((event.activation().clone(), kind))
            {
                return Err(LineRuntimeError::DuplicateContentEvent { event: kind }.into());
            }
        }
        let mut seen_advances = BTreeSet::new();
        for activation in advances {
            let frame = self.registry.active_frame(activation).ok_or_else(|| {
                if self.registry.is_published(activation) {
                    LineRuntimeError::ActivationFrameReleased
                } else {
                    LineRuntimeError::UnknownActivationLedger
                }
            })?;
            if !frame.is_ingress_ready() {
                return Err(LineRuntimeError::DialogueIngressNotReady {
                    activation: activation.clone(),
                }
                .into());
            }
            if frame.pending_advance || !seen_advances.insert(activation.clone()) {
                return Err(LineRuntimeError::DuplicateDialogueAdvance {
                    activation: activation.clone(),
                }
                .into());
            }
        }
        let mut seen_outcomes = BTreeSet::new();
        for outcome in line_outcomes {
            let activation = outcome.command().activation();
            if self.registry.is_published(activation) {
                continue;
            }
            let frame = self
                .registry
                .active_frame(activation)
                .ok_or(LineRuntimeError::UnknownActivationLedger)?;
            if frame
                .pending_line_outcomes
                .iter()
                .any(|pending| pending.command() == outcome.command())
                || !seen_outcomes.insert(outcome.command().clone())
            {
                return Err(LineRuntimeError::DuplicateCommandOutcome.into());
            }
        }
        let mut revisions = BTreeMap::<DialogueActivationId, u64>::new();
        for activation in foreground
            .iter()
            .filter(|_| dt.as_nanos() != 0)
            .chain(content_events.iter().map(|event| event.activation()))
            .chain(advances)
            .chain(seen_outcomes.iter().map(|command| command.activation()))
        {
            let count = revisions.entry(*activation).or_default();
            *count = count
                .checked_add(1)
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        }
        for (activation, commits) in revisions {
            self.registry
                .active_revision(&activation)
                .and_then(|revision| revision.checked_add(commits))
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
        }
        if let Some(activation) = foreground.as_ref() {
            let mut transaction = self.registry.begin_transaction(activation)?;
            if let super::ProductDialoguePhase::Activating { fiber, .. }
            | super::ProductDialoguePhase::Closing(super::ProductDialogueClosing {
                state: super::ProductDialogueClosingState::Activation { fiber, .. },
                ..
            }) = &mut transaction.frame_mut().phase
                && let Err(error) = fiber.advance_duration_wait(program, dt)
            {
                self.registry.restore_transaction(transaction)?;
                return Err(error.into());
            }
            if transaction.frame().is_ingress_ready() {
                transaction.frame_mut().elapsed_nanos += dt.as_nanos();
            }
            if dt.as_nanos() == 0 {
                // Duration authentication still ran, but zero elapsed time
                // changes no payload and must not spend a registry revision.
                self.registry.restore_transaction(transaction)?;
            } else {
                self.registry.commit(transaction)?;
            }
        }
        for event in content_events {
            let mut transaction = self.ready_transaction(event.activation())?;
            transaction
                .frame_mut()
                .pending_content_events
                .push(event.kind());
            self.registry.commit(transaction)?;
        }
        for activation in advances {
            let mut transaction = self.ready_transaction(activation)?;
            transaction.frame_mut().pending_advance = true;
            self.registry.commit(transaction)?;
        }
        for outcome in line_outcomes {
            let activation = outcome.command().activation();
            if self.registry.is_published(activation) {
                continue;
            }
            let mut transaction = self.registry.begin_transaction(activation)?;
            transaction
                .frame_mut()
                .pending_line_outcomes
                .push(outcome.clone());
            self.registry.commit(transaction)?;
        }
        Ok(self.registry.commit_published_outcomes(published))
    }

    fn ready_transaction(
        &mut self,
        activation: &DialogueActivationId,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        let transaction = self.registry.begin_transaction(activation)?;
        if transaction.frame().is_ingress_ready() {
            Ok(transaction)
        } else {
            self.registry.restore_transaction(transaction)?;
            Err(LineRuntimeError::DialogueIngressNotReady {
                activation: activation.clone(),
            })
        }
    }

    pub(super) fn reconcile_parent_fiber(
        &mut self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<crate::line_task::RuntimeHandleDropReceipt, LineRuntimeError> {
        self.registry
            .reconcile_parent_fiber(execution, before, after, drops)
    }

    pub(super) fn inspect_parent_fiber_reconciliation(
        &self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<PreparedRuntimeParentFiberReconciliation, LineRuntimeError> {
        self.registry
            .inspect_parent_fiber_reconciliation(execution, before, after, drops)
    }

    pub(super) fn borrow_stage_actor(
        &self,
        activation: &DialogueActivationId,
        local: &RuntimeLineHandleLedger,
        actor: RuntimeValueView<'_>,
        slot: RuntimeOwnedSlotId,
        character: &CharacterId,
    ) -> Result<RuntimeLineHandleToken, LineRuntimeError> {
        self.registry
            .borrow_stage_actor(activation, local, actor, slot, character)
    }

    pub(super) fn inspect_received_fiber_reconciliation(
        &self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<RuntimeLineHandleToken, crate::line_task::RuntimeHandleOwnerSlot>,
        after: &BTreeMap<RuntimeLineHandleToken, crate::line_task::RuntimeHandleOwnerSlot>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
        result: Option<(
            &DialogueActivationId,
            &[crate::value::ownership::RuntimeAffineLineHandle],
        )>,
    ) -> Result<Option<PreparedRuntimeParentFiberReconciliation>, LineRuntimeError> {
        self.registry
            .inspect_received_fiber_reconciliation(execution, before, after, drops, None, result)
    }

    pub(super) fn validate_registry_handle(
        &self,
        activation: &DialogueActivationId,
        ledger: &crate::line_task::RuntimeLineHandleLedger,
        handle: &crate::value::ownership::RuntimeAffineLineHandle,
    ) -> Result<(), LineRuntimeError> {
        self.registry
            .validate_registry_handle(activation, ledger, handle)
    }

    pub(super) fn reconcile_child_registry_step(
        &self,
        transaction: &mut ProductDialogueTransaction,
        execution: crate::runtime_id::ExecutionInstanceId,
        tag: &crate::line_task::LineTaskWorkTag,
        before: &std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        after: &std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
        change: Option<&crate::effect::RuntimeRegistryLineChange<'_>>,
    ) -> Result<(), LineRuntimeError> {
        self.registry.reconcile_child_registry_step(
            transaction,
            execution,
            tag,
            before,
            after,
            drops,
            change,
        )
    }

    pub(super) fn retire_line_registry(
        &self,
        transaction: &mut ProductDialogueTransaction,
        execution: crate::runtime_id::ExecutionInstanceId,
    ) -> Result<Option<PreparedRuntimeParentFiberReconciliation>, LineRuntimeError> {
        self.registry.retire_line_registry(transaction, execution)
    }

    pub(super) fn prepare_result_abandonment(
        &self,
        transaction: &mut ProductDialogueTransaction,
    ) -> Result<bool, LineRuntimeError> {
        self.registry.prepare_result_abandonment(transaction)
    }

    pub(super) fn inspect_received_result_retirement(
        &self,
        transaction: &ProductDialogueTransaction,
    ) -> Result<Option<PreparedRuntimeParentFiberReconciliation>, LineRuntimeError> {
        self.registry
            .inspect_received_result_retirement(transaction)
    }

    pub(super) fn has_pending_result_retirement(
        &self,
        transaction: &ProductDialogueTransaction,
    ) -> bool {
        self.registry
            .has_staged_or_pending_received_result_retirement(transaction)
    }

    pub(super) fn inspect_received_result_publication(
        &self,
        receiver: &DialogueActivationId,
        execution: crate::runtime_id::ExecutionInstanceId,
        handles: &[crate::value::ownership::RuntimeAffineLineHandle],
        destinations: &BTreeMap<RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        line_ready: bool,
    ) -> Result<(bool, Option<PreparedRuntimeParentFiberReconciliation>), LineRuntimeError> {
        self.registry.inspect_received_result_publication(
            receiver,
            execution,
            handles,
            destinations,
            line_ready,
        )
    }

    pub(super) fn commit_parent_fiber_reconciliation(
        &mut self,
        prepared: PreparedRuntimeParentFiberReconciliation,
    ) -> crate::line_task::RuntimeHandleDropReceipt {
        self.registry.commit_parent_fiber_reconciliation(prepared)
    }

    pub(super) fn retained_contour<'a>(
        &'a self,
        transactions: impl ExactSizeIterator<Item = &'a ProductDialogueTransaction>,
        admission: &mut crate::value::RuntimeRetainedValueAdmission,
    ) -> Result<
        crate::line_task::RuntimeDialogueRetainedContour<'a, ActiveDialogue, AwbcTypeId>,
        crate::line_task::RuntimeDialogueRegistrySnapshotError,
    > {
        self.registry.retained_contour(transactions, admission)
    }

    pub(super) fn snapshot_admitted<S>(
        &self,
        owner: &RuntimeProgramOwner,
        snapshot_frame: impl FnMut(
            &ActiveDialogue,
        )
            -> Result<S, crate::line_task::RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<
        crate::line_task::RuntimeDialogueRegistrySaveSnapshot<S, AwbcTypeId>,
        crate::line_task::RuntimeDialogueRegistrySnapshotError,
    > {
        self.registry.snapshot_admitted(Some(owner), snapshot_frame)
    }

    pub(super) fn from_admitted_save_snapshot<'a, S>(
        snapshot: crate::line_task::RuntimeDialogueRegistrySaveSnapshot<S, AwbcTypeId>,
        owner: impl Into<crate::value::RuntimeValueRestoreContext<'a>>,
        restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>,
        ) -> Result<
            ActiveDialogue,
            crate::line_task::RuntimeDialogueRegistrySnapshotError,
        >,
    ) -> Result<Self, crate::line_task::RuntimeDialogueRegistrySnapshotError> {
        let owner = owner.into();
        let RuntimeProgramOwner::Awbc(_) = owner.owner() else {
            return Err(
                crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                    message: "AWBC dialogue restore requires its AWBC program owner".to_owned(),
                },
            );
        };
        let registry = RuntimeDialogueActivationRegistry::from_admitted_save_snapshot(
            snapshot,
            owner,
            restore_frame,
        )?;
        Ok(Self { registry })
    }
}

impl ActiveDialogue {
    fn receiving_fiber(&self) -> Option<&FiberState> {
        match &self.phase {
            super::ProductDialoguePhase::Activating { fiber, .. } => Some(fiber),
            _ => None,
        }
    }

    pub(super) fn receiving_fiber_mut(&mut self) -> Option<&mut FiberState> {
        match &mut self.phase {
            super::ProductDialoguePhase::Activating { fiber, .. } => Some(fiber),
            _ => None,
        }
    }
}

fn visit_value_graph<E>(
    value: &RuntimeValue,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    crate::value::visit_runtime_value_graph(value, |nested| visitor(nested))
}

fn visit_value_slice<E>(
    values: &[RuntimeValue],
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    for value in values {
        visit_value_graph(value, visitor)?;
    }
    Ok(())
}

fn visit_pending_line_operation<E>(
    pending: Option<&super::ProductPendingLineOperation>,
    visitor: &mut impl FnMut(&RuntimeValue) -> Result<(), E>,
) -> Result<(), E> {
    if let Some(
        super::ProductPendingLineOperation::AcquireActor { value, .. }
        | super::ProductPendingLineOperation::ActorLook { value, .. },
    ) = pending
    {
        visit_value_graph(value, visitor)?;
    }
    Ok(())
}
