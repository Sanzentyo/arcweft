use crate::effect::RuntimeDropPolicy;
use crate::line_task::{
    LineRuntimeError, LineTaskLiveState, RuntimeDialogueActivationRegistry,
    RuntimeDialogueActivationState, RuntimeDialogueActivationTransaction,
    RuntimeDialogueCommitReceipt, RuntimeHandleDropReceipt,
};
use crate::pattern::RuntimePattern;
use crate::runtime_id::{DialogueActivationId, RuntimePlanTypeId};
use crate::step::{RuntimeDialogueContentEvent, RuntimeDialogueContentEventKind};
use crate::time::LogicalDuration;
use crate::value::ownership::RuntimeOwnedSlotId;
use crate::value::{RuntimeLocalBinding, RuntimeValue};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq)]
#[error("dialogue ingress rejected: {source}")]
pub(in crate::engine) struct DialogueIngressError {
    activation: Option<DialogueActivationId>,
    #[source]
    source: LineRuntimeError,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(in crate::engine) struct DialogueIngressReceipt {
    diagnostics: Vec<LineRuntimeError>,
}

impl DialogueIngressReceipt {
    pub(in crate::engine) fn into_diagnostics(self) -> Vec<LineRuntimeError> {
        self.diagnostics
    }
}

impl DialogueIngressError {
    pub(in crate::engine) const fn activation(&self) -> Option<&DialogueActivationId> {
        self.activation.as_ref()
    }

    pub(in crate::engine) fn into_source(self) -> LineRuntimeError {
        self.source
    }

    fn for_activation(activation: &DialogueActivationId, source: LineRuntimeError) -> Self {
        Self {
            activation: Some(activation.clone()),
            source,
        }
    }
}

/// Suspended dialogue line awaiting explicit host progression.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueActivationFrame {
    pub(in crate::engine) line: crate::plan::RuntimeLineId,
    pub(in crate::engine) content: crate::runtime_id::RuntimeDialogueContentPlanId,
    pub(in crate::engine) task_group: crate::runtime_id::RuntimeLineTaskGroupId,
    pub(in crate::engine) resume: Option<super::super::FlowCursor>,
    pub(in crate::engine) captures: Box<[RuntimeLocalBinding]>,
    /// Activation-local execution environment. The parent fiber never owns
    /// these bindings while the dialogue transaction is live.
    pub(in crate::engine) locals: crate::value::RuntimeEnv,
    pub(in crate::engine) line_task: DialogueLineTaskState,
    /// Logical time accumulated while this line has been active.
    pub(in crate::engine) elapsed: crate::time::LogicalDuration,
    pub(in crate::engine) phase: DialogueRuntimePhase,
    pub(in crate::engine) result_target: crate::plan::RuntimeDialogueResultTarget,
    pub(in crate::engine) voice: crate::presentation::RuntimeDialogueVoiceState,
    pub(in crate::engine) values: Box<[crate::plan::RuntimeDialogueValueBinding]>,
    pub(in crate::engine) activation_pc: usize,
    pub(in crate::engine) pending_line_operation: Option<PendingLineOperation>,
    pub(in crate::engine) failure: Option<super::DialogueExecutionError>,
}

