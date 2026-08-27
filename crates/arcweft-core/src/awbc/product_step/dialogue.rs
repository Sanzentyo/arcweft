use super::{ActiveDialogue, ProductStepError};
use crate::awbc::schema::AwbcTypeId;
use crate::line_task::{
    LineRuntimeError, RuntimeDialogueActivationRegistry, RuntimeDialogueActivationTransaction,
    RuntimeDialogueRegistryCommitReceipt,
};
use crate::runtime_id::DialogueActivationId;
use crate::step::RuntimeDialogueContentEvent;
use crate::time::LogicalDuration;
use crate::value::ownership::RuntimeOwnedSlotId;
use std::collections::BTreeMap;

/// Product adapter over the executor-neutral dialogue registry. Product owns
/// only ingress readiness and its AWBC frame payload; Active/PublishedHandles,
/// revisions, ledger/command transactions, publication, and parent drops are
/// shared with structured execution.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct ProductDialogueStore {
    registry: RuntimeDialogueActivationRegistry<ActiveDialogue, AwbcTypeId>,
}

pub(super) type ProductDialogueTransaction =
    RuntimeDialogueActivationTransaction<ActiveDialogue, AwbcTypeId>;

impl ProductDialogueStore {
    pub(super) fn begin(&mut self, frame: ActiveDialogue) -> Result<(), LineRuntimeError> {
        if !self.registry.active_ids().is_empty() {
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

    pub(super) fn begin_active_transaction(
        &self,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        let activation = self
            .active_activation()
            .ok_or(LineRuntimeError::UnknownActivationLedger)?;
        self.registry.begin_transaction(&activation)
    }

    pub(super) fn begin_transaction(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        self.registry.begin_transaction(activation)
    }

    pub(super) fn commit(
        &mut self,
        transaction: ProductDialogueTransaction,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        self.registry.commit(transaction)
    }

    pub(super) fn commit_published(
        &mut self,
        transaction: ProductDialogueTransaction,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        self.registry.commit_published(transaction)
    }

    pub(super) fn commit_abandoned(
        &mut self,
        transaction: ProductDialogueTransaction,
    ) -> Result<RuntimeDialogueRegistryCommitReceipt, LineRuntimeError> {
        self.registry.commit_abandoned(transaction)
    }

    pub(super) fn latch_step_input(
        &mut self,
        dt: LogicalDuration,
        content_events: &[RuntimeDialogueContentEvent],
        advances: &[DialogueActivationId],
        line_outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<Vec<LineRuntimeError>, ProductStepError> {
        let mut next = self.clone();
        if let Some(activation) = next.active_activation() {
            let mut transaction = next.registry.begin_transaction(&activation)?;
            if transaction.frame().is_ingress_ready() {
                transaction.frame_mut().elapsed_nanos = transaction
                    .frame()
                    .elapsed_nanos
                    .checked_add(dt.as_nanos())
                    .ok_or(LineRuntimeError::DialogueElapsedOverflow)?;
            }
            next.registry.commit(transaction)?;
        }
        for event in content_events {
            let mut transaction = next.ready_transaction(event.activation())?;
            let kind = event.kind();
            if transaction.frame().pending_content_events.contains(&kind) {
                return Err(LineRuntimeError::DuplicateContentEvent { event: kind }.into());
            }
            transaction.frame_mut().pending_content_events.push(kind);
            next.registry.commit(transaction)?;
        }
        for activation in advances {
            let mut transaction = next.ready_transaction(activation)?;
            if transaction.frame().pending_advance {
                return Err(LineRuntimeError::DuplicateDialogueAdvance {
                    activation: activation.clone(),
                }
                .into());
            }
            transaction.frame_mut().pending_advance = true;
            next.registry.commit(transaction)?;
        }
        let mut diagnostics = Vec::new();
        for outcome in line_outcomes {
            let activation = outcome.command().activation();
            if next.registry.is_published(activation) {
                if let Some(diagnostic) = next.registry.accept_published_outcome(outcome)? {
                    diagnostics.push(diagnostic);
                }
                continue;
            }
            let mut transaction = next.registry.begin_transaction(activation)?;
            if transaction
                .frame()
                .pending_line_outcomes
                .iter()
                .any(|pending| pending.command() == outcome.command())
            {
                return Err(LineRuntimeError::DuplicateCommandOutcome.into());
            }
            transaction
                .frame_mut()
                .pending_line_outcomes
                .push(outcome.clone());
            next.registry.commit(transaction)?;
        }
        *self = next;
        Ok(diagnostics)
    }

    fn ready_transaction(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<ProductDialogueTransaction, LineRuntimeError> {
        let transaction = self.registry.begin_transaction(activation)?;
        if transaction.frame().is_ingress_ready() {
            Ok(transaction)
        } else {
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
        drop_policy: Option<crate::effect::RuntimeDropPolicy>,
    ) -> Result<crate::line_task::RuntimeHandleDropReceipt, LineRuntimeError> {
        self.registry
            .reconcile_parent_fiber(execution, before, after, drop_policy)
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
        restore_frame: impl FnMut(
            &DialogueActivationId,
            S,
            &crate::line_task::RuntimeDialogueActivationState<AwbcTypeId>,
        ) -> Result<
            ActiveDialogue,
            crate::line_task::RuntimeDialogueRegistrySnapshotError,
        >,
    ) -> Result<Self, crate::line_task::RuntimeDialogueRegistrySnapshotError> {
        Ok(Self {
            registry: RuntimeDialogueActivationRegistry::from_save_snapshot(
                snapshot,
                restore_frame,
            )?,
        })
    }
}
