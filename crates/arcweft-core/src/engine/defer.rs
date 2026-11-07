//! Native lexical defer storage and original registration evidence.

use super::code_cursor::NativeSourceOperation;
use crate::line_task::{
    AwbcRuntimeDeferredRegistrationSnapshot, RuntimeLineDeferredRegistration, ScopeExit,
};
use crate::plan::{FlowOp, RuntimeDeferOwner};
use crate::runtime_id::{RuntimeDeferRegistrationId, RuntimeDeferSiteId};
use crate::scope::RuntimeScheduledScopeToken;
use crate::task::RuntimeProgramOwner;
use crate::value::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeRetainedValueAdmission,
    RuntimeStoredValue, RuntimeValue,
};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
pub(super) enum NativeDeferScopeOwner {
    Activation,
    Control,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum NativeDeferSource {
    Activation,
    Control(NativeSourceOperation),
}

#[derive(Debug, PartialEq)]
pub(super) struct NativeLexicalDeferred<V = RuntimeValue> {
    pub(super) packet: RuntimeLineDeferredRegistration<V>,
    pub(super) source: NativeDeferSource,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeDeferInFlight {
    pub(super) id: RuntimeDeferRegistrationId,
    pub(super) site: RuntimeDeferSiteId,
    pub(super) source: NativeDeferSource,
}

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize)]
pub(crate) enum NativeDeferParent {
    Root,
    Activation,
    Control { scope: usize },
}

