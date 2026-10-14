//! One original-program allowance for a complete retained live/inert group.

use super::{
    AwbcRuntimeValueSnapshot, AwbcRuntimeValueSnapshotError, RuntimeOpaqueValue,
    RuntimeProgramOwner, RuntimeSemanticTypeId, RuntimeValue,
};
use crate::entry::RuntimeSchemaLimits;
use crate::entry::schema::value_budget::ValidationWork;
use crate::entry::schema::value_encoding::{ValueAdmissionOrigin, ValueResourceAdmission};
use crate::program_types::RuntimeFunctionEffectInstantiation;
use crate::value::RuntimeValueView;

mod metadata;

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

    /// Admits a read-only public projection with its original storage provenance.
    /// Inspection cannot rebrand an inert descendant as a live carrier.
    pub fn validate_borrowed(
        &mut self,
        value: crate::value::RuntimeBorrowedValue<'_>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.validate_view(value.value, value.origin, None)
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
    /// shares relation work; the enclosing codec owns its encoded wire bytes.
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

    /// Returns the immutable original executable lease for domain-owned
    /// generation/role checks. The context cannot be rebound to another owner.
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
    pub(crate) fn admit_composite(
        &mut self,
        origin: ValueAdmissionOrigin,
        admit: impl FnOnce(
            Option<&RuntimeProgramOwner>,
            &mut ValueResourceAdmission,
        ) -> Result<(), AwbcRuntimeValueSnapshotError>,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        if let Some(failure) = &self.failure {
            return Err(failure.clone());
        }
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
