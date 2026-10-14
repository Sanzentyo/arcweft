//! One original-program allowance for a complete retained live/inert group.

use super::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeProgramOwner,
    RuntimeSemanticTypeId, RuntimeValue,
};
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_budget::ValidationWork;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::RuntimeValueView;

enum RetainedTypeObligation<'binding> {
    Stored(RuntimeSemanticTypeId),
    Invocation {
        expected: RuntimeSemanticTypeId,
        binding: Option<&'binding RuntimeFunctionEffectInstantiation>,
    },
}

/// Admits every retained root before an enclosing frame/session copies or
/// publishes them. Live values and inert caches share the same node, byte and
/// relation-work allowance. The first failure remains final for this context.
pub struct RuntimeRetainedValueAdmission {
    owner: Option<RuntimeProgramOwner>,
    resources: ValueResourceAdmission,
    failure: Option<AwbcRuntimeValueSnapshotError>,
}

impl RuntimeRetainedValueAdmission {
    /// Selects the exact immutable executable for all program-owned descendants.
    #[must_use]
    pub fn for_program(owner: RuntimeProgramOwner, limits: RuntimeSchemaLimits) -> Self {
        Self::new(Some(owner), limits)
    }

    /// Admits context-free data; program-owned descendants cannot acquire a
    /// guessed executable lease.
    #[must_use]
    pub fn standalone(limits: RuntimeSchemaLimits) -> Self {
        Self::new(None, limits)
    }

    fn new(owner: Option<RuntimeProgramOwner>, limits: RuntimeSchemaLimits) -> Self {
        Self {
            owner,
            resources: ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live),
            failure: None,
        }
    }

    /// Admits another borrowed live root without resetting any quota.
    pub fn validate(&mut self, value: &RuntimeValue) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.view(), ValueAdmissionOrigin::Live, None)
    }

    /// Checks the original expected type before any live value is copied.
    pub fn validate_typed(
        &mut self,
        value: &RuntimeValue,
        expected: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.view(),
            ValueAdmissionOrigin::Live,
            Some(RetainedTypeObligation::Stored(expected)),
        )
    }

    /// Uses the original invocation frame's binding for a scoped live result.
    /// The binding is bounded and matched through the same program authority.
    pub fn validate_typed_in_invocation(
        &mut self,
        value: &RuntimeValue,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.view(),
            ValueAdmissionOrigin::Live,
            Some(RetainedTypeObligation::Invocation { expected, binding }),
        )
    }

    /// Admits an inert retained root in the same group as its live values.
    pub fn validate_snapshot(
        &mut self,
        value: &AwbcRuntimeValueSnapshot,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.view(), ValueAdmissionOrigin::InertSnapshot, None)
    }

    /// Checks an inert root's original expected type before DTO copying or live
    /// construction, preserving the stricter inert binding requirements.
    pub fn validate_typed_snapshot(
        &mut self,
        value: &AwbcRuntimeValueSnapshot,
        expected: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.view(),
            ValueAdmissionOrigin::InertSnapshot,
            Some(RetainedTypeObligation::Stored(expected)),
        )
    }

    /// Restores a cached scoped result under its retained original invocation
    /// binding rather than inferring the destination from the candidate.
    pub fn validate_typed_snapshot_in_invocation(
        &mut self,
        value: &AwbcRuntimeValueSnapshot,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.view(),
            ValueAdmissionOrigin::InertSnapshot,
            Some(RetainedTypeObligation::Invocation { expected, binding }),
        )
    }

    pub(crate) fn work(&self) -> ValidationWork {
        self.resources.work()
    }

    fn validate_view(
        &mut self,
        value: RuntimeValueView<'_>,
        origin: ValueAdmissionOrigin,
        expected: Option<RetainedTypeObligation<'_>>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let result = self.admit(value, origin, expected);
        if let Err(failure) = &result {
            self.failure = Some(failure.clone());
        }
        result
    }

    fn admit(
        &mut self,
        value: RuntimeValueView<'_>,
        origin: ValueAdmissionOrigin,
        expected: Option<RetainedTypeObligation<'_>>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.resources.set_origin(origin);
        value.admit_unrestricted(&mut self.resources)?;
        if let Some(obligation) = expected {
            let expected = match &obligation {
                RetainedTypeObligation::Stored(expected)
                | RetainedTypeObligation::Invocation { expected, .. } => *expected,
            };
            let owner = self
                .owner
                .as_ref()
                .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired { expected })?;
            let ty = owner.types().value_type(expected)?;
            match obligation {
                RetainedTypeObligation::Stored(_) => {
                    ty.validate_snapshot_storage(value, &self.resources.work(), origin)?;
                }
                RetainedTypeObligation::Invocation { binding, .. } => {
                    ty.validate_admitted_view(value, binding, &self.resources)?;
                }
            }
        }
        AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
            value,
            self.owner.as_ref(),
            &self.resources,
        )
    }
}
