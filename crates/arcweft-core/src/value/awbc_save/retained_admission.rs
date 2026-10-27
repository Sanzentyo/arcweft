//! One original-program allowance for a complete retained live/inert group.

use super::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeOpaqueValue,
    RuntimeProgramOwner, RuntimeSemanticTypeId, RuntimeValue,
};
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_budget::ValidationWork;
use crate::entry::schema::value_encoding::{
    ValueAdmissionOrigin, ValueResourceAdmission, ValueResourceAdmissionState,
};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::RuntimeValueView;
use crate::value::ownership::RuntimeValueOwnershipRequirement;

mod fanout;
mod metadata;

#[cfg(test)]
mod state_tests;

enum RetainedTypeObligation<'binding> {
    Stored(RuntimeSemanticTypeId),
    OpaquePayload {
        outer: RuntimeSemanticTypeId,
        payload: RuntimeSemanticTypeId,
    },
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
    task_source: Option<crate::plan::task_source::RuntimeTaskSourceProgram>,
    resources: ValueResourceAdmission,
    failure: Option<AwbcRuntimeValueSnapshotError>,
}

struct RetainedProgramOwnerScope<'a> {
    admission: &'a mut RuntimeRetainedValueAdmission,
    previous: Option<RuntimeProgramOwner>,
    previous_source: Option<crate::plan::task_source::RuntimeTaskSourceProgram>,
}

impl Drop for RetainedProgramOwnerScope<'_> {
    fn drop(&mut self) {
        self.admission.owner = self.previous.take();
        self.admission.task_source = self.previous_source.take();
    }
}

/// Exclusive transport of the original bank between incremental publications.
/// It has no Clone or decoder and cannot issue a second allowance.
pub(crate) struct RuntimeRetainedValueAdmissionState {
    owner: Option<RuntimeProgramOwner>,
    task_source: Option<crate::plan::task_source::RuntimeTaskSourceProgram>,
    resources: ValueResourceAdmissionState,
    failure: Option<AwbcRuntimeValueSnapshotError>,
}

impl RuntimeRetainedValueAdmissionState {
    pub(crate) fn resume(self) -> RuntimeRetainedValueAdmission {
        RuntimeRetainedValueAdmission {
            owner: self.owner,
            task_source: self.task_source,
            resources: self.resources.resume(),
            failure: self.failure,
        }
    }
}

impl RuntimeRetainedValueAdmission {
    pub(crate) fn bind_presentation_custody(
        &mut self,
        proof: crate::presentation::resources::PresentationAdmission,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        self.resources
            .bind_presentation(proof)
            .map_err(AwbcRuntimeValueSnapshotError::from)
            .inspect_err(|error| self.retain_failure(error.clone()))
    }

    pub(crate) fn validate_presentation_scope(
        &mut self,
        binding: &crate::presentation::resources::PresentationScopeBinding,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        if self
            .resources
            .presentation_mut()
            .is_some_and(|proof| proof.accepts(binding))
        {
            return Ok(());
        }
        let (execution, fiber, scope) = binding.scope_key();
        let error = AwbcRuntimeValueSnapshotError::PresentationScopeCustodyRequired {
            execution,
            fiber,
            scope,
        };
        self.retain_failure(error.clone());
        Err(error)
    }

    pub(crate) fn finish_presentation_custody(
        &mut self,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        if self
            .resources
            .presentation_mut()
            .is_some_and(|proof| !proof.complete())
        {
            let error: AwbcRuntimeValueSnapshotError =
                crate::presentation::RuntimePresentationError::InvalidState.into();
            self.retain_failure(error.clone());
            return Err(error);
        }
        Ok(())
    }