pub(super) enum NativeDeferScopeStep {
    Waiting,
    Deferred(super::NativeLineTaskExecutionBatch),
    Drained,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct NativeLexicalExit<V> {
    pub(super) target: usize,
    pub(super) action: NativeLexicalExitAction<V>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum NativeLexicalExitAction<V> {
    Emitted,
    Scheduled(RuntimeScheduledScopeToken),
    Bind {
        source: NativeSourceOperation,
        value: V,
    },
    Transfer(NativeLoopTransfer<V>),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct NativeLoopTransfer<V> {
    pub(super) source: NativeSourceOperation,
    pub(super) target: NativeSourceOperation,
    pub(super) kind: NativeLoopTransferKind<V>,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum NativeLoopTransferKind<V> {
    Break(V),
    Continue,
}

impl<V> NativeLoopTransferKind<V> {
    pub(super) fn misplaced_error(&self) -> crate::value::RuntimeEvalError {
        let operation = match self {
            Self::Break(_) => "break",
            Self::Continue => "continue",
        };
        crate::value::RuntimeEvalError::MisplacedLoopControl(operation)
    }
}

impl<V> NativeLexicalExitAction<V> {
    fn borrowed(&self) -> NativeLexicalExitAction<&V> {
        match self {
            Self::Emitted => NativeLexicalExitAction::Emitted,
            Self::Scheduled(token) => NativeLexicalExitAction::Scheduled(*token),
            Self::Bind { source, value } => NativeLexicalExitAction::Bind {
                source: source.clone(),
                value,
            },
            Self::Transfer(transfer) => NativeLexicalExitAction::Transfer(NativeLoopTransfer {
                source: transfer.source.clone(),
                target: transfer.target.clone(),
                kind: match &transfer.kind {
                    NativeLoopTransferKind::Break(value) => NativeLoopTransferKind::Break(value),
                    NativeLoopTransferKind::Continue => NativeLoopTransferKind::Continue,
                },
            }),
        }
    }

    fn try_map<W, E>(
        self,
        map: impl FnOnce(V) -> Result<W, E>,
    ) -> Result<NativeLexicalExitAction<W>, E> {
        Ok(match self {
            Self::Emitted => NativeLexicalExitAction::Emitted,
            Self::Scheduled(token) => NativeLexicalExitAction::Scheduled(token),
            Self::Bind { source, value } => NativeLexicalExitAction::Bind {
                source,
                value: map(value)?,
            },
            Self::Transfer(transfer) => NativeLexicalExitAction::Transfer(NativeLoopTransfer {
                source: transfer.source,
                target: transfer.target,
                kind: match transfer.kind {
                    NativeLoopTransferKind::Break(value) => {
                        NativeLoopTransferKind::Break(map(value)?)
                    }
                    NativeLoopTransferKind::Continue => NativeLoopTransferKind::Continue,
                },
            }),
        })
    }

    pub(super) fn value(&self) -> Option<&V> {
        match self {
            Self::Bind { value, .. }
            | Self::Transfer(NativeLoopTransfer {
                kind: NativeLoopTransferKind::Break(value),
                ..
            }) => Some(value),
            _ => None,
        }
    }
}

impl NativeLexicalExit<RuntimeValue> {
    pub(super) fn snapshot(
        &self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NativeLexicalExit<AwbcRuntimeValueSnapshot>, String> {
        let action = self.action.borrowed().try_map(|value| {
            AwbcRuntimeValueSnapshot::from_admitted_runtime_value(value, owner)
                .map_err(|error| error.to_string())
        })?;
        Ok(NativeLexicalExit {
            target: self.target,
            action,
        })
    }
}

impl NativeLexicalExit<AwbcRuntimeValueSnapshot> {
    pub(super) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NativeLexicalExit<RuntimeValue>, String> {
        let action = self.action.try_map(|value| {
            value
                .into_admitted_runtime_value(owner)
                .map_err(|error| error.to_string())
        })?;
        Ok(NativeLexicalExit {
            target: self.target,
            action,
        })
    }
}

impl<V: RuntimeStoredValue> NativeLexicalExit<V> {
    pub(super) fn admit(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        binding: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "native lexical exit differs from its original operation or loop target"
                .into(),
        };
        admission.validate_metadata(&self.target)?;
        let (source, transfer_target) = match &self.action {
            NativeLexicalExitAction::Emitted => return Ok(()),
            NativeLexicalExitAction::Scheduled(token) => return admission.validate_metadata(token),
            NativeLexicalExitAction::Bind { source, .. } => (source, None),
            NativeLexicalExitAction::Transfer(transfer) => {
                (&transfer.source, Some(&transfer.target))
            }
        };
        source.admit_retained(admission)?;
        if let Some(target) = transfer_target {
            target.admit_retained(admission)?;
        }
        let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            unreachable!("native position has an admitted plan");
        };
        let expected = match (&self.action, source.operation(plan)) {
            (NativeLexicalExitAction::Bind { .. }, Some(FlowOp::ExitScopeBind { expr, .. })) => {
                Some(expr.ty())
            }
            (NativeLexicalExitAction::Transfer(transfer), Some(FlowOp::Break { depth, value }))
                if matches!(transfer.kind, NativeLoopTransferKind::Break(_))
                    && source.has_loop_target(plan, *depth, &transfer.target) =>
            {
                value.as_ref().map(|expr| expr.ty())
            }
            (NativeLexicalExitAction::Transfer(transfer), Some(FlowOp::Continue { depth }))
                if matches!(transfer.kind, NativeLoopTransferKind::Continue)
                    && source.has_loop_target(plan, *depth, &transfer.target) =>
            {
                return Ok(());
            }
            _ => return Err(invalid()),
        };
        let value = self.action.value().ok_or_else(invalid)?;
        if let Some(ty) = expected {
            let expected = plan
                .type_table()
                .get(ty)
                .ok_or_else(invalid)?
                .semantic_identity();
            admission.validate_owned_typed_borrowed_in_invocation(
                value.borrowed_value(),
                expected,
                binding,
            )
        } else if matches!(
            value.borrowed_view(),
            crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::Unit)
        ) {
            admission.validate_owned_borrowed(value.borrowed_value())
        } else {
            Err(invalid())
        }
    }
}

#[derive(Debug, PartialEq)]
pub(crate) struct NativeLexicalDeferScope<V = RuntimeValue> {
    pub(super) deferred: Vec<NativeLexicalDeferred<V>>,
    pub(super) exit: Option<ScopeExit>,
    pub(super) inflight: Option<NativeDeferInFlight>,
    pub(super) failure: Option<String>,
}

pub(super) type NativeLexicalDeferScopeImage = NativeLexicalDeferScope<AwbcRuntimeValueSnapshot>;

pub(super) type NativeReachedLineDefers<'a, V> = (
    &'a crate::line_task::RuntimeDialogueActivationState<crate::runtime_id::RuntimePlanTypeId, V>,
    &'a [NativeLexicalDeferScope<V>],
);

impl<V> NativeLexicalDeferScope<V> {
    pub(super) const fn new() -> Self {
        Self {
            deferred: Vec::new(),
            exit: None,
            inflight: None,
            failure: None,
        }
    }

