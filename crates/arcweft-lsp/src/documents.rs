use crate::positions::{LineIndex, PositionEncoding};
use crate::uri_key::LspUriKey;
use arcweft_source::{SourceDocument, SourceDocumentError, SourceDocumentId, SourceName};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Uri};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// One open text document snapshot.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    uri: Uri,
    version: i32,
    document: Arc<SourceDocument>,
    line_index: LineIndex,
}

/// Open document cache for FULL text synchronization.
#[derive(Clone, Debug, Default)]
pub struct DocumentStore {
    documents: BTreeMap<LspUriKey, DocumentSnapshot>,
}

/// Document cache update error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DocumentError {
    /// FULL sync requires exactly one full-text content change.
    #[error("Arcweft LSP currently expects exactly one full-text document change")]
    ExpectedFullSyncChange,
}

/// Failure to bind editor bytes to an accepted project-logical document identity.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OverlayBindingError {
    #[error("open URI does not match the accepted source adapter")]
    UriMismatch { open: String, accepted: String },
    #[error("failed to bind editor bytes to the accepted logical document: {0}")]
    Document(#[from] SourceDocumentError),
}

/// Rebinds one open editor snapshot through an explicit accepted URI adapter.
pub(crate) fn rebind_overlay(
    snapshot: &DocumentSnapshot,
    accepted: &crate::profiles::accepted_project::AcceptedSourceDocument,
) -> Result<Arc<SourceDocument>, OverlayBindingError> {
    let accepted_uri =
        accepted
            .locator()
            .uri()
            .ok_or_else(|| OverlayBindingError::UriMismatch {
                open: snapshot.uri().to_string(),
                accepted: "unavailable".to_owned(),
            })?;
    if accepted_uri != snapshot.uri() {
        return Err(OverlayBindingError::UriMismatch {
            open: snapshot.uri().to_string(),
            accepted: accepted_uri.to_string(),
        });
    }
    Ok(Arc::new(SourceDocument::try_new(
        accepted.document().identity().id().clone(),
        accepted.document().display_name().clone(),
        snapshot.text(),
    )?))
}

impl DocumentSnapshot {
    /// Document URI.
    pub const fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Last client-supplied document version.
    pub const fn version(&self) -> i32 {
        self.version
    }

    /// Current document text.
    pub fn text(&self) -> &str {
        self.document.text()
    }

    /// Exact revision-bound source document lease for this open snapshot.
    pub const fn source_document(&self) -> &Arc<SourceDocument> {
        &self.document
    }

    /// Source-aware line index for this snapshot.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

impl DocumentStore {
    /// Inserts an opened document and returns its snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if an already-validated LSP URI cannot form a source identity,
    /// or if the in-memory document length cannot be represented by the source model.
    pub fn open(
        &mut self,
        params: DidOpenTextDocumentParams,
        encoding: PositionEncoding,
    ) -> DocumentSnapshot {
        let text: Arc<str> = Arc::from(params.text_document.text);
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(params.text_document.uri.to_string())
                    .expect("an LSP URI is a valid source document id"),
                SourceName::path(params.text_document.uri.to_string()),
                Arc::clone(&text),
            )
            .expect("an open LSP document length fits its source identity"),
        );
        let snapshot = DocumentSnapshot {
            uri: params.text_document.uri,
            version: params.text_document.version,
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            document,
        };
        self.documents
            .insert(LspUriKey::from_uri(snapshot.uri()), snapshot.clone());
        snapshot
    }

    /// Applies a FULL document change and returns the new snapshot.
    ///
    /// # Panics
    ///
    /// Panics only if an already-validated LSP URI cannot form a source identity,
    /// or if the in-memory document length cannot be represented by the source model.
    pub fn change(
        &mut self,
        params: DidChangeTextDocumentParams,
        encoding: PositionEncoding,
    ) -> Result<DocumentSnapshot, DocumentError> {
        let mut changes = params.content_changes.into_iter();
        let Some(change) = changes.next() else {
            return Err(DocumentError::ExpectedFullSyncChange);
        };
        if changes.next().is_some() || change.range.is_some() {
            return Err(DocumentError::ExpectedFullSyncChange);
        }
        let text: Arc<str> = Arc::from(change.text);
        let document = Arc::new(
            SourceDocument::try_new(
                SourceDocumentId::try_new(params.text_document.uri.to_string())
                    .expect("an LSP URI is a valid source document id"),
                SourceName::path(params.text_document.uri.to_string()),
                Arc::clone(&text),
            )
            .expect("an open LSP document length fits its source identity"),
        );
        let snapshot = DocumentSnapshot {
            uri: params.text_document.uri,
            version: params.text_document.version,
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            document,
        };
        self.documents
            .insert(LspUriKey::from_uri(snapshot.uri()), snapshot.clone());
        Ok(snapshot)
    }

    /// Removes a closed document.
    pub fn close(&mut self, uri: &Uri) {
        self.documents.remove(&LspUriKey::from_uri(uri));
    }

    /// Gets an open document by URI.
    pub fn get(&self, uri: &Uri) -> Option<&DocumentSnapshot> {
        self.documents.get(&LspUriKey::from_uri(uri))
    }

    pub(crate) fn get_by_key(&self, uri: &LspUriKey) -> Option<&DocumentSnapshot> {
        self.documents.get(uri)
    }

    /// All open document snapshots.
    pub fn snapshots(&self) -> impl Iterator<Item = &DocumentSnapshot> {
        self.documents.values()
    }

    pub(crate) fn clear(&mut self) {
        self.documents.clear();
    }

    pub(crate) fn remove_by_key(&mut self, uri: &LspUriKey) {
        self.documents.remove(uri);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{
        DidChangeTextDocumentParams, TextDocumentContentChangeEvent,
        VersionedTextDocumentIdentifier,
    };

    #[test]
    fn full_sync_updates_document_text_and_version() {
        let uri = "file:///story.arcw".parse::<Uri>().expect("uri");
        let mut store = DocumentStore::default();

        let snapshot = store
            .change(
                DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri: uri.clone(),
                        version: 2,
                    },
                    content_changes: vec![TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: "flow @flow.opening opening {}".to_owned(),
                    }],
                },
                PositionEncoding::Utf16,
            )
            .expect("full sync change");

        assert_eq!(snapshot.version(), 2);
        let retained = store.get(&uri).expect("changed document is retained");
        assert!(Arc::ptr_eq(
            snapshot.source_document(),
            retained.source_document()
        ));
    }
}
