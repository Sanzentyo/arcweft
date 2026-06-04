use crate::positions::{LineIndex, PositionEncoding};
use lsp_types::{DidChangeTextDocumentParams, DidOpenTextDocumentParams, Uri};
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

/// One open text document snapshot.
#[derive(Clone, Debug)]
pub struct DocumentSnapshot {
    uri: Uri,
    version: Option<i32>,
    text: Arc<str>,
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
        &self.text
    }

    /// Source-aware line index for this snapshot.
    pub const fn line_index(&self) -> &LineIndex {
        &self.line_index
    }
}

impl DocumentStore {
    /// Inserts an opened document and returns its snapshot.
    pub fn open(
        &mut self,
        params: DidOpenTextDocumentParams,
        encoding: PositionEncoding,
    ) -> DocumentSnapshot {
        let text: Arc<str> = Arc::from(params.text_document.text);
        let snapshot = DocumentSnapshot {
            uri: params.text_document.uri,
            version: Some(params.text_document.version),
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            text,
        };
        self.documents
            .insert(snapshot.uri.to_string(), snapshot.clone());
        snapshot
    }

    /// Applies a FULL document change and returns the new snapshot.
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
        let snapshot = DocumentSnapshot {
            uri: params.text_document.uri,
            version: Some(params.text_document.version),
            line_index: LineIndex::new(Arc::clone(&text), encoding),
            text,
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