    pub(super) fn freeze_exit(&mut self, exit: ScopeExit) -> ScopeExit {
        *self.exit.get_or_insert(exit)
    }

    pub(super) fn ids(&self) -> impl Iterator<Item = RuntimeDeferRegistrationId> + '_ {
        self.deferred
            .iter()
            .map(|registration| registration.packet.id())
            .chain(self.inflight.iter().map(|row| row.id))
    }

    pub(super) fn has_work(&self) -> bool {
        self.exit.is_some() || !self.deferred.is_empty()
    }
}

impl super::NativeFlowExecutionFrame {
    pub(super) fn has_scope_defer_work(&self) -> bool {
        self.control_stack.iter().any(|entry| {
            matches!(&entry.kind,
            super::FlowControlStackEntryKind::Scope { defers, .. } if defers.has_work())
        })
    }
}

pub(super) enum NativeChildScopeProgress {
    None,
    Waiting,
    Pop {
        terminal: bool,
        failure: Option<String>,
    },
}

impl<V: RuntimeStoredValue> NativeLexicalDeferScope<V> {
    pub(super) fn admit_roots(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        scope_owner: NativeDeferScopeOwner,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "native lexical defer differs from its original scope and registration".into(),
        };
        if self.exit.is_none() && (self.inflight.is_some() || self.failure.is_some())
            || self
                .deferred
                .windows(2)
                .any(|rows| rows[0].packet.id() >= rows[1].packet.id())
            || self.inflight.as_ref().is_some_and(|inflight| {
                self.deferred
                    .last()
                    .is_some_and(|row| row.packet.id() >= inflight.id)
            })
        {
            return Err(invalid());
        }
        admission.validate_metadata(&(self.exit, &self.failure))?;
        admission.validate_metadata_collection(self.deferred.len())?;
        for registration in &self.deferred {
            registration.source.admit(
                admission,
                scope_owner,
                registration.packet.site(),
                Some(registration.packet.outcome_filter()),
            )?;
            registration.packet.admit_retained(admission)?;
        }
        if let Some(inflight) = &self.inflight {
            admission.validate_metadata(&(inflight.id, inflight.site))?;
            inflight
                .source
                .admit(admission, scope_owner, inflight.site, None)?;
        }
        Ok(())
    }
}

impl NativeDeferSource {
    fn admit(
        &self,
        admission: &mut RuntimeRetainedValueAdmission,
        scope_owner: NativeDeferScopeOwner,
        expected_site: RuntimeDeferSiteId,
        expected_outcome: Option<crate::line_task::RuntimeDeferOutcomeFilter>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "native lexical defer differs from its original scope and registration".into(),
        };
        match (self, scope_owner) {
            (Self::Activation, NativeDeferScopeOwner::Activation) => Ok(()),
            (Self::Control(source), NativeDeferScopeOwner::Control) => {
                source.admit_retained(admission)?;
                let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
                    return Err(invalid());
                };
                if matches!(source.operation(plan),
                    Some(FlowOp::RegisterDefer { site, outcome, owner: RuntimeDeferOwner::CurrentScope, .. })
                        if *site == expected_site && expected_outcome.is_none_or(|expected| *outcome == expected))
                {
                    Ok(())
                } else {
                    Err(invalid())
                }
            }
            _ => Err(invalid()),
        }
    }
}

impl Clone for NativeLexicalDeferScopeImage {
    fn clone(&self) -> Self {
        Self {
            deferred: self
                .deferred
                .iter()
                .map(|registration| NativeLexicalDeferred {
                    packet: registration.packet.clone(),
                    source: registration.source.clone(),
                })
                .collect(),
            exit: self.exit,
            inflight: self.inflight.clone(),
            failure: self.failure.clone(),
        }
    }
}

