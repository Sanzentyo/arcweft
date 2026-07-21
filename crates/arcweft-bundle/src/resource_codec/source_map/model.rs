use std::collections::BTreeMap;
use std::fmt::Write as _;

use arcweft_id::{IdError, PublicId};
use arcweft_source::{
    MAX_REGISTRATION_SOURCE_BYTES, SourceDocument, SourceDocumentId, SourceName, SourceRevision,
    SourceSetRevision, SourceSetRevisionError,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::SourceMapBuildError;

pub const MAX_SOURCE_MAP_DOCUMENTS: usize = 65_536;
pub const MAX_PRODUCT_SOURCE_ID_INPUT_BYTES: usize = 4_096;
pub const MAX_SOURCE_DISPLAY_NAME_BYTES: usize = 4_096;
pub const MAX_SOURCE_BYTES_PER_DOCUMENT: u64 = MAX_REGISTRATION_SOURCE_BYTES;
pub const MAX_SOURCE_MAP_TOTAL_UTF8_BYTES: u64 = 67_108_864;

/// Stable product identity derived only from one logical source-document ID.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProductSourceId(PublicId);

/// Exact source document embedded in a canonical product source map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapDocument {
    id: ProductSourceId,
    document_id: SourceDocumentId,
    display_name: SourceName,
    revision: SourceRevision,
    source_len: u64,
    utf8: Box<str>,
}

/// One immutable, canonically ordered multi-source product section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapSection {
    source_set_revision: SourceSetRevision,
    pub(super) primary_document_id: Option<SourceDocumentId>,
    documents: Vec<SourceMapDocument>,
}

