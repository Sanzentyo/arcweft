use std::{borrow::Cow, collections::BTreeMap};

use arcweft_manifest_model::NormalizedProjectPath;

use arcweft_source::{
    MAX_PRODUCT_SOURCE_ID_INPUT_BYTES, MAX_REGISTRATION_SOURCE_BYTES, ProductSourceId,
    ProductSourceIdentityError, ProductSourceRef, SourceDocument, SourceDocumentId,
    SourceDocumentIdentity, SourceName, SourceRevision, SourceSetRevision, SourceSetRevisionError,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::SourceMapBuildError;

pub const MAX_SOURCE_MAP_DOCUMENTS: usize = 65_536;
pub const MAX_SOURCE_DISPLAY_NAME_BYTES: usize = 4_096;
pub const MAX_SOURCE_BYTES_PER_DOCUMENT: u64 = MAX_REGISTRATION_SOURCE_BYTES;
pub const MAX_SOURCE_MAP_TOTAL_UTF8_BYTES: u64 = 67_108_864;

/// Shared source-map admission before document inventories or text are copied.
/// The caller supplies its complete document count before materializing records.
pub(super) struct SourceMapAdmission {
    total_utf8_bytes: u64,
}

impl SourceMapAdmission {
    pub(super) fn try_new(document_count: usize) -> Result<Self, SourceMapBuildError> {
        if document_count > MAX_SOURCE_MAP_DOCUMENTS {
            return Err(SourceMapBuildError::TooManyDocuments {
                actual: document_count,
                limit: MAX_SOURCE_MAP_DOCUMENTS,
            });
        }
        Ok(Self {
            total_utf8_bytes: 0,
        })
    }

    pub(super) fn admit_document(
        &mut self,
        id: &SourceDocumentId,
        utf8_bytes: u64,
    ) -> Result<(), SourceMapBuildError> {
        if utf8_bytes > MAX_SOURCE_BYTES_PER_DOCUMENT {
            return Err(SourceMapBuildError::DocumentTooLarge {
                id: id.clone(),
                bytes: utf8_bytes,
                limit: MAX_SOURCE_BYTES_PER_DOCUMENT,
            });
        }
        let total_utf8_bytes = self
            .total_utf8_bytes
            .checked_add(utf8_bytes)
            .ok_or(SourceMapBuildError::ArithmeticOverflow)?;
        if total_utf8_bytes > MAX_SOURCE_MAP_TOTAL_UTF8_BYTES {
            return Err(SourceMapBuildError::TotalBytesExceeded {
                actual: total_utf8_bytes,
                limit: MAX_SOURCE_MAP_TOTAL_UTF8_BYTES,
            });
        }
        self.total_utf8_bytes = total_utf8_bytes;
        Ok(())
    }
}

/// Exact source document embedded in a canonical product source map.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapDocument {
    id: ProductSourceId,
    identity: SourceDocumentIdentity,
    display_name: SourceName,
    utf8: Box<str>,
}

/// One accepted source document and its product-facing display metadata.
///
/// Product naming never changes the document identity, original diagnostic
/// display name or exact source bytes. The section constructor validates names.
#[derive(Clone, Debug)]
pub struct SourceMapDocumentInput<'a> {
    document: &'a SourceDocument,
    display_name: Cow<'a, SourceName>,
}

impl<'a> SourceMapDocumentInput<'a> {
    /// Retains an already portable or non-file source name.
    pub fn for_document(document: &'a SourceDocument) -> Self {
        Self {
            document,
            display_name: Cow::Borrowed(document.display_name()),
        }
    }

    /// Publishes a package-owned authored file coordinate as product metadata.
    pub fn for_project_file(document: &'a SourceDocument, path: &NormalizedProjectPath) -> Self {
        Self {
            document,
            display_name: Cow::Owned(SourceName::path(path.as_str())),
        }
    }
}

/// One immutable, canonically ordered multi-source product section.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceMapSection {
    source_set_revision: SourceSetRevision,
    pub(super) primary_document_id: Option<SourceDocumentId>,
    documents: Vec<SourceMapDocument>,
}

impl SourceMapDocument {
    pub const fn id(&self) -> &ProductSourceId {
        &self.id
    }

    pub const fn document_id(&self) -> &SourceDocumentId {
        self.identity.id()
    }

