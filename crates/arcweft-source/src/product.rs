//! Revision-bound source identities used by compiled product records.

use std::fmt::Write as _;

use arcweft_id::{IdError, PublicId};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::{SourceDocumentId, SourceDocumentIdentity, SourceRevision};

/// Maximum bytes admitted while deriving a stable product source identity.
pub const MAX_PRODUCT_SOURCE_ID_INPUT_BYTES: usize = 4_096;

/// Stable product identity derived only from one logical source-document ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductSourceId(PublicId);

/// Exact revision-bound source reference carried by compiled product records.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductSourceRef {
    id: ProductSourceId,
    revision: SourceRevision,
    source_len: u64,
}

/// Failure to derive or decode a product source identity.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ProductSourceIdentityError {
    #[error("source document ID contains {bytes} bytes, exceeding limit {limit}")]
    DocumentIdTooLong {
        id: SourceDocumentId,
        bytes: usize,
        limit: usize,
    },
    #[error("product source identity arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("invalid product source public ID")]
    InvalidPublicId,
}

impl ProductSourceId {
    /// Derives the canonical product identity for one logical source document.
    pub fn try_for_document_id(id: &SourceDocumentId) -> Result<Self, ProductSourceIdentityError> {
        let bytes = id.as_str().len();
        if bytes > MAX_PRODUCT_SOURCE_ID_INPUT_BYTES {
            return Err(ProductSourceIdentityError::DocumentIdTooLong {
                id: id.clone(),
                bytes,
                limit: MAX_PRODUCT_SOURCE_ID_INPUT_BYTES,
            });
        }
        let length =
            u32::try_from(bytes).map_err(|_| ProductSourceIdentityError::ArithmeticOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-product-source-id-v1\0");
        hasher.update(&length.to_le_bytes());
        hasher.update(id.as_str().as_bytes());
        let digest = hasher.finalize();
        let mut value = String::with_capacity(7 + 64);
        value.push_str("source.");
        digest.as_bytes().iter().try_for_each(|byte| {
            write!(&mut value, "{byte:02x}")
                .map_err(|_| ProductSourceIdentityError::ArithmeticOverflow)
        })?;
        PublicId::try_new(value)
            .map(Self)
            .map_err(|_| ProductSourceIdentityError::InvalidPublicId)
    }

    /// Decodes one already-derived canonical product source public ID.
    pub fn try_from_encoded(value: String) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl ProductSourceRef {
    /// Projects one exact source identity without copying source text.
    pub fn try_for_identity(
        identity: &SourceDocumentIdentity,
    ) -> Result<Self, ProductSourceIdentityError> {
        Ok(Self {
            id: ProductSourceId::try_for_document_id(identity.id())?,
            revision: identity.revision(),
            source_len: identity.source_len(),
        })
    }

    /// Reconstructs a checked product source reference from its closed wire
    /// components. Callers cannot omit the revision or source length.
    pub const fn new(id: ProductSourceId, revision: SourceRevision, source_len: u64) -> Self {
        Self {
            id,
            revision,
            source_len,
        }
    }

    pub const fn id(&self) -> &ProductSourceId {
        &self.id
    }

    pub const fn revision(&self) -> SourceRevision {
        self.revision
    }

    pub const fn source_len(&self) -> u64 {
        self.source_len
    }
}

impl Serialize for ProductSourceId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ProductSourceId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from_encoded(String::deserialize(deserializer)?).map_err(serde::de::Error::custom)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProductSourceRefWire {
    id: ProductSourceId,
    revision: [u8; 32],
    source_len: u64,
}

impl Serialize for ProductSourceRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ProductSourceRefWire {
            id: self.id.clone(),
            revision: *self.revision.as_bytes(),
            source_len: self.source_len,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProductSourceRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ProductSourceRefWire::deserialize(deserializer)?;
        Ok(Self::new(
            wire.id,
            SourceRevision::from_bytes(wire.revision),
            wire.source_len,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{ProductSourceId, ProductSourceRef};
    use crate::{SourceDocument, SourceDocumentId, SourceName};

    #[test]
    fn product_source_reference_retains_exact_revision_and_length() {
        let document = SourceDocument::try_new(
            SourceDocumentId::try_new("pkg/main.arcw").expect("source id"),
            SourceName::Memory,
            "flow main {}",
        )
        .expect("source document");
        let reference =
            ProductSourceRef::try_for_identity(document.identity()).expect("product source ref");
        assert_eq!(reference.revision(), document.identity().revision());
        assert_eq!(reference.source_len(), document.identity().source_len());
        assert_eq!(
            reference.id(),
            &ProductSourceId::try_for_document_id(document.identity().id()).expect("product id")
        );
    }
}
