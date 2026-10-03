use super::{ActiveDialogue, ProductStepError};
use crate::awbc::schema::AwbcTypeId;
use crate::line_task::{
    LineRuntimeError, PreparedRuntimeParentFiberReconciliation,
    RuntimeDialogueAbandonedCommitProof, RuntimeDialogueActivationRegistry,
    RuntimeDialogueActivationTransaction, RuntimeDialogueCommitProof,
    RuntimeDialoguePublishedCommitProof, RuntimeDialogueRegistryCommitReceipt,
};
use crate::runtime_id::DialogueActivationId;
use crate::step::RuntimeDialogueContentEvent;
use crate::task::RuntimeProgramOwner;
use crate::time::LogicalDuration;
use crate::value::{RuntimeValue, ownership::RuntimeOwnedSlotId};
use std::collections::{BTreeMap, BTreeSet};

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

impl ProductDialogueStore {
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
        self.registry.begin(frame.activation.clone(), frame)
    }

    pub(super) fn active_activation(&self) -> Option<DialogueActivationId> {
        self.registry.active_ids().into_iter().next()
    }

    pub(super) fn active_frame(&self) -> Option<&ActiveDialogue> {
        self.active_activation()
            .as_ref()
            .and_then(|activation| self.registry.active_frame(activation))
    }

    pub(super) fn active_line(
        &self,
    ) -> Option<&crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>> {
        self.active_activation()
            .as_ref()
            .and_then(|activation| self.registry.active_line(activation))
    }

    pub(super) fn visit_runtime_values<E>(
        &self,
        mut visitor: impl FnMut(&RuntimeValue) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Some(frame) = self.active_frame() {
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
        }
        if let Some(line) = self.active_line() {
            line.visit_runtime_values(&mut visitor)?;
        }
        Ok(())
    }

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
        dt: LogicalDuration,
        content_events: &[RuntimeDialogueContentEvent],
        advances: &[DialogueActivationId],
        line_outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<Vec<LineRuntimeError>, ProductStepError> {
        // Validate the complete ingress before taking the sole active frame.
        // Published ledgers are copyable metadata and are staged separately.
        let published = self.registry.stage_published_outcomes(line_outcomes)?;
        let active = self.active_activation();
        if let Some(frame) = self.active_frame()
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
        if let Some(activation) = active.as_ref() {
            let commits = 1_usize
                .checked_add(content_events.len())
                .and_then(|count| count.checked_add(advances.len()))
                .and_then(|count| count.checked_add(seen_outcomes.len()))
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;
            let commits = u64::try_from(commits)
                .map_err(|_| LineRuntimeError::ActivationTransactionRevisionOverflow)?;
            self.registry
                .active_revision(activation)
                .and_then(|revision| revision.checked_add(commits))
                .ok_or(LineRuntimeError::ActivationTransactionRevisionOverflow)?;

            let mut transaction = self.registry.begin_transaction(activation)?;
            if transaction.frame().is_ingress_ready() {
                transaction.frame_mut().elapsed_nanos += dt.as_nanos();
            }
            self.registry.commit(transaction)?;
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

    pub(super) fn commit_parent_fiber_reconciliation(
        &mut self,
        prepared: PreparedRuntimeParentFiberReconciliation,
    ) -> crate::line_task::RuntimeHandleDropReceipt {
        self.registry.commit_parent_fiber_reconciliation(prepared)
    }

    pub(super) fn to_save_snapshot<S>(
        &self,
        snapshot_frame: impl FnMut(
            &ActiveDialogue,
        )
            -> Result<S, crate::line_task::RuntimeDialogueRegistrySnapshotError>,
    ) -> Result<
        crate::line_task::RuntimeDialogueRegistrySaveSnapshot<S, AwbcTypeId>,
        crate::line_task::RuntimeDialogueRegistrySnapshotError,
    > {
        self.registry.to_save_snapshot(snapshot_frame)
    }

    pub(super) fn from_save_snapshot<S>(
        snapshot: crate::line_task::RuntimeDialogueRegistrySaveSnapshot<S, AwbcTypeId>,
        owner: &crate::task::RuntimeProgramOwner,
        expected_deferred_children: &BTreeMap<
            DialogueActivationId,
            (
                crate::runtime_id::RuntimeDeferRegistrationId,
                crate::runtime_id::RuntimeDeferSiteId,
            ),
        >,
        restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>,
        ) -> Result<
            ActiveDialogue,
            crate::line_task::RuntimeDialogueRegistrySnapshotError,
        >,
    ) -> Result<Self, crate::line_task::RuntimeDialogueRegistrySnapshotError> {
        let RuntimeProgramOwner::Awbc(program) = owner else {
            return Err(
                crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                    message: "AWBC dialogue restore requires its AWBC program owner".to_owned(),
                },
            );
        };
        let registry =
            RuntimeDialogueActivationRegistry::from_save_snapshot_with_deferred_children(
                snapshot,
                owner,
                expected_deferred_children,
                restore_frame,
            )?;
        for activation in registry.active_ids() {
            let Some(line) = registry.active_line(&activation) else {
                continue;
            };
            line.validate_deferred_sites(|site| program.defer_sites.get(site.index()).is_some())?;
            for registration in line.deferred_registrations() {
                let Some(function_id) = program.defer_sites.get(registration.site().index()) else {
                    return Err(
                        crate::line_task::LineRuntimeError::InvalidRestoredDeferredState.into(),
                    );
                };
                let Some(function) = program.functions.get(function_id.index()) else {
                    return Err(
                        crate::line_task::LineRuntimeError::InvalidRestoredDeferredState.into(),
                    );
                };
                let Some(signature) = program.signatures.get(function.signature.index()) else {
                    return Err(
                        crate::line_task::LineRuntimeError::InvalidRestoredDeferredState.into(),
                    );
                };
                if registration.captures().len() != signature.params.len()
                    || !signature.result.is_some_and(|result| {
                        matches!(
                            program
                                .runtime_types
                                .get(result.index())
                                .map(crate::awbc::schema::AwbcRuntimeType::shape),
                            Some(crate::awbc::schema::AwbcRuntimeTypeShape::Unit)
                        )
                    })
                    || registration.captures().iter().zip(&signature.params).any(
                        |(capture, expected)| {
                            !crate::awbc::fiber::runtime_value_matches_type(
                                program, capture, *expected, 0,
                            )
                        },
                    )
                {
                    return Err(
                        crate::line_task::LineRuntimeError::InvalidRestoredDeferredState.into(),
                    );
                }
            }
        }
        Ok(Self { registry })
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
