use crate::positions::{LineIndex, PositionEncoding};
use arcweft_source::{SourceDocument, SourceDocumentId, SourceName};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Uri};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// One open text document snapshot.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    uri: Uri,
    version: Option<i32>,
    document: Arc<SourceDocument>,
    line_index: LineIndex,
}

/// Open document cache for FULL text synchronization.
#[derive(Clone, Debug, Default)]
pub struct DocumentStore {
    documents: BTreeMap<String, DocumentSnapshot>,
}

/// Document cache update error.
#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum DocumentError {
    /// FULL sync requires exactly one full-text content change.
    #[error("Arcweft LSP currently expects exactly one full-text document change")]
    ExpectedFullSyncChange,
}

impl DocumentSnapshot {
    /// Document URI.
    pub const fn uri(&self) -> &Uri {
        &self.uri
    }

    /// Last client-supplied document version.
    pub const fn version(&self) -> Option<i32> {
        self.version
    }

    /// Current document text.
    pub fn text(&self) -> &str {
        self.document.text()
    }

    /// Exact revision-bound source document for this open snapshot.
    pub fn source_document(&self) -> &SourceDocument {
        self.document.as_ref()
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
            version: Some(params.text_document.version),
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            document,
        };
        self.documents
            .insert(snapshot.uri.to_string(), snapshot.clone());
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
            version: Some(params.text_document.version),
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            document,
        };
        self.documents
            .insert(snapshot.uri.to_string(), snapshot.clone());
        Ok(snapshot)
    }

    /// Removes a closed document.
    pub fn close(&mut self, uri: &Uri) {
        self.documents.remove(&uri.to_string());
    }

    /// Gets an open document by URI.
    pub fn get(&self, uri: &Uri) -> Option<&DocumentSnapshot> {
        self.documents.get(&uri.to_string())
    }

    /// All open document snapshots.
    pub fn snapshots(&self) -> impl Iterator<Item = &DocumentSnapshot> {
        self.documents.values()
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

        assert_eq!(snapshot.version(), Some(2));
        assert!(store.get(&uri).is_some());
    }
}
