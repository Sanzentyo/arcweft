//! Slot address construction/copy uses the original retained resource allowance.

use super::*;
use crate::pattern::{RuntimeOpaqueTypeOwner, RuntimePresentationSlotRefDecodeError};
use crate::presentation::{
    RuntimePresentationResourceLayout, RuntimePresentationResourceLayoutRef,
};
use crate::value::{
    RuntimeBorrowedValue, RuntimeOpaqueView, RuntimeScalarView, RuntimeTupleView, RuntimeUInt,
};
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::{PresentationResourceKind, PresentationSlotAddress};

impl RuntimeRetainedValueAdmission {
    /// Admits immutable presentation inputs under their original Source and
    /// construction invocation while sharing the enclosing consumer's quota.
    /// The resource carrier and its execution grant are never copied.
    pub fn validate_presentation_body(
        &mut self,
        value: &crate::presentation::RuntimePresentationResourceValue,
    ) -> Result<(), AwbcRuntimeValueSnapshotError> {
        self.with_program_owner(value.program_owner(), |admission| {
            admission.task_source = Some(value.source.clone());
            admission.validate_typed_in_invocation(
                value.body(),
                value.body_type(),
                value.type_instantiation(),
            )
        })
    }

    /// Native factories borrow the actual plan until the whole candidate and
    /// registration metadata pass. Source storage and value headers allocate last.
    pub(crate) fn construct_native_presentation_resource_with_metadata<T>(
        &mut self,
        program: std::sync::Arc<crate::plan::RuntimePlan>,
        call: std::sync::Arc<crate::presentation::RuntimePresentationFactoryCall>,
        binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        body: crate::value::RuntimePayload,
        metadata: impl FnOnce(
            &mut Self,
            &crate::presentation::RuntimePresentationFactoryCall,
            &RuntimeValue,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<(T, RuntimeValue), AwbcRuntimeValueSnapshotError> {
        self.construct_native_presentation_resource_with_layout(
            program,
            RuntimePresentationResourceLayout::Factory(call),
            binding,
            body,
            |admission, layout, body| {
                let RuntimePresentationResourceLayoutRef::Factory(call) = layout else {
                    unreachable!("the factory entry point supplies its own layout")
                };
                metadata(admission, call, body)
            },
        )
    }

    pub(crate) fn construct_native_presentation_view(
        &mut self,
        program: std::sync::Arc<crate::plan::RuntimePlan>,
        call: std::sync::Arc<crate::presentation::RuntimePresentationViewCall>,
        binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        body: crate::value::RuntimePayload,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.construct_native_presentation_resource_with_layout(
            program,
            RuntimePresentationResourceLayout::View(call),
            binding,
            body,
            |_, _, _| Ok(()),
        )
        .map(|(_, value)| value)
    }

    fn construct_native_presentation_resource_with_layout<T>(
        &mut self,
        program: std::sync::Arc<crate::plan::RuntimePlan>,
        layout: RuntimePresentationResourceLayout,
        binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        body: crate::value::RuntimePayload,
        metadata: impl FnOnce(
            &mut Self,
            RuntimePresentationResourceLayoutRef<'_>,
            &RuntimeValue,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<(T, RuntimeValue), AwbcRuntimeValueSnapshotError> {
        self.validate_view_with_requirement(
            RuntimeValueView::PresentationResource(
                crate::presentation::RuntimePresentationResourceView::NativeConstruction {
                    program: &program,
                    layout: layout.borrowed(),
                    binding: binding.as_deref(),
                    body: body.value(),
                },
            ),
            ValueAdmissionOrigin::Live,
            Some(RetainedTypeObligation::Stored(layout.resource_type())),
            RuntimeValueOwnershipRequirement::Owned,
        )?;
        let metadata = metadata(self, layout.borrowed(), body.value())
            .inspect_err(|error| self.retain_failure(error.clone()))?;
        let value = crate::presentation::RuntimePresentationResourceValue {
            source: crate::plan::task_source::RuntimeTaskSourceProgram::native(program),
            layout,
            binding,
            body,
        };
        Ok((
            metadata,
            RuntimeValue::PresentationResource(Box::new(value)),
        ))
    }

    /// Publishes the affine body only after its complete borrowed graph,
    /// original program, layout and invocation have used this owning bank.
    pub(crate) fn construct_presentation_resource(
        &mut self,
        source: crate::plan::task_source::RuntimeTaskSourceProgram,
        layout: RuntimePresentationResourceLayout,
        binding: Option<std::sync::Arc<crate::program_types::RuntimeFunctionEffectInstantiation>>,
        body: crate::value::RuntimePayload,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let candidate = crate::presentation::RuntimePresentationResourceValue {
            source,
            layout,
            binding,
            body,
        };
        self.validate_view_with_requirement(
            RuntimeValueView::PresentationResource(
                crate::presentation::RuntimePresentationResourceView::Construction(&candidate),
            ),
            ValueAdmissionOrigin::Live,
            Some(RetainedTypeObligation::Stored(
                candidate.layout().resource_type(),
            )),
            RuntimeValueOwnershipRequirement::Owned,
        )?;
        Ok(RuntimeValue::PresentationResource(Box::new(candidate)))
    }

    /// Admits the complete typed opaque graph before allocating its header,
    /// tuple or public identity strings. No resource lease is issued.
    pub fn construct_presentation_slot_ref(
        &mut self,
        owner: &RuntimeOpaqueTypeOwner,
        address: &PresentationSlotAddress,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        self.construct_presentation_slot_ref_parts(
            owner,
            address.resource(),
            address.target().as_str(),
            address.slot().as_str(),
        )
    }

    /// Admits borrowed identities directly. A caller does not need to allocate
    /// an intermediate owned address before the complete value-graph preflight.
    pub fn construct_presentation_slot_ref_parts(
        &mut self,
        owner: &RuntimeOpaqueTypeOwner,
        resource: PresentationResourceKind,
        target: &str,
        slot: &str,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let result: Result<_, AwbcRuntimeValueSnapshotError> = (|| {
            self.status()?;
            let expected = owner.presentation_slot_ref_resource()?;
            if resource != expected {
                return Err(
                    crate::value::RuntimeOpaqueValueError::PresentationResource {
                        expected,
                        actual: resource,
                    }
                    .into(),
                );
            }
            PresentationSlotAddress::validate_text(target, slot, resource).map_err(|error| {
                crate::value::RuntimeOpaqueValueError::PresentationSlot(
                    RuntimePresentationSlotRefDecodeError::Address(error),
                )
            })?;
            let fields = [
                RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(
                    RuntimeOpaqueTypeOwner::PRESENTATION_SLOT_REF_VALUE_VERSION,
                ))),
                RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(
                    resource.semantic_tag(),
                ))),
                RuntimeValueView::Scalar(RuntimeScalarView::String(target)),
                RuntimeValueView::Scalar(RuntimeScalarView::String(slot)),
            ];
            let payload = RuntimeValueView::Tuple(RuntimeTupleView::Views(&fields));
            self.validate_view(
                RuntimeValueView::Opaque(RuntimeOpaqueView::Construction {
                    owner,
                    payload: &payload,
                }),
                ValueAdmissionOrigin::Live,
                Some(RetainedTypeObligation::Stored(owner.semantic_identity())),
            )?;
            Ok(owner.try_wrap(RuntimeValue::Tuple(vec![
                RuntimeValue::UInt(RuntimeUInt::U8(
                    RuntimeOpaqueTypeOwner::PRESENTATION_SLOT_REF_VALUE_VERSION,
                )),
                RuntimeValue::UInt(RuntimeUInt::U8(resource.semantic_tag())),
                RuntimeValue::String(target.to_owned()),
                RuntimeValue::String(slot.to_owned()),
            ]))?)
        })();
        result.inspect_err(|error| self.retain_failure(error.clone()))
    }

