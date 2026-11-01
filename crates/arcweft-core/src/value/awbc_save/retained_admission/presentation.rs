//! Slot address construction/copy uses the original retained resource allowance.

use super::*;
use crate::pattern::RuntimeOpaqueTypeOwner;
use crate::value::{
    RuntimeBorrowedValue, RuntimeOpaqueView, RuntimeScalarView, RuntimeTupleView, RuntimeUInt,
};
use arcweft_id::PublicId;
use arcweft_interaction_model::presentation::PresentationSlotAddress;

impl RuntimeRetainedValueAdmission {
    /// Admits the complete typed opaque graph before allocating its header,
    /// tuple or public identity strings. No resource lease is issued.
    pub fn construct_presentation_slot_ref(
        &mut self,
        owner: &RuntimeOpaqueTypeOwner,
        address: &PresentationSlotAddress,
    ) -> Result<RuntimeValue, AwbcRuntimeValueSnapshotError> {
        let result: Result<_, AwbcRuntimeValueSnapshotError> = (|| {
            self.status()?;
            let expected = owner.presentation_slot_ref_resource()?;
            if address.resource() != expected {
                return Err(
                    crate::value::RuntimeOpaqueValueError::PresentationResource {
                        expected,
                        actual: address.resource(),
                    }
                    .into(),
                );
            }
            let fields = [
                RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(
                    RuntimeOpaqueTypeOwner::PRESENTATION_SLOT_REF_VALUE_VERSION,
                ))),
                RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(
                    address.resource().semantic_tag(),
                ))),
                RuntimeValueView::Scalar(RuntimeScalarView::String(address.target().as_str())),
                RuntimeValueView::Scalar(RuntimeScalarView::String(address.slot().as_str())),
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
                RuntimeValue::UInt(RuntimeUInt::U8(address.resource().semantic_tag())),
                RuntimeValue::String(address.target().as_str().to_owned()),
                RuntimeValue::String(address.slot().as_str().to_owned()),
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
