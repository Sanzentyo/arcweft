//! Producer and payload authority for typed presentation handle carriers.

use super::{RuntimeOpaqueTypeAdmission, RuntimeOpaqueTypeOwner, RuntimeOpaqueTypeProducerId};
use crate::{
    pattern::RuntimeSemanticTypeId,
    runtime_id::RuntimePresentationHandleToken,
    value::{
        RuntimeOpaquePersistence, RuntimeOpaqueValueClass, RuntimeOpaqueValueError,
        RuntimeValueView,
    },
};
use arcweft_interaction_model::presentation::PresentationResourceKind;

impl RuntimeOpaqueTypeOwner {
    /// Exact carrier for one checked `Handle<Resource>` type. A wrapped token
    /// remains inert until its original executable ledger admits its custody.
    #[must_use]
    pub fn presentation_handle(
        resource: PresentationResourceKind,
        semantic_identity: RuntimeSemanticTypeId,
    ) -> Self {
        Self::exact_with(
            RuntimeOpaqueTypeProducerId::try_new(Self::presentation_handle_producer(resource))
                .expect("standard presentation producer identities are valid"),
            semantic_identity,
            RuntimeOpaqueValueClass::PresentationHandle,
            RuntimeOpaquePersistence::SnapshotOnly,
        )
    }