    /// A retained Source supplies its genuine original lease, including across
    /// program replacement. Only the type authority changes for this field;
    /// counters, limits, storage provenance and the first failure stay shared.
    pub(crate) fn with_program_owner<T>(
        &mut self,
        owner: RuntimeProgramOwner,
        admit: impl FnOnce(&mut Self) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError> {
        self.status()?;
        let previous = self.owner.replace(owner);
        let previous_source = self.task_source.take();
        let scope = RetainedProgramOwnerScope {
            admission: self,
            previous,
            previous_source,
        };
        let result = admit(&mut *scope.admission);
        if let Err(error) = &result {
            scope.admission.retain_failure(error.clone());
        }
        result
    }

    /// Source lookup shares this group's consumed work and first hard failure.
    /// Dropping the temporary meter releases its alias; no quota is issued.
    pub(crate) fn task_source_meter(
        &self,
        limits: crate::plan::RuntimeTaskPlanSealLimits,
    ) -> Result<crate::task::semantic::TaskSemanticMeter, AwbcRuntimeValueSnapshotError> {
        self.status()?;
        Ok(crate::task::semantic::TaskSemanticMeter::for_retained(
            self.work(),
            limits,
        ))
    }
    /// Moves every counter, the original executable and the first failure.
    /// A temporary validator retaining the work handle prevents this transfer.
    pub(crate) fn try_into_state(self) -> Result<RuntimeRetainedValueAdmissionState, Self> {
        let Self {
            owner,
            task_source,
            resources,
            failure,
        } = self;
        match resources.try_into_state() {
            Ok(resources) => Ok(RuntimeRetainedValueAdmissionState {
                owner,
                task_source,
                resources,
                failure,
            }),
            Err(resources) => Err(Self {
                owner,
                task_source,
                resources,
                failure,
            }),
        }
    }

    /// Selects the exact immutable executable for all program-owned descendants.
    #[must_use]
    pub fn for_program(owner: RuntimeProgramOwner, limits: RuntimeSchemaLimits) -> Self {
        Self::new(Some(owner), limits)
    }

    /// Keeps the admitted Source and its exact executable with the same bank.
    /// Digest-equivalent programs cannot provide this retained Source lease.
    pub(crate) fn for_task_source(
        source: &crate::plan::task_source::RuntimeTaskSourceProgram,
        limits: RuntimeSchemaLimits,
    ) -> Self {
        let mut admission = Self::for_program(source.program_owner(), limits);
        admission.task_source = Some(source.clone());
        admission
    }

    pub(crate) fn task_source(
        &self,
    ) -> Option<&crate::plan::task_source::RuntimeTaskSourceProgram> {
        self.task_source.as_ref()
    }

    /// Admits context-free data; program-owned descendants cannot acquire a
    /// guessed executable lease.
    #[must_use]
    pub fn standalone(limits: RuntimeSchemaLimits) -> Self {
        Self::new(None, limits)
    }

    fn new(owner: Option<RuntimeProgramOwner>, limits: RuntimeSchemaLimits) -> Self {
        let mut resources = ValueResourceAdmission::snapshot(limits, ValueAdmissionOrigin::Live);
        resources.require_presentation_custody();
        Self {
            owner,
            task_source: None,
            resources,
            failure: None,
        }
    }

    /// Admits another borrowed live root without resetting any quota.
    pub fn validate(&mut self, value: &RuntimeValue) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.view(), ValueAdmissionOrigin::Live, None)
    }

    /// Admits a read-only public projection with its original storage provenance.
    /// Inspection cannot rebrand an inert descendant as a live carrier.
    pub fn validate_borrowed(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.value, value.origin, None)
    }

    /// Admits a newly authored scalar before constructing its owning carrier.
    /// Scalars have no retained descendants whose storage origin could change.
    pub(crate) fn validate_scalar(
        &mut self,
        value: crate::value::RuntimeScalarView<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            crate::value::RuntimeValueView::Scalar(value),
            ValueAdmissionOrigin::Live,
            None,
        )
    }

    /// Checks a borrowed domain field under the caller's original stored type.
    pub fn validate_typed_borrowed(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
        expected: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.value,
            value.origin,
            Some(RetainedTypeObligation::Stored(expected)),
        )
    }

    pub(crate) fn validate_typed_borrowed_in_invocation(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.value,
            value.origin,
            Some(RetainedTypeObligation::Invocation { expected, binding }),
        )
    }

    /// Rechecks an already admitted retained field against an additional
    /// original type. Its owning grammar has charged nodes and established
    /// custody; relation work and storage obligations still use this bank.
    pub(crate) fn validate_admitted_typed_borrowed_in_invocation(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        self.resources.set_origin(value.origin);
        let result = (|| {
            let owner = self
                .owner
                .as_ref()
                .ok_or(AwbcRuntimeValueSnapshotError::ProgramOwnerRequired { expected })?;
            owner.types().value_type(expected)?.validate_admitted_view(
                value.value,
                binding,
                &self.resources,
            )?;
            AwbcRuntimeValueSnapshot::validate_admitted_storage_view(
                value.value,
                Some(owner),
                &self.resources,
            )
        })();
        result.inspect_err(|error| self.retain_failure(error.clone()))
    }

    /// An enclosing owning field may retain affine custody as inert data.
    /// This inspection grants no Copy entitlement and never constructs a value.
    pub(crate) fn validate_owned_borrowed(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view_with_requirement(
            value.value,
            value.origin,
            None,
            RuntimeValueOwnershipRequirement::Owned,
        )
    }

    pub(crate) fn validate_owned_typed_borrowed(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
        expected: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view_with_requirement(
            value.value,
            value.origin,
            Some(RetainedTypeObligation::Stored(expected)),
            RuntimeValueOwnershipRequirement::Owned,
        )
    }

    pub(crate) fn validate_owned_typed_borrowed_in_invocation(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
        expected: RuntimeSemanticTypeId,
        binding: Option<&RuntimeFunctionEffectInstantiation>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view_with_requirement(
            value.value,
            value.origin,
            Some(RetainedTypeObligation::Invocation { expected, binding }),
            RuntimeValueOwnershipRequirement::Owned,
        )
    }

    /// Checks a domain's original opaque header and payload type obligations
    /// after one complete resource walk, sharing the same relation allowance.
    pub fn validate_typed_opaque_payload(
        &mut self,
        value: crate::value::RuntimeBorrowedOpaque<'_>,
        outer: RuntimeSemanticTypeId,
        payload: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            RuntimeValueView::Opaque(value.value),
            value.origin,
            Some(RetainedTypeObligation::OpaquePayload { outer, payload }),
        )
    }

    /// Admits the complete typed Content codec, including nested formatted
    /// values and original executable callbacks, before constructing a carrier.
    pub fn validate_dialogue_content(
        &mut self,
        value: &crate::value::RuntimeDialogueContentValue,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.view(), ValueAdmissionOrigin::Live, None)
    }

    /// Admits a domain-owned opaque value without copying its payload into a
    /// temporary `RuntimeValue` wrapper.
    pub fn validate_opaque(
        &mut self,
        value: &RuntimeOpaqueValue,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.view(), ValueAdmissionOrigin::Live, None)
    }

    /// Checks a borrowed opaque value against the original program's stored
    /// type obligation before any domain projection is constructed.
    pub fn validate_typed_opaque(
        &mut self,
        value: &RuntimeOpaqueValue,
        expected: RuntimeSemanticTypeId,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(
            value.view(),
            ValueAdmissionOrigin::Live,
            Some(RetainedTypeObligation::Stored(expected)),
        )
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

    /// Captures a complete Copy group only after every original input has passed
    /// the same resource bank and its optional declared type. Untyped inputs
    /// still require their complete stored program authority.
    pub fn capture_unrestricted_values(
        &mut self,
        values: &[(&RuntimeValue, Option<RuntimeSemanticTypeId>)],
    ) -> Result<Vec<AwbcRuntimeValueSnapshot>, AwbcRuntimeValueSnapshotError> {
        self.validate_metadata_collection(values.len())?;
        for (value, expected) in values {
            self.validate_view(
                value.view(),
                ValueAdmissionOrigin::Live,
                expected.map(RetainedTypeObligation::Stored),
            )?;
        }
        values
            .iter()
            .map(|(value, _)| {
                AwbcRuntimeValueSnapshot::from_admitted_storage(value, self.owner.as_ref())
            })
            .collect()
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

    /// Bounds an enclosing frame's retained name before cloning it. Metadata
    /// shares relation work and the resource owner with retained values; its
    /// metadata byte limit is distinct from canonical encoded-value bytes. The
    /// enclosing codec still owns its actual wire encoding and wire limit.
    pub fn validate_metadata_string(
        &mut self,
        value: &str,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let result = self
            .resources
            .metadata_string(value)
            .map_err(AwbcRuntimeValueSnapshotError::from);
        if let Err(failure) = &result {
            self.failure = Some(failure.clone());
        }
        result
    }

    /// Bounds a retained table and its complete metadata work before allocating
    /// or copying members. Empty payloads do not grant an unmetered table.
    pub fn validate_metadata_collection(
        &mut self,
        count: usize,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        let mut work = self.resources.work();
        let result = (|| {
            work.collection(count)?;
            work.charge(0)?;
            for _ in 0..count {
                work.charge(0)?;
            }
            Ok::<(), crate::entry::RuntimeSchemaError>(())
        })()
        .map_err(AwbcRuntimeValueSnapshotError::from);
        if let Err(failure) = &result {
            self.failure = Some(failure.clone());
        }
        result
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

    pub(crate) fn status(&self) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
        self.resources
            .work()
            .failure()
            .map_or(Ok(()), |failure| Err(failure.into()))
    }

    /// Digests the original Source's fully admitted borrowed argument group.
    /// Its byte transcript grants neither Copy nor standalone serialization.
    pub(crate) fn admitted_arguments_digest(
        &self,
        value: crate::value::RuntimeValueView<'_>,
    ) -> Result<crate::entry::RuntimeValueDigest, AwbcRuntimeValueSnapshotError> {
        self.status()?;
        if self.program_owner().is_none() {
            return Err(AwbcRuntimeValueSnapshotError::Message {
                message: "retained argument identity requires its original program".into(),
            });
        }
        crate::entry::schema::retained_argument_identity_digest(
            value,
            self.work().limits().platform_encoded_bytes(),
        )
        .map_err(Into::into)
    }

    pub(crate) fn admit_partial_record_header(
        &mut self,
        header: &crate::value::RuntimePlaceRecordHeader,
        depth: usize,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        let result = self
            .resources
            .partial_record_header(header, depth)
            .map_err(AwbcRuntimeValueSnapshotError::from);
        result.inspect_err(|error| self.retain_failure(error.clone()))
    }

    /// Returns the immutable original executable lease for domain-owned
    /// generation/role checks. Retained Source fields temporarily select their
    /// own original lease while preserving this context's allowance.
    #[must_use]
    pub fn program_owner(&self) -> Option<&RuntimeProgramOwner> {
        self.owner.as_ref()
    }

    pub(crate) fn retain_failure(&mut self, error: AwbcRuntimeValueSnapshotError) {
        if let AwbcRuntimeValueSnapshotError::Resource(resource) = &error {
            self.resources.work().retain_failure(resource.clone());
        }
        self.failure.get_or_insert(error);
    }

    /// An owning composite authenticates its custody and original metadata
    /// while borrowing this context's existing program and resource bank.
    /// Success grants no copying or executable capability.
    pub(crate) fn admit_composite<T>(
        &mut self,
        origin: ValueAdmissionOrigin,
        admit: impl FnOnce(
            Option<&RuntimeProgramOwner>,
            &mut ValueResourceAdmission,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError> {
        self.status()?;
        self.resources.set_origin(origin);
        let result = admit(self.owner.as_ref(), &mut self.resources);
        if let Err(failure) = &result {
            self.retain_failure(failure.clone());
        }
        result
    }

    fn validate_view(
        &mut self,
        value: RuntimeValueView<'_>,
        origin: ValueAdmissionOrigin,
        expected: Option<RetainedTypeObligation<'_>>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view_with_requirement(
            value,
            origin,
            expected,
            RuntimeValueOwnershipRequirement::Unrestricted,
        )
    }

    fn validate_view_with_requirement(
        &mut self,
        value: RuntimeValueView<'_>,
        origin: ValueAdmissionOrigin,
        expected: Option<RetainedTypeObligation<'_>>,
        requirement: RuntimeValueOwnershipRequirement,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.status()?;
        let result = self.admit(value, origin, expected, requirement);
        if let Err(failure) = &result {
            self.retain_failure(failure.clone());
        }
        result
    }

    fn admit(
        &mut self,
        value: RuntimeValueView<'_>,
        origin: ValueAdmissionOrigin,
        expected: Option<RetainedTypeObligation<'_>>,
        requirement: RuntimeValueOwnershipRequirement,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.resources.set_origin(origin);
        value.admit_ownership(requirement, &mut self.resources, 0)?;
        if let Some(obligation) = expected {
            let expected = match &obligation {
                RetainedTypeObligation::OpaquePayload { outer, .. } => *outer,
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
                RetainedTypeObligation::OpaquePayload { payload, .. } => {
                    ty.validate_snapshot_storage(value, &self.resources.work(), origin)?;
                    let RuntimeValueView::Opaque(value) = value else {
                        unreachable!(
                            "opaque payload obligation is issued only from an opaque inspection"
                        )
                    };
                    owner
                        .types()
                        .value_type(payload)?
                        .validate_snapshot_storage(
                            value.payload(),
                            &self.resources.work(),
                            origin,
                        )?;
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
