#[cfg(test)]
use crate::effect::RuntimeDropPolicy;
use crate::engine::DialogueRuntimePhase;
use crate::line_task::{
    LineRuntimeError, LineTaskLiveState, RuntimeDialogueAbandonedCommitProof,
    RuntimeDialogueActivationRegistry, RuntimeDialogueActivationState,
    RuntimeDialogueActivationTransaction, RuntimeDialogueCommitProof, RuntimeDialogueCommitReceipt,
    RuntimeDialoguePublishedCommitProof, RuntimeHandleDropReceipt,
};
use crate::pattern::RuntimePattern;
use crate::runtime_id::{DialogueActivationId, RuntimeLocalDeclarationId, RuntimePlanTypeId};
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
#[derive(Debug, PartialEq)]
pub(crate) struct DialogueActivationFrame {
    pub(in crate::engine) line: crate::plan::RuntimeLineId,
    pub(in crate::engine) content: crate::runtime_id::RuntimeDialogueContentPlanId,
    pub(in crate::engine) target: Option<crate::value::RuntimeOpaqueValue>,
    pub(in crate::engine) task_group: crate::runtime_id::RuntimeLineTaskGroupId,
    pub(in crate::engine) resume: Option<super::super::FlowCursor>,
    /// Source-order capture coordinates; the values themselves live only in
    /// `locals` and may be affine while activation executes.
    pub(in crate::engine) captures: Box<[RuntimeLocalDeclarationId]>,
    /// Copyable external and activation-local inputs for unscheduled line work.
    pub(in crate::engine) task_inputs: Box<[RuntimeLocalBinding]>,
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
    /// Exact callback closures captured when this dialogue content was
    /// activated. Reveal selects these rows by effect site; it never scans
    /// ordinary value slots for a callback.
    pub(in crate::engine) effect_callbacks:
        Box<[crate::value::RuntimeDialogueContentEffectBinding]>,
    pub(in crate::engine) activation_pc: usize,
    /// An `out` has staged the line result and exited the line-plan
    /// continuation. Only active lexical scopes may still unwind before reveal.
    pub(in crate::engine) exiting_for_result: bool,
    /// Pre-reveal lexical scopes, retained with the activation transaction
    /// across host commands and deferred-child suspension.
    pub(in crate::engine) scopes: Vec<DialogueActivationScope>,
    pub(in crate::engine) pending_line_operation: Option<PendingLineOperation>,
    pub(in crate::engine) pending_host_call: Option<PendingActivationHostCall>,
    pub(in crate::engine) failure: Option<super::DialogueExecutionError>,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDialogueBindingRollbackImage {
    local: RuntimeLocalDeclarationId,
    value: crate::value::AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDialogueValueRollbackImage {
    slot: crate::runtime_id::RuntimeDialogueValueSlotId,
    role: crate::plan::RuntimeDialogueValueRole,
    value: crate::value::AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDialogueEffectCallbackRollbackImage {
    site: crate::runtime_id::RuntimeDialogueEffectSiteId,
    callback: crate::value::AwbcRuntimeValueSnapshot,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDialogueScopeRollbackImage {
    deferred: Vec<crate::line_task::AwbcRuntimeDeferredRegistrationSnapshot>,
    exit: Option<crate::line_task::ScopeExit>,
    inflight: Option<(
        crate::runtime_id::RuntimeDeferRegistrationId,
        crate::runtime_id::RuntimeDeferSiteId,
    )>,
}

#[derive(Clone, Debug, PartialEq)]
enum NativePendingLineOperationRollbackImage {
    AcquireActor {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        value: crate::value::AwbcRuntimeValueSnapshot,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    ActorLook {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        value: crate::value::AwbcRuntimeValueSnapshot,
        token: crate::runtime_id::RuntimeLineHandleToken,
    },
    StartVoice {
        command: crate::presentation::RuntimeLineCommandId,
        binding: Option<RuntimePattern>,
        site: crate::runtime_id::RuntimeLineHandleSiteId,
    },
}

#[derive(Clone, Debug, PartialEq)]
struct NativeDialogueActivationFrameRollbackImage {
    line: crate::plan::RuntimeLineId,
    content: crate::runtime_id::RuntimeDialogueContentPlanId,
    target: Option<crate::value::AwbcRuntimeValueSnapshot>,
    task_group: crate::runtime_id::RuntimeLineTaskGroupId,
    resume: Option<super::super::FlowCursor>,
    captures: Box<[RuntimeLocalDeclarationId]>,
    task_inputs: Vec<NativeDialogueBindingRollbackImage>,
    locals: crate::value::RuntimeEnvRollbackImage,
    line_task: DialogueLineTaskState,
    elapsed: crate::time::LogicalDuration,
    phase: DialogueRuntimePhase,
    result_target: crate::plan::RuntimeDialogueResultTarget,
    voice: crate::presentation::RuntimeDialogueVoiceState,
    values: Vec<NativeDialogueValueRollbackImage>,
    effect_callbacks: Vec<NativeDialogueEffectCallbackRollbackImage>,
    activation_pc: usize,
    exiting_for_result: bool,
    scopes: Vec<NativeDialogueScopeRollbackImage>,
    pending_line_operation: Option<NativePendingLineOperationRollbackImage>,
    pending_host_call: Option<PendingActivationHostCall>,
    failure: Option<super::DialogueExecutionError>,
}

#[derive(Clone, Debug, PartialEq)]
struct NativeEngineDialogueFrameRollbackImage {
    frame: NativeDialogueActivationFrameRollbackImage,
    inbox: DialogueStepInbox,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DialogueActivationStoreRollbackImage {
    registry: crate::line_task::RuntimeDialogueRegistrySaveSnapshot<
        NativeEngineDialogueFrameRollbackImage,
        RuntimePlanTypeId,
    >,
}

fn inert_dialogue_value(
    value: &RuntimeValue,
    owner: &crate::task::RuntimeProgramOwner,
) -> Result<crate::value::AwbcRuntimeValueSnapshot, String> {
    crate::value::AwbcRuntimeValueSnapshot::from_runtime_value_for_program(value, owner)
        .map_err(|error| error.to_string())
}

fn live_dialogue_value(
    image: crate::value::AwbcRuntimeValueSnapshot,
    owner: &crate::task::RuntimeProgramOwner,
) -> Result<RuntimeValue, String> {
    image
        .into_runtime_value_for_program(owner)
        .map_err(|error| error.to_string())
}

impl NativeDialogueBindingRollbackImage {
    fn from_live(
        binding: &RuntimeLocalBinding,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            local: binding.local,
            value: inert_dialogue_value(&binding.value, owner)?,
        })
    }

    fn into_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<RuntimeLocalBinding, String> {
        Ok(RuntimeLocalBinding {
            local: self.local,
            value: live_dialogue_value(self.value, owner)?,
        })
    }
}

impl NativePendingLineOperationRollbackImage {
    fn from_live(
        pending: &PendingLineOperation,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(match pending {
            PendingLineOperation::AcquireActor {
                command,
                binding,
                value,
                token,
            } => Self::AcquireActor {
                command: command.clone(),
                binding: binding.clone(),
                value: inert_dialogue_value(value, owner)?,
                token: token.clone(),
            },
            PendingLineOperation::ActorLook {
                command,
                binding,
                value,
                token,
            } => Self::ActorLook {
                command: command.clone(),
                binding: binding.clone(),
                value: inert_dialogue_value(value, owner)?,
                token: token.clone(),
            },
            PendingLineOperation::StartVoice {
                command,
                binding,
                site,
            } => Self::StartVoice {
                command: command.clone(),
                binding: binding.clone(),
                site: *site,
            },
        })
    }

    fn into_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<PendingLineOperation, String> {
        Ok(match self {
            Self::AcquireActor {
                command,
                binding,
                value,
                token,
            } => PendingLineOperation::AcquireActor {
                command,
                binding,
                value: live_dialogue_value(value, owner)?,
                token,
            },
            Self::ActorLook {
                command,
                binding,
                value,
                token,
            } => PendingLineOperation::ActorLook {
                command,
                binding,
                value: live_dialogue_value(value, owner)?,
                token,
            },
            Self::StartVoice {
                command,
                binding,
                site,
            } => PendingLineOperation::StartVoice {
                command,
                binding,
                site,
            },
        })
    }
}

impl NativeDialogueScopeRollbackImage {
    fn from_live(
        scope: &DialogueActivationScope,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            deferred: scope
                .deferred
                .iter()
                .map(|registration| {
                    crate::line_task::AwbcRuntimeDeferredRegistrationSnapshot::from_live_for_program(
                        registration, owner,
                    )
                    .map_err(|error| error.to_string())
                })
                .collect::<Result<_, String>>()?,
            exit: scope.exit,
            inflight: scope.inflight,
        })
    }

    fn into_live(
        self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<DialogueActivationScope, String> {
        Ok(DialogueActivationScope {
            deferred: self
                .deferred
                .into_iter()
                .map(|registration| {
                    registration
                        .into_live(owner)
                        .map_err(|error| error.to_string())
                })
                .collect::<Result<_, String>>()?,
            exit: self.exit,
            inflight: self.inflight,
        })
    }
}

impl DialogueActivationFrame {
    fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<NativeDialogueActivationFrameRollbackImage, String> {
        Ok(NativeDialogueActivationFrameRollbackImage {
            line: self.line.clone(),
            content: self.content,
            target: self
                .target
                .as_ref()
                .map(|target| {
                    crate::value::AwbcRuntimeValueSnapshot::from_opaque_for_program(target, owner)
                        .map_err(|error| error.to_string())
                })
                .transpose()?,
            task_group: self.task_group,
            resume: self.resume,
            captures: self.captures.clone(),
            task_inputs: self
                .task_inputs
                .iter()
                .map(|binding| NativeDialogueBindingRollbackImage::from_live(binding, owner))
                .collect::<Result<_, String>>()?,
            locals: self.locals.inert_rollback_image(owner)?,
            line_task: self.line_task.clone(),
            elapsed: self.elapsed,
            phase: self.phase,
            result_target: self.result_target.clone(),
            voice: self.voice.clone(),
            values: self
                .values
                .iter()
                .map(|binding| {
                    Ok(NativeDialogueValueRollbackImage {
                        slot: binding.slot,
                        role: binding.role,
                        value: inert_dialogue_value(&binding.value, owner)?,
                    })
                })
                .collect::<Result<_, String>>()?,
            effect_callbacks: self
                .effect_callbacks
                .iter()
                .map(|binding| {
                    Ok(NativeDialogueEffectCallbackRollbackImage {
                        site: binding.site(),
                        callback:
                            crate::value::AwbcRuntimeValueSnapshot::from_callable_for_program(
                                binding.callback(),
                                owner,
                            )
                            .map_err(|error| error.to_string())?,
                    })
                })
                .collect::<Result<_, String>>()?,
            activation_pc: self.activation_pc,
            exiting_for_result: self.exiting_for_result,
            scopes: self
                .scopes
                .iter()
                .map(|scope| NativeDialogueScopeRollbackImage::from_live(scope, owner))
                .collect::<Result<_, String>>()?,
            pending_line_operation: self
                .pending_line_operation
                .as_ref()
                .map(|pending| NativePendingLineOperationRollbackImage::from_live(pending, owner))
                .transpose()?,
            pending_host_call: self.pending_host_call.clone(),
            failure: self.failure.clone(),
        })
    }

    fn from_rollback_image(
        image: NativeDialogueActivationFrameRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<Self, String> {
        let target = image
            .target
            .map(|saved| {
                let RuntimeValue::Opaque(value) = live_dialogue_value(saved, owner)? else {
                    return Err("dialogue rollback target is not opaque".to_owned());
                };
                Ok(value)
            })
            .transpose()?;
        Ok(Self {
            line: image.line,
            content: image.content,
            target,
            task_group: image.task_group,
            resume: image.resume,
            captures: image.captures,
            task_inputs: image
                .task_inputs
                .into_iter()
                .map(|binding| binding.into_live(owner))
                .collect::<Result<Vec<_>, String>>()?
                .into_boxed_slice(),
            locals: crate::value::RuntimeEnv::from_rollback_image(image.locals, owner)?,
            line_task: image.line_task,
            elapsed: image.elapsed,
            phase: image.phase,
            result_target: image.result_target,
            voice: image.voice,
            values: image
                .values
                .into_iter()
                .map(|binding| {
                    Ok(crate::plan::RuntimeDialogueValueBinding {
                        slot: binding.slot,
                        role: binding.role,
                        value: live_dialogue_value(binding.value, owner)?,
                    })
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_boxed_slice(),
            effect_callbacks: image
                .effect_callbacks
                .into_iter()
                .map(|binding| {
                    let RuntimeValue::Callable(callback) =
                        live_dialogue_value(binding.callback, owner)?
                    else {
                        return Err("dialogue rollback callback is not callable".to_owned());
                    };
                    Ok(crate::value::RuntimeDialogueContentEffectBinding::new(
                        binding.site,
                        callback,
                    ))
                })
                .collect::<Result<Vec<_>, String>>()?
                .into_boxed_slice(),
            activation_pc: image.activation_pc,
            exiting_for_result: image.exiting_for_result,
            scopes: image
                .scopes
                .into_iter()
                .map(|scope| scope.into_live(owner))
                .collect::<Result<_, String>>()?,
            pending_line_operation: image
                .pending_line_operation
                .map(|pending| pending.into_live(owner))
                .transpose()?,
            pending_host_call: image.pending_host_call,
            failure: image.failure,
        })
    }
}

impl DialogueActivationFrame {
    pub(in crate::engine) fn task_inputs_for_reveal(
        &self,
        exports: &[RuntimeLocalDeclarationId],
    ) -> Result<Box<[RuntimeLocalBinding]>, LineRuntimeError> {
        let mut inputs = Vec::with_capacity(self.captures.len() + exports.len());
        for local in self.captures.iter().chain(exports.iter()) {
            let value = self
                .locals
                .get(*local)
                .ok_or(LineRuntimeError::UnknownOwnedLocal { local: *local })?;
            if !value.ownership().permits_copy() {
                return Err(LineRuntimeError::AffineGroupCapture);
            }
            inputs.push(RuntimeLocalBinding {
                local: *local,
                value: value.clone(),
            });
        }
        Ok(inputs.into_boxed_slice())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PendingActivationHostCall {
    pub(in crate::engine) id: crate::step::RuntimeHostCallId,
    pub(in crate::engine) result: RuntimePlanTypeId,
    pub(in crate::engine) binding: Option<RuntimePattern>,
}

#[derive(Debug, PartialEq)]
pub(crate) struct DialogueActivationScope {
    pub(in crate::engine) deferred: Vec<crate::line_task::RuntimeLineDeferredRegistration>,
    /// Frozen on first exit. A failing cleanup must not change its filter.
    pub(in crate::engine) exit: Option<crate::line_task::ScopeExit>,
    pub(in crate::engine) inflight: Option<(
        crate::runtime_id::RuntimeDeferRegistrationId,
        crate::runtime_id::RuntimeDeferSiteId,
    )>,
}

impl DialogueActivationScope {
    pub(in crate::engine) fn new() -> Self {
        Self {
            deferred: Vec::new(),
            exit: None,
            inflight: None,
        }
    }

    pub(in crate::engine) fn freeze_exit(&mut self, exit: crate::line_task::ScopeExit) {
        self.exit.get_or_insert(exit);
    }
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

#[derive(Debug, PartialEq)]
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

/// Sole engine execution owner of dialogue frames and their line-runtime
/// transaction state. Fiber suspension retains only the activation identity
/// and its published execution phase, never the activation's values.
#[derive(Debug, Default, PartialEq)]
pub(crate) struct DialogueActivationStore {
    registry: RuntimeDialogueActivationRegistry<EngineDialogueActivationFrame, RuntimePlanTypeId>,
}

impl DialogueActivationStore {
    pub(crate) fn into_published(
        self,
    ) -> Result<crate::line_task::RuntimePublishedDialogueRegistry, (Self, LineRuntimeError)> {
        self.registry
            .into_published()
            .map_err(|(registry, reason)| (Self { registry }, reason))
    }
    pub(crate) fn from_published(
        custody: crate::line_task::RuntimePublishedDialogueRegistry,
    ) -> Self {
        Self {
            registry: custody.into_registry(),
        }
    }

    pub(crate) fn inspect_commit_transaction(
        &self,
        transaction: &DialogueActivationTransaction,
    ) -> Result<RuntimeDialogueCommitProof, LineRuntimeError> {
        if transaction.disposition.is_some() {
            return Err(LineRuntimeError::UnexpectedTerminalDisposition);
        }
        self.registry.inspect_commit(&transaction.inner)
    }

    pub(crate) fn commit_prepared(
        &mut self,
        transaction: DialogueActivationTransaction,
        proof: RuntimeDialogueCommitProof,
    ) -> DialogueCommitReceipt {
        assert!(transaction.disposition.is_none());
        DialogueCommitReceipt {
            line: self
                .registry
                .commit_prepared(transaction.inner, proof)
                .into_line(),
        }
    }

    pub(crate) fn restore_rejected_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
    ) -> Result<(), LineRuntimeError> {
        assert!(
            !matches!(
                transaction.disposition,
                Some(DialogueCommitDisposition::Published { .. })
            ),
            "published bindings require their prepared owning commit"
        );
        self.registry.restore_transaction(transaction.inner)
    }

    pub(crate) fn inspect_terminal_transaction(
        &self,
        transaction: &DialogueActivationTransaction,
    ) -> Result<RuntimeDialogueAbandonedCommitProof, LineRuntimeError> {
        if !matches!(
            transaction.disposition.as_ref(),
            Some(DialogueCommitDisposition::Failed { .. })
        ) {
            return Err(LineRuntimeError::TerminalDispositionMismatch);
        }
        self.registry.inspect_abandoned(&transaction.inner)
    }

    pub(crate) fn commit_terminal_prepared(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        proof: RuntimeDialogueAbandonedCommitProof,
    ) -> DialogueTerminalReceipt {
        let disposition = transaction
            .disposition
            .take()
            .expect("prepared failure close has one disposition");
        let line = self
            .registry
            .commit_abandoned_prepared(transaction.inner, proof)
            .into_line();
        DialogueTerminalReceipt { line, disposition }
    }

    pub(crate) fn inspect_published_transaction(
        &self,
        transaction: &DialogueActivationTransaction,
    ) -> Result<RuntimeDialoguePublishedCommitProof, LineRuntimeError> {
        if transaction.disposition.is_some() {
            return Err(LineRuntimeError::UnexpectedTerminalDisposition);
        }
        self.registry.inspect_published(&transaction.inner)
    }

    pub(crate) fn commit_published_prepared(
        &mut self,
        mut transaction: DialogueActivationTransaction,
        proof: RuntimeDialoguePublishedCommitProof,
    ) -> DialogueTerminalReceipt {
        let disposition = transaction
            .disposition
            .take()
            .expect("prepared publication has one disposition");
        assert!(matches!(
            disposition,
            DialogueCommitDisposition::Published { .. }
        ));
        let line = self
            .registry
            .commit_published_prepared(transaction.inner, proof)
            .into_line();
        DialogueTerminalReceipt { line, disposition }
    }

    pub(crate) fn active_line(
        &self,
        activation: &DialogueActivationId,
    ) -> Option<&RuntimeDialogueActivationState<RuntimePlanTypeId>> {
        self.registry.active_line(activation)
    }

    pub(crate) fn inert_rollback_image(
        &self,
        owner: &crate::task::RuntimeProgramOwner,
    ) -> Result<DialogueActivationStoreRollbackImage, String> {
        let registry = self
            .registry
            .to_rollback_snapshot(owner, |active| {
                Ok(NativeEngineDialogueFrameRollbackImage {
                    frame: active
                        .frame
                        .inert_rollback_image(owner)
                        .map_err(|message| {
                            crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                message,
                            }
                        })?,
                    inbox: active.inbox.clone(),
                })
            })
            .map_err(|error| error.to_string())?;
        Ok(DialogueActivationStoreRollbackImage { registry })
    }

    pub(crate) fn from_rollback_image(
        image: DialogueActivationStoreRollbackImage,
        owner: &crate::task::RuntimeProgramOwner,
        expected_deferred_children: &BTreeMap<
            DialogueActivationId,
            (
                crate::runtime_id::RuntimeDeferRegistrationId,
                crate::runtime_id::RuntimeDeferSiteId,
            ),
        >,
    ) -> Result<Self, String> {
        let registry =
            RuntimeDialogueActivationRegistry::from_save_snapshot_with_deferred_children(
                image.registry,
                owner,
                expected_deferred_children,
                |_, active, _| {
                    Ok(EngineDialogueActivationFrame {
                        frame: DialogueActivationFrame::from_rollback_image(active.frame, owner)
                            .map_err(|message| {
                                crate::line_task::RuntimeDialogueRegistrySnapshotError::Frame {
                                    message,
                                }
                            })?,
                        inbox: active.inbox,
                    })
                },
            )
            .map_err(|error| error.to_string())?;
        Ok(Self { registry })
    }
}

#[derive(Debug, PartialEq)]
struct EngineDialogueActivationFrame {
    frame: DialogueActivationFrame,
    inbox: DialogueStepInbox,
}

/// Opaque optimistic transaction over one complete dialogue activation.
/// The key and revision cannot be mixed with another frame or line component.
#[derive(Debug, PartialEq)]
pub(crate) struct DialogueActivationTransaction {
    inner: RuntimeDialogueActivationTransaction<EngineDialogueActivationFrame, RuntimePlanTypeId>,
    disposition: Option<DialogueCommitDisposition>,
}

#[derive(Debug, PartialEq)]
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

#[derive(Debug, PartialEq)]
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
        &mut self,
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
        let published = self
            .registry
            .stage_published_outcomes(line_outcomes)
            .map_err(|source| DialogueIngressError {
                activation: None,
                source,
            })?;
        let mut revision_steps = BTreeMap::<DialogueActivationId, u64>::new();
        for activation in self.registry.active_ids() {
            let frame = self
                .registry
                .active_frame(&activation)
                .expect("active id has frame");
            if frame.frame.phase == DialogueRuntimePhase::Ready
                && frame.frame.elapsed.checked_add(dt).is_none()
            {
                return Err(DialogueIngressError::for_activation(
                    &activation,
                    LineRuntimeError::DialogueElapsedOverflow,
                ));
            }
            *revision_steps.entry(activation).or_default() += 1;
        }
        let mut seen_events = Vec::new();
        for event in content_events {
            let activation = event.activation();
            let frame = self.registry.active_frame(activation).ok_or_else(|| {
                DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DialogueIngressNotReady {
                        activation: activation.clone(),
                    },
                )
            })?;
            if frame.frame.phase != DialogueRuntimePhase::Ready {
                return Err(DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DialogueIngressNotReady {
                        activation: activation.clone(),
                    },
                ));
            }
            let kind = event.kind();
            if frame.inbox.content_events.contains(&kind)
                || seen_events.contains(&(activation.clone(), kind))
            {
                return Err(DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DuplicateContentEvent { event: kind },
                ));
            }
            seen_events.push((activation.clone(), kind));
            *revision_steps.entry(activation.clone()).or_default() += 1;
        }
        let mut seen_advances = Vec::new();
        for activation in advances {
            let frame = self.registry.active_frame(activation).ok_or_else(|| {
                DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DialogueIngressNotReady {
                        activation: activation.clone(),
                    },
                )
            })?;
            if frame.frame.phase != DialogueRuntimePhase::Ready {
                return Err(DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DialogueIngressNotReady {
                        activation: activation.clone(),
                    },
                ));
            }
            if frame.inbox.advance || seen_advances.contains(activation) {
                return Err(DialogueIngressError::for_activation(
                    activation,
                    LineRuntimeError::DuplicateDialogueAdvance {
                        activation: activation.clone(),
                    },
                ));
            }
            seen_advances.push(activation.clone());
            *revision_steps.entry(activation.clone()).or_default() += 1;
        }
        let mut seen_outcomes: Vec<crate::presentation::RuntimeLineCommandId> = Vec::new();
        for outcome in line_outcomes {
            let command = outcome.command();
            if self.registry.is_published(command.activation()) {
                continue;
            }
            let frame = self
                .registry
                .active_frame(command.activation())
                .ok_or_else(|| {
                    DialogueIngressError::for_activation(
                        command.activation(),
                        LineRuntimeError::UnknownActivationLedger,
                    )
                })?;
            if frame
                .inbox
                .line_outcomes
                .iter()
                .any(|pending| pending.command() == command)
                || seen_outcomes.contains(&command)
            {
                return Err(DialogueIngressError::for_activation(
                    command.activation(),
                    LineRuntimeError::DuplicateCommandOutcome,
                ));
            }
            seen_outcomes.push(command.clone());
            *revision_steps
                .entry(command.activation().clone())
                .or_default() += 1;
        }
        for (activation, steps) in revision_steps {
            let revision = self
                .registry
                .active_revision(&activation)
                .expect("preflight active revision");
            if revision.checked_add(steps).is_none() {
                return Err(DialogueIngressError::for_activation(
                    &activation,
                    LineRuntimeError::ActivationTransactionRevisionOverflow,
                ));
            }
        }
        let next = self;
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
        receipt
            .diagnostics
            .extend(next.registry.commit_published_outcomes(published));
        Ok(receipt)
    }

