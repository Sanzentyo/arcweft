//! Actual execution authority borrowed for one VM slice, never a saved owner.

use super::{
    AwbcProgram, AwbcRegisterId, FiberState, FiberStateError, VmError, VmExecutionContext,
};
use crate::presentation::resources::{PresentationBanks, PresentationBody};
use crate::value::RuntimePayload;

pub(super) struct VmExecutionResources<'execution> {
    pub(super) context: Option<&'execution VmExecutionContext>,
    pub(super) presentation:
        Option<&'execution mut Option<PresentationBanks<PresentationBody<RuntimePayload>>>>,
}

impl VmExecutionResources<'_> {
    pub(super) fn banks_mut(
        &mut self,
    ) -> Option<&mut PresentationBanks<PresentationBody<RuntimePayload>>> {
        self.presentation.as_deref_mut().and_then(Option::as_mut)
    }

    pub(super) fn construct_presentation_factory(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        call: &std::sync::Arc<crate::presentation::RuntimePresentationFactoryCall>,
        values: Vec<crate::value::RuntimeValue>,
        result: crate::pattern::RuntimeSemanticTypeId,
    ) -> Result<crate::value::RuntimeValue, VmError> {
        use crate::{
            entry::RuntimeSchemaLimits, presentation::resources::PresentationLedgerLimits,
            value::RuntimeValue,
        };
        let error = |cause: &dyn std::fmt::Display| VmError::Runtime(cause.to_string());
        call.validate_arguments(&values)
            .map_err(|cause| error(&cause))?;
        let context = self.context.ok_or(VmError::MissingExecutionContext)?;
        let admitted = context.admitted_program(program)?;
        let owner = context.program_owner(program)?;
        let handle_owner = call
            .result_owner(owner.types(), result)
            .map_err(|cause| error(&cause))?;
        let source = admitted
            .original_task_source()
            .ok_or(VmError::MissingExecutionContext)?;
        let limits = RuntimeSchemaLimits::engine_default();
        let storage = self
            .presentation
            .as_deref_mut()
            .ok_or(VmError::MissingExecutionContext)?;
        let mut admission = admitted.retained_value_admission(limits);
        if let Some(banks) = storage.as_ref() {
            banks
                .admit_retained(&owner, context.artifact(), &mut admission)
                .map_err(|cause| error(&cause))?;
        }
        fiber
            .admit_retained(&mut admission)
            .map_err(|cause| error(&cause))?;
        let binding = fiber.active_frame()?.type_instantiation.clone();
        let body = RuntimeValue::Tuple(values);
        let site = call
            .registration_site(&mut admission, &body)
            .map_err(|cause| error(&cause))?;
        let resource = admission
            .construct_presentation_resource(
                source.clone(),
                std::sync::Arc::clone(call),
                binding.clone(),
                RuntimePayload::new(body),
            )
            .map_err(|cause| error(&cause))?;
        let banks = storage.get_or_insert_with(|| {
            PresentationBanks::new(
                owner.clone(),
                context.artifact(),
                PresentationLedgerLimits::for_schema(limits),
            )
        });
        banks
            .preflight_register(&owner)
            .map_err(|cause| error(&cause))?;
        let ledger = banks
            .ledger_mut(fiber.presentation_bank())
            .map_err(|cause| error(&cause))?;
        ledger
            .preflight_register(&owner)
            .map_err(|cause| error(&cause))?;
        let scope = fiber
            .bind_presentation_scope(&owner, ledger)
            .map_err(|cause| error(&cause))?;
        let token = banks
            .register_and_retire(
                &owner,
                scope,
                site,
                PresentationBody {
                    expected: call.resource_type(),
                    binding,
                    value: RuntimePayload::new(resource),
                },
            )
            .map_err(|cause| error(&cause.cause))?;
        handle_owner
            .try_wrap(token.encode_payload())
            .map_err(|cause| error(&cause))
    }

    /// Admit every actual result resource while it still lives in its source
    /// register. A nested return receives it in the caller's current scope;
    /// a terminal result owns its detached original-bank lease.
    pub(super) fn exit_return_scope(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let mut escaping = Self::escaping_tokens(fiber, registers)?;
        if let Some(return_to) = &fiber.active_frame()?.return_to {
            return_to.continuation.visit_runtime_values(|value| {
                if let crate::value::RuntimeValue::Opaque(opaque) = value
                    && opaque.value_class()
                        == crate::value::RuntimeOpaqueValueClass::PresentationHandle
                {
                    escaping.push(
                        crate::runtime_id::RuntimePresentationHandleToken::try_decode_payload(
                            opaque.payload(),
                        )
                        .map_err(|error| VmError::Runtime(error.to_string()))?,
                    );
                }
                Ok::<_, VmError>(())
            })?;
        }
        let (targets, destination) = fiber
            .presentation_return_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() && escaping.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .transition_scope_contour(&owner, targets, destination, &escaping, &[])
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }

    pub(super) fn exit_goto_scope(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
        replacement: &mut crate::awbc::fiber::PreparedFiberRootReplacement,
        registers: &[AwbcRegisterId],
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let escaping = Self::escaping_tokens(fiber, registers.iter().copied())?;
        let instance = fiber.instance;
        let targets = fiber
            .presentation_unwind_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() && escaping.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .transition_scope_contour(
                &owner,
                targets,
                Some(crate::presentation::resources::PresentationScopeExit {
                    execution: crate::runtime_id::ExecutionInstanceId::from_allocated(
                        instance.get(),
                    ),
                    fiber: crate::runtime_id::RuntimePersistentFiberId::from_allocated(
                        instance.get().get(),
                    ),
                    scope: replacement.presentation_scope_target(),
                }),
                &escaping,
                &[],
            )
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }

    fn escaping_tokens(
        fiber: &FiberState,
        registers: impl IntoIterator<Item = AwbcRegisterId>,
    ) -> Result<Vec<crate::runtime_id::RuntimePresentationHandleToken>, VmError> {
        let mut escaping = Vec::new();
        for register in registers {
            for handle in fiber
                .active_frame()?
                .register(register)?
                .view()
                .affine_handles()
                .map_err(|error| VmError::Runtime(error.to_string()))?
            {
                if let crate::value::ownership::RuntimeAffineHandle::Presentation {
                    token, ..
                } = handle
                {
                    escaping.push(token);
                }
            }
        }
        Ok(escaping)
    }

    /// All cleanup declarations and actual scope grants precede body movement.
    /// Without a bank, the standalone VM can unwind resource-free frames only.
    pub(super) fn retire_unwind(
        &mut self,
        program: &AwbcProgram,
        fiber: &mut FiberState,
    ) -> Result<(), VmError> {
        fiber.validate_cleanup_inputs(program)?;
        let targets = fiber
            .presentation_unwind_scopes_mut()
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        if targets.is_empty() {
            return Ok(());
        }
        let context = self.context;
        let banks = self.banks_mut().ok_or_else(|| {
            FiberStateError::Snapshot(
                crate::presentation::RuntimePresentationError::MissingLedger.into(),
            )
        })?;
        let owner = context
            .ok_or(VmError::MissingExecutionContext)?
            .program_owner(program)?;
        banks
            .retire_scope_contour(&owner, targets)
            .map_err(|error| FiberStateError::Snapshot(error.into()))?;
        Ok(())
    }
}