impl ProductSourceId {
    pub fn try_for_document_id(id: &SourceDocumentId) -> Result<Self, SourceMapBuildError> {
        let bytes = id.as_str().len();
        if bytes > MAX_PRODUCT_SOURCE_ID_INPUT_BYTES {
            return Err(SourceMapBuildError::DocumentIdTooLong {
                id: id.clone(),
                bytes,
                limit: MAX_PRODUCT_SOURCE_ID_INPUT_BYTES,
            });
        }
        let length = u32::try_from(bytes).map_err(|_| SourceMapBuildError::ArithmeticOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-product-source-id-v1\0");
        hasher.update(&length.to_le_bytes());
        hasher.update(id.as_str().as_bytes());
        let digest = hasher.finalize();
        let mut value = String::with_capacity(7 + 64);
        value.push_str("source.");
        digest.as_bytes().iter().try_for_each(|byte| {
            write!(&mut value, "{byte:02x}").map_err(|_| SourceMapBuildError::ArithmeticOverflow)
        })?;
        PublicId::try_new(value)
            .map(Self)
            .map_err(|_| SourceMapBuildError::ArithmeticOverflow)
    }

    pub const fn public_id(&self) -> &PublicId {
        &self.0
    }

    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    pub(crate) fn try_from_encoded(value: String) -> Result<Self, IdError> {
        PublicId::try_new(value).map(Self)
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

impl SourceMapDocument {
    pub const fn id(&self) -> &ProductSourceId {
        &self.id
    }

    pub const fn document_id(&self) -> &SourceDocumentId {
        &self.document_id
    }

    pub const fn display_name(&self) -> &SourceName {
        &self.display_name
    }

    pub const fn revision(&self) -> SourceRevision {
        self.revision
    }

    pub const fn source_len(&self) -> u64 {
        self.source_len
    }

    pub fn text(&self) -> &str {
        &self.utf8
    }
}

impl SourceMapSection {
    /// Builds one canonical source inventory whose first supplied document is
    /// the semantic primary/root document.
    ///
    /// Document records are sorted by `ProductSourceId` for deterministic
    /// lookup and encoding; that sort never changes the explicit primary.
    pub fn try_from_documents(documents: &[&SourceDocument]) -> Result<Self, SourceMapBuildError> {
        Self::try_from_documents_with(documents, ProductSourceId::try_for_document_id)
    }

    fn try_from_documents_with(
        documents: &[&SourceDocument],
        derive_id: impl Fn(&SourceDocumentId) -> Result<ProductSourceId, SourceMapBuildError>,
    ) -> Result<Self, SourceMapBuildError> {
        if documents.len() > MAX_SOURCE_MAP_DOCUMENTS {
            return Err(SourceMapBuildError::TooManyDocuments {
                actual: documents.len(),
                limit: MAX_SOURCE_MAP_DOCUMENTS,
            });
        }

        let primary_document_id = documents
            .first()
            .map(|document| document.identity().id().clone());
        let mut by_document = BTreeMap::<SourceDocumentId, ProductSourceId>::new();
        let mut by_product = BTreeMap::<ProductSourceId, SourceDocumentId>::new();
        let mut total_bytes = 0_u64;
        for document in documents {
            let identity = document.identity();
            let id_bytes = identity.id().as_str().len();
            if id_bytes > MAX_PRODUCT_SOURCE_ID_INPUT_BYTES {
                return Err(SourceMapBuildError::DocumentIdTooLong {
                    id: identity.id().clone(),
                    bytes: id_bytes,
                    limit: MAX_PRODUCT_SOURCE_ID_INPUT_BYTES,
                });
            }
            let display_bytes = document.display_name().display_name().len();
            if display_bytes > MAX_SOURCE_DISPLAY_NAME_BYTES {
                return Err(SourceMapBuildError::DisplayNameTooLong {
                    id: identity.id().clone(),
                    bytes: display_bytes,
                    limit: MAX_SOURCE_DISPLAY_NAME_BYTES,
                });
            }
            if identity.source_len() > MAX_SOURCE_BYTES_PER_DOCUMENT {
                return Err(SourceMapBuildError::DocumentTooLarge {
                    id: identity.id().clone(),
                    bytes: identity.source_len(),
                    limit: MAX_SOURCE_BYTES_PER_DOCUMENT,
                });
            }
            total_bytes = total_bytes
                .checked_add(identity.source_len())
                .ok_or(SourceMapBuildError::ArithmeticOverflow)?;
            if total_bytes > MAX_SOURCE_MAP_TOTAL_UTF8_BYTES {
                return Err(SourceMapBuildError::TotalBytesExceeded {
                    actual: total_bytes,
                    limit: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES,
                });
            }
            let product = derive_id(identity.id())?;
            if by_document
                .insert(identity.id().clone(), product.clone())
                .is_some()
            {
                return Err(SourceMapBuildError::DuplicateDocument(
                    identity.id().clone(),
                ));
            }
            if let Some(first) = by_product.insert(product.clone(), identity.id().clone()) {
                return Err(SourceMapBuildError::ProductSourceIdCollision {
                    product,
                    first,
                    second: identity.id().clone(),
                });
            }
        }

        let source_set_revision = SourceSetRevision::try_for_identities(
            documents.iter().map(|document| document.identity()),
        )
        .map_err(map_source_set_error)?;
        let mut entries = documents
            .iter()
            .map(|document| {
                let identity = document.identity();
                let id = by_document
                    .get(identity.id())
                    .cloned()
                    .ok_or(SourceMapBuildError::ArithmeticOverflow)?;
                Ok(SourceMapDocument {
                    id,
                    document_id: identity.id().clone(),
                    display_name: document.display_name().clone(),
                    revision: identity.revision(),
                    source_len: identity.source_len(),
                    utf8: document.text().into(),
                })
            })
            .collect::<Result<Vec<_>, SourceMapBuildError>>()?;
        entries.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self {
            source_set_revision,
            primary_document_id,
            documents: entries,
        })
    }

    /// Adds one exact document and recomputes the deterministic source-set
    /// revision, rejecting a reused document ID with different content.
    pub fn try_with_document(self, document: &SourceDocument) -> Result<Self, SourceMapBuildError> {
        if let Some(existing) = self
            .documents
            .iter()
            .find(|existing| existing.document_id() == document.identity().id())
        {
            if existing.display_name() == document.display_name()
                && existing.revision() == document.identity().revision()
                && existing.source_len() == document.identity().source_len()
                && existing.text() == document.text()
            {
                return Ok(self);
            }
            return Err(SourceMapBuildError::DuplicateDocument(
                document.identity().id().clone(),
            ));
        }

        let document_count = self
            .documents
            .len()
            .checked_add(1)
            .ok_or(SourceMapBuildError::ArithmeticOverflow)?;
        if document_count > MAX_SOURCE_MAP_DOCUMENTS {
            return Err(SourceMapBuildError::TooManyDocuments {
                actual: document_count,
                limit: MAX_SOURCE_MAP_DOCUMENTS,
            });
        }

        let primary_document_id = self.primary_document_id.clone();
        let mut owned = self
            .documents
            .iter()
            .map(|existing| {
                SourceDocument::try_new(
                    existing.document_id().clone(),
                    existing.display_name().clone(),
                    existing.text().to_owned(),
                )
                .map_err(|_| SourceMapBuildError::ArithmeticOverflow)
            })
            .collect::<Result<Vec<_>, _>>()?;
        owned.push(document.clone());
        let mut rebuilt = Self::try_from_documents(&owned.iter().collect::<Vec<_>>())?;
        rebuilt.primary_document_id = primary_document_id.or(rebuilt.primary_document_id);
        Ok(rebuilt)
    }

    pub const fn source_set_revision(&self) -> SourceSetRevision {
        self.source_set_revision
    }

    /// Root/primary document supplied first when this source set was built.
    pub const fn primary_document_id(&self) -> Option<&SourceDocumentId> {
        self.primary_document_id.as_ref()
    }

    /// Exact primary document independent of canonical hash ordering.
    pub fn primary_document(&self) -> Option<&SourceMapDocument> {
        let id = self.primary_document_id.as_ref()?;
        self.documents
            .iter()
            .find(|document| document.document_id() == id)
    }

    pub fn documents(&self) -> impl ExactSizeIterator<Item = &SourceMapDocument> {
        self.documents.iter()
    }

    pub fn get(&self, id: &ProductSourceId) -> Option<&SourceMapDocument> {
        self.documents
            .binary_search_by(|document| document.id.cmp(id))
            .ok()
            .map(|index| &self.documents[index])
    }
}

impl Serialize for SourceMapSection {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.encode_canonical_section()
            .map_err(serde::ser::Error::custom)?
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for SourceMapSection {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        Self::decode_canonical_section(&bytes).map_err(serde::de::Error::custom)
    }
}

fn map_source_set_error(error: SourceSetRevisionError) -> SourceMapBuildError {
    match error {
        SourceSetRevisionError::ConflictingDocument { id, .. } => {
            SourceMapBuildError::DuplicateDocument(id)
        }
        SourceSetRevisionError::DocumentCountOverflow
        | SourceSetRevisionError::DocumentIdLengthOverflow { .. } => {
            SourceMapBuildError::ArithmeticOverflow
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};

    use super::{ProductSourceId, SourceMapSection};
    use crate::resource_codec::SourceMapBuildError;

    fn document(id: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("source id"),
            SourceName::path(id),
            Arc::<str>::from(""),
        )
        .expect("source document")
    }

    #[test]
    fn collision_policy_rejects_distinct_logical_documents_before_construction() {
        let first = document("a.arcw");
        let second = document("b.arcw");
        let collision =
            ProductSourceId::try_for_document_id(first.identity().id()).expect("derived source id");

        let error =
            SourceMapSection::try_from_documents_with(
                &[&first, &second],
                |_| Ok(collision.clone()),
            )
            .expect_err("collision must reject");

        assert!(matches!(
            error,
            SourceMapBuildError::ProductSourceIdCollision { .. }
        ));
    }
}
