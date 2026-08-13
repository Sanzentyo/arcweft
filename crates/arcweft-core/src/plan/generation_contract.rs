//! Scalar identities used by raw and admitted runtime-generation contracts.
//!
//! These values are typed serialized evidence. Possessing or constructing one
//! does not admit a plan, program, catalog, producer, or runtime value.

use serde::{Deserialize, Serialize};

macro_rules! public_digest_newtype {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[repr(transparent)]
        #[serde(transparent)]
        pub struct $name([u8; 32]);

        impl $name {
            #[must_use]
            pub const fn from_bytes(bytes: [u8; 32]) -> Self {
                Self(bytes)
            }

            #[must_use]
            pub const fn as_bytes(&self) -> &[u8; 32] {
                &self.0
            }
        }
    };
}

public_digest_newtype!(RuntimeGenerationIdentity);
public_digest_newtype!(RuntimeProjectRootId);
public_digest_newtype!(RuntimeProducerRootId);
public_digest_newtype!(RuntimeViewId);
public_digest_newtype!(RuntimeCharacterCatalogDigest);
public_digest_newtype!(RuntimeViewCatalogDigest);

/// Digest of the canonical `CharacterDialogue` runtime custom-field catalog.
///
/// Construction remains private to core until the catalog owner computes the
/// version-1 canonical transcript. Raw deserialization is quarantine data and
/// does not grant operational authority.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct CharacterDialogueRuntimeCustomFieldDigest([u8; 32]);

impl CharacterDialogueRuntimeCustomFieldDigest {
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_generation_scalars_preserve_all_bytes() {
        let bytes = core::array::from_fn(|index| u8::try_from(index).expect("index fits u8"));
        assert_eq!(
            RuntimeGenerationIdentity::from_bytes(bytes).as_bytes(),
            &bytes
        );
        assert_eq!(RuntimeProjectRootId::from_bytes(bytes).as_bytes(), &bytes);
        assert_eq!(RuntimeProducerRootId::from_bytes(bytes).as_bytes(), &bytes);
        assert_eq!(RuntimeViewId::from_bytes(bytes).as_bytes(), &bytes);
        assert_eq!(
            RuntimeCharacterCatalogDigest::from_bytes(bytes).as_bytes(),
            &bytes
        );
        assert_eq!(
            RuntimeViewCatalogDigest::from_bytes(bytes).as_bytes(),
            &bytes
        );
    }

    #[test]
    fn scalar_serde_is_transparent_and_exact() {
        let identity = RuntimeGenerationIdentity::from_bytes([7; 32]);
        let encoded = serde_json::to_string(&identity).expect("serialize identity");
        assert_eq!(
            serde_json::from_str::<RuntimeGenerationIdentity>(&encoded)
                .expect("deserialize identity"),
            identity
        );
        assert_eq!(encoded.matches('7').count(), 32);
    }

    #[test]
    fn custom_digest_deserialization_is_only_raw_evidence() {
        let raw = serde_json::to_string(&[11_u8; 32]).expect("serialize raw digest bytes");
        let digest = serde_json::from_str::<CharacterDialogueRuntimeCustomFieldDigest>(&raw)
            .expect("deserialize raw digest evidence");
        assert_eq!(digest.as_bytes(), &[11; 32]);
    }
}