    const fn presentation_handle_producer(resource: PresentationResourceKind) -> &'static str {
        match resource {
            PresentationResourceKind::Image => "std.presentation.handle.image",
            PresentationResourceKind::View => "std.presentation.handle.view",
            PresentationResourceKind::Menu => "std.presentation.handle.menu",
            PresentationResourceKind::Overlay => "std.presentation.handle.overlay",
            PresentationResourceKind::RuntimeControl => "std.presentation.handle.runtime_control",
            PresentationResourceKind::Background => "std.presentation.handle.background",
            PresentationResourceKind::Character => "std.presentation.handle.character",
            PresentationResourceKind::Viewport => "std.presentation.handle.viewport",
        }
    }

    /// The exact standard handle family admitted by this owner.
    pub fn presentation_handle_resource(&self) -> Option<PresentationResourceKind> {
        if self.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity
            || self.value_class() != RuntimeOpaqueValueClass::PresentationHandle
            || self.persistence() != RuntimeOpaquePersistence::SnapshotOnly
        {
            return None;
        }
        PresentationResourceKind::ALL.into_iter().find(|resource| {
            self.producer().as_str() == Self::presentation_handle_producer(*resource)
        })
    }

    pub(super) fn validate_presentation_handle_payload(
        &self,
        payload: RuntimeValueView<'_>,
    ) -> Result<(), RuntimeOpaqueValueError> {
        let expected = PresentationResourceKind::ALL.into_iter().find(|resource| {
            self.producer().as_str() == Self::presentation_handle_producer(*resource)
        });
        if expected.is_some()
            && (self.admission() != RuntimeOpaqueTypeAdmission::ExactIdentity
                || self.value_class() != RuntimeOpaqueValueClass::PresentationHandle
                || self.persistence() != RuntimeOpaquePersistence::SnapshotOnly)
        {
            return Err(RuntimeOpaqueValueError::InvalidStandardPayload {
                producer: self.producer().clone(),
            });
        }
        if self.value_class() == RuntimeOpaqueValueClass::PresentationHandle {
            let token = RuntimePresentationHandleToken::try_decode_view(payload)?;
            if let Some(expected) = expected
                && token.resource() != expected
            {
                return Err(RuntimeOpaqueValueError::PresentationResource {
                    expected,
                    actual: token.resource(),
                });
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        effect::RuntimeArtifactFingerprint,
        pattern::RuntimeCheckedType,
        runtime_id::{ExecutionInstanceId, RuntimePersistentFiberId, RuntimePresentationBankId},
        value::RuntimeValue,
    };
    use std::num::NonZeroU64;

    fn token(resource: PresentationResourceKind) -> RuntimePresentationHandleToken {
        RuntimePresentationHandleToken::from_issued(
            RuntimeArtifactFingerprint::try_from_bytes([17; 32]).unwrap(),
            resource,
            ExecutionInstanceId::from_allocated(NonZeroU64::new(3).unwrap()),
            RuntimePersistentFiberId::from_allocated(0),
            NonZeroU64::new(1).unwrap(),
            RuntimePresentationBankId::Root,
        )
    }

    #[test]
    fn presentation_handle_type_rejects_every_foreign_resource_family() {
        for expected in PresentationResourceKind::ALL {
            let owner = RuntimeOpaqueTypeOwner::presentation_handle(
                expected,
                RuntimeSemanticTypeId::from_bytes([expected.semantic_tag(); 32]),
            );
            for actual in PresentationResourceKind::ALL {
                let wrapped = owner.try_wrap(token(actual).encode_payload());
                if actual == expected {
                    let value = wrapped.expect("same resource family");
                    assert!(
                        RuntimeCheckedType::Opaque {
                            owner: owner.clone()
                        }
                        .accepts_value(&value)
                    );
                    assert_eq!(
                        value.ownership(),
                        crate::value::ownership::RuntimeValueOwnership::Affine
                    );
                } else {
                    assert_eq!(
                        wrapped,
                        Err(RuntimeOpaqueValueError::PresentationResource { expected, actual })
                    );
                }
            }
        }
    }

    #[test]
    fn presentation_handle_admission_rechecks_deserialized_resource_payload() {
        let owner = RuntimeOpaqueTypeOwner::presentation_handle(
            PresentationResourceKind::View,
            RuntimeSemanticTypeId::from_bytes([31; 32]),
        );
        let value = owner
            .try_wrap(token(PresentationResourceKind::View).encode_payload())
            .unwrap();
        let mut raw = serde_json::to_value(&value).unwrap();
        raw["Opaque"]["payload"] =
            serde_json::to_value(token(PresentationResourceKind::Image).encode_payload()).unwrap();
        let forged = serde_json::from_value::<RuntimeValue>(raw).unwrap();
        assert!(!RuntimeCheckedType::Opaque { owner }.accepts_value(&forged));
    }

    #[test]
    fn presentation_handle_standard_owner_cannot_weaken_ownership_or_persistence() {
        let owner = RuntimeOpaqueTypeOwner::presentation_handle(
            PresentationResourceKind::View,
            RuntimeSemanticTypeId::from_bytes([32; 32]),
        );
        for (admission, value_class, persistence) in [
            (
                RuntimeOpaqueTypeAdmission::ExactIdentity,
                RuntimeOpaqueValueClass::Plain,
                RuntimeOpaquePersistence::SnapshotOnly,
            ),
            (
                RuntimeOpaqueTypeAdmission::ExactIdentity,
                RuntimeOpaqueValueClass::PresentationHandle,
                RuntimeOpaquePersistence::ConstantAndSnapshot,
            ),
            (
                RuntimeOpaqueTypeAdmission::ProducerWide,
                RuntimeOpaqueValueClass::PresentationHandle,
                RuntimeOpaquePersistence::SnapshotOnly,
            ),
        ] {
            let forged_owner = RuntimeOpaqueTypeOwner::with_admission(
                owner.producer().clone(),
                owner.semantic_identity(),
                admission,
                value_class,
                persistence,
            );
            assert!(
                forged_owner
                    .try_wrap(token(PresentationResourceKind::View).encode_payload())
                    .is_err()
            );
            let actual = owner
                .try_wrap(token(PresentationResourceKind::View).encode_payload())
                .unwrap();
            assert!(
                !RuntimeCheckedType::Opaque {
                    owner: forged_owner
                }
                .accepts_value(&actual)
            );
        }
    }
}
