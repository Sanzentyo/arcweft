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

mod flow_scope;

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

pub(super) struct NativeDeferScopeProgress {
    pub(super) step: NativeDeferScopeStep,
    pub(super) received: Option<crate::line_task::PreparedRuntimeParentFiberReconciliation>,
}

pub(super) struct NativeDeferScopeContext {
    pub(super) parent: super::FlowFiberId,
    pub(super) execution: crate::runtime_id::ExecutionInstanceId,
    pub(super) owner: NativeDeferParent,
}

pub(super) struct NativeDeferRegistrationReceipt {
    pub(super) local_tokens: std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    pub(super) received: Option<crate::line_task::PreparedRuntimeParentFiberReconciliation>,
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
    Return {
        source: NativeSourceOperation,
        result: NativeReturnResult<V>,
    },
    Fallthrough {
        site: crate::runtime_id::RuntimeFunctionSiteId,
    },
    Goto {
        source: NativeGotoOrigin,
        transfer: crate::plan::RuntimeFlowTransfer<V>,
        source_instantiation:
            Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        type_instantiation:
            Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum NativeGotoOrigin {
    Operation(NativeSourceOperation),
    Choice {
        source: NativeSourceOperation,
        option: usize,
    },
}

impl From<NativeSourceOperation> for NativeGotoOrigin {
    fn from(source: NativeSourceOperation) -> Self {
        Self::Operation(source)
    }
}

impl NativeGotoOrigin {
    pub(super) fn operation(&self) -> &NativeSourceOperation {
        match self {
            Self::Operation(source) | Self::Choice { source, .. } => source,
        }
    }

    fn admit_transfer<V: RuntimeStoredValue>(
        &self,
        transfer: &crate::plan::RuntimeFlowTransfer<V>,
        source_instantiation: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        type_instantiation: Option<&crate::program_types::RuntimeFunctionEffectInstantiation>,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        use crate::plan::RuntimeFlowTransferTarget;
        self.operation().admit_retained(admission)?;
        let invalid = || AwbcRuntimeValueSnapshotError::Message {
            message: "Native transfer inputs differ from their original source and destination ABI"
                .into(),
        };
        let RuntimeFlowTransferTarget::Static(destination) = &transfer.target else {
            return Err(invalid());
        };
        admission.validate_metadata(destination)?;
        admission.validate_metadata_collection(transfer.arguments.len())?;
        admission.validate_metadata(&source_instantiation)?;
        admission.validate_metadata(&type_instantiation)?;
        if let Self::Choice { option, .. } = self {
            admission.validate_metadata(option)?;
        }
        let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
            return Err(invalid());
        };
        let plan = std::sync::Arc::clone(plan);
        match self.operation().owner() {
            crate::plan::code::RuntimeCodeOwner::Function(site) => {
                plan.validate_function_instantiation(site, source_instantiation, &admission.work())
                    .map_err(|_| invalid())?;
            }
            crate::plan::code::RuntimeCodeOwner::LineTask { .. } => {
                if source_instantiation.is_some_and(|binding| {
                    !binding.is_valid_with_work(plan.as_ref(), &admission.work())
                }) {
                    return Err(invalid());
                }
            }
        }
        let original = match (self, self.operation().operation(&plan)) {
            (Self::Operation(_), Some(FlowOp::Goto(original))) => {
                let target_agrees = match &original.target {
                    RuntimeFlowTransferTarget::Static(target) => target == destination,
                    RuntimeFlowTransferTarget::Dynamic(_) => true,
                };
                if !target_agrees
                    || original.arguments.len() != transfer.arguments.len()
                    || !original
                        .arguments
                        .iter()
                        .zip(&transfer.arguments)
                        .all(|(source, value)| source.parameter == value.parameter)
                {
                    return Err(invalid());
                }
                original.validate_input_sources().map_err(|_| invalid())?;
                Some(original)
            }
            (
                Self::Operation(_),
                Some(FlowOp::Effect(crate::effect::LineEffectRequest::Goto(label))),
            ) if transfer.arguments.is_empty()
                && plan
                    .resolve_flow_target_value(label)
                    .is_ok_and(|target| target == *destination) =>
            {
                None
            }
            (Self::Choice { option, .. }, Some(FlowOp::Choice { options, .. }))
                if transfer.arguments.is_empty()
                    && options
                        .get(*option)
                        .and_then(|option| option.target.as_ref())
                        == Some(destination) =>
            {
                None
            }
            _ => return Err(invalid()),
        };
        plan.admit_retained_flow_transfer_inputs(
            destination,
            transfer,
            type_instantiation,
            admission,
        )?;
        if let Some(original) = original {
            for (source, argument) in original.arguments.iter().zip(&transfer.arguments) {
                admission.validate_metadata(&argument.parameter)?;
                let expected = plan
                    .type_table()
                    .get(source.value.ty())
                    .ok_or_else(invalid)?
                    .semantic_identity();
                admission.validate_admitted_typed_borrowed_in_invocation(
                    argument.value.borrowed_value(),
                    expected,
                    source_instantiation,
                )?;
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeDeferBoundary {
    Invocation,
    Flow,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum NativeReturnResult<V> {
    FlowLabel(String),
    Value(V),
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
    pub(super) fn pending_return_value(&self) -> Option<&V> {
        match self {
            Self::Return {
                result: NativeReturnResult::Value(value),
                ..
            } => Some(value),
            _ => None,
        }
    }

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
            Self::Return { source, result } => NativeLexicalExitAction::Return {
                source: source.clone(),
                result: match result {
                    NativeReturnResult::FlowLabel(label) => {
                        NativeReturnResult::FlowLabel(label.clone())
                    }
                    NativeReturnResult::Value(value) => NativeReturnResult::Value(value),
                },
            },
            Self::Fallthrough { site } => NativeLexicalExitAction::Fallthrough { site: *site },
            Self::Goto {
                source,
                transfer,
                source_instantiation,
                type_instantiation,
            } => NativeLexicalExitAction::Goto {
                source: source.clone(),
                transfer: transfer.borrowed(),
                source_instantiation: source_instantiation.clone(),
                type_instantiation: type_instantiation.clone(),
            },
        }
    }

    fn try_map<W, E>(
        self,
        mut map: impl FnMut(V) -> Result<W, E>,
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
            Self::Return { source, result } => NativeLexicalExitAction::Return {
                source,
                result: match result {
                    NativeReturnResult::FlowLabel(label) => NativeReturnResult::FlowLabel(label),
                    NativeReturnResult::Value(value) => NativeReturnResult::Value(map(value)?),
                },
            },
            Self::Fallthrough { site } => NativeLexicalExitAction::Fallthrough { site },
            Self::Goto {
                source,
                transfer,
                source_instantiation,
                type_instantiation,
            } => NativeLexicalExitAction::Goto {
                source,
                transfer: transfer.try_map(map)?,
                source_instantiation,
                type_instantiation,
            },
        })
    }

    pub(super) fn value(&self) -> Option<&V> {
        match self {
            Self::Bind { value, .. }
            | Self::Return {
                result: NativeReturnResult::Value(value),
                ..
            }
            | Self::Transfer(NativeLoopTransfer {
                kind: NativeLoopTransferKind::Break(value),
                ..
            }) => Some(value),
            _ => None,
        }
    }

    /// Pending transfer operands remain in authored order. They are distinct
    /// owners even when a receiving formal pattern discards its binding.
    pub(super) fn values(&self) -> impl Iterator<Item = &V> {
        let transfer = match self {
            Self::Goto { transfer, .. } => Some(transfer),
            _ => None,
        };
        self.value().into_iter().chain(
            transfer
                .into_iter()
                .flat_map(|transfer| transfer.arguments.iter().map(|argument| &argument.value)),
        )
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
            message: "native lexical exit differs from its original operation or control target"
                .into(),
        };
        admission.validate_metadata(&self.target)?;
        if let NativeLexicalExitAction::Fallthrough { site } = &self.action {
            admission.validate_metadata(&site.get())?;
            let Some(RuntimeProgramOwner::Plan(plan)) = admission.program_owner() else {
                return Err(invalid());
            };
            return plan
                .function_sites()
                .get(*site)
                .filter(|function| {
                    function.role().allows_implicit_unit_completion()
                        && matches!(
                            plan.checked_type(function.result()),
                            Ok(Some(crate::pattern::RuntimeCheckedType::Unit))
                        )
                })
                .map(|_| ())
                .ok_or_else(invalid);
        }
        if let NativeLexicalExitAction::Goto {
            source,
            transfer,
            source_instantiation,
            type_instantiation,
        } = &self.action
        {
            return source.admit_transfer(
                transfer,
                source_instantiation.as_deref(),
                type_instantiation.as_deref(),
                admission,
            );
        }
        let (source, transfer_target) = match &self.action {
            NativeLexicalExitAction::Emitted => return Ok(()),
            NativeLexicalExitAction::Scheduled(token) => return admission.validate_metadata(token),
            NativeLexicalExitAction::Bind { source, .. }
            | NativeLexicalExitAction::Return { source, .. } => (source, None),
            NativeLexicalExitAction::Goto { .. } => unreachable!("admitted goto above"),
            NativeLexicalExitAction::Fallthrough { .. } => {
                unreachable!("admitted fallthrough above")
            }
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
            (NativeLexicalExitAction::Return { result, .. }, Some(FlowOp::Return(text))) => {
                return match result {
                    NativeReturnResult::FlowLabel(label) if label == text => {
                        admission.validate_metadata(label)
                    }
                    NativeReturnResult::Value(value) if matches!(value.borrowed_view(), crate::value::RuntimeValueView::Scalar(crate::value::RuntimeScalarView::String(label)) if label == text) => {
                        admission.validate_owned_borrowed(value.borrowed_value())
                    }
                    _ => Err(invalid()),
                };
            }
            (NativeLexicalExitAction::Return { result, .. }, Some(FlowOp::ReturnExpr(expr))) => {
                if let NativeReturnResult::FlowLabel(label) = result {
                    return admission.validate_metadata(label);
                }
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

    pub(super) fn scope_defer_exit_pending(&self) -> bool {
        self.lexical_exit.is_some()
            || (matches!(
                self.status,
                super::FlowFiberStatus::Done(_) | super::FlowFiberStatus::Failed(_)
            ) && self.has_scope_defer_work())
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
        admission.validate_metadata(&(self.exit, &self.failure))?;
        admission.validate_metadata_collection(self.deferred.len())?;
        if self.exit.is_none() && (self.inflight.is_some() || self.failure.is_some())
            || !crate::scope::defer_registration_stack_is_ordered(
                self.deferred.iter().map(|row| row.packet.id()),
                self.inflight.as_ref().map(|row| row.id),
            )
        {
            return Err(invalid());
        }
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
    pub(super) fn return_from_source(
        &mut self,
        source: NativeSourceOperation,
        value: RuntimeValue,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) {
        let result = if self.fiber.frame.active_function().is_some()
            || matches!(self.fiber.launch, super::FlowFiberLaunch::Program(_))
        {
            NativeReturnResult::Value(value)
        } else {
            NativeReturnResult::FlowLabel(match value {
                RuntimeValue::String(label) => label,
                value => crate::value::runtime_value_label(&value),
            })
        };
        if let Some(target) = self
            .fiber
            .frame
            .defer_target(NativeDeferBoundary::Invocation)
        {
            self.fiber.frame.lexical_exit = Some(NativeLexicalExit {
                target,
                action: NativeLexicalExitAction::Return { source, result },
            });
        } else {
            self.complete_source_return(result, output, backend);
        }
    }

    fn complete_source_return(
        &mut self,
        result: NativeReturnResult<RuntimeValue>,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) {
        match result {
            NativeReturnResult::FlowLabel(label) => self.return_value(label, output, backend),
            NativeReturnResult::Value(value) => {
                let unhandled = self.return_typed_value(value, output, backend);
                debug_assert!(
                    unhandled.is_none(),
                    "the returning invocation remains active"
                );
            }
        }
    }

    pub(super) fn request_emitted_lexical_exit(
        &mut self,
        action: NativeLexicalExitAction<RuntimeValue>,
    ) -> Result<(), NativeLexicalExitAction<RuntimeValue>> {
        use super::FlowControlStackEntryKind;
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
            NativeDeferScopeContext {
                parent: child.id,
                execution: child.execution,
                owner: NativeDeferParent::Control { scope: index },
            },
            exit,
        )?;
        transaction.stage_received_handles(step.received);
        let receipt = self.dialogue_activations.commit_transaction(transaction)?;
        Self::publish_dialogue_line_receipt(receipt.into_line(), output);
        match step.step {
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

    pub(super) fn complete_deferred_scope_step(
        &mut self,
        terminal: bool,
        failure: Option<String>,
        output: &mut crate::step::RuntimeStepOutput,
        backend: &mut impl crate::pure::RuntimeCallBackend,
    ) -> Result<(), crate::value::RuntimeEvalError> {
        let closing = self.fiber.owner.requests_scope_close();
        let index = self.fiber.frame.control_stack.len().checked_sub(1);
        if let Some(index) = index {
            // A typed result stays in the continuation until its receiving
            // invocation can take it. Relocate only leases owned by this scope.
            if let Some(NativeLexicalExit {
                action:
                    NativeLexicalExitAction::Return { .. } | NativeLexicalExitAction::Goto { .. },
                ..
            }) = &self.fiber.frame.lexical_exit
            {
                if terminal || failure.is_some() {
                    let exit = self
                        .fiber
                        .frame
                        .lexical_exit
                        .take()
                        .expect("reached return");
                    if let Err(error) =
                        self.discard_pending_values_presentation(exit.action.values())
                    {
                        self.fiber.frame.lexical_exit = Some(exit);
                        return Err(error);
                    }
                } else if matches!(
                    self.fiber.frame.control_stack[index].kind,
                    super::FlowControlStackEntryKind::Scope { .. }
                        | super::FlowControlStackEntryKind::FormatAttempt(
                            super::NativeFormatAttemptFrame {
                                active: Some(_),
                                ..
                            }
                        )
                ) {
                    let exit = self
                        .fiber
                        .frame
                        .lexical_exit
                        .take()
                        .expect("reached return");
                    let transition = self.preserve_presentation_at_scope_exit(exit.action.values());
                    self.fiber.frame.lexical_exit = Some(exit);
                    transition?;
                }
            }
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
                        NativeLexicalExitAction::Return { result, .. } => {
                            self.complete_source_return(result, output, backend);
                        }
                        NativeLexicalExitAction::Fallthrough { .. } => {
                            if self.fiber.frame.active_function().is_some() {
                                self.complete_function_call_fallthrough(output, backend);
                            } else {
                                self.finish(output, backend);
                            }
                        }
                        NativeLexicalExitAction::Goto {
                            source,
                            transfer,
                            type_instantiation,
                            source_instantiation: _,
                        } => {
                            self.activate_flow_transfer(
                                transfer,
                                Some(&source),
                                type_instantiation,
                                output,
                                backend,
                            );
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
    ) -> Result<NativeDeferRegistrationReceipt, super::dialogue::DialogueExecutionError> {
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
        let source_slots = defer_source_slots(child.execution, &child.frame, captures)?;
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
        let mut prepared = inspect_line_defer(
            &self.plan,
            &child.frame.env,
            line,
            *site,
            captures,
            NativeDeferCaptureOwner::Child(tag),
            |handle, ordinal| {
                self.dialogue_activations.inspect_received_defer_capture(
                    self.published_receiver_execution(),
                    handle,
                    source_slots[ordinal],
                )
            },
        )?;
        let received = if prepared.received_captures.is_empty() {
            None
        } else {
            let mut before = std::collections::BTreeMap::new();
            let mut after = std::collections::BTreeMap::new();
            for (token, ordinal) in &prepared.received_captures {
                before.insert(token.clone(), source_slots[*ordinal]);
                after.insert(
                    token.clone(),
                    crate::value::ownership::RuntimeOwnedSlotId::LineDeferCapture {
                        execution: child.execution,
                        activation: *tag.activation_id(),
                        registration: prepared.registration.id(),
                        ordinal: u32::try_from(*ordinal)
                            .map_err(|_| LineRuntimeError::OwnedSlotOverflow)?,
                    },
                );
            }
            Some(
                self.dialogue_activations
                    .inspect_parent_fiber_reconciliation(
                        self.published_receiver_execution(),
                        &before,
                        &after,
                        &Default::default(),
                    )?,
            )
        };
        let captured = std::mem::take(&mut prepared.local_tokens);
        let packet = prepared
            .commit(&mut child.frame.env, line, *site, *outcome, captures)
            .expect("child defer always registers in a lexical scope");
        defers.deferred.push(NativeLexicalDeferred {
            packet,
            source: NativeDeferSource::Control(source),
        });
        Ok(NativeDeferRegistrationReceipt {
            local_tokens: captured,
            received,
        })
    }

    pub(super) fn step_line_defer_scope(
        &self,
        activation_id: &crate::runtime_id::DialogueActivationId,
        line: &mut crate::line_task::RuntimeDialogueActivationState<
            crate::runtime_id::RuntimePlanTypeId,
        >,
        scope: &mut NativeLexicalDeferScope,
        context: NativeDeferScopeContext,
        exit: ScopeExit,
    ) -> Result<NativeDeferScopeProgress, super::dialogue::DialogueExecutionError> {
        let exit = scope.freeze_exit(exit);
        if scope.inflight.is_some() || line.has_pending_commands() {
            return Ok(NativeDeferScopeProgress {
                step: NativeDeferScopeStep::Waiting,
                received: None,
            });
        }
        while let Some(registration) = scope.deferred.last() {
            let before = registration
                .packet
                .received_handle_slots(context.execution, activation_id)?;
            let prepared = registration
                .packet
                .outcome_filter()
                .matches(exit)
                .then(|| self.inspect_deferred_line_child(&registration.packet))
                .transpose()?;
            let received = if before.is_empty() {
                None
            } else {
                let after = prepared
                    .as_ref()
                    .map(|child| child.received_handle_slots(activation_id))
                    .unwrap_or_default();
                let mut drops = crate::line_task::RuntimeHandleDropAuthorization::default();
                if prepared.is_none() {
                    drops.authorize_tokens(before.keys().cloned())?;
                }
                Some(
                    self.dialogue_activations
                        .inspect_parent_fiber_reconciliation(
                            self.published_receiver_execution(),
                            &before,
                            &after,
                            &drops,
                        )?,
                )
            };
            let transition = line.inspect_scoped_defer_unwind_with(
                activation_id,
                registration.packet.id(),
                registration.packet.outcome_filter(),
                registration.packet.captures(),
                exit,
                |handle, registration, ordinal| {
                    self.dialogue_activations.inspect_received_defer_capture(
                        self.published_receiver_execution(),
                        handle,
                        crate::value::ownership::RuntimeOwnedSlotId::LineDeferCapture {
                            execution: context.execution,
                            activation: *activation_id,
                            registration,
                            ordinal,
                        },
                    )
                },
            )?;
            let decision = transition.decision();
            line.commit_scoped_defer_unwind(transition);
            let NativeLexicalDeferred { packet, source } =
                scope.deferred.pop().expect("checked lexical registration");
            match decision {
                crate::line_task::RuntimeScopedDeferDecision::Run => {
                    scope.inflight = Some(NativeDeferInFlight {
                        id: packet.id(),
                        site: packet.site(),
                        source,
                    });
                    return Ok(NativeDeferScopeProgress {
                        step: NativeDeferScopeStep::Deferred(
                            self.commit_prepared_deferred_line_child(
                                context.parent,
                                activation_id,
                                packet,
                                prepared.expect("run child was preflighted"),
                                context.owner,
                            ),
                        ),
                        received,
                    });
                }
                crate::line_task::RuntimeScopedDeferDecision::Skipped
                    if line.has_pending_commands() || received.is_some() =>
                {
                    return Ok(NativeDeferScopeProgress {
                        step: NativeDeferScopeStep::Waiting,
                        received,
                    });
                }
                crate::line_task::RuntimeScopedDeferDecision::Skipped => {}
            }
        }
        Ok(NativeDeferScopeProgress {
            step: NativeDeferScopeStep::Drained,
            received: None,
        })
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

fn inspect_defer_captures<'a>(
    plan: &crate::plan::RuntimePlan,
    env: &'a crate::value::RuntimeEnv,
    site: RuntimeDeferSiteId,
    captures: &[crate::value::RuntimeExpr],
) -> Result<Vec<&'a RuntimeValue>, super::dialogue::DialogueExecutionError> {
    use crate::line_task::LineRuntimeError;
    use crate::value::{RuntimeEvalError, RuntimeExprKind, RuntimeLocalReadMode};
    let function = plan
        .defer_function_site(site)
        .and_then(|id| plan.function_sites().get(id))
        .ok_or(RuntimeEvalError::UnknownDeferredSite { site })?;
    if captures.len() != function.capture_inputs().count() {
        return Err(LineRuntimeError::InvalidActivationOperation.into());
    }
    let mut moved_locals = std::collections::BTreeSet::new();
    captures
        .iter()
        .zip(function.capture_inputs())
        .map(|(capture, input)| {
            let RuntimeExprKind::Local(read) = capture.kind() else {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            };
            if capture.ty() != input.pattern().ty() {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let local = read.local();
            let value = env
                .get(local)
                .ok_or(RuntimeEvalError::UnknownLocal(local))?;
            if !plan
                .value_matches_type(capture.ty(), value)
                .map_err(RuntimeEvalError::from)?
            {
                return Err(RuntimeEvalError::InvalidExpressionType(capture.ty()).into());
            }
            if read.mode() == RuntimeLocalReadMode::Copy {
                if !value.ownership().permits_copy() {
                    return Err(RuntimeEvalError::AffineLocalCopy(local).into());
                }
            } else if !moved_locals.insert(local) {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            Ok(value)
        })
        .collect()
}

fn take_defer_captures(
    env: &mut crate::value::RuntimeEnv,
    captures: &[crate::value::RuntimeExpr],
) -> Vec<RuntimeValue> {
    use crate::value::{RuntimeExprKind, RuntimeLocalReadMode};
    let reads = || {
        captures.iter().map(|capture| {
            let RuntimeExprKind::Local(read) = capture.kind() else {
                unreachable!("defer capture was checked as a local");
            };
            read
        })
    };
    let mut values = reads()
        .map(|read| {
            (read.mode() == RuntimeLocalReadMode::Copy).then(|| {
                env.get(read.local())
                    .expect("checked copied defer local remains present")
                    .clone()
            })
        })
        .collect::<Vec<_>>();
    for (read, value) in reads().zip(&mut values) {
        if read.mode() == RuntimeLocalReadMode::Move {
            *value = Some(
                env.take(read.local())
                    .expect("checked moved defer local remains present"),
            );
        }
    }
    values
        .into_iter()
        .map(|value| value.expect("checked defer read has one mode"))
        .collect()
}

/// Admits every capture and ledger transfer before taking any local. The
/// actual lexical scope or line root receives the sole checked packet.
pub(super) fn defer_source_slots(
    execution: crate::runtime_id::ExecutionInstanceId,
    frame: &super::NativeFlowExecutionFrame,
    captures: &[crate::value::RuntimeExpr],
) -> Result<Vec<crate::value::ownership::RuntimeOwnedSlotId>, crate::line_task::LineRuntimeError> {
    captures
        .iter()
        .map(|capture| {
            let crate::value::RuntimeExprKind::Local(read) = capture.kind() else {
                return Err(crate::line_task::LineRuntimeError::InvalidActivationOperation);
            };
            Ok(frame.owned_local_slot(execution, read.local()))
        })
        .collect()
}

pub(super) struct NativePreparedLineDefer {
    registration: crate::scope::RuntimePreparedDeferredRegistration,
    ledger: crate::line_task::RuntimeLineHandleLedger,
    owner: RuntimeDeferOwner,
    local_tokens: std::collections::BTreeSet<crate::runtime_id::RuntimeLineHandleToken>,
    received_captures: std::collections::BTreeMap<crate::runtime_id::RuntimeLineHandleToken, usize>,
}

impl NativePreparedLineDefer {
    pub(super) fn commit(
        self,
        env: &mut crate::value::RuntimeEnv,
        activation: &mut crate::line_task::RuntimeDialogueActivationState<
            crate::runtime_id::RuntimePlanTypeId,
        >,
        site: RuntimeDeferSiteId,
        outcome: crate::line_task::RuntimeDeferOutcomeFilter,
        captures: &[crate::value::RuntimeExpr],
    ) -> Option<RuntimeLineDeferredRegistration> {
        let values = take_defer_captures(env, captures);
        let packet = match self.owner {
            RuntimeDeferOwner::CurrentScope => Some(activation.commit_deferred_registration(
                site,
                outcome,
                values,
                self.registration,
            )),
            RuntimeDeferOwner::LineRoot => {
                activation.register_deferred_prepared(site, outcome, values, self.registration);
                None
            }
        };
        activation.commit_ledger(self.ledger);
        packet
    }
}

pub(super) fn inspect_line_defer(
    plan: &crate::plan::RuntimePlan,
    env: &crate::value::RuntimeEnv,
    activation: &crate::line_task::RuntimeDialogueActivationState<
        crate::runtime_id::RuntimePlanTypeId,
    >,
    site: RuntimeDeferSiteId,
    captures: &[crate::value::RuntimeExpr],
    owner: NativeDeferCaptureOwner<'_>,
    inspect_received: impl FnMut(
        &crate::value::ownership::RuntimeAffineLineHandle,
        usize,
    ) -> Result<(), crate::line_task::LineRuntimeError>,
) -> Result<NativePreparedLineDefer, super::dialogue::DialogueExecutionError> {
    use super::dialogue::unique_affine_line_handles;
    use crate::line_task::{LineRuntimeError, RuntimeHandleOwnerSlot};
    use crate::value::{RuntimeExprKind, RuntimeLocalReadMode};

    let capture_refs = inspect_defer_captures(plan, env, site, captures)?;
    let mut ledger = activation.ledger().clone();
    let mut captured_handles = std::collections::BTreeSet::new();
    let mut scoped_transfers = Vec::new();
    let mut received_captures = std::collections::BTreeMap::new();
    for (ordinal, (capture, value)) in captures.iter().zip(&capture_refs).enumerate() {
        let RuntimeExprKind::Local(read) = capture.kind() else {
            unreachable!("defer capture was checked as a local");
        };
        if read.mode() == RuntimeLocalReadMode::Copy {
            continue;
        }
        for handle in unique_affine_line_handles(value)? {
            if !captured_handles.insert(handle.token().clone()) {
                return Err(LineRuntimeError::InvalidActivationOperation.into());
            }
            let Some(lease) = ledger.lease(handle.token()) else {
                // The registration preflight below authenticates received
                // handles through their issuer and original capture slot.
                received_captures.insert(handle.token().clone(), ordinal);
                continue;
            };
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
    let prepared =
        activation.inspect_deferred_registration_with(&capture_refs, inspect_received)?;
    drop(capture_refs);
    for (token, source) in &scoped_transfers {
        ledger.transfer(
            token,
            source,
            RuntimeHandleOwnerSlot::ScopedDefer(prepared.id()),
        )?;
    }
    Ok(NativePreparedLineDefer {
        registration: prepared,
        ledger,
        owner: if owner.is_scoped() {
            RuntimeDeferOwner::CurrentScope
        } else {
            RuntimeDeferOwner::LineRoot
        },
        local_tokens: captured_handles
            .into_iter()
            .filter(|token| !received_captures.contains_key(token))
            .collect(),
        received_captures,
    })
}
