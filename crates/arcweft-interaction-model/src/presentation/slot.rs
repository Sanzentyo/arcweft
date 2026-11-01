//! Inert target/slot addresses shared by execution and retained presentation.

use super::PresentationResourceKind;
use arcweft_id::PublicId;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// An address identifies a typed cell. It neither issues a resource lease nor
/// retains ownership of the cell's current occupant.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PresentationSlotAddress {
    #[serde(serialize_with = "serialize_id")]
    target: PublicId,
    #[serde(serialize_with = "serialize_id")]
    slot: PublicId,
    resource: PresentationResourceKind,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PresentationSlotAddressError {
    #[error("presentation target must be a valid target.* public identity")]
    Target,
    #[error("presentation slot must belong to the {resource:?} resource family")]
    Slot { resource: PresentationResourceKind },
}

impl PresentationSlotAddress {
    pub fn try_new(
        target: PublicId,
        slot: PublicId,
        resource: PresentationResourceKind,
    ) -> Result<Self, PresentationSlotAddressError> {
        Self::validate_text(target.as_str(), slot.as_str(), resource)?;
        Ok(Self {
            target,
            slot,
            resource,
        })
    }

    /// Borrowed codecs check the same grammar before allocating owned IDs.
    pub fn validate_text(
        target: &str,
        slot: &str,
        resource: PresentationResourceKind,
    ) -> Result<(), PresentationSlotAddressError> {
        if PublicId::validate_text(target).is_err()
            || target.strip_prefix("target.").is_none_or(str::is_empty)
        {
            return Err(PresentationSlotAddressError::Target);
        }
        let valid_slot = slot
            .strip_prefix("slot.")
            .and_then(|path| path.split_once('.'))
            .is_some_and(|(family, name)| family == resource.as_str() && !name.is_empty());
        if !valid_slot || PublicId::validate_text(slot).is_err() {
            return Err(PresentationSlotAddressError::Slot { resource });
        }
        Ok(())
    }

    pub fn validate(&self) -> Result<(), PresentationSlotAddressError> {
        Self::validate_text(self.target.as_str(), self.slot.as_str(), self.resource)
    }

    #[must_use]
    pub const fn target(&self) -> &PublicId {
        &self.target
    }

    #[must_use]
    pub const fn slot(&self) -> &PublicId {
        &self.slot
    }

    #[must_use]
    pub const fn resource(&self) -> PresentationResourceKind {
        self.resource
    }
}

fn serialize_id<S: Serializer>(id: &PublicId, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(id.as_str())
}

impl<'de> Deserialize<'de> for PresentationSlotAddress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Encoded {
            target: String,
            slot: String,
            resource: PresentationResourceKind,
        }
        let encoded = Encoded::deserialize(deserializer)?;
        Self::validate_text(&encoded.target, &encoded.slot, encoded.resource)
            .map_err(serde::de::Error::custom)?;
        let target = PublicId::try_new(encoded.target).map_err(serde::de::Error::custom)?;
        let slot = PublicId::try_new(encoded.slot).map_err(serde::de::Error::custom)?;
        Self::try_new(target, slot, encoded.resource).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_slot_addresses_share_exact_family_and_serde_admission() {
        for resource in PresentationResourceKind::ALL {
            let address = PresentationSlotAddress::try_new(
                PublicId::try_new("target.scene").unwrap(),
                PublicId::try_new(format!("slot.{}.main", resource.as_str())).unwrap(),
                resource,
            )
            .unwrap();
            let encoded = serde_json::to_value(&address).unwrap();
            assert_eq!(
                serde_json::from_value::<PresentationSlotAddress>(encoded.clone()).unwrap(),
                address
            );
            for foreign in PresentationResourceKind::ALL
                .into_iter()
                .filter(|other| *other != resource)
            {
                let mut forged = encoded.clone();
                forged["resource"] = serde_json::to_value(foreign).unwrap();
                assert!(serde_json::from_value::<PresentationSlotAddress>(forged).is_err());
                assert_eq!(
                    PresentationSlotAddress::validate_text(
                        address.target().as_str(),
                        address.slot().as_str(),
                        foreign
                    ),
                    Err(PresentationSlotAddressError::Slot { resource: foreign })
                );
            }
        }
    }

    #[test]
    fn slot_address_decode_rejects_invalid_ids_missing_and_extra_fields() {
        let valid = serde_json::json!({"target":"target.scene","slot":"slot.background.main","resource":"background"});
        for (field, value) in [
            ("target", "asset.scene"),
            ("target", "target."),
            ("target", "target.bad name"),
            ("slot", "slot.background"),
            ("slot", "slot.background.bad name"),
        ] {
            let mut forged = valid.clone();
            forged[field] = serde_json::json!(value);
            assert!(serde_json::from_value::<PresentationSlotAddress>(forged).is_err());
        }
        for field in ["target", "slot", "resource"] {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(serde_json::from_value::<PresentationSlotAddress>(missing).is_err());
        }
        let mut extra = valid;
        extra["bank"] = serde_json::json!("root");
        assert!(serde_json::from_value::<PresentationSlotAddress>(extra).is_err());
    }
}