/// Durable step ingress owned by one activation until its executor phase can
/// consume each channel exactly once.
#[derive(Clone, Debug, Default, PartialEq)]
struct DialogueStepInbox {
    content_events: Vec<RuntimeDialogueContentEventKind>,
    line_outcomes: Vec<crate::presentation::RuntimeLineHostOutcome>,
    advance: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DialogueLineTaskState {
    NotStarted,
    Live(LineTaskLiveState),
    Closed,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum PendingLineOperation {
    AcquireActor {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        value: RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    ActorLook {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        value: RuntimeValue,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    StartVoice {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DialogueRuntimePhase {
    Activating,
    Ready,
    Closing,
    Publishing,
}

/// Sole engine execution owner of dialogue frames and their line-runtime
/// transaction state. Fiber suspension retains only the activation key.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct DialogueActivationStore {
    registry: RuntimeDialogueActivationRegistry<EngineDialogueActivationFrame, RuntimePlanTypeId>,
}

#[derive(Clone, Debug, PartialEq)]
struct EngineDialogueActivationFrame {
    frame: DialogueActivationFrame,
    inbox: DialogueStepInbox,
}

/// Opaque optimistic transaction over one complete dialogue activation.
/// The key and revision cannot be mixed with another frame or line component.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueActivationTransaction {
    inner: RuntimeDialogueActivationTransaction<EngineDialogueActivationFrame, RuntimePlanTypeId>,
    disposition: Option<DialogueCommitDisposition>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum DialogueCommitDisposition {
    Published {
        resume: Option<super::super::FlowCursor>,
        bindings: Vec<crate::value::RuntimeLocalBinding>,
    },
    Failed {
        error: super::DialogueExecutionError,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueCommitReceipt {
    line: RuntimeDialogueCommitReceipt,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueTerminalReceipt {
    line: RuntimeDialogueCommitReceipt,
    disposition: DialogueCommitDisposition,
}

impl DialogueActivationTransaction {
    #[must_use]
    pub(crate) const fn activation(&self) -> &DialogueActivationId {
        self.inner.activation()
    }

    #[must_use]
    pub(crate) const fn frame(&self) -> &DialogueActivationFrame {
        &self.inner.frame().frame
    }

    pub(crate) const fn frame_mut(&mut self) -> &mut DialogueActivationFrame {
        &mut self.inner.frame_mut().frame
    }

    #[must_use]
    pub(crate) const fn line(&self) -> &RuntimeDialogueActivationState<RuntimePlanTypeId> {
        self.inner.line()
    }

    pub(crate) const fn line_mut(
        &mut self,
    ) -> &mut RuntimeDialogueActivationState<RuntimePlanTypeId> {
        self.inner.line_mut()
    }

    pub(crate) fn parts_mut(
        &mut self,
    ) -> (
        &mut DialogueActivationFrame,
        &mut RuntimeDialogueActivationState<RuntimePlanTypeId>,
    ) {
        let (frame, line) = self.inner.parts_mut();
        (&mut frame.frame, line)
    }

    pub(crate) fn stage_disposition(
        &mut self,
        disposition: DialogueCommitDisposition,
    ) -> Result<(), LineRuntimeError> {
        if self.disposition.is_some() {
            return Err(LineRuntimeError::InvalidResultTransition);
        }
        self.disposition = Some(disposition);
        Ok(())
    }

    pub(crate) fn take_line_outcomes(
        &mut self,
    ) -> Vec<crate::presentation::RuntimeLineHostOutcome> {
        std::mem::take(&mut self.inner.frame_mut().inbox.line_outcomes)
    }

    pub(crate) fn take_content_events(&mut self) -> Vec<RuntimeDialogueContentEventKind> {
        std::mem::take(&mut self.inner.frame_mut().inbox.content_events)
    }

    pub(crate) fn take_advance(&mut self) -> bool {
        std::mem::take(&mut self.inner.frame_mut().inbox.advance)
    }
}

impl DialogueCommitReceipt {
    pub(crate) fn into_line(self) -> RuntimeDialogueCommitReceipt {
        self.line
    }
}

impl DialogueTerminalReceipt {
    pub(crate) fn into_parts(self) -> (RuntimeDialogueCommitReceipt, DialogueCommitDisposition) {
        (self.line, self.disposition)
    }
}

impl DialogueActivationStore {
    pub(crate) fn begin(
        &mut self,
        activation: DialogueActivationId,
        frame: DialogueActivationFrame,
    ) -> Result<(), LineRuntimeError> {
        self.registry.begin(
            activation,
            EngineDialogueActivationFrame {
                frame,
                inbox: DialogueStepInbox::default(),
            },
        )
    }

    pub(crate) fn begin_transaction(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<DialogueActivationTransaction, LineRuntimeError> {
        Ok(DialogueActivationTransaction {
            inner: self.registry.begin_transaction(activation)?,
            disposition: None,
        })
    }

    /// Atomically latches every dialogue-owned input channel before root or
    /// scheduler execution. Only activations that existed at step ingress
    /// receive this step's logical duration.
    pub(in crate::engine) fn latch_step_input(
        &mut self,
        dt: LogicalDuration,
        content_events: &[RuntimeDialogueContentEvent],
        advances: &[DialogueActivationId],
        line_outcomes: &[crate::presentation::RuntimeLineHostOutcome],
    ) -> Result<DialogueIngressReceipt, DialogueIngressError> {
        let mut next = self.clone();
        let mut receipt = DialogueIngressReceipt::default();
        for activation in next.registry.active_ids() {
            let mut transaction = next
                .begin_transaction(&activation)
                .map_err(|source| DialogueIngressError::for_activation(&activation, source))?;
            if transaction.frame().phase == DialogueRuntimePhase::Ready {
                transaction.frame_mut().elapsed =
                    transaction.frame().elapsed.checked_add(dt).ok_or_else(|| {
                        DialogueIngressError::for_activation(
                            &activation,
                            LineRuntimeError::DialogueElapsedOverflow,
                        )
                    })?;
            }
            next.commit_transaction(transaction)
                .map_err(|source| DialogueIngressError::for_activation(&activation, source))?;
        }
        for event in content_events {
            let mut transaction = next
                .ready_transaction(event.activation())
                .map_err(|source| {
                    DialogueIngressError::for_activation(event.activation(), source)
                })?;
            let kind = event.kind();
            if transaction
                .inner
                .frame()
                .inbox
                .content_events
                .contains(&kind)
            {
                return Err(DialogueIngressError::for_activation(
                    event.activation(),
                    LineRuntimeError::DuplicateContentEvent { event: kind },
                ));
            }
            transaction
                .inner
                .frame_mut()
                .inbox
                .content_events
                .push(kind);
            next.commit_transaction(transaction).map_err(|source| {
                DialogueIngressError::for_activation(event.activation(), source)
            })?;
        }
        for advance in advances {
            let mut transaction = next
                .ready_transaction(advance)
                .map_err(|source| DialogueIngressError::for_activation(advance, source))?;
            if transaction.inner.frame().inbox.advance {
                return Err(DialogueIngressError::for_activation(
                    advance,
                    LineRuntimeError::DuplicateDialogueAdvance {
                        activation: advance.clone(),
                    },
                ));
            }
            transaction.inner.frame_mut().inbox.advance = true;
            next.commit_transaction(transaction)
                .map_err(|source| DialogueIngressError::for_activation(advance, source))?;
        }
        for outcome in line_outcomes {
            let command = outcome.command();
            if next.registry.is_published(command.activation()) {
                if let Some(diagnostic) =
                    next.registry
                        .accept_published_outcome(outcome)
                        .map_err(|source| {
                            DialogueIngressError::for_activation(command.activation(), source)
                        })?
                {
                    receipt.diagnostics.push(diagnostic);
                }
                continue;
            }
            let mut transaction =
                next.begin_transaction(command.activation())
                    .map_err(|source| {
                        DialogueIngressError::for_activation(command.activation(), source)
                    })?;
            if transaction
                .inner
                .frame()
                .inbox
                .line_outcomes
                .iter()
                .any(|pending| pending.command() == command)
            {
                return Err(DialogueIngressError::for_activation(
                    command.activation(),
                    LineRuntimeError::DuplicateCommandOutcome,
                ));
            }
            transaction
                .inner
                .frame_mut()
                .inbox
                .line_outcomes
                .push(outcome.clone());
            next.commit_transaction(transaction).map_err(|source| {
                DialogueIngressError::for_activation(command.activation(), source)
            })?;
        }
        *self = next;
        Ok(receipt)
    }

    pub(in crate::engine) fn reconcile_parent_fiber(
        &mut self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drop_policy: Option<RuntimeDropPolicy>,
    ) -> Result<RuntimeHandleDropReceipt, LineRuntimeError> {
        self.registry
            .reconcile_parent_fiber(execution, before, after, drop_policy)
    }

    pub(crate) fn commit_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
    ) -> Result<DialogueCommitReceipt, LineRuntimeError> {
        if transaction.disposition.is_some() {
            return Err(LineRuntimeError::UnexpectedTerminalDisposition);
        }
        Ok(DialogueCommitReceipt {
            line: self.registry.commit(transaction.inner)?.into_line(),
        })
    }

    pub(crate) fn commit_terminal_transaction(
        &mut self,
        mut transaction: DialogueActivationTransaction,
    ) -> Result<DialogueTerminalReceipt, LineRuntimeError> {
        if !transaction.line().is_terminal() {
            return Err(LineRuntimeError::ActivationTransactionNotTerminal);
        }
        let disposition = transaction
            .disposition
            .take()
            .ok_or(LineRuntimeError::TerminalDispositionMismatch)?;
        if !matches!(disposition, DialogueCommitDisposition::Failed { .. }) {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        let receipt = DialogueTerminalReceipt {
            line: self
                .registry
                .commit_abandoned(transaction.inner)?
                .into_line(),
            disposition,
        };
        Ok(receipt)
    }

    pub(crate) fn commit_published_transaction(
        &mut self,
        mut transaction: DialogueActivationTransaction,
    ) -> Result<DialogueTerminalReceipt, LineRuntimeError> {
        let disposition = transaction
            .disposition
            .take()
            .ok_or(LineRuntimeError::TerminalDispositionMismatch)?;
        if !matches!(disposition, DialogueCommitDisposition::Published { .. }) {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        let receipt = DialogueTerminalReceipt {
            line: self
                .registry
                .commit_published(transaction.inner)?
                .into_line(),
            disposition,
        };
        Ok(receipt)
    }

    fn ready_transaction(
        &self,
        activation: &DialogueActivationId,
    ) -> Result<DialogueActivationTransaction, LineRuntimeError> {
        let transaction = self.begin_transaction(activation)?;
        if transaction.frame().phase == DialogueRuntimePhase::Ready {
            Ok(transaction)
        } else {
            Err(LineRuntimeError::DialogueIngressNotReady {
                activation: activation.clone(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pattern::RuntimePatternKind;
    use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimeSemanticTypeId};
    use crate::runtime_id::{
        ExecutionInstanceId, RuntimeDialogueContentPlanId, RuntimeDialogueEffectSiteId,
        RuntimeLineHandleSiteId, RuntimeLineTaskGroupId, RuntimeLocalSlotId,
        RuntimePersistentFiberId, RuntimePlanTypeId,
    };
    use crate::value::ownership::{RuntimeOwnedSlotId, RuntimeValuePath};
    use crate::value::{
        RuntimeHandleKind, RuntimeOpaquePersistence, RuntimeOpaqueValue, RuntimeOpaqueValueClass,
    };
    use std::num::{NonZeroU32, NonZeroU64};

    fn activation(occurrence: u64) -> DialogueActivationId {
        DialogueActivationId::new(
            crate::effect::RuntimeArtifactFingerprint::try_from_bytes([0x3d; 32])
                .expect("fixture artifact"),
            RuntimePersistentFiberId::from_allocated(1),
            RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            occurrence,
        )
    }

    fn frame(phase: DialogueRuntimePhase) -> DialogueActivationFrame {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        DialogueActivationFrame {
            line: crate::plan::RuntimeLineId::from_runtime_line_value("line.fixture")
                .expect("line identity"),
            content: RuntimeDialogueContentPlanId::from_accepted_ordinal(NonZeroU32::MIN),
            task_group: RuntimeLineTaskGroupId::from_zero_based(0).expect("task group"),
            resume: None,
            captures: Box::default(),
            locals: crate::value::RuntimeEnv::default(),
            line_task: DialogueLineTaskState::NotStarted,
            elapsed: LogicalDuration::default(),
            phase,
            result_target: crate::plan::RuntimeDialogueResultTarget::new(
                ty,
                RuntimePattern::from_admitted_parts(ty, RuntimePatternKind::Discard),
            ),
            voice: crate::presentation::RuntimeDialogueVoiceState::Absent,
            values: Box::default(),
            activation_pc: 0,
            pending_line_operation: None,
            failure: None,
        }
    }

    fn stage_actor_owner() -> RuntimeOpaqueTypeOwner {
        RuntimeOpaqueTypeOwner::exact_with(
            RuntimeHandleKind::StageActor
                .try_producer()
                .expect("stage actor producer"),
            RuntimeSemanticTypeId::from_bytes([0x51; 32]),
            RuntimeOpaqueValueClass::AffineHandle(RuntimeHandleKind::StageActor),
            RuntimeOpaquePersistence::SnapshotOnly,
        )
    }

    fn publish_stage_actor(
        store: &mut DialogueActivationStore,
        id: &DialogueActivationId,
    ) -> (RuntimeValue, ExecutionInstanceId) {
        let ty = RuntimePlanTypeId::from_accepted_ordinal(NonZeroU32::MIN);
        let character = arcweft_character::id::CharacterId::try_new("character.fixture")
            .expect("fixture character");
        let site = crate::line_task::RuntimeLineHandleSite::new(
            RuntimeLineHandleSiteId::from_zero_based(0),
            0,
            crate::line_task::RuntimeLineHandleSiteKind::StageActor,
            ty,
            Some(character.clone()),
            None,
            stage_actor_owner(),
        )
        .expect("stage actor site");
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Publishing))
            .expect("activation");
        let mut transaction = store.begin_transaction(id).expect("transaction");
        let mut ledger = crate::line_task::RuntimeLineHandleLedger::default();
        let opaque = ledger
            .issue(
                id,
                &site,
                crate::line_task::RuntimeHandleResource::StageActor(
                    crate::line_task::RuntimeStageActorLease::new(character),
                ),
                crate::line_task::RuntimeHandleOwnerSlot::DialogueResult(RuntimeValuePath::root()),
            )
            .expect("issued handle");
        let value = RuntimeValue::Opaque(opaque);
        let token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&value)
            .expect("handle token");
        ledger
            .set_state(
                &token,
                crate::line_task::RuntimeHandleLeaseState::Allocating,
                crate::line_task::RuntimeHandleLeaseState::Active,
            )
            .expect("active actor");
        let execution = ExecutionInstanceId::from_allocated(NonZeroU64::new(17).expect("nonzero"));
        ledger
            .transfer(
                &token,
                &crate::line_task::RuntimeHandleOwnerSlot::DialogueResult(RuntimeValuePath::root()),
                crate::line_task::RuntimeHandleOwnerSlot::ParentFiber(
                    RuntimeOwnedSlotId::EnvironmentLocal {
                        execution,
                        local: RuntimeLocalSlotId::from_allocated(
                            NonZeroU64::new(23).expect("nonzero"),
                        ),
                    },
                ),
            )
            .expect("parent transfer");
        transaction.line_mut().commit_ledger(ledger);
        transaction
            .line_mut()
            .commit_result(ty, value.clone())
            .expect("result");
        transaction
            .line_mut()
            .begin_result_publication()
            .expect("publishing");
        transaction
            .line_mut()
            .finish_result_publication()
            .expect("published");
        transaction
            .line_mut()
            .release_frame()
            .expect("frame release");
        transaction
            .stage_disposition(DialogueCommitDisposition::Published {
                resume: None,
                bindings: Vec::new(),
            })
            .expect("disposition");
        store
            .commit_published_transaction(transaction)
            .expect("published handles");
        (value, execution)
    }

    #[test]
    fn duplicate_begin_preserves_the_live_activation() {
        let id = activation(0);
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Activating))
            .expect("first activation");
        let before = store.begin_transaction(&id).expect("live transaction");

        assert_eq!(
            store.begin(id.clone(), frame(DialogueRuntimePhase::Ready)),
            Err(LineRuntimeError::DuplicateActivationLedger)
        );
        assert_eq!(store.begin_transaction(&id).expect("preserved"), before);
    }

    #[test]
    fn stale_transaction_cannot_overwrite_a_newer_commit() {
        let id = activation(1);
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Activating))
            .expect("activation");
        let first = store.begin_transaction(&id).expect("first");
        let stale = store.begin_transaction(&id).expect("stale");
        store.commit_transaction(first).expect("first commit");

        assert_eq!(
            store.commit_transaction(stale),
            Err(LineRuntimeError::StaleActivationTransaction)
        );
    }

    #[test]
    fn ingress_is_atomic_durable_and_step_scoped() {
        let id = activation(2);
        let site = RuntimeDialogueEffectSiteId::from_zero_based(0).expect("effect site");
        let event = RuntimeDialogueContentEvent::new(
            id.clone(),
            RuntimeDialogueContentEventKind::Effect(site),
        );
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Ready))
            .expect("activation");
        store
            .latch_step_input(LogicalDuration::from_nanos(7), &[event.clone()], &[], &[])
            .expect("first ingress");
        assert!(matches!(
            store.latch_step_input(LogicalDuration::from_nanos(9), &[event], &[], &[]),
            Err(DialogueIngressError {
                source: LineRuntimeError::DuplicateContentEvent { .. },
                ..
            })
        ));
        let mut transaction = store.begin_transaction(&id).expect("transaction");
        assert_eq!(transaction.frame().elapsed, LogicalDuration::from_nanos(7));
        assert_eq!(
            transaction.take_content_events(),
            vec![RuntimeDialogueContentEventKind::Effect(site)]
        );
        store
            .commit_transaction(transaction)
            .expect("consume ingress");
        assert_eq!(
            store
                .begin_transaction(&id)
                .expect("committed")
                .frame()
                .elapsed,
            LogicalDuration::from_nanos(7)
        );
    }

    #[test]
    fn activation_created_after_ingress_does_not_receive_elapsed_time() {
        let id = activation(3);
        let mut store = DialogueActivationStore::default();
        store
            .latch_step_input(LogicalDuration::from_nanos(11), &[], &[], &[])
            .expect("empty ingress");
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Ready))
            .expect("activation");
        assert_eq!(
            store
                .begin_transaction(&id)
                .expect("transaction")
                .frame()
                .elapsed,
            LogicalDuration::default()
        );
    }

    #[test]
    fn terminal_commit_requires_matching_typed_outcome() {
        let id = activation(4);
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Closing))
            .expect("activation");
        let mut mismatch = store.begin_transaction(&id).expect("mismatch");
        mismatch.line_mut().abandon().expect("abandon");
        mismatch.line_mut().release_frame().expect("release frame");
        mismatch
            .stage_disposition(DialogueCommitDisposition::Published {
                resume: None,
                bindings: Vec::new(),
            })
            .expect("stage mismatch");
        assert_eq!(
            store.commit_terminal_transaction(mismatch),
            Err(LineRuntimeError::TerminalDispositionMismatch)
        );

        let mut terminal = store.begin_transaction(&id).expect("terminal");
        terminal.line_mut().abandon().expect("abandon");
        terminal.line_mut().release_frame().expect("release frame");
        let failure = super::super::DialogueExecutionError::Line(
            LineRuntimeError::InvalidActivationOperation,
        );
        terminal.frame_mut().failure = Some(failure.clone());
        terminal
            .stage_disposition(DialogueCommitDisposition::Failed { error: failure })
            .expect("stage failure");
        assert!(matches!(
            store.commit_terminal_transaction(terminal),
            Ok(DialogueTerminalReceipt { .. })
        ));
        assert_eq!(
            store.begin_transaction(&id),
            Err(LineRuntimeError::UnknownActivationLedger)
        );
    }

    #[test]
    fn published_parent_drop_issues_and_correlates_release_before_removal() {
        let id = activation(5);
        let mut store = DialogueActivationStore::default();
        let (value, execution) = publish_stage_actor(&mut store, &id);
        let token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&value)
            .expect("published token");
        let source = RuntimeOwnedSlotId::EnvironmentLocal {
            execution,
            local: RuntimeLocalSlotId::from_allocated(NonZeroU64::new(23).expect("nonzero")),
        };
        let before = BTreeMap::from([(token, source)]);

        let before_wrong_owner = store.clone();
        assert_eq!(
            store.reconcile_parent_fiber(
                ExecutionInstanceId::from_allocated(NonZeroU64::new(18).expect("nonzero")),
                &before,
                &BTreeMap::new(),
                Some(RuntimeDropPolicy::Default),
            ),
            Err(LineRuntimeError::WrongOwner)
        );
        assert_eq!(store, before_wrong_owner);

        let commands = store
            .reconcile_parent_fiber(
                execution,
                &before,
                &BTreeMap::new(),
                Some(RuntimeDropPolicy::Default),
            )
            .expect("parent drop")
            .into_commands();
        let [
            crate::presentation::RuntimeLineHostCommand::Stage(
                crate::presentation::RuntimeStageCommand::ReleaseActor { command, actor },
            ),
        ] = commands.as_slice()
        else {
            panic!("expected one typed actor release");
        };
        assert_eq!(command.activation(), &id);
        assert_eq!(actor.activation(), &id);
        assert_eq!(
            store.begin_transaction(&id),
            Err(LineRuntimeError::ActivationFrameReleased)
        );

        let before_mismatch = store.clone();
        let mismatch = crate::presentation::RuntimeLineHostOutcome::Stage(
            crate::presentation::RuntimeStageCommandOutcome::Acquired {
                command: command.clone(),
                actor: actor.clone(),
            },
        );
        assert!(matches!(
            store.latch_step_input(LogicalDuration::default(), &[], &[], &[mismatch]),
            Err(DialogueIngressError {
                source: LineRuntimeError::StageOutcomeMismatch,
                ..
            })
        ));
        assert_eq!(store, before_mismatch);

        let released = crate::presentation::RuntimeLineHostOutcome::Stage(
            crate::presentation::RuntimeStageCommandOutcome::ReleasedActor {
                command: command.clone(),
                actor: actor.clone(),
            },
        );
        assert_eq!(
            store
                .latch_step_input(LogicalDuration::default(), &[], &[], &[released])
                .expect("release outcome")
                .into_diagnostics(),
            Vec::new()
        );
        assert_eq!(
            store.begin_transaction(&id),
            Err(LineRuntimeError::UnknownActivationLedger)
        );
    }