    /// Copies only the inert address after validating the caller's original
    /// type and the live/inert storage provenance under the same allowance.
    pub fn read_presentation_slot_ref(
        &mut self,
        owner: &RuntimeOpaqueTypeOwner,
        value: RuntimeBorrowedValue<'_>,
    ) -> Result<PresentationSlotAddress, AwbcRuntimeValueSnapshotError> {
        let result: Result<_, AwbcRuntimeValueSnapshotError> = (|| {
            self.status()?;
            owner.presentation_slot_ref_resource()?;
            self.validate_typed_borrowed(value, owner.semantic_identity())?;
            let RuntimeValueView::Opaque(opaque) = value.value else {
                return Err(
                    crate::value::RuntimeOpaqueValueError::InvalidStandardPayload {
                        producer: owner.producer().clone(),
                    }
                    .into(),
                );
            };
            if !owner.accepts_opaque_view(opaque) {
                return Err(
                    crate::value::RuntimeOpaqueValueError::InvalidStandardPayload {
                        producer: owner.producer().clone(),
                    }
                    .into(),
                );
            }
            let (resource, target, slot) =
                RuntimeOpaqueTypeOwner::presentation_slot_ref_parts(opaque.payload())
                    .map_err(crate::value::RuntimeOpaqueValueError::from)?;
            Ok(PresentationSlotAddress::try_new(
                PublicId::try_new(target.to_owned())
                    .expect("borrowed address grammar was validated"),
                PublicId::try_new(slot.to_owned()).expect("borrowed address grammar was validated"),
                resource,
            )
            .expect("borrowed address grammar was validated"))
        })();
        result.inspect_err(|error| self.retain_failure(error.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        awbc::schema::{AwbcProgram, AwbcRuntimeType, AwbcRuntimeTypeShape, AwbcStringId},
        entry::{RuntimeSchemaError, RuntimeSchemaLimits},
        pattern::RuntimeSemanticTypeId,
        plan::{RuntimePlanBuilder, RuntimePlanTypeProjection, RuntimePlanTypeSeed},
        task::RuntimeProgramOwner,
        value::{AwbcRuntimeValueSnapshot, RuntimeStoredValue, ownership::RuntimeValueOwnership},
    };
    use arcweft_interaction_model::presentation::PresentationResourceKind;
    use std::sync::Arc;

    fn owners() -> ([RuntimeProgramOwner; 2], Vec<RuntimeOpaqueTypeOwner>) {
        let types = PresentationResourceKind::ALL
            .into_iter()
            .map(|resource| {
                RuntimeOpaqueTypeOwner::presentation_slot_ref(
                    resource,
                    RuntimeSemanticTypeId::from_bytes([resource.semantic_tag() + 96; 32]),
                )
            })
            .collect::<Vec<_>>();
        let mut builder = RuntimePlanBuilder::new();
        builder
            .admit_type_batch(
                types.iter().map(|owner| {
                    RuntimePlanTypeSeed::new(
                        owner.semantic_identity(),
                        RuntimePlanTypeProjection::Opaque {
                            producer: owner.producer().clone(),
                            admission: owner.admission(),
                            value_class: owner.value_class(),
                            persistence: owner.persistence(),
                            arguments: Box::new([]),
                        },
                    )
                }),
                std::iter::empty::<crate::plan::RuntimeLocalDeclarationSeed>(),
            )
            .unwrap();
        let awbc = AwbcProgram {
            strings: types
                .iter()
                .map(|owner| owner.producer().as_str().to_owned())
                .collect(),
            runtime_types: types
                .iter()
                .enumerate()
                .map(|(index, owner)| {
                    AwbcRuntimeType::new(
                        owner.semantic_identity(),
                        AwbcRuntimeTypeShape::Opaque {
                            producer: AwbcStringId(index as u32),
                            admission: owner.admission(),
                            value_class: owner.value_class(),
                            persistence: owner.persistence(),
                            arguments: vec![],
                        },
                    )
                })
                .collect(),
            ..AwbcProgram::default()
        };
        (
            [
                RuntimeProgramOwner::Plan(Arc::new(builder.finish().unwrap())),
                RuntimeProgramOwner::Awbc(Arc::new(awbc)),
            ],
            types,
        )
    }

    fn address(resource: PresentationResourceKind) -> PresentationSlotAddress {
        PresentationSlotAddress::try_new(
            PublicId::try_new("target.scene").unwrap(),
            PublicId::try_new(format!("slot.{}.main", resource.as_str())).unwrap(),
            resource,
        )
        .unwrap()
    }

    #[test]
    fn slot_reference_values_roundtrip_live_and_inert_under_plan_and_awbc_types() {
        let (programs, types) = owners();
        for program in programs {
            for (resource, owner) in PresentationResourceKind::ALL.into_iter().zip(&types) {
                let expected = address(resource);
                let mut admission = RuntimeRetainedValueAdmission::for_program(
                    program.clone(),
                    RuntimeSchemaLimits::engine_default(),
                );
                let value = admission
                    .construct_presentation_slot_ref(owner, &expected)
                    .unwrap();
                assert_eq!(value.ownership(), RuntimeValueOwnership::Unrestricted);
                assert_eq!(
                    admission
                        .read_presentation_slot_ref(owner, value.borrowed_value())
                        .unwrap(),
                    expected
                );
                let inert =
                    AwbcRuntimeValueSnapshot::from_runtime_value_for_program(&value, &program)
                        .unwrap();
                let encoded = serde_json::to_vec(&inert).unwrap();
                let decoded: AwbcRuntimeValueSnapshot = serde_json::from_slice(&encoded).unwrap();
                assert_eq!(
                    admission
                        .read_presentation_slot_ref(owner, decoded.borrowed_value())
                        .unwrap(),
                    expected
                );
                assert_eq!(
                    decoded.into_runtime_value_for_program(&program).unwrap(),
                    value
                );
            }
        }
    }

    #[test]
    fn slot_reference_construction_preflights_nodes_and_strings_and_keeps_failure_sticky() {
        let (programs, types) = owners();
        for program in programs {
            for (limits, budget) in [
                (
                    RuntimeSchemaLimits {
                        max_nodes: 5,
                        ..RuntimeSchemaLimits::engine_default()
                    },
                    "nodes",
                ),
                (
                    RuntimeSchemaLimits {
                        max_string_bytes: 4,
                        ..RuntimeSchemaLimits::engine_default()
                    },
                    "string_bytes",
                ),
            ] {
                let mut admission =
                    RuntimeRetainedValueAdmission::for_program(program.clone(), limits);
                let expected =
                    AwbcRuntimeValueSnapshotError::Resource(RuntimeSchemaError::BudgetExceeded {
                        budget,
                    });
                assert_eq!(
                    admission.construct_presentation_slot_ref(
                        &types[0],
                        &address(PresentationResourceKind::Image)
                    ),
                    Err(expected.clone())
                );
                assert_eq!(admission.validate_metadata(&0_u8), Err(expected));
            }
        }
    }

    #[test]
    fn slot_reference_construction_rejects_a_foreign_family_without_issuing_a_value() {
        let (programs, types) = owners();
        for program in programs {
            let mut admission = RuntimeRetainedValueAdmission::for_program(
                program,
                RuntimeSchemaLimits::engine_default(),
            );
            let expected = AwbcRuntimeValueSnapshotError::Opaque(
                crate::value::RuntimeOpaqueValueError::PresentationResource {
                    expected: PresentationResourceKind::Image,
                    actual: PresentationResourceKind::View,
                },
            );
            assert_eq!(
                admission.construct_presentation_slot_ref(
                    &types[0],
                    &address(PresentationResourceKind::View)
                ),
                Err(expected.clone())
            );
            assert_eq!(admission.validate_metadata(&0_u8), Err(expected));
        }
    }
}
