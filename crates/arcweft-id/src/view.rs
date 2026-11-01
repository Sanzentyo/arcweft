//! Authored and engine-owned View identities shared by runtime and presentation.

use crate::{IdError, PublicId};
use core::fmt;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

/// Stable semantic identity of one public View owner.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewId(PublicId);

/// Invalid public identity for the View family.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ViewIdError {
    #[error(transparent)]
    Invalid(#[from] IdError),
    #[error("public ID `{actual}` is not in the View family")]
    WrongFamily { actual: PublicId },
}

/// Stable runtime identity projected from one public View identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectedRuntimeViewId([u8; 32]);

impl ProjectedRuntimeViewId {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl ViewId {
    /// Constructs an authored `view.*` identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ViewIdError> {
        Self::try_from(PublicId::try_new(value)?)
    }

    /// Constructs a semantic identity for an engine-owned reserved View.
    pub fn try_new_engine_owned(value: impl Into<String>) -> Result<Self, ViewIdError> {
        let public_id = PublicId::try_new_engine_owned(value)?;
        if public_id
            .as_str()
            .strip_prefix("std.view.")
            .is_some_and(|tail| !tail.is_empty())
        {
            Ok(Self(public_id))
        } else {
            Err(ViewIdError::WrongFamily { actual: public_id })
        }
    }

    /// Parses either an authored `view.*` identity or an engine-owned
    /// `std.view.*` identity at a public data boundary.
    pub fn parse_public(value: impl Into<String>) -> Result<Self, ViewIdError> {
        Self::try_from(PublicId::try_new_engine_owned(value)?)
    }

    /// Engine-owned fallback used when a launch profile does not select a View.
    ///
    /// # Panics
    ///
    /// Panics if the compile-time reserved identity `std.view.dialogue` stops
    /// satisfying the engine-owned `PublicId` contract.
    pub fn standard_dialogue() -> Self {
        Self::try_new_engine_owned("std.view.dialogue")
            .expect("the reserved standard dialogue View identity is valid")
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub fn into_public_id(self) -> PublicId {
        self.0
    }

    /// Projects this public identity into the exact version-one runtime ID.
    #[must_use]
    pub fn projected_runtime_id_v1(&self) -> ProjectedRuntimeViewId {
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft.runtime-view-id.v1\0");
        hasher.update(&1_u32.to_le_bytes());
        hash_string(&mut hasher, self.as_str());
        ProjectedRuntimeViewId(*hasher.finalize().as_bytes())
    }
}

impl TryFrom<PublicId> for ViewId {
    type Error = ViewIdError;

    fn try_from(public_id: PublicId) -> Result<Self, Self::Error> {
        if public_id
            .as_str()
            .strip_prefix("view.")
            .or_else(|| public_id.as_str().strip_prefix("std.view."))
            .is_some_and(|tail| !tail.is_empty())
        {
            Ok(Self(public_id))
        } else {
            Err(ViewIdError::WrongFamily { actual: public_id })
        }
    }
}

impl fmt::Display for ViewId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for ViewId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::parse_public(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

fn hash_string(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(
        &u32::try_from(value.len())
            .expect("View identities always fit the runtime string grammar")
            .to_le_bytes(),
    );
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::{ViewId, ViewIdError};
    use crate::PublicId;
    #[test]
    fn standard_dialogue_identity_uses_the_engine_owned_namespace() {
        assert_eq!(ViewId::standard_dialogue().as_str(), "std.view.dialogue");
    }

    #[test]
    fn view_identity_owns_authored_and_engine_family_invariants() {
        assert_eq!(
            ViewId::try_new("view.dialogue").unwrap().as_str(),
            "view.dialogue"
        );
        assert_eq!(
            ViewId::try_new_engine_owned("std.view.dialogue")
                .unwrap()
                .as_str(),
            "std.view.dialogue"
        );
        assert_eq!(
            ViewId::parse_public("style.dialogue"),
            Err(ViewIdError::WrongFamily {
                actual: PublicId::try_new("style.dialogue").unwrap(),
            })
        );
        assert!(matches!(
            ViewId::parse_public("@view.dialogue"),
            Err(ViewIdError::Invalid(_))
        ));
        assert!(ViewId::try_new("std.view.dialogue").is_err());
        assert!(ViewId::try_new_engine_owned("view.dialogue").is_err());
        assert!(ViewId::try_new("view.").is_err());
    }

    #[test]
    fn projected_runtime_view_id_uses_the_version_one_byte_grammar() {
        let view = ViewId::try_new("view.dialogue.standard").unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(b"arcweft.runtime-view-id.v1\0");
        expected.extend_from_slice(&1_u32.to_le_bytes());
        expected.extend_from_slice(
            &u32::try_from(view.as_str().len())
                .expect("test identity length")
                .to_le_bytes(),
        );
        expected.extend_from_slice(view.as_str().as_bytes());

        assert_eq!(
            view.projected_runtime_id_v1().as_bytes(),
            blake3::hash(&expected).as_bytes()
        );
        assert_ne!(
            view.projected_runtime_id_v1(),
            ViewId::try_new("view.dialogue.other")
                .unwrap()
                .projected_runtime_id_v1()
        );
    }

    #[test]
    fn stable_view_identity_serializes_as_a_validated_public_id_string() {
        let view = ViewId::try_new("view.dialogue.standard").unwrap();
        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            r#""view.dialogue.standard""#
        );
        assert_eq!(
            serde_json::from_str::<ViewId>(r#""view.dialogue.standard""#).unwrap(),
            view
        );
        assert_eq!(
            serde_json::from_str::<ViewId>(r#""std.view.dialogue""#).unwrap(),
            ViewId::standard_dialogue()
        );
        assert!(serde_json::from_str::<ViewId>("\"#view.dialogue\"").is_err());
        assert!(serde_json::from_str::<ViewId>(r#""style.dialogue""#).is_err());
    }
}
