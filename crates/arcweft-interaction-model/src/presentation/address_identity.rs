//! Inert address components are values, independent of project declarations.

use super::{PresentationResourceKind, PresentationSlotAddressError};
use arcweft_id::{ProjectEntityReferenceFamily, PublicId};
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PresentationAddressIdentityKind {
    Target,
    Slot(PresentationResourceKind),
}

impl PresentationAddressIdentityKind {
    pub const fn family(self) -> ProjectEntityReferenceFamily {
        match self {
            Self::Target => ProjectEntityReferenceFamily::Target,
            Self::Slot(_) => ProjectEntityReferenceFamily::Slot,
        }
    }

    pub fn validate_text(self, text: &str) -> Result<(), PresentationSlotAddressError> {
        let valid = PublicId::validate_text(text).is_ok();
        match self {
            Self::Target
                if valid
                    && text
                        .strip_prefix("target.")
                        .is_some_and(|name| !name.is_empty()) =>
            {
                Ok(())
            }
            Self::Target => Err(PresentationSlotAddressError::Target),
            Self::Slot(resource) => {
                if valid
                    && text
                        .strip_prefix("slot.")
                        .and_then(|path| path.split_once('.'))
                        .is_some_and(|(family, name)| {
                            family == resource.as_str() && !name.is_empty()
                        })
                {
                    Ok(())
                } else {
                    Err(PresentationSlotAddressError::Slot { resource })
                }
            }
        }
    }

    /// Classifies the shared public-ID grammar without consulting a renderer,
    /// catalog, active occupant, or lexical resource lease.
    pub fn from_public_id_text(text: &str) -> Result<Self, PresentationSlotAddressError> {
        let kind = if text.starts_with("target.") {
            Self::Target
        } else {
            let family = text
                .strip_prefix("slot.")
                .and_then(|path| path.split_once('.'))
                .map(|(family, _)| family);
            let resource = PresentationResourceKind::ALL
                .into_iter()
                .find(|resource| Some(resource.as_str()) == family)
                .ok_or(PresentationSlotAddressError::Identity)?;
            Self::Slot(resource)
        };
        kind.validate_text(text)?;
        Ok(kind)
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
pub struct PresentationAddressIdentity {
    kind: PresentationAddressIdentityKind,
    #[serde(serialize_with = "serialize_id")]
    public_id: PublicId,
}

impl PresentationAddressIdentity {
    pub fn try_new(
        kind: PresentationAddressIdentityKind,
        public_id: PublicId,
    ) -> Result<Self, PresentationSlotAddressError> {
        kind.validate_text(public_id.as_str())?;
        Ok(Self { kind, public_id })
    }

    pub const fn kind(&self) -> PresentationAddressIdentityKind {
        self.kind
    }
    pub const fn public_id(&self) -> &PublicId {
        &self.public_id
    }
}

fn serialize_id<S: serde::Serializer>(id: &PublicId, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(id.as_str())
}

impl<'de> Deserialize<'de> for PresentationAddressIdentity {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            kind: PresentationAddressIdentityKind,
            public_id: String,
        }
        let wire = Wire::deserialize(deserializer)?;
        wire.kind
            .validate_text(&wire.public_id)
            .map_err(serde::de::Error::custom)?;
        Self::try_new(
            wire.kind,
            PublicId::try_new(wire.public_id).map_err(serde::de::Error::custom)?,
        )
        .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn address_components_preserve_their_exact_resource_family_and_reject_forged_headers() {
        let kinds = std::iter::once((
            PresentationAddressIdentityKind::Target,
            "target.scene".to_owned(),
        ))
        .chain(PresentationResourceKind::ALL.into_iter().map(|resource| {
            (
                PresentationAddressIdentityKind::Slot(resource),
                format!("slot.{}.main", resource.as_str()),
            )
        }));
        for (kind, text) in kinds {
            assert_eq!(
                PresentationAddressIdentityKind::from_public_id_text(&text).unwrap(),
                kind
            );
            let identity =
                PresentationAddressIdentity::try_new(kind, PublicId::try_new(text).unwrap())
                    .unwrap();
            let mut wire = serde_json::to_value(&identity).unwrap();
            assert_eq!(
                serde_json::from_value::<PresentationAddressIdentity>(wire.clone()).unwrap(),
                identity
            );
            wire["public_id"] = serde_json::json!("character.alice");
            assert!(serde_json::from_value::<PresentationAddressIdentity>(wire).is_err());
        }
        for text in [
            "target.",
            "slot.background.",
            "slot.missing.main",
            "character.alice",
            "slot.background.bad name",
        ] {
            assert!(PresentationAddressIdentityKind::from_public_id_text(text).is_err());
        }
    }
}