impl NativeLexicalDeferScopeImage {
    pub(super) fn from_live(
        scope: &NativeLexicalDeferScope,
        owner: &RuntimeProgramOwner,
    ) -> Result<Self, String> {
        Ok(Self {
            deferred: scope
                .deferred
                .iter()
                .map(|registration| {
                    let packet = AwbcRuntimeDeferredRegistrationSnapshot::from_admitted_live(
                        &registration.packet,
                        Some(owner),
                    )
                    .map_err(|error| error.to_string())?;
                    Ok(NativeLexicalDeferred {
                        packet,
                        source: registration.source.clone(),
                    })
                })
                .collect::<Result<_, String>>()?,
            exit: scope.exit,
            inflight: scope.inflight.clone(),
            failure: scope.failure.clone(),
        })
    }

    pub(super) fn into_live(
        self,
        owner: &RuntimeProgramOwner,
    ) -> Result<NativeLexicalDeferScope, String> {
        Ok(NativeLexicalDeferScope {
            deferred: self
                .deferred
                .into_iter()
                .map(|registration| {
                    let packet = registration
                        .packet
                        .into_admitted_live(owner)
                        .map_err(|error| error.to_string())?;
                    Ok(NativeLexicalDeferred {
                        packet,
                        source: registration.source,
                    })
                })
                .collect::<Result<_, String>>()?,
            exit: self.exit,
            inflight: self.inflight,
            failure: self.failure,
        })
    }
}