    pub const fn source_identity(&self) -> &SourceDocumentIdentity {
        &self.identity
    }

    pub const fn display_name(&self) -> &SourceName {
        &self.display_name
    }

    pub const fn revision(&self) -> SourceRevision {
        self.identity.revision()
    }

    pub const fn source_len(&self) -> u64 {
        self.identity.source_len()
    }

    /// Projects the exact lower-layer product source reference retained by
    /// static product records.
    pub fn product_source_ref(&self) -> ProductSourceRef {
        ProductSourceRef::new(self.id.clone(), self.revision(), self.source_len())
    }

    pub fn text(&self) -> &str {
        &self.utf8
    }
}

impl SourceMapSection {
    /// Checks a complete inventory's count before a producer builds its metadata.
    /// Constructors also validate their supplied inventory and exact UTF-8 extents.
    pub fn check_document_count(document_count: usize) -> Result<(), SourceMapBuildError> {
        SourceMapAdmission::try_new(document_count).map(|_| ())
    }

    /// Builds one canonical source inventory whose first supplied document is
    /// the semantic primary/root document. File display names must be portable.
    ///
    /// Document records are sorted by `ProductSourceId` for deterministic lookup
    /// and encoding; that sort never changes the explicit primary.
    pub fn try_from_documents(documents: &[&SourceDocument]) -> Result<Self, SourceMapBuildError> {
        Self::try_from_documents_with(documents, derive_product_source_id)
    }

