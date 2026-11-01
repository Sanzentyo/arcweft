//! Exact typed ownership and borrowed payload grammar for inert slot references.

use super::{
    RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId,
    RuntimeSemanticTypeId,
};
use crate::value::{
    RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeOpaqueValueError, RuntimeScalarView,
    RuntimeUInt, RuntimeValueView,
};
use arcweft_interaction_model::presentation::{
    PresentationResourceKind, PresentationSlotAddress, PresentationSlotAddressError,
};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum RuntimePresentationSlotRefDecodeError {
    #[error("slot reference must contain its four canonical fields")]
    Shape,
    #[error("slot reference version {actual} is unsupported; expected 1")]
    Version { actual: u8 },
    #[error("slot reference contains unknown resource tag {tag}")]
    Resource { tag: u8 },
    #[error(transparent)]
    Address(#[from] PresentationSlotAddressError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        pattern::RuntimeCheckedType,
        value::{RuntimeValue, ownership::RuntimeValueOwnership},
    };

    fn payload(resource: PresentationResourceKind) -> RuntimeValue {
        RuntimeValue::Tuple(vec![
            RuntimeValue::UInt(RuntimeUInt::U8(1)),
            RuntimeValue::UInt(RuntimeUInt::U8(resource.semantic_tag())),
            RuntimeValue::String("target.scene".to_owned()),
            RuntimeValue::String(format!("slot.{}.main", resource.as_str())),
        ])
    }

    #[test]
    fn slot_reference_predicates_reject_every_foreign_resource_family() {
        for expected in PresentationResourceKind::ALL {
            let owner = RuntimeOpaqueTypeOwner::presentation_slot_ref(
                expected,
                RuntimeSemanticTypeId::from_bytes([expected.semantic_tag() + 32; 32]),
            );
            for actual in PresentationResourceKind::ALL {
                let result = owner.try_wrap(payload(actual));
                if expected == actual {
                    let value = result.unwrap();
                    assert!(
                        RuntimeCheckedType::Opaque {
                            owner: owner.clone()
                        }
                        .accepts_value(&value)
                    );
                    assert_eq!(value.ownership(), RuntimeValueOwnership::Unrestricted);
                    assert!(!value.contains_nonconstant_opaque());
                } else {
                    assert_eq!(
                        result,
                        Err(RuntimeOpaqueValueError::PresentationResource { expected, actual })
                    );
                }
            }
        }
    }

    #[test]
    fn slot_reference_payload_rejects_unknown_versions_widths_tags_and_addresses() {
        use RuntimePresentationSlotRefDecodeError as Error;
        let resource = PresentationResourceKind::Background;
        let owner = RuntimeOpaqueTypeOwner::presentation_slot_ref(
            resource,
            RuntimeSemanticTypeId::from_bytes([63; 32]),
        );
        for (index, value, error) in [
            (
                0,
                RuntimeValue::UInt(RuntimeUInt::U8(2)),
                Error::Version { actual: 2 },
            ),
            (0, RuntimeValue::u32(1), Error::Shape),
            (
                1,
                RuntimeValue::UInt(RuntimeUInt::U8(8)),
                Error::Resource { tag: 8 },
            ),
            (
                2,
                RuntimeValue::String("asset.scene".into()),
                Error::Address(PresentationSlotAddressError::Target),
            ),
            (
                3,
                RuntimeValue::String("slot.character.main".into()),
                Error::Address(PresentationSlotAddressError::Slot { resource }),
            ),
        ] {
            let RuntimeValue::Tuple(mut fields) = payload(resource) else {
                unreachable!()
            };
            fields[index] = value;
            assert_eq!(
                owner.try_wrap(RuntimeValue::Tuple(fields)),
                Err(RuntimeOpaqueValueError::PresentationSlotRef(error))
            );
        }
        for extra in [false, true] {
            let RuntimeValue::Tuple(mut fields) = payload(resource) else {
                unreachable!()
            };
            if extra {
                fields.push(RuntimeValue::Bool(false));
            } else {
                fields.pop();
            }
            assert_eq!(
                owner.try_wrap(RuntimeValue::Tuple(fields)),
                Err(RuntimeOpaqueValueError::PresentationSlotRef(Error::Shape))
            );
        }
        let good = owner.try_wrap(payload(resource)).unwrap();
        let mut raw = serde_json::to_value(good).unwrap();
        raw["Opaque"]["payload"] =
            serde_json::to_value(payload(PresentationResourceKind::Character)).unwrap();
        let forged = serde_json::from_value::<RuntimeValue>(raw).unwrap();
        assert!(!RuntimeCheckedType::Opaque { owner }.accepts_value(&forged));
    }

    #[test]
    fn slot_reference_standard_owner_cannot_weaken_its_metadata() {
        let resource = PresentationResourceKind::View;
        let owner = RuntimeOpaqueTypeOwner::presentation_slot_ref(
            resource,
            RuntimeSemanticTypeId::from_bytes([64; 32]),
        );
        let good = owner.try_wrap(payload(resource)).unwrap();
        for (admission, class, persistence) in [
            (
                RuntimeOpaqueTypeAdmission::ProducerWide,
                RuntimeOpaqueValueClass::Plain,
                RuntimeOpaquePersistence::ConstantAndSnapshot,
            ),
            (
                RuntimeOpaqueTypeAdmission::ExactIdentity,
                RuntimeOpaqueValueClass::PresentationHandle,
                RuntimeOpaquePersistence::ConstantAndSnapshot,
            ),
            (
                RuntimeOpaqueTypeAdmission::ExactIdentity,
                RuntimeOpaqueValueClass::Plain,
                RuntimeOpaquePersistence::SnapshotOnly,
            ),
        ] {
            let forged = RuntimeOpaqueTypeOwner::with_admission(
                owner.producer().clone(),
                owner.semantic_identity(),
                admission,
                class,
                persistence,
            );
            assert!(forged.try_wrap(payload(resource)).is_err());
            assert!(!RuntimeCheckedType::Opaque { owner: forged }.accepts_value(&good));
        }
    }
}