impl super::Engine {
    pub(super) fn request_emitted_lexical_exit(
        &mut self,
        action: NativeLexicalExitAction<RuntimeValue>,
    ) -> Result<(), NativeLexicalExitAction<RuntimeValue>> {
        use super::FlowControlStackEntryKind;
        if !matches!(self.fiber.owner, super::FlowFiberOwner::LineTask(_)) {
            return Err(action);
        }
        let mut pending = false;
        for (index, entry) in self.fiber.frame.control_stack.iter().enumerate().rev() {
            let FlowControlStackEntryKind::Scope { origin, defers, .. } = &entry.kind else {
                break;
            };
            pending |= defers.has_work();
            if origin.kind() == crate::scope::RuntimeScopeFrameKind::EmittedLexical {
                if pending {
                    self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
                        target: index,
                        action,
                    });
                    return Ok(());
                }
                break;
            }
        }
        Err(action)
    }

    pub(super) fn request_scheduled_lexical_exit(
        &mut self,
        token: RuntimeScheduledScopeToken,
    ) -> bool {
        let Some(target) = self.fiber.frame.control_stack.len().checked_sub(1) else {
            return false;
        };
        if !matches!(&self.fiber.frame.control_stack[target].kind,
            super::FlowControlStackEntryKind::Scope { defers, .. } if defers.has_work())
        {
            return false;
        }
        self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
            target,
            action: NativeLexicalExitAction::Scheduled(token),
        });
        true
    }

    pub(super) fn prepare_line_child_scope_step(
        &mut self,
        child: &mut super::FlowFiber,
        output: &mut crate::step::RuntimeStepOutput,
    ) -> Result<NativeChildScopeProgress, super::dialogue::DialogueExecutionError> {
        use super::{FlowControlStackEntryKind, FlowFiberOwner, FlowFiberStatus};
        let FlowFiberOwner::LineTask(owner) = &child.owner else {
            return Ok(NativeChildScopeProgress::None);
        };
        let terminal = owner.closing
            || matches!(
                child.frame.status,
                FlowFiberStatus::Done(_) | FlowFiberStatus::Failed(_)
            );
        if child.frame.lexical_exit.is_none()
            && (!terminal
                || (child.frame.control_stack.is_empty() && child.frame.root_cleanups.is_empty()))
        {
            return Ok(NativeChildScopeProgress::None);
        }
        let exit = match child.frame.status {
            FlowFiberStatus::Failed(_) => ScopeExit::Failed,
            _ if owner.closing => ScopeExit::Cancelled,
            _ => ScopeExit::Completed,
        };
        let Some(index) = child.frame.control_stack.len().checked_sub(1) else {
            return Ok(NativeChildScopeProgress::Pop {
                terminal,
                failure: None,
            });
        };
        let FlowControlStackEntryKind::Scope { defers, .. } =
            &mut child.frame.control_stack[index].kind
        else {
            return Ok(NativeChildScopeProgress::Pop {
                terminal,
                failure: None,
            });
        };
        if !defers.has_work() {
            return Ok(NativeChildScopeProgress::Pop {
                terminal,
                failure: None,
            });
        }
        let activation = owner.tag.activation_id();
        let mut transaction = self.dialogue_activations.begin_transaction(activation)?;
        let step = self.step_line_defer_scope(
            activation,
            transaction.line_mut(),
            defers,
            child.id,
            NativeDeferParent::Control { scope: index },
            exit,
        )?;
        let receipt = self.dialogue_activations.commit_transaction(transaction)?;
        Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        match step {
            NativeDeferScopeStep::Waiting => Ok(NativeChildScopeProgress::Waiting),
            NativeDeferScopeStep::Deferred(batch) => {
                self.commit_line_task_execution_batch(batch);
                Ok(NativeChildScopeProgress::Waiting)
            }
            NativeDeferScopeStep::Drained => {
                let failure = defers.failure.take();
                defers.exit = None;
                Ok(NativeChildScopeProgress::Pop { terminal, failure })
            }
        }
    }

    pub(super) fn complete_line_child_scope_step(
        &mut self,
        terminal: bool,
        failure: Option<String>,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        let closing = self.fiber.owner.requests_scope_close();
        let index = self.fiber.frame.control_stack.len().checked_sub(1);
        if let Some(index) = index {
            self.unwind_control_entry(output, backend)?;
            if self
                .fiber
                .frame
                .lexical_exit
                .as_ref()
                .is_some_and(|exit| exit.target == index)
            {
                let exit = self
                    .fiber
                    .frame
                    .lexical_exit
                    .take()
                    .expect("completed exact lexical exit");
                if !terminal && failure.is_none() {
                    match exit.action {
                        NativeLexicalExitAction::Emitted
                        | NativeLexicalExitAction::Scheduled(_) => {}
                        NativeLexicalExitAction::Bind { source, value } => {
                            let plan = self.plan.clone();
                            let Some(FlowOp::ExitScopeBind { pattern, .. }) =
                                source.operation(&plan)
                            else {
                                return Err(
                                    crate::scope::RuntimeScopeExitError::OriginalExitMissing.into(),
                                );
                            };
                            self.bind_value(pattern, value, output);
                        }
                        NativeLexicalExitAction::Transfer(transfer) => {
                            self.resume_loop_transfer(transfer, output, backend);
                        }
                    }
                }
            }
        } else {
            self.unwind_control_stack(output, backend)?;
        }
        if let Some(failure) = failure {
            if !matches!(self.fiber.frame.status, super::FlowFiberStatus::Failed(_)) {
                self.fiber.frame.status = super::FlowFiberStatus::Failed(failure);
            }
        }
        if terminal && self.fiber.frame.control_stack.is_empty() {
            self.fiber.frame.pending_ops.clear();
            self.fiber.frame.await_observer = None;
        }
        if closing && !matches!(self.fiber.frame.status, super::FlowFiberStatus::Failed(_)) {
            self.fiber.frame.status = super::FlowFiberStatus::Done(super::FlowExit::Done);
        }
        Ok(())
    }

    pub(super) fn register_control_defer(
        &self,
        child: &mut super::FlowFiber,
        line: &mut crate::line_task::RuntimeDialogueActivationState<
            crate::runtime_id::RuntimePlanTypeId,
        >,
        tag: &crate::line_task::LineTaskWorkTag,
        source: NativeSourceOperation,
    ) -> Result<
        std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
        super::dialogue::DialogueExecutionError,
    > {
        use super::FlowControlStackEntryKind;
        use crate::line_task::LineRuntimeError;
        let Some(FlowOp::RegisterDefer {
            site,
            outcome,
            captures,
            owner: RuntimeDeferOwner::CurrentScope,
        }) = source.operation(&self.plan)
        else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        let Some(defers) = child
            .frame
            .control_stack
            .iter_mut()
            .rev()
            .find_map(|entry| match &mut entry.kind {
                FlowControlStackEntryKind::Scope { defers, .. } => Some(defers),
                _ => None,
            })
        else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        if defers.exit.is_some() {
            return Err(LineRuntimeError::InvalidDeferredTransition.into());
        }
        let packet = register_line_defer(
            &self.plan,
            &mut child.frame.env,
            line,
            *site,
            *outcome,
            captures,
            NativeDeferCaptureOwner::Child(tag),
        )?
        .expect("child defer always registers in a lexical scope");
        let mut captured = std::collections::BTreeSet::new();
        for value in packet.captures() {
            captured.extend(
                super::dialogue::unique_affine_line_handles(value)?
                    .into_iter()
                    .map(|handle| handle.token().clone()),
            );
        }
        defers.deferred.push(NativeLexicalDeferred {
            packet,
            source: NativeDeferSource::Control(source),
        });
        Ok(captured)
    }

    pub(super) fn step_line_defer_scope(
        &self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        line: &mut crate::line_task::RuntimeDialogueActivationState<
            crate::runtime_id::RuntimePlanTypeId,
        >,
        scope: &mut NativeLexicalDeferScope,
        parent: super::FlowFiberId,
        owner: NativeDeferParent,
        exit: ScopeExit,
    ) -> Result<NativeDeferScopeStep, super::dialogue::DialogueExecutionError> {
        let exit = scope.freeze_exit(exit);
        if scope.inflight.is_some() || line.has_pending_commands() {
            return Ok(NativeDeferScopeStep::Waiting);
        }
        while let Some(registration) = scope.deferred.last() {
            let prepared = registration
                .packet
                .outcome_filter()
                .matches(exit)
                .then(|| self.inspect_deferred_line_child(&registration.packet))
                .transpose()?;
            let transition =
                line.prepare_scoped_deferred(activation_id, &registration.packet, exit)?;
            let NativeLexicalDeferred { packet, source } =
                scope.deferred.pop().expect("checked lexical registration");
            match transition {
                crate::line_task::RuntimeScopedDeferDecision::Run => {
                    scope.inflight = Some(NativeDeferInFlight {
                        id: packet.id(),
                        site: packet.site(),
                        source,
                    });
                    return Ok(NativeDeferScopeStep::Deferred(
                        self.commit_prepared_deferred_line_child(
                            parent,
                            activation_id,
                            packet,
                            prepared.expect("run child was preflighted"),
                            owner,
                        ),
                    ));
                }
                crate::line_task::RuntimeScopedDeferDecision::Skipped
                    if line.has_pending_commands() =>
                {
                    return Ok(NativeDeferScopeStep::Waiting);
                }
                crate::line_task::RuntimeScopedDeferDecision::Skipped => {}
            }
        }
        Ok(NativeDeferScopeStep::Drained)
    }
}

