//! Original caller authority for lazy-context continuation admission.

use super::{
    AwbcFunctionId, AwbcInstruction, AwbcProgram, AwbcRegisterId, AwbcTypeId, FiberCursor,
    FiberStateError,
};
use crate::entry::schema::value_encoding::ValueResourceAdmission;
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::{RuntimeArcErrorContextKind, RuntimeArcErrorContextPending, RuntimeValueView};

/// Borrowed return coordinates and immutable destination binding. Candidate
/// value provenance never supplies the caller's type or operation authority.
pub(super) struct FiberReturnContext<'a> {
    pub(super) program: &'a AwbcProgram,
    pub(super) caller: Option<AwbcFunctionId>,
    pub(super) binding: Option<&'a RuntimeFunctionEffectInstantiation>,
    pub(super) returning_function: AwbcFunctionId,
    pub(super) cursor: FiberCursor,
    pub(super) destination: Option<AwbcRegisterId>,
}

impl FiberReturnContext<'_> {
    pub(super) fn validate_context_return<V: super::FiberStoredValue>(
        &self,
        site: FiberCursor,
        pending: &RuntimeArcErrorContextPending<V>,
        default_stage: bool,
        callable_state: crate::runtime_id::RuntimeCallableStateId,
    ) -> Result<(), FiberStateError> {
        let mut resources =
            V::resource_admission(crate::entry::RuntimeSchemaLimits::engine_default());
        self.validate_context_pending(
            site,
            pending,
            default_stage,
            callable_state,
            V::borrowed_view,
            &mut resources,
        )
    }

    pub(super) fn validate_context_pending<V>(
        &self,
        site: FiberCursor,
        pending: &RuntimeArcErrorContextPending<V>,
        default_stage: bool,
        callable_state: crate::runtime_id::RuntimeCallableStateId,
        view: impl Fn(&V) -> RuntimeValueView<'_>,
        resources: &mut ValueResourceAdmission,
    ) -> Result<(), FiberStateError> {
        let expected =
            self.context_source_type(site, pending.kind(), default_stage, callable_state)?;
        pending
            .validate_source(
                crate::program_types::RuntimeProgramValueType::Awbc {
                    program: self.program,
                    ty: expected,
                },
                self.binding,
                view,
                resources,
            )
            .map_err(|error| {
                FiberStateError::Snapshot(crate::value::AwbcRuntimeValueSnapshotError::ProgramType(
                    Box::new(error.within_value_path("context.source")),
                ))
            })
    }

    fn context_source_type(
        &self,
        site: FiberCursor,
        kind: RuntimeArcErrorContextKind,
        default_stage: bool,
        callable_state: crate::runtime_id::RuntimeCallableStateId,
    ) -> Result<AwbcTypeId, FiberStateError> {
        if self.caller != Some(site.function) || self.cursor != site || self.destination.is_some() {
            return Err(FiberStateError::InvalidFrame);
        }
        let AwbcInstruction::CallIntrinsic {
            dst: Some(_),
            intrinsic,
            args,
        } = super::instruction_at_site(self.program, site)?
        else {
            return Err(FiberStateError::InvalidFrame);
        };
        let [_, _] = args.as_slice() else {
            return Err(FiberStateError::InvalidFrame);
        };
        let intrinsic = self
            .program
            .intrinsics
            .get(intrinsic.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let expected_kind = match intrinsic.identity.as_intrinsic() {
            Some(crate::value::RuntimeIntrinsic::StdResultWithContext) => {
                RuntimeArcErrorContextKind::Result
            }
            Some(crate::value::RuntimeIntrinsic::StdOptionWithContext) => {
                RuntimeArcErrorContextKind::Option
            }
            _ => return Err(FiberStateError::InvalidFrame),
        };
        if kind != expected_kind {
            return Err(FiberStateError::InvalidFrame);
        }
        let signature = self
            .program
            .signatures
            .get(intrinsic.signature.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        let [source, callback] = signature.params.as_slice() else {
            return Err(FiberStateError::InvalidFrame);
        };
        let state = self
            .program
            .callable_states
            .get(callable_state.index())
            .ok_or(FiberStateError::InvalidFrame)?;
        if state.function_type != *callback {
            return Err(FiberStateError::InvalidFrame);
        }
        if default_stage {
            crate::value::RuntimeCallableBodyReference::Awbc(self.returning_function)
                .validate_default_return(
                    crate::program_types::RuntimeProgramTypes::Awbc(self.program),
                    callable_state,
                )
                .map_err(|_| FiberStateError::InvalidFrame)?;
        } else if !matches!(state.transition,
            crate::plan::RuntimeCallableTransition::Invoke { function, .. } if function == self.returning_function
        ) {
            return Err(FiberStateError::InvalidFrame);
        }
        Ok(*source)
    }
}