    #[test]
    fn parent_fiber_reconciliation_is_exact_and_noop_does_not_advance_registry() {
        let id = activation(6);
        let mut store = DialogueActivationStore::default();
        let (value, execution) = publish_stage_actor(&mut store, &id);
        let token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&value)
            .expect("published token");
        let source = RuntimeOwnedSlotId::EnvironmentLocal {
            execution,
            local: RuntimeLocalSlotId::from_allocated(NonZeroU64::new(23).expect("nonzero")),
        };
        let before = BTreeMap::from([(token.clone(), source)]);

        let unchanged = store.clone();
        assert!(
            store
                .reconcile_parent_fiber(execution, &before, &before, None)
                .expect("no-op reconciliation")
                .into_commands()
                .is_empty()
        );
        assert_eq!(store, unchanged);

        assert_eq!(
            store.reconcile_parent_fiber(execution, &before, &BTreeMap::new(), None),
            Err(LineRuntimeError::UnjournaledHandleDrop)
        );
        assert_eq!(store, unchanged);

        let destination = RuntimeOwnedSlotId::EnvironmentLocal {
            execution,
            local: RuntimeLocalSlotId::from_allocated(NonZeroU64::new(24).expect("nonzero")),
        };
        let after = BTreeMap::from([(token.clone(), destination)]);
        assert!(
            store
                .reconcile_parent_fiber(execution, &before, &after, None)
                .expect("exact parent move")
                .into_commands()
                .is_empty()
        );
        let moved = store.clone();
        assert_eq!(
            store.reconcile_parent_fiber(
                execution,
                &before,
                &BTreeMap::new(),
                Some(RuntimeDropPolicy::Default),
            ),
            Err(LineRuntimeError::WrongOwner)
        );
        assert_eq!(store, moved);
    }

    #[test]
    fn malformed_affine_payload_never_falls_back_to_string_drop() {
        let owner = stage_actor_owner();
        let malformed = RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
            &owner,
            RuntimeValue::String("legacy-handle-key".to_owned()),
        ));
        assert!(malformed.affine_line_handles().is_err());
    }
}