    /// Projects exact source documents with owning product display metadata.
    pub fn try_from_inputs(
        inputs: &[SourceMapDocumentInput<'_>],
    ) -> Result<Self, SourceMapBuildError> {
        Self::try_from_inputs_with(inputs, derive_product_source_id)
    }

    fn try_from_documents_with(
        documents: &[&SourceDocument],
        derive_id: impl Fn(&SourceDocumentId) -> Result<ProductSourceId, SourceMapBuildError>,
    ) -> Result<Self, SourceMapBuildError> {
        Self::check_document_count(documents.len())?;
        let inputs = documents
            .iter()
            .map(|document| SourceMapDocumentInput::for_document(document))
            .collect::<Vec<_>>();
        Self::try_from_inputs_with(&inputs, derive_id)
    }

    fn try_from_inputs_with(
        inputs: &[SourceMapDocumentInput<'_>],
        derive_id: impl Fn(&SourceDocumentId) -> Result<ProductSourceId, SourceMapBuildError>,
    ) -> Result<Self, SourceMapBuildError> {
        let mut admission = SourceMapAdmission::try_new(inputs.len())?;
        // Bound source-byte copies before materializing immutable product records.
        for input in inputs {
            let identity = input.document.identity();
            let display_bytes = input.display_name.display_name().len();
            if display_bytes > MAX_SOURCE_DISPLAY_NAME_BYTES {
                return Err(SourceMapBuildError::DisplayNameTooLong {
                    id: identity.id().clone(),
                    bytes: display_bytes,
                    limit: MAX_SOURCE_DISPLAY_NAME_BYTES,
                });
            }
            if let SourceName::Path(path) = input.display_name.as_ref() {
                NormalizedProjectPath::new(path.as_str()).map_err(|source| {
                    SourceMapBuildError::InvalidDisplayPath {
                        id: identity.id().clone(),
                        source,
                    }
                })?;
            }
            admission.admit_document(identity.id(), identity.source_len())?;
        }
        let primary_document_id = inputs
            .first()
            .map(|input| input.document.identity().id().clone());
        let entries = inputs
            .iter()
            .map(|input| {
                Ok(SourceMapDocument {
                    id: derive_id(input.document.identity().id())?,
                    identity: input.document.identity().clone(),
                    display_name: input.display_name.clone().into_owned(),
                    utf8: input.document.text().into(),
                })
            })
            .collect::<Result<Vec<_>, SourceMapBuildError>>()?;
        Self::try_from_entries(entries, primary_document_id)
    }

    fn try_from_entries(
        mut documents: Vec<SourceMapDocument>,
        primary_document_id: Option<SourceDocumentId>,
    ) -> Result<Self, SourceMapBuildError> {
        let mut admission = SourceMapAdmission::try_new(documents.len())?;
        let mut by_document = BTreeMap::<SourceDocumentId, ProductSourceId>::new();
        let mut by_product = BTreeMap::<ProductSourceId, SourceDocumentId>::new();
        for document in &documents {
            let identity = document.source_identity();
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
            if let SourceName::Path(path) = document.display_name() {
                NormalizedProjectPath::new(path.as_str()).map_err(|source| {
                    SourceMapBuildError::InvalidDisplayPath {
                        id: identity.id().clone(),
                        source,
                    }
                })?;
            }
            admission.admit_document(identity.id(), identity.source_len())?;
            if by_document
                .insert(identity.id().clone(), document.id.clone())
                .is_some()
            {
                return Err(SourceMapBuildError::DuplicateDocument(
                    identity.id().clone(),
                ));
            }
            if let Some(first) = by_product.insert(document.id.clone(), identity.id().clone()) {
                return Err(SourceMapBuildError::ProductSourceIdCollision {
                    product: document.id.clone(),
                    first,
                    second: identity.id().clone(),
                });
            }
        }
        let source_set_revision = SourceSetRevision::try_for_identities(
            documents.iter().map(SourceMapDocument::source_identity),
        )
        .map_err(map_source_set_error)?;
        documents.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(Self {
            source_set_revision,
            primary_document_id,
            documents,
        })
    }

    /// Adds one exact document, preserving the existing admitted product names.
    /// No source document or identity is reconstructed from product text.
    pub fn try_with_document(
        mut self,
        document: &SourceDocument,
    ) -> Result<Self, SourceMapBuildError> {
        if let Some(existing) = self
            .documents
            .iter()
            .find(|existing| existing.document_id() == document.identity().id())
        {
            if existing.source_identity() == document.identity()
                && existing.text() == document.text()
            {
                // The accepted product name remains authoritative even when the
                // exact original document has an absolute diagnostic label.
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
        let mut admission = SourceMapAdmission::try_new(document_count)?;
        for existing in &self.documents {
            admission.admit_document(existing.document_id(), existing.source_len())?;
        }
        admission.admit_document(document.identity().id(), document.identity().source_len())?;
        let mut addition = Self::try_from_documents(&[document])?;
        let added = addition
            .documents
            .pop()
            .ok_or(SourceMapBuildError::ArithmeticOverflow)?;
        let primary = self
            .primary_document_id
            .or_else(|| Some(document.identity().id().clone()));
        self.documents.push(added);
        Self::try_from_entries(self.documents, primary)
    }

    /// Selects an exact authored source set from this already admitted map.
    /// The first requested identity remains the primary, independent of sorting.
    pub fn try_for_source_identities(
        &self,
        identities: &[&SourceDocumentIdentity],
    ) -> Result<Self, SourceMapBuildError> {
        let mut admission = SourceMapAdmission::try_new(identities.len())?;
        let selected = identities
            .iter()
            .map(|identity| {
                let id = derive_product_source_id(identity.id())?;
                let source = self
                    .get(&id)
                    .ok_or_else(|| SourceMapBuildError::MissingDocument(identity.id().clone()))?;
                if source.source_identity() != *identity {
                    return Err(SourceMapBuildError::DocumentIdentityMismatch {
                        expected: Box::new((*identity).clone()),
                        actual: Box::new(source.source_identity().clone()),
                    });
                }
                Ok(source)
            })
            .collect::<Result<Vec<_>, SourceMapBuildError>>()?;
        // Duplicate requests cannot force unbounded copies of an admitted document.
        for source in &selected {
            admission.admit_document(source.document_id(), source.source_len())?;
        }
        let documents = selected.into_iter().cloned().collect();
        Self::try_from_entries(
            documents,
            identities.first().map(|identity| identity.id().clone()),
        )
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

fn derive_product_source_id(id: &SourceDocumentId) -> Result<ProductSourceId, SourceMapBuildError> {
    ProductSourceId::try_for_document_id(id).map_err(|error| match error {
        ProductSourceIdentityError::DocumentIdTooLong { id, bytes, limit } => {
            SourceMapBuildError::DocumentIdTooLong { id, bytes, limit }
        }
        ProductSourceIdentityError::ArithmeticOverflow
        | ProductSourceIdentityError::InvalidPublicId => SourceMapBuildError::ArithmeticOverflow,
    })
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
