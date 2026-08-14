//! Stable semantic identities for authored and public Views.

use arcweft_id::{DeclarationName, IdError, PublicId};
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

/// Stable identity of one accepted Arcweft View program.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ViewProgramId(PublicId);

/// Stable runtime identity projected from one public View identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProjectedRuntimeViewId([u8; 32]);

impl ProjectedRuntimeViewId {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Semantic content revision of one accepted View program catalog.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceptedViewProgramRevision([u8; 32]);

/// Invalid stable View identity data.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ViewIdentityError {
    #[error("accepted View-program revision must not be all zero")]
    ZeroAcceptedRevision,
    #[error("accepted View-program revision is not canonical lowercase hex")]
    NonCanonicalAcceptedRevision,
}

impl ViewId {
    /// Constructs an authored `view.*` identity.
    pub fn try_new(value: impl Into<String>) -> Result<Self, ViewIdError> {
        let public_id = PublicId::try_new(value)?;
        if public_id
            .as_str()
            .strip_prefix("view.")
            .is_some_and(|tail| !tail.is_empty())
        {
            Ok(Self(public_id))
        } else {
            Err(ViewIdError::WrongFamily { actual: public_id })
        }
    }

    /// Constructs the public identity of a bare authored View declaration in
    /// its canonical module. The crate root contributes no segment; child
    /// modules remain part of the published identity without requiring a
    /// compiler-local prefix convention.
    pub fn try_from_module_name<'a>(
        module: impl IntoIterator<Item = &'a str>,
        name: &DeclarationName,
    ) -> Result<Self, ViewIdError> {
        let mut value = String::from("view");
        for segment in module {
            let segment = PublicId::try_new(segment.to_owned())?;
            value.push('.');
            value.push_str(segment.as_str());
        }
        value.push('.');
        value.push_str(name.as_str());
        Self::try_new(value)
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
        let public_id = PublicId::try_new_engine_owned(value)?;
        let value = public_id.as_str();
        if value
            .strip_prefix("view.")
            .or_else(|| value.strip_prefix("std.view."))
            .is_some_and(|tail| !tail.is_empty())
        {
            Ok(Self(public_id))
        } else {
            Err(ViewIdError::WrongFamily { actual: public_id })
        }
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

impl ViewProgramId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn from_public_id(value: PublicId) -> Self {
        Self(value)
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
}

impl fmt::Display for ViewId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl fmt::Display for ViewProgramId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl AcceptedViewProgramRevision {
    /// Hashes one canonical typed View-program transcript.
    ///
    /// Transcript construction belongs to the typed producer. This identity
    /// boundary owns the hash algorithm and domain separation so callers
    /// cannot accidentally substitute a bundle, source-map, or file digest.
    pub fn try_for_semantic_transcript(transcript: &[u8]) -> Result<Self, ViewIdentityError> {
        let mut hasher =
            blake3::Hasher::new_derive_key("arcweft.view.accepted-program-semantic-revision.v1");
        hasher.update(transcript);
        Self::try_from_bytes(*hasher.finalize().as_bytes())
    }

    pub fn try_from_bytes(bytes: [u8; 32]) -> Result<Self, ViewIdentityError> {
        if bytes == [0; 32] {
            return Err(ViewIdentityError::ZeroAcceptedRevision);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    pub fn to_hex(self) -> String {
        const LOWER_HEX: &[u8; 16] = b"0123456789abcdef";

        let mut encoded = String::with_capacity(64);
        for byte in self.0 {
            encoded.push(char::from(LOWER_HEX[usize::from(byte >> 4)]));
            encoded.push(char::from(LOWER_HEX[usize::from(byte & 0x0f)]));
        }
        encoded
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

impl Serialize for ViewProgramId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ViewProgramId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_new(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

impl Serialize for AcceptedViewProgramRevision {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_hex())
    }
}

impl<'de> Deserialize<'de> for AcceptedViewProgramRevision {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = String::deserialize(deserializer)?;
        let bytes = decode_revision(&encoded).map_err(serde::de::Error::custom)?;
        Self::try_from_bytes(bytes).map_err(serde::de::Error::custom)
    }
}

fn decode_revision(encoded: &str) -> Result<[u8; 32], ViewIdentityError> {
    if encoded.len() != 64
        || !encoded
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ViewIdentityError::NonCanonicalAcceptedRevision);
    }

    let mut decoded = [0; 32];
    for (index, pair) in encoded.as_bytes().chunks_exact(2).enumerate() {
        decoded[index] = (hex_value(pair[0]) << 4) | hex_value(pair[1]);
    }
    Ok(decoded)
}

const fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => byte - b'a' + 10,
        _ => unreachable!(),
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
    use super::{
        AcceptedViewProgramRevision, ViewId, ViewIdError, ViewIdentityError, ViewProgramId,
    };
    use arcweft_id::{DeclarationName, PublicId};

    #[test]
    fn stable_view_identities_serialize_as_validated_public_id_strings() {
        let view = ViewId::try_new("view.dialogue.standard").unwrap();
        let program = ViewProgramId::try_new("view-program.dialogue").unwrap();

        assert_eq!(
            serde_json::to_string(&view).unwrap(),
            r#""view.dialogue.standard""#
        );
        assert_eq!(
            serde_json::to_string(&program).unwrap(),
            r#""view-program.dialogue""#
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
    fn authored_module_identity_is_owned_by_the_view_id_boundary() {
        let name = DeclarationName::try_new("Dialogue").unwrap();
        assert_eq!(
            ViewId::try_from_module_name(std::iter::empty(), &name)
                .unwrap()
                .as_str(),
            "view.Dialogue"
        );
        assert_eq!(
            ViewId::try_from_module_name(["ui", "mobile"], &name)
                .unwrap()
                .as_str(),
            "view.ui.mobile.Dialogue"
        );
        assert!(ViewId::try_from_module_name([""], &name).is_err());
    }

    #[test]
    fn accepted_revision_uses_only_canonical_lowercase_hex() {
        let revision = AcceptedViewProgramRevision::try_from_bytes([0xab; 32]).unwrap();
        let encoded = format!("\"{}\"", "ab".repeat(32));

        assert_eq!(serde_json::to_string(&revision).unwrap(), encoded);
        assert_eq!(
            serde_json::from_str::<AcceptedViewProgramRevision>(&encoded).unwrap(),
            revision
        );
        assert!(
            serde_json::from_str::<AcceptedViewProgramRevision>(&format!(
                "\"{}\"",
                "AB".repeat(32)
            ))
            .is_err()
        );
        assert_eq!(
            AcceptedViewProgramRevision::try_from_bytes([0; 32]),
            Err(ViewIdentityError::ZeroAcceptedRevision)
        );
        assert!(
            serde_json::from_str::<AcceptedViewProgramRevision>(&format!(
                "\"{}\"",
                "00".repeat(32)
            ))
            .is_err()
        );
    }

    #[test]
    fn semantic_revision_is_domain_owned_and_content_sensitive() {
        let first = AcceptedViewProgramRevision::try_for_semantic_transcript(b"program-a").unwrap();
        let first_again =
            AcceptedViewProgramRevision::try_for_semantic_transcript(b"program-a").unwrap();
        let second =
            AcceptedViewProgramRevision::try_for_semantic_transcript(b"program-b").unwrap();

        assert_eq!(first, first_again);
        assert_ne!(first, second);
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
}