impl RuntimeOpaqueTypeOwner {
    pub(crate) const PRESENTATION_SLOT_REF_VALUE_VERSION: u8 = 1;
    /// A slot address is unrestricted data, independent of the current occupant.
    #[must_use]
    pub fn presentation_slot_ref(
        resource: PresentationResourceKind,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self::exact_with(
            RuntimeOpaqueTypeProducerId::try_new(Self::presentation_slot_ref_producer(resource))
                .expect("standard slot reference producer"),
            semantic_identity,
            RuntimeOpaqueValueClass::Plain,
            RuntimeOpaquePersistence::ConstantAndSnapshot,
        )
    }

    pub(crate) const fn presentation_slot_ref_producer(
        resource: PresentationResourceKind,
    ) -> &'static str {
        match resource {
            PresentationResourceKind::Image => "std.presentation.slot_ref.image",
            PresentationResourceKind::View => "std.presentation.slot_ref.view",
            PresentationResourceKind::Menu => "std.presentation.slot_ref.menu",
            PresentationResourceKind::Overlay => "std.presentation.slot_ref.overlay",
            PresentationResourceKind::RuntimeControl => "std.presentation.slot_ref.runtime_control",
            PresentationResourceKind::Background => "std.presentation.slot_ref.background",
            PresentationResourceKind::Character => "std.presentation.slot_ref.character",
            PresentationResourceKind::Viewport => "std.presentation.slot_ref.viewport",
        }
    }

    pub(crate) fn presentation_slot_ref_resource(
        &self,
    ) -> Result<PresentationResourceKind, RuntimeOpaqueValueError> {
        let resource = PresentationResourceKind::ALL
            .into_iter()
            .find(|kind| self.producer().as_str() == Self::presentation_slot_ref_producer(*kind));
        match resource {
            Some(resource)
                if self.admission() == RuntimeOpaqueTypeAdmission::ExactIdentity
                    && self.value_class() == RuntimeOpaqueValueClass::Plain
                    && self.persistence() == RuntimeOpaquePersistence::ConstantAndSnapshot =>
            {
                Ok(resource)
            }
            _ => Err(RuntimeOpaqueValueError::InvalidStandardPayload {
                producer: self.producer().clone(),
            }),
        }
    }

    pub(super) fn validate_presentation_slot_ref_payload(
        &self,
        payload: RuntimeValueView<'_>,
    ) -> Result<(), RuntimeOpaqueValueError> {
        if !PresentationResourceKind::ALL
            .into_iter()
            .any(|kind| self.producer().as_str() == Self::presentation_slot_ref_producer(kind))
        {
            return Ok(());
        }
        let expected = self.presentation_slot_ref_resource()?;
        let (actual, _, _) = Self::presentation_slot_ref_parts(payload)?;
        if expected != actual {
            return Err(RuntimeOpaqueValueError::PresentationResource { expected, actual });
        }
        Ok(())
    }

    pub(crate) fn presentation_slot_ref_parts(
        payload: RuntimeValueView<'_>,
    ) -> Result<(PresentationResourceKind, &str, &str), RuntimePresentationSlotRefDecodeError> {
        use RuntimePresentationSlotRefDecodeError as Error;
        let RuntimeValueView::Tuple(fields) = payload else {
            return Err(Error::Shape);
        };
        if fields.len() != 4 {
            return Err(Error::Shape);
        }
        let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(version)))) =
            fields.get(0)
        else {
            return Err(Error::Shape);
        };
        if version != Self::PRESENTATION_SLOT_REF_VALUE_VERSION {
            return Err(Error::Version { actual: version });
        }
        let Some(RuntimeValueView::Scalar(RuntimeScalarView::UInt(RuntimeUInt::U8(tag)))) =
            fields.get(1)
        else {
            return Err(Error::Shape);
        };
        let resource =
            PresentationResourceKind::from_semantic_tag(tag).ok_or(Error::Resource { tag })?;
        let Some(RuntimeValueView::Scalar(RuntimeScalarView::String(target))) = fields.get(2)
        else {
            return Err(Error::Shape);
        };
        let Some(RuntimeValueView::Scalar(RuntimeScalarView::String(slot))) = fields.get(3) else {
            return Err(Error::Shape);
        };
        PresentationSlotAddress::validate_text(target, slot, resource)?;
        Ok((resource, target, slot))
    }
}