#[derive(Clone, Copy)]
pub(super) enum NativeDeferCaptureOwner<'a> {
    Activation(RuntimeDeferOwner),
    Child(&'a crate::line_task::LineTaskWorkTag),
}

impl NativeDeferCaptureOwner<'_> {
    fn is_scoped(self) -> bool {
        !matches!(self, Self::Activation(RuntimeDeferOwner::LineRoot))
    }
}

/// Admits every capture and ledger transfer before taking any local. The
/// actual lexical scope or line root receives the sole checked packet.
pub(super) fn register_line_defer(
    plan: &crate::plan::RuntimePlan,
    env: &mut crate::value::RuntimeEnv,
    activation: &mut crate::line_task::RuntimeDialogueActivationState<
        crate::runtime_id::RuntimePlanTypeId,
    >,
    site: RuntimeDeferSiteId,
    outcome: crate::line_task::RuntimeDeferOutcomeFilter,
    captures: &[crate::value::RuntimeExpr],
    owner: NativeDeferCaptureOwner<'_>,
) -> Result<Option<RuntimeLineDeferredRegistration>, super::dialogue::DialogueExecutionError> {
    use super::dialogue::unique_affine_line_handles;
    use crate::line_task::{LineRuntimeError, RuntimeHandleOwnerSlot};
    use crate::value::{RuntimeEvalError, RuntimeExprKind};

    let function_id = plan
        .defer_function_site(site)
        .ok_or(RuntimeEvalError::UnknownDeferredSite { site })?;
    let function = plan
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
    let mut capture_refs = Vec::with_capacity(captures.len());
    for (capture, input) in captures.iter().zip(function.capture_inputs()) {
        let RuntimeExprKind::Local(read) = capture.kind() else {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        };
        let local = read.local();
        if capture.ty() != input.pattern().ty() {
            return Err(LineRuntimeError::InvalidActivationOperation.into());
        }
        let value = env
            .get(local)
            .ok_or(RuntimeEvalError::UnknownLocal(local))?;
        capture_refs.push(value);
        if !plan
            .value_matches_type(capture.ty(), value)
            .map_err(RuntimeEvalError::from)?
        {
            return Err(RuntimeEvalError::InvalidExpressionType(capture.ty()).into());
        }
        if read.mode() == crate::value::RuntimeLocalReadMode::Copy {
            if !value.ownership().permits_copy() {
                return Err(RuntimeEvalError::AffineLocalCopy(local).into());
            }
            continue;
        }
        if !moved_locals.insert(local) {
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
            match (owner, lease.owner()) {
                (NativeDeferCaptureOwner::Activation(_), RuntimeHandleOwnerSlot::LineScope) => {
                    if owner.is_scoped() {
                        scoped_transfers
                            .push((handle.token().clone(), RuntimeHandleOwnerSlot::LineScope));
                    }
                }
                (
                    NativeDeferCaptureOwner::Activation(_),
                    source @ RuntimeHandleOwnerSlot::ActivationLocal(_),
                ) => {
                    let source = source.clone();
                    if owner.is_scoped() {
                        scoped_transfers.push((handle.token().clone(), source));
                    } else {
                        ledger.transfer(
                            handle.token(),
                            &source,
                            RuntimeHandleOwnerSlot::LineScope,
                        )?;
                    }
                }
                (
                    NativeDeferCaptureOwner::Child(tag),
                    source @ RuntimeHandleOwnerSlot::ChildScope(current),
                ) if *current == *tag => {
                    scoped_transfers.push((handle.token().clone(), source.clone()));
                }
                _ => return Err(LineRuntimeError::WrongOwner.into()),
            }
        }
    }
    let prepared = activation.inspect_deferred_registration_refs(&capture_refs)?;
    drop(capture_refs);
    for (token, source) in &scoped_transfers {
        ledger.transfer(
            token,
            source,
            RuntimeHandleOwnerSlot::ScopedDefer(prepared.id()),
        )?;
    }
    let mut values = (0..captures.len()).map(|_| None).collect::<Vec<_>>();
    for (index, capture) in captures.iter().enumerate() {
        let RuntimeExprKind::Local(read) = capture.kind() else {
            unreachable!("defer capture was checked as a local")
        };
        if read.mode() == crate::value::RuntimeLocalReadMode::Copy {
            values[index] = Some(
                env.get(read.local())
                    .expect("checked copied defer local remains present")
                    .clone(),
            );
        }
    }
    for (index, capture) in captures.iter().enumerate() {
        let RuntimeExprKind::Local(read) = capture.kind() else {
            unreachable!("defer capture was checked as a local")
        };
        if read.mode() == crate::value::RuntimeLocalReadMode::Move {
            values[index] = Some(
                env.take(read.local())
                    .expect("checked moved defer local remains present"),
            );
        }
    }
    let values = values
        .into_iter()
        .map(|value| value.expect("checked defer read has one mode"))
        .collect::<Vec<_>>();
    let scoped_registration = match owner.is_scoped() {
        true => Some(activation.commit_deferred_registration(site, outcome, values, prepared)),
        false => {
            activation.register_deferred_prepared(site, outcome, values, prepared);
            None
        }
    };
    activation.commit_ledger(ledger);
    Ok(scoped_registration)
}
