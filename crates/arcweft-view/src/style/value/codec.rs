//! Serde decoding routed through checked native Style value constructors.

use super::{
    ViewFontFamily, ViewFontFamilyList, ViewFontWeight, ViewMask, ViewRatioMilli,
    ViewStyleTransition, ViewSystemFontFamily,
};
use crate::style::ViewPropertyKind;
use arcweft_id::PublicId;
use serde::{Deserialize, Deserializer};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum EncodedFontFamily {
    Named(String),
    System(ViewSystemFontFamily),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedFontFamilyList {
    families: Vec<ViewFontFamily>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
enum EncodedMask {
    None,
    Resource(#[serde(with = "public_id")] PublicId),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EncodedTransition {
    property: ViewPropertyKind,
    duration_millis: u32,
    delay_millis: u32,
}

impl<'de> Deserialize<'de> for ViewRatioMilli {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            serde::de::Error::custom("View ratio must be between 0 and 1000 inclusive")
        })
    }
}

impl<'de> Deserialize<'de> for ViewFontWeight {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u16::deserialize(deserializer)?;
        Self::new(value).ok_or_else(|| {
            serde::de::Error::custom("View font weight must be between 1 and 1000 inclusive")
        })
    }
}

impl<'de> Deserialize<'de> for ViewFontFamily {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match EncodedFontFamily::deserialize(deserializer)? {
            EncodedFontFamily::Named(name) => Self::named(name)
                .ok_or_else(|| serde::de::Error::custom("named font family must not be blank")),
            EncodedFontFamily::System(family) => Ok(Self::system(family)),
        }
    }
}

impl<'de> Deserialize<'de> for ViewFontFamilyList {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedFontFamilyList::deserialize(deserializer)?;
        Self::new(encoded.families)
            .ok_or_else(|| serde::de::Error::custom("font family list must not be empty"))
    }
}

impl<'de> Deserialize<'de> for ViewMask {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(match EncodedMask::deserialize(deserializer)? {
            EncodedMask::None => Self::None,
            EncodedMask::Resource(resource) => Self::Resource(resource),
        })
    }
}

impl<'de> Deserialize<'de> for ViewStyleTransition {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = EncodedTransition::deserialize(deserializer)?;
        Self::new(
            encoded.property,
            encoded.duration_millis,
            encoded.delay_millis,
        )
        .ok_or_else(|| serde::de::Error::custom("Style transition property is not transitionable"))
    }
}

pub(super) mod public_id {
    use arcweft_id::PublicId;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(id: &PublicId, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(id.as_str())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<PublicId, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        PublicId::try_new(value).map_err(serde::de::Error::custom)
    }
}
