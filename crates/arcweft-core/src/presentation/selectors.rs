//! Borrowed slot selectors shared by address reads and occupant writes.

use super::{RuntimePresentationError, RuntimePresentationSlotInput};
use crate::value::{
    AwbcRuntimeValueSnapshotError, RuntimeEntityReference, RuntimeRetainedValueAdmission,
    RuntimeValue,
};
use arcweft_id::{DeclarationIdentityFamily, ProjectEntityReferenceFamily};
use arcweft_interaction_model::presentation::PresentationResourceKind;

pub(crate) struct RuntimePresentationSlotSelectors<'a> {
    target: Option<&'a str>,
    slot: Option<&'a str>,
    character: Option<&'a str>,
}

impl<'a> RuntimePresentationSlotSelectors<'a> {
    pub(crate) fn construct_address(
        self,
        resource: PresentationResourceKind,
        admission: &mut RuntimeRetainedValueAdmission,
    ) -> Result<
        arcweft_interaction_model::presentation::PresentationSlotAddress,
        AwbcRuntimeValueSnapshotError,
    > {
        use arcweft_id::PublicId;
        use arcweft_interaction_model::presentation::PresentationSlotAddress;
        self.with_parts(resource, admission, |admission, target, slot| {
            PresentationSlotAddress::validate_text(target, slot, resource)
                .map_err(RuntimePresentationError::from)?;
            admission.validate_metadata_string(target)?;
            admission.validate_metadata_string(slot)?;
            let target =
                PublicId::try_new(target).map_err(|_| RuntimePresentationError::InvalidState)?;
            let slot =
                PublicId::try_new(slot).map_err(|_| RuntimePresentationError::InvalidState)?;
            Ok(PresentationSlotAddress::try_new(target, slot, resource)
                .map_err(RuntimePresentationError::from)?)
        })
    }

    pub(crate) fn from_values(
        values: impl IntoIterator<Item = (RuntimePresentationSlotInput, &'a RuntimeValue)>,
    ) -> Result<Self, RuntimePresentationError> {
        let mut selected = Self {
            target: None,
            slot: None,
            character: None,
        };
        for (role, value) in values {
            let invalid = || RuntimePresentationError::SlotReferenceInput { role };
            let RuntimeValue::EntityRef(reference) = value else {
                return Err(invalid());
            };
            let (family, identity) = match reference {
                RuntimeEntityReference::PresentationAddress(identity) => {
                    (identity.kind().family(), identity.public_id())
                }
                RuntimeEntityReference::ImportedProject(reference) => {
                    (reference.family(), reference.public_id())
                }
                RuntimeEntityReference::Project {
                    family: DeclarationIdentityFamily::Character,
                    public_id,
                } => (ProjectEntityReferenceFamily::Character, public_id),
                _ => return Err(invalid()),
            };
            let destination = match role {
                RuntimePresentationSlotInput::Target
                    if family == ProjectEntityReferenceFamily::Target =>
                {
                    &mut selected.target
                }
                RuntimePresentationSlotInput::Slot
                    if family == ProjectEntityReferenceFamily::Slot =>
                {
                    &mut selected.slot
                }
                RuntimePresentationSlotInput::Character
                    if family == ProjectEntityReferenceFamily::Character =>
                {
                    &mut selected.character
                }
                _ => return Err(invalid()),
            };
            if destination.replace(identity.as_str()).is_some() {
                return Err(invalid());
            }
        }
        Ok(selected)
    }

    pub(crate) fn with_parts<T>(
        self,
        resource: PresentationResourceKind,
        admission: &mut RuntimeRetainedValueAdmission,
        construct: impl FnOnce(
            &mut RuntimeRetainedValueAdmission,
            &str,
            &str,
        ) -> Result<T, AwbcRuntimeValueSnapshotError>,
    ) -> Result<T, AwbcRuntimeValueSnapshotError> {
        let target = self.target.unwrap_or("target.scene");
        let default;
        let slot = match self.slot {
            Some(slot) => slot,
            None => {
                default = if resource == PresentationResourceKind::Character {
                    let character = self
                        .character
                        .and_then(|identity| identity.strip_prefix("character."))
                        .filter(|name| !name.is_empty())
                        .ok_or(RuntimePresentationError::SlotReferenceInput {
                            role: RuntimePresentationSlotInput::Character,
                        })?;
                    admission.construct_metadata_string(&[
                        "slot.character.",
                        character,
                        ".default",
                    ])?
                } else {
                    admission.construct_metadata_string(&[
                        "slot.",
                        resource.as_str(),
                        ".default",
                    ])?
                };
                &default
            }
        };
        construct(admission, target, slot)
    }
}