    pub(in crate::engine) fn reconcile_parent_fiber(
        &mut self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<RuntimeHandleDropReceipt, LineRuntimeError> {
        self.registry
            .reconcile_parent_fiber(execution, before, after, drops)
    }

    pub(in crate::engine) fn inspect_parent_fiber_reconciliation(
        &self,
        execution: crate::runtime_id::ExecutionInstanceId,
        before: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        after: &BTreeMap<crate::runtime_id::RuntimeLineHandleToken, RuntimeOwnedSlotId>,
        drops: &crate::line_task::RuntimeHandleDropAuthorization,
    ) -> Result<crate::line_task::PreparedRuntimeParentFiberReconciliation, LineRuntimeError> {
        self.registry
            .inspect_parent_fiber_reconciliation(execution, before, after, drops)
    }

    pub(crate) fn commit_transaction(
        &mut self,
        transaction: DialogueActivationTransaction,
    ) -> Result<DialogueCommitReceipt, LineRuntimeError> {
        assert!(
            transaction.disposition.is_none(),
            "ordinary dialogue commit cannot discard a terminal disposition"
        );
        Ok(DialogueCommitReceipt {
            line: self.registry.commit(transaction.inner)?.into_line(),
        })
    }

    fn ready_transaction(
        &mut self,
        activation: &DialogueActivationId,
    ) -> Result<DialogueActivationTransaction, LineRuntimeError> {
        if self
            .registry
            .active_frame(activation)
            .is_none_or(|frame| frame.frame.phase != DialogueRuntimePhase::Ready)
        {
            Err(LineRuntimeError::DialogueIngressNotReady {
                activation: activation.clone(),
            })
        } else {
            self.begin_transaction(activation)
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
            target: Some(RuntimeOpaqueValue::new_exact(
                &RuntimeOpaqueTypeOwner::exact(
                    crate::value::RuntimeCharacterDialogueProducerId::get(),
                    RuntimeSemanticTypeId::from_bytes([0x47; 32]),
                ),
                crate::value::RuntimeValue::Unit,
            )),
            task_group: RuntimeLineTaskGroupId::from_zero_based(0).expect("task group"),
            resume: None,
            captures: Box::default(),
            task_inputs: Box::default(),
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
            effect_callbacks: Box::default(),
            activation_pc: 0,
            exiting_for_result: false,
            scopes: Vec::new(),
            pending_line_operation: None,
            pending_host_call: None,
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

    #[test]
    fn reveal_task_inputs_keep_external_order_and_reject_affine_exports() {
        let external = RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::MIN);
        let export = RuntimeLocalDeclarationId::from_accepted_ordinal(
            NonZeroU32::new(2).expect("nonzero local"),
        );
        let mut state = frame(DialogueRuntimePhase::Activating);
        state.captures = vec![external].into_boxed_slice();
        state.locals.set(external, RuntimeValue::Unit);
        state.locals.set(export, RuntimeValue::Bool(true));
        let inputs = state
            .task_inputs_for_reveal(&[export])
            .expect("copyable activation local is available to line work");
        assert_eq!(
            inputs.iter().map(|row| row.local).collect::<Vec<_>>(),
            [external, export]
        );
        assert_eq!(inputs[1].value, RuntimeValue::Bool(true));

        state.locals.set(
            export,
            RuntimeValue::Opaque(RuntimeOpaqueValue::new_exact(
                &stage_actor_owner(),
                RuntimeValue::Unit,
            )),
        );
        assert_eq!(
            state.task_inputs_for_reveal(&[export]),
            Err(LineRuntimeError::AffineGroupCapture),
        );
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
            .commit_result(ty, value)
            .expect("result");
        transaction
            .line_mut()
            .begin_result_publication()
            .expect("publishing");
        let proof = store
            .inspect_published_transaction(&transaction)
            .expect("publication proof");
        let (_, value) = transaction
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
        store.commit_published_prepared(transaction, proof);
        (value, execution)
    }

    fn published_registry_snapshot(
        store: &DialogueActivationStore,
    ) -> crate::line_task::RuntimeDialogueRegistrySaveSnapshot<(), RuntimePlanTypeId> {
        store
            .registry
            .to_save_snapshot(|_| panic!("published fixture has no active frame"))
            .expect("published metadata snapshot")
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
        assert_eq!(
            store.begin_transaction(&id),
            Err(LineRuntimeError::StaleActivationTransaction)
        );
        store
            .commit_transaction(before)
            .expect("first owner commits");
        assert_eq!(
            store
                .begin_transaction(&id)
                .expect("preserved")
                .frame()
                .phase,
            DialogueRuntimePhase::Activating
        );
    }

    #[test]
    fn in_flight_transaction_excludes_a_second_owner() {
        let id = activation(1);
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Activating))
            .expect("activation");
        let first = store.begin_transaction(&id).expect("first");
        assert_eq!(
            store.begin_transaction(&id),
            Err(LineRuntimeError::StaleActivationTransaction)
        );
        store.commit_transaction(first).expect("first commit");
        let next = store.begin_transaction(&id).expect("next sole owner");
        store.commit_transaction(next).expect("next commit");
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
            .latch_step_input(
                LogicalDuration::from_nanos(7),
                std::slice::from_ref(&event),
                &[],
                &[],
            )
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
    fn terminal_preflight_preserves_the_owner_for_matching_disposition() {
        let id = activation(4);
        let mut store = DialogueActivationStore::default();
        store
            .begin(id.clone(), frame(DialogueRuntimePhase::Closing))
            .expect("activation");
        let mut mismatch = store.begin_transaction(&id).expect("mismatch");
        mismatch.line_mut().abandon().expect("abandon");
        mismatch
            .stage_disposition(DialogueCommitDisposition::Published {
                resume: None,
                bindings: Vec::new(),
            })
            .expect("stage mismatch");
        assert!(matches!(
            store.inspect_terminal_transaction(&mismatch),
            Err(LineRuntimeError::TerminalDispositionMismatch)
        ));
        mismatch.disposition = None;
        let failure = super::super::DialogueExecutionError::Line(
            LineRuntimeError::InvalidActivationOperation,
        );
        mismatch.frame_mut().failure = Some(failure.clone());
        mismatch
            .stage_disposition(DialogueCommitDisposition::Failed { error: failure })
            .expect("stage failure");
        let proof = store
            .inspect_terminal_transaction(&mismatch)
            .expect("terminal proof");
        mismatch.line_mut().release_frame().expect("release frame");
        let _ = store.commit_terminal_prepared(mismatch, proof);
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

        let before_wrong_owner = published_registry_snapshot(&store);
        assert_eq!(
            store.reconcile_parent_fiber(
                ExecutionInstanceId::from_allocated(NonZeroU64::new(18).expect("nonzero")),
                &before,
                &BTreeMap::new(),
                &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                    RuntimeDropPolicy::Default
                )),
            ),
            Err(LineRuntimeError::WrongOwner)
        );
        assert_eq!(published_registry_snapshot(&store), before_wrong_owner);

