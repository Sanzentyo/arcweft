use std::{collections::BTreeMap, fmt, sync::Arc};

use thiserror::Error;

use crate::{SourceName, SourceRange};

/// Maximum number of exact source bytes accepted by registration decoders.
pub const MAX_REGISTRATION_SOURCE_BYTES: u64 = 8_388_608;

/// Stable identity for one logical source document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceDocumentId(String);

impl SourceDocumentId {
    pub fn try_new(value: impl Into<String>) -> Result<Self, SourceDocumentIdError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SourceDocumentIdError::Empty);
        }
        if let Some((byte, _)) = value
            .char_indices()
            .find(|(_, character)| character.is_control())
        {
            return Err(SourceDocumentIdError::Control { byte });
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceDocumentId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// BLAKE3 digest of the exact UTF-8 bytes in a source document.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevision([u8; 32]);

impl SourceRevision {
    #[must_use]
    pub fn for_utf8(source: &str) -> Self {
        Self(*blake3::hash(source.as_bytes()).as_bytes())
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Reconstructs a revision retained by a typed product reference.
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Lowercase hexadecimal spelling used by content-addressed identities.
    pub fn to_hex(self) -> String {
        self.0
            .iter()
            .fold(String::with_capacity(64), |mut output, byte| {
                use std::fmt::Write as _;
                write!(&mut output, "{byte:02x}").expect("writing to String cannot fail");
                output
            })
    }
}

/// Canonical revision of a complete set of source identities.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSetRevision([u8; 32]);

impl SourceSetRevision {
    pub fn try_for_identities<'a>(
        identities: impl IntoIterator<Item = &'a SourceDocumentIdentity>,
    ) -> Result<Self, SourceSetRevisionError> {
        let mut by_id = BTreeMap::<&SourceDocumentId, (SourceRevision, u64)>::new();
        for identity in identities {
            match by_id.get(identity.id()) {
                None => {
                    by_id.insert(identity.id(), (identity.revision(), identity.source_len()));
                }
                Some(&(first_revision, first_len))
                    if first_revision == identity.revision()
                        && first_len == identity.source_len() => {}
                Some(&(first_revision, first_len)) => {
                    return Err(SourceSetRevisionError::ConflictingDocument {
                        id: identity.id().clone(),
                        first_revision,
                        first_len,
                        conflicting_revision: identity.revision(),
                        conflicting_len: identity.source_len(),
                    });
                }
            }
        }

        let count = u32::try_from(by_id.len())
            .map_err(|_| SourceSetRevisionError::DocumentCountOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(b"arcweft-source-set-revision-v1\0");
        hasher.update(&1_u32.to_le_bytes());
        hasher.update(&count.to_le_bytes());
        for (id, (revision, source_len)) in by_id {
            let id_len = u32::try_from(id.as_str().len()).map_err(|_| {
                SourceSetRevisionError::DocumentIdLengthOverflow {
                    id: id.clone(),
                    length: id.as_str().len(),
                }
            })?;
            hasher.update(&id_len.to_le_bytes());
            hasher.update(id.as_str().as_bytes());
            hasher.update(revision.as_bytes());
            hasher.update(&source_len.to_le_bytes());
        }
        Ok(Self(*hasher.finalize().as_bytes()))
    }

    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Revision-bound identity shared by every span into one source document.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceDocumentIdentity {
    id: SourceDocumentId,
    revision: SourceRevision,
    source_len: u64,
}

impl SourceDocumentIdentity {
    pub const fn id(&self) -> &SourceDocumentId {
        &self.id
    }

    pub const fn revision(&self) -> SourceRevision {
        self.revision
    }

    pub const fn source_len(&self) -> u64 {
        self.source_len
    }
}

/// Immutable UTF-8 source text, display metadata, and revision identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceDocument {
    identity: Arc<SourceDocumentIdentity>,
    display_name: SourceName,
    text: Arc<str>,
}

impl SourceDocument {
    pub fn try_new(
        id: SourceDocumentId,
        display_name: SourceName,
        text: impl Into<Arc<str>>,
    ) -> Result<Self, SourceDocumentError> {
        let text = text.into();
        let source_len = u64::try_from(text.len())
            .map_err(|_| SourceDocumentError::LengthOverflow { length: text.len() })?;
        let identity = Arc::new(SourceDocumentIdentity {
            id,
            revision: SourceRevision::for_utf8(&text),
            source_len,
        });
        Ok(Self {
            identity,
            display_name,
            text,
        })
    }

    pub fn identity(&self) -> &SourceDocumentIdentity {
        &self.identity
    }

    pub const fn display_name(&self) -> &SourceName {
        &self.display_name
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self, range: SourceRange) -> Result<SourceSpan, SourceSpanError> {
        if range.start() > range.end() {
            return Err(SourceSpanError::Reversed);
        }
        if range.end() > self.text.len() {
            return Err(SourceSpanError::OutOfBounds);
        }
        if !self.text.is_char_boundary(range.start()) || !self.text.is_char_boundary(range.end()) {
            return Err(SourceSpanError::NotUtf8Boundary);
        }
        Ok(SourceSpan {
            source: Arc::clone(&self.identity),
            range,
        })
    }

    /// Zero-width span at the beginning of this exact document revision.
    #[must_use]
    pub fn start_span(&self) -> SourceSpan {
        SourceSpan {
            source: Arc::clone(&self.identity),
            range: SourceRange::new(0, 0),
        }
    }

    /// Zero-width span immediately after the final byte of this exact document revision.
    #[must_use]
    pub fn end_span(&self) -> SourceSpan {
        SourceSpan {
            source: Arc::clone(&self.identity),
            range: SourceRange::new(self.text.len(), self.text.len()),
        }
    }
}

/// Byte range permanently bound to one exact source document revision.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceSpan {
    source: Arc<SourceDocumentIdentity>,
    range: SourceRange,
}

impl SourceSpan {
    pub fn source(&self) -> &SourceDocumentIdentity {
        &self.source
    }

    /// Shared revision identity retained by source-aware intermediate records.
    pub fn source_identity(&self) -> Arc<SourceDocumentIdentity> {
        Arc::clone(&self.source)
    }

    pub const fn range(&self) -> SourceRange {
        self.range
    }

    /// Verifies that this span belongs to the exact supplied document revision.
    pub fn validate_for(&self, document: &SourceDocument) -> Result<(), SourceSpanValidationError> {
        if self.source.id() != document.identity().id() {
            return Err(SourceSpanValidationError::WrongDocument {
                expected: document.identity().id().clone(),
                actual: self.source.id().clone(),
            });
        }
        if self.source.revision() != document.identity().revision() {
            return Err(SourceSpanValidationError::WrongRevision {
                expected: document.identity().revision(),
                actual: self.source.revision(),
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceDocumentIdError {
    #[error("source document id must not be empty")]
    Empty,
    #[error("source document id contains a control character at byte {byte}")]
    Control { byte: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceDocumentError {
    #[error("source length does not fit the revision-bound identity")]
    LengthOverflow { length: usize },
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum SourceSetRevisionError {
    #[error("source-set document count exceeds u32::MAX")]
    DocumentCountOverflow,
    #[error("source document id `{id}` has {length} bytes, exceeding u32::MAX")]
    DocumentIdLengthOverflow { id: SourceDocumentId, length: usize },
    #[error("source document id `{id}` occurs with conflicting revision or length")]
    ConflictingDocument {
        id: SourceDocumentId,
        first_revision: SourceRevision,
        first_len: u64,
        conflicting_revision: SourceRevision,
        conflicting_len: u64,
    },
}

#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceSpanError {
    #[error("source range start exceeds its end")]
    Reversed,
    #[error("source range lies outside the bound document")]
    OutOfBounds,
    #[error("source range endpoint is not a UTF-8 boundary")]
    NotUtf8Boundary,
}

/// Failure to use a span with a different document identity or revision.
#[derive(Clone, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum SourceSpanValidationError {
    /// The span and supplied document have different logical identifiers.
    #[error("source span belongs to document `{actual}`, not `{expected}`")]
    WrongDocument {
        expected: SourceDocumentId,
        actual: SourceDocumentId,
    },
    /// The span and supplied document have different content revisions.
    #[error("source span belongs to revision {actual:?}, not {expected:?}")]
    WrongRevision {
        expected: SourceRevision,
        actual: SourceRevision,
    },
}

#[cfg(test)]
mod tests {
    use std::fmt::Write as _;

    use super::{
        SourceDocument, SourceDocumentId, SourceDocumentIdError, SourceRevision, SourceSetRevision,
        SourceSetRevisionError, SourceSpanError, SourceSpanValidationError,
    };
    use crate::{SourceName, SourceRange};

    fn document(id: &str, text: &str) -> SourceDocument {
        SourceDocument::try_new(
            SourceDocumentId::try_new(id).expect("document id"),
            SourceName::path(format!("/{id}")),
            text,
        )
        .expect("source document")
    }

    #[test]
    fn source_revision_vectors() {
        assert_eq!(
            hex(SourceRevision::for_utf8("").as_bytes()),
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
        assert_eq!(
            hex(SourceRevision::for_utf8("abc").as_bytes()),
            "6437b3ac38465133ffb63b75273a8db548c558465d79db03fd359c6cd5bd9d85"
        );
    }

    #[test]
    fn source_document_id_validation() {
        assert_eq!(
            SourceDocumentId::try_new(""),
            Err(SourceDocumentIdError::Empty)
        );
        assert_eq!(
            SourceDocumentId::try_new("good\nnot-good"),
            Err(SourceDocumentIdError::Control { byte: 4 })
        );
        assert_eq!(
            SourceDocumentId::try_new("arcweft-project://game/main.arcw")
                .expect("valid URI")
                .as_str(),
            "arcweft-project://game/main.arcw"
        );
    }

    #[test]
    fn span_exact_range_succeeds() {
        let source = document("manifest", "aéz");
        let span = source
            .span(SourceRange::new(1, 3))
            .expect("exact UTF-8 span");
        assert_eq!(span.source(), source.identity());
        assert_eq!(span.range(), SourceRange::new(1, 3));
    }

    #[test]
    fn span_reversed_fails() {
        assert_eq!(
            document("manifest", "abc").span(SourceRange::new(2, 1)),
            Err(SourceSpanError::Reversed)
        );
    }

    #[test]
    fn boundary_spans_preserve_exact_multibyte_revision() {
        let source = document("manifest", "aéz");
        let start = source.start_span();
        let end = source.end_span();

        assert_eq!(start.range(), SourceRange::new(0, 0));
        assert_eq!(
            end.range(),
            SourceRange::new(source.text().len(), source.text().len())
        );
        assert_eq!(start.source(), source.identity());
        assert_eq!(end.source(), source.identity());
    }

    #[test]
    fn span_out_of_bounds_fails() {
        assert_eq!(
            document("manifest", "abc").span(SourceRange::new(1, 4)),
            Err(SourceSpanError::OutOfBounds)
        );
    }

    #[test]
    fn span_utf8_boundary_fails() {
        assert_eq!(
            document("manifest", "aéz").span(SourceRange::new(2, 3)),
            Err(SourceSpanError::NotUtf8Boundary)
        );
    }

    #[test]
    fn parser_diagnostic_fixture_rejects_invalid_ranges() {
        let source = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
        let document = document("view-export", source);

        assert_eq!(
            document.span(SourceRange::new(54, 47)),
            Err(SourceSpanError::Reversed)
        );
        assert_eq!(
            document.span(SourceRange::new(69, 70)),
            Err(SourceSpanError::OutOfBounds)
        );
        assert_eq!(
            document.span(SourceRange::new(35, 36)),
            Err(SourceSpanError::NotUtf8Boundary)
        );
    }

    #[test]
    fn parser_diagnostic_fixture_rejects_cross_document_and_stale_spans() {
        let source = "pub view Card() {\n    export part タイトル heading\n    Panel()\n}\n";
        let original = document("view-export", source);
        let diagnostic_span = original
            .span(SourceRange::new(47, 54))
            .expect("fixture diagnostic span");
        let edit_span = original
            .span(SourceRange::new(47, 47))
            .expect("fixture insertion span");

        let other = document("other-view-export", source);
        for span in [&diagnostic_span, &edit_span] {
            assert!(matches!(
                span.validate_for(&other),
                Err(SourceSpanValidationError::WrongDocument { expected, actual })
                    if &expected == other.identity().id()
                        && &actual == original.identity().id()
            ));
        }

        let current = document(
            "view-export",
            "pub view Card() {\n    export part タイトル as heading\n    Panel()\n}\n",
        );
        for span in [&diagnostic_span, &edit_span] {
            assert!(matches!(
                span.validate_for(&current),
                Err(SourceSpanValidationError::WrongRevision { expected, actual })
                    if expected == current.identity().revision()
                        && actual == original.identity().revision()
            ));
        }
    }

    #[test]
    fn source_set_revision_is_order_independent_and_fixed_framed() {
        let first = document("a", "one");
        let second = document("b", "two");
        let ordered = SourceSetRevision::try_for_identities([
            first.identity(),
            second.identity(),
            first.identity(),
        ])
        .expect("ordered revision");
        let reversed = SourceSetRevision::try_for_identities([second.identity(), first.identity()])
            .expect("reversed revision");
        assert_eq!(ordered, reversed);
        assert_eq!(
            hex(ordered.as_bytes()),
            "1285968c205c3e92d16320703086656540fe2b4db84a6bfb14f4c3d65aeb6cda"
        );

        let changed = document("a", "changed");
        assert!(matches!(
            SourceSetRevision::try_for_identities([first.identity(), changed.identity()]),
            Err(SourceSetRevisionError::ConflictingDocument { .. })
        ));
    }

    #[test]
    fn source_document_display_name_is_not_identity() {
        let id =
            SourceDocumentId::try_new("arcweft-project://game/main.arcw").expect("document id");
        let path = SourceDocument::try_new(id.clone(), SourceName::path("src/main.arcw"), "éx")
            .expect("path document");
        let generated =
            SourceDocument::try_new(id, SourceName::Generated, "éx").expect("generated document");

        assert_eq!(path.identity(), generated.identity());
        assert_eq!(
            SourceSetRevision::try_for_identities([path.identity()]),
            SourceSetRevision::try_for_identities([generated.identity()])
        );
        assert_eq!(
            path.span(SourceRange::new(0, 2)).expect("path span"),
            generated
                .span(SourceRange::new(0, 2))
                .expect("generated span")
        );
        assert_ne!(path.display_name(), generated.display_name());
    }

    fn hex(bytes: &[u8]) -> String {
        bytes
            .iter()
            .fold(String::with_capacity(bytes.len() * 2), |mut hex, byte| {
                write!(&mut hex, "{byte:02x}").expect("writing to String cannot fail");
                hex
            })
    }
}