        let commands = store
            .reconcile_parent_fiber(
                execution,
                &before,
                &BTreeMap::new(),
                &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                    RuntimeDropPolicy::Default,
                )),
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

        let before_mismatch = published_registry_snapshot(&store);
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
        assert_eq!(published_registry_snapshot(&store), before_mismatch);

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

        let unchanged = published_registry_snapshot(&store);
        assert!(
            store
                .reconcile_parent_fiber(execution, &before, &before, &Default::default())
                .expect("no-op reconciliation")
                .into_commands()
                .is_empty()
        );
        assert_eq!(published_registry_snapshot(&store), unchanged);

        let forged = BTreeMap::from([(
            token.clone(),
            RuntimeOwnedSlotId::EnvironmentLocal {
                execution,
                local: RuntimeLocalSlotId::from_allocated(NonZeroU64::new(99).expect("nonzero")),
            },
        )]);
        assert_eq!(
            store.reconcile_parent_fiber(execution, &forged, &forged, &Default::default()),
            Err(LineRuntimeError::WrongOwner)
        );
        assert_eq!(published_registry_snapshot(&store), unchanged);

        assert_eq!(
            store.reconcile_parent_fiber(execution, &before, &BTreeMap::new(), &Default::default()),
            Err(LineRuntimeError::UnjournaledHandleDrop)
        );
        assert_eq!(published_registry_snapshot(&store), unchanged);

        let destination = RuntimeOwnedSlotId::EnvironmentLocal {
            execution,
            local: RuntimeLocalSlotId::from_allocated(NonZeroU64::new(24).expect("nonzero")),
        };
        let after = BTreeMap::from([(token.clone(), destination)]);
        assert!(
            store
                .reconcile_parent_fiber(execution, &before, &after, &Default::default())
                .expect("exact parent move")
                .into_commands()
                .is_empty()
        );
        let moved = published_registry_snapshot(&store);
        assert_eq!(
            store.reconcile_parent_fiber(
                execution,
                &before,
                &BTreeMap::new(),
                &crate::line_task::RuntimeHandleDropAuthorization::at_boundary(Some(
                    RuntimeDropPolicy::Default
                )),
            ),
            Err(LineRuntimeError::WrongOwner)
        );
        assert_eq!(published_registry_snapshot(&store), moved);
    }

    #[test]
    fn partial_record_replacement_drops_only_remaining_handles() {
        let mut store = DialogueActivationStore::default();
        let (remaining, execution) = publish_stage_actor(&mut store, &activation(51));
        let (moved, _) = publish_stage_actor(&mut store, &activation(52));
        let (new, _) = publish_stage_actor(&mut store, &activation(53));
        let token = |value: &RuntimeValue| {
            crate::line_task::RuntimeLineHandleLedger::token_from_value(value).unwrap()
        };
        let remaining_token = token(&remaining);
        let moved_token = token(&moved);
        let new_token = token(&new);
        let local = |ordinal| {
            RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(ordinal).unwrap())
        };
        let destination_local = local(23);
        let incoming_local = local(24);
        let moved_local = local(25);
        let destination = RuntimeOwnedSlotId::environment_local(execution, destination_local);
        let incoming = RuntimeOwnedSlotId::environment_local(execution, incoming_local);
        let moved_destination = RuntimeOwnedSlotId::environment_local(execution, moved_local);
        let original = BTreeMap::from([
            (remaining_token.clone(), destination),
            (moved_token.clone(), destination),
            (new_token.clone(), destination),
        ]);
        let before = BTreeMap::from([
            (remaining_token.clone(), destination),
            (moved_token.clone(), moved_destination),
            (new_token.clone(), incoming),
        ]);
        store
            .reconcile_parent_fiber(execution, &original, &before, &Default::default())
            .unwrap();
        let mut env = crate::value::RuntimeEnv::default();
        env.set(
            destination_local,
            RuntimeValue::try_record(vec![
                ("remaining".into(), remaining),
                ("moved".into(), moved),
            ])
            .unwrap(),
        );
        env.set(incoming_local, new);
        let read = crate::value::RuntimeLocalRead::from_admitted_place(
            destination_local,
            crate::value::RuntimeLocalReadMode::Move,
            vec![crate::value::RuntimeRecordFieldId::try_from_zero_based_ordinal(1).unwrap()]
                .into_boxed_slice(),
        );
        let moved = env.read(&read).unwrap();
        env.set(moved_local, moved);
        assert!(env.get(destination_local).is_none());
        let new = env.take(incoming_local).unwrap();
        let discarded = env
            .assign_runtime_place(
                crate::value::RuntimeMutablePlace::Local(destination_local),
                new,
            )
            .unwrap();
        assert_eq!(discarded.len(), 1);
        assert_eq!(
            discarded[0].affine_line_handles().unwrap()[0].token(),
            &remaining_token
        );
        let drops = env.take_assignment_discard_authorization();
        assert_eq!(
            drops.policy_for(&remaining_token),
            Some(RuntimeDropPolicy::Default)
        );
        assert!(drops.policy_for(&moved_token).is_none());
        assert!(drops.policy_for(&new_token).is_none());
        let after = BTreeMap::from([
            (moved_token.clone(), moved_destination),
            (new_token.clone(), destination),
        ]);
        let commands = store
            .reconcile_parent_fiber(execution, &before, &after, &drops)
            .unwrap()
            .into_commands();
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], crate::presentation::RuntimeLineHostCommand::Stage(
            crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. }) if actor == &remaining_token)
        );
        assert_eq!(token(env.get(moved_local).unwrap()), moved_token);
    }

    #[test]
    fn replacement_discards_only_the_old_nested_graph_and_transfers_the_new_owner() {
        let mut store = DialogueActivationStore::default();
        let (old, execution) = publish_stage_actor(&mut store, &activation(41));
        let (new, _) = publish_stage_actor(&mut store, &activation(42));
        let old_token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&old).unwrap();
        let new_token = crate::line_task::RuntimeLineHandleLedger::token_from_value(&new).unwrap();
        let destination_local =
            RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(23).unwrap());
        let incoming_local =
            RuntimeLocalDeclarationId::from_accepted_ordinal(NonZeroU32::new(24).unwrap());
        let destination = RuntimeOwnedSlotId::environment_local(execution, destination_local);
        let incoming = RuntimeOwnedSlotId::environment_local(execution, incoming_local);
        store
            .reconcile_parent_fiber(
                execution,
                &BTreeMap::from([(new_token.clone(), destination)]),
                &BTreeMap::from([(new_token.clone(), incoming)]),
                &Default::default(),
            )
            .unwrap();
        let before = BTreeMap::from([
            (old_token.clone(), destination),
            (new_token.clone(), incoming),
        ]);
        let mut env = crate::value::RuntimeEnv::default();
        env.set(
            destination_local,
            RuntimeValue::Tuple(vec![RuntimeValue::Tuple(vec![old])]),
        );
        env.set(
            incoming_local,
            RuntimeValue::Tuple(vec![RuntimeValue::Tuple(vec![new])]),
        );
        let value = env.take(incoming_local).unwrap();
        let displaced = env
            .assign_runtime_place(
                crate::value::RuntimeMutablePlace::Local(destination_local),
                value,
            )
            .unwrap();
        assert!(
            !displaced
                .first()
                .expect("live assignment displaces the old owner")
                .affine_line_handles()
                .unwrap()
                .is_empty()
        );
        let drops = env.take_assignment_discard_authorization();
        let after = BTreeMap::from([(new_token.clone(), destination)]);
        assert_eq!(
            drops.policy_for(&old_token),
            Some(RuntimeDropPolicy::Default)
        );
        assert!(drops.policy_for(&new_token).is_none());
        let published = published_registry_snapshot(&store);
        assert_eq!(
            store.reconcile_parent_fiber(execution, &before, &BTreeMap::new(), &drops),
            Err(LineRuntimeError::UnjournaledHandleDrop)
        );
        assert_eq!(published_registry_snapshot(&store), published);
        let commands = store
            .reconcile_parent_fiber(execution, &before, &after, &drops)
            .unwrap()
            .into_commands();
        assert_eq!(commands.len(), 1);
        assert!(
            matches!(&commands[0], crate::presentation::RuntimeLineHostCommand::Stage(
            crate::presentation::RuntimeStageCommand::ReleaseActor { actor, .. }) if actor == &old_token)
        );
        assert!(env.get(incoming_local).is_none());
        assert!(env.get(destination_local).is_some());
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
